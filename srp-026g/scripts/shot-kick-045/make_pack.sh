#!/usr/bin/env bash
# usage: make_pack.sh <0.45a6 resources.00.db> <work folder>
#
# The content pack shot-kick-045@<gen.py's VERSION>: the stock guns' item fire clips with 0.45a6's
# bone curves (docs/notes/shot-kick-045.md), into $SKHID_PACKS_DIR; prints its pack.json's sha256
# and its row for release/content.tsv.
#   1. the live clips gen.py names (--paths), from what packs are composed over: the stock
#      resources.db and the manifest's base patches, the newest that has the file (the Mosin's
#      clip is base patch 17's);
#   2. 0.45a6's clips at the same paths, from <0.45a6 resources.00.db> (its unpacked volume with
#      animations/single/weapons);
#   3. gen.py makes the pack (it refuses a clip of another rig), vostok-port pack check reads it
#      against the same base.
# Settings: SKHID_STOCK_DB, SKHID_PACKS_DIR, SKHID_VOSTOK_PORT (resource-porter's vostok-port).
# Writes <work folder> (new) and the pack folder (new).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../.." && pwd)
(($# == 2)) || { sed -n '2,/^set/p' "$0" | sed '$d'; exit 2; }
donor=$(realpath "$1") work=$2
: "${SKHID_STOCK_DB:?the stock resources.db}" "${SKHID_PACKS_DIR:?the packs folder}" "${SKHID_VOSTOK_PORT:?vostok-port}"
version=$(sed -n "s/^VERSION = '\(.*\)'$/\1/p" "$here/gen.py")
pack=$SKHID_PACKS_DIR/shot-kick-045-$version
[[ ! -e $work && ! -e $pack ]] || { echo "$work or $pack is already there" >&2; exit 1; }
mkdir -p "$work"
# What packs are composed over (release/content.tsv's base rows), the newest first.
cache=$HOME/.cache/skhid-release/patches
layers=("$SKHID_STOCK_DB") bases=()
while IFS=$'\t' read -r kind name _; do
    [[ $kind == base ]] || continue
    [[ -f $cache/$name ]] || { echo "$cache/$name is missing (release/build-content fetches it)" >&2; exit 1; }
    layers=("$cache/$name" "${layers[@]}") bases+=(--base-patch "$cache/$name")
done <"$root/release/content.tsv"
while read -r path; do
    mkdir -p "$work/live/$(dirname "$path")" "$work/donor/$(dirname "$path")"
    for layer in "${layers[@]}"; do
        "$SKHID_VOSTOK_PORT" extract --db "$layer" --path "$path" --output "$work/live/$path" >/dev/null 2>&1 && break
    done
    [[ -s $work/live/$path ]] || { echo "$path: in no archive of the base" >&2; exit 1; }
    "$SKHID_VOSTOK_PORT" extract --db "$donor" --path "$path" --output "$work/donor/$path" >/dev/null
done < <(python3 -I "$here/gen.py" --paths)
python3 -I "$here/gen.py" --live "$work/live" --donor "$work/donor" --out "$pack"
"$SKHID_VOSTOK_PORT" pack check --pack "$pack" --base-db "$SKHID_STOCK_DB" "${bases[@]}"
