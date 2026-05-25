{taimi'legacyPackages, ...}: {
  projectRootFile = "flake.nix";
  programs = {
    alejandra.enable = true;
    deadnix.enable = true;
    statix.enable = true;
    rustfmt = {
      enable = true;
      package = taimi'legacyPackages.rustfmt;
      # XXX: keep in sync with Cargo.toml!
      edition = "2021";
    };
  };
  settings = {
    excludes = [
      "LICENSE"
      "*.md"
      ".envrc*"
      ".*ignore"
      # data
      "*.png"
      "*.shaderdesc"
      "*.lock"
      "*.yml"
      "*.json"
      # TODO?
      "*.toml"
      "*.hlsl"
      "*.ftl"
    ];
    walk = "auto";
    # TODO: consider cargo fmt --check instead?
    #formatter.rustfmt.options = lib.mkForce [ "--all" ]
  };
}
