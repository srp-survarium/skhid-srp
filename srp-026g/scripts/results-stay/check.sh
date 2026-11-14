#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: the match results stay up while the rooms window changes
# under them (docs/notes/rooms.md, "The results stay until the player closes them"), in Ruffle
# (CPU only: lavapipe on its own virtual X display, never the GPU). ResultsHarness.as makes the
# lobby movie's own top menu, results screen and rooms window, opens the results with
# LobbyMenu.show_victory_screen and tells the window what the server answers after a match: let in
# from a room's queue, the room's next vote, out of the room. The results stay open, the top menu's
# tab as it was; closing them shows the room's window, its tab chosen.
# Exits 0 when it says pass; prints its lines. A work dir given is kept (the harness, Ruffle's log).
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"

# The texts the exe gives the movie (set_localization_data), from the dictionaries.
"${PYTHON:-python3}" "$here/../localization-check/harness_text.py" "$work" english
harness_compile "$here/ResultsHarness.as" 1280,720 -source-path+="$work"
# The native Scaleform placements Ruffle refuses, so the results screen's named children exist.
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
harness_display 1280x720
HARNESS_TIMEOUT=120 harness_run ResultsHarness.log -p low --width 1280 --height 720 ResultsHarness.swf
wait_for ResultsHarness.log 'RESULTS RESULT' 1200 || true
harness_stop
harness_report 'RESULTS .*' ResultsHarness.log
grep -q 'RESULTS RESULT pass' "$work/ResultsHarness.log"
