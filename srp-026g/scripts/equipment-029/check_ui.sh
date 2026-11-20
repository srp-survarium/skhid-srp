#!/usr/bin/env bash
# check_ui.sh <lobby_menu.swf> <work directory>: compiled premium UI in CPU Ruffle.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "$2"
harness_compile "$here/EquipmentHarness.as" 640,480
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
harness_display 640x480
HARNESS_TIMEOUT=60 harness_run ruffle.log -p low --width 640 --height 480 EquipmentHarness.swf
wait_for ruffle.log 'EQUIPMENT RESULT' 600 || true
harness_stop
harness_report 'EQUIPMENT RESULT.*' ruffle.log
grep -q 'EQUIPMENT RESULT pass' "$work/ruffle.log"
