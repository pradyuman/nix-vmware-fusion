{
  lib,
  requireFile,
}:

requireFile {
  name = "VMware-Fusion-26H1-25388279_universal.dmg";
  url =
    let
      params = {
        subFamily = "VMware Fusion";
        displayGroup = "VMware Fusion 26H1";
        release = "26H1";
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
}
