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
' --command bash -c '
  set -euo pipefail
  python3 -I srp-026g/scripts/content-edit/content_edit.py \
    srp-026g/scripts/content-edit/scope-centering-1.json "$2/coverage-live" "$CHECK_WORK/centered-configs"
  python3 -I srp-026g/scripts/rifle-scopes/check_geometry.py \
    --pack "$1" --live "$2/coverage-live" --donor "$2/coverage-donor-final" --report "$CHECK_WORK/geometry.tsv"
  python3 -I srp-026g/scripts/scope-coverage/check_mounts.py --pack "$1" --donor "$2/coverage-donor-final"
  python3 -I srp-026g/scripts/scope-coverage/check_centering.py \
    --files "$1/files" --configs "$CHECK_WORK/centered-configs" --live "$2/coverage-live" --repairs "$1/centering.json"
' scopes "$common/private/content-packs/scope-coverage-2" "$common/artifacts/scopes-069"
