#!/usr/bin/env bash
# Patch reticle (its table): srp's A9 taken at the lobby's dispatcher, each scope item's srp_reticle, srp_set_reticle sent as 46 05.
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
    python3 -u check_reticle_table_026g.py "$2"
' reticle-table "$unicorn" "$exe"
