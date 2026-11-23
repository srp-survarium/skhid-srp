#!/usr/bin/env bash
# check.sh <hud.swf> <player_icons.swf> [work dir]: Artifact Hunt's hud (docs/notes/artefact_hunt.md
# "HUD calls") in Ruffle on a private CPU-only virtual display (lavapipe, never the GPU):
# - HuntHudHarness.as: the hud movie's own root fed a match's start and made-up
#   root.artefact_hunt_update states (the bars, the artefact's row, the round's texts and fade, the
#   minimaps' bases, artefact and carrier, the Tab list's carrier row, a late join, refused calls,
#   another mode after it, every language's round texts);
# - HuntIconsHarness.as: the player icons movie's carriers (root.srp_show_carriers).
# Screenshots into the work dir: hunt-<state>.png, hunt-icons.png. Exits 0 when both say pass;
# prints their lines. A work dir given is kept. Java and Flex from private/actionscript/tools.json
# (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
hud=$(readlink -f "$1")
icons=$(readlink -f "$2")
. "$here/../ruffle_harness.sh"
harness_work "${3:-}"

"${PYTHON:-python3}" "$here/hunt_text.py" "$work"
for harness in HuntHudHarness HuntIconsHarness; do
    harness_compile "$here/$harness.as" 1280,720 -source-path+="$work" -default-background-color=0x4A5257
done
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$hud" "$work/hud.swf"
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$icons" "$work/player_icons.swf"
harness_display 1280x720 ffmpeg-full

run() { harness_run "$1.log" -p high --no-gui --width 1280 --height 720 "$1.swf"; }
shot() { ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size 1280x720 -i "$display" -frames:v 1 "$work/$1"; }

run HuntHudHarness
for state in waiting lying tab-ally-carrier enemy-carrier you-carry restart fade countdown-start countdown \
    sudden-coming sudden respawn other-languages; do
    wait_for HuntHudHarness.log "HUNT SHOT $state\$\|HUNT RESULT" && shot "hunt-$state.png"
done
wait_for HuntHudHarness.log 'HUNT RESULT' || true
kill "$ruffle_pid" 2>/dev/null || true
run HuntIconsHarness
wait_for HuntIconsHarness.log 'HUNT SHOT icons\|HUNT RESULT' && shot hunt-icons.png
wait_for HuntIconsHarness.log 'HUNT RESULT' || true
harness_stop

harness_report 'HUNT .*' HuntHudHarness.log HuntIconsHarness.log
! grep -q "HUNT RESULT fail" "$work/HuntHudHarness.log" "$work/HuntIconsHarness.log" \
    && grep -q "HUNT RESULT pass" "$work/HuntHudHarness.log" && grep -q "HUNT RESULT pass" "$work/HuntIconsHarness.log"
