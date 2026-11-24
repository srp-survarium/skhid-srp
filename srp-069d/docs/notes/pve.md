# PvE team missions (0.69d)

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

Status words: PROVEN = read from the 0.69d exe (VA given), the 0.69d data (file given) or the lobby Flash ABC; LIKELY = layout
proven, meaning inferred; OPEN = not traced, nothing invented. The server side is in sections 8 and 9. Addresses are exe VAs (image base 0x200000000).
Data: `data/pve_069d.json` (made by `crates/data-extract`). Official texts of the 0.50 release (Oct 2017, PC Gamer,
MMOBomb, WorthPlaying) agree with what the files say; the game's own English strings (`st_pve_*`) are quoted below.

## 0. Headline

1. **PvE is an ONLINE match, not a local one.** `5F` start -> the lobby answers `82 connect_to_game_server` like a PvP match.
   The world of a mission (doors, loot, story, every body incl. the bots') is simulated by
   the clients in deterministic lockstep, scripted by `gameplay/scripting/pve.lua`; the 17 bots' AI ran on the match server, which
   relayed their inputs like a player's (section 10). The world host builds the starting world of a mission like for online
   PvP (`online.md`).
2. `pve.lua` is NOT Lua. It is a 1.09 MB binary config (volume 22): a compiled event-script graph with 123
   "actions processors" (section 3). The exe never needs a server to run it, but its AI actions do nothing on an online client
   (section 10).
3. There is exactly ONE mission: "Dangerous knowledge" (`id` 15), 3 difficulties, 3 players, 6 map projects.
4. Solo is allowed ("You can play alone or in a squad", `st_pve_mission_hint`): the matchmaker fills a mission of 3 with other
   players; a commander can lock slots to play with fewer.

## 1. Flow, mission pick to results

```
lobby (Flash PVEManager)                  client exe                                  server
 mission window shows 1 mission,   <- root.set_pve_missions (built from pve_missions.options + 8D + game.options)
 difficulty 0/1/2
 [squad commander changes mission/diff] 5F 00 u32 map_id, u32 diff  ------------------> push C2 00 u32 map_id u32 diff to the other members
 [member] accept_pve_commander_settings  (local only, no message; 0x1EC2D0)
 [member] decline_..._settings           49 04 (= leave_squad, call 0x1EC608)   -------> AE
 [member] pays_for / locks_slot          49 05 u8 state, u8 0  ------------------------> AE with changed state bytes
 Play, solo                              5F 02 u32 map_id, u32 diff  ------------------>  queue; 85 04 order.. ; later 82
 Play, commander of a squad              5F 01 u32 map_id, u32 diff  ------------------>  same for the whole squad
 Play, non-commander squad member        49 05 u8 state(bit0 ready toggled), u8 0
 Play again while queued                 31 u32 order (cancel, call 0x18AE30) ---------->  85 00
 denied                                                                        <------ 84 x 5F  u8 1, u8 code
 match found                                                                   <------ 82 host,port,match id,side,order  (match server, online.md)
 mission (match server, lockstep, pve.lua)
 end                                                                           <------ 99 (victory screen) ... then 85 00
 rewards screen: close 5F 04, unlock chests 5F 05, reroll 5F 06  -------------------->  83 45 0B ("End screen")
```

### 1.1 C->S layouts (all builders PROVEN)
| message | layout | builder / site | UI call |
|---|---|---|---|
| `5F 00 u32 a, u32 b` | commander only (needs L+0x798898 squad count != 0 and L+0x7988A0 commander flag, else `IGNORE ... not a squad commander`) | 0x2001EBF40 (send 0x2001EC0F3) | `set_pve_match_settings(map_id, difficulty)` |
| `5F 01 u32 map, u32 diff` | squad commander Play | 0x2001EB730, send 0x2001EBDCE (start kind 1 stored at lobby +0x73FA0) | `pve_match_start_clicked(map, diff)` |
| `5F 02 u32 map, u32 diff` | solo Play (squad count 0) | 0x2001EB730, send 0x2001EBB68 (start kind 2) | same |
| `5F 03` | top 100, at most once per 60 s | 0x2001EC8E0 (site 0x2001EC93F, 60 s guard) | `ask_pve_top_100` |
| `5F 04` | close the PvE stats screen | 0x2001973E0 (call 0x19E202) | `close_pve_stats_clicked` |
| `5F 05` | unlock the locked reward chests | 0x2001974E0 (call 0x19E294) | `pve_unlock_rewards` |
| `5F 06` | reroll the rewards | 0x200197460 (call 0x19E24B) | `pve_reroll_rewards` |
| `49 04` | = leave_squad | 0x20018DA70 (call 0x1EC608) | `decline_pve_commander_settings` |
| `49 05 u8 state, u8 0` | own squad record `state` byte | 0x20018DAF0 (calls 0x1EBD0E, 0x1EC896, 0x1ECC81) | start (non-commander), `pve_squad_player_pays_for(bool[])`, `pve_squad_player_locks_slot(bool[2])` |
| `4D 17` | invite to PvE squad (u64 id, u8 0x17, str); the accept answer is `4D <id> 0x18` | 0x200291360 | `invite_to_pve_squad` |
| `31 u32 order` | cancel the search | 0x20018AE30 | Play while queued |

`5F 04/05/06` are close stats / unlock rewards / reroll rewards, not commander settings (call sites 0x19E202/0x19E294/0x19E24B
inside the Flash call table at 0x19D8EC, names compared with the strings `close_pve_stats_clicked`, `pve_unlock_rewards`,
`pve_reroll_rewards`). `crates/protocol` `Pve5F` covers 00..06.

PvE options byte +0x1082 = the difficulty AND an unchecked index into the 3 anomaly-maze damage profiles (0x20030EC81):
the PvE match options must send 0..2 (pve.rs `valid` keeps the difficulty in the data's range).
`49 05` second byte: 0 from Play and pays-for, 0xFF from the commander's lock-slot button (builder 0x20018DAF0,
caller 0x2001ECC81); the server's bit checks by role cover both.
`49 05` state byte (PROVEN for pays-for bits, LIKELY for the rest; AS `SquadInfo.getLockedSlots`, 0x1EC6xx loops): it is the new value of the
sender's own `state` byte (+0x19 of the `AE` squad record). bit 0 = ready (Play pressed by a member), bits 1..3 = "I pay for squad slot i-1"
(bit i+1 set from `array[i]`, 0x1EC700), bits 4 and 5 = commander locked slot 0 / 1 (read from the COMMANDER record).
The pays-for button exists for a member without a key: `PVESquadPlayer.btnBuyTokens`; an ally pays for him (official: "you can pay for
yourself, and also for one or both allies").

### 1.2 S->C layouts
| message | layout | reader | what |
|---|---|---|---|
| `C2 00 u32 map_id, u32 difficulty` | PROVEN | inline 0x20018FB55 -> 0x2001CB940 | members: "The squad leader changed the settings of the mission" + `root.on_pve_settings_changed_by_commander`; Flash then calls `accept_pve_commander_settings` |
| `C2 03 <u8 n> n x entry <u32 own> [entry]` | PROVEN (section 5) | 0x200197740 | top 100 |
| `84 x 5F <u8 1> <u8 code>` | PROVEN | 0x20018F4BF -> `on_pve_action_denied` 0x2001CBBD0 | codes: 3 `st_profile_check_failed_not_enough_money` (loop var), 4 `st_not_enougth_money`, 5 `st_not_all_team_members_ready`, 6 `st_not_enough_match_servers`; other = generic `st_pve_action_denied` ("Unable to start the team mission"). Clears `lobby_menu+0xEBBD4` (the "Play waiting" flag). Code `0`/other than 1: nothing is read |
| `85 04 u32 order, u32 search, u32` | PROVEN | 0x200186280 | queue state 4 = "PvE search": same body as state 1; the status case calls `root.set_pve_match_making(queue_state == 4)` (0x200175639, state test 0x200179951), then 0x2001C16F0 with `search` (0x2001756B9): 0 = "match found" (`set_mm_sync_state`), not 0 = search runs (`set_game_search_enabled(true)`). Sent 100 (the exe's own running value): with 0 the movie's game search does not turn on, the `85 00` changes nothing and `MatchMaking.hide` (set at the level load) keeps the mode window shut. `85 00` ends it |
| `82 str host, u16 port, u32 match id, u8 side, u32 order` | PROVEN | 0x20018E0FA | connect to the match server (`online.md`) |
| `AE` squad update, byte "x" = squad `type` (0 normal, 1 PvE) | PROVEN | 0x20018DC00, UI 0x1C4891 (`type`), AS `PVEManager.setSquadType` | `type == 1` opens the PvE window on its own; the squad `state` byte carries ready/pays/locks |
| `8D` service prices, tail `u8 flag, u16 price` | PROVEN | 0x200180830 | the u16 (stored +0x2C, flag +0x2E) replaces the gold price of a ticket in `play_cost_gold` (0x1BA3AA) |
| `99` PvE result | PROVEN layout | block 0x20018ECDD, `0x180D10`, `0x180980` | section 4.4 |
| `83 x 5F` | no case in the 0x83 table (0x20018F264, byte 0x5F -> default) | - | success is expressed by `82` / `85`, not by `83` |

`set_pve_missions` data (0x2001BA350): per mission `id, name, description, preview images, play_cost = 1 (constant, 0x1BA61B), play_cost_gold,
free_chests_num, unlock_drop_price, min_squad_members, difficulties[{label, description, rewards[{dict_id, count}]}], closed_chest_dict_id`.
`play_cost_gold` = `pve_default_prices.ticket_price` (50), replaced by the `8D` u16 when its flag is set; `free_chests_num` = 3,
`unlock_drop_price` = 50 (config +0x3DD2 / +0x3DD0 loaded at 0x2002DE331).

## 2. Missions (PROVEN, `gameplay/pve_missions.options`)
One entry: `id 15` (= `match_making_params.map_id`), `name st_pve_mission_1_name` ("Dangerous knowledge"), `min_squad_members 3`,
`chests_total 10`, `closed_chest_dict_id 1385`, `chests_premium_bonus_coef 1`. Difficulties (labels Easy / Medium / Hard, "Recommended for
newcomers / experts / veterans"): preview reward dict 1483 (`pve_mission_1_chest_rank_11`), 1468 (`rank_12`), 1469 (`rank_13`).
**No unlock rule exists**: the file has no level or prior-mission field. The only gate is advisory: the squad equipment level (maximum
`items_level` of the members, AE record) is compared with recommendations in Flash (`equipment_level_ok / lower_recommended / over_allowed`,
`st_pve_misson_requirements`); it never blocks Play. Server-side refusals are only the `84 5F` codes above (money, not all ready, no server).
`min_squad_members` (3) is the player count of a mission: the settings window shows `squad size / 3` while searching
(`PVEMatchSettingsWindow.playersCountTxt`), locking a slot lowers it ("Reduce the number of players for the mission").

Game modes (`gameplay/game_modes/pve_01_v{1,2,3}_maze{1,2}.options`): six files that differ ONLY in `match_making_params.map_name`
(`pve_01/v1_maze1` ...); `mode 7`, `map_id 15`, `map_weather_id 1`, `match_time 1800`, `scripting.script resources/gameplay/scripting/pve.lua`,
`parameters.wait_time 300`, `countdown_time 10`, `time_needed_to_continue_match 20`, `min_respawn [20,10]`, `max_respawn [100,30]`, `rules [521,109,520,103,111]`,
`anomaly_maze` = 3 anomaly damage profiles (fire, pulse every 3..9 s, `anomaly_damage_amount` 0.25 ...). `scripting.pve_profiles[difficulty][5][17]`:
17 enemy bots per difficulty (the 5 groups are identical): easy `1lvl_*`/`2lvl_*` profiles, medium `2lvl_*`/`3lvl_*`, hard `4lvl_*`/`5lvl_*` kits,
all `team 1`, `hit_chance 50`, `hit_damage_multiplier 0.5`, full `ai_params` (vision 40 m, hearing, PID aim). `v1/v2/v3` is the map
project folder (LIKELY = difficulty version) and `maze1/maze2` two variants (LIKELY chosen at random by the server): OPEN which one the
server must put in the options `project`/map strings for a difficulty. Difficulty itself reaches the script as the engine variable
`game_difficulty` = options byte `+0x1082` (read 0x2003C8E18), `equipment_level` = `+0x1081` (0x2003C8EA6), both set by the options reader
0x200125133..0x2001251F0 (PROVEN; the slot-to-variable order of the config parser LIKELY). `human_players_count` is not a byte: it counts the
player records whose ai_id (+0x2E0) is -1 (0x2003C9464..0x2003C951D, PROVEN), so every human record must carry ai_id 0xFFFFFFFF. pve.lua
compares `game_difficulty` with 0 and 1 only (0/1/2 = easy/medium/hard), declares `equipment_level` but never reads it, and tests
`human_players_count == 3`. No file or exe code maps a difficulty to a `pve_01_*` variant (the six mode files differ only in `map_name`):
the variant is a server choice. The six projects have identical `server_objects` (96 patrol graphs, 112 respawn points, 32 sensors,
168 usable zones, the victory item); `v1/v2/v3` differ only in their collision objects (6948..7028 instances) and visuals, `maze1/maze2`
only in client objects (the anomaly maze walls). PROVEN (data). The bot values above (`hit_chance 50`, vision 40 m) are the easy
group's; section 10.3 lists all three.

## 3. pve.lua (PROVEN content; semantics of action types LIKELY)
Keys: `actions_processors` (123), `lootboxes` (60), `lootbox_shuffled_indices` (1024 x 6), `combat_loot_corpses` / `combat_loot_bags` (17),
`bots_spawn_sawmill_forward/backward` (36 / 38 spawn + patrol-graph pairs), `animations_to_select/exit` (7 bots), `random_tp_players`,
`other_2_players`, `required_game_effects ["artefact_spring"]`, `engine_string_constants` (player_0..19, `current_time_in_ms`, `game_difficulty`,
`equipment_level`, `human_players_count`). Story: forest (scouts, anomaly nuts) -> sawmill (clear bots, reinforcements, open the anomaly)
-> lab (portal rooms, ventilation puzzle, take the Spring) -> sawmill again (army bots "Boar") -> exit zone. Objective texts `st_pve_mission_1_obj_title_1..9`
(Pick up the nuts; Toss a nut into the anomaly; Get out from the forest; Clear the sawmill; Explore the anomaly; Explore the laboratory; Meet the Boar group;
Survive!; Get to the extraction point), dialogs `st_pve_mission_1_dialog_*`, hints `st_pve_map_hint_1..3`.
Match rules inside the script (all values read from the data):
* Players are `player_0..2`; before the start a pre-game timeout of `minutes_to_wait = 7` min ends the match unsuccessfully (`type 26`, `success false`);
  when 2 of 3 humans have entered, a 60 s countdown starts the match (`2_from_3_players_ready`); when all `human_players_count` have entered it starts at once
  (`all_players_ready`, `start_match_instantly`). If everybody left before the start, `cancel_match_instantly`.
* Match length `minutes_per_match = 45` (success `false`/timeout side; the mode file says `match_time 1800` s: OPEN which wins, official text: 45 minutes).
* Friendly fire modifier 0 for entered players. Corpse respawn / paid resurrection: `respawn_duration_in_ms 12000`; event 17 `player_paid_for_respawn`
  (the server-side payment confirmation) revives the player; when all humans are dead (`can_resurrect` all false) the match is lost (`finish_match_if_all_players_are_killed`).
* Chests = 60 `lootboxes` (shuffled by `lootbox_shuffled_indices`, 10 per mission = `chests_total`); opening one is the score event `pve_open_chest`.
* Success: the `exit` processor (usable zone at the end): `type 26 success true, winner_team 0, match_time`, player counter +1 (variable 0), after 3 s.
Nothing here needs the server except lockstep relay and the match-end report.

## 4. Costs and rewards
### 4.1 Prices (PROVEN, `game.options` v75 `pve_default_prices`, parsed 0x2002DE331)
`ticket_price 50`, `respawn_price 50`, `sponsor_price 50`, `block_slot_price 50`, `unlock_drop_price 50`, `reroll_start_price 50`, `reroll_step 25`,
`free_chests_num 3`; `pve_reward_reroll_info [[50, 25]]`. Currency gold (premium) in every case (voucher `currency 1`).
### 4.2 Access keys (tokens) = inventory items (PROVEN)
dict 1384 `pve_ticket_voucher` ("Access keys", premium, cost 50 gold, `vch_drop`, category 27) and dict 1617 `pve_ticket_voucher_free` (free key; the same
name). Flash `PVE_TOKEN_PREM_ID 1384`, `PVE_TOKEN_FREE_ID 1617`; the UI counts them via `getVoucherCountByDictId`. A mission costs `play_cost = 1` key per
player (gold price = key `cost` x 1 = 50 when buying). Official: 5 keys at the first login after 0.50 (2 free + 3 premium), 2 free keys for the first two wins
each day (`st_pve_tokens_resupply_info`), keys are bought in the shop ("Insufficient Gold to purchase the Access keys", `st_insufficient_funds_for_pve_tokens`),
a resurrection costs a key (`pve_respawn_with_voucher`) or gold (`pve_respawn_with_premium`, `respawn_price`), a locked slot costs `block_slot_price`.
LIKELY rule: at start the server takes one key (free first) or the gold price from each paying player (his own, or the sponsor's for allies with the pays-for bits);
refusal = `84 5F` code 4 (money) or the Flash text `st_not_enought_pve_tokens`. OPEN: exact order free/premium key, whether bits set by a sponsor
consume his key first.
### 4.3 Chests and rewards (PROVEN data)
The reward is chests. During the mission each cache found = `chests_found` (of `chests_total` 10). `closed_chest_dict_id 1385` (`pve_mission_1_chest_closed`,
drop type `pve_mission_1_chest_rank_1_drop`) is the locked cache shown on the results screen. The opened content is `pve_mission_1_chest_rank_NM`
(dict 1468 = 12 ... see `data/pve_069d.json chest_items`, tables in `chest_drops.options`, already in `data/chests_069d.json`): N = 1..5 (equipment / level tier), M = 1..3 =
difficulty + 1 (the three preview dicts of the mission are N=1). Each chest gives 1 item (`items_count 1`). The first `free_chests_num` (3) caches open free; more cost
`unlock_drop_price` 50 gold each ("open all" `openAllChestsCost`); `5F 06` rerolls the contents for `reroll_start_price 50 + reroll_step 25` per reroll.
`chests_premium_bonus_coef 1`. Score event `match_state_event_pve_open_chest` = 33 score, 0 money / xp / reputation (`data/progression_069d.json`).
Module drops: `used_weapon_pve_dropping_modules_percent 100`, `modules_mods_pair_pve_prob` 11 x 11 (non-zero only rows 6..9, 0.04..0.15).
The mapping of equipment level to N is LIKELY (names + Flash `equipment_level`), the Flash end screen additionally has `rewards_data`, `auto_open_chest`, `afk`
("reject_drop_for_inactivity"): afk players get no drop.
### 4.4 `99` PvE result (readers 0x180D10 team, 0x180980/0x180DC3 players) PROVEN fields, memory order (the wire order of the header: "Protocol audit" at the end)
```
u32 match_id (+0, compared with the running match id 0x20018EDA7)   u32 (+4, not displayed)   u32 match_time seconds (+8)
u8 n players (+0xC)   u8 (+0xD, unused by the screen)   u8 chests_found (+0xE)   u8 chests_total (+0xF)   u8 success (+0x10, != 0 = victory)
n x { str(128) name, str(32) clan abbreviation, u64 public account id, u16 score (signed), u16 kills, u16 deaths, u8 sort order, u8 premium }
u8 index of the receiving player in the list        (max 20 records are prepared)
```
UI 0x13EA20: `match_result, match_id, chests_found, chests_total, match_time`, per player `"[abbr] name", public_account_id ("%llu"), kills, deaths, score, sort_order, premium`.
### 4.5 Respawn (match side, PROVEN client; server rule: section 9 "Paid respawn")
`show_pve_respawn_message` (0x1AD500, texts `st_ask_respawn_now`, `st_respawn_is_possible_for_tokens`, `st_pve_respawn_timer_msg` "Press SPACE to resurrect right now"):
the confirm button (`pve_respawn_confirmed`, 0x12FC30) sets an input flag of the match; the match server tells the script event 17 and charges key or gold through the lobby.
Free respawn at the next checkpoint (`st_pve_respawn_next_checkpoint`). `set_dropped_pve_items` (0x1AC044, HUD) lists items with `count`, `dictId`, `cost`
and whether `player_premium_money` covers it; it is fed by `83 45 0B` ("End screen").

## 5. Top 100 (PROVEN layout)
`5F 03` -> `C2 03` reader 0x200197740 (frame 0xCE00): `u8 n` then n entries, then `u32 own index (FFFFFFFF = none)`, then, if not FFFFFFFF, one more entry (the
player's own squad). Entry (0x181320, 0x208 B in memory): `u8 k (<= 3)`, k x { str(128) nick, str(32) clan abbr, u8 score }, `u16 play_time` seconds.
UI (0x1CBE80): `position` (1-based), `players[{name, abbr, score}]`, `play_time` formatted `h:mm:ss` (0x1CC02A), `is_player` = own entry. So it ranks squads
(1 to 3 players) by mission time; official: the position depends on found stashes and completion time (so sort key: more caches, then shorter time; LIKELY, only
the displayed time is proven). Empty list text `st_pve_top_100_empty` ("Currently there's no players who passed the mission"). The exe asks at most every 60 s.

## 6. What the server stores (`crates/accounts/src/pve.rs`)
Per account: `pve_keys` are inventory items 1384 / 1617 (no new column); `pve_runs` count of wins today (for the two free daily keys; reset by day);
last settings is client state. Per finished run (table `pve_results`: match id, mission 15, difficulty 0..2, success, time s, chests found, started_at, squad of 1..3 account ids
with score / kills / deaths / sort / premium) feeds `99` and the top 100 (best run per squad: key = sorted account ids). Per run and player (`pve_rewards`): chest list
(closed chest dict, tier N, difficulty M, opened flag, reward item, reroll count, paid) so that `5F 05` / `5F 06` are idempotent; plus achievement counters
(`win_pve`, `win_pve_all_stashes`, in `Profile::ach`; `record_run_notify` / `record_run` count them only when the match id is
stored the first time, for every player of a won run with caches found >= `num_stashes_found` 3 / 10, and push the B9 before the `83 45 0B`;
progression.md section 1). In memory only: PvE queue tickets, squad type = 1 and settings
(map, difficulty, locked slots) of a squad.

## 7. LOCAL (client only) vs SERVER
Client only: pve.lua's world part (spawns, deaths, doors, loot boxes, anomalies, objectives), lockstep simulation, the Flash windows, price
display from `pve_default_prices`, accept of commander settings (no message), mission list, equipment recommendation. Server: queue / team assembly, key and gold payment, the
squad `type` and `state` byte, `C2 00` fan-out, the match server (world, relay, end report, paid respawn, the 17 bots' AI and their inputs: section 10), `99`, chests / rewards / reroll, top 100, daily free keys, `8D` price override.

## 8. Server parts
1. **Protocol** (`crates/protocol`): `Pve5F` 00 settings, 01 / 02 start (commander / solo), 03 top 100, 04 close stats, 05 unlock
   rewards, 06 reroll rewards; S->C `C2 00`, `C2 03`, `99`, `84 x 5F`, `85 04` queue state; AE `type`.
2. **Data** (`crates/game-data`): `data/pve_069d.json` (missions, prices, chest ids).
3. **Lobby** (`servers/lobby-server/src/pve.rs`, `squads.rs`, `queue.rs`): squad type 1 from `4D 17`, commander settings, `C2 00`, lock bits in
   `49 05`, `5F 03` from `pve_results`, the solo / squad queue of 3 (`85 04`, cancel `31`), key payment (inventory 1617 first, then 1384,
   else gold), `84 5F` codes. Storage: section 6.
4. **Match** (`online.md` path): `82` with a PvE options blob (project `pve_01_v{d}_maze{m}`, difficulty byte +0x1082, equipment level
   +0x1081, human records with ai_id -1), the relay, the end -> `99`, chests, `85 00` (section 9).
A local (training-style `5B`) PvE would need `human_players_count`, difficulty and the paid-respawn flow; NOT proven possible and not the
original design, so not built.

## 9. Implementation

Code: protocol `c2s::Pve5F` (00..06), `squad 49 05 PveState`, `s2c/pve.rs` (`C2 00`, `C2 03`, `84 5F` via `DeniedBody::Pve`, `85 04`,
`99`, AE type byte `Squad::kind`, `8D` tail helper); game data `crates/game-data/src/pve.rs`; storage schema **v14**
(`pve_results`, `pve_result_players`, `pve_rewards`, `pve_daily_wins`); lobby `servers/lobby-server/src/pve.rs`, squad kind /
state bits in `squads.rs`, `MatchKind::Pve` + `OnlineMatches::create_pve` (default refuses) in `online.rs`. Tests: `tests/pve.rs`
of protocol, accounts and lobby-server.

`84 5F` is `u8 1, u8 code` (the generic `Denied` decoder has a 0x5F special case, so the 1 is not read as the "text" code). The
PvE squad is made by `4D 17`, whose accept `4D .. 0x18` is relayed like the normal squad invite, or by the commander of a normal
squad who opens the PvE window (below).

### Rules as implemented (labels)
* PROVEN message flow and layouts (section 1). Locked slots / pays-for / ready bits: LIKELY meaning (section 1.1); the server
  strips pays-for bits of the own or a missing slot and ignores the commander's bit 0.
* Lock bits only for an empty seat: the window fills
  seats `player1..` with the other members in record order and seat `player(k+1)` has lock bit `0x10 << k`, so a bit is kept only
  when `k + 1 >= members`: 1 member `0x10` / `0x20`, 2 members `0x20` only, 3 members none.
* Squad type from the PvE window (client rules PROVEN: the `5F 00` gate 0x2001EBF79 / 0x2001EBF9C tests
  only the record count and "I am commander", never the AE type byte; what the original server did is NOT FOUND, so the
  conversion is a CHOICE):
  - the commander's `5F 00` (or his `49 05` with a lock bit) turns a normal squad into a PvE squad: AE type 1, settings, state
    bits and ready flags cleared, `C2 00` to the members (they then see the seats instead of "Awaiting other players");
  - the commander's PvP `2F` turns a PvE squad of 2+ members back into a normal squad (AE type 0, ready cleared: that first `2F`
    ends in "not every member is ready" + `85`); a member's `2F` in a PvE squad keeps `85 00`;
  - a member's `49 05` / `5F 00` in a normal squad stays refused; nothing converts while the squad searches (PvE or PvP);
  - OPEN: a `4D 0x17` invite by a normal squad's commander is refused ("other type").
  - OPEN: a member's PvE preview may stay open after the reverse switch (`setSquadType(0)` does not hide it).
* Seat locks of a player in no squad (checked in the exe).
  - What the client does (PROVEN):
    - The lock button sends `49 05 state, FF` with no squad check (0x2001EC9E0).
    - It draws a lock only from the commander record of an `AE` (`SquadInfo.getLockedSlots`, bits 0x10 / 0x20).
    - An `AE` with squad id 0 has no records (0x20018DD6D).
    - Play needs members + locks >= 3 over the records (0x2001B9EB3..0x2001B9F05). With no squad, Play is the
      "find match" search `5F 02`.
  - What the server does (CHOICE):
    - `49 05` with lock bits from a player in no squad makes a one-man PvE squad (type 1, he is the commander, his record
      holds the locks).
    - Clearing both locks dissolves it.
    - A `2F` (PvP Play) leaves it and searches alone.
    - With both seats locked, `5F 01` wants 1 player.

  No capture shows what the original server did.
* Settings `5F 00` clear every member's ready flag (CHOICE: they must accept again) and a joiner gets the stored settings.
* Play checks, in this order: valid mission / difficulty (generic), already searching, normal squad or non-commander (generic),
  not all ready (5), no match server (6), a payer cannot pay (4). The "no match server" check comes before any charge, so when
  no match server can serve the mission nothing is charged and the client gets `84 5F 6` + `85 00`.
* Charge (CHOICE): at Play nothing is taken; when the match is formed each payer is charged (free key 1617, then premium key 1384,
  else `ticket_price` gold; a pays-for sponsor pays `sponsor_price` gold for that member; the commander pays `block_slot_price`
  per locked slot), then the match server is asked; a failed creation refunds exactly. A payer who became short leaves with code 4.
  Code 3 (`profile check failed`, text with an unknown loop variable) is never sent: OPEN.
* Queue of 3 (CHOICE): same mission and difficulty, whole squads, oldest first, fills `want = 3 - locked slots`; no timeout (the
  7 min / 60 s rules are inside the mission script). Maze 1 / 2 is picked at random; project `pve_01_v{d+1}_maze{m}` is LIKELY.
* Top 100 from `pve_results`: successful runs only, best run per squad (more chests, then shorter time: LIKELY from the official
  text); own entry = the best entry the player is in; the per-player score byte is the run's chests found (see "Result screen
  sequence").
* Keys: daily rule "free key (1617) for each of the first 2 wins of a UTC day" implemented in `pve::win_key` / `record_run`
  (LIKELY 0.69d: the client has `st_pve_tokens_resupply_info`; the number 2 is from the 0.50 release notes). The one-time "5 keys at
  the first login after 0.50" was a release gift: OPEN, not given. The client is told about a new daily key at once (see below).
* `5F 04` ends the result screen ("Result screen sequence"); `5F 05` / `5F 06`: section "End screen" at the end.

### Equipment level and the daily key popup
* `AE items_level` (PROVEN structure, LIKELY byte): the client computes its own equipment level (`root.set_equipment_level`,
  0x1C7440 over 0x3330C0) as the highest dictionary level byte of the worn items over the playable profiles, skipping the pocket
  slots (13..17, 21..22) and stacks with no units; the byte is `item_level` (1..5, LIKELY: the ranked check "minimal tier 3" uses it).
  `items::equipment_level` does the same from the stored profile and `squads::ae_for` sends it for every member (0 = nothing
  worn). (It does not pick the chest tier: tier 1 always, see "End screen".) The AE is built when the squad changes, so a member who changes
  gear while in the squad shows the old number until the next AE (OPEN: whether the client asks again).
* Daily key popup: `pve::key_messages` / `pve::record_run_notify` push the `83 46 00` received-item popup (flag 1, the key stack's real
  uid, one unit), like a match drop. The online end calls `record_run_notify`; the test is
  `pve::tests::a_new_daily_key_comes_with_a_popup_of_one_unit`. Not sent: a full `89` inventory push (the popup record carries the
  stack; that the popup alone moves the displayed count is LIKELY, to check in the game).

### Online missions
* Options (`local_match::pve_options`): the humans (team 0, ai id -1, `+0x2F8` = the data's respawn price, `+0x2D0` keys
  and `+0x2D4` gold budget 0), then the 17 script bots `pve_bot_1..17` of the difficulty (`data/pve_bots_069d.json`,
  `crates/data-extract`: team 1, ai id 0..16, face, the AI block, kit `profile` as match profile 0 and
  `profile1` as profile 1, both present), `n_players` humans + 17, `+0x1082` difficulty, `+0x1081` =
  `PVE_EQUIPMENT_LEVEL` 1 (a client capture has the same value), `single_player` 0, seed = the project world's.
* World: built by the world host per match; `create_pve` (srp `Online`) takes the lobby's project, else the other maze of
  the difficulty.
* Relay: the bots are server-owned slots (`PlayerSpec::bot`): no ticket, ready at once, enter in the same DF as the
  humans with no start profile, no input (idle where the script spawns them), outside the enter gate, the silence rule,
  the "all left" end and the match result. PvE humans carry no booster (none is charged; CHOICE).
* End (lobby `online::pve_result`): when the reports are in (or the first is 30 s old) the run is stored and `99` +
  `83 45 0B` go to the players; `PVE_END_AFTER_RESULT` (3 s) later the relay match is ended with DF code 2 (the result
  screen, see "Result screen sequence" below), and `85 00` follows the player's `5F 04` (at most 120 s after the `99`).
  Without the world host's outcome ("Mission end from the world host"): success = majority of the reports; time = the
  match server's clock from the enter DF to the first report (`OnlineMatches::played`); chests found, score, kills,
  deaths = 0 (no client reports them), so a won run gives the daily key and the achievements but no chest.
* Paid respawn:
  * **A zero budget (PROVEN):** a zero budget makes the client's dialog 0x20023DE1A show "can't" and send
    nothing.
  * **What a respawn is:** the confirm is a CD `profile_input` with b1 = 1, which the relay relays unchanged. The world
    charges in its scripting step (0x2003CF230) against the options record: `+0x2D0` keys, then `+0x2D4` gold at
    `+0x2F8` (the price, 50).
  * **With the world host:** srp writes the human's spendable access keys after the entry fee into `+0x2D0`, and his
    gold balance after the start charges into `+0x2D4`. The world host reports what the world charged
    (`keys_spent` / `gold_spent`, world entry `+0x54` / `+0x58`, protocol version 2). The lobby charges it at the result
    (`pve_end::charge_respawns`):
    * won and failed missions alike;
    * before `83 45 0B`, so a `5F 05` cannot spend a key the player already used;
    * keys free first (1617), then bought (1384); gold through `8B`.
  * **Without a host:** no budget and no charge.
  * **OPEN:**
    * the original's exact values (all keys or only bought ones, a gold cap);
    * a charge after the world's end tick;
    * the "Insufficient funds to resurrect instantly" popup: the code at 0x20023DE1A reads as opening
      `pveCantRespawn` with a zero budget, but a test match did not show it.

### Mission failure ends (relay; `match_server::PveEndRule`)
An online client never ends a match by itself and sends nothing when its world ends, so
the relay mirrors the script ends it can compute without a world and closes every client with the fin DF from the
first DF after the script's end step (the time-limit pattern). All of them are failures, `winner_team` 1; the relay
reports winner 1 (`EndReason::PveCancel` / `PveTimeout` / `AllLeft`). Extraction (`exit`, the only success) and "all
humans dead" are not computable and are not inferred.

| End | Script (pve.lua processor: expression) | World step of action 26 (state) | Status |
|---|---|---|---|
| no start | `match` (index 1) enables `wait_before_cancel` (2): A0 `minutes_to_wait * 60000`, 7 | `world.time + tick + 420000` (4) | PROVEN |
| every human entered | `has_entered_match` (3, event 18) -> `on_player_entered_before_match_started` (11) -> `all_players_ready` (13): disables `wait_before_cancel`, A0 `seconds_to_wait * 1000`, 10 -> `start_match_instantly` (15): A0 `minutes_per_match * 60000`, 45 | `T_enter + 10000 + 2700000` (5) | PROVEN |
| 2 of a 3-human mission entered, none left | ... (11) -> `2_from_3_players_ready` (12): A0 60 s -> (15) 45 min | `T_enter + 60000 + 2700000` (5) | PROVEN |
| a leave before the start leaves at most one human | `has_left_match` (4, event 19) -> `on_player_left_before_match_started` (16): `(has_left_match.players_count + 1) >= human_players_count` -> `cancel_match_instantly` (17) | `T_leave` (4) | PROVEN |
| 3-human mission, 2 entered, 1 left before the start | (16) -> `wait_before_cancel2` (14): disables (12), A0 5 min | `T_leave + tick + 300000` (4) | PROVEN |
| every human left | (16) -> (17) before the start; after it `on_player_killed_no_resurrect` (18) when no `can_resurrect` | (nobody to close) | PROVEN |

Tick rules (PROVEN, exe): the script steps only on grid steps (0x2003626E0); a wait (action 0, 0x2003C10AC) completes at
the first scripting step at or after its time, and the following actions run in that step; processors run in pve.lua
index order, so one enabled by a lower index runs in the same step and one enabled by a higher index in the next; events
18 / 19 are handled in the world step of their DF, so a leave in the step where `start_match_instantly` sets `started` is
still "before" (index 4 runs before 15); bots never count (record +0x2E0 ai id != -1, 0x2003C0078); the end is checked
after the rule ticks of the same world step (0x2002F3984). A start world holds the scene with 8 processors enabled and
none running (loader 0x2003F8D60), so `match` runs in the first step after the DE. The relay's backstop lies beyond the
script's latest end (45 minutes).

Lobby: a PvE match ended with winner 1 gets `99` with success 0 (CHOICE: the client's own failure
screen, HUD `victory = match_result == 1`), `83 45 0B` without chests (the failed run is stored: no daily key, never on
the top 100), then `85 00`; no refund, no leaver fine. Not for a mission nobody entered. OPEN: what the original lobby
sent (its match-server -> lobby link is not in the exe), and whether a `99` / `83 45 0B` sent after the relay's closing
DF still reaches the HUD (reader gates 0x20018ED84..0x20018EDAE / 0x2001ABD97 not traced through the close). The relay does not close at its
mirrored cancel / 45-minute end: it hands the end to the lobby first, so the `99` goes out while the match is loaded ("Result screen sequence" below). The
close-first path is left only for "every human left" and the hand-over's backstop.

### Mission end from the world host
The relay feeds every PvE match to the world host daemon (match-server `host.rs`, protocol crate
`world-host-proto`): `Create` (the `.srprec` head), each normal DF's unfiltered bytes, `Close` at the last normal DF (the
fin DFs never). The host's latched `Outcome` (world state 4 / 5) reaches the lobby while the match runs
(`OnlineMatches::take_outcomes`): `99` (success = winner team 0, the host's mission time, kills / deaths / score of the
statistics block, chests found = the humans' `pve_open_chest` event row 42) and `83 45 0B` at once; 3 s later the
relay's closing DFs with code 2, and `85 00` after every human's `5F 04` or leave, at most 120 s after the `99`
(`PVE_RESULT_SCREEN_CAP`, the lobby's timer; "Result screen sequence" below). The relay takes its winner from the
outcome (no report grace). The clients' reports wait up to 10 s
(`HOST_OUTCOME_WAIT`) for the host's outcome, are cross-checked against it (a disagreement is logged), and make the
result themselves only without it. No host, a failed host (2 catch-up restarts from the in-memory DF log, then off) or a
diverged host = the behaviour above. OPEN until a finished mission is recorded: the success value of a real extraction, the time the screen shows, row 42 = the HUD's chest count.

### Result screen sequence
What the exe does (PROVEN, static reading):

* A closing DF with the local player's leave bit ends the match (`close_current_match(3)`, DF handler 0x2001B0B76); with
  the DF code byte 2 (0x2001B0B7C) it skips the tear-down (UI clean-up, world unload, lobby return). Any other code
  (0xFF) tears down at once: the client is back in the lobby. `on_match_finished` 0x200132F90 closes the match connection.
* When pve.lua's action 26 ended the mission, the scripting rule's end hook (0x200362DC0) shows the victory container and
  gives the HUD the mouse pointer.
* The result screen's "Return to lobby" (`close_pve_stats_clicked`, 0x20019E167) does the skipped tear-down itself, then
  sends `5F 04` (0x2001973E0).

So the sequence is: `99` + `83 45 0B` while the match connection is up (the `99` reader gate 0x20018ED84 needs it, and
`on_match_finished` closes it), then the closing DF with the leave bit and **code 2**. The client keeps its ended world
with the victory container and the pointer. The player's "Return to lobby" unloads it and sends `5F 04`. Implemented:

* Lobby: `PVE_END_AFTER_RESULT` (3 s) after the `99`, `OnlineMatches::end_after_result` (srp: `Hub::end_match_after_result`).
  The `85 00` waits until every human still in the match sent `5F 04` or left, at most `PVE_RESULT_SCREEN_CAP` (120 s
  after the `99`).
* Relay: the closing DFs carry code 2 for a PvE match whose script ended: a host or mirror Outcome, a mirrored failure
  end, or the lobby's end after its result (the result came from the mission's end; the clients' reports 53 event 5 / 6
  are sent when their world ended). Any other end (a lobby end before a result, the ready timeout, every human left)
  stays 0xFF: no screen would show.
* The relay's own failure ends (pre-start cancel, 45-minute end) are handed to the lobby (`MatchConfig::pve_handover`,
  default 10 s). After `mirror_wait` with a live host, or at once without one, the relay gives an outcome with state
  4 / 5, winner 1 and no counters. The lobby sends `99` (success 0) and `83 45 0B` while the match is loaded, then ends it
  as above. Backstop: the relay closes by itself `mirror_wait + pve_handover` after its end step. A host Outcome after
  the hand-over is only logged (a disagreement as an error).

OPEN: what an `85 00` does while the result screen is open (kept until the screen is left: CHOICE); the meaning of the
scene event `0x4F + flag`; whether the unlock / reroll answers (`83 45 0B`, reader gate 0x2001ABD97 / 0x2001ABDB0) are
still taken after the code-2 close (the original server must have had the same order, so LIKELY yes, not traced); the
original server's delay between `99` and the close.

Top 100: the lobby movie's PVETop100Screen shows the players' `score` byte under its "chests_num" column. The lobby
sends the run's chests found (the world host's count, `PveRun.chests_found`) for every player of the entry. OPEN: for a squad, whether the column shows one
player's byte or a sum (the list row renderer is not in the traced movie code). The time column (`play_time`) was right.

### Reward messages and match length (exe)

* `5F 05` `pve_unlock_rewards` / `5F 06` `pve_reroll_rewards`: `5F` + one byte, no payload, no wait flag (0x2001974E0 / 0x200197460).
  No reply reader exists (C2 handles sub 0 and 3 only; `84 5F` is a refusal). The end screen's `rewards_data`, `reroll_cost`,
  `auto_open_chest` have no exe feed: NOT FOUND.
* `set_dropped_pve_items` <- `83 45 0B` (reader 0x200193DF0 -> 0x2001ABD80 -> Flash 0x2001AC0AB): `u8 n`, n x {`u32, u32, u16, u16 dict`,
  item tail when dict != 0}, `u32 cost`, `u8 flag`. What triggers it: NOT FOUND.
* Top 100: the client does not sort (reader 0x200197740, `LobbyMenu.set_pve_top_100`): the position is the server's row order. The
  per-player score is LIKELY the `chests_num` column.
* Match length: the client's phase clock is `parameters.match_time` (1800 s) of the mode file (0x2003653DC / 0x2003653FD); later
  adjustments only shorten it. pve.lua's 45 min is its own later timeout: the effective cap is LIKELY 30 min.

## End screen (chests found in a mission; code `servers/lobby-server/src/pve_end.rs`, protocol `Inventory45::PveDrops`)

Sources: the exe (the `83 45 0B` push, its reader layout, the 8 Flash arguments), the HUD movie's `PVEManager` and the 0.56 /
0.50 official texts. Labels: PROVEN (address / data), LIKELY (reason), CHOICE (server author's pick), OPEN.

### Wire
* **`83 45 0B` (PROVEN, reader 0x200193DF0 -> 0x2001ABD80 -> `root.set_dropped_pve_items` 0x2001AC0AB)**: `u8 n, n x item record, u32 cost, u8 flag`.
  The record is the common one (`u32 uid, u32 +4, u16 +8, u16 dict`, tail for dict != 0, reader 0x2001263D0): the unit test
  `pve_drops_83_45_0b_layout` checks the bytes against it. Modelled as `Inventory45::PveDrops { items, cost, flag }`.
  The client has no `45 0B` request (all 15 `45` builders read): it is a server push to the in-match HUD, so it only works while the
  client still is in the match (0x1ABD97 / 0x1ABDB0).
* `cost` = HUD `rerollCost` (price of the next reroll; the client sets `rerollAvailable = cost <= premium money`, 0x1ABF73), `flag` = HUD
  `hasLockedChests` (shows the "open all" button priced `unlock_drop_price`, HUD `unlockChestsCost`). PROVEN (exe argument order against the HUD signature).
* **The screen shows only after BOTH `99` and `83 45 0B` (PROVEN, `HUD.checkShowPVEContainer`).** The server sends `99` at the end of the match to every human player,
  with the stored run's match id (the client compares it with its running match, 0x18EDA1), before `85 00`;
  `pve::record_run_notify` pushes the `83 45 0B` (and the grant's inventory / money updates before it) at the same moment.
* `5F 05` / `5F 06` have no reply reader (PROVEN); the answer is a fresh `83 45 0B` plus `8B` money plus the inventory change `83 45 0F` (LIKELY: the only code that
  changes the screen is another `83 45 0B`). A second `83 45 0B` redraws the open screen (PROVEN, HUD movie: every
  `set_dropped_pve_items` runs `checkShowPVEContainer` -> `show_pve_match_end` -> `PVEManager.showPVEMatchEndScreen`, which calls `setData`
  on the screen it already has; `HUD.pveAutoOpenChest` is false for the first push and true after it, so from the second push on the
  rewards panel opens the chest by itself and shows the new rewards).

### Rules (stored in `pve_rewards`)
| Rule | Label |
|---|---|
| One stored row per chest found, per player of a won run (`chests_found`, capped by the mission's `chests_total` 10) | PROVEN data (chest count, 10 chests); who gets rows: LIKELY |
| The first `free_chests_num` (3) rows are opened at once (rolled through the chest tables, granted like an opened chest), the rest stay locked | PROVEN count + locked flag/button; "rest locked" = official 0.56 text |
| A lost run gives no chests; a player with score 0 gets none | LIKELY (0.50 text "when the mission is a success"; HUD `afk = own score is 0`, text `reject_drop_for_inactivity`) |
| Both get an empty `83 45 0B` (n 0, cost 0, flag 0) so the screen shows | LIKELY (the HUD needs `pveLootData` non-null) |
| Chest content = `pve_mission_1_chest_rank_1M`, M = difficulty + 1, at every equipment level | DATA + OFFICIAL: `chest_drops.options` of volumes 00 / 22 / 26 has all 15 tables `rank_11..53`; the newest volumes with the file (60, 67) removed exactly the 12 of tiers 2..5; the difficulty previews are the tier-1 chests 1483 / 1468 / 1469; 0.56 notes: "only 3 difficulty levels in PvE regardless of equipment tier" |
| One reward per chest (`items_count 1`), the roll is `game_data::chests::open` | PROVEN |
| `chests_premium_bonus_coef 1`: not applied (identity) | OPEN meaning |
| `5F 05`: one price `unlock_drop_price` (50 gold) for ALL locked chests of the latest run, once; refused (no change, no answer) without a run, without locked chests, without the gold | price PROVEN (HUD "open all" price); "once for all" PROVEN (one price shown); refusal handling CHOICE |
| `5F 05` pays ONE access key when the player has one, else the gold price | PROVEN: the HUD's `unlockChestsAvailableTokens` = keys the player may spend in the match (options player record +0x2D0) > keys spent (match player entry +0x54) (0x2001AC00F..0x1AC05B), token cost the constant 1; the in-match paid respawn uses the same fields (0x2003CF5A6..0x2003CF667: a key first while one is left, event 0x2B; else gold at the price record +0x2F8 within the budget record +0x2D4, spent gold at entry +0x58, event 0x2C; the respawn prompt 0x20023DE35 checks the same); the button shows "x1" with the key icon when a key is left, else the gold price, and sends the same payload-less `pve_unlock_rewards` either way. Which key: free 1617 first, then premium 1384 (the entry-fee order, CHOICE) |
| `5F 06` price = `reroll_start_price + reroll_step * rerolls done` on that run (50, 75, 100, ...); the same count is kept in every row of the run | start/step PROVEN (`pve_default_prices` +0x3DD4 / +0x3DD6, equal to `pve_reward_reroll_info [[50, 25]]`); the progression rule LIKELY (names; the client only shows the `cost` it is sent) |
| Reroll re-rolls the opened chests only; the old rewards are taken back and the new granted in one profile update | CHOICE |
| A reroll whose old reward cannot be taken back in full (equipped item, spent stack / money) is refused and charges nothing | CHOICE |
| Experience / reputation are taken back too; the silver paid for a level reached is not | CHOICE |
| Idempotent: a match id stored before does nothing again; rows keyed (account, match id, chest idx); a player only reads / changes his own rows of his latest run | CHOICE |
| "Latest run" = the highest `pve_results` id among the runs the player was in; if that run has no chest rows (lost, afk) `5F 05` / `5F 06` are refused | CHOICE |
| The first WON mission of a day opens every chest found; later won missions that day open `free_chests_num` (3) | PROVEN (0.69d client text on this screen: `PVEMatchEndRewards.setData` fills `pveTokensInfo` with `st_coop_mission_passage` "The first team mission you complete daily rewards you with all the stashes you find. Subsequent missions will provide 3 stashes." plus `st_coop_mission_lobby_msg_all_stashes` "At the moment you will receive all stashes you find." when `hasLockedChests` is false, else `..._avaliable_stashes` "... for free only 3 stashes."). The day = the UTC day the free daily keys count (`pve_daily_wins`, CHOICE) |

The chest rows and the profile change commit in ONE store transaction (`Store::pve_change_chests`), and only when the stored rows
are still the ones the change was computed from (a run end stored twice, or a second `5F 05` / `5F 06` racing the first, changes nothing).
OPEN: the HUD's use of `worth` beyond sorting.

Tests: protocol `pve_drops_83_45_0b_layout`; game-data (`chest(3, 0)` with the leading-slash name); `pve_end::tests` (fallback tier, taking rewards back exactly, equipped refusal,
screen message); lobby `tests/pve.rs` (`a_won_run_opens_three_chests_and_keeps_the_rest_locked`, `a_lost_run_and_an_inactive_player_...`, `unlock_opens_all_locked_chests_...`,
`reroll_costs_start_plus_step_...`, `the_end_screen_belongs_to_the_players_latest_run_and_own_rows`, `the_first_won_mission_of_the_day_opens_every_chest`);
accounts `tests/pve.rs` (`chest_rows_and_profile_change_in_one_transaction`).

## 10. Bots in an online mission (exe)

### 10.1 Who ran the bots: the match server (gates PROVEN, conclusion LIKELY-strong)
* A client cannot send a bot's input (`CD` carries only the local player's inputs, writer 0x20032EFD0) and never runs the AI
  online (the controller tick needs `options.single_player` +0x107E != 0, gate 0x20023A19B; pve.lua's AI actions test the
  scene's controllers pointer `scene+0x1BB10`, which only a local match sets, 0x2001F21A0).
* Yet `DF` carries `ai_player_input {u8 idx, u16 type, u16 param}` per player (applied 0x2001B0E2C), pve.lua needs it (10.2), and
  `game_world_core` has a server-mode byte (`W+0x180AD9`, always 0 in the client) and `create_scene` takes a `server_game_project`.
So the official match server ran the world, its own pve.lua scene with AI controllers and the 17 bots' AI, and relayed the bots'
`player_input` and `ai_player_input` in `DF`. Every client ran the same script without controllers.

### 10.2 Roster and bot life
* Options: 3 human records (`ai_id` -1) + 17 bot records = 20 = the `DF` player limit. Bot records must be named exactly `pve_bot_1` ..
  `pve_bot_17`: the scene start 0x2003C9430..0x2003C94E1 binds script names to record names (humans get `player_0..`). PROVEN.
* The server built them from the mode file's `scripting.pve_profiles[difficulty][tier][17]` (the exe never reads `pve_profiles`: the
  string is absent, PROVEN). Each record needs two profiles: `profile` (index 0: forest and sawmill bandits) and `profile1` (index 1:
  the army squad), plus the AI block, `hit_chance`, `hit_damage_multiplier`, `forgive_headshots`. The kits named there are defined in
  `gameplay/players/ai_players.options`. Index mapping LIKELY.
* Enter bits: the script spawns only entered players (0x2003CD87D..0x2003CD89C), so all 17 bot bits are set at the start tick (LIKELY;
  a local match enters everyone at the start, 0x2001F47AF).
* Spawns, deaths, corpse loot, encounter respawns, reinforcements: script actions every client simulates; the server sends nothing for
  them, but must mirror them to drive the right bot. No bot `profile_input` in PvE (whether one would trigger a rule respawn: OPEN).
* `ai_player_input` type 1 = a scripted animation: pve.lua (type 36) gives 7 forest bots `type 1, param = animation`; the simulation sets
  `player+0x74B` (0x20030164E) and `player_logic_script_state` (vtable 0x2009A30A0, slot 0x2004091E0) plays `param`, then raises event 16;
  type 0 / param 0 ends it (type 1 pauses the AI and drives the animation). PvE bots never speak
  (`sound_scheme silent`), so no type 2 entries.
* Display names: script type 31 sets `st_botN_ah_name`; whether it or the record's name indexes win: OPEN.

### 10.3 Difficulty (options +0x1082)
| | easy | medium | hard |
|---|---|---|---|
| kits (`profile` / `profile1`) | `1lvl_*` / `2lvl_*` | `2lvl_*` / `3lvl_*` | `4lvl_*` / `5lvl_*` |
| vision fov / range | 30 deg / 40 m | 60 / 80 | 90 / 200 |
| hearing crouch / walk / sprint / shot (m) | 5 / 20 / 30 / 75 | 10 / 30 / 40 / 100 | 15 / 40 / 80 / 300 |
| AI think period | 100 ms | 100 | 50 |
| hit chance / damage multiplier / forgive headshots | 50 / 0.5 / yes | 75 / 1.0 / yes | 100 / 1.25 / no |
| damage / bleeding resistance (pve.lua type 47) | 0 / 0 | 0.15 / 0.25 | 0.4 / 0.5 |

Hit chance, damage and headshots are read by every client's hit code (0x200302F60..0x200303036); the rest is server AI. PROVEN (data).
pve.lua reads `game_difficulty` nowhere else and never reads `equipment_level`. The five equipment groups of `pve_profiles` are identical.

### 10.4 What the server sends, what the clients compute
* Server (LIKELY for the original): the `DD` options (20 records; +0x1082, +0x1081, `single_player` 0, `match_id` = world seed, project; human budgets +0x2D0 /
  +0x2D4 / +0x2F8), enter bits, each alive bot's `player_input` when it changes, `ai_player_input` when its script runs type 36, the
  relay of human inputs, and at the end `99`, `83 45 0B`, `85 00`.
* Every client: spawns, deaths, loot, doors, sensors, stashes, anomalies, paid respawn (0x2003CF4C2..0x2003CF667), the 15 s resurrect
  window, cancel / wait rules, the mission end. A weak server model cannot desync the clients (all apply the same inputs); it only makes
  the bots play worse.

### 10.5 Results
Clients report only won / lost (`53` events 5 / 6). Stashes found, mission time and per-player score / kills / deaths live in each
client's statistics object (pve.lua type 46 per stash and objective) and in no client message; the server takes them, and the
paid-respawn charges (`entry+0x54` / `+0x58`), from the world host (section 9, "Mission end from the world host").

### 10.6 OPEN
1. Byte-exact path of the controllers pointer from world creation (+0x38) to `scene+0x1BB10`.
2. Whether a bot `profile_input` triggers a rule respawn in PvE.
3. Which display name the HUD shows (type 31 or the name indexes).
4. Map variant v1/v2/v3 for a difficulty (no data link; collision differs, so the bots' line of sight differs).
5. Exact player-logic transition for type 1 (0x200381A10 / 0x200381A90 in the builder 0x20037D7E0); the animation index space of `param`.
6. RNG of the lootbox shuffle and the other random script expressions.
7. Event enum names beyond those used.
8. `A16 player loopback_0 profile 2` (looks like a no-op).
9. Whether the original server set the bot enter bits at the start tick or at the first script spawn (start tick LIKELY).

## Protocol audit

See `protocol-audit.md`: the `99` wire header order is unused, success, found, total, n (section 4.4 gives the memory order); `49 05`
second byte 0xFF for the lock-slot button.
