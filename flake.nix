{
  description = "cign - the tool that checks if you can go now";

  inputs = {
    nixpkgs.url = github:NixOS/nixpkgs/release-22.11;
    rust-overlay.url = github:oxalica/rust-overlay;
    flake-utils.url = github:numtide/flake-utils;
    cargo2nix.url = github:cargo2nix/cargo2nix;
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, cargo2nix }:
    flake-utils.lib.eachDefaultSystem (
      system:
        let
          lib = (import nixpkgs { inherit system; }).lib;
          pkgs = import nixpkgs {
            overlays = [cargo2nix.overlays.default];
            inherit system;
          };

          rustChannel = "1.61.0";
          rustPkgs = pkgs.rustBuilder.makePackageSet {
            inherit rustChannel;
            packageFun = import ./Cargo.nix;
            packageOverrides = pkgs: pkgs.rustBuilder.overrides.all;
          };

        in
          rec {

            devShell = pkgs.mkShell {
              buildInputs = [
                cargo2nix.packages.${system}.cargo2nix
                pkgs.rust-bin.stable."${rustChannel}".default
              ];
              nativeBuildInputs = with pkgs; [openssl pkg-config];
            };
            packages.cign = (rustPkgs.workspace.cign {}).bin;
            defaultPackage = packages.cign;
            defaultApp = { type = "app"; program = "${defaultPackage}/bin/cign"; };
          }
    );
}
