#!/usr/bin/env bash
# Exercise the built range room card and world-to-minimap transform on CPU Ruffle.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
root=$(cd "$here/../../.." && pwd)
. "$here/../ruffle_harness.sh"
harness_work "${CHECK_WORK:?}"
"${PYTHON:-python3}" "$here/../localization-check/harness_text.py" "$work"
harness_compile "$here/RangeHarness.as" 1280,720 -source-path+="$work"
for movie in hud lobby_menu; do
    "$root/tools/checks/movie.sh" "$movie" "$work/$movie.swf"
done
harness_display 1280x720
for movie in hud lobby_menu; do
    log=$movie.log
    harness_run "$log" -p low --width 1280 --height 720 -Pmovie="$movie.swf" RangeHarness.swf
    wait_for "$log" 'RANGE RESULT' || true
    kill "$ruffle_pid" 2>/dev/null || true
    wait "$ruffle_pid" 2>/dev/null || true
    ruffle_pid=
    harness_report 'RANGE .*' "$log"
    grep -q 'RANGE RESULT pass' "$work/$log"
done
