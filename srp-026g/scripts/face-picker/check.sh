#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: the player panel's Face button and its chooser
# (docs/notes/face-choice.md) in Ruffle (CPU only: lavapipe on its own virtual X display, never the
# GPU). FacePickerHarness.as makes the lobby movie's own player panel, feeds srp's answers as the
# ranking list gives them and clicks the button and the tiles: hidden until srp says and with one
# face; the lobby button's size and look, left of it; every language's word fitting; the chooser
# under it, a transient window (Esc, the button, a press outside close it); a tile a face, srp's
# framed gold; a pick shown at once, the last pick kept while one choice is asked, a refusal back to
# srp's face, fewer faces; the faces pack's atlas cells as the pictures (a stand-in atlas), the
# number without one. Captures button.png, chooser.png and icons.png into the work dir. Exits 0
# when it says pass; prints its lines.
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
"${PYTHON:-python3}" "$here/../localization-check/harness_text.py" "$work"
harness_compile "$here/FacePickerHarness.as" 760,400 -source-path+="$work"
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
harness_display 760x400 imagemagick
HARNESS_TIMEOUT=90 harness_run ruffle.log -p low --width 760 --height 400 FacePickerHarness.swf
for shot in button chooser icons; do
    wait_for ruffle.log "FACE shot $shot" 600 || break
    sleep 1
    DISPLAY=$display import -window root "$work/$shot.png" 2>/dev/null || true
done
wait_for ruffle.log 'FACE RESULT' 600 || true
harness_stop
harness_report 'FACE .*' ruffle.log
grep -q 'FACE RESULT pass' "$work/ruffle.log"
