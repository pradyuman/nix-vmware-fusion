{ inputs, ... }:

{
  perSystem =
    { pkgs, ... }:
    let
      localPkgs = import ../pkgs { inherit pkgs; };

      craneLib = inputs.crane.mkLib pkgs;
      cliArgs = {
        src = ../cli;
        strictDeps = true;
      };
      cliArtifacts = craneLib.buildDepsOnly cliArgs;
    in
    {
      checks = {
        clippy = craneLib.cargoClippy (
          cliArgs
          // {
            cargoArtifacts = cliArtifacts;
            cargoClippyExtraArgs = "--all-targets --all-features -- --deny warnings";
          }
        );
        cli = craneLib.cargoTest (cliArgs // { cargoArtifacts = cliArtifacts; });

        darwin-module =
          pkgs.runCommand "darwin-module-tests"
            {
              nativeBuildInputs = [ pkgs.nix-unit ];
            }
            ''
              nix-unit \
                --arg nixpkgs '${inputs.nixpkgs}' \
                --arg nixDarwin '${inputs.nix-darwin}' \
                ${../.}/tests/modules/darwin.nix
              touch "$out"
            '';

        home-module =
          pkgs.runCommand "home-module-tests"
            {
              nativeBuildInputs = [ pkgs.nix-unit ];
            }
            ''
              nix-unit \
                --arg homeManager '${inputs.home-manager}' \
                --arg nixpkgs '${inputs.nixpkgs}' \
                ${../.}/tests/modules/home-manager.nix
              touch "$out"
            '';

        command-line-tools = localPkgs.commandLineTools;
      };
    };
}
