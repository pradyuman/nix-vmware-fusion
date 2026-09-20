{
  description = "Manage VMware Fusion on macOS with Nix";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    nix-darwin = {
      url = "github:nix-darwin/nix-darwin";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    home-manager = {
      url = "github:nix-community/home-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    inputs@{
      flake-parts,
      treefmt-nix,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [
        treefmt-nix.flakeModule
        ./tests
      ];

      systems = [ "aarch64-darwin" ];

      perSystem =
        {
          pkgs,
          system,
          ...
        }:
        let
          localPkgs = import ./pkgs { inherit pkgs; };
        in
        {
          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            config.allowUnfreePredicate = package: inputs.nixpkgs.lib.getName package == "vmware-fusion-dmg";
          };

          packages.default = localPkgs.cli;

          apps.default = {
            type = "app";
            program = pkgs.lib.getExe localPkgs.cli;
          };

          treefmt = {
            projectRootFile = "flake.nix";
            programs = {
              actionlint.enable = true;
              mdformat = {
                enable = true;
                plugins = ps: [
                  ps.mdformat-frontmatter
                  ps.mdformat-gfm
                ];
                settings.number = true;
              };
              nixfmt.enable = true;
              rustfmt = {
                enable = true;
                edition = "2024";
              };
              shellcheck.enable = true;
            };
          };
        };

      flake = {
        darwinModules.default = ./modules/darwin.nix;
        homeModules.default = ./modules/home-manager;
      };
    };
}
