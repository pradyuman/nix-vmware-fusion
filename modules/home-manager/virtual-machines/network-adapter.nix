{ lib }:

lib.types.addCheck (lib.types.submodule {
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
}) (adapter: (adapter.mode == "custom") == (adapter.vmnet != null))
