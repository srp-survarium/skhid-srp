#!/usr/bin/env bash
# Generated ABI/hooks and the module report's native callback checks.
set -euo pipefail
root=$(git rev-parse --show-toplevel)
main=$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")
exe=${SKHID_ORIGINAL_EXE:-$main/private/function-map/inputs/026g/survarium.exe}
cd "$root/srp-026g"
unicorn=$(nix build --no-link --print-out-paths --inputs-from . nixpkgs#python3Packages.unicorn)
nix develop -c bash -c '
    py=$(python3 -c "import sys; print(\"python%d.%d\" % sys.version_info[:2])")
    export PYTHONPATH="$1/lib/$py/site-packages"
    export SRP_026G_EXE="$2"
    cd client-patches
    python3 engine_026g.py --check
    python3 hooks_026g.py --check
    python3 link_026g.py --check
    python3 check_engine_026g.py "$2"
    python3 check_hooks_026g.py "$2"
    python3 check_module_report_026g.py "$2"
' bash "$unicorn" "$exe"
