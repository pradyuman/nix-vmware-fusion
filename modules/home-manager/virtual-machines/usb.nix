{ lib }:

lib.types.submodule {
  options.newDeviceAction = lib.mkOption {
    type = lib.types.enum [
      "ask"
      "connect-to-vm"
      "connect-to-host"
    ];
    default = "ask";
    description = "Action to take when a new USB device is connected to the host.";
  };
}
