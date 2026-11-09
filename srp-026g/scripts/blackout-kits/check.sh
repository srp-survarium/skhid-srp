#!/usr/bin/env bash
# check.sh <hud.swf> <player_icons.swf> [work dir]: Blackout's Tab view (docs/notes/blackout.md) in
# Ruffle (CPU only: lavapipe on its own virtual X display, never the GPU):
# - KitsHarness.as: the hud's own kit panel and body doll kept by its own KeptLayer beside the hidden
#   root, then a kit key and a real mouse click (xdotool) pick kits there; the use prompt
#   (SrvContextInfo) kept with its progress while forced, given back after;
# - IconsHarness.as: the player icons movie itself told the forced state: the teammates' markers
#   always, an enemy's never, the objects' not while forced.
# Exits 0 when both say pass; prints their lines. A work dir given is kept (the harnesses, Ruffle's
# logs). Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or
# SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
hud=$(readlink -f "$1")
icons=$(readlink -f "$2")
. "$here/../ruffle_harness.sh"
harness_work "${3:-}"
for harness in KitsHarness IconsHarness; do
    harness_compile "$here/$harness.as" 800,480
done
cp "$hud" "$work/hud.swf"
cp "$icons" "$work/player_icons.swf"
harness_display 1024x768 xdotool
export HARNESS_TIMEOUT=120

# run <harness> <movie>: Ruffle on the harness in the background, its log <harness>.log
run() { harness_run "$1.log" -p low --width 800 --height 480 -Pmovie="$2" "$1.swf"; }

run KitsHarness hud.swf
log=$work/KitsHarness.log
wait_for KitsHarness.log 'KITS target' 600 || { echo "the kits harness did not get to the click: see $log" >&2; }
# the one window on this display: Ruffle's
window=$(DISPLAY=$display timeout 20 xdotool search --sync --onlyvisible --name "." | head -1 || true)
read -r tx ty < <(sed -n 's/.*KITS target kit 3 at \([0-9]*\),\([0-9]*\).*/\1 \2/p' "$log")
# where the window's (400, 300) lands on the stage: the movie's offset in the window (a menu bar)
DISPLAY=$display xdotool mousemove --window "$window" 400 300 click 1
wait_for KitsHarness.log 'KITS mouse down' 600 || true
read -r sx sy < <(sed -n 's/.*KITS mouse down at \([0-9]*\),\([0-9]*\).*/\1 \2/p' "$log" | head -1)
DISPLAY=$display xdotool mousemove --window "$window" $((tx + 400 - sx)) $((ty + 300 - sy)) click 1
wait_for KitsHarness.log 'KITS RESULT' 600 || true
kill $ruffle_pid 2>/dev/null || true

run IconsHarness player_icons.swf
wait_for IconsHarness.log 'ICONS RESULT' 600 || true
harness_stop

harness_report 'KITS .*\|ICONS .*' KitsHarness.log IconsHarness.log
grep -q 'KITS RESULT pass' "$work/KitsHarness.log" && grep -q 'ICONS RESULT pass' "$work/IconsHarness.log"
