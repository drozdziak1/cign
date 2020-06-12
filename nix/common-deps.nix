{sources ? import ./sources.nix, pkgs ? import sources.nixpkgs { }, rust ? import rust.nix { inherit sources;}}:
with pkgs; [
  openssl
  pkgconfig
  zlib
] ++ [rust]
