{ lib }:

lib.types.submodule {
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
}
