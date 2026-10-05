{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.vmware-fusion;
  homeDirectory = config.home.homeDirectory;

  networkAdapterType = import ./network-adapter.nix { inherit lib; };
  virtualDiskType = import ./disk.nix { inherit lib; };
  sharedFolderType = import ./shared-folder.nix { inherit lib; };
  displayType = import ./display.nix { inherit lib; };
  soundCardType = import ./sound.nix { inherit lib; };
  opticalDriveType = import ./optical-drive.nix { inherit lib; };

  virtualMachineModule =
    { name, config, ... }:
    {
      options = {
        displayName = lib.mkOption {
          type = lib.types.str;
          default = name;
          description = "Display name of the virtual machine.";
        };

        path = lib.mkOption {
          type = lib.types.str;
          default = "${homeDirectory}/Virtual Machines.localized/${name}.vmwarevm";
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

        coresPerSocket = lib.mkOption {
          type = lib.types.nullOr lib.types.ints.positive;
          default = null;
          example = 4;
          description = ''
            Number of virtual CPU cores per socket. When null, VMware chooses
            the topology when the virtual machine powers on.
          '';
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

        networkAdapters = lib.mkOption {
          type = lib.types.attrsOf networkAdapterType;
          default = { };
          description = "Virtual network adapters attached to the VM. Undeclared adapters are removed.";
        };

        disks = lib.mkOption {
          type = lib.types.attrsOf (virtualDiskType config.path);
          default = { };
          description = "Virtual disks attached to the VM. Undeclared disks will be detached without deleting their files.";
        };

        sharedFolders = lib.mkOption {
          type = lib.types.attrsOf sharedFolderType;
          default = { };
          description = "Host directories shared with the VM. Undeclared shared folders are removed.";
        };

        display = lib.mkOption {
          type = lib.types.nullOr displayType;
          default = null;
          description = "Virtual display configuration for the VM.";
        };

        sound = lib.mkOption {
          type = lib.types.nullOr soundCardType;
          default = null;
          description = "VMware HD Audio sound card attached to the VM.";
        };

        opticalDrives = lib.mkOption {
          type = lib.types.attrsOf opticalDriveType;
          default = { };
          description = "Virtual optical drives attached to the VM. Removing a declaration detaches the drive without deleting its image.";
        };
      };
    };

  localPkgs = import ../../../pkgs { inherit pkgs; };

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

  config = lib.mkIf cfg.enable {
    assertions = lib.mapAttrsToList (name: vm: {
      assertion = vm.coresPerSocket == null || lib.mod vm.vcpus vm.coresPerSocket == 0;
      message = "programs.vmware-fusion.virtualMachines.${name}.coresPerSocket must evenly divide vcpus";
    }) cfg.virtualMachines;

    home.activation.vmwareFusionVirtualMachines = lib.mkIf (irFiles != { }) (
      lib.hm.dag.entryAfter [ "writeBoundary" ] ''
        ${lib.concatMapAttrsStringSep "\n" (
          _: irFile: "run ${lib.getExe localPkgs.cli} vm apply ${lib.escapeShellArg irFile}"
        ) irFiles}
      ''
    );
  };
}
