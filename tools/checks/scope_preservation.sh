#!/usr/bin/env bash
# Content-pipeline output must keep every released scope pair (srp-026g/scripts/scope-fits/released-pairs.tsv).
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
bash "$root/tools/checks/build_content.sh"
nix develop "$root/srp-026g" --command cargo run --quiet \
    --manifest-path "$root/srp-026g/Cargo.toml" -p survarium --bin srp-check-scope-fits -- \
    "$root/srp-026g/scripts/scope-fits/released-pairs.tsv" "$CHECK_WORK/content/server-game-data.db" > "$CHECK_WORK/scope-pairs.tsv"
