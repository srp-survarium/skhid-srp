#!/usr/bin/env bash
# shots.sh <lobby_menu.swf> <out dir> [<width>x<height> ...]: screenshots of the Watch window (and a
# History row) as the lobby movie draws them (WatchShots.as), in Ruffle (CPU only: lavapipe on its own
# virtual X display, never the GPU), at the game's resolutions (default 1280x720 and 1920x1080), in
# English, fed WatchFake's rows. One PNG a view, <out>/<w>x<h>-<view>.png, at most 1280 px wide.
# Java and Flex from private/actionscript/tools.json, or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
out=$(readlink -f "$2")
shift 2
sizes=("$@")
[[ ${#sizes[@]} -gt 0 ]] || sizes=(1280x720 1920x1080)
mkdir -p "$out"
. "$here/../ruffle_harness.sh"
harness_work ""

"${PYTHON:-python3}" "$here/../localization-check/harness_text.py" "$work" english
harness_compile "$here/WatchShots.as" 1280,720 -source-path+="$work" -default-background-color=0x101418
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
views=(live roster recent recent-answered mine player empty history)
for size in "${sizes[@]}"; do
    w=${size%x*}
    h=${size#*x}
    harness_display "${w}x$((h + 200))" xorg.xwd imagemagick
    for view in "${views[@]}"; do
        log=$size-$view.log
        HARNESS_TIMEOUT=120 harness_run "$log" -p low --width "$w" --height "$h" -Pview="$view" WatchShots.swf
        wait_for "$log" 'SHOT ' 900 || true
        sleep 1
        if grep -q 'SHOT ready' "$work/$log"; then
            DISPLAY=$display xwd -root -silent >"$work/shot.xwd"
            # The stage only (under Ruffle's menu bar, 22 px), at most 1280 wide.
            magick "$work/shot.xwd" -crop "${w}x${h}+0+22" +repage -resize '1280x>' "$out/$size-$view.png"
            echo "$out/$size-$view.png: $(grep -o 'SHOT ready.*' "$work/$log")"
        else
            echo "$size $view: no shot: $(grep -o 'SHOT fail.*' "$work/$log" || echo "see $work/$log")" >&2
        fi
        kill "$ruffle_pid" 2>/dev/null || true
        wait "$ruffle_pid" 2>/dev/null || true
    done
    harness_stop
done
