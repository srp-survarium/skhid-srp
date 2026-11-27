#!/usr/bin/env bash
# check.sh <hud.swf> [work dir]: the Tab list's player menu (docs/notes/tab-cursor-friends.md) in
# Ruffle (CPU only: lavapipe on its own virtual X display, never the GPU), by RowMenuHarness.as: the
# menu's right-clicks and the exe's answers, then a real mouse click (xdotool) on "Add to friends".
# Exits 0 when it says pass; prints its lines. A work dir given is kept (the harness, Ruffle's log).
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
hud=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
harness_compile "$here/RowMenuHarness.as" 800,480
cp "$hud" "$work/hud.swf"
harness_display 1024x768 xdotool
HARNESS_TIMEOUT=120 harness_run RowMenuHarness.log -p low --width 800 --height 480 -Pmovie=hud.swf RowMenuHarness.swf
log=$work/RowMenuHarness.log

wait_for RowMenuHarness.log 'MENU calibrate' 600 || echo "the harness did not get to the click: see $log" >&2
# the one window on this display: Ruffle's
window=$(DISPLAY=$display timeout 20 xdotool search --sync --onlyvisible --name "." | head -1 || true)
# where the window's (400, 300) lands on the stage: the movie's offset in the window (a menu bar)
DISPLAY=$display xdotool mousemove --window "$window" 400 300 click 1
wait_for RowMenuHarness.log 'MENU target' 600 || true
read -r sx sy < <(sed -n 's/.*MENU mouse down at \([0-9]*\),\([0-9]*\).*/\1 \2/p' "$log" | tail -1)
read -r tx ty < <(sed -n 's/.*MENU target the entry at \([0-9]*\),\([0-9]*\).*/\1 \2/p' "$log")
DISPLAY=$display xdotool mousemove --window "$window" $((tx + 400 - sx)) $((ty + 300 - sy)) click 1
wait_for RowMenuHarness.log 'MENU RESULT' 600 || true
harness_stop

harness_report 'MENU .*' RowMenuHarness.log
grep -q 'MENU RESULT pass' "$log"
