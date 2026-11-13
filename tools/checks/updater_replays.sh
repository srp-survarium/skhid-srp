#!/usr/bin/env bash
# Build and test the file handler with a stand-in game in a disposable Wine prefix,
# then Save replay's end-to-end tests (tests/test_replay.py) under the same Wine.
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

# test_replay.py reads the updater's messages in English: a fresh prefix links Documents to
# $HOME/Documents, whose survarium/user.cfg may name another language, so the prefix is made
# (and the tests run) with HOME an empty folder of its own.
home=$(mktemp -d -t skhid-replay-home-XXXXXX)
wine_env=(HOME="$home" WINEPREFIX="$home/prefix" WINEDEBUG=-all
          WINEDLLOVERRIDES="mscoree,mshtml=;winemenubuilder.exe=d")
cleanup() {
    # The server writes the prefix's registry as it ends: wait for it before deleting.
    env "${wine_env[@]}" "$wine/bin/wineserver" -k 2>/dev/null || true
    env "${wine_env[@]}" "$wine/bin/wineserver" -w 2>/dev/null || true
    rm -rf "$home"
}
trap cleanup EXIT
env "${wine_env[@]}" "$xvfb/bin/xvfb-run" -a "$wine/bin/wine" wineboot -i >/dev/null 2>&1
nix develop ./srp-026g -c env "${wine_env[@]}" SKHID_TEST_WINE="$wine/bin/wine" \
    SKHID_TEST_WINEPREFIX="$home/prefix" \
    "$xvfb/bin/xvfb-run" -a python3 client/updater/tests/test_replay.py -v
