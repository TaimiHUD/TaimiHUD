{ inputs }: let
  nixlib = inputs.nixpkgs.lib;
  inherit (inputs.self.lib) treefmt;
in {
  imports = [
    ./config.nix
  ];

  inherit (inputs.treefmt-nix) lib;
  argsForSystem = system: _: {
    _module.args = {
      taimi'legacyPackages = nixlib.mkOptionDefault inputs.self.legacyPackages.${system};
    };
  };
  configForSystem = system: let
    legacyPackages = inputs.self.legacyPackages.${system};
    pkgs = legacyPackages.pkgs.buildPackages.buildPackages;
  in treefmt.lib.evalModule pkgs (_: {
    imports = treefmt.imports ++ [
      (treefmt.argsForSystem system)
    ];
  });
}
