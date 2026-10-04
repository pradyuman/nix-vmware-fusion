{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.vmware-fusion;
  localPkgs = import ../pkgs { inherit pkgs; };

  toggleType = lib.types.submodule {
    options.enable = lib.mkOption {
      type = lib.types.bool;
      description = "Whether to enable the feature.";
    };
  };

  networkType = lib.types.submodule {
    options = {
      subnet = lib.mkOption {
        type = lib.types.submodule {
          options = {
            address = lib.mkOption {
              type = lib.types.nonEmptyStr;
              example = "192.168.200.0";
              description = "IPv4 network address.";
            };

            prefixLength = lib.mkOption {
              type = lib.types.ints.between 0 32;
              example = 24;
              description = "IPv4 network prefix length.";
            };
          };
        };
        description = "IPv4 subnet assigned to the VMware network.";
      };

      dhcp = lib.mkOption {
        type = toggleType;
        description = "DHCP service for the VMware network.";
      };

      nat = lib.mkOption {
        type = toggleType;
        description = "NAT service for the VMware network.";
      };

      hostAdapter = lib.mkOption {
        type = toggleType;
        description = "Virtual host adapter for the VMware network.";
      };
    };
  };

  networkConfiguration = pkgs.writeText "nix-vmware-fusion-networks.json" (
    builtins.toJSON cfg.networking.networks
  );
in
{
  options.programs.vmware-fusion = {
    enable = lib.mkEnableOption "VMware Fusion";

    networking.networks = lib.mkOption {
      type = lib.types.attrsOf networkType;
      default = { };
      example = {
        vmnet8 = {
          subnet = {
            address = "192.168.200.0";
            prefixLength = 24;
          };
          dhcp.enable = true;
          nat.enable = true;
          hostAdapter.enable = true;
        };
      };
      description = ''
        VMware Fusion networks to manage. Removing a declaration removes its
        network.
      '';
    };

    onActivation.cleanup = lib.mkOption {
      type = lib.types.enum [
        "none"
        "uninstall"
        "purge"
      ];
      default = "none";
      example = "uninstall";
      description = ''
        Action to take during activation when
        `programs.vmware-fusion.enable` is `false`.

        When set to `"none"` (the default), VMware Fusion is left installed.
        `"uninstall"` removes only the application, while `"purge"` also
        removes its support files. Neither action touches existing virtual
        machine bundles.
      '';
    };
  };

  config = lib.mkMerge [
    (lib.mkIf (cfg.enable || cfg.onActivation.cleanup != "none") {
      nixpkgs.config.allowUnfreePackages = [ "vmware-fusion-dmg" ];
    })
    (lib.mkIf cfg.enable {

      assertions = [
        {
          assertion = pkgs.stdenv.hostPlatform.system == "aarch64-darwin";
          message = "VMware Fusion is only supported on aarch64-darwin.";
        }
      ];

      environment.systemPackages = [
        localPkgs.commandLineTools
        localPkgs.cli
      ];

      system.activationScripts.postActivation.text = lib.mkAfter ''
        target_app="/Applications/VMware Fusion.app"
        info_plist="$target_app/Contents/Info.plist"
        installed_build=""

        if [[ -f "$info_plist" ]]; then
          installed_build="$(
            /usr/libexec/PlistBuddy -c "Print :CFBundleVersion" "$info_plist" 2>/dev/null || true
          )"
        fi

        if [[ "$installed_build" != "${localPkgs.dmg.build}" ]]; then
          ${lib.getExe localPkgs.cli} install
        fi

        ${lib.getExe localPkgs.cli} network apply ${lib.escapeShellArg networkConfiguration}
      '';
    })

    (lib.mkIf (!cfg.enable && cfg.onActivation.cleanup == "uninstall") {
      system.activationScripts.postActivation.text = lib.mkAfter ''
        ${lib.getExe localPkgs.cli} uninstall
      '';
    })

    (lib.mkIf (!cfg.enable && cfg.onActivation.cleanup == "purge") {
      system.requiresPrimaryUser = [ "programs.vmware-fusion.onActivation.cleanup" ];

      system.activationScripts.postActivation.text = lib.mkAfter ''
        ${lib.getExe localPkgs.cli} purge \
          --yes \
          --user ${lib.escapeShellArg config.system.primaryUser}
      '';
    })
  ];
}
