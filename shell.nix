let
  sources = import ./nix/sources.nix;
  pkgs = import sources.nixpkgs {};
  rust = import ./nix/rust.nix { inherit sources; };
  common-deps = import ./nix/common-deps.nix { inherit sources pkgs rust; };
in
pkgs.mkShell {
  buildInputs = with pkgs; [
    gdb
  ] ++ common-deps;
}
