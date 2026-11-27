#!/usr/bin/env bash
# Patch 206: a weapon's second sight and the switch to it and back, simulated (the key's bit 30, the
# press, the tick's end, the deactivation, the weapon's making and end, the shot's reads of the aim, the
# player's state's end saved and loaded).
set -euo pipefail
main=$(git worktree list --porcelain | sed -n '1s/^worktree //p')
exe=${SKHID_ORIGINAL_EXE:-$main/private/function-map/inputs/026g/survarium.exe}
cd srp-026g
unicorn=$(nix build --no-link --print-out-paths --inputs-from . nixpkgs#python3Packages.unicorn)
nix develop -c bash -c '
    set -euo pipefail
    py=$(python3 -c "import sys; print(\"python%d.%d\" % sys.version_info[:2])")
    export PYTHONPATH="$1/lib/$py/site-packages" SRP_026G_EXE="$2"
    cd client-patches
    python3 -u check_second_sights_026g.py "$2"
' second-sights "$unicorn" "$exe"
