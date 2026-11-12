#!/usr/bin/env bash
# check.sh <lobby_menu.swf> <hud.swf> [work dir]: free for all's two player lists and its top bar
# (docs/notes/ffa.md) in Ruffle, on a private CPU-only virtual display (lavapipe, never the GPU):
# - FfaResultsHarness.as: the lobby's match results, one ranking over both columns for 1, 3, 4 and
#   10 players (the reader in either half), by kills, then score, fewer deaths, the server's order;
#   no team names or score; Team Deathmatch as before; Player vs Bots' mode and map menus with the
#   server's free for all rows;
# - FfaTabHarness.as: the hud's Tab list, the same ranking from the exe's rows (the local player the
#   ally, every other an enemy), through an update, a player gone, "Hide player scores" and Team
#   Deathmatch after it; plain names, left-first overflow and reordering across columns; a timer-only
#   top bar, including score updates, timeline initialization and transitions to another mode.
# Screenshots into the work dir: results-tdm.png, results-ffa.png, topbar-ffa.png, tab-ffa.png.
# Exits 0 when both say pass; prints their lines. A work dir given is kept (the harnesses, Ruffle's
# logs). Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or
# SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
hud=$(readlink -f "$2")
. "$here/../ruffle_harness.sh"
harness_work "${3:-}"

# The texts the exe gives the lobby movie (set_localization_data), from the dictionaries.
"${PYTHON:-python3}" "$here/../localization-check/harness_text.py" "$work" english
for harness in FfaResultsHarness FfaTabHarness; do
    harness_compile "$here/$harness.as" 1280,720 -source-path+="$work"
done
# The native Scaleform placements Ruffle refuses, so the results screen's named children exist, and
# the top bar's (its second team's last players, placed hidden).
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
"${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$hud" "$work/hud.swf"
harness_display 1280x720 ffmpeg-full

# run <harness>: Ruffle on the harness in the background, its log <harness>.log
run() { harness_run "$1.log" -p high --no-gui --width 1280 --height 720 "$1.swf"; }
shot() { ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -video_size 1280x720 -i "$display" -frames:v 1 "$work/$1"; }

run FfaResultsHarness
wait_for FfaResultsHarness.log 'RESULTS SHOT tdm\|RESULTS RESULT' && shot results-tdm.png
wait_for FfaResultsHarness.log 'RESULTS SHOT ffa\|RESULTS RESULT' && shot results-ffa.png
wait_for FfaResultsHarness.log 'RESULTS SHOT overflow\|RESULTS RESULT' && shot results-ffa-overflow.png
wait_for FfaResultsHarness.log 'RESULTS RESULT' || true
kill "$ruffle_pid" 2>/dev/null || true
run FfaTabHarness
wait_for FfaTabHarness.log 'TAB SHOT topbar\|TAB RESULT' && shot topbar-ffa.png
wait_for FfaTabHarness.log 'TAB SHOT ffa\|TAB RESULT' && shot tab-ffa.png
wait_for FfaTabHarness.log 'TAB SHOT overflow\|TAB RESULT' && shot tab-ffa-overflow.png
wait_for FfaTabHarness.log 'TAB RESULT' || true
harness_stop

harness_report 'RESULTS .*\|TAB .*' FfaResultsHarness.log FfaTabHarness.log
! grep -q "RESULT fail" "$work/FfaResultsHarness.log" "$work/FfaTabHarness.log" \
    && grep -q "RESULTS RESULT pass" "$work/FfaResultsHarness.log" && grep -q "TAB RESULT pass" "$work/FfaTabHarness.log"
