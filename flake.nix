{
  description = "cign - the tool that checks if you can go now";

  inputs = {
    nixpkgs.url = github:NixOS/nixpkgs/release-21.11;
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
            overlays = cargo2nix.overlays.${system};
            inherit system;
          };

          rustPkgs = pkgs.rustBuilder.makePackageSet' {
            rustChannel = "1.56.1";
            packageFun = import ./Cargo.nix;
            # localPatterns = [
            #   ''^(assets|src)(/.*)?''
            #   ''[^/]*\.(rs|toml|txt)$''
            # ];
          };

        in
          rec {

            devShell = pkgs.mkShell {
              buildInputs = [ cargo2nix.packages.${system}.cargo2nix];
            };
            packages.cign = (rustPkgs.workspace.cign {}).bin;
            defaultPackage = packages.cign;
            defaultApp = { type = "app"; program = "${defaultPackage}/bin/cign"; };
          }
    );
}
