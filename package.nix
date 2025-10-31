{
  lib,
  buildPackages,
  craneLib,
  stdenv,
  windows,
  libgit2,
  pkg-config,
  builtInfo ? {},
  features ? [],
  doCheck ? false,
  buildType ? "release",
  enableBuilt ? (builtInfo == {} && (! source ? sourceInfo.rev) && (! source ? sourceInfo.dirtyRev)) || (builtInfo.platform or false == null),
  enableLibgit ? enableBuilt && lib.versionAtLeast libgit2.version "1.9.0",
  enableUpdates ? builtInfo != {} || (enableBuilt && (! source ? sourceInfo.dirtyRev)),
  enableStatistics ? false,
  enableEnvFilter ? false,
  enableCache ? true,
  enableTimers ? true,
  enableMarkers ? true,
  enableSpace ? true,
  enableNexus ? true,
  enableArcdps ? true,
  source ? ./.,
  cargoArtifacts ? null,
}: let
  inherit (lib.trivial) mapNullable;
  inherit (lib.lists) optional optionals;
  inherit (lib.strings) concatStringsSep optionalString;
  libgit2'build = libgit2.__spliced.buildHost or buildPackages.libgit2 or libgit2;
  #TARGET_CC = "${pkgsCross.stdenv.cc}/bin/${pkgsCross.stdenv.cc.targetPrefix}cc";
  #TARGET_CC = "${stdenv.cc.targetPrefix}cc";
  builtInfo' = let
    platform =
      builtInfo.platform
      or (
        if source ? sourceInfo
        then "flake"
        else "nix"
      );
    ref = builtInfo.ref or null;
    rev = builtInfo.rev or source.sourceInfo.rev or source.sourceInfo.dirtyRev or null;
    shortRev = builtInfo.shortRev or source.sourceInfo.shortRev or source.sourceInfo.dirtyShortRev or (mapNullable (builtins.substring 0 8) rev);
    revSuffix = optionalString (builtInfo.dirty or (source ? sourceInfo.dirtyRev)) "-dirty";
  in {
    ${mapNullable (_: "BUILT_OVERRIDE_taimi_hud_CI_PLATFORM") platform} = platform;
    ${mapNullable (_: "BUILT_OVERRIDE_taimi_hud_GIT_HEAD_REF") ref} = ref;
    ${mapNullable (_: "BUILT_OVERRIDE_taimi_hud_GIT_COMMIT_HASH") rev} = rev + revSuffix;
    ${mapNullable (_: "BUILT_OVERRIDE_taimi_hud_GIT_COMMIT_HASH_SHORT") shortRev} = shortRev + revSuffix;
  };
  cargoBuildFeatures =
    features
    ++ optional enableTimers "timers"
    ++ optionals enableMarkers [
      "markers"
      "markers-edit"
    ]
    ++ optional stdenv.hostPlatform.isWindows "windows"
    ++ optional enableNexus "extension-nexus"
    ++ optional enableArcdps "extension-arcdps"
    ++ optional enableStatistics "statistics"
    ++ optional enableEnvFilter "env-filter"
    ++ optional enableCache "meta-cache"
    ++ optional enableSpace "space"
    ++ optional enableUpdates "updates"
    ++ optional enableLibgit "built-info";
  /*
    dummySrc = let
    manifestSrc = builtins.path {
      inherit (dummySrc) name;
      path = source;
      filter = path: type:
        type
        == "directory"
        || builtins.elem (baseNameOf path) [
          "Cargo.toml"
          "Cargo.lock"
          "build.rs"
        ];
    };
    workspaceCrates = [
      "."
      "rt/input"
      "pathing/meta"
      "pathing/pack"
      "space/d3d"
    ];
  in symlinkJoin {
    name = "taimihud-src-deps";
    inherit workspaceCrates;
    paths = [
      manifestSrc
    ];
    postBuild = ''
      for crateroot in $workspaceCrates; do
        if [[ $crateroot = . ]]; then
          cargosrc=$(readlink -f $out/$crateroot/Cargo.toml)
          touch $out/$crateroot/src/lib.rs

          rm $out/$crateroot/Cargo.toml
          sed \
            -e '/^.* path = "/d' \
            -e '/dep:taimi/d' \
            -e '/"taimi_.*\?\//d' \
            $cargosrc > $out/$crateroot/Cargo.toml

          # ensure lockfile is writable
          # (it will try to remove workspace members .-.)
          rm $out/$crateroot/Cargo.lock
          cat $(dirname $cargosrc)/Cargo.lock > $out/$crateroot/Cargo.lock
        else
          # remove from workspace members list
          sed -i \
            -e "s|^  \"$crateroot\",$||" \
            $out/Cargo.toml
          rm $out/$crateroot/Cargo.toml
        fi
      done
    '';
  };
  */
in
  craneLib.buildPackage ({
      src = source;
      #inherit dummySrc;
      strictDeps = true;
      ${
        if cargoArtifacts != null
        then "cargoArtifacts"
        else null
      } =
        cargoArtifacts;
      cargoExtraArgs = optionalString (cargoBuildFeatures != []) (
        toString
        ["--no-default-features" "--features" (concatStringsSep "," (lib.unique cargoBuildFeatures))]
      );

      buildInputs = [
        stdenv.cc
        windows.pthreads
      ];

      depsBuildBuild =
        optional enableLibgit pkg-config;

      LD_LIBRARY_PATH = lib.makeLibraryPath (
        optional enableLibgit libgit2'build
      );

      nativeBuildInputs =
        [
          buildPackages.stdenv.cc
        ]
        ++ optional enableLibgit libgit2'build;

      ${
        if doCheck
        then "cargoBuildCommand"
        else null
      } = "cargoWithProfile check --workspace";
      ${
        if doCheck
        then "installPhaseCommand"
        else null
      } = "touch $out";

      LIBGIT2_NO_VENDOR = true;

      # Tells Cargo that we're building for Windows.
      # (https://doc.rust-lang.org/cargo/reference/config.html#buildtarget)
      CARGO_BUILD_TARGET = "x86_64-pc-windows-gnu";

      # Build without a dependency not provided by wine
      CXXFLAGS_x86_64_pc_windows_gnu = "-Oz -shared -fno-threadsafe-statics";
      CARGO_PROFILE = buildType;
      #CARGO_BUILD_RUSTFLAGS = ["-C" "linker=${TARGET_CC}"];
    }
    // builtInfo')
