#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: the match results stay up while the rooms window changes
# under them (docs/notes/rooms.md, "The results stay until the player closes them"), in Ruffle
# (CPU only: lavapipe on its own virtual X display, never the GPU). ResultsHarness.as makes the
# lobby movie's own top menu, results screen and rooms window, opens the results with
# LobbyMenu.show_victory_screen and tells the window what the server answers after a match: let in
# from a room's queue, the room's next vote, out of the room. The results stay open, the top menu's
# tab as it was; closing them shows the room's window, its tab chosen.
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
"${PYTHON:-python3}" "$here/../localization-check/harness_text.py" "$work" english

java=$(sed -n 's/.*"java": *"\([^"]*\)".*/\1/p' "$tools")
flex=$(sed -n 's/.*"flex": *"\([^"]*\)".*/\1/p' "$tools")
"$java" -Xmx1g -Djava.awt.headless=true -jar "$flex/lib/mxmlc.jar" \
    +flexlib="$flex/frameworks" +playerglobalHome="$flex/frameworks/libs/player" -library-path= \
    -external-library-path="$flex/frameworks/libs/player/32.0/playerglobal.swc" -source-path="$here" -source-path+="$work" \
    -omit-trace-statements=false -default-frame-rate=60 -default-size=1280,720 \
    -output="$work/ResultsHarness.swf" "$here/ResultsHarness.as" >"$work/ResultsHarness.mxmlc.log" 2>&1 \
    || { cat "$work/ResultsHarness.mxmlc.log"; exit 1; }
# The native Scaleform placements Ruffle refuses, so the results screen's named children exist.
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
        --width 1280 --height 720 ResultsHarness.swf
) >"$work/ResultsHarness.log" 2>&1 &
ruffle_pid=$!
for _ in $(seq 1200); do grep -q 'RESULTS RESULT' "$work/ResultsHarness.log" && break; sleep 0.1; done
cleanup

if ! grep -q 'graphics API vulkan on llvmpipe' "$work/ResultsHarness.log"; then
    echo "Ruffle did not run on lavapipe (CPU): see $work/ResultsHarness.log" >&2
fi
grep -ho 'RESULTS .*' "$work/ResultsHarness.log" || true
grep -q 'RESULTS RESULT pass' "$work/ResultsHarness.log"
