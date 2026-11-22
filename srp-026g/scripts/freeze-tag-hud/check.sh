#!/usr/bin/env bash
# check.sh <player_icons.swf> <hud.swf> [work dir]: Freeze Tag's frozen players (docs/notes/freeze-tag.md,
# "The frozen name") in Ruffle on a private CPU-only virtual display (lavapipe, never the GPU), each
# harness at 1920x1080 and 1280x720, each screenshot into the work dir:
# - FreezeIconsHarness.as: the player icons movie's names and arrows over heads, freeze-names-<w>x<h>.png.
#   With FREEZE_BACKGROUND (an image of the game's screen) the tags are drawn over it, else over a sky
#   and ground drawn by the harness;
# - FreezeHudHarness.as: the hud movie's minimap, Tab map and Tab list, freeze-maps-<w>x<h>.png.
# The two movies' ice blue must be the same. Exits 0 when every run says pass; prints their lines. A
# work dir given is kept. Java and Flex from private/actionscript/tools.json
# (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
icons=$(readlink -f "$1")
hud=$(readlink -f "$2")
. "$here/../ruffle_harness.sh"
harness_work "${3:-}"

"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$icons" "$work/player_icons.swf"
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$hud" "$work/hud.swf"
if [[ -n ${FREEZE_BACKGROUND:-} ]]; then cp "$FREEZE_BACKGROUND" "$work/background.png"; fi
logs=()
# each harness, its shot and its screenshots' name
for run in "FreezeIconsHarness tags freeze-names" "FreezeHudHarness maps freeze-maps"; do
    read -r harness shot png <<<"$run"
    for size in 1920x1080 1280x720; do
        harness_compile "$here/$harness.as" "${size/x/,}" -default-background-color=0x4A5257
        mv "$work/$harness.swf" "$work/$harness-$size.swf"
        harness_display "$size" ffmpeg-full
        log=$harness-$size.log
        harness_run "$log" -p high --no-gui --width "${size%x*}" --height "${size#*x}" "$harness-$size.swf"
        wait_for "$log" "FREEZE SHOT $shot\|FREEZE RESULT" && sleep 1 &&
            ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size "$size" -i "$display" -frames:v 1 \
                "$work/$png-$size.png"
        wait_for "$log" 'FREEZE RESULT' || true
        harness_stop
        logs+=("$log")
    done
done

harness_report 'FREEZE .*' "${logs[@]}"
colours=$(cd "$work" && grep -ho 'FREEZE \(hud \)\?colour 0x[0-9A-F]*' "${logs[@]}" | sed 's/.* //' | sort -u)
[[ $(wc -l <<<"$colours") == 1 ]] || { echo "FREEZE FAIL the movies' ice blue differ: $colours"; exit 1; }
! grep -q "FREEZE RESULT fail" "${logs[@]/#/$work/}" && for log in "${logs[@]}"; do
    grep -q "FREEZE RESULT pass" "$work/$log" || exit 1
done
