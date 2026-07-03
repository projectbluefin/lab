{
  description = "Compare versions according to the UAPI Version Format Specification";

  inputs = {

    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    systems.url = "github:nix-systems/default";

    flake-utils = {
      url = "github:numtide/flake-utils";
      inputs.systems.follows = "systems";
    };

    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };

    pre-commit-hooks-nix = {
      url = "github:cachix/pre-commit-hooks.nix";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };

  };

  outputs = inputs@{ self, flake-parts, systems, ... }: flake-parts.lib.mkFlake { inherit inputs; } {
    systems = import systems;

    imports = [
      inputs.pre-commit-hooks-nix.flakeModule
    ];

    perSystem = { config, system, pkgs, lib, ... }:
      let
        uapiVersion = pkgs.callPackage ./nix/build.nix { };
      in
      {

        packages = {
          # This is mostly here for development
          inherit uapiVersion;
          default = uapiVersion;
        };

        checks = {
          clippy = uapiVersion.overrideAttrs (_: previousAttrs: {
            nativeCheckInputs = (previousAttrs.nativeCheckInputs or [ ]) ++ [ pkgs.clippy ];
            checkPhase = "cargo clippy";
          });
          rustfmt = uapiVersion.overrideAttrs (_: previousAttrs: {
            nativeCheckInputs = (previousAttrs.nativeCheckInputs or [ ]) ++ [ pkgs.rustfmt ];
            checkPhase = "cargo fmt --check";
          });
        };

        pre-commit = {
          check.enable = true;

          settings = {
            hooks = {
              nixpkgs-fmt.enable = true;
              typos.enable = true;
              statix = {
                enable = true;
                settings.ignore = [ "sources.nix" ];
              };
            };
          };
        };

        devShells.default = pkgs.mkShell {
          shellHook = ''
            ${config.pre-commit.installationScript}
          '';

          packages = [
            pkgs.clippy
            pkgs.rustfmt
            pkgs.cargo-machete
            pkgs.cargo-edit
            pkgs.cargo-bloat
            pkgs.cargo-deny
            pkgs.cargo-cyclonedx
          ];

          inputsFrom = [ uapiVersion ];

          RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
        };

      };
  };
}
