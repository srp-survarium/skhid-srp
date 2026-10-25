#!/usr/bin/env bash
# check.sh <hud.swf> [work dir]: the hud movie's kill feed (SrvCombatLogList) fed every kind of row
# the exe sends, in Ruffle (CPU only: lavapipe on a virtual X display, never the GPU), by
# KillFeedHarness.as. Exits 0 when every row is added and the feed keeps a later row; prints the
# harness's lines. A work dir given is kept (the harness, Ruffle's log). Java and Flex from
# private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
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
    -omit-trace-statements=false -default-frame-rate=60 -default-size=400,400 \
    -output="$work/harness.swf" "$here/KillFeedHarness.as" >"$work/mxmlc.log" 2>&1 || { cat "$work/mxmlc.log"; exit 1; }
cp "$movie" "$work/hud.swf"

paths=$(nix build nixpkgs#ruffle nixpkgs#mesa nixpkgs#xvfb-run --no-link --print-out-paths)
ruffle=$(grep -- '-ruffle-' <<<"$paths")
mesa=$(grep -- '-mesa-' <<<"$paths")
xvfb=$(grep -- '-xvfb-run-' <<<"$paths")
lvp=$mesa/share/vulkan/icd.d/lvp_icd.x86_64.json
(
    cd "$work"
    unset WAYLAND_DISPLAY XDG_SESSION_TYPE
    VK_ICD_FILENAMES=$lvp VK_DRIVER_FILES=$lvp RUST_LOG=info timeout 180 \
        "$xvfb/bin/xvfb-run" -a -s "-screen 0 640x480x24" \
        "$ruffle/bin/ruffle" -g vulkan -p low --storage memory --width 400 --height 400 -Pmovie=hud.swf harness.swf \
        >"$work/ruffle.log" 2>&1 || true
)
if ! grep -q 'graphics API vulkan on llvmpipe' "$work/ruffle.log"; then
    echo "Ruffle did not run on lavapipe (CPU): see $work/ruffle.log" >&2
fi
grep -o 'KF .*' "$work/ruffle.log" || true
grep -q 'KF RESULT pass' "$work/ruffle.log"
