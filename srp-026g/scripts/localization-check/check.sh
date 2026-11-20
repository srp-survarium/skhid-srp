#!/usr/bin/env bash
# check.sh <rebuilt lobby_menu.swf> [work dir]: srp's own menu texts in every language, given to the
# movie as the exe gives them (set_localization_data, from the dictionaries): the movie's lookups by
# number, its placeholders and the embedded Calibri's glyphs of every text, in Ruffle, on a private
# CPU-only virtual display.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
# The texts the exe gives the movie (set_localization_data), from the dictionaries.
"${PYTHON:-python3}" "$here/harness_text.py" "$work"
harness_compile "$here/LocalizationHarness.as" 1280,720 -source-path+="$work"
cp "$lobby" "$work/lobby_menu.swf"
harness_display 1280x720
harness_run LocalizationHarness.log -p low --width 1280 --height 720 -Pmovie=lobby_menu.swf LocalizationHarness.swf
wait_for LocalizationHarness.log 'LOCALIZATION RESULT' || true
harness_stop
harness_report 'LOCALIZATION .*' LocalizationHarness.log
grep -q 'LOCALIZATION RESULT pass' "$work/LocalizationHarness.log"
