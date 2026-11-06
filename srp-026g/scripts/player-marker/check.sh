#!/usr/bin/env bash
# check.sh <player_icons.swf> [work dir]: partial marker updates in CPU Ruffle.
# Optional RUFFLE_BIN, XVFB_BIN and LVP_ICD select already installed tools.
set -euo pipefail
[[ $# -ge 1 && $# -le 2 ]] || { echo 'usage: check.sh <player_icons.swf> [work dir]' >&2; exit 2; }
here=$(dirname "$(readlink -f "$0")")
movie=$(readlink -f "$1")
main=$(git -C "$here" worktree list --porcelain | sed -n '1s/^worktree //p')
tools=${SRP_AS_TOOLS:-$main/private/actionscript/tools.json}
if [[ $# == 2 ]]; then
    mkdir -p "$2"
    work=$(readlink -f "$2")
else
    work=$(mktemp -d)
fi
xvfb_pid=''
cleanup() {
    if [[ -n $xvfb_pid ]]; then
        kill "$xvfb_pid" 2>/dev/null || true
        wait "$xvfb_pid" 2>/dev/null || true
    fi
    if [[ $# == 1 ]]; then rm -rf -- "$work"; fi
}
trap 'cleanup "$@"' EXIT

java=$(sed -n 's/.*"java": *"\([^"]*\)".*/\1/p' "$tools")
flex=$(sed -n 's/.*"flex": *"\([^"]*\)".*/\1/p' "$tools")
"$java" -Xmx1g -Djava.awt.headless=true -jar "$flex/lib/mxmlc.jar" \
    +flexlib="$flex/frameworks" +playerglobalHome="$flex/frameworks/libs/player" -library-path= \
    -external-library-path="$flex/frameworks/libs/player/32.0/playerglobal.swc" -source-path="$here" \
    -omit-trace-statements=false -default-frame-rate=60 -default-size=800,480 \
    -output="$work/BatteryMarkerHarness.swf" "$here/BatteryMarkerHarness.as" >"$work/build.log" 2>&1 \
    || { cat "$work/build.log"; exit 1; }
cp "$movie" "$work/player_icons.swf"

if [[ -z ${RUFFLE_BIN:-} || -z ${XVFB_BIN:-} || -z ${LVP_ICD:-} ]]; then
    paths=$(nix build nixpkgs#ruffle nixpkgs#mesa nixpkgs#xorg.xorgserver --no-link --print-out-paths)
    ruffle=$(grep -- '-ruffle-' <<<"$paths")
    mesa=$(grep -- '-mesa-' <<<"$paths")
    xvfb=$(grep -- '-xorg-server-' <<<"$paths")
    RUFFLE_BIN=${RUFFLE_BIN:-$ruffle/bin/ruffle}
    XVFB_BIN=${XVFB_BIN:-$xvfb/bin/Xvfb}
    LVP_ICD=${LVP_ICD:-$mesa/share/vulkan/icd.d/lvp_icd.x86_64.json}
fi
: >"$work/display"
"$XVFB_BIN" -displayfd 3 -screen 0 1024x768x24 -nolisten tcp \
    3>"$work/display" >"$work/xvfb.log" 2>&1 &
xvfb_pid=$!
for _ in {1..50}; do
    [[ -s $work/display ]] && break
    kill -0 "$xvfb_pid" 2>/dev/null || { cat "$work/xvfb.log"; exit 1; }
    sleep 0.1
done
[[ -s $work/display ]] || { echo 'Xvfb did not become ready' >&2; exit 1; }
(
    cd "$work"
    unset WAYLAND_DISPLAY XDG_SESSION_TYPE
    DISPLAY=":$(cat display)" VK_ICD_FILENAMES="$LVP_ICD" VK_DRIVER_FILES="$LVP_ICD" RUST_LOG=info \
        timeout 30 "$RUFFLE_BIN" -g vulkan -p low --storage memory --scale no-scale \
        --width 800 --height 480 BatteryMarkerHarness.swf
) >"$work/ruffle.log" 2>&1 || { cat "$work/ruffle.log"; exit 1; }
grep 'BATTERY ' "$work/ruffle.log" || true
grep -q 'graphics API vulkan on llvmpipe' "$work/ruffle.log"
grep -q 'BATTERY RESULT pass' "$work/ruffle.log"
