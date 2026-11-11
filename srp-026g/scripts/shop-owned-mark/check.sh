#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: the shop's owned mark and its "Hide owned" check box
# (docs/notes/shop-owned-mark.md), in Ruffle (CPU only: lavapipe on its own virtual X display, never
# the GPU). ShopOwnedHarness.as makes the lobby movie's own inventory list and shop: the entries of
# owned items (a weapon, a stack, a scope loose or worn on a weapon, a camouflage) have the mark, the
# others (an item not owned, an empty stack, a bundle whose id is an owned item's) not; a bought item
# gets it, a sold one loses it; the box on leaves the owned entries out (the upgrades tab too, and an
# item bought while it is on), and off brings them back; the list has no sort and stays in level
# order (opened, a buy, the box, a tab, the inventory); the window's last row in every language:
# [Comparing items] [Hide owned].
# Screenshots into the work dir: before.png (the owned entries marked, the box off), bought.png (the
# helmet bought too), not-owned.png (the box on: the owned entries gone), upgrades.png (the box on,
# the upgrades tab).
# Exits 0 when it says pass; prints its lines. A work dir given is kept (the harness, the shots,
# Ruffle's log). Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md),
# or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"

# The texts the exe gives the movie (set_localization_data), from the dictionaries.
"${PYTHON:-python3}" "$here/../localization-check/harness_text.py" "$work"
harness_compile "$here/ShopOwnedHarness.as" 1280,720 -source-path+="$work"
# The native Scaleform placements Ruffle refuses, so the list's and the card's named children exist.
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
harness_display 1280x720 imagemagick
HARNESS_TIMEOUT=120 harness_run ShopOwnedHarness.log -p low --width 1280 --height 720 ShopOwnedHarness.swf
# The harness holds the list a while after each SHOT line: each one shot as it comes.
taken=0
while :; do
    for _ in $(seq 1200); do
        (($(grep -c 'SHOPMARK SHOT ' "$work/ShopOwnedHarness.log") > taken)) && break
        grep -q 'SHOPMARK RESULT' "$work/ShopOwnedHarness.log" && break
        sleep 0.1
    done
    name=$(grep -o 'SHOPMARK SHOT .*' "$work/ShopOwnedHarness.log" | sed -n "$((taken + 1))s/^SHOPMARK SHOT //p")
    [[ -n $name ]] || break
    sleep 0.5
    DISPLAY=$display import -window root "$work/$name.png" 2>/dev/null || true
    taken=$((taken + 1))
done
wait_for ShopOwnedHarness.log 'SHOPMARK RESULT' 1200 || true
harness_stop
harness_report 'SHOPMARK .*' ShopOwnedHarness.log
grep -q 'SHOPMARK RESULT pass' "$work/ShopOwnedHarness.log"
