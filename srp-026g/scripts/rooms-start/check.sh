#!/usr/bin/env bash
# check.sh <lobby_menu.swf> [work dir]: rooms and player actions
# (docs/notes/rooms.md) in Ruffle (CPU only: lavapipe on its own virtual X display, never the GPU).
# RoomsHarness.as makes the lobby movie's own rooms window and tells it room states: Start and the
# move buttons (» «) for a private room's owner only, never for a member, an open room or a room
# playing its match; its two team columns, a member in the other after a move; no "N v N" and no
# max level for a private room (its info line, its list row, the facts beside the list); the create
# window's sliders hidden once a PIN is typed, Max level too; the last room's PIN pretyped, a press on
# the field putting the caret after it; squads' icons (docs/notes/squads.md)
# after their names in the room and the list's side panel, ours green, the names not moved; a
# squad soldier's greyed Join, New room and "IN SQUAD" Play. Then in
# every language (the dictionaries' texts): the list (a room's queue ours too), a room in each phase
# and hint (with squads too), the create window and the preset offer's buttons, no text or button
# label cut by its field and no two on one line meeting (a "ROOMS OVERLAP" line each); and the top
# bar's Play as the rooms' Leave (LEAVE ROOM, LEAVE QUEUE) and IN SQUAD, whole. A second run
# checks the player menu (friend, Mute chat and its choices, message, squad) and the server-pushed
# friend and ignore lists, on a copy adapted by medical-cards/ruffle_movie.py. Before them, the vote's four
# cards at the game's 1920x1080 and 1280x720, the window laid out as the game lays it out: each
# whole in the window, apart, over the players' panel, its texts whole, in every language.
# Exits 0 when it says pass; prints its lines. A work dir given is kept (the harness, Ruffle's log).
# Java and Flex from private/actionscript/tools.json (tools/actionscript/README.md), or SRP_AS_TOOLS.
set -euo pipefail
here=$(dirname "$(readlink -f "$0")")
lobby=$(readlink -f "$1")
. "$here/../ruffle_harness.sh"
harness_work "${2:-}"

# The texts the exe gives the movie (set_localization_data), from the dictionaries.
"${PYTHON:-python3}" "$here/../localization-check/harness_text.py" "$work"
harness_compile "$here/RoomsHarness.as" 1280,720 -source-path+="$work"
# Big enough for the 1920x1080 run of the cards (Ruffle's window has its menu bar too).
harness_display 1920x1300

# The vote's four cards at the game's resolutions first (RoomsHarness.checkCardsAt): the window
# laid out and scaled as in the game, every card whole, apart and over the players in every language.
cp "$lobby" "$work/lobby_menu.swf"
for size in 1920x1080 1280x720; do
log=RoomsHarness-cards-$size.log
harness_run "$log" -p low --width "${size%x*}" --height "${size#*x}" -Pmovie=lobby_menu.swf -Pcards=1 RoomsHarness.swf
wait_for "$log" 'ROOMS RESULT' || true
kill "$ruffle_pid" 2>/dev/null || true
wait "$ruffle_pid" 2>/dev/null || true
ruffle_pid=
harness_report 'ROOMS .*' "$log"
grep -q 'ROOMS RESULT pass' "$work/$log"
done

for players in 0 1; do
if [[ $players == 0 ]]; then
    cp "$lobby" "$work/lobby_menu.swf"
else
    # Friends/context-menu symbols use native Scaleform placements that Ruffle
    # rejects. Adapt only the test copy, retaining the production ActionScript.
    "${PYTHON:-python3}" "$here/../medical-cards/ruffle_movie.py" "$lobby" "$work/lobby_menu.swf"
fi
log=RoomsHarness-$players.log
harness_run "$log" -p low --width 1280 --height 720 -Pmovie=lobby_menu.swf -Pplayers="$players" RoomsHarness.swf
wait_for "$log" 'ROOMS RESULT' || true
kill "$ruffle_pid" 2>/dev/null || true
wait "$ruffle_pid" 2>/dev/null || true
ruffle_pid=
harness_report 'ROOMS .*' "$log"
grep -q 'ROOMS RESULT pass' "$work/$log"
done
