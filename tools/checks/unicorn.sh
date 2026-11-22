#!/usr/bin/env bash
# unicorn.sh <check_*.py>: one of srp-026g/client-patches' Unicorn checks of an exe patch, run on the
# original exe ($SKHID_ORIGINAL_EXE, default private/function-map/inputs/026g/survarium.exe of the main
# checkout) in the dev shell, with nixpkgs' Python unicorn (as release/lib.sh's self-test). Run by
# tools/check.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
main=$(dirname "$(git -C "$root" rev-parse --path-format=absolute --git-common-dir)")
exe=${SKHID_ORIGINAL_EXE:-$main/private/function-map/inputs/026g/survarium.exe}
[[ -f $exe ]] || { echo "unicorn.sh: $exe: no original exe (SKHID_ORIGINAL_EXE)" >&2; exit 1; }
cd "$root/srp-026g/client-patches"
# shellcheck disable=SC2016 # the dev shell's bash expands them
nix develop .. -c bash -c '
    py=$(python3 -c "import sys; print(f\"python{sys.version_info.major}.{sys.version_info.minor}\")")
    unicorn=$(nix build --no-link --print-out-paths --inputs-from .. nixpkgs#python3Packages.unicorn | head -n 1)
    SRP_026G_EXE=$2 PYTHONPATH=$unicorn/lib/$py/site-packages exec python3 -u "$1" "$2"' - "$1" "$exe"
