let
  sources = import ./nix/sources.nix;
  pkgs = import sources.nixpkgs {};
  rust = import ./nix/rust.nix { inherit sources; };
in
pkgs.mkShell {
  buildInputs = with pkgs; [
    gdb
    openssl
    pkgconfig
    zlib
  ] ++ [rust]
    ;
}
