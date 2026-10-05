{ lib }:

let
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

      scaledHighResolution = lib.mkOption {
        type = lib.types.enum [
          "full-screen"
          "single-window"
          "all"
        ];
        default = "all";
        description = "View modes in which non-Retina displays use scaled high-resolution rendering.";
      };
    };
  };
in
lib.types.submodule {
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
}
