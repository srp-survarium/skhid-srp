#!/usr/bin/env bash
# check.sh <hud.swf> [work dir]: the eyes closing on the death screen (docs/notes/death-eyes.md) in
# Ruffle (CPU only: lavapipe on its own virtual X display, never the GPU), by EyesHarness.as: the
# openness over a 5 s and a 10 s countdown, the respawn, the edge cases, and a real mouse click
# (xdotool) on a kit over the closing eyes. Exits 0 when it says pass; prints its lines.
# check.sh --video <out.mp4> [--blood under|over|over-normal] <hud.swf> [backdrop.png] [work dir]:
# records the closing over a 5 s countdown at 1280x720 (1 s alive, the countdown, the respawn, 1.5 s
# alive) over the backdrop, with the hud's blood while dead under or over the eyes if asked.
# A work dir given is kept (the harness, Ruffle's log). Java and Flex from
# private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
video='' blood=''
if [[ ${1:-} == --video ]]; then
    video=$(readlink -f "$2")
    shift 2
    if [[ ${1:-} == --blood ]]; then
        blood=$2
        shift 2
    fi
fi
hud=$(readlink -f "$1")
shift
backdrop=''
if [[ -n $video && $# -ge 1 && $1 == *.png ]]; then
    backdrop=$(readlink -f "$1")
    shift
fi
main=$(git -C "$here" worktree list --porcelain | sed -n '1s/^worktree //p')
tools=${SRP_AS_TOOLS:-$main/private/actionscript/tools.json}
if [[ $# -ge 1 ]]; then
    work=$(readlink -f "$1")
    mkdir -p "$work"
    keep=1
else
    work=$(mktemp -d)
    keep=''
fi
if [[ -n $video ]]; then
    width=1280 height=720
else
    width=800 height=480
fi

java=$(sed -n 's/.*"java": *"\([^"]*\)".*/\1/p' "$tools")
flex=$(sed -n 's/.*"flex": *"\([^"]*\)".*/\1/p' "$tools")
"$java" -Xmx1g -Djava.awt.headless=true -jar "$flex/lib/mxmlc.jar" \
    +flexlib="$flex/frameworks" +playerglobalHome="$flex/frameworks/libs/player" -library-path= \
    -external-library-path="$flex/frameworks/libs/player/32.0/playerglobal.swc" -source-path="$here" \
    -omit-trace-statements=false -default-frame-rate=60 -default-size=$width,$height \
    -default-background-color=0x5A6066 -output="$work/EyesHarness.swf" "$here/EyesHarness.as" \
    >"$work/mxmlc.log" 2>&1 || { cat "$work/mxmlc.log"; exit 1; }
cp "$hud" "$work/hud.swf"
params=(-Pmovie=hud.swf)
if [[ -n $video ]]; then
    params+=(-Pmode=video)
    [[ -z $blood ]] || params+=(-Pblood="$blood")
    if [[ -n $backdrop ]]; then
        cp "$backdrop" "$work/backdrop.png"
        params+=(-Pbackdrop=backdrop.png)
    fi
fi

paths=$(nix build nixpkgs#ruffle nixpkgs#mesa nixpkgs#xdotool nixpkgs#xorg.xorgserver nixpkgs#ffmpeg-full --no-link --print-out-paths)
ruffle=$(grep -- '-ruffle-' <<<"$paths")
mesa=$(grep -- '-mesa-' <<<"$paths")
xdotool=$(grep -- '-xdotool-' <<<"$paths")/bin/xdotool
xvfb=$(grep -- '-xorg-server-' <<<"$paths")/bin/Xvfb
ffmpeg=$(grep -- '-ffmpeg-full-.*-bin$' <<<"$paths")/bin/ffmpeg
lvp=$mesa/share/vulkan/icd.d/lvp_icd.x86_64.json
display=:$((90 + RANDOM % 9))
"$xvfb" "$display" -screen 0 ${width}x${height}x24 -nolisten tcp >"$work/xvfb.log" 2>&1 &
xvfb_pid=$!
ruffle_pid='' ffmpeg_pid=''
cleanup() { kill $ruffle_pid $ffmpeg_pid "$xvfb_pid" 2>/dev/null || true; }
trap 'cleanup; [[ -n $keep ]] || rm -rf "$work"' EXIT
sleep 1

log=$work/EyesHarness.log
(
    cd "$work"
    unset WAYLAND_DISPLAY XDG_SESSION_TYPE
    DISPLAY=$display VK_ICD_FILENAMES=$lvp VK_DRIVER_FILES=$lvp RUST_LOG=info \
        exec timeout 120 "$ruffle/bin/ruffle" -g vulkan -p high --storage memory --scale no-scale --no-gui \
        --width $width --height $height "${params[@]}" EyesHarness.swf
) >"$log" 2>&1 &
ruffle_pid=$!
wait_for() { for _ in $(seq 600); do grep -q "$1" "$log" && return 0; sleep 0.05; done; return 1; }

if [[ -n $video ]]; then
    wait_for 'EYES video start' || { echo "the harness did not start: see $log" >&2; exit 1; }
    "$ffmpeg" -loglevel error -y -f x11grab -draw_mouse 0 -framerate 30 -video_size ${width}x${height} -i "$display" \
        -t 7.5 -c:v libx264 -pix_fmt yuv420p -crf 18 "$video" &
    ffmpeg_pid=$!
    wait "$ffmpeg_pid" || true
    ffmpeg_pid=''
    echo "video: $video"
    exit 0
fi

wait_for 'EYES openness .*target' || { echo "the harness did not get to the click: see $log" >&2; }
window=$(DISPLAY=$display timeout 20 "$xdotool" search --sync --onlyvisible --name "." | head -1 || true)
read -r tx ty < <(sed -n 's/.*target kit 3 at \([0-9]*\),\([0-9]*\).*/\1 \2/p' "$log")
DISPLAY=$display "$xdotool" mousemove --window "$window" "$tx" "$ty" click 1
wait_for 'EYES RESULT' || true
cleanup

if ! grep -q 'graphics API vulkan on llvmpipe' "$log"; then
    echo "Ruffle did not run on lavapipe (CPU): see $log" >&2
fi
grep -o 'EYES .*' "$log" || true
grep -q 'EYES RESULT pass' "$log"
