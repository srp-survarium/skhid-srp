#!/usr/bin/env bash
# check.sh <hud.swf> [work dir]: the Tab list's player menu (docs/notes/tab-cursor-friends.md) in
# Ruffle (CPU only: lavapipe on its own virtual X display, never the GPU), by RowMenuHarness.as: the
# menu's right-clicks and the exe's answers, then a real mouse click (xdotool) on "Add to friends".
# Exits 0 when it says pass; prints its lines. A work dir given is kept (the harness, Ruffle's log).
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
hud=$(readlink -f "$1")
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
    -omit-trace-statements=false -default-frame-rate=60 -default-size=800,480 \
    -output="$work/RowMenuHarness.swf" "$here/RowMenuHarness.as" >"$work/mxmlc.log" 2>&1 \
    || { cat "$work/mxmlc.log"; exit 1; }
cp "$hud" "$work/hud.swf"

paths=$(nix build nixpkgs#ruffle nixpkgs#mesa nixpkgs#xdotool nixpkgs#xorg.xorgserver --no-link --print-out-paths)
ruffle=$(grep -- '-ruffle-' <<<"$paths")
mesa=$(grep -- '-mesa-' <<<"$paths")
xdotool=$(grep -- '-xdotool-' <<<"$paths")/bin/xdotool
xvfb=$(grep -- '-xorg-server-' <<<"$paths")/bin/Xvfb
lvp=$mesa/share/vulkan/icd.d/lvp_icd.x86_64.json
display=:$((90 + RANDOM % 9))
"$xvfb" "$display" -screen 0 1024x768x24 -nolisten tcp >"$work/xvfb.log" 2>&1 &
xvfb_pid=$!
ruffle_pid=
cleanup() { kill $ruffle_pid "$xvfb_pid" 2>/dev/null || true; }
trap 'cleanup; [[ $# -ge 2 ]] || rm -rf "$work"' EXIT
sleep 1

(
    cd "$work"
    unset WAYLAND_DISPLAY XDG_SESSION_TYPE
    DISPLAY=$display VK_ICD_FILENAMES=$lvp VK_DRIVER_FILES=$lvp RUST_LOG=info \
        exec timeout 120 "$ruffle/bin/ruffle" -g vulkan -p low --storage memory --scale no-scale \
        --width 800 --height 480 -Pmovie=hud.swf RowMenuHarness.swf
) >"$work/RowMenuHarness.log" 2>&1 &
ruffle_pid=$!
log=$work/RowMenuHarness.log
wait_for() { for _ in $(seq 600); do grep -q "$1" "$log" && return 0; sleep 0.1; done; return 1; }

wait_for 'MENU calibrate' || echo "the harness did not get to the click: see $log" >&2
# the one window on this display: Ruffle's
window=$(DISPLAY=$display timeout 20 "$xdotool" search --sync --onlyvisible --name "." | head -1 || true)
# where the window's (400, 300) lands on the stage: the movie's offset in the window (a menu bar)
DISPLAY=$display "$xdotool" mousemove --window "$window" 400 300 click 1
wait_for 'MENU target' || true
read -r sx sy < <(sed -n 's/.*MENU mouse down at \([0-9]*\),\([0-9]*\).*/\1 \2/p' "$log" | tail -1)
read -r tx ty < <(sed -n 's/.*MENU target the entry at \([0-9]*\),\([0-9]*\).*/\1 \2/p' "$log")
DISPLAY=$display "$xdotool" mousemove --window "$window" $((tx + 400 - sx)) $((ty + 300 - sy)) click 1
wait_for 'MENU RESULT' || true
cleanup

if ! grep -q 'graphics API vulkan on llvmpipe' "$log"; then
    echo "Ruffle did not run on lavapipe (CPU): see $log" >&2
fi
grep -o 'MENU .*' "$log" || true
grep -q 'MENU RESULT pass' "$log"
