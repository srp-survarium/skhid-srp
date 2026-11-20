#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: the lobby scene's fade (srp_scene_fade, exe patch 81,
# docs/notes/lobby-scenes.md) in Ruffle (CPU only: lavapipe on its own virtual X display, never the
# GPU). FadeHarness.as calls the lobby movie's own root: the black is the root's first child, under
# every panel; half, whole with the loading spinner, gone; no click lands on it; the lobby window
# opened and closed by its toggle, then left open for window.png.
# Exits 0 when it says pass; prints its lines. A work dir given is kept (the harness, Ruffle's log).
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
# The texts the exe gives the movie (set_localization_data), from the dictionaries.
"${PYTHON:-python3}" "$here/../localization-check/harness_text.py" "$work"
harness_compile "$here/FadeHarness.as" 1280,720 -source-path+="$work"
cp "$lobby" "$work/lobby_menu.swf"
harness_display 1280x720 imagemagick
harness_run FadeHarness.log -p low --width 1280 --height 720 -Pmovie=lobby_menu.swf FadeHarness.swf
wait_for FadeHarness.log 'FADE RESULT' || true
# the lobby window as Ruffle draws it, left open at the end (window.png in the work dir)
sleep 1
DISPLAY=$display import -window root "$work/window.png" 2>/dev/null || true
harness_stop
harness_report 'FADE .*' FadeHarness.log
grep -q 'FADE RESULT pass' "$work/FadeHarness.log"
