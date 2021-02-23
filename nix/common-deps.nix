{ sources ? import ./sources.nix, pkgs ? import sources.nixpkgs {} }:
with pkgs; [
  openssl
  pkgconfig
  zlib
]
