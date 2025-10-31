{inputs}: let
  nixlib = inputs.nixpkgs.lib;
  overlayForSystem = let
    inherit (builtins) attrNames;
    entry'path = inputs.git-hooks + "/nix/default.nix";
    entry'fn = import entry'path;
    entry =
      if builtins.pathExists entry'path && builtins.isFunction entry'fn
      then entry'fn
      else inputs.git-hooks.lib.run;
    entry'args = builtins.functionArgs entry;
    expected = {
      system = true;
      nixpkgs = false;
      gitignore-nix-src = false;
      isFlakes = true;
    };
    overrides = system': {
      system = system';
      nixpkgs = ./id.nix;
      gitignore-nix-src = throw "unexpected";
      isFlakes = true;
    };
    fallback = system: _final: _prev: {
      inherit (inputs.git-hooks.lib.${system}) run;
    };
    overlay = system: nixlib.composeManyExtensions (entry (overrides system)).overlays;
  in
    system:
      if attrNames entry'args == attrNames expected
      then overlay system
      else nixlib.warn "git-hooks API changed" (fallback system);
  args = {pkgs, ...}: {
    _module.args = {
      inherit inputs;
      inherit (pkgs) system;
    };
  };
  imports = [
    args
    ./config.nix
  ];
in {
  inherit overlayForSystem imports;
  configForSystem = let
    inherit (inputs.self.lib) git-hooks;
  in
    system: rec {
      extendPkgs = {
        system,
        overlay ? git-hooks.overlayForSystem system,
      }: let
        legacyPackages = inputs.self.legacyPackages.${system};
        overlaid = legacyPackages.pkgs.extend overlay;
      in
        overlaid.buildPackages.buildPackages;
      pkgs = extendPkgs {inherit system;};
      config = pkgs.run {
        src = inputs.self;
        inherit (git-hooks) imports;
      };
      check = config.config.run.overrideAttrs (old: {
        nativeBuildInputs =
          old.nativeBuildInputs or []
          ++ [
            pkgs.gitMinimal
          ];
      });
      inherit (config.config) installationScriptBin package configFile;
    };
}
