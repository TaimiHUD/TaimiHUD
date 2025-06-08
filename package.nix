{ lib
, buildPackages
, craneLib
, stdenv
, windows
, libgit2
, pkg-config
, features ? []
, buildType ? "release"
, enableLibgit ? lib.versionAtLeast libgit2.version "1.9.0"
, enableBuilt ? enableLibgit
}: let
  inherit (lib.lists) optional;
  libgit2'build = libgit2.__spliced.buildHost or buildPackages.libgit2 or libgit2;
  #TARGET_CC = "${pkgsCross.stdenv.cc}/bin/${pkgsCross.stdenv.cc.targetPrefix}cc";
  #TARGET_CC = "${stdenv.cc.targetPrefix}cc";
  cargoBuildFeatures = features
    ++ optional enableBuilt "built-info";
in craneLib.buildPackage {
  src = ./.;
  strictDeps = true;
  cargoExtraArgs = if cargoBuildFeatures != [] then lib.escapeShellArgs (
    ["--features"] ++ (lib.unique cargoBuildFeatures)
  ) else "";

  buildInputs = [
    stdenv.cc
    windows.pthreads
  ];

  depsBuildBuild = [
    pkg-config
  ];

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
}
