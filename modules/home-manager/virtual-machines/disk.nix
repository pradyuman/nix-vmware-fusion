{ lib }:

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
)
