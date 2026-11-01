#!/usr/bin/env bash
# usage: make_pack.sh <resource-porter checkout> <029a3|069d> <donor> <work folder> <version>
#
# The content pack tactical-clips-029@<version> or tactical-clips-069@<version>: live guns whose
# reload is 0.26g's one-clip one (state type 303) given a donor's two-clip reload (314), as the live
# vz. 58 and the other 0.29a3 ports reload (docs/notes/tactical-reload.md), into $SKHID_PACKS_DIR;
# prints its pack.json's sha256 for release/content.tsv.
#   1. resource-porter's port-tactical-reload (src/weapons_026/tactical.rs) of
#      ports/tactical-reload-<029a3|069d>.json from <donor> over what packs are composed over (the
#      stock resources.db and the manifest's base patches): each listed config with the donor's
#      tactical and empty clips and sound lists, the clips and sounds 0.26g lacks, each gun checked
#      against 0.26g's rig and rest pose;
#   2. vostok-port pack from-bundle against the same files.
# <donor>: 0.29a3's .split (its volumes, Survarium_archives.zip's Survarium29a); for 0.69d0 the
# archive of the files the reloads name, with gameplay/game.options:
#   scripts/weapons-069d-donor.py <survey vostok-port> <0.69d0 .split> paths069.txt LIST --reload <items>
#   <survey vostok-port> extract-bundle --db <0.69d0 .split> $(sed 's/^/--prefix /' LIST) --output TREE
#   cargo run --release --example pack-donor-tree -- TREE/overlay donor.db
# (the items: the manifest's donor_item values). Needs cargo (the porter's nix develop shell).
# Writes <work folder> (new) and the pack folder (new).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../.." && pwd)
(($# == 5)) || { sed -n '2,/^set/p' "$0" | sed '$d'; exit 2; }
porter=$(cd "$1" && pwd) build=$2 donor=$(realpath "$3") work=$4 version=$5
case $build in
    029a3) id=tactical-clips-029 ;;
    069d) id=tactical-clips-069 ;;
    *) echo "the donor build is 029a3 or 069d" >&2; exit 2 ;;
esac
: "${SKHID_STOCK_DB:?the stock resources.db}" "${SKHID_PACKS_DIR:?the packs folder}"
pack=$SKHID_PACKS_DIR/$id-$version
[[ ! -e $work && ! -e $pack ]] || { echo "$work or $pack is already there" >&2; exit 1; }
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
"$port" port-tactical-reload --manifest "$porter/ports/tactical-reload-$build.json" --donor "$donor" \
    --target-db "$SKHID_STOCK_DB" "${layers[@]}" --output "$work/bundle" >"$work/port.log"
"$port" pack from-bundle --base-db "$SKHID_STOCK_DB" "${bases[@]}" --bundle "$work/bundle" \
    --output "$pack" --id "$id" --version "$version"
echo "$id@$version: $(sha256sum "$pack/pack.json" | cut -d' ' -f1)"
