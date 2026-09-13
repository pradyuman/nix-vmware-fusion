{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.vmware-fusion;

  virtualMachineModule =
    { name, ... }:
    {
      options = {
        displayName = lib.mkOption {
          type = lib.types.str;
          default = name;
          description = "Display name of the virtual machine.";
        };

        path = lib.mkOption {
          type = lib.types.str;
          default = "${config.home.homeDirectory}/Virtual Machines.localized/${name}.vmwarevm";
          description = "Path to the virtual machine bundle.";
        };

        guestOS = lib.mkOption {
          type = lib.types.str;
          example = "arm-other6xlinux-64";
          description = "Guest operating system identifier.";
        };

        vcpus = lib.mkOption {
          type = lib.types.ints.positive;
          example = 4;
          description = "Number of virtual CPUs.";
        };

        memory = lib.mkOption {
          type = lib.types.ints.positive;
          example = 8192;
          description = "Amount of memory in megabytes.";
        };

        secureBoot = lib.mkOption {
          type = lib.types.bool;
          description = "Whether to enable UEFI Secure Boot.";
        };
      };
    };

  localPkgs = import ../../pkgs { inherit pkgs; };

  irFiles = lib.mapAttrs (
    name: vm:
    pkgs.writeText "nix-vmware-fusion-${lib.strings.sanitizeDerivationName name}-ir.json" (
      builtins.toJSON vm
    )
  ) cfg.virtualMachines;
in
{
  options.programs.vmware-fusion.virtualMachines = lib.mkOption {
    type = lib.types.attrsOf (lib.types.submodule virtualMachineModule);
    default = { };
    description = "VMware Fusion virtual machines managed by Home Manager.";
  };

  config = lib.mkIf (cfg.enable && irFiles != { }) {
    home.activation.vmwareFusionVirtualMachines = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
      ${lib.concatMapAttrsStringSep "\n" (
        _: irFile: "run ${lib.getExe localPkgs.cli} vm apply ${lib.escapeShellArg irFile}"
      ) irFiles}
    '';
  };
}
