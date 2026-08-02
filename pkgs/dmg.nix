{
  lib,
  requireFile,
}:

let
  version = "26H1";
  build = "25388279";

  dmg = requireFile {
    name = "VMware-Fusion-${version}-${build}_universal.dmg";
    url =
      let
        params = {
          subFamily = "VMware Fusion";
          displayGroup = "VMware Fusion ${version}";
          release = version;
          servicePk = "543219";
          language = "EN";
          freeDownloads = "true";
        };
        query = lib.concatStringsSep "&" (
          lib.mapAttrsToList (name: value: "${lib.escapeURL name}=${lib.escapeURL value}") params
        );
      in
      "https://support.broadcom.com/group/ecx/productfiles?${query}";
    hash = "sha256-wdNzqiG+JWdOPsxRiBniVXhd6p1FbYdHvLCipZJEvfY=";
  };
in
dmg.overrideAttrs (_: {
  pname = "vmware-fusion-dmg";
  passthru = {
    inherit build version;
  };
})
