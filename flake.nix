{
  description = "Install VMware Fusion with Nix";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      treefmt-nix,
      ...
    }:
    let
      pkgs = nixpkgs.legacyPackages.aarch64-darwin;
      dmg = pkgs.callPackage ./pkgs/dmg.nix { };
      installer = pkgs.callPackage ./pkgs/installer.nix {
        inherit dmg;
      };
      uninstaller = pkgs.callPackage ./pkgs/uninstaller.nix { };
    in
    {
      apps.aarch64-darwin = {
        install = {
          type = "app";
          program = nixpkgs.lib.getExe installer;
        };

        uninstall = {
          type = "app";
          program = nixpkgs.lib.getExe uninstaller;
        };
      };

      formatter.aarch64-darwin =
        let
          treefmt = treefmt-nix.lib.evalModule pkgs {
            projectRootFile = "flake.nix";
            programs = {
              mdformat = {
                enable = true;
                plugins = ps: [
                  ps.mdformat-frontmatter
                  ps.mdformat-gfm
                ];
                settings.number = true;
              };
              nixfmt.enable = true;
            };
          };
        in
        treefmt.config.build.wrapper;
    };
}
