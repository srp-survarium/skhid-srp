#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir] [atlas.png]: the player panel's Face button and the faces'
# window (docs/notes/face-choice.md) in Ruffle (CPU only: lavapipe on its own virtual X display,
# never the GPU). FacePickerHarness.as makes the lobby movie's own player panel and window, feeds
# srp's answers as the ranking list gives them and presses the cards: the button hidden until srp
# says and with one face, the lobby button's size and look left of it; every language's word fitting
# (the button, the window's title, the cards' names); the window of the lobby window's kind
# (CardWindow), in the middle, a modal (Esc and the toggle close it); a card a face in rows of six
# and five, srp's framed gold; a press (on the card or routed by the window) choosing at once and
# closing, the last pick kept while one choice is asked, a refusal back to srp's face, fewer faces;
# the equipment atlas's cells as the portraits (atlas.png, the faces pack's LobbyMenu_EquipmentIcons,
# or a stand-in), the number without one. Captures button.png, window.png, icons.png and compare.png
# (beside the lobby window) into the work dir. Exits 0 when it says pass; prints its lines.
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
atlas=${3:+$(readlink -f "$3")}
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
"${PYTHON:-python3}" "$here/../localization-check/harness_text.py" "$work"
harness_compile "$here/FacePickerHarness.as" 1640,540 -source-path+="$work"
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
rm -f "$work/atlas.png"
if [[ -n $atlas ]]; then cp "$atlas" "$work/atlas.png"; fi
harness_display 1640x540 imagemagick
HARNESS_TIMEOUT=120 harness_run ruffle.log -p low --width 1640 --height 540 FacePickerHarness.swf
for shot in button window icons compare; do
    wait_for ruffle.log "FACE shot $shot" 900 || break
    sleep 1
    DISPLAY=$display import -window root "$work/$shot.png" 2>/dev/null || true
done
wait_for ruffle.log 'FACE RESULT' 600 || true
harness_stop
harness_report 'FACE .*' ruffle.log
grep -q 'FACE RESULT pass' "$work/ruffle.log"
