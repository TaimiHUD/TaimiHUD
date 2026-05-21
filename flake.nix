{
  description = "TaimiHUD; timers, markers and hopefully paths for raidcore.gg nexus";
  inputs = {
    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    libimgui = {
      url = "git+https://codeberg.org/TaimiHUD/libimgui.git?ref=refs/heads/symver";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        fenix.follows = "fenix";
        flake-utils.follows = "flake-utils";
        flake-compat.follows = "flake-compat";
      };
    };
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

  outputs = {
    self,
    libimgui,
    fenix,
    flake-utils,
    crane,
    nixpkgs,
    ...
  } @ inputs: let
    outputs.lib = {
      git-hooks = import ./ci/git-hooks {inherit inputs;};
      treefmt = import ./ci/treefmt {inherit inputs;};
    };
  in
    outputs
    // flake-utils.lib.eachDefaultSystem (system: let
      legacyPackages = self.legacyPackages.${system};
      packages = self.packages.${system};
      devShells = self.devShells.${system};
      inherit (legacyPackages) pkgs callPackage fenixPackages;
      channel = "1.92.0";
      channelHash = "sha256-sqSWJDUxc+zaz1nBWMAJKTAGBuGWP25GCftIOlCEAtA=";
      fenixW64 = fenixPackages.targets.x86_64-pc-windows-gnu.toolchainOf {
        inherit channel;
        sha256 = channelHash;
      };
      fenixChannel = fenixPackages.toolchainOf {
        inherit channel;
        sha256 = channelHash;
      };
    in {
      # TaimiHUD Package
      packages = {
        taimiHUD-develop = callPackage ./package.nix {
          source = self;
          builtInfo = {
            ref = null;
            rev = null;
            shortRev = null;
            dirty = false;
            platform = null;
          };
          inherit (legacyPackages) arcdps-imgui_18000 arcdps-imgui_19270;
        };
        taimiHUD = packages.taimiHUD-develop.override {
          inherit (packages.taimiHUD-develop) cargoArtifacts;
          builtInfo = {};
        };
        taimiHUD-debug = packages.taimiHUD.override {
          cargoArtifacts = null;
          buildType = "dev";
        };
        taimiHUD-check = packages.taimiHUD.override {
          doCheck = true;
        };

        default = packages.taimiHUD;
      };

      # TaimiHUD devShell
      devShells =
        import ./devShells.nix {
          inherit inputs system;
        }
        // {
          default = devShells.taimiShell;
        };

      legacyPackages = {
        pkgs = (import nixpkgs) {
          inherit system;
          crossSystem.config = "x86_64-w64-mingw32";
          overlays = [(import ./ci/lua-w64.nix)];
        };
        callPackage = pkgs.newScope {
          inherit
            (legacyPackages)
            lua
            lua-build
            craneLib
            fenixPackages
            fenixToolchain
            fenixToolchainShell
            ;
          taimiHUD = packages.taimiHUD.override {
            enableLibgit = true;
          };
          inherit (packages) packs;
        };

        # see ci/lua-w64.nix
        inherit (pkgs) luajit_2_0 luajit_2_1 lua5_1 lua5_2 lua5_3 lua5_4 lua5_5;
        lua = legacyPackages.luajit_2_1;
        lua-build = pkgs.buildPackages.luajit_2_1;

        inherit fenixChannel fenixW64;
        fenixPackages = fenix.packages.${system};
        fenixToolchain = fenixPackages.combine [
          fenixChannel.minimalToolchain
          fenixW64.rust-std
        ];
        fenixToolchainShell = fenixPackages.combine [
          (fenixChannel.withComponents [
            "cargo"
            "rust-src"
            "clippy"
            "rustc"
          ])
          fenixPackages.rust-analyzer
          legacyPackages.rustfmt
          fenixW64.rust-std
        ];
        fenixToolchainShellBuild =
          fenixChannel.minimalToolchain;
        inherit (fenixPackages.latest) rustfmt;

        craneLib = (crane.mkLib pkgs).overrideToolchain (_p: legacyPackages.fenixToolchain);
        craneLibBuild = (crane.mkLib pkgs.buildPackages).overrideToolchain (_p: legacyPackages.fenixToolchainBuild);
        craneLibShell = (crane.mkLib pkgs).overrideToolchain (_p: legacyPackages.fenixToolchainShell);

        inherit (libimgui.packages.${system}) arcdps-imgui_18000 arcdps-imgui_19270;

        git-hooks = self.lib.git-hooks.configForSystem system;
        treefmt = self.lib.treefmt.configForSystem system;
        formatter = legacyPackages.treefmt.config.build.wrapper;
      };
      inherit (legacyPackages) formatter;
      checks = {
        formatting = legacyPackages.treefmt.config.build.check self;
        git-hooks = legacyPackages.git-hooks.check;
      };
    });
}
