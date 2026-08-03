{
  nixpkgs,
  nixDarwin,
}:

let
  lib = import "${nixpkgs}/lib";

  mkSystem =
    vmwareFusionConfig:
    import "${nixDarwin}/eval-config.nix" {
      inherit lib;
      modules = [
        ../../modules/darwin.nix
        (
          { config, ... }:
          {
            nix.enable = false;
            nixpkgs = {
              hostPlatform = "aarch64-darwin";

              # Evaluate the module without requiring the proprietary DMG.
              overlays = [
                (_final: prev: {
                  requireFile =
                    _:
                    prev.runCommand "mock-vmware-fusion-dmg" { } ''
                      touch "$out"
                    '';
                })
              ];
              source = nixpkgs;
            };
            programs.vmware-fusion = vmwareFusionConfig;
            system.stateVersion = config.system.maxStateVersion;
          }
        )
      ];
    };

  defaultSystem = mkSystem { };
  enabledSystem = mkSystem { enable = true; };

  enabledVmwareFusionPackageNames = builtins.filter (lib.hasPrefix "vmware-fusion-") (
    map lib.getName enabledSystem.config.environment.systemPackages
  );

  enabledActivation = enabledSystem.config.system.activationScripts.postActivation.text;
in
{
  testDisabledByDefault = {
    expr = defaultSystem.config.programs.vmware-fusion.enable;
    expected = false;
  };

  testAllowsVmwareFusionDmg = {
    expr = enabledSystem.config.nixpkgs.config.allowUnfreePackages;
    expected = [ "vmware-fusion-dmg" ];
  };

  testEnabledPackages = {
    expr = enabledVmwareFusionPackageNames;
    expected = [
      "vmware-fusion-command-line-tools"
      "vmware-fusion-install"
      "vmware-fusion-uninstall"
    ];
  };

  testActivationChecksInstalledBuild = {
    expr = lib.hasInfix "Print :CFBundleVersion" enabledActivation;
    expected = true;
  };

  testActivationRunsInstaller = {
    expr = lib.hasInfix "vmware-fusion-install/bin/vmware-fusion-install" enabledActivation;
    expected = true;
  };
}
