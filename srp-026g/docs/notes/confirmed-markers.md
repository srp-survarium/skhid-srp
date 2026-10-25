# Kill marker and hit marker: confirmed only (patch 73)

The rules the code and the tests follow. Code: `client-patches/confirmed_markers_026g.asm` and
`CONFIRMED_MARKERS_*` in `client-patches/patch_026g.py` (README patch 73, `client_patch.md`
section 3.58); the check: `client-patches/check_confirmed_markers_026g.py`.

- **The owner, 2026-10-25 (live rev 34):** "I see the red cross and assume the person died, yet
  they didn't"; "red hit rerolls are the worst".
- **What was wrong:** the game simulates ahead of the other players' inputs and rolls back when
  a late one arrives. The kill marker (the red cross, `show_enemy_killed_marker`) showed a kill
  the moment the local game predicted it. The hit marker (the red crosshair bars) was drawn from
  the predicted world every frame. A rollback that took the kill or the hit back left the marker
  shown. A re-simulation with another result showed a second marker (the "reroll").
- **The rule:** a marker shows only a kill or a hit at or before the event horizon. That is the
  point every game agrees on, the one 0xDF hashes: now minus 500 ms (`max_ping_time_in_ms` 250
  + 250). An input older than that is moved to a later tick, never put before it, so nothing
  there can change.
- **The kill marker:** shows with the kill feed's row. The row has always waited for the event
  horizon, so a row was never taken back.
- **The hit marker:** shows once for each hit that stays, with the shooter's damage summed on
  that victim after the hit (the game's own sum, 0 to 1, the same bars as before). Every
  confirmed hit shows, even when its sum equals the last one (a sum clamped at 1).
- **Taken back or made again:** a kill or a hit that a rollback takes back shows nothing. One
  that a re-simulation makes again shows once, as the re-simulation made it (its tick, its sum).
- **The delay:** 500 ms after the local game first showed the kill or the hit (the game clock;
  the next frame after it). A kill that only a re-simulation revealed shows at the same point:
  its tick + 500 ms.
- **Unchanged:** the kill feed's rows, the indicator of being hit (`hit_player`, still at once),
  the option `show_enemy_killed_marker`, everything the match computes. The marker comes after
  the 500 ms even if the local player died in between (the crosshair may then be hidden).
- **Not done (options for the owner):** a faster marker, once every input for the tick has come
  (about 100 to 250 ms). The exe has no signal that proves this. The 0.26g transport does not
  keep order, and a player who changes nothing sends nothing. So the fast marker could still show
  a kill whose late input (an Onyx) arrives after that player's newer inputs.
- **Not done either:** 0.69's choice, waiting one round trip of the local player (at most 500 ms;
  `netcode_069.md`). 0.26g has the place for it (the hud's latency field), but never fills it. It
  is wrong for a victim whose input comes later than our round trip (a higher ping, a resend):
  the very case of the report.
