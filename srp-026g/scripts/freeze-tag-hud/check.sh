#!/usr/bin/env bash
# check.sh <player_icons.swf> [work dir]: Freeze Tag's frozen names (docs/notes/freeze-tag.md, the HUD)
# in Ruffle on a private CPU-only virtual display (lavapipe, never the GPU): FreezeIconsHarness.as at
# 1920x1080 and 1280x720, each screenshot into the work dir as freeze-names-<w>x<h>.png. With
# FREEZE_BACKGROUND (an image of the game's screen) the tags are drawn over it, else over a sky and
# ground drawn by the harness. Exits 0 when both runs say pass; prints their lines. A work dir given
# is kept. Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or
# SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
icons=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"

"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$icons" "$work/player_icons.swf"
if [[ -n ${FREEZE_BACKGROUND:-} ]]; then cp "$FREEZE_BACKGROUND" "$work/background.png"; fi
logs=()
for size in 1920x1080 1280x720; do
    harness_compile "$here/FreezeIconsHarness.as" "${size/x/,}" -default-background-color=0x4A5257
    mv "$work/FreezeIconsHarness.swf" "$work/FreezeIconsHarness-$size.swf"
    harness_display "$size" ffmpeg-full
    harness_run "freeze-$size.log" -p high --no-gui --width "${size%x*}" --height "${size#*x}" \
        "FreezeIconsHarness-$size.swf"
    wait_for "freeze-$size.log" 'FREEZE SHOT tags\|FREEZE RESULT' && sleep 1 &&
        ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size "$size" -i "$display" -frames:v 1 \
            "$work/freeze-names-$size.png"
    wait_for "freeze-$size.log" 'FREEZE RESULT' || true
    harness_stop
    logs+=("freeze-$size.log")
done

harness_report 'FREEZE .*' "${logs[@]}"
! grep -q "FREEZE RESULT fail" "${logs[@]/#/$work/}" && for log in "${logs[@]}"; do
    grep -q "FREEZE RESULT pass" "$work/$log" || exit 1
done
