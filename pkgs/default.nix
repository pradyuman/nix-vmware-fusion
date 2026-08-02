{ pkgs }:

let
  dmg = pkgs.callPackage ./dmg.nix { };
in
{
  inherit dmg;

  installer = pkgs.callPackage ./installer.nix {
    inherit dmg;
  };

  uninstaller = pkgs.callPackage ./uninstaller.nix { };
}
