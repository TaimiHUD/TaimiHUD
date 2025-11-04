{
  config,
  pkgs,
  lib,
  env,
  ...
}:
with pkgs;
with lib; let
  taimiHUD-rs = import ../.;
  checks = taimiHUD-rs.checks.${pkgs.system};
  packages = taimiHUD-rs.packages.${pkgs.system};
  legacyPackages = taimiHUD-rs.legacyPackages.${pkgs.system};
  taimiHUD = disableCache (packages.taimiHUD.override {
    builtInfo = {
      ${
        if env.platform != "none"
        then "platform"
        else null
      } =
        env.platform;
      ${
        if env.git-ref != null
        then "ref"
        else null
      } =
        env.git-ref;
      ${
        if env.git-commit != null
        then "rev"
        else null
      } =
        env.git-commit;
      ${
        if env.git-commit != null
        then "shortRev"
        else null
      } =
        builtins.substring 0 8 env.git-commit;
      ${
        if env.platform != "none"
        then "dirty"
        else null
      } =
        false;
    };
  });
  taimiHUD-check = disableCache packages.taimiHUD-check;
  disableCache = pkg:
    pkg.overrideAttrs (old: {
      passthru =
        old.passthru
        or {}
        // {
          ci.cache.enable = false;
        };
    });
  artifactRoot = ".ci/artifacts";
  artifacts = "${artifactRoot}/lib/TaimiHUD*.dll";
  release = "${artifactRoot}/lib/TaimiHUD.dll";
  artifactShare = {
    nexusTagName = "share/taimi/release.nexus.tag";
  };
  intOr = s: let
    int = builtins.tryEval (toInt s);
  in
    if int.success
    then int.value
    else s;
  parseTag = ref: let
    name = removePrefix "v" (removePrefix "refs/tags/" ref);
    parts = versions.splitVersion name;
    isPre = length parts > 4 && pre.channel != "+";
    pre = {
      channel = elemAt parts 3;
      revision = intOr (elemAt parts 4);
    };
  in {
    inherit name;
    success = ref != null && length (splitString "." ref) > 2;
    major =
      if length parts > 0
      then intOr (elemAt parts 0)
      else 0;
    minor =
      if length parts > 1
      then intOr (elemAt parts 1)
      else 0;
    patch =
      if length parts > 2
      then intOr (elemAt parts 2)
      else 0;
    #revision = if length (versions.splitVersion name) > 4 && isNumericOrSomething (elemAt thatSplittedThing 4) && elemAt thatSplittedThing 3 != "+";
    pre =
      if isPre
      then pre
      else null;
    revision =
      if isPre && builtins.isInt pre.revision
      then pre.revision
      else 0;
  };
  tag2nexus = tag: let
    rcTag = {
      inherit (tag) success;
      major =
        if tag.minor > 0
        then tag.major
        else tag.major - 1;
      minor =
        if tag.minor > 0
        then tag.minor - 1
        else 99;
      patch = 900 + tag.revision;
      # TODO: stop using extension-nexus-codegen feature because it won't set this
      revision = 0;
    };
    preTag = {
      inherit (tag) success major minor patch;
      revision = -(tag.revision + 1);
    };
  in
    if ! tag.success or false || tag.pre == null
    then tag
    else if tag.pre.channel == "rc"
    then rcTag
    else preTag;
  tag = parseTag env.git-ref;
  nexusTag = tag2nexus tag;
  nexusTagName = "v${toString nexusTag.major}.${toString nexusTag.minor}.${toString nexusTag.patch}.${toString nexusTag.revision}";
