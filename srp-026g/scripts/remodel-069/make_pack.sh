#!/usr/bin/env bash
# usage: make_pack.sh <resource-porter checkout> <survey vostok-port> <0.69d0 .split> <0.69d0 paths> <work folder>
#            <version> <camo version>
#
# The content packs remodel-069@<version> (every live gun on 0.69d0's model, docs/notes/remodel-069.md)
# and remodel-069-camo@<camo version> (their camouflage masks) into $SKHID_PACKS_DIR; prints each
# pack.json's sha256 for release/content.tsv.
#   1. the 0.69d0 files the guns' models draw (resource-porter's scripts/weapons-069d-donor.py for
#      remodel-069d.json's guns, its models, materials and textures only), as one archive with
#      gameplay/game.options;
#   2. resource-porter's remodel-weapons-069d (docs/weapons-069d-026.md): each gun's model at its
#      model_path, on its live rig, over what players have (the stock resources.db and every patch
#      of release/patches.tsv: a 0.69d0 file at a path they have otherwise gets a _069 name);
#   3. vostok-port pack from-bundle against the same files (what they have the same is left out);
#   4. the camouflage masks (_aoac, client patch 44) of camo.tsv (../weapons-069/camo_rows.py writes it
#      from the bundle; checked) by ../weapon-camo/weapon_camo.py --pack --bare-masks: every large part
#      painted, parts.tsv leaves the cartridges and the MP7's glowing dots plain.
# <survey vostok-port>: one that reads 0.69d0's compressed volumes (the weapon survey's).
# Needs cargo (nix-shell -p cargo rustc gcc zlib), magick (the porter's texture encoder), Python with
# numpy and Pillow (weapon_camo.py).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../.." && pwd)
(($# == 7)) || { sed -n '2,/^set/p' "$0" | sed '$d'; exit 2; }
porter=$(cd "$1" && pwd) survey=$2 split=$3 paths069=$4 work=$5 version=$6 camo_version=$7
: "${SKHID_STOCK_DB:?the stock resources.db}" "${SKHID_PACKS_DIR:?the packs folder}"
pack=$SKHID_PACKS_DIR/remodel-069-$version camo=$SKHID_PACKS_DIR/remodel-069-camo-$camo_version
[[ ! -e $work && ! -e $pack && ! -e $camo ]] || { echo "$work, $pack or $camo is already there" >&2; exit 1; }
mkdir -p "$work"
cache=$HOME/.cache/skhid-release/patches
layers=() bases=()
while read -r n; do
    [[ -f $cache/resources.db.patch_$n ]] || { echo "$cache/resources.db.patch_$n is missing (release/build-content fetches it)" >&2; exit 1; }
    layers+=(--target-patch "$cache/resources.db.patch_$n") bases+=(--base-patch "$cache/resources.db.patch_$n")
done < <(grep -v '^#' "$root/release/patches.tsv" | cut -f1 | grep -o '[0-9]*$')
(cd "$porter" && cargo build -q --release --bin vostok-port --example pack-donor-tree)
port=$porter/target/release/vostok-port
mapfile -t guns < <(python3 -I -c "import json,sys; print('\n'.join(json.load(open(sys.argv[1]))['guns']))" "$here/remodel-069d.json")
python3 -I "$porter/scripts/weapons-069d-donor.py" "$survey" "$split" "$paths069" "$work/list.all" "${guns[@]}"
grep -E '^(models|material_instances|textures)/' "$work/list.all" >"$work/list"
mapfile -t prefixes < <(sed 's/^/--prefix\n/' "$work/list")
"$survey" extract-bundle --db "$split" "${prefixes[@]}" --output "$work/tree" >/dev/null
"$survey" extract --db "$split" --path gameplay/game.options --output "$work/tree/overlay/gameplay/game.options" >/dev/null
"$porter/target/release/examples/pack-donor-tree" "$work/tree/overlay" "$work/donor.db" >/dev/null
"$port" remodel-weapons-069d --id remodel-069 --donor-spec "$here/remodel-069d.json" --donor "$work/donor.db" \
    --target-db "$SKHID_STOCK_DB" "${layers[@]}" --output "$work/bundle" >"$work/remodel.log"
"$port" pack from-bundle --base-db "$SKHID_STOCK_DB" "${bases[@]}" --bundle "$work/bundle" \
    --output "$pack" --id remodel-069 --version "$version" >/dev/null
# Camouflage masks: weapon_camo.py reads the bundle's models and textures and the stock _aoac options
# it copies.
python3 -I "$here/../weapons-069/camo_rows.py" "$work/bundle" | tail -n +2 | diff - <(grep -v '^#' "$here/camo.tsv") \
    || { echo "camo.tsv is not the bundle's (../weapons-069/camo_rows.py)" >&2; exit 1; }
mkdir -p "$work/camo/textures/character/human/black_market_01"
cp -as "$(realpath "$work/bundle/overlay")/." "$work/camo/"
options=textures/character/human/black_market_01/black_market_01_torso_aoac.options
"$port" extract --db "$SKHID_STOCK_DB" --path "$options" --output "$work/camo/$options" >/dev/null
python3 -I "$root/srp-026g/scripts/weapon-camo/weapon_camo.py" "$here/camo.tsv" "$work/camo" "$work/camo" \
    "$camo" --pack "remodel-069-camo@$camo_version" --bare-masks >"$work/camo.log"
echo "remodel-069@$version: $(sha256sum "$pack/pack.json" | cut -d' ' -f1)"
echo "remodel-069-camo@$camo_version: $(sha256sum "$camo/pack.json" | cut -d' ' -f1)"
