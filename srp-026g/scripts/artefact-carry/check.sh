#!/usr/bin/env bash
# check.sh <hud.swf> [work dir]: artefact take prompt in isolated CPU Ruffle.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
hud=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
harness_compile "$here/CarryHarness.as" 1280,720
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$hud" "$work/hud.swf"
harness_display 1280x720
harness_run CarryHarness.log -p low --width 1280 --height 720 -Pmovie=hud.swf CarryHarness.swf
wait_for CarryHarness.log 'CARRY RESULT' || true
harness_stop
harness_report 'CARRY .*' CarryHarness.log
grep -q 'CARRY RESULT pass' "$work/CarryHarness.log"
