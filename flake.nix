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
    };
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay/master";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs = {
        flake-compat.follows = "flake-compat";
        nixpkgs.follows = "nixpkgs";
      };
    };
  };

  outputs = { self, fenix, flake-utils, crane, nixpkgs, rust-overlay, ... }@inputs:
    flake-utils.lib.eachDefaultSystem (system:
      let
        legacyPackages = self.legacyPackages.${system};
        packages = self.packages.${system};
        devShells = self.devShells.${system};
        inherit (legacyPackages) pkgs callPackage fenixPackages;
        treefmtEval = inputs.treefmt-nix.lib.evalModule inputs.nixpkgs.legacyPackages.${system} ./treefmt.nix;
      in
      {
        # TaimiHUD Package
        packages = {
          taimiHUD = callPackage ./package.nix {
            source = self;
          };
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
            taimiHUD = packages.taimiHUD.override {
              enableLibgit = true;
            };
            inherit (packages) packs;
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
        formatter = treefmtEval.config.build.wrapper;
        checks = let
          git-hooks = system:
          inputs.git-hooks.lib.${system}.run {
            src = ./.;
            hooks = {
              treefmt = {
                enable = true;
                packageOverrides = {treefmt = inputs.self.formatter.${system};};
              };
              flake-checker.enable = true;
              ripsecrets.enable = true;
            };
          };
        in {
          formatting = treefmtEval.config.build.check inputs.self;
          git-hooks = git-hooks system;
        };
      });
}

