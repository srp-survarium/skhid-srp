#!/usr/bin/env bash
# The exe's Rust patches crate: its tests and Clippy, for the game's i686 (its .cargo/config.toml),
# over the sources the build generates from the tables and the original exe.
set -euo pipefail
main=$(git worktree list --porcelain | sed -n '1s/^worktree //p')
exe=${SKHID_ORIGINAL_EXE:-$main/private/function-map/inputs/026g/survarium.exe}
cd srp-026g
nix develop -c bash -c '
    set -euo pipefail
    cd client-patches
    SRP_026G_EXE="$1" python3 link_026g.py --generate
    cd rust
    cargo test --locked -p patches
    cargo clippy --locked -p patches --all-targets -- -D warnings
' patches-crate "$exe"
