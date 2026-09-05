{ pkgs }:

let
  dmg = pkgs.callPackage ./dmg.nix { };
  cli = pkgs.callPackage ../cli { inherit dmg; };
in
{
  inherit dmg cli;

  commandLineTools = pkgs.callPackage ./command-line-tools.nix { };

}
