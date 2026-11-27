#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: medical cards in isolated CPU Ruffle.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
harness_compile "$here/MedicalCardsHarness.as" 1280,720
"${PYTHON:-python3}" "$here/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
harness_display 1280x720
harness_run MedicalCardsHarness.log -p low --width 1280 --height 720 -Pmovie=lobby_menu.swf MedicalCardsHarness.swf
wait_for MedicalCardsHarness.log 'MEDICAL RESULT' || true
harness_stop
harness_report 'MEDICAL .*' MedicalCardsHarness.log
grep -q 'MEDICAL RESULT pass' "$work/MedicalCardsHarness.log"
