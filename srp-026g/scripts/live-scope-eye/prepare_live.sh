#!/usr/bin/env bash
# The players' files make_pack.py reads, from the game's archives as the players mount them:
#   prepare_live.sh <srp-game-data> <out folder> --db <resources.db> --patch <patch> ...
set -euo pipefail
tool=$1 out=$2
shift 2
[[ ! -e $out ]] || { echo "$out: already there" >&2; exit 1; }
list=$(mktemp)
trap 'rm -f "$list"' EXIT
"$tool" files "$@" | cut -f1 | grep -E '^(gameplay/items/scopes/|gameplay/items/weapons/wpn_[^/]*\.options$|gameplay/db_static_dictionaries$|models/weapons/)' > "$list"
"$tool" get "$@" --out "$out" --paths "$list" >/dev/null
echo "$out: $(wc -l < "$list") files"
