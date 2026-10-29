#!/usr/bin/env bash
# check.sh <rebuilt lobby_menu.swf> [work dir]: Language detection, menu text and
# embedded Latin and Cyrillic glyphs in Ruffle, on a private CPU-only virtual display.
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
    -output="$work/LocalizationHarness.swf" "$here/LocalizationHarness.as" >"$work/LocalizationHarness.mxmlc.log" 2>&1 \
    || { cat "$work/LocalizationHarness.mxmlc.log"; exit 1; }
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
        --width 1280 --height 720 -Pmovie=lobby_menu.swf LocalizationHarness.swf
) >"$work/LocalizationHarness.log" 2>&1 &
ruffle_pid=$!
for _ in $(seq 1800); do grep -q 'LOCALIZATION RESULT' "$work/LocalizationHarness.log" && break; sleep 0.1; done
cleanup

if ! grep -q 'graphics API vulkan on llvmpipe' "$work/LocalizationHarness.log"; then
    echo "Ruffle did not run on lavapipe (CPU): see $work/LocalizationHarness.log" >&2
fi
grep -ho 'LOCALIZATION .*' "$work/LocalizationHarness.log" || true
grep -q 'LOCALIZATION RESULT pass' "$work/LocalizationHarness.log"
