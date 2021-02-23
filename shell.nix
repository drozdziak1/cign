let
  sources = import ./nix/sources.nix;
  nixpkgsMozilla = import sources.nixpkgs-mozilla;
  cargo2nix-olay = import "${sources.cargo2nix}/overlay";
  pkgs = import sources.nixpkgs { overlays = [ cargo2nix-olay ]; };
  cargo2nix = import sources.cargo2nix {
    nixpkgsMozilla =
      sources.nixpkgs-mozilla;
    nixpkgs = sources.nixpkgs;
  };
  common-deps = import ./nix/common-deps.nix { inherit sources; };
in
cargo2nix.shell.overrideAttrs (
  oldAttrs: rec {
    nativeBuildInputs = oldAttrs.nativeBuildInputs ++ [ pkgs.rust-analyzer pkgs.gdb cargo2nix.package ] ++ common-deps;
  }
)
