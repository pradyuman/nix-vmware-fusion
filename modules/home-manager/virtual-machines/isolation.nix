{ lib }:

lib.types.submodule {
  options = {
    clipboard = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Whether to enable sharing clipboard contents between the host and guest.";
    };

    dragAndDrop = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Whether to enable drag and drop between the host and guest.";
    };
  };
}
