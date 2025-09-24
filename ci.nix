{ config, pkgs, lib, env, ... }: with pkgs; with lib; let
  taimiHUD-rs = import ./.;
  packages = taimiHUD-rs.packages.${pkgs.system};
  taimiHUD = packages.taimiHUD.override {
    builtInfo = {
      ${if env.platform != "none" then "platform" else null} = env.platform;
      ${if env.git-ref != null then "ref" else null} = env.git-ref;
      ${if env.git-commit != null then "rev" else null} = env.git-commit;
      ${if env.git-commit != null then "shortRev" else null} = builtins.substring 0 8 env.git-commit;
      ${if env.platform != "none" then "dirty" else null} = false;
    };
  };
  artifactRoot = ".ci/artifacts";
  artifacts = "${artifactRoot}/lib/TaimiHUD*.dll";
  release = "${artifactRoot}/lib/taimi_hud.dll";
in
{
  config = {
    name = "taimiHUD";
    ci.gh-actions = {
      enable = true;
      export = true;
    };
    # TODO: add cachix
    cache.cachix.taimihud = {
      enable = true;
      publicKey = "taimihud.cachix.org-1:2LByDgq5eUVU2FoeIlMd5NMgUeCDXuuVarS+XbNsIkY=";
      signingKey = "nya";
    };
    channels = {
      nixpkgs = {
        # see https://github.com/arcnmx/nixexprs-rust/issues/10
        args.config.checkMetaRecursively = false;
        version = "22.11";
      };
    };
    tasks = {
      build.inputs = [ taimiHUD ]; #taimiHUDSpace ];
      cache.inputs = [ taimiHUD taimiHUD.cargoArtifacts ]; #taimiHUDSpace ];
    };
    jobs = {
      main = {
        tasks = {
          build-windows.inputs = singleton taimiHUD;
        };
        artifactPackages = {
          main = taimiHUD;
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

