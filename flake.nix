{
  description = "Manage VMware Fusion on macOS with Nix";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
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
  };

  outputs =
    {
      nix-darwin,
      home-manager,
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

        purge = {
          type = "app";
          program = nixpkgs.lib.getExe localPkgs.purge;
        };

        uninstall = {
          type = "app";
          program = nixpkgs.lib.getExe localPkgs.uninstaller;
        };
      };

      checks.aarch64-darwin = {
        command-line-tools = localPkgs.commandLineTools;

        darwin-module =
          pkgs.runCommand "darwin-module-tests"
            {
              nativeBuildInputs = [ pkgs.nix-unit ];
            }
            ''
              nix-unit \
                --arg nixpkgs '${nixpkgs}' \
                --arg nixDarwin '${nix-darwin}' \
                ${./.}/tests/modules/darwin.nix
              touch "$out"
            '';

        home-module =
          pkgs.runCommand "home-module-tests"
            {
              nativeBuildInputs = [ pkgs.nix-unit ];
            }
            ''
              nix-unit \
                --arg homeManager '${home-manager}' \
                --arg nixpkgs '${nixpkgs}' \
                ${./.}/tests/modules/home-manager.nix
              touch "$out"
            '';
      };

      darwinModules.default = ./modules/darwin.nix;

      homeModules.default = ./modules/home-manager;

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
