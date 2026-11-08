#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: the Character panel's face picker (docs/notes/face-choice.md)
# in Ruffle (CPU only: lavapipe on its own virtual X display, never the GPU). FacePickerHarness.as
# makes the lobby movie's own panel, feeds srp's answers as the ranking list gives them and presses
# the picker: hidden until srp says and with one face; its place beside the helmet's slot, under the
# ammo window; a pick shown at once, the last pick kept while one choice is asked, a refusal back to
# srp's face, the round, fewer faces; every language's word fitting. Captures english.png and
# hungarian.png into the work dir. Exits 0 when it says pass; prints its lines.
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
"${PYTHON:-python3}" "$here/../localization-check/harness_text.py" "$work"
harness_compile "$here/FacePickerHarness.as" 1000,940 -source-path+="$work"
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
harness_display 1000x940 imagemagick
HARNESS_TIMEOUT=90 harness_run ruffle.log -p low --width 1000 --height 940 FacePickerHarness.swf
for shot in english hungarian; do
    wait_for ruffle.log "FACE shot $shot" 600 || break
    sleep 1
    DISPLAY=$display import -window root "$work/$shot.png" 2>/dev/null || true
done
wait_for ruffle.log 'FACE RESULT' 600 || true
harness_stop
harness_report 'FACE .*' ruffle.log
grep -q 'FACE RESULT pass' "$work/ruffle.log"
