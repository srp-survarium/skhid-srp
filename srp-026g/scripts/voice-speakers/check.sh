#!/usr/bin/env bash
# check.sh <hud.swf> [work dir]: the hud's talking list (VoiceSpeakers) in Ruffle on a
# private CPU-only virtual display (lavapipe, never the GPU), by VoiceSpeakersHarness.as: a
# teammate's speaker blue, an opponent's pale red, a side change redrawn, a fade, names that
# are methods of Object. Screenshots into the work dir: talking.png (the whole 1280x720),
# talking-list.png (the list, 3x). Exits 0 when it says pass; prints its lines. A work dir given
# is kept.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
hud=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
harness_compile "$here/VoiceSpeakersHarness.as" 1280,720 -default-background-color=0x5A6066
cp "$hud" "$work/hud.swf"
harness_display 1280x720 ffmpeg-full imagemagick
harness_run VoiceSpeakersHarness.log -p high --no-gui --width 1280 --height 720 VoiceSpeakersHarness.swf
if wait_for VoiceSpeakersHarness.log 'VOICE SHOT\|VOICE RESULT' 300; then
    ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size 1280x720 -i "$display" -frames:v 1 "$work/talking.png"
    magick "$work/talking.png" -crop 220x70+1040+400 +repage -filter point -resize 300% "$work/talking-list.png"
fi
wait_for VoiceSpeakersHarness.log 'VOICE RESULT' 300 || true
harness_stop
harness_report 'VOICE .*' VoiceSpeakersHarness.log
grep -q 'VOICE RESULT pass' "$work/VoiceSpeakersHarness.log"
