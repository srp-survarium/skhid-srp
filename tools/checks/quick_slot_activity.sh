#!/usr/bin/env bash
# Patch 77: the quick slots' activity (medicines, patch 84's red, Oxygen Rush's blue) at the native update.
set -euo pipefail
main=$(git worktree list --porcelain | sed -n '1s/^worktree //p')
exe=${SKHID_ORIGINAL_EXE:-$main/private/function-map/inputs/026g/survarium.exe}
cd srp-026g
unicorn=$(nix build --no-link --print-out-paths --inputs-from . nixpkgs#python3Packages.unicorn)
nix develop -c bash -c '
    set -euo pipefail
    py=$(python3 -c "import sys; print(\"python%d.%d\" % sys.version_info[:2])")
    export PYTHONPATH="$1/lib/$py/site-packages"
    cd client-patches
    python3 -u check_consumable_activity_026g.py "$2"
' quick-slot-activity "$unicorn" "$exe"
