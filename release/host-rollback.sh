#!/bin/sh
# The rollback of one release, run ON the target host. release/release.sh and
# release/deploy put a copy of it in every release folder as rollback.sh,
# beside rollback.env (what was there before the swap: no secret), and run it
# themselves when the swap fails; release/rollback runs it from the release
# PC. By hand, on the host (as root: it writes srp.db in the volume's folder):
#
#   sh <release folder>/rollback.sh <release folder> [--keep-db]
#
# Puts back what was there just before that release's swap, then makes srp26
# anew and waits for it: download/Survarium-0.26g-srp-files.zip and
# download/server-game-data.db (old-*, hard links in the folder, renamed into
# place), the bots' data files the release put in the volume's folder (the
# old ones back from old-bot-data/, the new ones it created removed:
# bot-data.log says which), SRP_MIN_PATCH_REVISION (that line only), the server source
# (old-src.tgz, when the release built one), the image that ran before
# (image.old) as srp-026g:latest, and srp.db, unless --keep-db, as it was
# before the release: the copy the new srp made with VACUUM INTO at its start
# (in the folder, named by srp-db.name; or, when the run did not get to keep
# it, the one in the volume's backups/ newer than BACKUP_BEFORE). When the
# new srp made none, it never opened srp.db: it is kept as it is.
#   (default)   every change to srp.db since the release (sign-ups, match
#               results, purchases) is LOST
#   --keep-db   srp.db as it is now: only when the release changed no
#               database schema and no player got an item of its new patch
# When srp refuses the files from before (players were given items they
# lack), it stops srp26 (it would only refuse again) and says how to put the
# release back from the folder: roll forward then, with a fixed patch.
set -eu
dir=${1:-}
keep_db=0
case ${2:-} in
--keep-db) keep_db=1 ;;
'') ;;
*) dir='' ;;
esac
if test -z "$dir" || test ! -f "$dir/rollback.env"; then
    echo "usage: sh <release folder>/rollback.sh <release folder> [--keep-db] (a folder with rollback.env)" >&2
    exit 2
fi
dir=$(cd "$dir" && pwd)
# TARGET_DIR COMPOSE IMAGE OLD_IMAGE VOLUME_DIR BACKUP_BEFORE OLD_MIN NEW_MIN
# HAVE_ZIP HAVE_DATA SRC BOT_DATA ZIP_NAME DATA_NAME
# shellcheck disable=SC1091 # written by the release
. "$dir/rollback.env"
cd "$TARGET_DIR"
# What a hard link or rename puts into download/ atomically.
put() { # <file in the folder> <name in download/>
    if test "$1" -ef "download/$2"; then return 0; fi
    ln -f "$1" "download/.$2.new" && mv -f "download/.$2.new" "download/$2"
}
number() { n=${1##*/}; n=${n#srp-}; n=${n%.db}; echo "${n:-0}"; }

# The srp.db to put back: the folder's copy, else srp's own newer than
# BACKUP_BEFORE, else none (the new srp never opened srp.db).
db=''
if test -f "$dir/srp-db.name"; then
    db=$dir/$(cat "$dir/srp-db.name")
else
    before=$(number "${BACKUP_BEFORE:-}")
    for f in "$VOLUME_DIR"/backups/srp-*.db; do
        if test -f "$f" && test "$(number "$f")" -gt "$before"; then
            if test -z "$db" || test "$(number "$f")" -lt "$(number "$db")"; then db=$f; fi
        fi
    done
fi

need=''
if test -n "$db" && test "$keep_db" = 0; then need=$db; fi
if test "$HAVE_ZIP" = 1; then need="$need $dir/old-$ZIP_NAME"; fi
if test "$HAVE_DATA" = 1; then need="$need $dir/old-$DATA_NAME"; fi
if test "$SRC" = 1; then need="$need $dir/old-src.tgz"; fi
if test "${BOT_DATA:-0}" = 1 && test -f "$dir/bot-data.log"; then
    while read -r what path; do
        if test "$what" = replaced; then need="$need $dir/old-bot-data/$path"; fi
    done <"$dir/bot-data.log"
fi
for f in $need; do
    test -s "$f" || { echo "$f is missing: nothing was rolled back" >&2; exit 1; }
done
docker image inspect "$OLD_IMAGE" >/dev/null ||
    { echo "the image $OLD_IMAGE is gone: nothing was rolled back" >&2; exit 1; }
test -f "$VOLUME_DIR/srp.db" || { echo "$VOLUME_DIR/srp.db is not there: nothing was rolled back" >&2; exit 1; }

$COMPOSE stop srp26
if test "$HAVE_DATA" = 1; then put "$dir/old-$DATA_NAME" "$DATA_NAME"; else rm -f "download/$DATA_NAME"; fi
if test "$HAVE_ZIP" = 1; then put "$dir/old-$ZIP_NAME" "$ZIP_NAME"; else rm -f "download/$ZIP_NAME"; fi
rm -f "download/.$DATA_NAME.new" "download/.$ZIP_NAME.new"
if test "${BOT_DATA:-0}" = 1 && test -f "$dir/bot-data.log"; then
    # srp26 is stopped. A file that was there is copied back over the new
    # one (its owner and mode kept: cp -p as root), one the release made is
    # removed (its folders stay).
    while read -r what path; do
        case $what in
        replaced)
            cp -p "$dir/old-bot-data/$path" "$VOLUME_DIR/$path.rollback" || { echo "bot data: $path could not be copied back: the rollback stops here (the old file is in $dir/old-bot-data)" >&2; exit 1; }
            mv -f "$VOLUME_DIR/$path.rollback" "$VOLUME_DIR/$path"
            rm -f "$VOLUME_DIR/$path.new"
            ;;
        created) rm -f "$VOLUME_DIR/$path" "$VOLUME_DIR/$path.new" ;;
        esac
    done <"$dir/bot-data.log"
    echo "bot data: the files of the release put back or removed (bot-data.log)"
