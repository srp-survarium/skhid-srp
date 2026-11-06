#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: rooms and player actions
# (docs/notes/rooms.md) in Ruffle (CPU only: lavapipe on its own virtual X display, never the GPU).
# RoomsHarness.as makes the lobby movie's own rooms window and tells it room states: Start and the
# move buttons (» «) for a private room's owner only, never for a member, an open room or a room
# playing its match; its two team columns, a member in the other after a move; no "N v N" and no
# max level for a private room (its info line, its list row, the facts beside the list); the create
# window's sliders hidden once a PIN is typed, Max level too; squads' icons (docs/notes/squads.md)
# after their names in the room and the list's side panel, ours green, the names not moved; a
# squad soldier's greyed Join, New room and "IN SQUAD" Play. Then in
# every language (the dictionaries' texts): the list (a room's queue ours too), a room in each phase
# and hint (with squads too), the create window and the preset offer's buttons, no text or button
# label cut by its field and no two on one line meeting (a "ROOMS OVERLAP" line each); and the top
# bar's Play as the rooms' Leave (LEAVE ROOM, LEAVE QUEUE) and IN SQUAD, whole. A second run
# checks the player menu (friend, Mute chat and its choices, message, squad) and the server-pushed
# friend and ignore lists, on a copy adapted by medical-cards/ruffle_movie.py.
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

# The texts the exe gives the movie (set_localization_data), from the dictionaries.
"${PYTHON:-python3}" "$here/../localization-check/harness_text.py" "$work"

java=$(sed -n 's/.*"java": *"\([^"]*\)".*/\1/p' "$tools")
flex=$(sed -n 's/.*"flex": *"\([^"]*\)".*/\1/p' "$tools")
"$java" -Xmx1g -Djava.awt.headless=true -jar "$flex/lib/mxmlc.jar" \
    +flexlib="$flex/frameworks" +playerglobalHome="$flex/frameworks/libs/player" -library-path= \
    -external-library-path="$flex/frameworks/libs/player/32.0/playerglobal.swc" -source-path="$here" -source-path+="$work" \
    -omit-trace-statements=false -default-frame-rate=60 -default-size=1280,720 \
    -output="$work/RoomsHarness.swf" "$here/RoomsHarness.as" >"$work/RoomsHarness.mxmlc.log" 2>&1 \
    || { cat "$work/RoomsHarness.mxmlc.log"; exit 1; }

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

for players in 0 1; do
if [[ $players == 0 ]]; then
    cp "$lobby" "$work/lobby_menu.swf"
else
    # Friends/context-menu symbols use native Scaleform placements that Ruffle
    # rejects. Adapt only the test copy, retaining the production ActionScript.
    "${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
fi
log=$work/RoomsHarness-$players.log
(
    cd "$work"
    unset WAYLAND_DISPLAY XDG_SESSION_TYPE
    DISPLAY=$display VK_ICD_FILENAMES=$lvp VK_DRIVER_FILES=$lvp RUST_LOG=info \
        exec timeout 180 "$ruffle/bin/ruffle" -g vulkan -p low --storage memory --scale no-scale \
        --width 1280 --height 720 -Pmovie=lobby_menu.swf -Pplayers="$players" RoomsHarness.swf
) >"$log" 2>&1 &
ruffle_pid=$!
for _ in $(seq 1800); do grep -q 'ROOMS RESULT' "$log" && break; sleep 0.1; done
kill "$ruffle_pid" 2>/dev/null || true
wait "$ruffle_pid" 2>/dev/null || true
ruffle_pid=

if ! grep -q 'graphics API vulkan on llvmpipe' "$log"; then
    echo "Ruffle did not run on lavapipe (CPU): see $log" >&2
fi
grep -ho 'ROOMS .*' "$log" || true
grep -q 'ROOMS RESULT pass' "$log"
done
