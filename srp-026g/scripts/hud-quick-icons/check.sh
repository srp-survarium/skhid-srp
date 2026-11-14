#!/usr/bin/env bash
# check.sh <hud.swf> [work dir]: the quick slots' item icons (stock and srp oxygen tanks and medicines, a used-up
# medicine's gray) in isolated CPU Ruffle.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
hud=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
harness_compile "$here/QuickIconsHarness.as" 1280,720
cp "$hud" "$work/hud.swf"
harness_display 1280x720
harness_run QuickIconsHarness.log -p low --width 1280 --height 720 -Pmovie=hud.swf QuickIconsHarness.swf
wait_for QuickIconsHarness.log 'QUICK ICONS RESULT' || true
harness_stop
harness_report 'QUICK ICONS .*' QuickIconsHarness.log
grep -q 'QUICK ICONS RESULT pass' "$work/QuickIconsHarness.log"
