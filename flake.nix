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
        p.rust-bin.stable.latest.default);
      inherit (pkgs) lib;

      darwinArgs = {
        # `coreaudio-sys` calls `bindgen` at build time _always_ 😞
        LIBCLANG_PATH="${pkgs.llvmPackages.libclang.lib}/lib";

        # `coreaudio-sys` expects the headers for this packages to be available
        # in a directory structure matching the MacOS SDKs.
        #
        # If we don't set this env var, `coreaudio-sys` will ask `xcrun` for the
        # MacOS SDK path and the nix `xcrun` wrapper just points to a stub SDK
        # without any frameworks or headers: https://github.com/NixOS/nixpkgs/blob/bcf1085724f62e860f2cddd2c6eaee7dceb22888/pkgs/development/tools/xcbuild/wrapper.nix#L54
        #
        # See:
        #  - https://discourse.nixos.org/t/develop-shell-environment-setup-for-macos/11399/6
        #  - https://github.com/RustAudio/coreaudio-sys/blob/8185e0704754a0d3e3c41e9557d24f5f406ce5ef/build.rs#L6
        COREAUDIO_SDK_PATH = pkgs.symlinkJoin {
          name = "sdk";
          # See: https://github.com/RustAudio/coreaudio-sys/blob/8185e0704754a0d3e3c41e9557d24f5f406ce5ef/build.rs#L50-L102
          paths = with pkgs.darwin.apple_sdk.frameworks;
            [ AudioToolbox AudioUnit CoreAudio CoreFoundation CoreMIDI OpenAL ];
          postBuild = ''
            mkdir $out/System
            mv $out/Library $out/System
          '';
        };
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

      src = craneLib.cleanCargoSource ./.;

      commonArgs = {
        inherit src;
        strictDeps = true;

        nativeBuildInputs = lib.optionals pkgs.stdenv.isLinux (with pkgs; [
          pkg-config makeWrapper alsa-lib openssl
        ]);

        buildInputs = with pkgs; [ libiconv ] ++
          (lib.optional (stdenv.isLinux) alsa-lib) ++
          (lib.optionals (stdenv.isDarwin)
            (with darwin.apple_sdk.frameworks; [ AudioUnit CoreAudio pkgs.xcbuild ]));
      } // (pkgs.lib.optionalAttrs (pkgs.stdenv.isDarwin) darwinArgs);

      cargoArtifacts = addBindgenEnvVar (craneLib.buildDepsOnly commonArgs);

      tetris = craneLib.buildPackage (commonArgs // {
        inherit cargoArtifacts;
        doCheck = false; # there's a separate check for that
      } // (pkgs.lib.optionalAttrs (pkgs.stdenv.isLinux) {
        postInstall = ''
          wrapProgram $out/bin/tetris \
            --set-default "ALSA_PLUGIN_DIR" "${pkgs.alsa-plugins}/lib/alsa-lib"
        '';
      }));

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
        test = craneLib.cargoNextest (commonArgs // { inherit cargoArtifacts; });
        clippy = addBindgenEnvVar (craneLib.cargoClippy (commonArgs // {
          inherit cargoArtifacts;
          cargoClippyExtraArgs = "--all-targets -- --deny warnings";
        }));
        doc = craneLib.cargoDoc (commonArgs // { inherit cargoArtifacts; });
        fmt = craneLib.cargoFmt { inherit src; };
        toml-fmt = craneLib.taploFmt {
          src = pkgs.lib.sources.sourceFilesBySuffices src [ ".toml" ];
        };
      };

      devShells.default = craneLib.devShell {
        inputsFrom = [ tetris ];
        checks = self.checks.${system};
        packages = with pkgs; [ rust-analyzer ] ++ jstrisScriptDeps;
      };
    }
  );
}
