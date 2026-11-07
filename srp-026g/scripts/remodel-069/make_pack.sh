#!/usr/bin/env bash
# usage: make_pack.sh <resource-porter checkout> <survey vostok-port> <0.69d0 .split> <0.69d0 paths> <work folder>
#            <version>
#
# The content packs remodel-069@<version> (every live gun on 0.69d0's model, docs/notes/remodel-069.md),
# remodel-069-camo@<version> (their camouflage masks) and remodel-069-variants@<version> (the live scope
# variants' weapon models on them) into $SKHID_PACKS_DIR, and ../content-edit/remodel-069-variants-1.json;
# prints each pack.json's sha256 for release/content.tsv.
#   1. the 0.69d0 files the guns' models draw (resource-porter's scripts/weapons-069d-donor.py for
#      remodel-069d.json's guns, its models, materials and textures only), as one archive with
#      gameplay/game.options;
#   2. resource-porter's remodel-weapons-069d (docs/weapons-069d-026.md): each gun's model at its
#      model_path, on its live rig, over what players have (the stock resources.db and every patch
#      of release/patches.tsv: a 0.69d0 file at a path they have otherwise gets a _069 name);
#   3. vostok-port pack from-bundle against the same files (what they have the same is left out);
#   4. the camouflage masks (_aoac, client patch 44) of camo.tsv (../weapons-069/camo_rows.py writes it
#      from the bundle, less the textures players have a mask of already, an earlier port's; checked)
#      by ../weapon-camo/weapon_camo.py --pack --bare-masks: every large part painted, parts.tsv leaves
#      the cartridges and the MP7's glowing dots plain;
#   5. variants.py over the players' scope items, dictionary and weapon models (srp-026g's
#      srp-game-data get, the same files).
# <survey vostok-port>: one that reads 0.69d0's compressed volumes (the weapon survey's).
# Needs cargo (nix-shell -p cargo rustc gcc zlib), magick (the porter's texture encoder), Python with
# numpy and Pillow (weapon_camo.py).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../.." && pwd)
(($# == 6)) || { sed -n '2,/^set/p' "$0" | sed '$d'; exit 2; }
porter=$(cd "$1" && pwd) survey=$2 split=$3 paths069=$4 work=$5 version=$6
: "${SKHID_STOCK_DB:?the stock resources.db}" "${SKHID_PACKS_DIR:?the packs folder}"
pack=$SKHID_PACKS_DIR/remodel-069-$version camo=$SKHID_PACKS_DIR/remodel-069-camo-$version
variants=$SKHID_PACKS_DIR/remodel-069-variants-$version
for d in "$work" "$pack" "$camo" "$variants"; do
    [[ ! -e $d ]] || { echo "$d is already there" >&2; exit 1; }
done
mkdir -p "$work"
cache=$HOME/.cache/skhid-release/patches
layers=() bases=() mounted=(--db "$SKHID_STOCK_DB")
while read -r n; do
    [[ -f $cache/resources.db.patch_$n ]] || { echo "$cache/resources.db.patch_$n is missing (release/build-content fetches it)" >&2; exit 1; }
    layers+=(--target-patch "$cache/resources.db.patch_$n") bases+=(--base-patch "$cache/resources.db.patch_$n")
    mounted+=(--patch "$cache/resources.db.patch_$n")
done < <(grep -v '^#' "$root/release/patches.tsv" | cut -f1 | grep -o '[0-9]*$')
(cd "$porter" && cargo build -q --release --bin vostok-port --example pack-donor-tree)
game_data=${SRP_GAME_DATA_BIN:-$root/srp-026g/target/release/srp-game-data}
[[ -n ${SRP_GAME_DATA_BIN:-} ]] || (cd "$root" && nix develop ./srp-026g -c bash -c \
    "cd srp-026g && cargo build --quiet --release -p resources-db --bin srp-game-data")
port=$porter/target/release/vostok-port
"$game_data" files "${mounted[@]}" | cut -f1 >"$work/players.paths"
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
python3 -I "$here/../weapons-069/camo_rows.py" "$work/bundle" | tail -n +2 \
    | awk -F'\t' 'NR == FNR { have[$1] = 1; next } !(("textures/" $1 "_aoac.dds") in have)' "$work/players.paths" - \
    | diff - <(grep -v '^#' "$here/camo.tsv") \
    || { echo "camo.tsv is not the bundle's (../weapons-069/camo_rows.py, less the players' masks)" >&2; exit 1; }
mkdir -p "$work/camo/textures/character/human/black_market_01"
cp -as "$(realpath "$work/bundle/overlay")/." "$work/camo/"
options=textures/character/human/black_market_01/black_market_01_torso_aoac.options
"$port" extract --db "$SKHID_STOCK_DB" --path "$options" --output "$work/camo/$options" >/dev/null
python3 -I "$root/srp-026g/scripts/weapon-camo/weapon_camo.py" "$here/camo.tsv" "$work/camo" "$work/camo" \
    "$camo" --pack "remodel-069-camo@$version" --bare-masks >"$work/camo.log"
# The live scope variants' weapon models on the remodels.
grep -E '^(gameplay/items/scopes/|gameplay/db_static_dictionaries$|models/weapons/.*\.skinned_model/)' \
    "$work/players.paths" >"$work/live.list"
"$game_data" get "${mounted[@]}" --out "$work/live" --paths "$work/live.list" >/dev/null
python3 -I "$here/variants.py" "$work/live" "$work/bundle" "$variants" "$version" >"$work/variants.log"
echo "remodel-069@$version: $(sha256sum "$pack/pack.json" | cut -d' ' -f1)"
echo "remodel-069-camo@$version: $(sha256sum "$camo/pack.json" | cut -d' ' -f1)"
echo "remodel-069-variants@$version: $(sha256sum "$variants/pack.json" | cut -d' ' -f1)"
