#!/usr/bin/env bash
set -euo pipefail
. release/lib.sh
load_env SKHID_STOCK_DB SKHID_PACKS_DIR
pack=$SKHID_PACKS_DIR/stalker-maps-6
"$(game_data_bin)" get --db "$SKHID_STOCK_DB" --out "$CHECK_WORK/native" \
    models/level/04/level_border.model/settings \
    models/level/04/level_border.model/render/export_properties \
    models/level/04/level_border.model/render/donotwalkhere_sg/export_properties \
    models/level/04/level_border.model/render/donotwalkhere_sg/converted_model \
    models/level/04/level_border.model/walker_collision/exported_primitives
python3 srp-026g/scripts/stalker-maps/check_sources.py --pack "$pack" --native "$CHECK_WORK/native"
cargo test --locked --manifest-path srp-026g/scripts/stalker-maps/ffa-points/Cargo.toml
cargo clippy --locked --manifest-path srp-026g/scripts/stalker-maps/ffa-points/Cargo.toml --all-targets -- -D warnings
