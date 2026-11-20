#!/usr/bin/env bash
# Native renderer/options regression, then the menu driven by its callback traces.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
main=$(git -C "$root" worktree list --porcelain | sed -n '1s/^worktree //p')
exe=${SKHID_ORIGINAL_EXE:-$main/private/function-map/inputs/026g/survarium.exe}
: "${CHECK_WORK:?run by tools/check}"
cd "$root/srp-026g"
unicorn=$(nix build --no-link --print-out-paths --inputs-from . nixpkgs#python3Packages.unicorn)
nix develop -c bash -c '
    py=$(python3 -c "import sys; print(\"python%d.%d\" % sys.version_info[:2])")
    export PYTHONPATH="$1/lib/$py/site-packages"
    python3 client-patches/check_detail_026g.py "$2" --out "$CHECK_WORK/native"
' distance-detail "$unicorn" "$exe"
cd "$root"
tools/checks/movie.sh main_menu "$CHECK_WORK/main_menu.swf"
bash srp-026g/scripts/distance-detail/check.sh "$CHECK_WORK/main_menu.swf" "$CHECK_WORK/menu" "$CHECK_WORK/native/native-check.json"
