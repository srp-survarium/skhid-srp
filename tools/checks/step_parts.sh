#!/usr/bin/env bash
# Patches 166 and 168: a part of a tick moves a body as a whole tick does, and the measured speed is a tick's (Unicorn),
# with patch 72's free fall beside it (the two share player_step's parts).
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
    python3 check_step_parts_026g.py "$2"
    python3 check_fall_gravity_026g.py "$2"
' step-parts "$unicorn" "$exe"
