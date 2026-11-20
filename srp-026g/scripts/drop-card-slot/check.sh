#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: the results screen's drop card slot (docs/notes/drop_cards.md,
# "The slot"), in Ruffle (CPU only: lavapipe on its own virtual X display, never the GPU).
# CardHarness.as makes the lobby movie's own results screen and tells it what a match, a click,
# the buy dialog and the lobby give: the slot opens on any match that took no card, a click with
# none buys one and uses it as it comes, a reopened report shows the card's item.
# Exits 0 when it says pass; prints its lines. A work dir given is kept (the harness, Ruffle's log).
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"

# The texts the exe gives the movie (set_localization_data), from the dictionaries.
"${PYTHON:-python3}" "$here/../localization-check/harness_text.py" "$work" english
harness_compile "$here/CardHarness.as" 1280,720 -source-path+="$work"
# The native Scaleform placements Ruffle refuses, so the results screen's named children exist.
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
harness_display 1280x720
HARNESS_TIMEOUT=120 harness_run CardHarness.log -p low --width 1280 --height 720 CardHarness.swf
wait_for CardHarness.log 'CARD RESULT' 1200 || true
harness_stop
harness_report 'CARD .*' CardHarness.log
grep -q 'CARD RESULT pass' "$work/CardHarness.log"
