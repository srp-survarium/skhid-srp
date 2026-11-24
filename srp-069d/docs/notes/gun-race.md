# Arms Race (gun race, mode 3, "Перегони озброєнь")

PROVEN = read in the exe (address), the client's data or the script; LIKELY = reasoned; CHOICE = a server rule no client
evidence decides; OPEN = not settled. Addresses are 0.69d exe VAs.

## 1. What the client has (PROVEN)

* Mode id 3 = `gunrace` in the exe's mode table 0x200987060 (flags 0x101). The PvP picker's check box 3
  (`checkBoxDomination`, label `st_match_settings_checkbox_gun_race`) sends bit 3 of the `2F` mask; the `AC` play gate
  bit 3 enables it.
* Four mode files: `arena_01` (8-8), `arena_01_mini` (3-3), `arena_01_solo` (1-1), `arena_01_test_profiles` (its own
  test script). All: map 13 (Olivier mall), variation 0, day, `score` 10, rules 109 (player controls), 520 (scripting),
  103 (kd stats), 108 (winner spawn for domination); no time keys. The level ships as
  `projects/arena_01/default/client_project`; `map_name` is the bare `arena_01`.
* No custom-match and no bots-training entry (`singleplayer_match_maps.options`).
* `scripting.player_profiles` = [5 equipment levels][9 race levels][kit variants] of `gameplay/players/ai.profiles`
  kits (assault rifle, SMG, SMG, carbine, sniper rifle, shotgun, pistol, snowball, icicle; every kit wears the
  `newyear_gunrase_helmet` hat). The string `player_profiles` is not in survarium.exe: the exe never reads the list; the
  server writes the kits into the options (as the PvE bots' `pve_profiles`).

## 2. arena.lua (PROVEN; `gameplay/scripting/arena.lua`, a binary config)

* Players `player_0` .. `player_7` (eight). Per player: `profiles` (race level 0..9), `match` (kills on this level),
  `alive`, `player_id` (= N).
* Entry (`has_entered_match`, event 18): only records without an AI id (`not(E20)`, record +0x2E0 = -1). It adds the
  damage resistance modifiers 0.2 / head 0.15 and spawns the player (action 14) with `profile = profiles.player_N` at a
  `logic/respawn/first/*` point, 1.5 s spawn protection. **An AI record is never spawned: the AI cannot play.**
* Start: when every human entered (or six did), 10 s later `start_match_instantly`; before that a 7-minute pre-game
  timeout ends the match with `success false, winner_team 1` (world state 4).
* Kill (`is_killer`, event 4, killer != victim): the killer's `match` +1 (event 45 `gunrace_rank_progress` +1) up to 3
  kills; a level-up after 3 kills on the level, after 2 on levels 7 and 8 (snowball, icicle), or at once with a melee
  kill: `profiles` +1, `match` = 0, damage resistance -0.05, action 16 switches the living killer to the next profile.
  The level-up past the icicle (`profiles == 9`): 1 s later action 26 `success true, winner_team = player_id` (the
  winner's record index).
* Death (`is_alive_player_N`): a melee death costs the victim his partial kills and one level (event 46
  `gunrace_rank_loss`); respawn after 5 s at `logic/respawn/*` with his current profile.
* Time end: 10 minutes after the start, action 26 `success true, winner_team 9` (nobody won).
* Action 26 sets the scene's finished / success flags (0x2003CCC40), the winner byte `S+0xEBC0 = winner_team`
  (0x20035C6D0) and `S+0xEBD4` (0x2003BF5B0); the world state becomes 5 (success) or 4.
* Score: events.options' `gunrace` block (the exe picks it by the mode table, 0x2002E0741): kills pay 45 silver,
  6 experience, 3 reputation and **score 0**; rank progress +10, rank loss -10. So a player's score (+0xA1C) is ten
  times his race progress (3 points per level plus the kills on the current level).

## 3. What the server does

| Item | Rule | Label |
|---|---|---|
| Race level 7 | the tier's icicle kits (level 8's list) instead of the data's `level*_snowball` kits, in the extractor (`crates/data-extract`; `data/gunrace_069d.json` `level7_replaced` keeps the data's names and the reason): the snowball's `grenade_set` file is not in the 0.69d resources and a snowball profile makes the exe fault building the match (section 4). arena.lua's "2 kills on levels 7 and 8" stays | CHOICE |
| Row | `(13, 3, 0, 1, arena_01, arena_01/default, 8, 8)` from `arena_01.options` (`crates/data-extract` -> `data/gunrace_069d.json`); the world host serves `arena_01` | PROVEN data, CHOICE (the 8-8 file of the general list) |
| Options | each human's record carries the nine race kits of the match's equipment level as profiles 0..8 (state 0, enabled), fresh item ids, the kits' modules; his name, account, face, skills and boosters stay; ammo / pocket storage as every online match (`local_match::gunrace`) | PROVEN (script) / CHOICE (skills, boosters kept) |
| Kit variant | per player and level, uniform over the listed variants, from a hash of (match id, record, level) | CHOICE |
| Equipment level | options +0x1081 (the script's `equipment_level`, the outer list index) = the highest equipment level of the humans, 1..5 | LIKELY (+0x1081 is `equipment_level`, 0x2003C8EA6) / CHOICE (the highest) |
| Teams | each record its own team = its record index. The winner byte is the winner's `player_id`; the client reports "won" (53 event 5) when the winner byte equals its team (0x20011695E), so only the race winner's client reports a win. The exe's ally checks compare team bytes (friendly fire 0x2003E9CF0): nobody is an ally | LIKELY (the original's layout) |
| Players | 2..8 humans (the queue's general minimum), no AI players, no bot fill, never ranked; squads may queue (each member is an opponent) | PROVEN (script) / CHOICE |
| End | arena.lua's own end in the world host: the race winner or the 10-minute time end (state 5), the 7-minute pre-game cancel (state 4). The relay has no end rule for the mode (the mode file has no times) and ends from the host's Outcome | PROVEN (script and a world host run, section 4) |
| Winner to the lobby | state 5: the winner byte as it is (a record index, or 9); any other state: "no result" (`match_server::GUNRACE_NO_RESULT`) | PROVEN (script) |
| Verdict | winner = the seat whose team is the byte; 9 = a played match nobody won; no result = nobody is paid. Without a host: the clients' reports (one claimed win = that seat; all lost = 9) | CHOICE |
| Places | by score over the whole table (97 place byte, team place, event-currency place) | PROVEN (score = race progress), CHOICE |
| Rewards | the `gunrace` events.options rows, no level factor (the client takes 1.0 in mode 3 for its own award amounts, 0x200332080), reward level 1 for everybody ("The list of rewards does not depend on your maximum equipment tier", OFFICIAL Guns Race notes 0.61b / 0.65d / 0.69b) | PROVEN (factor, notes), LIKELY (events follow the factor), CHOICE (level 1 at factor 1.0) |
| After-match charge | none: the race kits are the match's, not the player's (no equipment use, no pockets, no ammo boxes) | CHOICE |
| Queue / `AC` | bit 3 offered and the row in the unrated pool (`queue::GUNRACE_OPEN`: a match with the race kits runs to arena.lua's end in the world host) | CHOICE |

## 4. World host runs (offline: synthetic recording + replay, `servers/world-host` `HOST_BOTS_SYNTH`)

* `arena_01/default` loads in the world host (one placeholder human: match created in 267 ms, collision checksum 210e82e9),
  so the level path with `/default` is right. PROVEN.
* **The snowball kit stops every gun race match.** With the nine race kits the exe faults while it builds the players'
  weapons (`on_match_ready` profile cook): a null resource pushed at 0x2006d078c (queue push 0x200557500, weapon cook
  0x2006cfd20). One race level per run: levels 0..6 and 8 build, level 7 (`level*_snowball`) faults at every tier; the
  null-cooked render / sound classes do not matter (the fault stays with other lists). The snowball is the only weapon
  whose fire state is a throw state (`state_type` 319) with a `grenade_set`,
  `resources/gameplay/items/weapons/grenades/snowball_grenade.options`, and that file is not in the 0.69d client's
  resources (resource path list; `gameplay/items/weapons/` holds only three `common/*.user_animations`). The world host
  runs the client's own code, so a real 0.69d client LIKELY faults the same way: the 0.69c / 0.69d server cannot have
  sent the snowball kits of the client's (stale) `player_profiles` list. So race level 7 carries the icicle kits
  (section 3).
* With level 7 replaced by the icicle kit (profiles = levels 0..6, 8, 8; four idle humans, tier 3), the match runs to
  arena.lua's own end in the host: rule state 0 -> 3 at the first DF, -> 5 at DF 611100 = the enter DF 1100 + 10 s
  (`all_players_ready`) + 10 minutes (`minutes_per_match`), winner byte 9, present mask 0xf, no kills. PROVEN (the time
  end; a race win needs real players).

## 5. Open

* The cosmetic hat of the kits (`helmet_visual_slot`, `newyear_gunrase_helmet`): the bot kit writer has no slot for
  it (the profile slot of the visual helmet is not traced), so it is not carried, like every bot kit.
* The original's kit draw (one variant per match or per player) and its equipment level per match.
* Whether the 3-3 / 1-1 files were queue rows (the server offers the 8-player file with the general 2-player minimum).
* The meaning of the mode table's flag bit 0 (gunrace and dom): one reader 0x20030177D skips a call when it is set.
* A race won in a real match (kills, profile switches, the winner screen) needs a game run with two clients.
