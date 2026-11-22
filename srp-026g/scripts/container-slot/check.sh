#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: Artifact Container's slot in the kit (docs/notes/artefact-skills.md,
# "Artifact Container (522)") in Ruffle (CPU only: lavapipe on its own virtual X display, never the
# GPU). ContainerSlotHarness.as makes the lobby movie's own player panel, tells it srp's `K` row of the
# presets list and the kit's pockets as the exe fills them, and clicks the slot: the yellow outline on
# the first container and on the last pocket, the hint, the click's ask, one at a time, a pocket's item
# kept, no mark without the skill; every language's hints. Captures container.png and pocket.png into
# the work dir. Exits 0 when it says pass; prints its lines.
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
"${PYTHON:-python3}" "$here/../localization-check/harness_text.py" "$work"
harness_compile "$here/ContainerSlotHarness.as" 900,700 -source-path+="$work"
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
harness_display 900x700 imagemagick
HARNESS_TIMEOUT=120 harness_run ruffle.log -p low --width 900 --height 700 ContainerSlotHarness.swf
for shot in container pocket; do
    wait_for ruffle.log "CONTAINER shot $shot" 900 || break
    sleep 1
    DISPLAY=$display import -window root "$work/$shot.png" 2>/dev/null || true
done
wait_for ruffle.log 'CONTAINER RESULT' 600 || true
harness_stop
harness_report 'CONTAINER .*' ruffle.log
grep -q 'CONTAINER RESULT pass' "$work/ruffle.log"
