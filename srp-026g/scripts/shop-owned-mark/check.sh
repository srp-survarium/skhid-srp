#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: the shop's owned mark and its "Hide owned" check box
# (docs/notes/shop-owned-mark.md), in Ruffle (CPU only: lavapipe on its own virtual X display, never
# the GPU). ShopOwnedHarness.as makes the lobby movie's own inventory list and shop: the entries of
# owned items (a weapon, a stack, a scope loose or worn on a weapon, a camouflage) have the mark, the
# others (an item not owned, an empty stack, a bundle whose id is an owned item's) not; a bought item
# gets it, a sold one loses it; the box on leaves the owned entries out (the upgrades tab too, and an
# item bought while it is on), and off brings them back; the box sits left of Buy, a
# Russian label too.
# Screenshots into the work dir: before.png (the owned entries marked, the box off), bought.png (the
# helmet bought too), not-owned.png (the box on: the owned entries gone), upgrades.png (the box on,
# the upgrades tab), long-label.png (the Russian label left of Buy).
# Exits 0 when it says pass; prints its lines. A work dir given is kept (the harness, the shots,
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
    -output="$work/ShopOwnedHarness.swf" "$here/ShopOwnedHarness.as" >"$work/ShopOwnedHarness.mxmlc.log" 2>&1 \
    || { cat "$work/ShopOwnedHarness.mxmlc.log"; exit 1; }
# The native Scaleform placements Ruffle refuses, so the list's and the card's named children exist.
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

(
    cd "$work"
    unset WAYLAND_DISPLAY XDG_SESSION_TYPE
    DISPLAY=$display VK_ICD_FILENAMES=$lvp VK_DRIVER_FILES=$lvp RUST_LOG=info \
        exec timeout 120 "$ruffle/bin/ruffle" -g vulkan -p low --storage memory --scale no-scale \
        --width 1280 --height 720 ShopOwnedHarness.swf
) >"$work/ShopOwnedHarness.log" 2>&1 &
ruffle_pid=$!
# The harness holds the list a while after each SHOT line.
for name in before bought not-owned upgrades long-label; do
    for _ in $(seq 1200); do grep -q "SHOPMARK SHOT $name\|SHOPMARK RESULT" "$work/ShopOwnedHarness.log" && break; sleep 0.1; done
    grep -q "SHOPMARK SHOT $name" "$work/ShopOwnedHarness.log" || break
    sleep 0.5
    DISPLAY=$display "$magick/bin/import" -window root "$work/$name.png" 2>/dev/null || true
done
for _ in $(seq 1200); do grep -q 'SHOPMARK RESULT' "$work/ShopOwnedHarness.log" && break; sleep 0.1; done
cleanup

if ! grep -q 'graphics API vulkan on llvmpipe' "$work/ShopOwnedHarness.log"; then
    echo "Ruffle did not run on lavapipe (CPU): see $work/ShopOwnedHarness.log" >&2
fi
grep -ho 'SHOPMARK .*' "$work/ShopOwnedHarness.log" || true
grep -q 'SHOPMARK RESULT pass' "$work/ShopOwnedHarness.log"
