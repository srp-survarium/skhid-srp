#!/usr/bin/env bash
# build.sh DONOR.split STOCK.db CONVERTER PACKER OUT PACKS
# CONVERTER: resource-porter 9863002dd8a734c24c53bff587af6f3e5af4982e.
# PACKER: the current resource-porter pack command used by release/build-content.
set -euo pipefail
[[ $# == 6 ]] || { echo 'usage: build.sh DONOR.split STOCK.db CONVERTER PACKER OUT PACKS' >&2; exit 2; }
here=$(dirname "$(readlink -f "$0")")
donor=$1 stock=$2 converter=$3 packer=$4 out=$5 packs=$6
mkdir -p "$out"
"$converter" build-survarium-069d-map --manifest "$here/port.json" \
    --donor-db "$donor" --target-db "$stock" --output "$out/bundle"
"$packer" pack from-bundle --base-db "$stock" --bundle "$out/bundle" \
    --output "$packs/range-069-1" --id range-069 --version 1
"$converter" extract --db "$donor" --path flash_movies/textures/ui_hud_maps/shooting_range.png \
    --output "$out/donor-minimap.png"
"$converter" extract --db "$donor" --path flash_movies/textures/ui_loading/shooting_range.png \
    --output "$out/donor-loading.png"
"${PYTHON:-python3}" -I "$here/ui.py" "$out"
sha256sum "$packs/range-069-1/pack.json"
