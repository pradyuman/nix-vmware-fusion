{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.vmware-fusion;
  localPkgs = import ../pkgs { inherit pkgs; };

  networkingFile =
    if cfg.networking.text == null then
      null
    else
      pkgs.writeText "vmware-fusion-networking" cfg.networking.text;
in
{
  options.programs.vmware-fusion = {
    enable = lib.mkEnableOption "VMware Fusion";

    networking.text = lib.mkOption {
      type = lib.types.nullOr lib.types.lines;
      default = null;
      description = ''
        Contents of VMware Fusion's system-wide networking file. During
        activation, the module replaces the existing file with this text.
        When unset, the module leaves the networking configuration unmanaged.
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
    (lib.mkIf cfg.enable {
      nixpkgs.config.allowUnfreePackages = [ "vmware-fusion-dmg" ];

      assertions = [
        {
          assertion = pkgs.stdenv.hostPlatform.system == "aarch64-darwin";
          message = "VMware Fusion is only supported on aarch64-darwin.";
        }
      ];

      environment.systemPackages = [
        localPkgs.commandLineTools
        localPkgs.installer
        localPkgs.purge
        localPkgs.uninstaller
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
          ${lib.getExe localPkgs.installer}
        fi

        ${lib.optionalString (cfg.networking.text != null) ''
          source_networking=${lib.escapeShellArg networkingFile}
          target_networking="/Library/Preferences/VMware Fusion/networking"

          if [[ ! -f "$target_networking" ]] \
            || ! /usr/bin/cmp -s "$source_networking" "$target_networking"; then
            echo "Updating VMware Fusion's networking configuration..."
            /bin/mkdir -p "''${target_networking%/*}"
            /usr/bin/install -o root -g wheel -m 0644 "$source_networking" "$target_networking"
          fi
        ''}
      '';
    })

    (lib.mkIf (!cfg.enable && cfg.onActivation.cleanup == "uninstall") {
      system.activationScripts.postActivation.text = lib.mkAfter ''
        ${lib.getExe localPkgs.uninstaller}
      '';
    })

    (lib.mkIf (!cfg.enable && cfg.onActivation.cleanup == "purge") {
      system.requiresPrimaryUser = [ "programs.vmware-fusion.onActivation.cleanup" ];

      system.activationScripts.postActivation.text = lib.mkAfter ''
        ${lib.getExe localPkgs.purge} \
          --yes \
          --user ${lib.escapeShellArg config.system.primaryUser}
      '';
    })
  ];
}
