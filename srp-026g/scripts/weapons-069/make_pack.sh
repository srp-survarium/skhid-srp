#!/usr/bin/env bash
# usage: make_pack.sh <resource-porter checkout> <donor.db> <work folder> <version> <camo version>
#            [<pack id>@<version> ...]
#
# The content packs weapons-069@<version> (the 0.69d0 weapons, docs/notes/weapons-069.md) and
# weapons-069-camo@<camo version> (their camouflage masks), into $SKHID_PACKS_DIR; prints each
# pack.json's sha256 for release/content.tsv.
#   0. the packs named after the versions (weapons-031@3 weapons-046@3: the other weapon batches,
#      whose rows come before this one's) composed into one archive over that base, so the port
#      reuses what they add (the MP5's sounds, weapons-046's, that the G3SG/1 plays) instead of
#      adding it again;
#   1. resource-porter's port-weapons-069d (its shared weapon core with 0.69d0 as the donor,
#      docs/weapons-069d-026.md): the bundle of ports/weapons-069d.json and its sidecar, from
#      <donor.db> (the 0.69d0 files, its scripts/weapons-069d-donor.py) over what packs are
#      composed over: the stock resources.db and the manifest's base patches (with each gun's
#      lobby icon: 0.69d0's own, levelled);
#   2. vostok-port pack from-bundle against the same files;
#   3. the camouflage masks (_aoac, client patch 44) of camo.tsv (camo_rows.py writes it from the
#      bundle; checked) by ../weapon-camo/weapon_camo.py --pack --bare-masks: parts.tsv leaves the
#      cartridges and the loose shell plain, and each still gets a mask (one that paints nothing), so
#      every surface of a camouflaged gun finds its _aoac. The configs take the camouflage by the
#      edit list weapon-camo-069-1.json.
# Needs cargo (nix-shell -p cargo rustc gcc zlib), Python with numpy and Pillow (weapon_camo.py), magick (the
# porter's texture encoder). Writes <work folder> (new) and the pack folders (new).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../.." && pwd)
(($# >= 5)) || { sed -n '2,/^set/p' "$0" | sed '$d'; exit 2; }
porter=$(cd "$1" && pwd) donor=$(realpath "$2") work=$3 version=$4 camo_version=$5
shift 5
: "${SKHID_STOCK_DB:?the stock resources.db}" "${SKHID_PACKS_DIR:?the packs folder}"
pack=$SKHID_PACKS_DIR/weapons-069-$version camo=$SKHID_PACKS_DIR/weapons-069-camo-$camo_version
[[ ! -e $work && ! -e $pack && ! -e $camo ]] || { echo "$work, $pack or $camo is already there" >&2; exit 1; }
mkdir -p "$work"
# What packs are composed over (release/content.tsv's base rows).
cache=$HOME/.cache/skhid-release/patches
layers=() bases=()
while IFS=$'\t' read -r kind name _; do
    [[ $kind == base ]] || continue
    [[ -f $cache/$name ]] || { echo "$cache/$name is missing (release/build-content fetches it)" >&2; exit 1; }
    layers+=(--target-patch "$cache/$name") bases+=(--base-patch "$cache/$name")
done <"$root/release/content.tsv"
(cd "$porter" && cargo build -q --release --bin vostok-port)
port=$porter/target/release/vostok-port
if (($#)); then
    over=()
    for p in "$@"; do over+=(--pack "$SKHID_PACKS_DIR/${p%@*}-${p#*@}"); done
    "$port" pack patch --base-db "$SKHID_STOCK_DB" "${bases[@]}" "${over[@]}" --out-dir "$work/over" \
        --archive "$work/resources.db.patch_255" >/dev/null
    layers+=(--target-patch "$work/resources.db.patch_255") bases+=(--base-patch "$work/resources.db.patch_255")
fi
"$port" port-weapons-069d --manifest "$porter/ports/weapons-069d.json" \
    --donor-spec "$porter/ports/donor-weapons-069d.json" --donor "$donor" \
    --target-db "$SKHID_STOCK_DB" "${layers[@]}" --output "$work/bundle" >"$work/port.log"
"$port" pack from-bundle --base-db "$SKHID_STOCK_DB" "${bases[@]}" --bundle "$work/bundle" \
    --output "$pack" --id weapons-069 --version "$version"
# Camouflage masks: weapon_camo.py reads the bundle's models and textures and the stock _aoac
# options it copies.
python3 -I "$here/camo_rows.py" "$work/bundle" | diff - "$here/camo.tsv" \
    || { echo "camo.tsv is not the bundle's (camo_rows.py)" >&2; exit 1; }
mkdir -p "$work/camo/textures/character/human/black_market_01"
cp -as "$(realpath "$work/bundle/overlay")/." "$work/camo/"
options=textures/character/human/black_market_01/black_market_01_torso_aoac.options
"$port" extract --db "$SKHID_STOCK_DB" --path "$options" --output "$work/camo/$options" >/dev/null
python3 -I "$root/srp-026g/scripts/weapon-camo/weapon_camo.py" "$here/camo.tsv" "$work/camo" "$work/camo" \
    "$camo" --pack "weapons-069-camo@$camo_version" --bare-masks
echo "weapons-069@$version: $(sha256sum "$pack/pack.json" | cut -d' ' -f1)"
echo "weapons-069-camo@$camo_version: $(sha256sum "$camo/pack.json" | cut -d' ' -f1)"
