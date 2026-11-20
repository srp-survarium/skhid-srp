#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
common=$(dirname "$(git -C "$root" rev-parse --path-format=absolute --git-common-dir)")
export SCOPE_CHECK_FLAKE="git+file://$root?dir=srp-026g"
cd "$root"
nix shell --impure --expr '
  let pkgs = import (builtins.getFlake (builtins.getEnv "SCOPE_CHECK_FLAKE")).inputs.nixpkgs
    { system = builtins.currentSystem; };
  in pkgs.python3.withPackages (p: [ p.numpy p.pillow p.scipy ])
' --command python3 -I srp-026g/scripts/scope-coverage/test_coverage.py
