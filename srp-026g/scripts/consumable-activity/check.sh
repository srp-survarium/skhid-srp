#!/usr/bin/env bash
# check.sh <hud.swf> [work dir]: native countdown presentation in isolated CPU Ruffle. Screenshot
# into the work dir: boost.png (the whole 1280x720: a drug's "x4", a tank's own countdown and one in
# Oxygen Rush, over a dark ground and a bright one). A work dir given is kept.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
hud=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
harness_compile "$here/ActivityHarness.as" 1280,720
cp "$hud" "$work/hud.swf"
harness_display 1280x720 ffmpeg-full
harness_run ActivityHarness.log -p low --width 1280 --height 720 -Pmovie=hud.swf ActivityHarness.swf
if wait_for ActivityHarness.log 'ACTIVITY SHOT\|ACTIVITY RESULT'; then
    ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size 1280x720 -i "$display" -frames:v 1 "$work/boost.png" || true
fi
wait_for ActivityHarness.log 'ACTIVITY RESULT' || true
harness_stop
harness_report 'ACTIVITY .*' ActivityHarness.log
grep -q 'ACTIVITY RESULT pass' "$work/ActivityHarness.log"
