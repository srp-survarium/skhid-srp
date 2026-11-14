#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: only the shop asks before buying an item owned already, a
# camouflage too; the player's own items and the craft window do not (docs/notes/items.md §6), in
# Ruffle (CPU only: lavapipe on its own virtual X display, never the GPU). BuyOwnedHarness.as calls
# the lobby movie's own root (try_buy_item): an owned camouflage, one not owned, an owned helmet
# and an owned scope.
# Exits 0 when it says pass; prints its lines. A work dir given is kept (the harness, Ruffle's log).
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
harness_compile "$here/BuyOwnedHarness.as" 1280,720
# The native Scaleform placements Ruffle refuses, so the list's and the card's named children exist.
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
harness_display 1280x720
HARNESS_TIMEOUT=120 harness_run BuyOwnedHarness.log -p low --width 1280 --height 720 BuyOwnedHarness.swf
wait_for BuyOwnedHarness.log 'BUYOWNED RESULT' 1200 || true
harness_stop
harness_report 'BUYOWNED .*' BuyOwnedHarness.log
grep -q 'BUYOWNED RESULT pass' "$work/BuyOwnedHarness.log"
