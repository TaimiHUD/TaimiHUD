{
  lib,
  config,
  pkgs,
  system,
  inputs,
  ...
}: let
  hooks.treefmt = {
    enable = true;
    packageOverrides = {
      treefmt = inputs.self.legacyPackages.${system}.formatter;
    };
  };
  hooks.flake-checker.enable = true;
  hooks.ripsecrets.enable = true;
  conf = {
    inherit hooks;
    #settings.rust.check.cargoDeps = inputs.self.packages.${system}.taimiHUD.cargoDeps; (or .taimiHUD.cargoArtifacts?)
  };

  scaffold = {
    installationScriptBin = mkOptionDefault (pkgs.writeShellScriptBin "git-hooks-install" config.installationScript);

    gitPackage = mkDefault (pkgs.writeShellScriptBin "git" ''
        realpath() {
          local p=$1
          if ! command readlink -f "$1" 2>/dev/null && ! command realpath "$1" 2>/dev/null; then
            printf '%s\n' "$p"
          fi
        }

        self=$(realpath "''${BASH_SOURCE[0]}")
        binname=$(basename "$0")
        if [[ -z $binname ]]; then binname=$(basename "''${BASH_SOURCE[0]}"); fi
        if [[ -z $binname ]]; then binname=$(basename "$self"); fi

        IFS=: paths=($PATH)
        for path in "''${paths[@]}"; do
          bin="$path/$binname"
          if [[ ! -e $bin ]] || [[ $(realpath "$bin") = $self ]]; then
            continue
          fi
          exec "$bin" "$@"
        done

        echo "$binname not found; $0 $*" >&2
        exit 1
      ''
      // {
        passthru = {
          #${if true then "propagatedBuildInputs" else "depsHostHostPropagated"} = [ pkgs.gitMinimal ];
        };
      });
  };
  inherit (lib.modules) mkMerge mkOptionDefault mkDefault;
  inherit (lib.options) mkOption;
  inherit (lib) types;
in {
  config = mkMerge [conf scaffold];

  options = {
    installationScriptBin = mkOption {
      type = types.package;
    };
  };
}
