#!/usr/bin/env bash
# check.sh <main_menu.swf> [work dir] [WxH...]: Settings > Controls > Bindings (SrpKeysWindow) in Ruffle, on a
# private CPU-only virtual display: KeysWindowHarness.as with the rows the exe gives (rows.py: hide_ui_026g.asm's
# `controls`, their default keys, English labels), the key binder's table (the original exe's, at 0xA74FA0;
# $SKHID_ORIGINAL_EXE, else the main checkout's private/function-map/inputs/026g/survarium.exe) and every
# dictionary's texts, at each size (default 1920x1080 and 1280x720). Screenshots into the work dir: keys-<size>-<shot>.png. Exits 0 when
# the harness says pass at every size. No game, no server.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
menu=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
# Ruffle draws the window's 61 rows slowly on the CPU renderer
export HARNESS_TIMEOUT=${HARNESS_TIMEOUT:-600}
sizes=("${@:3}")
[[ ${#sizes[@]} -gt 0 ]] || sizes=(1920x1080 1280x720)

# the original exe: the key binder's table (rows.py binder)
main=$(git -C "$here" worktree list --porcelain | sed -n '1s/^worktree //p')
exe=${SKHID_ORIGINAL_EXE:-$main/private/function-map/inputs/026g/survarium.exe}
for part in rows names binder texts; do
    declare "$part=$("${PYTHON:-python3}" "$here/rows.py" "$here/../../client-patches" "$exe" "$part")"
done

cp "$menu" "$work/main_menu.swf"
status=0
for size in "${sizes[@]}"; do
    w=${size%x*} h=${size#*x}
    harness_compile "$here/KeysWindowHarness.as" "$w,$h"
    harness_display "$size" ffmpeg-full
    harness_run "keys-$size.log" -p high --no-gui --dummy-external-interface --width "$w" --height "$h" \
        "-Prows=$rows" "-Pnames=$names" "-Pbinder=$binder" "-Ptexts=$texts" KeysWindowHarness.swf
    while wait_for "keys-$size.log" 'KEYS SHOT [a-z-]*$\|KEYS END' 900; do
        shot=$(grep -o 'KEYS SHOT [a-z-]*$' "$work/keys-$size.log" | sed -n '$p' | cut -d' ' -f3)
        if [[ -n $shot && ! -f $work/keys-$size-$shot.png ]]; then
            ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size "$size" -i "$display" -frames:v 1 "$work/keys-$size-$shot.png"
            echo "KEYS SHOT $shot taken" >>"$work/keys-$size.shots"
        fi
        grep -q 'KEYS END' "$work/keys-$size.log" && break
        sleep 0.2
    done
    harness_stop
    harness_report 'KEYS .*' "keys-$size.log"
    grep -q "KEYS RESULT pass" "$work/keys-$size.log" && ! grep -q "KEYS RESULT fail" "$work/keys-$size.log" || status=1
done
exit $status
