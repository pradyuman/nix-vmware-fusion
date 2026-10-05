{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.vmware-fusion;
  homeDirectory = config.home.homeDirectory;

  networkAdapterType = lib.types.addCheck (lib.types.submodule {
    options = {
      mode = lib.mkOption {
        type = lib.types.enum [
          "nat"
          "bridged"
          "host-only"
          "custom"
        ];
        default = "nat";
        description = "How the virtual network adapter connects to the host network.";
      };

      vmnet = lib.mkOption {
        type = lib.types.nullOr lib.types.nonEmptyStr;
        default = null;
        example = "vmnet2";
        description = "VMware network used when mode is custom.";
      };

      model = lib.mkOption {
        type = lib.types.enum [
          "vmxnet3"
          "e1000e"
          "e1000"
        ];
        default = "vmxnet3";
        description = "Virtual network adapter model.";
      };

      startConnected = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Whether to connect the adapter when the virtual machine starts.";
      };
    };
  }) (adapter: (adapter.mode == "custom") == (adapter.vmnet != null));

  virtualDiskType =
    bundlePath:
    lib.types.submodule (
      { name, ... }:
      {
        options = {
          path = lib.mkOption {
            type = lib.types.str;
            default = "${bundlePath}/${name}.vmdk";
            description = "Absolute path to the virtual disk.";
          };

          size = lib.mkOption {
            type = lib.types.ints.positive;
            description = "Virtual disk capacity in GiB. The configured capacity cannot be smaller than the disk's current capacity.";
          };

          bus = lib.mkOption {
            type = lib.types.enum [
              "nvme"
              "sata"
            ];
            default = "nvme";
            description = "Virtual disk bus type.";
          };

          preallocate = lib.mkOption {
            type = lib.types.bool;
            default = false;
            description = "Whether to pre-allocate disk space.";
          };

          split = lib.mkOption {
            type = lib.types.bool;
            default = false;
            description = "Whether to split the virtual disk into multiple files.";
          };
        };
      }
    );

  sharedFolderType = lib.types.submodule {
    options = {
      hostPath = lib.mkOption {
        type = lib.types.str;
        example = "/Users/asuna/projects";
        description = "Absolute path to the directory on the host.";
      };

      readOnly = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "Whether the shared folder is read-only in the guest.";
      };
    };
  };

  graphicsType = lib.types.submodule {
    options = {
      accelerate3D = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Whether to enable accelerated 3D graphics.";
      };

      memory = lib.mkOption {
        type = lib.types.ints.positive;
        default = 256;
        description = "Maximum amount of virtual machine memory available for graphics (in MiB).";
      };
    };
  };

  nativeDisplayResolutionType = lib.types.submodule {
    options = {
      enable = lib.mkEnableOption "the host display's native pixel resolution for the guest";
    };
  };

  displayType = lib.types.submodule {
    options = {
      graphics = lib.mkOption {
        type = graphicsType;
        default = { };
        description = "Virtual graphics configuration for the VM.";
      };

      nativeDisplayResolution = lib.mkOption {
        type = nativeDisplayResolutionType;
        default = { };
        description = "Native display resolution configuration for the VM.";
      };

      singleWindowFit = lib.mkOption {
        type = lib.types.enum [
          "inherit"
          "stretch"
          "resize"
        ];
        default = "inherit";
        description = "How to size the virtual machine display in single-window mode.";
      };

      fullScreenFit = lib.mkOption {
        type = lib.types.enum [
          "inherit"
          "center"
          "stretch"
          "resize"
        ];
        default = "inherit";
        description = "How to size the virtual machine display in full-screen mode.";
      };

      useAllDisplaysInFullScreen = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "Whether to use all host displays in full-screen mode.";
      };

    };
  };

  soundCardType = lib.types.submodule {
    options = {
      startConnected = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Whether to connect the sound card when the virtual machine starts.";
      };

      echoCancellation = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "Whether to enable echo cancellation.";
      };
    };
  };

  opticalDriveType = lib.types.submodule {
    options = {
      source = lib.mkOption {
        type = lib.types.submodule {
          options = {
            type = lib.mkOption {
              type = lib.types.enum [ "image" ];
              description = "Optical drive backing type.";
            };

            path = lib.mkOption {
              type = lib.types.str;
              example = "/path/to/installer.iso";
              description = "Absolute path to the ISO image.";
            };
          };
        };
        description = "Media inserted into the optical drive.";
      };

      startConnected = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Whether to connect the drive when the virtual machine starts.";
      };
    };
  };

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
