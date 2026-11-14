#!/usr/bin/env bash
# check.sh <rebuilt lobby_menu.swf> <rebuilt hud.swf> [work dir]: players' names, clan tags and lines in
# the lobby's and the match's chats shown as typed, no markup read from them, the lobby's name link
# read back whole (ChatNamesHarness.as), in Ruffle on a private CPU-only virtual display. Python for the
# movies' copies Ruffle reads (medical-cards/ruffle_movie.py): nix develop.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
hud=$(readlink -f "$2")
. "$here/../ruffle_harness.sh"
harness_work "${3:-}"
harness_compile "$here/ChatNamesHarness.as" 1280,720
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$hud" "$work/hud.swf"
harness_display 1280x720
harness_run ChatNamesHarness.log -p low --width 1280 --height 720 ChatNamesHarness.swf
wait_for ChatNamesHarness.log 'NAMES RESULT' || true
harness_stop
harness_report 'NAMES .*' ChatNamesHarness.log
grep -q 'NAMES RESULT pass' "$work/ChatNamesHarness.log"
