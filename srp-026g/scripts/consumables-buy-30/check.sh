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
    -output="$work/ConsumablesBuyHarness.swf" "$here/ConsumablesBuyHarness.as" >"$work/ConsumablesBuyHarness.mxmlc.log" 2>&1 \
    || { cat "$work/ConsumablesBuyHarness.mxmlc.log"; exit 1; }
# The native Scaleform placements Ruffle refuses, so the dialog's named children exist.
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"

paths=$(nix build nixpkgs#ruffle nixpkgs#mesa nixpkgs#xorg.xorgserver nixpkgs#imagemagick --no-link --print-out-paths)
magick=$(grep -- 'imagemagick-' <<<"$paths" | grep -v dev | head -1)
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
display=:$(cat "$work/display")

# A game box calls the exe as it is made (ExternalInterface, sound_play): Ruffle's dummy interface
# lets the real buy dialog be made.
(
    cd "$work"
    unset WAYLAND_DISPLAY XDG_SESSION_TYPE
    DISPLAY=$display VK_ICD_FILENAMES=$lvp VK_DRIVER_FILES=$lvp RUST_LOG=info \
        exec timeout 120 "$ruffle/bin/ruffle" -g vulkan -p low --storage memory --scale no-scale \
        --dummy-external-interface --width 1280 --height 720 ConsumablesBuyHarness.swf
) >"$work/ConsumablesBuyHarness.log" 2>&1 &
ruffle_pid=$!
# The harness holds the dialog a while after the SHOT line.
for _ in $(seq 1200); do grep -q "BUY30 SHOT dialog-30\|BUY30 RESULT" "$work/ConsumablesBuyHarness.log" && break; sleep 0.1; done
if grep -q "BUY30 SHOT dialog-30" "$work/ConsumablesBuyHarness.log"; then
    sleep 0.5
    DISPLAY=$display "$magick/bin/import" -window root "$work/dialog-30.png" 2>/dev/null || true
fi
for _ in $(seq 1200); do grep -q 'BUY30 RESULT' "$work/ConsumablesBuyHarness.log" && break; sleep 0.1; done
cleanup

if ! grep -q 'graphics API vulkan on llvmpipe' "$work/ConsumablesBuyHarness.log"; then
    echo "Ruffle did not run on lavapipe (CPU): see $work/ConsumablesBuyHarness.log" >&2
fi
grep -ho 'BUY30 .*' "$work/ConsumablesBuyHarness.log" || true
grep -q 'BUY30 RESULT pass' "$work/ConsumablesBuyHarness.log"
