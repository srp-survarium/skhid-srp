#!/usr/bin/env bash
# Build and test the file handler with a stand-in game in a disposable Wine prefix.
set -euo pipefail
nix develop ./srp-026g -c bash -c '
    set -euo pipefail
    (cd client/updater/logic && cargo test --locked && cargo clippy --locked --all-targets -- -D warnings)
    bash client/updater/build.sh
    host=$(rustc -vV | sed -n "s/^host: //p")
    export PATH="$(rustc --print sysroot)/lib/rustlib/$host/bin:$PATH"
    (cd client/updater/exe && cargo clippy --locked --release -- -D warnings)
'
cd srp-026g
wine=$(nix build --no-link --print-out-paths --inputs-from . nixpkgs#wineWow64Packages.staging)
xvfb=$(nix build --no-link --print-out-paths --inputs-from . nixpkgs#xvfb-run)
cd ..
# python3 is the dev shell's (there may be none outside it).
nix develop ./srp-026g -c "$xvfb/bin/xvfb-run" -a python3 client/updater/tests/check_local_replay.py --wine "$wine/bin/wine"
