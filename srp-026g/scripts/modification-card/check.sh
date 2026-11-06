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
main=$(git -C "$here" worktree list --porcelain | sed -n '1s/^worktree //p')
tools=${SRP_AS_TOOLS:-$main/private/actionscript/tools.json}
if [[ $# -ge 3 ]]; then
    work=$(readlink -f "$3")
    mkdir -p "$work"
else
    work=$(mktemp -d)
fi

# The test copy: the recorder for ExternalInterface.
rm -rf "$work/lobby_menu" "$work/lobby_menu.built.swf"
cp -r "$tree" "$work/lobby_menu"
cp "$here/../message-focus/HarnessExternal.as" "$work/lobby_menu/src/HarnessExternal.as"
grep -rlZ "ExternalInterface" "$work/lobby_menu/src" | xargs -0 perl -pi -e 's/^\s*import flash\.external\.ExternalInterface;\n//; s/\bExternalInterface\./HarnessExternal./g'
"${PYTHON:-python3}" "$here/../../../tools/actionscript/flash_tree.py" --tools "$tools" build "$work/lobby_menu" "$original" \
    "$work/lobby_menu.built.swf" >"$work/lobby_menu.build.log" 2>&1 || { cat "$work/lobby_menu.build.log"; exit 1; }
lobby=$work/lobby_menu.built.swf

java=$(sed -n 's/.*"java": *"\([^"]*\)".*/\1/p' "$tools")
flex=$(sed -n 's/.*"flex": *"\([^"]*\)".*/\1/p' "$tools")
"$java" -Xmx1g -Djava.awt.headless=true -jar "$flex/lib/mxmlc.jar" \
    +flexlib="$flex/frameworks" +playerglobalHome="$flex/frameworks/libs/player" -library-path= \
    -external-library-path="$flex/frameworks/libs/player/32.0/playerglobal.swc" -source-path="$here" \
    -omit-trace-statements=false -default-frame-rate=60 -default-size=2560,1440 \
    -output="$work/ModificationCardHarness.swf" "$here/ModificationCardHarness.as" >"$work/ModificationCardHarness.mxmlc.log" 2>&1 \
    || { cat "$work/ModificationCardHarness.mxmlc.log"; exit 1; }
# The native Scaleform placements Ruffle refuses, and the sliders' labels, so the root sets up its cards.
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf" --slider-labels

paths=$(nix build nixpkgs#ruffle nixpkgs#mesa nixpkgs#xorg.xorgserver --no-link --print-out-paths)
ruffle=$(grep -- '-ruffle-' <<<"$paths")
mesa=$(grep -- '-mesa-' <<<"$paths")
xvfb=$(grep -- '-xorg-server-' <<<"$paths")/bin/Xvfb
lvp=$mesa/share/vulkan/icd.d/lvp_icd.x86_64.json
# Xvfb chooses an unused display without touching the owner's desktop.
"$xvfb" -displayfd 3 -screen 0 2560x1440x24 -nolisten tcp 3>"$work/display" >"$work/xvfb.log" 2>&1 &
xvfb_pid=$!
ruffle_pid=
cleanup() { kill $ruffle_pid "$xvfb_pid" 2>/dev/null || true; }
trap 'cleanup; [[ $# -ge 3 ]] || rm -rf "$work"' EXIT
for _ in $(seq 100); do [[ -s $work/display ]] && break; sleep 0.1; done
[[ -s $work/display ]] || { cat "$work/xvfb.log"; exit 1; }

(
    cd "$work"
    unset WAYLAND_DISPLAY XDG_SESSION_TYPE
    DISPLAY=:$(cat display) VK_ICD_FILENAMES=$lvp VK_DRIVER_FILES=$lvp RUST_LOG=info \
        exec timeout 120 "$ruffle/bin/ruffle" -g vulkan -p low --storage memory --scale no-scale \
        --width 2560 --height 1440 ModificationCardHarness.swf
) >"$work/ModificationCardHarness.log" 2>&1 &
ruffle_pid=$!
for _ in $(seq 1200); do grep -q 'MODCARD RESULT' "$work/ModificationCardHarness.log" && break; sleep 0.1; done
cleanup

if ! grep -q 'graphics API vulkan on llvmpipe' "$work/ModificationCardHarness.log"; then
    echo "Ruffle did not run on lavapipe (CPU): see $work/ModificationCardHarness.log" >&2
fi
grep -ho 'MODCARD .*' "$work/ModificationCardHarness.log" || true
grep -q 'MODCARD RESULT pass' "$work/ModificationCardHarness.log"
