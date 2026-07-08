{
  lib,
  buildPackages,
  craneLib,
  stdenv,
  windows,
  libgit2,
  pkg-config,
  lua,
  arcdps-imgui_18000,
  arcdps-imgui_19270,
  builtInfo ? {},
  features ? [],
  buildType ? "release",
  # compile-time features
  enableBuilt ? (builtInfo == {} && (! source ? sourceInfo.rev) && (! source ? sourceInfo.dirtyRev)) || (builtInfo.platform or false == null),
  enableLibgit ? enableBuilt && lib.versionAtLeast libgit2.version "1.9.0",
  enableUpdates ? builtInfo != {} || (enableBuilt && (! source ? sourceInfo.dirtyRev)),
  enableStatistics ? false,
  enableEnvFilter ? false,
  enableCache ? true,
  enableSpace ? enablePaths,
  enableLua ? enableExperimentalFeatures,
  # components
  enableTimers ? true,
  enableTimersSpace ? true,
  enableMarkers ? true,
  enablePaths ? true,
  enablePathsApi ? true,
  # XXX: could be enabled prior to stabilizing interactions but mostly pointless without it...
  enablePathsFilter ? enableExperimentalFeatures,
  enablePathsInteract ? enableExperimentalFeatures,
  enablePathsEdit ? enableExperimentalFeatures,
  enableScripts ? enableExperimentalFeatures,
  # hosts
  enableNexus ? true,
  enableArcdps ? true,
  # enable presets
  enableExperimentalFeatures ? false,
  # build opts
  buildWithDebugInfo ?
    if stdenv.hostPlatform.isMsvc
    then true
    else null,
  buildWithUnwind ? null,
  buildWithLto ? null,
  buildCheckOnly ? false,
  source ? ./.,
  cargoArtifacts ? null,
}: let
  inherit (lib.trivial) mapNullable defaultTo;
  inherit (lib.lists) optional optionals;
  inherit (lib.strings) concatStringsSep optionalString toUpper isStringLike;
  inherit (stdenv.hostPlatform) isWindows;
  mkProfileKey = key: "CARGO_PROFILE_${toUpper buildType}_${key}";
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
  cargoBuildFeatures' = let
    # currently required regardless
    enablePathsSpace = enableSpace || enablePaths;
    # depends on paths space...
    enableTimersSpace' = enableTimersSpace && enablePathsSpace;
    # non-optional
    enableMarkersEdit = true;
    # depends on paths-dyn atm...
    enableScripts' = enablePaths && enableScripts;
    featuresTimers =
      ["timers"]
      ++ optional enableTimersSpace' "timers-space";
    featuresPaths =
      ["paths"]
      ++ optional enablePathsApi "paths-api"
      ++ optional enablePathsSpace "paths-space"
      ++ optional enablePathsFilter "paths-filter"
      ++ optional enablePathsInteract "paths-interact"
      ++ optional enablePathsEdit "paths-edit"
      ++ optional enableLua "paths-lua";
    featuresMarkers =
      ["markers"]
      ++ optional enableMarkersEdit "markers-edit";
    featuresScripts =
      ["scripts"]
      ++ optional enableLua "scripts-lua";
    # hosts
    featuresNexus = ["extension-nexus"];
    featuresArcdps = ["extension-arcdps"];
  in
    features
    ++ optionals enableTimers featuresTimers
    ++ optionals enableMarkers featuresMarkers
    ++ optionals enablePaths featuresPaths
    ++ optionals enableScripts' featuresScripts
    ++ optional isWindows "windows"
    ++ optionals enableNexus featuresNexus
    ++ optionals enableArcdps featuresArcdps
    ++ optional enableStatistics "statistics"
    ++ optional enableEnvFilter "env-filter"
    ++ optional enableCache "meta-cache"
    ++ optional enableUpdates "updates"
    ++ optional enableLibgit "built-info";
  cargoBuildFeatures = lib.unique cargoBuildFeatures';
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
      cargoExtraArgs = optionalString (cargoBuildFeatures' != []) (
        toString
        ["--no-default-features" "--features" (concatStringsSep "," cargoBuildFeatures)]
      );

      CARGO_BUILD_INCREMENTAL = "false";
      ${
        if buildWithUnwind != null || buildType != "dev"
        then mkProfileKey "PANIC"
        else null
      } =
        if !defaultTo false buildWithUnwind
        then "abort"
        else toString buildWithUnwind;
      ${mapNullable (_: mkProfileKey "LTO") buildWithLto} = toString buildWithLto;
      ${mapNullable (_: mkProfileKey "DEBUG") buildWithDebugInfo} =
        if isStringLike buildWithDebugInfo
        then toString buildWithDebugInfo
        else if !buildWithDebugInfo
        then "off"
        else if buildType == "dev"
        then "limited"
        else "line-tables-only";

      outputs =
        ["out"]
        ++ optional (defaultTo false buildWithDebugInfo) "debug";

      buildInputs =
        [
          stdenv.cc
          windows.pthreads
          arcdps-imgui_18000.cimgui-static
          arcdps-imgui_19270.cimgui-static
        ]
        ++ optional enableLua lua;

      depsBuildBuild =
        optional enableLibgit pkg-config;

      LD_LIBRARY_PATH = lib.makeLibraryPath (
        optional enableLibgit libgit2'build
      );

      nativeBuildInputs =
        [
          buildPackages.stdenv.cc
          pkg-config
        ]
        ++ optional enableLibgit libgit2'build;

      ${
        if buildCheckOnly
        then "cargoBuildCommand"
        else null
      } = "cargoWithProfile check --workspace";
      ${
        if buildCheckOnly
        then "installPhaseCommand"
        else null
      } = "touch $out";
      #doCheck = false;
      #doInstallCheck = false;

      LIBGIT2_NO_VENDOR = true;
      preConfigure = optionalString (stdenv.hostPlatform.config != stdenv.buildPlatform.config) ''
        if [[ -n ''${PKG_CONFIG_PATH_FOR_BUILD-} ]]; then
          export "PKG_CONFIG_PATH_${lib.replaceStrings ["-"] ["_"] stdenv.buildPlatform.config}=$PKG_CONFIG_PATH_FOR_BUILD"
        fi
        if [[ -n ''${PKG_CONFIG_FOR_BUILD-} ]]; then
          export "PKG_CONFIG_${lib.replaceStrings ["-"] ["_"] stdenv.buildPlatform.config}=$PKG_CONFIG_FOR_BUILD"
        fi
      '';

      # Tells Cargo that we're building for Windows.
      # (https://doc.rust-lang.org/cargo/reference/config.html#buildtarget)
      CARGO_BUILD_TARGET = "x86_64-pc-windows-gnu";

      # Build without a dependency not provided by wine
      CXXFLAGS_x86_64_pc_windows_gnu = "-Oz -shared -fno-threadsafe-statics";
      CARGO_PROFILE = buildType;
      CARGO_BUILD_RUSTFLAGS = [
        #"-C" "linker=${TARGET_CC}"
        # avoid "unimplemented function combase.dll.RoOriginateErrorW, aborting" on wine .-.
        "--cfg=windows_slim_errors"
      ];
      #RUSTC_BOOTSTRAP = 1; # tobj/merging?
      passthru = {
        CARGO_FEATURES = cargoBuildFeatures;
      };
    }
    // builtInfo')
