{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.vmware-fusion;
  localPkgs = import ../pkgs { inherit pkgs; };
in
{
  options.programs.vmware-fusion.enable = lib.mkEnableOption "VMware Fusion";

  config = lib.mkIf cfg.enable {
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

      if [[ "$installed_build" == "${localPkgs.dmg.build}" ]]; then
        echo "VMware Fusion build ${localPkgs.dmg.build} is already installed"
      else
        ${lib.getExe localPkgs.installer}
      fi
    '';
  };
}
