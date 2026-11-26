#!/usr/bin/env bash
# Patch 212: keys in texts (Unicorn); and the results screen's NULL map name (match_stat_map_026g.asm),
# whose check runs the same translator.
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
    python3 -u check_key_prompts_026g.py "$2"
    python3 -u check_match_stat_map_026g.py "$2"
' key-prompts "$unicorn" "$exe"
