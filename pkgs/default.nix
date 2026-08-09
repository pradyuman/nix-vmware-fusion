{ pkgs }:

let
  dmg = pkgs.callPackage ./dmg.nix { };
in
{
  inherit dmg;

  commandLineTools = pkgs.callPackage ./command-line-tools.nix { };

  installer = pkgs.callPackage ./installer.nix {
    inherit dmg;
  };

  purge = pkgs.callPackage ./purge {
    vmwareFusionVersion = dmg.version;
  };
  uninstaller = pkgs.callPackage ./uninstaller.nix { };
}
