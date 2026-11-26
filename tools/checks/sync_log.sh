#!/usr/bin/env bash
# Patch 210: the sync log (SKHID_SYNC_LOG): its lines for an input and a tick off the 50 ms grid and
# the server's world, none on the grid or off; the sites' registers and memory the original's (Unicorn).
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
    python3 check_sync_log_026g.py "$2"
' sync-log "$unicorn" "$exe"
