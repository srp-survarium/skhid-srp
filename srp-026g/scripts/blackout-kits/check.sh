#!/usr/bin/env bash
# check.sh <hud.swf> [work dir]: Blackout's kits on Tab (docs/notes/blackout.md) in Ruffle (CPU only:
# lavapipe on its own virtual X display, never the GPU), by KitsHarness.as: the hud's own kit panel
# moved into a kept layer beside its hidden root as HUD.keep moves it, then a kit key and a real mouse
# click (xdotool) pick kits there. Exits 0 when the harness says pass; prints its lines. A work dir
# given is kept (the harness, Ruffle's log). Java and Flex from private/actionscript/tools.json
# (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
movie=$(readlink -f "$1")
main=$(git -C "$here" worktree list --porcelain | sed -n '1s/^worktree //p')
tools=${SRP_AS_TOOLS:-$main/private/actionscript/tools.json}
if [[ $# -ge 2 ]]; then
    work=$(readlink -f "$2")
    mkdir -p "$work"
else
    work=$(mktemp -d)
    trap 'rm -rf "$work"' EXIT
fi

java=$(sed -n 's/.*"java": *"\([^"]*\)".*/\1/p' "$tools")
flex=$(sed -n 's/.*"flex": *"\([^"]*\)".*/\1/p' "$tools")
"$java" -Xmx1g -Djava.awt.headless=true -jar "$flex/lib/mxmlc.jar" \
    +flexlib="$flex/frameworks" +playerglobalHome="$flex/frameworks/libs/player" -library-path= \
    -external-library-path="$flex/frameworks/libs/player/32.0/playerglobal.swc" -source-path="$here" \
    -omit-trace-statements=false -default-frame-rate=60 -default-size=800,480 \
    -output="$work/harness.swf" "$here/KitsHarness.as" >"$work/mxmlc.log" 2>&1 || { cat "$work/mxmlc.log"; exit 1; }
cp "$movie" "$work/hud.swf"

paths=$(nix build nixpkgs#ruffle nixpkgs#mesa nixpkgs#xdotool nixpkgs#xorg.xorgserver --no-link --print-out-paths)
ruffle=$(grep -- '-ruffle-' <<<"$paths")
mesa=$(grep -- '-mesa-' <<<"$paths")
xdotool=$(grep -- '-xdotool-' <<<"$paths")/bin/xdotool
xvfb=$(grep -- '-xorg-server-' <<<"$paths")/bin/Xvfb
lvp=$mesa/share/vulkan/icd.d/lvp_icd.x86_64.json
log=$work/ruffle.log
display=:$((90 + RANDOM % 9))
"$xvfb" "$display" -screen 0 1024x768x24 -nolisten tcp >"$work/xvfb.log" 2>&1 &
xvfb_pid=$!
cleanup() { kill "$ruffle_pid" "$xvfb_pid" 2>/dev/null || true; }
trap 'cleanup; [[ $# -ge 2 ]] || rm -rf "$work"' EXIT
sleep 1
(
    cd "$work"
    unset WAYLAND_DISPLAY XDG_SESSION_TYPE
    DISPLAY=$display VK_ICD_FILENAMES=$lvp VK_DRIVER_FILES=$lvp RUST_LOG=info \
        exec timeout 120 "$ruffle/bin/ruffle" -g vulkan -p low --storage memory --scale no-scale \
        --width 800 --height 480 -Pmovie=hud.swf harness.swf
) >"$log" 2>&1 &
ruffle_pid=$!

wait_for() { for _ in $(seq 600); do grep -q "$1" "$log" && return 0; sleep 0.1; done; return 1; }
wait_for 'KITS target' || { echo "the harness did not get to the click: see $log" >&2; }
# the one window on this display: Ruffle's
window=$(DISPLAY=$display timeout 20 "$xdotool" search --sync --onlyvisible --name "." | head -1 || true)
read -r tx ty < <(sed -n 's/.*KITS target kit 3 at \([0-9]*\),\([0-9]*\).*/\1 \2/p' "$log")
# where the window's (400, 300) lands on the stage: the movie's offset in the window (a menu bar)
DISPLAY=$display "$xdotool" mousemove --window "$window" 400 300 click 1
wait_for 'KITS mouse down' || true
read -r sx sy < <(sed -n 's/.*KITS mouse down at \([0-9]*\),\([0-9]*\).*/\1 \2/p' "$log" | head -1)
DISPLAY=$display "$xdotool" mousemove --window "$window" $((tx + 400 - sx)) $((ty + 300 - sy)) click 1
wait_for 'KITS RESULT' || true
cleanup
if ! grep -q 'graphics API vulkan on llvmpipe' "$log"; then
    echo "Ruffle did not run on lavapipe (CPU): see $log" >&2
fi
grep -o 'KITS .*' "$log" || true
grep -q 'KITS RESULT pass' "$log"
