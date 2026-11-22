#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: the perks' slots in the kit (docs/notes/artefact-skills.md,
# "The slot a pocket again, by kit") in Ruffle (CPU only: lavapipe on its own virtual X display,
# never the GPU). PerkSlotsHarness.as makes the lobby movie's own player panel, tells it srp's `K`
# row of the presets list and the kit's pockets as the exe fills them, and clicks the slots:
# Artifact Container's container and pocket, Deep Pockets' or Supply Pocket's pocket and second
# weapon slot with their red locks, the hints, the clicks' asks, one at a time, nothing moving while
# srp answers, a slot's item kept; every language's hints. Then the looks before and after:
# zoom.png (Artifact Container's slot at three times) and weapon.png (the weapon perk's slots at
# their size) into the work dir. The slot labels are the stock English. Exits 0 when it says pass;
# prints its lines.
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"
"${PYTHON:-python3}" "$here/../localization-check/harness_text.py" "$work"
harness_compile "$here/PerkSlotsHarness.as" 2100,1450 -source-path+="$work"
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
cp "$here/../../client-patches/localization/english.tsv" "$work/"
harness_display 2100x1450 imagemagick
HARNESS_TIMEOUT=240 harness_run ruffle.log -p low --width 2100 --height 1450 PerkSlotsHarness.swf
for shot in zoom weapon; do
    wait_for ruffle.log "PERK shot $shot" 900 || break
    sleep 1
    DISPLAY=$display import -window root "$work/$shot.png" 2>/dev/null || true
done
wait_for ruffle.log 'PERK RESULT' 600 || true
harness_stop
harness_report 'PERK .*' ruffle.log
grep -q 'PERK RESULT pass' "$work/ruffle.log"
