#!/usr/bin/env bash
# check.sh <player_icons.swf> <stock|kinds> [work dir]: the markers over thrown grenades
# (GrenadeIconsHarness.as, docs/notes/grenade-icons.md) in Ruffle on a private CPU-only virtual
# display (lavapipe, never the GPU). stock: the unpatched game's feed (no kind told); kinds: each
# grenade's kind told (MarkerType), and a marker's type changed back and forth. Screenshot:
# <work>/grenades-<feed>.png (640 x 320, 1:1 with the game's pixels). A work dir given is kept.
set -euo pipefail
[[ $# -ge 2 && $# -le 3 ]] || { echo 'usage: check.sh <player_icons.swf> <stock|kinds> [work dir]' >&2; exit 2; }
here=$(dirname "$(readlink -f "$0")")
movie=$(readlink -f "$1")
feed=$2
[[ $feed == stock || $feed == kinds ]] || { echo "check.sh: the feed is stock or kinds, not $feed" >&2; exit 2; }
. "$here/../ruffle_harness.sh"
harness_work "${3:-}"
kinds=false
[[ $feed == kinds ]] && kinds=true
harness_compile "$here/GrenadeIconsHarness.as" 640,320 -define=CONFIG::kinds,$kinds -default-background-color=0x30373B
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$movie" "$work/player_icons.swf"
harness_display 640x320 ffmpeg-full
harness_run grenades.log -p high --no-gui --width 640 --height 320 GrenadeIconsHarness.swf
# Ruffle presents a frame a little after the movie's trace
wait_for grenades.log 'GRENADES SHOT\|GRENADES RESULT' && sleep 1 && \
    ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size 640x320 -i "$display" -frames:v 1 "$work/grenades-$feed.png"
wait_for grenades.log 'GRENADES RESULT' || true
harness_stop
harness_report 'GRENADES .*' grenades.log
grep -q 'GRENADES RESULT pass' "$work/grenades.log"
