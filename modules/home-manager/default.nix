{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.vmware-fusion;
in
{
  imports = [ ./settings ];

  options.programs.vmware-fusion.enable = lib.mkEnableOption "VMware Fusion";

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = pkgs.stdenv.hostPlatform.system == "aarch64-darwin";
        message = "VMware Fusion is only supported on aarch64-darwin.";
      }
    ];
  };
}
