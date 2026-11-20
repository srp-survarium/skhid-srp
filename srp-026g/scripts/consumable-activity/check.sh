#!/usr/bin/env bash
# check.sh <hud.swf> [work dir]: native countdown presentation in isolated CPU Ruffle.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
hud=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
harness_compile "$here/ActivityHarness.as" 1280,720
cp "$hud" "$work/hud.swf"
harness_display 1280x720
harness_run ActivityHarness.log -p low --width 1280 --height 720 -Pmovie=hud.swf ActivityHarness.swf
wait_for ActivityHarness.log 'ACTIVITY RESULT' || true
harness_stop
harness_report 'ACTIVITY .*' ActivityHarness.log
grep -q 'ACTIVITY RESULT pass' "$work/ActivityHarness.log"
