#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: a private room's Start and teams (docs/notes/rooms.md) in
# Ruffle (CPU only: lavapipe on its own virtual X display, never the GPU). RoomsHarness.as makes the
# lobby movie's own rooms window and tells it room states: Start and the move buttons (» «) for a
# private room's owner only, never for a member, an open room or a room playing its match; its two
# team columns, a member in the other after a move; no "N v N" for a private room; the room's bots
# a team on its info line, and New room's "Bots per team" slider (open and private).
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
    -output="$work/RoomsHarness.swf" "$here/RoomsHarness.as" >"$work/RoomsHarness.mxmlc.log" 2>&1 \
    || { cat "$work/RoomsHarness.mxmlc.log"; exit 1; }
cp "$lobby" "$work/lobby_menu.swf"

paths=$(nix build nixpkgs#ruffle nixpkgs#mesa nixpkgs#xorg.xorgserver --no-link --print-out-paths)
ruffle=$(grep -- '-ruffle-' <<<"$paths")
mesa=$(grep -- '-mesa-' <<<"$paths")
xvfb=$(grep -- '-xorg-server-' <<<"$paths")/bin/Xvfb
lvp=$mesa/share/vulkan/icd.d/lvp_icd.x86_64.json
display=:$((90 + RANDOM % 9))
"$xvfb" "$display" -screen 0 1280x720x24 -nolisten tcp >"$work/xvfb.log" 2>&1 &
xvfb_pid=$!
ruffle_pid=
cleanup() { kill $ruffle_pid "$xvfb_pid" 2>/dev/null || true; }
trap 'cleanup; [[ $# -ge 2 ]] || rm -rf "$work"' EXIT
sleep 1

(
    cd "$work"
    unset WAYLAND_DISPLAY XDG_SESSION_TYPE
    DISPLAY=$display VK_ICD_FILENAMES=$lvp VK_DRIVER_FILES=$lvp RUST_LOG=info \
        exec timeout 180 "$ruffle/bin/ruffle" -g vulkan -p low --storage memory --scale no-scale \
        --width 1280 --height 720 -Pmovie=lobby_menu.swf RoomsHarness.swf
) >"$work/RoomsHarness.log" 2>&1 &
ruffle_pid=$!
for _ in $(seq 1800); do grep -q 'ROOMS RESULT' "$work/RoomsHarness.log" && break; sleep 0.1; done
cleanup

if ! grep -q 'graphics API vulkan on llvmpipe' "$work/RoomsHarness.log"; then
    echo "Ruffle did not run on lavapipe (CPU): see $work/RoomsHarness.log" >&2
fi
grep -ho 'ROOMS .*' "$work/RoomsHarness.log" || true
grep -q 'ROOMS RESULT pass' "$work/RoomsHarness.log"
