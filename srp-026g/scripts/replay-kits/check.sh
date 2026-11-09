#!/usr/bin/env bash
# check.sh <hud.swf> <replay_player.swf> [work dir]: a spectator's read-only kit chooser, the hud movie's own
# (`HUD.srp_spectator_*`): its plates are the kits it is given and no other, it is read-only, and its measured
# place overlaps nothing shown in each layout, at 1600x1200 and 1280x720, in Ruffle (CPU only).
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
hud=$(readlink -f "$1")
player=$(readlink -f "$2")
. "$here/../ruffle_harness.sh"
harness_work "${3:-}"
"${PYTHON:-python3}" "$here/../localization-check/harness_text.py" "$work"
harness_compile "$here/KitsHarness.as" 1600,1200 -source-path+="$work" -externs=HUD
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$hud" "$work/hud.swf"
"${PYTHON:-python3}" "$here/hud_root.py" "$work/hud.swf" "$work/KitsHarness.swf" "$work/KitsHarness.swf"
cp "$player" "$work/replay_player.swf"
harness_display 1600x1300   # room for the window's menu bar above a 1200 stage
status=0
for size in 1600x1200 1280x720; do
    log=KitsHarness-$size.log
    HARNESS_TIMEOUT=90 harness_run "$log" -p low --width "${size%x*}" --height "${size#*x}" KitsHarness.swf
    wait_for "$log" 'KITS RESULT\|Error running AVM2 construction' 800 || true
    kill "$ruffle_pid" 2>/dev/null || true
    wait "$ruffle_pid" 2>/dev/null || true
    echo "== $size"
    harness_report 'KITS .*' "$log"
    grep -q 'KITS RESULT pass' "$work/$log" || status=1
done
harness_stop
exit $status
