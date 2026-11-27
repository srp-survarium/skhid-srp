#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: the results screen has no premium (the owner, 2026-11-09:
# "no premium state anywhere should exist"; docs/notes/premium.md), in Ruffle (CPU only: lavapipe on
# its own virtual X display, never the GPU). NoPremiumHarness.as makes the lobby movie's own results
# screen and gives it a match's report with an old server's premium block: no premium row, no
# "Activate premium" button, totals without the premium block, the sums right under the last bonus
# row. Screenshots into the work dir: results.png (with the day's bonus), results-no-daily.png.
# Exits 0 when it says pass; prints its lines. A work dir given is kept (the harness, the shots,
# Ruffle's log). Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md),
# or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"

# The texts the exe gives the movie (set_localization_data), from the dictionaries.
"${PYTHON:-python3}" "$here/../localization-check/harness_text.py" "$work" english
harness_compile "$here/NoPremiumHarness.as" 1280,720 -source-path+="$work"
# The native Scaleform placements Ruffle refuses, so the results screen's named children exist.
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
harness_display 1280x720 imagemagick
HARNESS_TIMEOUT=120 harness_run NoPremiumHarness.log -p low --width 1280 --height 720 NoPremiumHarness.swf
taken=0
while :; do
    for _ in $(seq 1200); do
        (($(grep -c 'NOPREM SHOT ' "$work/NoPremiumHarness.log") > taken)) && break
        grep -q 'NOPREM RESULT' "$work/NoPremiumHarness.log" && break
        sleep 0.1
    done
    name=$(grep -o 'NOPREM SHOT .*' "$work/NoPremiumHarness.log" | sed -n "$((taken + 1))s/^NOPREM SHOT //p")
    [[ -n $name ]] || break
    sleep 0.5
    DISPLAY=$display import -window root "$work/$name.png" 2>/dev/null || true
    taken=$((taken + 1))
done
wait_for NoPremiumHarness.log 'NOPREM RESULT' 1200 || true
harness_stop
harness_report 'NOPREM .*' NoPremiumHarness.log
grep -q 'NOPREM RESULT pass' "$work/NoPremiumHarness.log"
