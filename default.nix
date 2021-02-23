{ system ? builtins.currentSystem
, sources ? import nix/sources.nix
, nixpkgs ? sources.nixpkgs
, nixpkgsMozilla ? sources.nixpkgs-mozilla
, cargo2nix ? sources.cargo2nix
}:
let
  rustOverlay = import "${nixpkgsMozilla}/rust-overlay.nix";
  cargo2nixOverlay = import "${sources.cargo2nix}/overlay";

  pkgs = import <nixpkgs> {
    inherit system;
    overlays = [ cargo2nixOverlay rustOverlay ];
  };

  rustPkgs = pkgs.rustBuilder.makePackageSet' {
    rustChannel = "1.49.0";
    packageFun = import ./Cargo.nix;
    localPatterns = [
      ''^(assets|src)(/.*)?''
      ''[^/]*\.(rs|toml|txt)$''
    ];
  };
in
rustPkgs.workspace.cign {}
