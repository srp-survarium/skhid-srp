#!/usr/bin/env bash
# check.sh <hud.swf> [work dir]: the view fading on the death screen (docs/notes/death-eyes.md) in
# Ruffle (CPU only: lavapipe on its own virtual X display, never the GPU), by EyesHarness.as: the
# openness over a 5 s and a 10 s countdown, the respawn, the edge cases, and a real mouse click
# (xdotool) on a kit over the faded view. Exits 0 when it says pass; prints its lines.
# check.sh --video <out.mp4> [--blood under|over|over-normal] <hud.swf> [backdrop.png] [work dir]:
# records the fading over a 5 s countdown at 1280x720 (1 s alive, the countdown, the respawn, 1.5 s
# alive) over the backdrop, with the hud's blood while dead under or over it if asked.
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
. "$here/../ruffle_harness.sh"
harness_work "${1:-}"
if [[ -n $video ]]; then
    width=1280 height=720
else
    width=800 height=480
fi

harness_compile "$here/EyesHarness.as" $width,$height -default-background-color=0x5A6066
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

harness_display ${width}x${height} xdotool ffmpeg-full
log=$work/EyesHarness.log
HARNESS_TIMEOUT=120 harness_run EyesHarness.log -p high --no-gui --width $width --height $height "${params[@]}" EyesHarness.swf

if [[ -n $video ]]; then
    wait_for EyesHarness.log 'EYES video start' 300 || { echo "the harness did not start: see $log" >&2; exit 1; }
    ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -framerate 30 -video_size ${width}x${height} -i "$display" \
        -t 7.5 -c:v libx264 -pix_fmt yuv420p -crf 18 "$video" || true
    echo "video: $video"
    exit 0
fi

wait_for EyesHarness.log 'EYES openness .*target' 300 || { echo "the harness did not get to the click: see $log" >&2; }
window=$(DISPLAY=$display timeout 20 xdotool search --sync --onlyvisible --name "." | head -1 || true)
read -r tx ty < <(sed -n 's/.*target kit 3 at \([0-9]*\),\([0-9]*\).*/\1 \2/p' "$log")
DISPLAY=$display xdotool mousemove --window "$window" "$tx" "$ty" click 1
wait_for EyesHarness.log 'EYES RESULT' 300 || true
harness_stop

harness_report 'EYES .*' EyesHarness.log
grep -q 'EYES RESULT pass' "$log"
