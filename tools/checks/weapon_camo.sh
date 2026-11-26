#!/usr/bin/env bash
# The weapon camouflage masks' tool: its rule, its tables and their hand picks (numpy, Pillow, SciPy).
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
export WEAPON_CAMO_FLAKE="git+file://$root?dir=srp-026g"
cd "$root/srp-026g/scripts/weapon-camo"
nix shell --impure --expr '
  let pkgs = import (builtins.getFlake (builtins.getEnv "WEAPON_CAMO_FLAKE")).inputs.nixpkgs
    { system = builtins.currentSystem; };
  in pkgs.python3.withPackages (p: [ p.numpy p.pillow p.scipy ])
' --command python3 -W ignore -m unittest test_weapon_camo
