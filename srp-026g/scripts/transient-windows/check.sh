#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: the lobby's transient windows (docs/notes/transient-windows.md)
# in Ruffle (CPU only: lavapipe on its own virtual X display, never the GPU). TransientHarness.as
# makes the lobby movie's own root, History, bottom bar, top bar (the match settings under Play and
# their dropdowns) and rooms window, and checks: History closed by the shop, the rooms window and a
# press on Settings, opened again, kept with nothing else opened; one transient at a time (a
# dropdown inside the match settings keeps them); a press outside closes them and still arrives;
# Esc closes the top one first; the match settings on the transient layer over the rooms window,
# on top where pressed, no part of the top bar over it; the presets list; a resize.
# Exits 0 when it says pass; prints its lines. A work dir given is kept (the harness, Ruffle's log).
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"

# The texts the exe gives the movie (set_localization_data), from the dictionaries.
"${PYTHON:-python3}" "$here/../localization-check/harness_text.py" "$work"
harness_compile "$here/TransientHarness.as" 1280,720 -source-path+="$work"
# Ruffle needs the movie's Scaleform placements adapted (a copy; the top bar's named parts).
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
harness_display 1280x720
HARNESS_TIMEOUT=120 harness_run TransientHarness.log -p low --width 1280 --height 720 TransientHarness.swf
wait_for TransientHarness.log 'TRANSIENT RESULT' 1200 || true
harness_stop
harness_report 'TRANSIENT .*' TransientHarness.log
grep -q 'TRANSIENT RESULT pass' "$work/TransientHarness.log"
