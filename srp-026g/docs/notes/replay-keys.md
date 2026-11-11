# A replay's keys and its player list

The owner (2026-11-10): "can <- and -> be used to move a few seconds back, ./ to move per frame
(like in youtube)"; "ad maybe to work too"; "I cannot even pause the game with space"; "why player
list duplicated many players", "only one of those two is clickable"; (2026-11-11) "if the person
leaves, their replay just stops even if they enter again".

- **When:** watching a `.sur` replay. A live match's keys are untouched (the hook asks the game's
  network client for its class, as patch 135 does).
- **Left / Right:** 5 s back / on.
- **A / D:** the same while a player is followed. In the free view (F2, or the free view button)
  they only fly the camera; F1 or F3, or a click on a player, makes them seek again.
- **Comma / Full stop:** one 50 ms game tick back / on.
- **Space:** play or pause, as the play button.
- A key acts once as it goes down; held, it does nothing more. The game still gets the key (Space
  jumps nobody: a replay has no player of the viewer's).
- **Player list:** one row a player of a team, whatever places it had (a player who left and came
  back has a roster place for each time). A click follows the place that is in the match now.
- **A followed player who comes back** (the same place again, or a new one) is followed again as it
  comes in, also after a seek, in the view (first or third person) last clicked in the list. Not in
  the free view, and not after F5 / F6 (the game's own previous / next player: the list does not
  know whom they chose) until a row is clicked again.

## How

Exe patch 140 (Rust `replay_keys`) and the replay player movie (`flash/replay_player`).

- The movie gets no key from the game: a `KeyboardEvent` listener on its stage never fires. The
  patch hooks `game_world::on_keyboard_action`'s start (0x25CD00, the Tab cursor's site) and names
  the key to the movie, `root.srp_key(name)`: `back`, `forward`, `strafe_back`, `strafe_forward`,
  `tick_back`, `tick_forward`, `pause`, `followed` (F1, F3), `free_view` (F2). The movie moves its
  seek bar, as its frame buttons do, or toggles its play button.
- **The movie is told at the frame's end** (`game_world::tick` after the simulation's move,
  0x1F7C3C), not at the key. Its `update_position` seeks at once (`game_world_ui::scaleform_callback`
  0x233E2F calls the seek 0x1EE2E0: the key frame loaded, the body read to the target, the clock
  set), and the frame's replay reading (0x1EDEF0) still has the clock read before the keys. A seek
  back at the key was followed by a reading on to the old time: the world 5 s ahead of the clock,
  nothing moving until the clock came to it, then a jump ("cannot find older item for time", 48 623
  lines in one run).
- **The places in the match.** `select_player(place, view)` (0x233EBD) follows a place only when
  it is in the match at that moment (`get_active_player` 0x26D6A0 by the core's `active_places`),
  and nothing follows it again. At the same frame's end the patch tells the movie the core's
  places, `root.srp_places(Boolean x 20)`, whenever they are others than it last told that movie;
  the movie asks `select_player` again for a place of the followed row that came in.
- The list's rows are merged in the movie (`set_players_list`: by team and name); a click asks
  `select_player` for every place of the row, and the game does nothing for those not in the match.
- Nothing a match computes; no simulation change.
