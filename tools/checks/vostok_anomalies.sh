#!/usr/bin/env bash
set -euo pipefail
. release/lib.sh
load_env SKHID_STOCK_DB SKHID_PACKS_DIR
bin=$(game_data_bin)
script=srp-026g/scripts/v011e-vostok-anomalies
cache=${XDG_CACHE_HOME:-$HOME/.cache}/skhid-release/patches
layers=(--db "$SKHID_STOCK_DB")
for revision in 20 22 23; do layers+=(--patch "$cache/resources.db.patch_$revision"); done
awk '{print $2}' "$script/inputs.sha256" >"$CHECK_WORK/paths"
"$bin" get "${layers[@]}" --paths "$CHECK_WORK/paths" --out "$CHECK_WORK/base"
"$bin" files "${layers[@]}" >"$CHECK_WORK/files.tsv"
python3 "$script/make_pack.py" --base "$CHECK_WORK/base" --out "$CHECK_WORK/pack"
python3 "$script/check_pack.py" --base "$CHECK_WORK/base" --pack "$CHECK_WORK/pack" --files "$CHECK_WORK/files.tsv"
python3 "$script/make_modes_edit.py" --base "$CHECK_WORK/base" \
    --research-pack "$SKHID_PACKS_DIR/research-mode-3" --out "$CHECK_WORK/modes.json"
cmp "$CHECK_WORK/modes.json" srp-026g/scripts/content-edit/v011e-vostok-modes-1.json
