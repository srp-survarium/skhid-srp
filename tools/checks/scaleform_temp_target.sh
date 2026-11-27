#!/usr/bin/env bash
# Patch 216: a Scaleform filter whose temporary render target was not made leaves no null target
# on the HAL's render-target stack (the build, the original and two broken builds).
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
    python3 -u check_scaleform_temp_target_026g.py "$2"
' scaleform-temp-target "$unicorn" "$exe"
