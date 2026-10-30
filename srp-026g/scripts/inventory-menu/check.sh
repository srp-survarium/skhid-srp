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
main=$(git -C "$here" worktree list --porcelain | sed -n '1s/^worktree //p')
tools=${SRP_AS_TOOLS:-$main/private/actionscript/tools.json}
if [[ $# -ge 2 ]]; then
    work=$(readlink -f "$2")
    mkdir -p "$work"
else
    work=$(mktemp -d)
fi

java=$(sed -n 's/.*"java": *"\([^"]*\)".*/\1/p' "$tools")
flex=$(sed -n 's/.*"flex": *"\([^"]*\)".*/\1/p' "$tools")
"$java" -Xmx1g -Djava.awt.headless=true -jar "$flex/lib/mxmlc.jar" \
    +flexlib="$flex/frameworks" +playerglobalHome="$flex/frameworks/libs/player" -library-path= \
    -external-library-path="$flex/frameworks/libs/player/32.0/playerglobal.swc" -source-path="$here" \
    -omit-trace-statements=false -default-frame-rate=60 -default-size=1280,720 \
    -output="$work/InventoryMenuHarness.swf" "$here/InventoryMenuHarness.as" >"$work/InventoryMenuHarness.mxmlc.log" 2>&1 \
    || { cat "$work/InventoryMenuHarness.mxmlc.log"; exit 1; }
# The native Scaleform placements Ruffle refuses, so the list's and the card's named children exist.
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"

paths=$(nix build nixpkgs#ruffle nixpkgs#mesa nixpkgs#xorg.xorgserver --no-link --print-out-paths)
ruffle=$(grep -- '-ruffle-' <<<"$paths")
mesa=$(grep -- '-mesa-' <<<"$paths")
xvfb=$(grep -- '-xorg-server-' <<<"$paths")/bin/Xvfb
lvp=$mesa/share/vulkan/icd.d/lvp_icd.x86_64.json
# Xvfb chooses an unused display without touching the owner's desktop.
"$xvfb" -displayfd 3 -screen 0 1280x720x24 -nolisten tcp 3>"$work/display" >"$work/xvfb.log" 2>&1 &
xvfb_pid=$!
ruffle_pid=
cleanup() { kill $ruffle_pid "$xvfb_pid" 2>/dev/null || true; }
trap 'cleanup; [[ $# -ge 2 ]] || rm -rf "$work"' EXIT
for _ in $(seq 100); do [[ -s $work/display ]] && break; sleep 0.1; done
[[ -s $work/display ]] || { cat "$work/xvfb.log"; exit 1; }

(
    cd "$work"
    unset WAYLAND_DISPLAY XDG_SESSION_TYPE
    DISPLAY=:$(cat display) VK_ICD_FILENAMES=$lvp VK_DRIVER_FILES=$lvp RUST_LOG=info \
        exec timeout 120 "$ruffle/bin/ruffle" -g vulkan -p low --storage memory --scale no-scale \
        --width 1280 --height 720 InventoryMenuHarness.swf
) >"$work/InventoryMenuHarness.log" 2>&1 &
ruffle_pid=$!
for _ in $(seq 1200); do grep -q 'INVMENU RESULT' "$work/InventoryMenuHarness.log" && break; sleep 0.1; done
cleanup

if ! grep -q 'graphics API vulkan on llvmpipe' "$work/InventoryMenuHarness.log"; then
    echo "Ruffle did not run on lavapipe (CPU): see $work/InventoryMenuHarness.log" >&2
fi
grep -ho 'INVMENU .*' "$work/InventoryMenuHarness.log" || true
grep -q 'INVMENU RESULT pass' "$work/InventoryMenuHarness.log"
