#!/usr/bin/env bash
# check.sh <player_icons.swf> [work dir]: partial marker updates in CPU Ruffle.
set -euo pipefail
[[ $# -ge 1 && $# -le 2 ]] || { echo 'usage: check.sh <player_icons.swf> [work dir]' >&2; exit 2; }
here=$(dirname "$(readlink -f "$0")")
movie=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
harness_compile "$here/BatteryMarkerHarness.as" 800,480
cp "$movie" "$work/player_icons.swf"
harness_display 1024x768
# The harness quits Ruffle when it is done.
HARNESS_TIMEOUT=30 harness_run ruffle.log -p low --width 800 --height 480 BatteryMarkerHarness.swf
wait "$ruffle_pid" || { cat "$work/ruffle.log"; exit 1; }
grep 'BATTERY ' "$work/ruffle.log" || true
grep -q 'graphics API vulkan on llvmpipe' "$work/ruffle.log"
grep -q 'BATTERY RESULT pass' "$work/ruffle.log"
