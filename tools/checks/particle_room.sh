#!/usr/bin/env bash
# Patches 117 and 122: the effect presenters' lists (a missing entry ignored; room for 32, a full list takes no play).
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
    python3 check_effect_presenters_026g.py "$2"
    python3 check_particle_room_026g.py "$2"
' particle-room "$unicorn" "$exe"
