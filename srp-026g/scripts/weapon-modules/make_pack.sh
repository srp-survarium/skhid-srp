#!/usr/bin/env bash
# usage: make_pack.sh <resource-porter checkout> <survey vostok-port> <0.69d0 .split> <work folder> <version>
#            [<0.69d0 item>...]
#
# The content pack weapon-modules@<version> into $SKHID_PACKS_DIR (docs/notes/attachments.md,
# docs/notes/remodel-069.md): every gun of ../remodel-069/remodel-069d.json on 0.69d0's model in two
# bodies (whole and bare), 0.69d0's modules as static models, and the modules table
# (gameplay/weapon_modules.srpm). Prints the pack.json's sha256 for release/content.tsv.
#   1. 0.69d0's weapon models, materials and textures (all of them: which modules a gun takes is what
#      the build works out) as one archive with gameplay/game.options; kept in the work folder
#      (donor.db) and made once;
#   2. resource-porter's port-modules (its docs/modules-069d-026.md) over what players have (the
#      stock resources.db and every patch of release/patches.tsv); its check of every gun and fit
#      against 0.69d0 is in <work folder>/bundle/modules-069d.json, and a failure is printed;
#   3. vostok-port pack from-bundle against the same files.
# With 0.69d0 items named: those guns alone (a quick pack for a local test; its id says so).
# $SKHID_RENDERS: a folder for the pictures of the check (review.py makes the pages of them).
# <survey vostok-port>: one that reads 0.69d0's compressed volumes and .split lists.
# Needs cargo (nix-shell -p cargo rustc gcc zlib) and magick (the porter's texture encoder).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../.." && pwd)
(($# >= 5)) || { sed -n '2,/^set/p' "$0" | sed '$d'; exit 2; }
porter=$(cd "$1" && pwd) survey=$2 split=$3 work=$4 version=$5
shift 5
: "${SKHID_STOCK_DB:?the stock resources.db}" "${SKHID_PACKS_DIR:?the packs folder}"
id=weapon-modules only=()
for item in "$@"; do only+=(--only "$item"); id=weapon-modules-quick; done
pack=$SKHID_PACKS_DIR/$id-$version
for d in "$work/bundle" "$pack"; do
    [[ ! -e $d ]] || { echo "$d is already there" >&2; exit 1; }
done
mkdir -p "$work"
cache=$HOME/.cache/skhid-release/patches
layers=() bases=()
while read -r n; do
    [[ -f $cache/resources.db.patch_$n ]] || { echo "$cache/resources.db.patch_$n is missing (release/build-content fetches it)" >&2; exit 1; }
    layers+=(--target-patch "$cache/resources.db.patch_$n") bases+=(--base-patch "$cache/resources.db.patch_$n")
done < <(grep -v '^#' "$root/release/patches.tsv" | cut -f1 | grep -o '[0-9]*$')
(cd "$porter" && cargo build -q --release --bin vostok-port --example pack-donor-tree)
port=$porter/target/release/vostok-port
if [[ ! -f $work/donor.db ]]; then
    "$survey" extract-bundle --db "$split" --prefix models/weapons/ --prefix material_instances/weapons/ \
        --prefix textures/weapons/ --prefix textures/editor/ --output "$work/tree" >/dev/null
    "$survey" extract --db "$split" --path gameplay/game.options --output "$work/tree/overlay/gameplay/game.options" >/dev/null
    "$porter/target/release/examples/pack-donor-tree" "$work/tree/overlay" "$work/donor.db" >/dev/null
    rm -r "$work/tree"
fi
"$port" port-modules --id "$id" --donor-spec "$here/../remodel-069/remodel-069d.json" \
    --items "$here/../remodel-069/items.tsv" --ids "$here/ids.tsv" "${only[@]}" --donor "$work/donor.db" \
    --target-db "$SKHID_STOCK_DB" "${layers[@]}" ${SKHID_RENDERS:+--renders "$SKHID_RENDERS"} --output "$work/bundle" \
    | tee "$work/port-modules.log"
"$port" pack from-bundle --base-db "$SKHID_STOCK_DB" "${bases[@]}" --bundle "$work/bundle" \
    --output "$pack" --id "$id" --version "$version" >/dev/null
echo "$id@$version: $(sha256sum "$pack/pack.json" | cut -d' ' -f1)"
