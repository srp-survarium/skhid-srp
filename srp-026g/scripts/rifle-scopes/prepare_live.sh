#!/usr/bin/env bash
# The players' files make_pack.py builds on, from the game's archives as the players mount them:
#   prepare_live.sh <srp-game-data> <out folder> --db <resources.db> --patch <patch> ...
# <srp-game-data>: srp-026g's tool (target/release/srp-game-data); the layers as `srp-game-data
# files` takes them. Gets: the scope items and the weapon configs (gameplay/items/scopes/,
# gameplay/items/weapons/wpn_*), the dictionary, every weapon model (models/weapons/**/*.skinned_model,
# the scopes' weapon variants among them) and every scope model of ours (models/weapons/attach/fit*,
# patch 19's), the loc2 materials and the attachments' textures (for the icons), and writes paths.txt (every path the archives have).
set -euo pipefail
tool=$1 out=$2
shift 2
[[ ! -e $out ]] || { echo "$out: already there" >&2; exit 1; }
list=$(mktemp)
trap 'rm -f "$list"' EXIT
"$tool" files "$@" > "$list.all"
cut -f1 "$list.all" | grep -E '^(gameplay/items/scopes/|gameplay/items/weapons/wpn_[^/]*\.options$|gameplay/db_static_dictionaries$|models/weapons/attach/fit[0-9]*/|models/weapons/.*\.skinned_model/|material_instances/weapons/loc2/|textures/weapons/attach/)' > "$list"
"$tool" get "$@" --out "$out" --paths "$list" >/dev/null
cut -f1 "$list.all" > "$out/paths.txt"
rm -f "$list.all"
echo "$out: $(wc -l < "$list") files"
