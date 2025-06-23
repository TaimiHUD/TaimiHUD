{ config, pkgs, lib, ... }: with pkgs; with lib; let
  taimiHUD-rs = import ./.;
  packages = taimiHUD-rs.packages.${pkgs.system};
  artifactRoot = ".ci/artifacts";
  artifacts = "${artifactRoot}/lib/TaimiHUD*.dll";
  release = "${artifactRoot}/lib/taimi_hud.dll";
in
{
  config = {
    name = "taimiHUD";
    ci.gh-actions.enable = true;
    # TODO: add cachix
    cache.cachix.taimihud.enable = true;
    channels = {
      nixpkgs = {
        # see https://github.com/arcnmx/nixexprs-rust/issues/10
        args.config.checkMetaRecursively = false;
        version = "22.11";
      };
    };
    tasks = {
      build.inputs = with packages; [ taimiHUD ]; #taimiHUDSpace ];
    };
    jobs = {
      main = {
        tasks = {
          build-windows.inputs = singleton packages.taimiHUD;
          build-windows-space.inputs = singleton packages.taimiHUD;
        };
        artifactPackages = {
          main = packages.taimiHUD;
          #space = packages.taimiHUDSpace;
        };
      };
    };

    # XXX: symlinks are not followed, see https://github.com/softprops/action-gh-release/issues/182
    #artifactPackage = config.artifactPackages.win64;
    artifactPackage = runCommand "taimihud-artifacts" { } (''
      mkdir -p $out/lib
      cp ${config.artifactPackages.main}/lib/taimi_hud.dll $out/lib/
    '' + concatStringsSep "\n" (mapAttrsToList (key: taimi: ''
        cp ${taimi}/lib/taimi_hud.dll $out/lib/TaimiHUD-${key}.dll
    '') config.artifactPackages));

    gh-actions = {
      jobs = mkIf (config.id != "ci") {
        ${config.id} = {
          permissions = {
            contents = "write";
          };
          step = {
            artifact-build = {
              order = 1100;
              name = "artifact build";
              uses = {
                # XXX: a very hacky way of getting the runner
                inherit (config.gh-actions.jobs.${config.id}.step.ci-setup.uses) owner repo version;
                path = "actions/nix/build";
              };
              "with" = {
                file = "<ci>";
                attrs = "config.jobs.${config.jobId}.artifactPackage";
                out-link = artifactRoot;
              };
            };
            artifact-upload = {
              order = 1110;
              name = "artifact upload";
              uses.path = "actions/upload-artifact@v4";
              "with" = {
                name = "TaimiHUD";
                path = artifacts;
              };
            };
            release-upload = {
              order = 1111;
              name = "release";
              "if" = "startsWith(github.ref, 'refs/tags/')";
              uses.path = "softprops/action-gh-release@v1";
              "with".files = release;
            };
          };
        };
      };
    };
  };
  options = {
    artifactPackage = mkOption {
      type = types.package;
    };
    artifactPackages = mkOption {
      type = with types; attrsOf package;
    };
  };
}

