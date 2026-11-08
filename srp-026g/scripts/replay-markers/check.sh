#!/usr/bin/env bash
# check.sh <replay_player.swf> [work dir]: the replay player's one-string lists from the exe
# (set_markers_text as ticks on the bar, set_players_text, set_replay_speed; MarkersHarness.as says
# what), at 1280x720 and 1920x1080, in Ruffle (CPU only: lavapipe
# on its own virtual X display, never the GPU). Exits 0 when it says pass; prints its lines.
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
player=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
harness_compile "$here/MarkersHarness.as" 1280,720
# The native Scaleform placements Ruffle refuses, so the player's named children exist.
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$player" "$work/replay_player.swf"
# at the game's two common sizes (the controls are centred: the ticks move with them)
harness_display 1920x1280
for size in 1280x720 1920x1080; do
    HARNESS_TIMEOUT=60 harness_run "MarkersHarness-$size.log" -p low --width "${size%x*}" --height "${size#*x}" MarkersHarness.swf
    wait_for "MarkersHarness-$size.log" 'MARKERS RESULT' 600 || true
    kill "$ruffle_pid" 2>/dev/null || true
done
harness_stop
harness_report 'MARKERS .*' MarkersHarness-1280x720.log MarkersHarness-1920x1080.log
grep -q 'MARKERS RESULT pass' "$work/MarkersHarness-1280x720.log"
grep -q 'MARKERS RESULT pass' "$work/MarkersHarness-1920x1080.log"
