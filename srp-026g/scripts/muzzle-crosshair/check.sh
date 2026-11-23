#!/usr/bin/env bash
# check.sh <hud.swf> [work dir]: exe patch 200's crosshair (docs/notes/muzzle-shots.md) in the hud
# movie, in Ruffle on a private CPU-only virtual display (lavapipe, never the GPU), at 1920x1080 and
# 1280x720: CrosshairHarness.as feeds the movie's own root a match's start and
# root.srp_crosshair_at as the game calls it. Exits 0 when every run says pass; prints their lines.
# A work dir given is kept. Java and Flex from private/actionscript/tools.json
# (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
hud=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"

"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$hud" "$work/hud.swf"
logs=()
for size in 1920x1080 1280x720; do
    harness_compile "$here/CrosshairHarness.as" "${size/x/,}" -default-background-color=0x4A5257
    mv "$work/CrosshairHarness.swf" "$work/CrosshairHarness-$size.swf"
    harness_display "$size" ffmpeg-full
    log=CrosshairHarness-$size.log
    harness_run "$log" -p high --no-gui --width "${size%x*}" --height "${size#*x}" "CrosshairHarness-$size.swf"
    wait_for "$log" 'CROSSHAIR RESULT' || true
    harness_stop
    logs+=("$log")
done

harness_report 'CROSSHAIR .*' "${logs[@]}"
! grep -q "CROSSHAIR RESULT fail" "${logs[@]/#/$work/}" && for log in "${logs[@]}"; do
    grep -q "CROSSHAIR RESULT pass" "$work/$log" || exit 1
done
