{
  argc,
  gum,
  lib,
  vmwareFusionVersion,
  writeShellApplication,
}:

writeShellApplication {
  name = "vmware-fusion-purge";

  runtimeInputs = [
    argc
    gum
  ];

  text = builtins.replaceStrings [ "@vmwareFusionVersion@" ] [ vmwareFusionVersion ] (
    builtins.readFile ./purge.sh
  );

  meta = {
    description = "Remove VMware Fusion and its support files";
    license = lib.licenses.isc;
    platforms = lib.platforms.darwin;
    mainProgram = "vmware-fusion-purge";
  };
}
