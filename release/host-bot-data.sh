#!/bin/sh
# Puts the bots' data files of one release in the data volume's folder, ON the
# target host, with srp26 stopped. release/release.sh stages the files in the
# release folder (bot-data/<path>, their paths in bot-data.list) and runs
# this copy of it, as bot-data.sh:
#
#   sh <release folder>/bot-data.sh <release folder> <the volume's folder>
#
# For each path of the list: the file in the volume's folder, when there is
# one, is copied into old-bot-data/<path> (cp -p) and noted as `replaced`
# in bot-data.log, else noted as `created` (the rollback puts back or
# removes by that log); the new file is copied beside it as <name>.new with
# the owner of srp.db and mode 644, and renamed over it (atomic: a reader
# has the old file or the new, never half of one). A folder it makes (mapdata/
# on a volume that never had one) takes srp.db's owner too: srp writes its
# built grids and trees there. Nothing is changed unless every staged file
# is there.
set -eu
dir=${1:-}
vol=${2:-}
if test -z "$dir" || test -z "$vol" || test ! -f "$dir/bot-data.list"; then
    echo "usage: sh <release folder>/bot-data.sh <release folder> <volume folder> (a folder with bot-data.list)" >&2
    exit 2
fi
ref=$vol/srp.db
test -f "$ref" || { echo "$ref is not there: nothing was put" >&2; exit 1; }
while read -r path; do
    test -f "$dir/bot-data/$path" || { echo "$dir/bot-data/$path is missing: nothing was put" >&2; exit 1; }
done <"$dir/bot-data.list"
: >"$dir/bot-data.log"
mkdir -p "$dir/old-bot-data"
while read -r path; do
    dst=$vol/$path
    # The folders above it.
    d=$vol
    old_ifs=$IFS
    IFS=/
    # shellcheck disable=SC2046 # the path's parts, split on /
    set -- $(dirname "$path")
    IFS=$old_ifs
    for part in "$@"; do
        if test "$part" = .; then continue; fi
        d=$d/$part
        if test ! -d "$d"; then
            mkdir "$d"
            chown --reference="$ref" "$d"
            chmod 755 "$d"
        fi
    done
    if test -e "$dst"; then
        mkdir -p "$(dirname "$dir/old-bot-data/$path")"
        cp -p "$dst" "$dir/old-bot-data/$path"
        echo "replaced $path" >>"$dir/bot-data.log"
    else
        echo "created $path" >>"$dir/bot-data.log"
    fi
    cp "$dir/bot-data/$path" "$dst.new"
    chown --reference="$ref" "$dst.new"
    chmod 644 "$dst.new"
    mv -f "$dst.new" "$dst"
    echo "bot data: $path"
done <"$dir/bot-data.list"
