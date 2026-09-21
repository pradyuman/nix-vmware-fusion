# Testing strategy
#
# nix-vmware-fusion uses several command-line tools bundled with VMware Fusion.
# The test suite is split based on whether a check needs to execute those tools.
#
# `nix flake check` runs everything that can work without VMware Fusion:
#
# - treefmt checks formatting and lints shell scripts and GitHub Actions workflows.
# - Clippy lints the Rust CLI.
# - The Rust suite contains in-process unit and component tests.
# - The module suites evaluate nix-darwin and Home Manager output without activation.
# - The command-line tools check builds the VMware wrappers without executing them.
#
# VMware contract tests run the installed tools to verify the assumptions the CLI
# makes about them. They require VMware Fusion and run separately with
# `nix run .#vmware-contract-tests`.

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

      vmwareContractTests = pkgs.writeShellApplication {
        name = "nix-vmware-fusion-vmware-contract-tests";
        runtimeInputs = [
          pkgs.cargo
          pkgs.rustc
        ];
        text = ''
          export NIX_VMWARE_FUSION_DMG=/dev/null
          export NIX_VMWARE_FUSION_DICT_TOOL=${pkgs.lib.getExe' localPkgs.commandLineTools "dictTool"}
          export NIX_VMWARE_FUSION_VDISK_MANAGER=${pkgs.lib.getExe' localPkgs.commandLineTools "vmware-vdiskmanager"}
          export NIX_VMWARE_FUSION_VMDK_SERVER=${pkgs.lib.getExe' localPkgs.commandLineTools "vmware-vmdkserver"}
          export NIX_VMWARE_FUSION_VMCLI=${pkgs.lib.getExe' localPkgs.commandLineTools "vmcli"}

          exec cargo test --locked --manifest-path cli/Cargo.toml --features vmware-contract-tests "$@"
        '';
      };
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

      apps.vmware-contract-tests = {
        type = "app";
        program = pkgs.lib.getExe vmwareContractTests;
      };
    };
}
