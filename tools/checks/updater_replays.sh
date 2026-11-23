#!/usr/bin/env bash
# Build and test the file handler with a stand-in game in disposable Wine prefixes.
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
"$xvfb/bin/xvfb-run" -a "${PYTHON:-python3}" client/updater/tests/check_local_replay.py --wine "$wine/bin/wine"
# The patch step, the restore of resources.db, and play_skhid.bat as make_client_026g.py writes it.
(cd client/updater && SKHID_TEST_WINE="$wine/bin/wine" "${PYTHON:-python3}" tests/test_db_patches.py)
