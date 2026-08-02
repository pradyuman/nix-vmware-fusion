{
  description = "Manage VMware Fusion on macOS with Nix";

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
      localPkgs = import ./pkgs { inherit pkgs; };
    in
    {
      apps.aarch64-darwin = {
        install = {
          type = "app";
          program = nixpkgs.lib.getExe localPkgs.installer;
        };

        uninstall = {
          type = "app";
          program = nixpkgs.lib.getExe localPkgs.uninstaller;
        };
      };

      darwinModules.default = import ./modules/darwin.nix;

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
