#!/usr/bin/env bash
# Patch 211: the world of a tick (exact_world): the ring of hashed worlds, a request for a held tick
# uploads it byte for byte, anything else goes on to the snapshot upload as the original (Unicorn).
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
    python3 check_exact_world_026g.py "$2"
' exact-world "$unicorn" "$exe"
