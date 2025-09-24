{ lib
, buildPackages
, craneLib
, stdenv
, windows
, libgit2
, pkg-config
, builtInfo ? {}
, features ? []
, buildType ? "release"
, enableBuilt ? builtInfo == {} && (! source ? sourceInfo.rev) && (! source ? sourceInfo.dirtyRev)
, enableLibgit ? enableBuilt && lib.versionAtLeast libgit2.version "1.9.0"
, enableCache ? true
, enableMarkers ? true
, enableSpace ? true
, enableNexus ? true
, enableArcdps ? true
, source ? ./.
}: let
  inherit (lib.trivial) mapNullable;
  inherit (lib.lists) optional optionals;
  inherit (lib.strings) concatStringsSep optionalString;
  libgit2'build = libgit2.__spliced.buildHost or buildPackages.libgit2 or libgit2;
  #TARGET_CC = "${pkgsCross.stdenv.cc}/bin/${pkgsCross.stdenv.cc.targetPrefix}cc";
  #TARGET_CC = "${stdenv.cc.targetPrefix}cc";
  builtInfo' = let
    platform = builtInfo.platform or null;
    ref = builtInfo.ref or null;
    rev = builtInfo.rev or source.sourceInfo.rev or source.sourceInfo.dirtyRev or null;
    shortRev = builtInfo.shortRev or source.sourceInfo.shortRev or source.sourceInfo.dirtyShortRev or (mapNullable (builtins.substring 0 8));
    revSuffix = optionalString (builtInfo.dirty or (source ? sourceInfo.dirtyRev)) "-dirty";
  in {
    ${mapNullable (_: "BUILT_OVERRIDE_taimi_hud_CI_PLATFORM") platform} = platform;
    ${mapNullable (_: "BUILT_OVERRIDE_taimi_hud_GIT_HEAD_REF") ref} = ref;
    ${mapNullable (_: "BUILT_OVERRIDE_taimi_hud_GIT_COMMIT_HASH") rev} = rev + revSuffix;
    ${mapNullable (_: "BUILT_OVERRIDE_taimi_hud_GIT_COMMIT_HASH_SHORT") shortRev} = shortRev + revSuffix;
  };
  cargoBuildFeatures = features
    ++ optionals enableMarkers [
      "markers"
      "markers-edit"
    ] ++ optional stdenv.hostPlatform.isWindows "windows"
    ++ optional enableNexus "extension-nexus"
    ++ optional enableArcdps "extension-arcdps"
    ++ optional enableCache "meta-cache"
    ++ optional enableSpace "space"
    ++ optional enableLibgit "built-info";
in craneLib.buildPackage ({
  src = source;
  strictDeps = true;
  cargoExtraArgs = optionalString (cargoBuildFeatures != []) (toString
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

  nativeBuildInputs = [
    buildPackages.stdenv.cc
  ] ++ optional enableLibgit libgit2'build;

  doCheck = false;

  LIBGIT2_NO_VENDOR = true;

  # Tells Cargo that we're building for Windows.
  # (https://doc.rust-lang.org/cargo/reference/config.html#buildtarget)
  CARGO_BUILD_TARGET = "x86_64-pc-windows-gnu";

  # Build without a dependency not provided by wine
  CXXFLAGS_x86_64_pc_windows_gnu = "-Oz -shared -fno-threadsafe-statics";
  CARGO_PROFILE = buildType;
  #CARGO_BUILD_RUSTFLAGS = ["-C" "linker=${TARGET_CC}"];
} // builtInfo')
