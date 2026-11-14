#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: the right-click menu's Buy buys the item the menu was opened
# on (docs/notes/shop-character-menu.md), in Ruffle (CPU only: lavapipe on its own virtual X
# display, never the GPU). InventoryMenuHarness.as makes the lobby movie's own inventory list, item
# card, character menu and shop, right-clicks a list row, the kit's items in the inventory and in the
# shop, a bundle whose id is an owned item's, and a shop entry locked at its seller, chooses Buy and checks what reaches the buy dialog
# (GangShop.buyFunction).
# Exits 0 when it says pass; prints its lines. A work dir given is kept (the harness, Ruffle's log).
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
harness_compile "$here/InventoryMenuHarness.as" 1280,720
# The native Scaleform placements Ruffle refuses, so the list's and the card's named children exist.
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
harness_display 1280x720
HARNESS_TIMEOUT=120 harness_run InventoryMenuHarness.log -p low --width 1280 --height 720 InventoryMenuHarness.swf
wait_for InventoryMenuHarness.log 'INVMENU RESULT' 1200 || true
harness_stop
harness_report 'INVMENU .*' InventoryMenuHarness.log
grep -q 'INVMENU RESULT pass' "$work/InventoryMenuHarness.log"
