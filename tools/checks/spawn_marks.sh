#!/usr/bin/env bash
# The spawn marks (srp-026g/client-patches/rust/patches/src/spawn_marks.rs): a local build's tool,
# which no other build may hold. The default build as the original and without the tool, the
# --spawn-marks build's keys, file, camera switches and markers (Unicorn), and the crate with and without
# its cfg.
set -euo pipefail
root=$(git rev-parse --show-toplevel)
main=$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")
exe=${SKHID_ORIGINAL_EXE:-$main/private/function-map/inputs/026g/survarium.exe}
cd "$root/srp-026g"
unicorn=$(nix build --no-link --print-out-paths --inputs-from . nixpkgs#python3Packages.unicorn)
nix develop -c bash -c '
    set -euo pipefail
    py=$(python3 -c "import sys; print(\"python%d.%d\" % sys.version_info[:2])")
    export PYTHONPATH="$1/lib/$py/site-packages" SRP_026G_EXE="$2" SRP_026G_SCRATCH="$3"
    cd client-patches
    python3 engine_026g.py --check
    python3 hooks_026g.py --check
    python3 link_026g.py --check
    python3 check_spawn_marks_026g.py "$2"
    python3 -c "import sys, selftest_026g as st; st.test_spawn_marks(); sys.exit(1 if st.failures else 0)"
    cd rust
    cargo test --locked -p patches
    cargo clippy --locked -p patches --all-targets -- -D warnings
    RUSTFLAGS="--cfg srp_spawn_marks" cargo clippy --locked -p patches --all-targets -- -D warnings
' spawn-marks "$unicorn" "$exe" "${CHECK_WORK:-$(mktemp -d)}"
