#!/usr/bin/env bash
# shot.sh <replay_player.swf> <out.png> [<width>x<height>]: a screenshot of the replay player's
# controls (MarkersHarness.as, shot=1: markers, 10 players of 16-character names, 2x), in Ruffle (CPU
# only: lavapipe on its own virtual X display), default 1920x1080, at most 1280 px wide.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
player=$(readlink -f "$1")
out=$(readlink -m "$2")
size=${3:-1920x1080}
w=${size%x*}
h=${size#*x}
. "$here/../ruffle_harness.sh"
harness_work ""
harness_compile "$here/MarkersHarness.as" "$w,$h" -default-background-color=0x30383E
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$player" "$work/replay_player.swf"
harness_display "${w}x$((h + 200))" xorg.xwd imagemagick
HARNESS_TIMEOUT=60 harness_run shot.log -p low --width "$w" --height "$h" -Pshot=1 MarkersHarness.swf
wait_for shot.log 'SHOT ready' 600 || { cat "$work/shot.log" >&2; exit 1; }
sleep 1
DISPLAY=$display xwd -root -silent >"$work/shot.xwd"
magick "$work/shot.xwd" -crop "${w}x${h}+0+22" +repage -resize '1280x>' "$out"
harness_stop
echo "$out"
