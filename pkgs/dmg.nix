{
  lib,
  requireFile,
}:

let
  version = "26H1u1";
  build = "25689522";

  dmg = requireFile {
    name = "VMware-Fusion-${version}-${build}_universal.dmg";
    url =
      let
        params = {
          subFamily = "VMware Fusion";
          displayGroup = "VMware Fusion 26H1";
          release = version;
          os = "";
          servicePk = "546858";
          language = "EN";
          freeDownloads = "true";
        };
        query = lib.concatStringsSep "&" (
          lib.mapAttrsToList (name: value: "${lib.escapeURL name}=${lib.escapeURL value}") params
        );
      in
      "https://support.broadcom.com/group/ecx/productfiles?${query}";
    hash = "sha256-3xkR+N5lGBikPCDKEFRAPafjlTTxWi4OD51B/89yirg=";
  };
in
dmg.overrideAttrs (_: {
  pname = "vmware-fusion-dmg";
  passthru = {
    inherit build version;
  };
})
