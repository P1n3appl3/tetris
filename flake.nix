{
  description = "P1n3appl3's terminal tetris repo";

  inputs = {
    flu.url = github:numtide/flake-utils;
    nixpkgs.url = github:nixos/nixpkgs/nixos-unstable;
    crane = {
      url = github:ipetkov/crane;
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs@{ flu, nixpkgs, ... }: with flu.lib; eachDefaultSystem (system: let
    np = import nixpkgs { inherit system; };
    crane = inputs.crane.lib.${system};
    inherit (np) lib;

    tetrisDeps = {
      nativeBuildInputs = lib.optionals np.stdenv.isLinux (with np; [
        pkg-config alsa-lib
      ]);
      buildInputs = with np;
        [ libiconv ] ++
        (lib.optional (stdenv.isDarwin)
          (with darwin.apple_sdk.frameworks; [ AudioUnit CoreAudio np.xcbuild ]));
    } // (np.lib.optionalAttrs (np.stdenv.isDarwin) {
      # `coreaudio-sys` calls `bindgen` at build time _always_ :(
      LIBCLANG_PATH="${np.llvmPackages_11.libclang.lib}/lib";

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
      COREAUDIO_SDK_PATH = np.symlinkJoin {
        name = "sdk";
        paths = with np.darwin.apple_sdk.frameworks; [
          # See: https://github.com/RustAudio/coreaudio-sys/blob/8185e0704754a0d3e3c41e9557d24f5f406ce5ef/build.rs#L50-L102
          AudioToolbox
          AudioUnit
          CoreAudio
          CoreFoundation
          CoreMIDI
          OpenAL
        ];
        postBuild = ''
          mkdir $out/System
          mv $out/Library $out/System
        '';
      };
    });

    tetris = let
      addBindgenEnvVar = base: base.overrideAttrs (old: {
          # (the above fixes for the macOS framework paths do not seem to be
          # enough anymore; poking at `cbindgen` reveals that it now seems to
          # "sanitize" our NIX_CFLAGS_COMPILE injected `-iframework` includes
          # and turn them into regular `-isystem` includes, breaking
          # framework headers. to get around this we use the inelegant
          # "big hammer" solution below: forcibly reintroducing our
          # `-iframework` flags to `cbindgen`'s list of flags passed to
          # `libclang`):
          preBuild = (old.preBuild or "") + np.lib.optionalString (np.stdenv.isDarwin) ''
            export BINDGEN_EXTRA_CLANG_ARGS="$NIX_CFLAGS_COMPILE"
          '';
        });
    in rec {
      commonArgs = tetrisDeps // {
        src = crane.cleanCargoSource ./.;
      };

      cargoArtifacts = let
        base = crane.buildDepsOnly commonArgs;
      in addBindgenEnvVar base;

      package = let
        base = crane.buildPackage (commonArgs // {
          inherit cargoArtifacts;
        });
      # See above.
      #
      # The build script on `coreaudio-sys` will run again if
      # `$BINDGEN_EXTRA_CLANG_ARGS` changes so we set it here too:
      in addBindgenEnvVar base;

      clippy = let
        base = crane.cargoClippy (commonArgs // {
          inherit cargoArtifacts;
          cargoClippyExtraArgs = "--all-targets -- --deny warnings";
        });
      in addBindgenEnvVar base;
    };

    jstrisGraphPackage = let
      py = np.python3.withPackages (ps: with ps;
        [ beautifulsoup4 requests statistics matplotlib ]
      );

      gruvbox = np.fetchurl {
        url = "https://raw.githubusercontent.com/thriveth/gruvbox-mpl/f7079ddfba4cae31e5f3e3c1c98d53bf866a3c5f/mpl/gruvbox.mplstyle";
        sha256 = "sha256-R8gIaNTcNEFsh/1mkgb3j5lfkzfhGuTYuNOddhlEhls=";
      };
    in np.stdenv.mkDerivation {
      pname = "jstrisGraph";
      version = "0.1.0";

      src = ./jstris.py;
      unpackPhase = "true";
      installPhase = ''
        mkdir -p $out/bin

        cp $src $out/jstris.py

        # because `jstris.py` looks for `gruvbox` in the PWD and because it also
        # expects the current directory to be writeable, we have to use a
        # tempdir, a nix store dir won't work

        cat > $out/bin/jstrisGraph <<'EOF'
        #!${np.bash}/bin/bash
        set -e
        export PATH="${np.coreutils}/bin"
        cd "$(mktemp -d)"
        ln -s "${gruvbox}" gruvbox
        exec "${py}/bin/python3" ${builtins.placeholder "out"}/jstris.py "$@"
        EOF

        chmod +x $out/bin/jstrisGraph
      '';
    };

  in let t = tetris; in {
    packages = rec {
      tetris = t.package;
      default = tetris;
    };
    apps = rec {
      tetris = { type = "app"; program = np.lib.getExe t.package; };
      default = tetris;
      jstrisGraph = { type = "app"; program = np.lib.getExe jstrisGraphPackage; };
    };
    devShells.default = np.mkShell {
      inputsFrom = [ tetris.package jstrisGraphPackage ];
    };
    checks = {
      inherit (t) package clippy;
    };
  });
}
