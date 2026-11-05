#!/usr/bin/env bash
# replay.sh <game dir> <exe> <file.sur> <out dir> [seconds]
#
# Plays a match replay (.sur, from srp-recording-to-sur) in the local 0.26g
# game, headless, and keeps what a crash leaves: the game log, the crash
# report (.txt) and the dump's path, srp_patches.log's lines of this game,
# a screenshot a minute. Needs a display ($DISPLAY: an Xorg dummy at 60 Hz
# with openbox, docs in the skhid-local-game skill), and these settings:
#   WINE, WINESERVER  the Wine build (private/wine-keyboard/runtime/bin/...)
#   WINEPREFIX        an inactive prefix with DXVK, used for this game only
# It waits until the game exits or [seconds] (1200) pass, then stops the
# prefix's wineserver. Prints the result; exit 0 either way.
set -u
GAME=$1 EXE=$2 SUR=$3 OUT=$4 SECS=${5:-1200}
: "${WINE:?}" "${WINESERVER:?}" "${WINEPREFIX:?}" "${DISPLAY:?}"
unset WAYLAND_DISPLAY XDG_SESSION_TYPE
export WINEDEBUG=-all WINEDLLOVERRIDES="mscoree,mshtml,qwave=;winemenubuilder.exe=d;d3d11,dxgi=n,b"
mkdir -p "$OUT"
BIN=$GAME/binaries/x86
LOGD=$WINEPREFIX/drive_c/users/$USER/Documents/Survarium
rm -f "$LOGD"/Survarium*.log
before=$(ls "$BIN"/SRP-crash-* 2>/dev/null | sort)
patches_from=$(wc -l < "$BIN/srp_patches.log" 2>/dev/null || echo 0)
cd "$BIN"
start=$(date +%s)
"$WINE" "$EXE" "-match_replay=Z:${SUR//\//\\}" -mt_allocator=768 \
    -minimum_resources_memory_size=768 -splash_screen > "$OUT/wine.log" 2>&1 &
pat="[${EXE:0:1}]${EXE:1}"
up() { pgrep -f "$pat" > /dev/null; }
sleep 20
n=0
while up && (( $(date +%s) - start < SECS )); do
    sleep 10
    (( n++ % 6 == 0 )) && import -display "$DISPLAY" -window root "$OUT/t$(printf %04d $(( $(date +%s) - start ))).png" 2>/dev/null
done
alive=no; up && alive=yes
echo "elapsed $(( $(date +%s) - start )) s, game still running: $alive" > "$OUT/result.txt"
cp "$LOGD"/Survarium*.log "$OUT/" 2>/dev/null
tail -n +$((patches_from + 1)) "$BIN/srp_patches.log" > "$OUT/srp_patches.log" 2>/dev/null
for f in $(comm -13 <(echo "$before") <(ls "$BIN"/SRP-crash-* 2>/dev/null | sort)); do
    case $f in
        *.txt) cp "$f" "$OUT/"; echo "report: $f" >> "$OUT/result.txt" ;;
        *) echo "dump: $f" >> "$OUT/result.txt" ;;
    esac
done
sleep 3
"$WINESERVER" -k 2>/dev/null
grep -h -m3 "^Exception\|^Address" "$OUT"/SRP-crash-*.txt 2>/dev/null >> "$OUT/result.txt"
cat "$OUT/result.txt"
