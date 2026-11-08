#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: srp's replay buttons (the History rows' Download, Watch and
# chip; the results screen's Watch replay and Watch my deaths) in every language, in Ruffle (CPU
# only: lavapipe on its own virtual X display, never the GPU). ReplayButtonsHarness.as says what.
# Exits 0 when it says pass; prints its lines. A work dir given is kept (the harness, Ruffle's log).
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"

# The texts the exe gives the movie (set_localization_data), from the dictionaries.
"${PYTHON:-python3}" "$here/../localization-check/harness_text.py" "$work"
harness_compile "$here/ReplayButtonsHarness.as" 1280,800 -source-path+="$work"
# The native Scaleform placements Ruffle refuses, so the results screen's named children exist.
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
harness_display 1280x800
HARNESS_TIMEOUT=120 harness_run ReplayButtonsHarness.log -p low --width 1280 --height 800 ReplayButtonsHarness.swf
wait_for ReplayButtonsHarness.log 'REPLAYBUTTONS RESULT' 1200 || true
harness_stop
harness_report 'REPLAYBUTTONS .*' ReplayButtonsHarness.log
grep -q 'REPLAYBUTTONS RESULT pass' "$work/ReplayButtonsHarness.log"
