#!/usr/bin/env bash
# check.sh <hud.swf> <player_icons.swf> [work dir]: Blackout's Tab view (docs/notes/blackout.md) in
# Ruffle (CPU only: lavapipe on its own virtual X display, never the GPU):
# - KitsHarness.as: the hud's own kit panel and body doll kept by its own KeptLayer beside the hidden
#   root, then a kit key and a real mouse click (xdotool) pick kits there;
# - IconsHarness.as: the player icons movie itself told the forced state: the teammates' markers
#   always, an enemy's never, the objects' not while forced.
# Exits 0 when both say pass; prints their lines. A work dir given is kept (the harnesses, Ruffle's
# logs). Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or
# SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
hud=$(readlink -f "$1")
icons=$(readlink -f "$2")
main=$(git -C "$here" worktree list --porcelain | sed -n '1s/^worktree //p')
tools=${SRP_AS_TOOLS:-$main/private/actionscript/tools.json}
if [[ $# -ge 3 ]]; then
    work=$(readlink -f "$3")
    mkdir -p "$work"
else
    work=$(mktemp -d)
fi

java=$(sed -n 's/.*"java": *"\([^"]*\)".*/\1/p' "$tools")
flex=$(sed -n 's/.*"flex": *"\([^"]*\)".*/\1/p' "$tools")
for harness in KitsHarness IconsHarness; do
    "$java" -Xmx1g -Djava.awt.headless=true -jar "$flex/lib/mxmlc.jar" \
        +flexlib="$flex/frameworks" +playerglobalHome="$flex/frameworks/libs/player" -library-path= \
        -external-library-path="$flex/frameworks/libs/player/32.0/playerglobal.swc" -source-path="$here" \
        -omit-trace-statements=false -default-frame-rate=60 -default-size=800,480 \
        -output="$work/$harness.swf" "$here/$harness.as" >"$work/$harness.mxmlc.log" 2>&1 \
        || { cat "$work/$harness.mxmlc.log"; exit 1; }
done
cp "$hud" "$work/hud.swf"
cp "$icons" "$work/player_icons.swf"

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
trap 'cleanup; [[ $# -ge 3 ]] || rm -rf "$work"' EXIT
sleep 1

# run <harness> <movie>: Ruffle on the harness in the background, its log <harness>.log
run() {
    (
        cd "$work"
        unset WAYLAND_DISPLAY XDG_SESSION_TYPE
        DISPLAY=$display VK_ICD_FILENAMES=$lvp VK_DRIVER_FILES=$lvp RUST_LOG=info \
            exec timeout 120 "$ruffle/bin/ruffle" -g vulkan -p low --storage memory --scale no-scale \
            --width 800 --height 480 -Pmovie="$2" "$1.swf"
    ) >"$work/$1.log" 2>&1 &
    ruffle_pid=$!
}
wait_for() { for _ in $(seq 600); do grep -q "$2" "$work/$1.log" && return 0; sleep 0.1; done; return 1; }

run KitsHarness hud.swf
log=$work/KitsHarness.log
wait_for KitsHarness 'KITS target' || { echo "the kits harness did not get to the click: see $log" >&2; }
# the one window on this display: Ruffle's
window=$(DISPLAY=$display timeout 20 "$xdotool" search --sync --onlyvisible --name "." | head -1 || true)
read -r tx ty < <(sed -n 's/.*KITS target kit 3 at \([0-9]*\),\([0-9]*\).*/\1 \2/p' "$log")
# where the window's (400, 300) lands on the stage: the movie's offset in the window (a menu bar)
DISPLAY=$display "$xdotool" mousemove --window "$window" 400 300 click 1
wait_for KitsHarness 'KITS mouse down' || true
read -r sx sy < <(sed -n 's/.*KITS mouse down at \([0-9]*\),\([0-9]*\).*/\1 \2/p' "$log" | head -1)
DISPLAY=$display "$xdotool" mousemove --window "$window" $((tx + 400 - sx)) $((ty + 300 - sy)) click 1
wait_for KitsHarness 'KITS RESULT' || true
kill $ruffle_pid 2>/dev/null || true

run IconsHarness player_icons.swf
wait_for IconsHarness 'ICONS RESULT' || true
cleanup

for harness in KitsHarness IconsHarness; do
    if ! grep -q 'graphics API vulkan on llvmpipe' "$work/$harness.log"; then
        echo "Ruffle did not run on lavapipe (CPU): see $work/$harness.log" >&2
    fi
done
grep -ho 'KITS .*\|ICONS .*' "$work/KitsHarness.log" "$work/IconsHarness.log" || true
grep -q 'KITS RESULT pass' "$work/KitsHarness.log" && grep -q 'ICONS RESULT pass' "$work/IconsHarness.log"