in {
  config = {
    name = "taimiHUD";
    ci.gh-actions = {
      enable = true;
      export = true;
      checkoutOptions.fetch-depth = 0;
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
        version = "24.11";
      };
    };
    jobs = {
      ci-check = {
        tasks = {
          check.inputs = [
            taimiHUD-check
            checks.formatting
            checks.git-hooks
          ];
          cache.inputs = [
            taimiHUD.cargoArtifacts
            legacyPackages.git-hooks.package
            legacyPackages.formatter
          ];
        };
      };
      release = {
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
    artifactPackage =
      runCommand "taimihud-artifacts" {
        nexusTagName =
          if nexusTag.success or false
          then nexusTagName
          else "";
      } (''
          mkdir -p $out/lib $out/share/taimi
          printf '%s\n' "$nexusTagName" > $out/${artifactShare.nexusTagName}
        ''
        + concatStringsSep "\n" (mapAttrsToList (key: taimi: let
            outname = "TaimiHUD" + optionalString (key != "main") "-${key}";
          in ''
            cp ${taimi}/lib/taimi_hud.dll $out/lib/${outname}.dll
          '')
          config.artifactPackages));

    gh-actions = {
      on = {
        push = let
          d = "[0-9]+";
        in {
          branches = ["**"];
          tags = [
            "v${d}.${d}.${d}"
            "v${d}.${d}.${d}-**"
          ];
        };
        pull_request = {};
        workflow_dispatch = {};
      };
      jobs = let
        ifAll = concatMapStringsSep " && " (cond: "(${cond})");
        ifAny = concatMapStringsSep " || " (cond: "(${cond})");
        expr = e: "\${{ ${e} }}";
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
          artifact-parse = {
            order = 1111;
            name = "artifact parse";
            shell = "bash";
            run = ''
              NEXUS_TAG_NAME=$(cat ${artifactRoot}/${artifactShare.nexusTagName})
              echo "release-nexus-tag=$NEXUS_TAG_NAME" >> $GITHUB_OUTPUT
              if [[ -n $NEXUS_TAG_NAME && $NEXUS_TAG_NAME != "''${{ github.ref_name }}" ]]; then
                git fetch origin "refs/tags/$NEXUS_TAG_NAME" || true
                git tag -f "$NEXUS_TAG_NAME" "''${{ github.ref }}" &&
                git push -f origin "$NEXUS_TAG_NAME" || true
              fi
            '';
          };
          release-upload = {
            order = 1112;
            name = "release";
            "if" = condRelease;
            uses.path = "softprops/action-gh-release@v1";
            "with" = let
              pre_name = "format('{0} ({1}-nexus)', ${real_tag}, ${nexus_tag})";
            in {
              files = release;
              prerelease = expr is_pre;
              tag_name = expr tag_name;
              name = expr "${nexus_tag} && ${pre_name} || ${real_tag}";
              #target_commitish = channel branch?
            };
          };
          release-describe = {
            order = 1113;
            name = "describe release tag";
            "if" = condRelease;
            continue-on-error = true;
            run = "./ci/get-tag-description.sh";
            env = {
              #TAG_REF = expr "github.ref";
              RELEASE_REF = expr "format('refs/tags/{0}', ${tag_name})";
            };
          };
          release-notify = {
            order = 1114;
            name = "notify discord";
            "if" = condRelease;
            continue-on-error = true;
            uses.path = "tsickert/discord-webhook@v5.3.0";
            "with" = mapAttrs (_: expr) {
              webhook-url = "secrets.WEBHOOK_URL";
              embed-title = "steps.release-describe.outputs.TAG_SUBJECT";
              embed-description = "steps.release-describe.outputs.TAG_BODY";
              embed-url = "steps.release-describe.outputs.TAG_URL";
            };
          };
        };
        condRelease = ifAll [
          "github.event_name == 'push'"
          "startsWith(github.ref, 'refs/tags/v')"
        ];
        condCheck = ifAny [
          "github.event_name != 'pull_request'"
          "github.event.pull_request.head.repo.fork"
        ];
        is_pre = "contains(${real_tag}, '-')";
        nexus_tag = "${is_pre} && steps.artifact-parse.outputs.release-nexus-tag != ${real_tag} && steps.artifact-parse.outputs.release-nexus-tag";
        real_tag = "github.ref_name";
        tag_name = "${nexus_tag} || ${real_tag}";
        confDeploy = {
          name = "${config.name} build --release";
          "if" = let
            notPush = "github.event_name != 'push'";
            longLivedBranches = [
              "develop"
              "main"
              "arc/dev"
            ];
            pushAllowed = ifAny (
              [
                "startsWith(github.event.ref, 'refs/heads/v')"
                "startsWith(github.event.ref, 'refs/tags/')"
              ]
              ++ map (branch: "github.event.ref == 'refs/heads/${branch}'") longLivedBranches
            );
            allowed = ifAny [notPush pushAllowed];
          in
            expr (ifAll [allowed "!cancelled()"]);
          permissions = {
            contents = "write";
          };
          inherit step;
          needs = ["ci-check"];
        };
        confDuplicatePr = {
          "if" = condCheck;
        };
        confCheck = _: {
          imports = [confDuplicatePr];
          name = "${config.name} check";
        };
      in {
        release = mkIf (config.id == "release") confDeploy;
        ci-check = mkIf (config.id == "ci-check") confCheck;
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
