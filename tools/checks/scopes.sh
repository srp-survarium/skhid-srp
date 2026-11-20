#!/usr/bin/env bash
# Scope geometry and its native camera oracle, with the repository's pinned nixpkgs.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
common=$(dirname "$(git -C "$root" rev-parse --path-format=absolute --git-common-dir)")
# A Git flake excludes Cargo's target directory from the Nix source snapshot.
export SCOPE_CHECK_FLAKE="git+file://$root?dir=srp-026g"
cd "$root"
nix shell --impure --expr '
  let pkgs = import (builtins.getFlake (builtins.getEnv "SCOPE_CHECK_FLAKE")).inputs.nixpkgs
    { system = builtins.currentSystem; };
  in pkgs.python3.withPackages (p: [ p.numpy p.pillow p.unicorn ])
' --command bash -c '
  set -euo pipefail
  python3 -m unittest discover -s srp-026g/scripts/rifle-scopes -p "test_*.py"
  python3 srp-026g/scripts/sight-alignment/check_engine.py "$1"
' scopes "$common/private/function-map/inputs/026g/survarium.exe"
