#!/usr/bin/env bash
set -euo pipefail
. release/lib.sh
load_env SKHID_STOCK_DB SKHID_PACKS_DIR
src=srp-026g/scripts/weapon-decal-uv
tools/dev c++ -std=c++17 -O2 "$src/check_layout.cpp" -o "$CHECK_WORK/check-layout"
python3 -I "$src/check_pack.py" --manifest release/content.tsv --packs "$SKHID_PACKS_DIR" \
  --game-data "$(game_data_bin)" --stock "$SKHID_STOCK_DB" --checker "$CHECK_WORK/check-layout" --work "$CHECK_WORK"
