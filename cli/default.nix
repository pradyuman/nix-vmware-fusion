{
  lib,
  rustPlatform,
  makeWrapper,
  bash,
  gum,
  qemu-utils,
  dmg,
  commandLineTools,
}:

rustPlatform.buildRustPackage {
  pname = "nix-vmware-fusion";
  version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;
  src = lib.cleanSourceWith {
    src = ./.;
    filter = path: type: baseNameOf path != "target" && lib.cleanSourceFilter path type;
  };
  cargoLock.lockFile = ./Cargo.lock;
  nativeBuildInputs = [ makeWrapper ];

  postInstall = ''
    wrapProgram "$out/bin/nix-vmware-fusion" \
      --set NIX_VMWARE_FUSION_DMG ${lib.escapeShellArg (toString dmg)} \
      --set NIX_VMWARE_FUSION_DICT_TOOL ${lib.escapeShellArg (lib.getExe' commandLineTools "dictTool")} \
      --set NIX_VMWARE_FUSION_QEMU_IMG ${lib.escapeShellArg (lib.getExe' qemu-utils "qemu-img")} \
      --set NIX_VMWARE_FUSION_VDISK_MANAGER ${lib.escapeShellArg (lib.getExe' commandLineTools "vmware-vdiskmanager")} \
      --set NIX_VMWARE_FUSION_VMDK_SERVER ${lib.escapeShellArg (lib.getExe' commandLineTools "vmware-vmdkserver")} \
      --set NIX_VMWARE_FUSION_VMCLI ${lib.escapeShellArg (lib.getExe' commandLineTools "vmcli")} \
      --prefix PATH : ${
        lib.makeBinPath [
          bash
          gum
        ]
      }
  '';

  meta = {
    description = "Manage VMware Fusion on macOS";
    license = lib.licenses.isc;
    platforms = lib.platforms.darwin;
    mainProgram = "nix-vmware-fusion";
  };
}
