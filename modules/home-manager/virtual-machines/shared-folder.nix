{ lib }:

lib.types.submodule {
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
}
