#!/usr/bin/env bash
# check_ui.sh <lobby_menu.swf> <work directory>: weight numbers and native hover tooltip in CPU Ruffle.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
mkdir -p "$2"
work=$(readlink -f "$2")
harness=$(readlink -f "${3:-$here/WeightHarness.as}")
harness_name=$(basename "$harness" .as)
result_tag=${4:-WEIGHT}
main=$(git -C "$here" worktree list --porcelain | sed -n '1s/^worktree //p')
tools=${SRP_AS_TOOLS:-$main/private/actionscript/tools.json}
java=$(sed -n 's/.*"java": *"\([^"]*\)".*/\1/p' "$tools")
flex=$(sed -n 's/.*"flex": *"\([^"]*\)".*/\1/p' "$tools")
"$java" -Xmx1g -Djava.awt.headless=true -jar "$flex/lib/mxmlc.jar" \
    +flexlib="$flex/frameworks" +playerglobalHome="$flex/frameworks/libs/player" -library-path= \
    -external-library-path="$flex/frameworks/libs/player/32.0/playerglobal.swc" -source-path="$(dirname "$harness")" \
    -omit-trace-statements=false -default-frame-rate=60 -default-size=960,720 \
    -output="$work/$harness_name.swf" "$harness" >"$work/compile.log" 2>&1 \
    || { cat "$work/compile.log"; exit 1; }
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
paths=$(nix build nixpkgs#ruffle nixpkgs#mesa nixpkgs#xorg.xorgserver --no-link --print-out-paths)
ruffle=$(grep -- '-ruffle-' <<<"$paths")
mesa=$(grep -- '-mesa-' <<<"$paths")
xvfb=$(grep -- '-xorg-server-' <<<"$paths")/bin/Xvfb
lvp=$mesa/share/vulkan/icd.d/lvp_icd.x86_64.json
# Xvfb chooses an unused display without touching the owner's desktop.
"$xvfb" -displayfd 3 -screen 0 960x720x24 -nolisten tcp 3>"$work/display" >"$work/xvfb.log" 2>&1 &
xvfb_pid=$!
ruffle_pid=
cleanup() { kill $ruffle_pid "$xvfb_pid" 2>/dev/null || true; }
trap cleanup EXIT
for _ in $(seq 100); do [[ -s $work/display ]] && break; sleep 0.1; done
[[ -s $work/display ]] || { cat "$work/xvfb.log"; exit 1; }
(
    cd "$work"
    unset WAYLAND_DISPLAY XDG_SESSION_TYPE
    DISPLAY=:$(cat display) VK_ICD_FILENAMES=$lvp VK_DRIVER_FILES=$lvp RUST_LOG=info \
        exec timeout 60 "$ruffle/bin/ruffle" -g vulkan -p low --storage memory --scale no-scale \
        --width 960 --height 720 "$harness_name.swf"
) >"$work/ruffle.log" 2>&1 &
ruffle_pid=$!
for _ in $(seq 600); do grep -q "$result_tag RESULT" "$work/ruffle.log" && break; sleep 0.1; done
cleanup
grep -ho "$result_tag RESULT.*" "$work/ruffle.log" || true
grep -q "$result_tag RESULT pass" "$work/ruffle.log"
