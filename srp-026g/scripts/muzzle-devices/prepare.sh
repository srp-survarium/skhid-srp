#!/usr/bin/env bash
# The files make_pack.py reads, into a new folder:
#   prepare.sh <vostok-port> <0.69d0 vostok-port> <0.69d0 .split> <0.46c2 .split> <srp-game-data> <out folder> \
#       --db <resources.db> --patch <patch> ...
# <vostok-port>: resource-porter's (it reads 0.46c2's split volumes); <0.69d0 vostok-port>: one that
# reads 0.69d0's compressed volumes (the one ../weapon-modules/make_pack.sh takes); the .split
# files: the weapon survey's lists of each build's volumes; <srp-game-data>: srp-026g's tool
# (target/release/srp-game-data) and, after the folder, the players' archives as `srp-game-data
# files` takes them (the stock db and every patch of release/patches.tsv).
#   d069/  0.69d0's gameplay/game.options (the devices' groups, modifications and shot effects)
#   d046/  0.46c2's weapon shot effects: particles/fx/weapons/attach/, their materials and
#          textures (material_instances/fx/weapons/, textures/fx/weapons/), as they are
#   live/  the players' dictionary, modifications, every particle and every effect material (what a
#          ported particle and material is checked against), and paths.txt (every path they have)
# make_pack.py checks d069 and d046 against inputs.sha256.
set -euo pipefail
(($# >= 8)) || { sed -n '2,/^set/p' "$0" | sed '$d'; exit 2; }
port=$1 port069=$2 split069=$3 split046=$4 tool=$5 out=$6
shift 6
[[ ! -e $out ]] || { echo "$out: already there" >&2; exit 1; }
mkdir -p "$out/d069/gameplay"
"$port069" extract --db "$split069" --path gameplay/game.options --output "$out/d069/gameplay/game.options" >/dev/null
"$port" extract-bundle --db "$split046" --prefix particles/fx/weapons/attach/ --prefix material_instances/fx/weapons/ \
    --prefix textures/fx/weapons/ --output "$out/d046.bundle" >/dev/null
mv "$out/d046.bundle/overlay" "$out/d046"
rm -r "$out/d046.bundle"
list=$(mktemp)
trap 'rm -f "$list" "$list.all"' EXIT
"$tool" files "$@" > "$list.all"
cut -f1 "$list.all" | grep -E '^(gameplay/db_static_dictionaries$|gameplay/items/modifications/modifications\.options$|particles/|material_instances/fx/)' > "$list"
"$tool" get "$@" --out "$out/live" --paths "$list" >/dev/null
cut -f1 "$list.all" > "$out/live/paths.txt"
echo "$out: $(find "$out" -type f | wc -l) files"
