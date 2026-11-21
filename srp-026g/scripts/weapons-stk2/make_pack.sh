#!/usr/bin/env bash
# usage: make_pack.sh <resource-porter checkout> <stk2-ak74 folder> <work folder> <version> <camo version>
#            <scopes version>
#
# The content packs weapons-stk2@<version> (the two STK2 AK-74s, docs/notes/weapons-stk2.md),
# weapons-stk2-camo@<camo version> (their camouflage masks) and weapons-stk2-scopes@<scopes
# version> (their scope fits) into $SKHID_PACKS_DIR; prints each pack.json's sha256 for
# release/content.tsv, and writes ../content-edit/weapons-stk2-scopes-1.json.
#   1. donor.py: STK2's two guns in 0.26g's layouts on the live AK-74M's rig (<stk2-ak74>: the
#      files of /mnt/toshiba/surv-builds/STK2.7z under xr2-prog/resources/), packed into one
#      archive (resource-porter's pack-donor-tree); the AK-74M's two-clip reload from the patch
#      that shipped pack tactical-clips-029 (revision 43);
#   2. resource-porter's port-weapons (ports/weapons-stk2.json, donor build "stk2") over the
#      stock resources.db and every patch of release/patches.tsv (the AK-74M's clips, sounds and
#      rounds are reused, not shipped), and vostok-port pack from-bundle against the same files;
#   3. the camouflage masks (_aoac, client patch 44) of camo.tsv by ../weapon-camo/weapon_camo.py
#      --pack --bare-masks (the large parts of each texture, its suggestion);
#   4. scope_fits.py: every optic the AK-74M takes, on both, from the live files.
# Needs cargo (the porter's dev shell: its ImageMagick), Python with NumPy, SciPy and Pillow.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../.." && pwd)
(($# == 6)) || { sed -n '2,/^set/p' "$0" | sed '$d'; exit 2; }
porter=$(cd "$1" && pwd) stk2=$(realpath "$2") work=$3 version=$4 camo_version=$5 scopes_version=$6
: "${SKHID_STOCK_DB:?the stock resources.db}" "${SKHID_PACKS_DIR:?the packs folder}"
pack=$SKHID_PACKS_DIR/weapons-stk2-$version camo=$SKHID_PACKS_DIR/weapons-stk2-camo-$camo_version
scopes=$SKHID_PACKS_DIR/weapons-stk2-scopes-$scopes_version
[[ ! -e $work && ! -e $pack && ! -e $camo && ! -e $scopes ]] || { echo "$work or a pack folder is already there" >&2; exit 1; }
mkdir -p "$work"
cache=$HOME/.cache/skhid-release/patches
layers=() bases=()
while IFS=$'\t' read -r name sha _; do
    [[ $name == resources.db.patch_* ]] || continue
    [[ -f $cache/$name ]] || { echo "$cache/$name is missing (release/build-content fetches it)" >&2; exit 1; }
    layers+=(--target-patch "$cache/$name") bases+=(--base-patch "$cache/$name")
done <"$root/release/patches.tsv"
(cd "$porter" && nix develop -c cargo build -q --release --bin vostok-port --example pack-donor-tree)
port=$porter/target/release/vostok-port
magick=$(cd "$porter" && nix develop -c bash -c 'command -v magick')
"$port" extract --db "$cache/resources.db.patch_43" --path gameplay/items/weapons/wpn_ak_74m.options \
    --output "$work/wpn_ak_74m.tactical.options" >/dev/null
python3 -I "$here/donor.py" --porter "$port" --stock "$SKHID_STOCK_DB" --stk2 "$stk2" \
    --reload-from "$work/wpn_ak_74m.tactical.options" --magick "$magick" --out "$work/donor"
"$porter/target/release/examples/pack-donor-tree" "$work/donor" "$work/donor.db"
"$port" port-weapons --manifest "$porter/ports/weapons-stk2.json" --donor "$work/donor.db" \
    --target-db "$SKHID_STOCK_DB" "${layers[@]}" --output "$work/bundle" >"$work/port.log"
"$port" pack from-bundle --base-db "$SKHID_STOCK_DB" "${bases[@]}" --bundle "$work/bundle" \
    --output "$pack" --id weapons-stk2 --version "$version"
# Camouflage masks: weapon_camo.py reads the bundle's models and textures and a stock _aoac's
# options it copies.
mkdir -p "$work/camo/textures/character/human/black_market_01"
cp -as "$(realpath "$work/bundle/overlay")/." "$work/camo/"
options=textures/character/human/black_market_01/black_market_01_torso_aoac.options
"$port" extract --db "$SKHID_STOCK_DB" --path "$options" --output "$work/camo/$options" >/dev/null
python3 -I "$root/srp-026g/scripts/weapon-camo/weapon_camo.py" "$here/camo.tsv" "$work/camo" "$work/camo" \
    "$camo" --pack "weapons-stk2-camo@$camo_version" --bare-masks
# Scope fits: the AK-74M's variants, their models and their weapon models, from the live files.
python3 -I "$here/live_files.py" --porter "$port" --stock "$SKHID_STOCK_DB" "${layers[@]}" --out "$work/live"
python3 -I "$here/scope_fits.py" --live "$work/live" --guns "$work/bundle/overlay" --out "$scopes" \
    --version "$scopes_version"
echo "weapons-stk2@$version: $(sha256sum "$pack/pack.json" | cut -d' ' -f1)"
echo "weapons-stk2-camo@$camo_version: $(sha256sum "$camo/pack.json" | cut -d' ' -f1)"
echo "weapons-stk2-scopes@$scopes_version: $(sha256sum "$scopes/pack.json" | cut -d' ' -f1)"
