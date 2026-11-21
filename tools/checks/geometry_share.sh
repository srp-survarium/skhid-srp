#!/usr/bin/env bash
# Patch 164: generated ABI/hooks/arena and the geometry pools shared between buffers with the same bytes.
set -euo pipefail
root=$(git rev-parse --show-toplevel)
main=$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")
exe=${SKHID_ORIGINAL_EXE:-$main/private/function-map/inputs/026g/survarium.exe}
cd "$root/srp-026g"
unicorn=$(nix build --no-link --print-out-paths --inputs-from . nixpkgs#python3Packages.unicorn)
nix develop -c bash -c '
    set -euo pipefail
    py=$(python3 -c "import sys; print(\"python%d.%d\" % sys.version_info[:2])")
    export PYTHONPATH="$1/lib/$py/site-packages"
    export SRP_026G_EXE="$2"
    cd client-patches
    python3 engine_026g.py --check
    python3 hooks_026g.py --check
    python3 link_026g.py --check
    python3 -u check_geometry_share_026g.py "$2"
' geometry-share "$unicorn" "$exe"
