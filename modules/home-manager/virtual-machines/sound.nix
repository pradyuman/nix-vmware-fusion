{ lib }:

lib.types.submodule {
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
}
