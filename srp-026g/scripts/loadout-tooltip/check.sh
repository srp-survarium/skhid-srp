#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: the kit's item card after a weapon swap is the new weapon,
# not the helmet (LobbyMenu.updateHintItem), in Ruffle (CPU only: lavapipe on its own virtual X
# display, never the GPU). TooltipHarness.as drops another rifle on the weapon slot with the lobby
# movie's own root, profile and inventory, gives the server's new kit, and checks the cards of the
# weapon slot, the helmet slot and an inventory row, and the weapon slot's right-click menu.
# Exits 0 when it says pass; prints its lines. A work dir given is kept (the harness, Ruffle's log).
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
harness_compile "$here/TooltipHarness.as" 1280,720
# The native Scaleform placements Ruffle refuses, and the sliders' labels, so the root sets up its cards.
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf" --slider-labels
harness_display 1280x720
HARNESS_TIMEOUT=120 harness_run TooltipHarness.log -p low --width 1280 --height 720 TooltipHarness.swf
wait_for TooltipHarness.log 'LOADOUT RESULT' 1200 || true
harness_stop
harness_report 'LOADOUT .*' TooltipHarness.log
grep -q 'LOADOUT RESULT pass' "$work/TooltipHarness.log"
