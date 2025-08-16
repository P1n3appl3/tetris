{
  description = "tetris and related tools";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, flake-utils, crane, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
      craneLib = (crane.mkLib pkgs).overrideToolchain (p:
        p.rust-bin.stable.latest.default.override {
          targets = [ "wasm32-unknown-unknown" ];
      });
      inherit (pkgs) lib;

      darwinArgs = {
        # `coreaudio-sys` calls `bindgen` at build time _always_ 😞
        LIBCLANG_PATH="${pkgs.llvmPackages.libclang.lib}/lib";
        # might still need to set COREAUDIO_SDK_PATH to $SDK_ROOT?
      };

      addBindgenEnvVar = base: base.overrideAttrs (old: {
          # (the above fixes for the macOS framework paths do not seem to be
          # enough anymore; poking at `cbindgen` reveals that it now seems to
          # "sanitize" our NIX_CFLAGS_COMPILE injected `-iframework` includes
          # and turn them into regular `-isystem` includes, breaking
          # framework headers. to get around this we use the inelegant
          # "big hammer" solution below: forcibly reintroducing our
          # `-iframework` flags to `cbindgen`'s list of flags passed to
          # `libclang`):
          preBuild = (old.preBuild or "") +
            pkgs.lib.optionalString (pkgs.stdenv.isDarwin) ''
              export BINDGEN_EXTRA_CLANG_ARGS="$NIX_CFLAGS_COMPILE"
            '';
      });

      src = let
        kdlFilter = path: _type: builtins.match ".*kdl$" path != null;
        kdlOrCargo = path: type:
          (kdlFilter path type) || (craneLib.filterCargoSources path type);
      in lib.cleanSourceWith { src = ./.; filter = kdlOrCargo; name = "source"; };

      commonArgs = {
        inherit src;
        strictDeps = true;

        nativeBuildInputs = lib.optionals pkgs.stdenv.isLinux (with pkgs; [
          pkg-config makeWrapper
        ]);

        buildInputs = with pkgs; [ libiconv openssl ] ++
          (lib.optional (stdenv.isLinux) alsa-lib) ++
          (lib.optional (stdenv.isDarwin) pkgs.xcbuild);
      } // (pkgs.lib.optionalAttrs (pkgs.stdenv.isDarwin) darwinArgs);

      cargoArtifacts = addBindgenEnvVar (craneLib.buildDepsOnly commonArgs);

      tetris = craneLib.buildPackage (commonArgs // {
        inherit cargoArtifacts;
        doCheck = false; # there's a separate check for that
      } // {
        cargoExtraArgs = "-p tui";
        postInstall = ''
          mv $out/bin/tui $out/bin/tetris
        '' + (pkgs.lib.optionalString (pkgs.stdenv.isLinux) ''
          wrapProgram $out/bin/tetris \
            --set-default "ALSA_PLUGIN_DIR" "${pkgs.alsa-plugins}/lib/alsa-lib"
        '');
      });

      # jstrisSprint = pkgs.writeShellApplication {
      #   name = "jstris-sprint";
      #   runtimeInputs = jjstrisScriptDeps;
      #   text = ./jstris-sprint.sh;
      # };

      jstrisScriptDeps = with pkgs; [
        xh pup jq bc sd choose moreutils gnuplot
        (python3.withPackages (pp: with pp;[matplotlib]))
      ];

    in {
      packages.default = tetris;

      checks = {
        build = tetris;
        test = craneLib.cargoNextest (commonArgs // {
          inherit cargoArtifacts;
          cargoNextestExtraArgs = "--workspace";
        });
        clippy = addBindgenEnvVar (craneLib.cargoClippy (commonArgs // {
          inherit cargoArtifacts;
          cargoClippyExtraArgs = "--all-targets --workspace -- --deny warnings";
        }));
        doc = craneLib.cargoDoc (commonArgs // {
          inherit cargoArtifacts;
          cargoDocExtraArgs = "--no-deps -p tetris";
        });
      };

      devShells.default = craneLib.devShell {
        inputsFrom = [ tetris ];
        checks = self.checks.${system};
        INTER="${pkgs.inter}/share/fonts/truetype/InterVariable.ttf";
        packages = with pkgs; [
          rust-analyzer
          inter
          wasm-pack minify
          static-web-server
        ] ++ jstrisScriptDeps;
      };
    }
  );
}
