#!/usr/bin/env bash
# usage: make_pack.sh <resource-porter checkout> <donor.db> <work folder> <version>
#
# The content pack pkp-animations-069@<version>: the live PKP Pecheneg's idle, fire, show, hide
# and reload clips 0.69d0's (newer than 0.26g's and 0.29a3's, the same rig; 0.46c2 has the same
# but the hands' show), under animations/single/weapons/pkp_pecheneg_069d0/, and its models' left
# hand points (LeftHand_idle and the fingers', where the hands' IK puts them) 0.69d0's
# (docs/notes/pkp-animations.md); into $SKHID_PACKS_DIR; prints its pack.json's sha256 for
# release/content.tsv.
#   1. resource-porter's port-animation-set (src/weapons_026/animation_set.rs) of
#      ports/animations-pkp-069d.json from <donor.db> over what packs are composed over (the stock
#      resources.db and the manifest's base patches), the clips checked against 0.26g's rig and
#      the gun's idle;
#   2. vostok-port pack from-bundle against the same files.
# <donor.db>: 0.69d0's gameplay/game.options, the PKP's clips (pkp_pecheneg/{item,player}/...
# idle, shoot, show, hide, reload) and its model's render/export_properties
# (models/weapons/lmg/pkp_pecheneg/pkp_pecheneg.skinned_model), extracted from 0.69d0's volumes
# (<survey vostok-port> extract-bundle --db <0.69d0 .split> --prefix <each> --output TREE) and
# packed (cargo run --release --example pack-donor-tree -- TREE/overlay donor.db).
# Writes <work folder> (new) and the pack folder (new).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../.." && pwd)
(($# == 4)) || { sed -n '2,/^set/p' "$0" | sed '$d'; exit 2; }
porter=$(cd "$1" && pwd) donor=$(realpath "$2") work=$3 version=$4
: "${SKHID_STOCK_DB:?the stock resources.db}" "${SKHID_PACKS_DIR:?the packs folder}"
pack=$SKHID_PACKS_DIR/pkp-animations-069-$version
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
"$port" port-animation-set --manifest "$porter/ports/animations-pkp-069d.json" --donor "$donor" \
    --target-db "$SKHID_STOCK_DB" "${layers[@]}" --output "$work/bundle" >"$work/port.log"
"$port" pack from-bundle --base-db "$SKHID_STOCK_DB" "${bases[@]}" --bundle "$work/bundle" \
    --output "$pack" --id pkp-animations-069 --version "$version"
echo "pkp-animations-069@$version: $(sha256sum "$pack/pack.json" | cut -d' ' -f1)"
