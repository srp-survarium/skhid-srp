#!/usr/bin/env bash
# The native replay reader and its generated hooks, without Windows side effects.
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
    python3 engine_026g.py --check
    python3 hooks_026g.py --check
    python3 link_026g.py --check
    python3 check_replay_file_026g.py "$2"
    cd rust
    cargo test --locked -p patches
    cargo clippy --locked -p patches --all-targets -- -D warnings
' replay-files "$unicorn" "$exe"
