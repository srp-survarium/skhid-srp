#!/usr/bin/env bash
# prepare_live.sh <srp-game-data> <new output> --db <stock> --patch <archive> ...
# Use the same ordered archive stack as the fixed rifle-scopes@18/scope-fits@12
# candidate. Extra clips are needed to check camera binding during reloads.
set -euo pipefail
tool=$1 out=$2
shift 2
here=$(cd "$(dirname "$0")" && pwd)
bash "$here/../scope-fits/prepare_live.sh" "$tool" "$out" "$@"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
"$tool" get "$@" --out "$work/extra" --paths "$here/live-extra-paths.txt"
while IFS= read -r path; do
    [[ ! -e $out/$path ]] || { echo "already extracted: $path" >&2; exit 1; }
done < "$here/live-extra-paths.txt"
cp -a "$work/extra/." "$out/"
