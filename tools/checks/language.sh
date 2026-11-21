#!/usr/bin/env bash
# The game and launcher use one language catalog and the same Windows locale rules.
set -euo pipefail
main=$(git worktree list --porcelain | sed -n '1s/^worktree //p')
exe=${SKHID_ORIGINAL_EXE:-$main/private/function-map/inputs/026g/survarium.exe}
launcher=${SKHID_ORIGINAL_LAUNCHER:-$main/../srp/0.26g-original/Survarium/game/binaries/x86/survarium_launcher.exe}
cd srp-026g
unicorn=$(nix build --no-link --print-out-paths --inputs-from . nixpkgs#python3Packages.unicorn)
nix develop -c bash -c '
    set -euo pipefail
    py=$(python3 -c "import sys; print(\"python%d.%d\" % sys.version_info[:2])")
    export PYTHONPATH="$1/lib/$py/site-packages"
    cd client-patches
    python3 -u check_language_026g.py "$2"
    cd launcher
    python3 -u selftest_launcher_026g.py "$3" --nasm=nasm
    python3 -u check_language_026g.py "$3"
    python3 -u check_torrent_box_026g.py "$3"
    python3 -u check_relay_box_026g.py "$3"
' language-runtime "$unicorn" "$exe" "$launcher"
