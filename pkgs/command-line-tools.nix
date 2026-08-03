{
  lib,
  symlinkJoin,
  writeShellApplication,
}:

let
  app = "/Applications/VMware Fusion.app";

  tools = {
    ovftool = "VMware OVF Tool/ovftool";
    vmcli = "vmcli";
    vmnet-bridge = "vmnet-bridge";
    vmnet-cfgcli = "vmnet-cfgcli";
    vmnet-cli = "vmnet-cli";
    vmnet-dhcpd = "vmnet-dhcpd";
    vmnet-natd = "vmnet-natd";
    vmnet-netifup = "vmnet-netifup";
    vmnet-sniffer = "vmnet-sniffer";
    vmrest = "vmrest";
    vmrun = "vmrun";
    vmss2core = "vmss2core";
    vmware-aewp = "vmware-aewp";
    vmware-id = "vmware-id";
    vmware-ntfs = "vmware-ntfs";
    vmware-rawdiskAuthTool = "vmware-rawdiskAuthTool";
    vmware-rawdiskCreator = "vmware-rawdiskCreator";
    vmware-remotemks = "vmware-remotemks";
    vmware-usbarbitrator = "vmware-usbarbitrator";
    vmware-vdiskmanager = "vmware-vdiskmanager";
    vmware-vmdkserver = "vmware-vmdkserver";
    vmware-vmx = "vmware-vmx";
    vmware-vmx-debug = "vmware-vmx-debug";
    vmware-vmx-stats = "vmware-vmx-stats";
  };

  toolPackages = lib.mapAttrsToList (
    name: relativePath:
    writeShellApplication {
      inherit name;

      text = ''
        target=${lib.escapeShellArg "${app}/Contents/Library/${relativePath}"}

        if [[ ! -x "$target" ]]; then
          echo "${name} is unavailable at $target. Install VMware Fusion first." >&2
          exit 1
        fi

        exec "$target" "$@"
      '';
    }
  ) tools;
in
symlinkJoin {
  name = "vmware-fusion-command-line-tools";
  paths = toolPackages;

  meta = {
    description = "Command-line tools bundled with VMware Fusion";
    license = lib.licenses.isc;
    platforms = lib.platforms.darwin;
  };
}
