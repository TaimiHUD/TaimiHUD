{
  inputs ? (import ./default.nix).inputs,
  legacyPackages ? inputs.self.legacyPackages.${system},
  system ? builtins.currentSystem,
  callPackage ? legacyPackages.callPackage or legacyPackages.pkgs.callPackage,
}: let
  inherit (legacyPackages) git-hooks formatter;
  taimiShell = {
    mkShell,
    lib,
    taimiHUD,
    fenixToolchainShell,
    buildPackages,
    stdenv,
    windows ? {},
    libgit2,
    pkg-config,
  }: let
    libgit2'build = libgit2.__spliced.buildHost or buildPackages.libgit2 or libgit2;
    TARGET_CC = "${stdenv.cc.targetPrefix}cc";
    inherit (taimiHUD) LD_LIBRARY_PATH;
  in
    mkShell {
      allowSubstitutes = false;
      buildInputs =
        [
          stdenv.cc
        ]
        ++ lib.optional stdenv.hostPlatform.isWindows windows.pthreads;

      depsBuildBuild = [
        pkg-config
      ];

      nativeBuildInputs = [
        buildPackages.stdenv.cc
        libgit2'build
        fenixToolchainShell
        git-hooks.package
        git-hooks.installationScriptBin
        formatter
      ];

      shellHook = ''
        if [[ -n ''${FLAKE_OPT_HOOKS_INSTALL-1} ]]; then
          git-hooks-install
        fi
        export LD_LIBRARY_PATH="''${LD_LIBRARY_PATH-}:${LD_LIBRARY_PATH}";
        export "PKG_CONFIG_PATH_${lib.replaceStrings ["-"] ["_"] stdenv.buildPlatform.config}=$PKG_CONFIG_PATH_FOR_BUILD"
        export "PKG_CONFIG_${lib.replaceStrings ["-"] ["_"] stdenv.buildPlatform.config}=$PKG_CONFIG_FOR_BUILD"
      '';

      inherit (taimiHUD) LIBGIT2_NO_VENDOR CXXFLAGS_x86_64_pc_windows_gnu CARGO_BUILD_TARGET;
      inherit TARGET_CC;
      CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = TARGET_CC;
      # may be needed for rustfmt...
      #RUSTC_BOOTSTRAP = 1;
    };
in {
  taimiShell = callPackage taimiShell {};
  taimiShell-minimal = callPackage taimiShell {
    fenixToolchainShell = with legacyPackages.fenixPackages;
      combine [
        legacyPackages.fenixToolchainShell
        legacyPackages.fenixChannel.rust-src
        rust-analyzer
      ];
  };
  taimiShell-build = callPackage taimiShell {
    fenixToolchainShell = legacyPackages.fenixToolchain;
  };
  taimiShell-native = legacyPackages.pkgs.buildPackages.callPackage taimiShell {
    inherit (legacyPackages) fenixToolchainShell;
    taimiHUD = {
      LD_LIBRARY_PATH = "";
      LIBGIT2_NO_VENDOR = null;
      CXXFLAGS_x86_64_pc_windows_gnu = null;
      CARGO_BUILD_TARGET = null;
    };
  };
}
