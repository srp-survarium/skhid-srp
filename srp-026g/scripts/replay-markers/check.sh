#!/usr/bin/env bash
# check.sh <replay_player.swf> [work dir]: the replay player's one-string lists from the exe
# (set_markers_text, set_players_text; MarkersHarness.as says what), in Ruffle (CPU only: lavapipe
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
harness_display 1280x720
HARNESS_TIMEOUT=60 harness_run MarkersHarness.log -p low --width 1280 --height 720 MarkersHarness.swf
wait_for MarkersHarness.log 'MARKERS RESULT' 600 || true
harness_stop
harness_report 'MARKERS .*' MarkersHarness.log
grep -q 'MARKERS RESULT pass' "$work/MarkersHarness.log"
