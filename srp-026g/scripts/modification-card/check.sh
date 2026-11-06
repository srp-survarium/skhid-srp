#!/usr/bin/env bash
# check.sh <lobby_menu tree> <its original .swf> [work dir]: the modification window's item card is
# the inventory's, at its size, on the screen (also grown), gone with the pointer and the window;
# an item tip is drawn at the lobby's scale once (docs/notes/reroll-keep-old.md), at 2560 x 1440 in
# Ruffle (CPU only: lavapipe on its own virtual X display, never the GPU), with the lobby movie's own
# root. The window plays sounds (ExternalInterface, the exe; Ruffle has none) as it is made: a test
# copy of the tree is built (flash_tree.py) with ../message-focus/HarnessExternal.as in its place.
# The original is revision 24's flash_movies/lobby_menu.swf (movie.json; srp-game-data get).
# Exits 0 when it says pass; prints its lines. A work dir given is kept (the harness, Ruffle's log).
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
tree=$(readlink -f "$1")
original=$(readlink -f "$2")
. "$here/../ruffle_harness.sh"
harness_work "${3:-}"

# The test copy: the recorder for ExternalInterface.
rm -rf "$work/lobby_menu" "$work/lobby_menu.built.swf"
cp -r "$tree" "$work/lobby_menu"
cp "$here/../message-focus/HarnessExternal.as" "$work/lobby_menu/src/HarnessExternal.as"
grep -rlZ "ExternalInterface" "$work/lobby_menu/src" | xargs -0 perl -pi -e 's/^\s*import flash\.external\.ExternalInterface;\n//; s/\bExternalInterface\./HarnessExternal./g'
"${PYTHON:-python3}" "$here/../../../tools/actionscript/flash_tree.py" --tools "$tools" build "$work/lobby_menu" "$original" \
    "$work/lobby_menu.built.swf" >"$work/lobby_menu.build.log" 2>&1 || { cat "$work/lobby_menu.build.log"; exit 1; }
lobby=$work/lobby_menu.built.swf

harness_compile "$here/ModificationCardHarness.as" 2560,1440
# The native Scaleform placements Ruffle refuses, and the sliders' labels, so the root sets up its cards.
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf" --slider-labels
harness_display 2560x1440
HARNESS_TIMEOUT=120 harness_run ModificationCardHarness.log -p low --width 2560 --height 1440 ModificationCardHarness.swf
wait_for ModificationCardHarness.log 'MODCARD RESULT' 1200 || true
harness_stop
harness_report 'MODCARD .*' ModificationCardHarness.log
grep -q 'MODCARD RESULT pass' "$work/ModificationCardHarness.log"
