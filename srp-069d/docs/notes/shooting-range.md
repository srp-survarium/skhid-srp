# Shooting range (`5B 03`, `goto_shooting_range`)

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

A local bots match gives no item reward of any kind: no drops and no free treasure-hunt card (`MatchFacts.local_bots`, `hunt::record`); the quest / achievement / league counters stay as documented (OPEN).

Status words: PROVEN = read in the 0.69d exe or its game data (address / file given); LIKELY = inferred; CHOICE = a
server rule with no client evidence; OPEN = no evidence found. Static analysis only: nothing was run against the real
client. Addresses are 0.69d exe VAs (image base 0x200000000).

## What the client sends (PROVEN)

`goto_shooting_range` is the Flash callback of the inventory's "test the weapon" button (string 0x20097A338, handler
`lobby_menu::process_goto_shooting_range` 0x2002CAD20). The handler finds the item (an inventory item by uid, else a
fresh item built from the dictionary id the Flash side passes, 0x2002CAF55) and calls the sender 0x2001970D0, which
writes `5B`, `u8 03`, an **item record** (the weapon to test, with its modules; writer 0x200198A70) and one more `u8`,
copied from the client state byte `L+0x72501E` (0x20019717A). That byte is the selected profile index: the account's own profile table
stores its index there when a `48 02` answer names the current profile (0x200192CDD). The protocol crate decodes it as
`Play5B::ShootingRange { weapon, tail }`. The request sets no "waiting for the server" flag (unlike 5B 01 / 5B 02), so the
Play button is not blocked by it.

## What the client expects back (PROVEN structure, LIKELY mapping)

* `83 5B <u8 sub> <u32 packed size> <packed options>`: the same body for every sub. The handler 0x2001F4070 unpacks it,
  reads the match options (reader 0x2001250E0), stores the sub in `nc+0x7A9ADC` and starts the level load with the project
  string of the options (0x20011B5F0). Only the sub value 2 is special in the code read (tutorial hooks 0x2001F4A80 /
  0x2001F5302); 1 and 3 behave the same. So the range is a **local match** (the
  `base_local_network_client`), with the sub echoed.
* An answer with packed size 0 makes the unpack fail, so no level is loaded and the range button does nothing.
* Before the `83 5B` the lobby sends `85 02` (queue state 2), exactly as for Training: the load-complete epilogue
  checks the queue state. LIKELY for the range (same loader).

## The options (PROVEN data)

`gameplay/game_modes/shooting_range.options` (volume 75 of 0.69d):

| key | value |
|---|---|
| `mode.level_name` | `shooting_range` (the project string of the blob) |
| `match_making_params` | `map_id 0`, `map_weather_id 1`, `map_name "shooting_range/test"` (the map string of the blob) |
| `mode.mode` | 1 |
| `parameters` | `match_time 0` (no time limit), `score 200`, `tickets 200`, `countdown_time 5`, `local_game_countdown_time 5` |
| `rules` | 109, 103, 111, 102, 99, 107, 148 (rule ids, meaning not decoded) |
| `scripting` | none (the tutorial's file has a script and nine scripted bots; this one has neither) |
| `ui` | `mode_name st_match_settings_bots`, loading screen `shooting_range` |

The level files exist in the volumes: `projects/shooting_range/test/client_project`. The blob is built by
`local_match::shooting_range_options` with the same writer as the tutorial and Training (byte layout PROVEN and
golden-tested for those two): `project = shooting_range`, `map = shooting_range/test`, nine players (the human and eight AI
players), `single_player = 1`, the three trailing tables. A unit test pins the two strings, the player count and the bots'
team.

### Bots on the range

* The level has 8 respawn points with `team 1` and `use_for_ai true` and 8 team-1 patrol graphs: six posts where a bot
  stands (two crouch) and two 4-point walking loops. The human's point is `team 0`, `use_for_ai false`, at least 16 m from
  the nearest bot point (PROVEN, `projects/shooting_range/test/client_project`). So the server sends 8 AI players, all on
  team 1, with the same records as Training (the count is in no client file: LIKELY one bot per point).
* They are targets and never attack the player (PROVEN in the exe): for a local match of kind 3 the client sets its
  bots-ignore-players byte at match start (0x2001337E2 / 0x200133817; reset by `close_current_match`), so sight
  (0x20032CAC0), sound (0x200368B50) and the enemy update (0x20036C320) skip the human; bots of one team never choose each
  other as enemies, and the fire check refuses a shot with a team mate in the line (0x20036D450).
* OPEN: whether a bot the player wounds fires back: the enemy selector's damage memory (last 3000 ms, 0x20036AF10) has no
  ignore check. If it does, the fallback is range bots without ammo.
* Bots respawn by the level's rule 99; the range has no end rule and no time limit.

## The kit (CHOICE)

The client names only the weapon; the exe sends no slot. `servers/srp/src/matches.rs::range_loadout` builds the human from
the account's own loadout and puts the tested weapon into the weapon slot of its kind: pistols (`wpn_pstl`) take slot 10
(ammo slot 9), every other weapon slot 7 (ammo slot 8). An owned weapon keeps its saved upgrades and modules. A weapon the
account does not own (the shop's test button) becomes a temporary item with the upgrades and modules of the request record.
If the ammo in the slot does not fit the weapon, the first ammo type of the game data's compatibility list replaces it, loaded
with one clip (the `clip_size` of the data). The match gets the usual ammo storage. Nothing is saved: the profile, inventory
and slots stay as they were. A record that is not a weapon leaves the kit alone.

## The end (CHOICE, evidence for the rule)

`match_time 0` means the range has no end of its own: the player leaves through the client's exit menu. The lobby treats the
first `53` event flush more than `match_min_time` after the start as the end (the same rule as Training and the tutorial) and
answers `85 00` (queue state 0) so the Play button returns, and the friends' status goes back to "in the lobby".

The range **pays nothing and builds no results screen** (CHOICE): its data file has no reward events, no objective and an
endless timer, no `97` is read for it, and a weapon test is not a game of the account. No boosters are charged, no quest,
achievement or statistic is counted, the first-game bonus stays unspent. If the client sends no `53` flush when it leaves
the range (OPEN: no recording of a real range session exists), the peer stays marked as "in a local match" until the next
Play / range request overwrites it; nothing else depends on it.

## Tests

`servers/lobby-server/tests/lobby.rs::shooting_range_is_a_local_match_that_pays_nothing` (85 02, `83 5B 03` with the hook's
blob, early flush ignored, end with `85 00` only, profile unchanged), `servers/srp/tests/matches.rs` (the kit rules, the blob
names the range level), `crates/local-match` unit test (strings and layout).

## Checks in the game

Inventory -> select a weapon -> test button: the shooting range level loads (loading screen "shooting range"), the player
stands with the chosen weapon and ammo among eight bots (six standing, two walking) that never attack; leaving the range
returns to the lobby with Play usable and no money, experience
or results change. Watch: does the level load without a patrol-graph wait (the tutorial's `level_name` path is the same), does
the weapon fire with its modules, and does the client send anything when it leaves (the server log line
`shooting range left` shows the flush).

## Leaving the range (exe)

* The in-match `leave_match` (0x200145DD3) runs `close_current_match(2)` (0x200145E95), which sends nothing for the
  range (`CE` only for an online match, 0x2001F53E1); it pushes tracker marker 0x41, dropped unless `B8` whitelists it; then game
  state 2. LIKELY a `53` follows from the lobby-loaded markers (same path as a Training exit), so the server's "first 53
  after the start ends it" rule stands. Adding 0x41 to `B8` would make the exit explicit (not done: unproven effect).