fi
if test "$OLD_MIN" = none; then
    sed -i '/^SRP_MIN_PATCH_REVISION=/d' .env
else
    sed -i "s/^SRP_MIN_PATCH_REVISION=.*/SRP_MIN_PATCH_REVISION=$OLD_MIN/" .env
fi
if test "$SRC" = 1; then tar -xzf "$dir/old-src.tgz"; fi
docker tag "$OLD_IMAGE" "$IMAGE"
if test "$keep_db" = 1; then
    echo "srp.db: kept as it is now (--keep-db)"
elif test -z "$db"; then
    echo "srp.db: kept as it is (the new srp made no backup at its start: it never opened it)"
else
    # srp26 is stopped: no writer. The copy takes the owner and mode of the
    # file it replaces; the write-ahead log of the replaced one goes.
    cp "$db" "$VOLUME_DIR/srp.db.rollback"
    chown --reference="$VOLUME_DIR/srp.db" "$VOLUME_DIR/srp.db.rollback"
    chmod --reference="$VOLUME_DIR/srp.db" "$VOLUME_DIR/srp.db.rollback"
    rm -f "$VOLUME_DIR/srp.db-wal" "$VOLUME_DIR/srp.db-shm"
    mv -f "$VOLUME_DIR/srp.db.rollback" "$VOLUME_DIR/srp.db"
    echo "srp.db: as it was before the release ($(basename "$db"))"
fi
$COMPOSE up -d --no-deps --force-recreate srp26

# Up: running (healthy, with a health check), not restarted. Refused: exited
# or restarting. About 90 s at most.
state=none i=0
while test $i -lt 45; do
    id=$($COMPOSE ps -aq srp26 | head -n 1)
    state=$(docker inspect -f '{{.State.Status}} {{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}} {{.RestartCount}}' "$id" 2>/dev/null || echo none)
    case $state in
    "running healthy 0" | "running none 0" | exited* | dead* | restarting* | *" "[1-9]*) break ;;
    esac
    i=$((i + 1))
    sleep 2
done
case $state in
"running healthy 0" | "running none 0")
    echo rolled-back >"$dir/state"
    echo "rolled back $(basename "$dir"): srp26 is up on the files from before it"
    exit 0
    ;;
esac
echo "srp26 did not come up on the files from before $(basename "$dir"): $state; its log:" >&2
$COMPOSE logs --no-log-prefix --tail=30 srp26 2>&1 | sed 's/^/  | /' >&2
$COMPOSE stop srp26 >/dev/null 2>&1 || true
echo rollback-refused >"$dir/state"
if $COMPOSE logs --no-log-prefix --tail=300 srp26 2>&1 | grep -q 'lacks'; then
    echo "srp refuses the files from before: srp.db names what they lack (players were given items of this release)." >&2
    echo "Do NOT roll back: roll forward with a fixed patch (the next revision)." >&2
else
    echo "srp refuses the files from before: see its log above." >&2
fi
echo "srp26 is stopped (it would only refuse again). To put the release back as it was:" >&2
echo "  cd $TARGET_DIR && ln -f $dir/$DATA_NAME download/.$DATA_NAME.new && mv -f download/.$DATA_NAME.new download/$DATA_NAME && ln -f $dir/$ZIP_NAME download/.$ZIP_NAME.new && mv -f download/.$ZIP_NAME.new download/$ZIP_NAME && sed -i 's/^SRP_MIN_PATCH_REVISION=.*/SRP_MIN_PATCH_REVISION=$NEW_MIN/' .env && docker tag \$(cat $dir/image.new) $IMAGE && $COMPOSE up -d --no-deps --force-recreate srp26" >&2
exit 1
