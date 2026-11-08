#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: srp's Watch (the top menu's tab and badge, the Watch window,
# the History rows' Watch, the results screen's Watch replay) in every language, at 1280x720 and
# 1920x1080, in Ruffle (CPU only: lavapipe on its own virtual X display, never the GPU).
# ReplayButtonsHarness.as says what.
# Exits 0 when it says pass; prints its lines. A work dir given is kept (the harness, Ruffle's log).
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"

# The texts the exe gives the movie (set_localization_data), from the dictionaries.
"${PYTHON:-python3}" "$here/../localization-check/harness_text.py" "$work"
harness_compile "$here/ReplayButtonsHarness.as" 1280,720 -source-path+="$work"
# The native Scaleform placements Ruffle refuses, so the results screen's named children exist.
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
harness_display 1920x1300
status=0
for size in 1280x720 1920x1080; do
    log=ReplayButtonsHarness-$size.log
    HARNESS_TIMEOUT=180 harness_run "$log" -p low --width "${size%x*}" --height "${size#*x}" ReplayButtonsHarness.swf
    wait_for "$log" 'REPLAYBUTTONS RESULT' 1700 || true
    kill "$ruffle_pid" 2>/dev/null || true
    wait "$ruffle_pid" 2>/dev/null || true
    echo "== $size"
    harness_report 'REPLAYBUTTONS .*' "$log"
    grep -q 'REPLAYBUTTONS RESULT pass' "$work/$log" || status=1
done
harness_stop
exit $status
