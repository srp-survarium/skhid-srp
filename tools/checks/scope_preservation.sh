#!/usr/bin/env bash
# Content-pipeline output must extend the released catalog without losing saved fits.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
bash "$root/tools/checks/build_content.sh"
. "$root/release/lib.sh"
load_env SKHID_STOCK_DB
use_patches live
read_patch_list
cache_patches
bin=$(game_data_bin)
args=()
while read -r name sha size urls; do
    [[ -n $name ]] || continue
    args+=(--patch "$PATCH_CACHE/$name=$urls")
done <<<"$LIST"
"$bin" build --db "$SKHID_STOCK_DB" "${args[@]}" --out "$CHECK_WORK/released.db"
nix develop "$root/srp-026g" --command cargo run --quiet \
    --manifest-path "$root/srp-026g/Cargo.toml" -p survarium --bin srp-check-scope-fits -- \
    "$CHECK_WORK/released.db" "$CHECK_WORK/content/server-game-data.db" > "$CHECK_WORK/scope-pairs.tsv"
