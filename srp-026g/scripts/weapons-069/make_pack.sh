#!/usr/bin/env bash
# usage: make_pack.sh <resource-porter checkout> <donor.db> <work folder> <version>
#
# The content pack weapons-069@<version>: the 0.69d0 weapons (docs/notes/weapons-069.md), into
# $SKHID_PACKS_DIR/weapons-069-<version>; prints pack.json's sha256 for release/content.tsv.
#   1. resource-porter's build-weapons-069d (the porter's docs/weapons-069d-026.md): the bundle,
#      from <donor.db> (the 0.69d0 files, its scripts/weapons-069d-donor.py) over what packs are
#      composed over: the stock resources.db and the manifest's base patches;
#   2. the lobby icons (render-weapons-069d-icons.py) into the cells the configs name;
#   3. each body texture's camouflage mask (_aoac), by ../weapon-camo/weapon_camo.py from
#      camo_rows.py's rows (client patch 44);
#   4. vostok-port pack from-bundle against the same players' files.
# Needs cargo (nix-shell -p cargo rustc gcc zlib), Python with numpy and Pillow, magick (the
# porter's texture encoder). Writes <work folder> (new) and the pack folder (new).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../.." && pwd)
(($# == 4)) || { sed -n '2,/^set/p' "$0" | sed '$d'; exit 2; }
porter=$(cd "$1" && pwd) donor=$(realpath "$2") work=$3 version=$4
: "${SKHID_STOCK_DB:?the stock resources.db}" "${SKHID_PACKS_DIR:?the packs folder}"
pack=$SKHID_PACKS_DIR/weapons-069-$version
[[ ! -e $work && ! -e $pack ]] || { echo "$work or $pack is already there" >&2; exit 1; }
mkdir -p "$work"
# What packs are composed over (release/content.tsv's base rows): the item configs the port starts
# from must be the ones the edit lists start from, which then apply to the port's as to the rest.
cache=$HOME/.cache/skhid-release/patches
layers=() bases=()
while IFS=$'\t' read -r kind name _; do
    [[ $kind == base ]] || continue
    [[ -f $cache/$name ]] || { echo "$cache/$name is missing (release/build-content fetches it)" >&2; exit 1; }
    layers+=(--target-patch "$cache/$name") bases+=(--base-patch "$cache/$name")
done <"$root/release/content.tsv"
(cd "$porter" && cargo build -q --release --example build-weapons-069d --bin vostok-port)
port=$porter/target/release
"$port/examples/build-weapons-069d" --donor "$donor" --target-db "$SKHID_STOCK_DB" "${layers[@]}" \
    --icons --output "$work/bundle"
# The players' lobby movie (the last patch's): the native icons' exposure and the cells, which must
# be empty.
last=$cache/$(cut -f1 "$root/release/patches.tsv" | grep '^resources.db.patch_' | tail -1)
"$port/vostok-port" extract --db "$last" --path flash_movies/lobby_menu.swf --output "$work/lobby_menu.swf" >/dev/null
python3 -I "$porter/scripts/render-weapons-069d-icons.py" --bundle "$work/bundle" --lobby-swf "$work/lobby_menu.swf"
# Camouflage masks: weapon_camo.py reads the bundle's models and textures and the stock _aoac
# options it copies.
mkdir -p "$work/camo/game/textures/character/human/black_market_01"
cp -as "$(realpath "$work/bundle/overlay")/." "$work/camo/game/"
options=textures/character/human/black_market_01/black_market_01_torso_aoac.options
"$port/vostok-port" extract --db "$SKHID_STOCK_DB" --path "$options" --output "$work/camo/game/$options" >/dev/null
python3 -I "$here/camo_rows.py" "$work/bundle" >"$work/camo/guns.tsv"
python3 -I "$root/srp-026g/scripts/weapon-camo/weapon_camo.py" "$work/camo/guns.tsv" "$work/camo/game" \
    "$work/camo/game" "$work/camo/out"
python3 -I "$here/camo_rows.py" "$work/bundle" --add "$work/camo/out"
"$port/vostok-port" pack from-bundle --base-db "$SKHID_STOCK_DB" "${bases[@]}" --bundle "$work/bundle" \
    --output "$pack" --id weapons-069 --version "$version"
echo "weapons-069@$version: $(sha256sum "$pack/pack.json" | cut -d' ' -f1)"
