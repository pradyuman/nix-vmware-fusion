{ pkgs }:

let
  dmg = pkgs.callPackage ./dmg.nix { };
  commandLineTools = pkgs.callPackage ./command-line-tools.nix { };
  cli = pkgs.callPackage ../cli { inherit dmg commandLineTools; };
in
{
  inherit dmg cli commandLineTools;
}
