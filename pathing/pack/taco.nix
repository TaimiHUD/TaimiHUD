{ lib, unzip, fetchurl, mkDerivationNoCC }: let
  inherit (lib.attrsets) attrValues mapAttrsToList;
  inherit (lib.strings) concatStringsSep escapeShellArg;
  sources = {
    tekkits-aio = fetchurl {
      name = "tw_ALL_IN_ONE.taco";
      url = "https://www.tekkitsworkshop.net/download?download=1:tw-all-in-one";
      hash = "sha256-bHpKeBBIr8F4BNO2sAaaCloQ5I5N7CFNZ0o059Iba2M=";
    };
    tehs-trails = fetchurl {
      url = "https://github.com/xrandox/TehsTrails/releases/download/v5.2.0/TehsTrails.taco";
      hash = "sha256-uY+lolPEnR7QbcexpU9wzpy5CfVXFeQaxzLsZBp+tSo=";
    };
    tehs-trails-hp = fetchurl {
      url = "https://github.com/xrandox/TehsTrails-HeroPoints/releases/download/v1.0/TehsTrails-HeroPoints.taco";
      hash = "sha256-hbdUuFNlr+5FFQG4wqQhm1/JMJ/CQP34mE9VR0Ztafg=";
    };
    lady-elyssa = fetchurl {
      url = "https://github.com/LadyElyssa/LadyElyssaTacoTrails/releases/download/v20.5.1/LadyElyssa.taco";
      hash = "sha256-kNqfXQxYECwdrFOcY6xZbPKhuMn/22Xnmf2P3nrMuIY=";
    };
  };
  mkInstall = name: src: ''
    install -d ${escapeShellArg src.name} $out/share/gw2taco/${escapeShellArg name}
  '';

in mkDerivationNoCC {
  name = "gw2-taco-packs";
  srcs = attrValues sources;

  #nativeBuildInputs = [ unzip ];

  buildPhase = ''
    runHook preBuild;
    true
    runHook postBuild;
  '';

  installPhase = ''
    runHook preInstall;

    ${concatStringsSep "\n" (mapAttrsToList mkInstall sources)}

    runHook postInstall;
  '';

  passthru = {
    inherit sources;
  };
}
