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
            system.primaryUser = "test";
            system.stateVersion = config.system.maxStateVersion;
          }
        )
      ];
    };

  defaultSystem = mkSystem { };
  enabledSystem = mkSystem { enable = true; };
  networkedSystem = mkSystem {
    enable = true;
    networking.text = ''
      VERSION=1,0
      answer VNET_1_DHCP yes
      answer VNET_1_HOSTONLY_NETMASK 255.255.255.0
      answer VNET_1_HOSTONLY_SUBNET 192.0.2.0
      answer VNET_1_VIRTUAL_ADAPTER yes
    '';
  };
  uninstallingSystem = mkSystem { onActivation.cleanup = "uninstall"; };
  purgingSystem = mkSystem { onActivation.cleanup = "purge"; };

  enabledVmwareFusionPackageNames = builtins.filter (name: lib.hasInfix "vmware-fusion" name) (
    map lib.getName enabledSystem.config.environment.systemPackages
  );

  enabledActivation = enabledSystem.config.system.activationScripts.postActivation.text;
  networkedActivation = networkedSystem.config.system.activationScripts.postActivation.text;
  uninstallingActivation = uninstallingSystem.config.system.activationScripts.postActivation.text;
  purgingActivation = purgingSystem.config.system.activationScripts.postActivation.text;

in
{
  testDisabledByDefault = {
    expr = defaultSystem.config.programs.vmware-fusion.enable;
    expected = false;
  };

  testCleanupDefaultsToNone = {
    expr = defaultSystem.config.programs.vmware-fusion.onActivation.cleanup;
    expected = "none";
  };

  testAllowsVmwareFusionDmg = {
    expr = enabledSystem.config.nixpkgs.config.allowUnfreePackages;
    expected = [ "vmware-fusion-dmg" ];
  };

  testEnabledPackages = {
    expr = enabledVmwareFusionPackageNames;
    expected = [
      "vmware-fusion-command-line-tools"
      "nix-vmware-fusion"
    ];
  };

  testActivationChecksInstalledBuild = {
    expr = lib.hasInfix "Print :CFBundleVersion" enabledActivation;
    expected = true;
  };

  testActivationRunsInstaller = {
    expr = lib.hasInfix "/bin/nix-vmware-fusion install" enabledActivation;
    expected = true;
  };

  testNetworkingTextDefaultsToNull = {
    expr = defaultSystem.config.programs.vmware-fusion.networking.text;
    expected = null;
  };

  testNetworkingTextIsInstalled = {
    expr =
      lib.hasInfix "vmware-fusion-networking" networkedActivation
      && lib.hasInfix "/usr/bin/install -o root -g wheel -m 0644" networkedActivation;
    expected = true;
  };

  testCleanupUninstalls = {
    expr = lib.hasInfix "/bin/nix-vmware-fusion uninstall" uninstallingActivation;
    expected = true;
  };

  testCleanupPurges = {
    expr =
      lib.hasInfix "/bin/nix-vmware-fusion purge" purgingActivation
      && lib.hasInfix "--yes" purgingActivation
      && lib.hasInfix "--user test" purgingActivation;
    expected = true;
  };
}
