{
  description = "TaimiHUD; timers, markers and hopefully paths for raidcore.gg nexus";
  inputs = {
    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils = {
      url = "github:numtide/flake-utils";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay/master";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, fenix, flake-utils, crane, nixpkgs, rust-overlay, ... }@inputs:
    flake-utils.lib.eachDefaultSystem (system:
      let
        legacyPackages = self.legacyPackages.${system};
        packages = self.packages.${system};
        devShells = self.devShells.${system};
        inherit (legacyPackages) pkgs callPackage fenixPackages;

      in
      {
        # TaimiHUD Package
        packages = {
          taimiHUD = callPackage ./package.nix {};
          taimiHUD-debug = packages.taimiHUD.override {
            buildType = "dev";
          };

          packs = callPackage ./pathing/pack/taco.nix;

          default = packages.taimiHUD;
        };

        # TaimiHUD devShell
        devShells = import ./devShells.nix {
          inherit inputs system;
        } // {
          default = devShells.taimiShell;
        };

        legacyPackages = {
          pkgs = (import nixpkgs) {
            inherit system;
            crossSystem.config = "x86_64-w64-mingw32";
          };
          callPackage = pkgs.newScope {
            inherit (legacyPackages)
              craneLib
              fenixPackages fenixToolchain fenixToolchainShell
            ;
            inherit (packages) taimiHUD packs;
          };

          fenixPackages = fenix.packages.${system};
          fenixToolchain = with fenixPackages;
            combine [
              minimal.rustc
              minimal.cargo
              targets.x86_64-pc-windows-gnu.latest.rust-std
            ];
          fenixToolchainShell = with fenixPackages;
            combine [
              (complete.withComponents [
                "cargo"
                "rust-src"
                "clippy"
                "rustc"
              ])
              rust-analyzer
              latest.rustfmt
              targets.x86_64-pc-windows-gnu.latest.rust-std
            ];
          fenixToolchainShellBuild = with fenixPackages;
            combine [
              minimal.rustc
              minimal.cargo
            ];

          craneLib = (crane.mkLib pkgs).overrideToolchain (p: legacyPackages.fenixToolchain);
          craneLibBuild = (crane.mkLib pkgs.buildPackages).overrideToolchain (p: legacyPackages.fenixToolchainBuild);
          craneLibShell = (crane.mkLib pkgs).overrideToolchain (p: legacyPackages.fenixToolchainShell);
        };
      });
}

