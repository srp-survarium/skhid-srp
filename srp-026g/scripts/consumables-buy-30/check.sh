#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: a consumable's buy dialog starts at 30, or at what the money
# buys when less (docs/notes/items.md §8), in Ruffle (CPU only: lavapipe on its own virtual X
# display, never the GPU). ConsumablesBuyHarness.as makes each buy with the lobby movie's own root
# (try_buy_item) and reads the game's own buy dialog (MessageAsk): medkits, a grenade and a trap
# start at 30 (12 with money for 12, 0 with none), a drop on a pocket too (no pocket cap); a booster,
# ammo and a helmet as before.
# Screenshot into the work dir: dialog-30.png (the medkit's dialog at 30).
# Exits 0 when it says pass; prints its lines. A work dir given is kept (the harness, the shot,
# Ruffle's log). Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md),
# or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
harness_compile "$here/ConsumablesBuyHarness.as" 1280,720
# The native Scaleform placements Ruffle refuses, so the dialog's named children exist.
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
harness_display 1280x720 imagemagick
# A game box calls the exe as it is made (ExternalInterface, sound_play): Ruffle's dummy interface
# lets the real buy dialog be made.
HARNESS_TIMEOUT=120 harness_run ConsumablesBuyHarness.log -p low --dummy-external-interface \
    --width 1280 --height 720 ConsumablesBuyHarness.swf
# The harness holds the dialog a while after the SHOT line.
wait_for ConsumablesBuyHarness.log 'BUY30 SHOT dialog-30\|BUY30 RESULT' 1200 || true
if grep -q "BUY30 SHOT dialog-30" "$work/ConsumablesBuyHarness.log"; then
    sleep 0.5
    DISPLAY=$display import -window root "$work/dialog-30.png" 2>/dev/null || true
fi
wait_for ConsumablesBuyHarness.log 'BUY30 RESULT' 1200 || true
harness_stop
harness_report 'BUY30 .*' ConsumablesBuyHarness.log
grep -q 'BUY30 RESULT pass' "$work/ConsumablesBuyHarness.log"
