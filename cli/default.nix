{
  lib,
  rustPlatform,
  makeWrapper,
  bash,
  gum,
  dmg,
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
      --set VMWARE_FUSION_DMG ${lib.escapeShellArg (toString dmg)} \
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
