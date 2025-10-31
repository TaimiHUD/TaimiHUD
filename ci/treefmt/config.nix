{ taimi'legacyPackages, ... }: {
  projectRootFile = "flake.nix";
  programs = {
    alejandra.enable = true;
    deadnix.enable = true;
    statix.enable = true;
    rustfmt = {
      enable = true;
      package = taimi'legacyPackages.fenixPackages.latest.rustfmt;
      # XXX: keep in sync with Cargo.toml!
      edition = "2021";
    };
  };
}
