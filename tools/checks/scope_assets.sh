#!/usr/bin/env bash
# Prepared inputs are documented in scopes-069-completion.md; no archives are mutated.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
common=$(dirname "$(git -C "$root" rev-parse --path-format=absolute --git-common-dir)")
assets="$common/artifacts/scopes-069"
# A Git flake excludes Cargo's target directory from the Nix source snapshot.
export SCOPE_CHECK_FLAKE="git+file://$root?dir=srp-026g"
cd "$root"
nix shell --impure --expr '
  let pkgs = import (builtins.getFlake (builtins.getEnv "SCOPE_CHECK_FLAKE")).inputs.nixpkgs
    { system = builtins.currentSystem; };
  in pkgs.python3.withPackages (p: [ p.numpy p.pillow ])
' --command bash -c '
  set -euo pipefail
  python3 -I srp-026g/scripts/rifle-scopes/check_geometry.py \
    --pack "$1/private/content-packs/rifle-scopes-18" --live "$2/live" \
    --donor "$2/donor-rifles-2" --report "$CHECK_WORK/rifles.tsv"
  python3 -I srp-026g/scripts/rifle-scopes/check_geometry.py \
    --pack "$1/private/content-packs/scope-fits-12" --live "$2/live-rifles" \
    --donor "$2/donor-fits-2" --report "$CHECK_WORK/fits.tsv"
' scopes "$common" "$assets"
