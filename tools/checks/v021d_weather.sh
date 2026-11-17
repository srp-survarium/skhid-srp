#!/usr/bin/env bash
set -euo pipefail
. release/lib.sh
load_env SKHID_STOCK_DB SKHID_VOSTOK_PORT SKHID_PACKS_DIR
common=$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")
cache=${XDG_CACHE_HOME:-$HOME/.cache}/skhid-release
python3 -m unittest discover -s srp-026g/scripts/v021d-weather -p 'test_*.py'
python3 srp-026g/scripts/v021d-weather/source_projects.py \
    --stock "$SKHID_STOCK_DB" --patches "$cache/patches" \
    --game-data "$(game_data_bin)" --porter "$SKHID_VOSTOK_PORT" \
    --packs "$SKHID_PACKS_DIR" --flash-tools "${SKHID_FLASH_TOOLS:-$common/private/actionscript/tools.json}" \
    --cache "$cache/build-content" --out "$CHECK_WORK/day-source"
python3 srp-026g/scripts/v021d-weather/make_geometry.py \
    --donor "$common/private/v021d-five-weathers/donor-layout" \
    --imported "$common/private/v021d-five-weathers/geometry-import" \
    --pack "$SKHID_PACKS_DIR/maps-021d-weathers-2" \
    --out "$CHECK_WORK/geometry.json" --provenance "$CHECK_WORK/geometry-inputs.json"
cmp "$CHECK_WORK/geometry.json" srp-026g/scripts/content-edit/v021d-weather-geometry-1.json
cmp "$CHECK_WORK/geometry-inputs.json" srp-026g/scripts/v021d-weather/geometry-inputs.json
python3 srp-026g/scripts/v021d-weather/check_packs.py --packs "$SKHID_PACKS_DIR" --source "$CHECK_WORK/day-source"
bash tools/checks/build_content.sh >"$CHECK_WORK/build-content.log" 2>&1 || {
    tail -n 20 "$CHECK_WORK/build-content.log"
    exit 1
}
nix develop ./srp-026g --command bash -euo pipefail -c '
    cargo build --manifest-path srp-026g/Cargo.toml -p survarium --lib --message-format=json >"$CHECK_WORK/cargo.json"
    library=$(python3 -c '\''import json, os
from pathlib import Path
rows = [json.loads(s) for s in (Path(os.environ["CHECK_WORK"]) / "cargo.json").read_text().splitlines()]
files = [p for r in rows if r.get("reason") == "compiler-artifact" and r["target"]["name"] == "survarium" for p in r["filenames"] if p.endswith(".rlib")]
assert len(files) == 1
print(files[0])'\'')
    rustc --edition=2024 srp-026g/scripts/v021d-weather/check_data.rs \
        -L "dependency=$(dirname "$library")/deps" --extern "survarium=$library" -o "$CHECK_WORK/check-weather-data"
    "$CHECK_WORK/check-weather-data" "$CHECK_WORK/content/server-game-data.db"
'
# Follow every model, collision, material, particle and texture reference from
# the final projects over the same mounted archives the client receives.
weather_porter=${SKHID_WEATHER_PORTER:-$common/private/v021d-five-weathers/target/release/vostok-port}
layers=()
while read -r patch _; do
    [[ $patch == resources.db.patch_* ]] || continue
    layers+=(--donor-patch "$cache/patches/$patch" --target-patch "$cache/patches/$patch")
done <release/patches.tsv
candidate=("$CHECK_WORK"/content/resources.db.patch_*)
[[ ${#candidate[@]} == 1 && -f ${candidate[0]} ]]
layers+=(--donor-patch "${candidate[0]}" --target-patch "${candidate[0]}")
roots=()
for level in 01 02 03 05 06; do
    for weather in rain evening; do
        for mode in 2 4; do
            roots+=(--root "projects/v021d_level_$level/${weather}_$mode/client_project")
        done
    done
done
"$weather_porter" scan --donor-db "$SKHID_STOCK_DB" --target-db "$SKHID_STOCK_DB" \
    "${layers[@]}" "${roots[@]}" --output "$CHECK_WORK/references.json"
python3 -c 'import json, sys; p=json.load(open(sys.argv[1])); assert not p["unresolved_references"], p["unresolved_references"]; print("All final weather model/material/particle/texture references resolve over the composed archives")' "$CHECK_WORK/references.json"
