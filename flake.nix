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

  outputs = { self, fenix, flake-utils, crane, nixpkgs, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = (import nixpkgs) {
          inherit system;
          crossSystem.config = "x86_64-w64-mingw32";
        };

        # TaimiHUD Package
        packageToolchain = with fenix.packages.${system};
          combine [
            minimal.rustc
            minimal.cargo
            targets.x86_64-pc-windows-gnu.latest.rust-std
          ];

        packageCraneLib = (crane.mkLib pkgs).overrideToolchain (p: packageToolchain);

        taimiHUD = pkgs.callPackage ./package.nix {
          craneLib = packageCraneLib;
        };

        taimiHUDSpace = pkgs.callPackage ./package.nix {
          craneLib = packageCraneLib;
          features = [ "space" ];
        };

        packs = pkgs.callPackage ./pathing/pack/taco.nix;

        # TaimiHUD devShell
        shellToolchain = with fenix.packages.${system};
          combine [
            complete
            rust-analyzer
            targets.x86_64-pc-windows-gnu.latest.rust-std
          ];

        shellCraneLib = (crane.mkLib pkgs).overrideToolchain (p: shellToolchain);

        taimiShell = import ./shell.nix {
          inherit fenix pkgs system;
        };
      in
      rec {
        defaultPackage = packages.x86_64-pc-windows-gnu;
        inherit pkgs;
        devShells.default = taimiShell;

        packages = {
          inherit taimiHUD taimiHUDSpace packs;
          default = taimiHUD;
        };
      });
}

