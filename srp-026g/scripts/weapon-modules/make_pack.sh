#!/usr/bin/env bash
# usage: make_pack.sh <resource-porter checkout> <survey vostok-port> <0.69d0 .split> <work folder> <version>
#            [<0.69d0 item>...]
#
# The content pack weapon-modules@<version> into $SKHID_PACKS_DIR (docs/notes/attachments.md,
# docs/notes/remodel-069.md): every gun of ../remodel-069/remodel-069d.json on 0.69d0's model in two
# bodies (whole and bare), 0.69d0's modules as static models, the modules table
# (gameplay/weapon_modules.srpm) and the bodies' camouflage masks. Prints the pack.json's sha256 for
# release/content.tsv.
#   1. 0.69d0's weapon models, materials and textures (all of them: which modules a gun takes is what
#      the build works out) as one archive with gameplay/game.options; kept in the work folder
#      (donor.db) and made once;
#   2. resource-porter's port-modules (its docs/modules-069d-026.md) over what players have (the
#      stock resources.db and every patch of release/patches.tsv), with the released scope pairs
#      (../scope-fits/released-pairs.tsv: one 0.69d0 does not give is a legacy row); its check of
#      every gun and fit against 0.69d0 is in <work folder>/bundle/modules-069d.json, and a failure
#      is printed;
#   3. the camouflage masks (_aoac, client patch 44) of the textures the whole bodies draw, into the
#      bundle: ../weapons-069/camo_rows.py's rows less the textures players have a mask for (an
#      earlier port's stands), by ../weapon-camo/weapon_camo.py --bare-masks with
#      ../remodel-069/parts.tsv (the cartridges and the MP7's glowing dots stay plain; every other
#      texture takes the maker's suggestion, its large parts). ../remodel-069/camo.tsv is the full
#      build's rows: when it is not, the pack is made of the bundle's rows all the same, the new
#      table is left in <work folder>/masks/camo.tsv to commit, and the build fails at its end;
#   4. vostok-port pack from-bundle against the same files; the build's record
#      (bundle/modules-069d.json) is kept in the pack's folder.
# With 0.69d0 items named: those guns alone (a quick pack for a local test; its id says so; a mask
# of it paints the parts its own guns draw of a texture, not those of a gun left out).
# $SKHID_RENDERS: a folder for the pictures of the check (review.py makes the pages of them).
# <survey vostok-port>: one that reads 0.69d0's compressed volumes and .split lists.
# Needs cargo (nix-shell -p cargo rustc gcc zlib), magick (the porter's texture encoder) and Python
# with numpy and Pillow (weapon_camo.py).
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
    --items "$here/../remodel-069/items.tsv" --ids "$here/ids.tsv" "${only[@]}" \
    --released "$here/../scope-fits/released-pairs.tsv" --donor "$work/donor.db" \
    --target-db "$SKHID_STOCK_DB" "${layers[@]}" ${SKHID_RENDERS:+--renders "$SKHID_RENDERS"} --output "$work/bundle" \
    | tee "$work/port-modules.log"
# The camouflage masks: weapon_camo.py reads the bundle's models and textures and the stock _aoac
# options it copies.
masks=$work/masks
rm -rf "$masks"
mkdir -p "$masks/game/textures/character/human/black_market_01"
for db in "$SKHID_STOCK_DB" "${layers[@]}"; do
    [[ $db == --target-patch ]] || "$port" list --db "$db" --prefix textures/
done | grep '_aoac\.dds$' | sort -u >"$masks/players.aoac"
{
    sed -n '/^#/p' "$here/../remodel-069/camo.tsv"
    python3 -I "$here/../weapons-069/camo_rows.py" "$work/bundle" | tail -n +2 \
        | awk -F'\t' 'NR == FNR { have[$1] = 1; next } !(("textures/" $1 "_aoac.dds") in have)' "$masks/players.aoac" -
} >"$masks/camo.tsv"
cp "$here/../remodel-069/parts.tsv" "$masks/parts.tsv"
cp -as "$(realpath "$work/bundle/overlay")/." "$masks/game/"
options=textures/character/human/black_market_01/black_market_01_torso_aoac.options
"$port" extract --db "$SKHID_STOCK_DB" --path "$options" --output "$masks/game/$options" >/dev/null
python3 -I "$here/../weapon-camo/weapon_camo.py" "$masks/camo.tsv" "$masks/game" "$masks/game" "$masks/out" \
    --bare-masks >"$masks/camo.log"
rm -r "$masks/game"
python3 -I "$here/masks.py" "$masks/out" "$work/bundle"
"$port" pack from-bundle --base-db "$SKHID_STOCK_DB" "${bases[@]}" --bundle "$work/bundle" \
    --output "$pack" --id "$id" --version "$version" >/dev/null
cp "$work/bundle/modules-069d.json" "$pack/modules-069d.json"
echo "$id@$version: $(sha256sum "$pack/pack.json" | cut -d' ' -f1)"
if ((${#only[@]} == 0)) && ! diff "$masks/camo.tsv" "$here/../remodel-069/camo.tsv" >"$masks/camo.diff"; then
    echo "../remodel-069/camo.tsv is not the bundle's ($masks/camo.diff): commit $masks/camo.tsv as it" >&2
    exit 1
fi
