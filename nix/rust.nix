{ sources ? import ./sources.nix }:

let
  pkgs =
  import sources.nixpkgs {overlays = [ (import sources.nixpkgs-mozilla) ]; };
  channel = "stable";
  date = null;
  targets = [];
  chan = pkgs.rustChannelOfTargets channel date targets;
in chan
