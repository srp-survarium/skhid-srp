#!/usr/bin/env bash
# check.sh <rebuilt hud.swf> [work dir]: mode introduction lifecycle, translations and
# temporary chat visibility in Ruffle on a private CPU-only virtual display.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
hud=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
"${PYTHON:-python3}" "$here/../localization-check/harness_text.py" "$work"
harness_compile "$here/ModeIntroHarness.as" 1280,720 -source-path+="$work"
cp "$hud" "$work/hud.swf"
harness_display 1280x720
harness_run ModeIntroHarness.log -p low --width 1280 --height 720 -Pmovie=hud.swf ModeIntroHarness.swf
wait_for ModeIntroHarness.log 'INTRO RESULT' || true
harness_stop
harness_report 'INTRO .*' ModeIntroHarness.log
grep -q 'INTRO RESULT pass' "$work/ModeIntroHarness.log"
