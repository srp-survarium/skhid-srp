#!/usr/bin/env bash
# The players' files make_pack.py builds on (../rifle-scopes/prepare_live.sh's, and every weapon
# attachment's model and material: the HK416's stock Walther is the Walther Competition II's reference):
#   prepare_live.sh <srp-game-data> <out folder> --db <resources.db> --patch <patch> ...
set -euo pipefail
tool=$1 out=$2
shift 2
[[ ! -e $out ]] || { echo "$out: already there" >&2; exit 1; }
list=$(mktemp)
trap 'rm -f "$list" "$list.all"' EXIT
"$tool" files "$@" > "$list.all"
cut -f1 "$list.all" | grep -E '^(gameplay/items/scopes/|gameplay/items/weapons/wpn_[^/]*\.options$|gameplay/db_static_dictionaries$|models/weapons/attach/|models/weapons/.*\.skinned_model/|material_instances/weapons/|textures/weapons/attach/)' > "$list"
"$tool" get "$@" --out "$out" --paths "$list" >/dev/null
cut -f1 "$list.all" > "$out/paths.txt"
echo "$out: $(wc -l < "$list") files"
