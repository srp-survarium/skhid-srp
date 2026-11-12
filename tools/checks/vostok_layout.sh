#!/usr/bin/env bash
# Compose the candidate, check preservation, then test actual mounted geometry.
set -euo pipefail
. release/lib.sh
load_env SKHID_STOCK_DB SKHID_PACKS_DIR
bash tools/checks/build_content.sh >"$CHECK_WORK/build-content.log" 2>&1 || {
    tail -n 20 "$CHECK_WORK/build-content.log"
    exit 1
}
cache=${XDG_CACHE_HOME:-$HOME/.cache}/skhid-release/patches
layers=(--db "$SKHID_STOCK_DB")
while read -r name _; do
    [[ $name == resources.db.patch_* ]] || continue
    layers+=(--patch "$cache/$name")
done <release/patches.tsv
candidates=("$CHECK_WORK"/content/resources.db.patch_*)
[[ ${#candidates[@]} == 1 && -f ${candidates[0]} ]]
candidate=${candidates[0]}
python3 srp-026g/scripts/v011e-vostok-anomalies/check_release.py \
    --tool "$(game_data_bin)" "${layers[@]}" --candidate "$candidate" \
    --packs "$SKHID_PACKS_DIR" --work "$CHECK_WORK/comparison"
cargo run --quiet --release --manifest-path srp-026g/Cargo.toml -p mapdata \
    --example check_vostok_batteries -- "${layers[@]}" --patch "$candidate" \
    --research-config gameplay/game_modes/v011e_level_03_day_5.options
