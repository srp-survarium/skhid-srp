#!/usr/bin/env bash
# usage: make_pack.sh <resource-porter checkout> <0.69d0 .split> <survey vostok-port> <work folder> <version>
#
# The content pack muzzle-devices@<version> and its edit list (docs/notes/muzzle-devices.md), into
# $SKHID_PACKS_DIR/muzzle-devices-<version> and ../content-edit/muzzle-devices-1.json; prints
# pack.json's sha256 for release/content.tsv.
#   1. 0.69d0's files the port reads (donor_paths.py: game.options, five languages, every
#      muzzle-device model with its materials and textures, every weapon's render folder), packed
#      as one resources.db by the porter's pack-donor-tree (a vostok-port that reads 0.69d0's
#      split volumes: the weapon survey's);
#   2. resource-porter's port-muzzles (its docs/muzzles-069d-026.md) from its
#      ports/muzzle-devices-069d.json, over what packs are composed over (the stock resources.db
#      and the manifest's base patches): the bundle, the guns' edit list, the renders;
#   3. the device icons (render-muzzle-icons.py) into their cells of the players' lobby movie;
#   4. vostok-port pack from-bundle against the same players' files.
# Needs cargo (the porter's nix develop), Python with numpy and Pillow, magick. Writes <work folder>
# (new; the renders are <work folder>/bundle/renders) and the pack folder (new).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../.." && pwd)
(($# == 5)) || { sed -n '2,/^set/p' "$0" | sed '$d'; exit 2; }
porter=$(cd "$1" && pwd) split=$(realpath "$2") survey=$(realpath "$3") work=$4 version=$5
: "${SKHID_STOCK_DB:?the stock resources.db}" "${SKHID_PACKS_DIR:?the packs folder}"
pack=$SKHID_PACKS_DIR/muzzle-devices-$version
[[ ! -e $work && ! -e $pack ]] || { echo "$work or $pack is already there" >&2; exit 1; }
mkdir -p "$work"
work=$(cd "$work" && pwd)
cache=$HOME/.cache/skhid-release/patches
layers=() bases=()
while IFS=$'\t' read -r kind name _; do
    [[ $kind == base ]] || continue
    [[ -f $cache/$name ]] || { echo "$cache/$name is missing (release/build-content fetches it)" >&2; exit 1; }
    layers+=(--target-patch "$cache/$name") bases+=(--base-patch "$cache/$name")
done <"$root/release/content.tsv"
(cd "$porter" && cargo build -q --release --example pack-donor-tree --bin vostok-port)
port=$porter/target/release
"$survey" list --db "$split" --prefix "" >"$work/paths069.txt"
python3 -I "$here/donor_paths.py" "$survey" "$split" "$work/paths069.txt" "$work/donor_paths.txt"
prefixes=()
while read -r p; do prefixes+=(--prefix "$p"); done <"$work/donor_paths.txt"
"$survey" extract-bundle --db "$split" "${prefixes[@]}" --output "$work/donor-tree" >/dev/null
"$port/examples/pack-donor-tree" "$work/donor-tree/overlay" "$work/donor.db"
# The texture encoder (a sandboxed magick) sees only the folder it runs in.
(cd "$work" && "$port/vostok-port" port-muzzles --manifest "$porter/ports/muzzle-devices-069d.json" \
    --donor "$work/donor.db" --target-db "$SKHID_STOCK_DB" "${layers[@]}" --output "$work/bundle")
last=$cache/$(cut -f1 "$root/release/patches.tsv" | grep '^resources.db.patch_' | tail -1)
"$port/vostok-port" extract --db "$last" --path flash_movies/lobby_menu.swf --output "$work/lobby_menu.swf" >/dev/null
python3 -I "$porter/scripts/render-muzzle-icons.py" --bundle "$work/bundle" --lobby-swf "$work/lobby_menu.swf"
python3 -I "$here/edit_list.py" "$work/bundle/muzzle-edits.json" "$root/srp-026g/scripts/content-edit/muzzle-devices-1.json"
"$port/vostok-port" pack from-bundle --base-db "$SKHID_STOCK_DB" "${bases[@]}" --bundle "$work/bundle" \
    --output "$pack" --id muzzle-devices --version "$version"
echo "muzzle-devices@$version: $(sha256sum "$pack/pack.json" | cut -d' ' -f1)"
