# Artifact Hunt as 0.46c2 does it (read from its executable)

Read-only reverse engineering of `survarium.exe` of build 0.46c2 (2026-11-21), for a re-implementation
in 0.26g. Nothing here was played; every claim is from the code or the build's configs.

- **PROVEN**: read in the code at the given address. **LIKELY**: inferred, the reason is given.
- Addresses are VAs at the preferred image base 0x10000 (as in `tools/re-kit`), of the **decoded** exe.
- The exe on disk (`.../v046c2/out/game/binaries/x86/survarium.exe`, sha256 `29f65e7b...`) still has the
  updater's x86 BCJ filter on it. Decoded copy (lzma `FILTER_X86`, start_offset 0): sha256 `e2daf269...`,
  97.98 % of its E8 calls land on function starts. All addresses below are of that copy.
- The exe holds both the simulation classes (`*_core`) and the client's subclasses (`artefact_hunt_rule`
  over `artefact_hunt_rule_core`, `victory_artefact` over `victory_artefact_core`). RTTI is intact;
  class names below are the exe's own.
- Times: configs are in seconds, the rule keeps milliseconds (x 1000). "now" is the world time
  `world+0xBC01C`; the match start is `world+0xBC064` (set when the match countdown ends, 0x7B0CF0).

## 0. Summary

Two teams, one artefact in the world at a time. It appears at a random spawner 50 to 70 s after a round
starts, anyone takes it with the use key, the carrier is slowed and marked, and brings it to his own
team's container (use key again): +1 for the team. Each delivery ends the round: 3 s, a 1 s fade,
everyone respawns at the spawns, 5 s frozen countdown, next round. First to `score` (5) wins; at
`match_time` (900 s) the leader wins; equal: 180 s of sudden death (no respawns, a wiped team loses,
a delivery wins), then a draw.

## 1. The rule classes (`mode.rules [109, 103, 149, 111, 97, 107]`)

Each id is a resource class id a cook registers itself with (the id is the dword at cook+8; found by
scanning every static cook constructor).

| id | class (RTTI) | cook | what it reads of the mode config |
|---|---|---|---|
| 109 | `player_controls_rule` | `simple_rule_cook<player_controls_rule>` | `mode.controls.*` (killcam times, `preview_camera`), ctor 0x6560C0 |
| 103 | `kd_stats_rule` | `simple_core_rule_cook<kd_stats_rule>` | nothing found (no config key in its functions) |
| 149 | `timeout_anomalies_rule_core` | `timeout_anomalies_rule_core_cook` 0x7055C0 | `mode.anomalies.*` (all six keys), see 8 |
| 111 | `change_profile_rule_core` | `simple_core_rule_cook<change_profile_rule_core>` | nothing found |
| 97 | `artefact_hunt_rule_core` (client: `artefact_hunt_rule`) | `artefact_hunt_rule_core_cook` 0x73F210 | see below |
| 107 | `winner_spawn_rule` | `winner_spawn_rule_cook` | `mode.victory_screen.*` (0x6A3810) |

PROVEN for the ids and classes. The same scan gives Team Deathmatch's 98 `tdm_rule`, 99
`player_respawn_rule_core`, 102 `timelimit_rule`, and Research's 105 `capture_the_point_rule`.
Artifact Hunt has **no** `timelimit_rule` and **no** `player_respawn_rule`: rule 97 does the waiting,
the countdown, the match time and the respawns itself.

What rule 97 reads (all from `mode.parameters`, PROVEN):

- constructor 0x7839F0: `wait_player_percent`, `match_time` (rule+0x200), `time_needed_to_continue_match`
  (+0x204), `draw_additional_time` (+0x22C), `time_before_new_round` (+0x208), `round_countdown_time`
  (+0x20C), `score` (+0x210, one byte), `respawn_time` (+0x214), `time_needed_to_respawn` (+0x218);
- cook 0x73FC30 (builds the state machine): `wait_time` (**minutes**: x 60000), `countdown_time`
  (`local_game_countdown_time` instead in a local game, world static +0x97D6A), `vi_spawn_time`,
  `vi_spawn_variation`, `vi_not_picked_ttl`, `vi_dropped_ttl`, `time_before_new_round`,
  `round_countdown_time`;
- cook 0x73F730: the map's `server_objects.respawn_points` (by `team` 0/1), `victory_item_spawners`,
  `victory_artefact_containers`; the item `gameplay/victory_items/artefact.options` (class 96,
  `victory_artefact_core`), requested at 0x73F409;
- the client cook 0x5D3560: `mode.sound_notifications` (`base_path` and the seven names).
- Not read by this exe: `play_time_for_drop_in_s` (the string is not in the exe). `min_respawn` /
  `max_respawn` are read only by 0x7886F0 (the ordinary respawn rule's code), not by rule 97.
- `match_time` can be shortened by a start option (min of the two, 0x783CA2..0x783CCF). LIKELY a
  developer switch.

## 2. The state machine

Rule 97 is an `ai::fsm` of eleven states (`rule+0x1A8`; current state `rule+0x1B8`). A state is
`{+0x18 rule, +0x1C id, +0x20 flags, +0x24 timeout ms, +0x28 start time}`. Built at 0x73FD86..0x740125,
transitions added at 0x740332..0x740D64 (PROVEN). Each tick (0x844F70) the current state's transitions
are tried **in the order below**; the first true one is taken (old state's finalize, new state's
initialize), at most one a tick.

"timeout" is 0x785DF0: `now >= state.start + state.timeout`.

| id | state (RTTI) | flags | timeout | transitions, in order |
|---|---|---|---|---|
| 0 | `waiting_for_players_state` | 4 | `wait_time` min | enough players (0x785D80) -> 1; timeout -> 9 |
| 1 | `match_countdown_state` | 5 | `countdown_time` | timeout -> 2 |
| 2 | `waiting_for_artefact_state` | 3 | random, see 3 | match over (0x785E10) -> 10; timeout -> 3 |
| 3 | `artefact_spawned_state` | 3 | `vi_not_picked_ttl` | match over -> 10; has a carrier (0x785E70) -> 4; artefact not in the world (0x785EA0) -> 2 |
| 4 | `artefact_hunt_base_state` (client: `artefact_picked_game_state`) | 3 | none | match over -> 10; delivered this tick (0x785E90) -> 6; no carrier (0x785E80) -> 5 |
| 5 | `artefact_dropped_state` | 3 | `vi_dropped_ttl` | match over -> 10; has a carrier -> 4; artefact not in the world -> 2 |
| 6 | `artefact_brought_state` | 3 | `time_before_new_round` | timeout and match decided (0x785EC0) -> 10; timeout -> 7 |
| 7 | `transition_to_new_round_state` | 0xB | 1000 ms (constant, 0x740000) | timeout -> 8 |
| 8 | `round_countdown_state` | 7 | `round_countdown_time` | timeout -> 2 |
| 9 | `finished_state` | 3 | none | none (world match state 4) |
| 10 | `finished_successfully_state` | 3 | none | none (world match state 5) |

- Flag bit 2 (value 4): every player's input command is zeroed while it is set. PROVEN: the state's
  initialize (0x7B0BA0, base 0x741030) writes `!(flags >> 2 & 1)` to each player slot's byte +0x3A, and
  the player's tick 0x72C0D0 (at 0x72C1CE) clears the input when that byte is 0. So players stand
  frozen in states 0, 1 and 8.
- Flag bit 3 (value 8): the client fades the screen (see 7). Bits 0 and 1 are client-only edges
  (0x69F870); what they switch was not traced.
- World match state `world+0xBC060`: 1 waiting for players (0x7B0C20), 2 countdown (0x7B0CD0), 3 running
  (0x7B0CF0), 4 / 5 finished. `rule_completed` (vtable slot 22, 0x7850F0) is true for 4 and 5 and
  reports success for 5. PROVEN.
- "Enough players" 0x785D80: popcount of `world+0xBC054` >= `rule+0x1FC`, which the constructor sets to
  `n + floor((max_players - n) * wait_player_percent + 0.5)` with n the slots whose +0x7964 is not -1
  (0x783BB8..0x783C74). PROVEN for the arithmetic; LIKELY that n is the pre-filled (bot) slots.

## 3. The artefact's life

There is exactly **one** artefact object for the whole match (`rule+0x190`, a `victory_artefact_core`
made by the cook; PROVEN, no array, no second one anywhere in the rule).

- **First spawn.** The match countdown ends (state 1 -> 2, the match time starts), then state 2 waits a
  random time and state 3 spawns it. PROVEN.
- **The wait** (0x7B0DC0): `timeout = lo + ((hi - lo) * rng) >> 32`, with
  `v = min(vi_spawn_time, vi_spawn_variation)`, `lo = vi_spawn_time - v`, `hi = vi_spawn_time + v`
  (0x73FCDA..0x73FD0D). With the shipped 60 and 10: uniform in **[50 s, 70 s)**, in ms. PROVEN.
- **The rng** is the rule's own 32-bit state `rule+0x1F8`, seeded from the match's static data
  (world static +0x97D64, 0x783ADE): `s = s * 0x08088405 + 1`, value = high dword of `s * range`. The
  same state also shuffles respawn points (0x786070). PROVEN.
- **The spawner** (0x7B0E90): `index = (rng * spawner_count) >> 32`, uniform over all
  `victory_item_spawners`; the artefact is put at that spawner's `spawned_item` position and rotation
  (spawner entry +0x18; without a `spawned_item` block, at the spawner's own position plus a fixed
  offset, 0x712100). PROVEN.
  - `priority` is parsed into the entry (+0x58, 0x712100) but **nothing in rule 97 reads it**; `point_id`
    is not read at all by the spawner parser. PROVEN (the only reader of the spawner array is 0x7B0E90).
  - Nothing remembers the last spawner: the same one can come twice in a row. PROVEN.
- **Not picked up**: on spawn the artefact's deadline is `now + vi_not_picked_ttl` (artefact+0x200,
  0x7B0EF9). The artefact's own tick (0x7980F0, runs only while it lies in the world) removes it from
  the world when `now >= deadline` (0x7981EB). The rule then goes 3 -> 2: a new random 50..70 s wait and
  a new random spawner. PROVEN.
- **Dropped**: entering state 5 sets the deadline to `now + vi_dropped_ttl` (0x7B0F20); the full time
  again on every drop. At the deadline it is removed the same way, 5 -> 2. PROVEN.
- **After a delivery** the artefact is in nobody's hands and not in the world. The next one comes after
  `time_before_new_round` + 1 s + `round_countdown_time` + the random 50..70 s (states 6, 7, 8, 2).
  PROVEN.
- On the client the removal shows "Artifact destroyed" (`st_on_destroyed_victory_artefact`, 0x6E17D3)
  and the spawn "Artifact has spawned" (`st_on_spawn_victory_artefact`, 0x6E16AA).
- The client object has spawn / idle / destroy particles and durations (`data.spawn_duration` etc.,
  0x6E11D0); the simulation has no spawn delay. LIKELY visuals only.

## 4. Picking up, carrying, dropping

- **Pick-up is the use key** on the artefact (prompt `st_pickup_item` "Pick up ({USE})", 0x797ED0), not
  touch. PROVEN: the artefact is a usable object (interface at artefact+0x10, vtable 0xB519D4).
- **Who**: anyone, either team; `can_use` 0x797E00 has no team test. It needs: nobody else using it at
  that moment (+0x2C == 0), and a clear line from the player's eye (player+0x82C) to the artefact (a
  ray query whose first hit must be the artefact itself). PROVEN. The reach of the use key is the
  generic usable system's and was not traced; the artefact's `usable_shape` is a 0.3 sphere.
- **No take time.** `use_execute` 0x797EF0 attaches it in the same call: carrier = player,
  `player.items+0x1F0` = artefact, modifiers on, event "picked up". PROVEN for this code path (the
  battery's take / carry / put states, `victory_item_core_take_state` etc., are not used by the
  artefact class, and `artefact.options` has none). LIKELY instant on the key press (the generic use
  flow around it was not traced).
- **One at a time**: only one artefact exists.
- **The carrier's modifiers** (0x798830 on, 0x7988E0 off; PROVEN):
  - movement speed (player parameter 3): `movement_speed_modifier * max(0, 1 + p)` where p is the
    player's `victory_item_speed_modifier` (parameter 105) with its modifiers (0x7477D0). Shipped -0.2:
    20 % slower, less with a skill that lowers parameter 105;
  - health regeneration speed (parameter 1): + `health_regeneration_speed_modifier` (0.05).
- **Cannot sprint.** `base_player::can_sprint` 0x72D700 requires `player.items+0x1F0 == 0`. PROVEN for
  the test; the function is identified by its callers (0x706CD0 / 0x7070E0 test input bits 0x200 =
  sprint keys and 1 = forward, per the input mapper 0x688360; `player_logic_stand_jump_state`).
- **Keeps his weapons** and can shoot, aim, reload: the artefact never becomes the active object
  (player+0x1B8), unlike the battery, and its vtable slot 5 (0x432800) returns 0 where the battery
  returns its hands animations. LIKELY (no restriction found; not every reader of +0x1F0 was read).
- **A visible effect on the carrier**: `data.effect` ("artefact_wood_shine") is added to the player's
  effects on pick-up (0x798960) and removed on drop. LIKELY a glow; the effect itself was not read.
- **Shown to both teams.**
  - World player icons (0x61A470): the list holds every living teammate and, in mode 4 only, also any
    living **enemy who carries** the artefact; each icon has the Boolean `carry_artifact` (0x61A8B9).
    PROVEN.
  - The player list entry has `carry_item` true for the carrier (0x6180DB, flag set at pick-up for the
    carrier of either team, 0x6E17ED). PROVEN.
  - The player markers' update names its per-player flag `artifact_visible` in mode 4 (0x6D2330).
    PROVEN for the name; which value feeds it was not traced.
  - The carrier himself gets `root.show_artifact_carry(true)` (0x61BAC0) and his own base highlighted
    on the minimap.
- **Voluntary drop exists**: the "Drop" key (`st_drop_action`, action 0x35 -> input bit 0x400000,
  0x688360) drops the carried item (0x7269CD in the player's action handler 0x726820). PROVEN.
- **Dropped on death** (`base_player` vtable slot 9, 0x72BE90 at 0x72BF92) and when the player's items
  are reset, e.g. he leaves (0x74D190). PROVEN.
- **Where it lies**: at the carrier's transform (his feet) plus `collision_shape.position` (0, 0.13, 0),
  upright, with zero velocity; then it is a rigid body (mass 10) and may roll. PROVEN (0x797320).
- **Who can take a dropped one**: anyone, same rule as a fresh one (same `can_use`). PROVEN.
- Killing the carrier gives no `victory_item_carry_kill` award: the kill code asks the carried item's
  vtable slot 5, which is 0 for the artefact (0x7284AC). LIKELY (the flag's use in the statistics was
  not followed to the end).

## 5. Delivery, rounds, respawn

- **Delivery is the use key** on the team's `victory_artefact_container` (prompt `st_put_item`
  "Put ({USE})", 0x7B0B30). `can_use` 0x7B0AF0: the container's `team` equals the player's team and he
  carries the artefact. `use_execute` 0x7B0B70 puts it at once: no put time. PROVEN. The container's
  usable shape is its `collision_geometries`.
- The rule expects **exactly two** containers (index 0 and 1, 0x200 bytes each, `rule+0x19C`). PROVEN.
- **Score**: the put sends event 3 / kind 3 with the carrier's team; `on_event` 0x7840E0 adds 1 to that
  team's byte (`rule+0x211` team 0, `+0x212` team 1) and sets "delivered this tick" (`+0x213`). PROVEN.
- **Award events** (0x7B2700, the statistics' victory-item event manager): the deliverer gets event 16
  `put_victory_artefact_in_container`; everyone of the team who carried it towards the base gets event
  35 `bring_victory_artefact`, in hundredths by his share of the way (0x7B29F0, 100 x share); event 34
  `bring_victory_artefact_singly` if no teammate had a share (0x7B2960). PROVEN. Their weights in this
  build's `gameplay/awards/events.options`: bring 50 score / 25 exp / 100 money / 25 reputation
  (`reward_factor` 100), put 10 / 4 / 5 / 2, singly 0.
- **Yes, there are rounds.** After a delivery (state 6):
  1. `time_before_new_round` (3 s): play goes on, but respawns stop (`rule+0x220 = 0`, 0x7B0F70). The
     HUD shows "Round restarts in" with the time.
  2. If the match is decided (see 6) it ends here. Otherwise state 7 for 1 s: the screen fades, and
     0x785C40 marks **every** player, alive or dead, for respawn now.
  3. State 8 for `round_countdown_time` (5 s): respawns on again (`+0x220 = 1`), so everybody is
     respawned at his team's spawn points next tick (living players too, 0x785440..0x78547B), frozen,
     with "Awaiting match start: 00:05". Its initialize 0x7B1020 also empties a world list
     (0x799860) and visits every item of every player. LIKELY: thrown / placed things are removed
     and weapons reset; the visitor's bodies (0x7B1200..0x7B1350) were only skimmed.
  4. State 2: the 50..70 s wait for the next artefact.
- The scores, the match time and the rng carry over; nothing else is reset. PROVEN.
- **`time_needed_to_continue_match`** (90 s): after a delivery a new round starts only if at least this
  much match time is left; otherwise the match ends (scores differ) or goes to sudden death (equal).
  PROVEN (0x785CF0, 0x78519B).
- **Respawn during a round** (tick 0x785130, PROVEN): a dead player comes back at
  `max(death + time_needed_to_respawn, wave + m)`, where `wave` is the first multiple of
  `respawn_time` (15 s, counted from the match start) at or after `death + time_needed_to_respawn`
  (7 s), and `m = round(respawn_time * respawn_time_modifier)` of the player (parameter 59), rounded
  up to the 50 ms grid. So: waves every 15 s, never sooner than 7 s after death. Spawn points are the
  team's free `respawn_points`, shuffled with the rule's rng and ordered by nearby enemies (0x7858C0,
  the ordinary respawn rule's logic). A player entering the match spawns at once (0x7842A0).

## 6. The end

- **Win at `score`**: tested only in state 6, when `time_before_new_round` has passed (0x785EC0 ->
  0x785CF0): a team's score equals `score` -> state 10. So the match ends 3 s after the fifth delivery.
  PROVEN.
- **`match_time` runs out** (in states 2, 3, 4, 5; 0x785E10):
  - scores differ: the match ends at once, the higher score wins. PROVEN.
  - scores equal: **sudden death** (`rule+0x224 = 2`, start time `+0x228 = now`, 0x78517D). PROVEN.
- **A delivery with less than `time_needed_to_continue_match` left**: scores differ -> the match ends
  after the 3 s; equal -> sudden death is armed (`+0x224 = 1`, its start set to
  `now + round_countdown_time + time_before_new_round`), a new round is played and the transition
  state turns it on (1 -> 2, 0x7B1004). PROVEN.
- **An edge of the original**: the "less than 90 s left" test for arming sudden death is made in the
  tick of the delivery (0x78519B), the one for ending the match 3 s later (0x785CF0). A delivery that
  **equalises** the score with between 90 and 93 s left arms nothing, and 3 s later the match ends
  with equal scores: a draw without sudden death. PROVEN by reading both tests; not played.
- **Sudden death** lasts `draw_additional_time` (180 s) from its start:
  - no respawns: a death scheduled while it is on gets no respawn time (0x785704). Players whose
    respawn was already scheduled before it began still come back once. PROVEN.
  - each death clears the player's bit in `rule+0x380` (0x78423A); the teams' remaining counts are
    `+0x388` / `+0x389`. When a team has nobody left and the scores are equal, **the other team gets
    +1** (0x785F80), which ends the match. A player leaving counts the same (0x784350). PROVEN.
  - a delivery makes the scores differ and ends the match (after the 3 s).
  - 180 s without either: the match ends with equal scores.
- **The result** is the statistics' "leading team" byte (handler+0x590C, 0x785770): 0 or 1 for the
  team with more points, **2 = draw** for equal. PROVEN. The end text is `st_timelimit_completed` when
  the time test is true, else `st_artefact_hunt_completed` (0x785EF0; "Time is out" / "All artifacts
  collected").
- Not enough players within `wait_time` (5 min): state 9, world state 4, no result. PROVEN.

## 7. Sounds and the HUD movie

### Sound notifications (client rule tick 0x69F870, PROVEN)

Played once when the state id changes, never for a viewer without a team (local team byte 3). The sound
is `<base_path>/<voice language>/<file>` ("vo/<lang>/artifact_hunt/art_spawned"; language "original"
plays "russian", 0x5D35B4). A playing notification is stopped first (0x69FA40). Index = position in the
exe's name table 0xB2E540.

| new state | index, name | condition |
|---|---|---|
| 3 spawned | 0 `artefact_spawned` | always |
| 4 picked | 1 `artefact_picked_up` / 2 `artefact_picked_up_by_enemy` | carrier's team == / != mine |
| 5 dropped | 3 `artefact_dropped` / 4 `artefact_dropped_by_enemy` | last carrier's team (artefact+0x300) == / != mine |
| 6 brought | 5 `artefact_brought` / 6 `artefact_brought_by_enemy` | last carrier's team == / != mine |

A despawn (ttl) has no sound. Also: when less than 30 s of match time are left and no sudden death is
on, music track 3 starts with a 25 s fade (0x69F7D0 -> 0x5ADC10); it stops when sudden death begins.

### HUD calls (Scaleform `Invoke` by name; GFx types: Boolean, UInt, Number, String)

The rule fills the match hud state (0x784EF0 core, state part via the state's slot 7); the HUD object
keeps a copy at hud+0x24 and 0x61CB60 turns it into calls, each only when its value changed.

| call | arguments | when / source |
|---|---|---|
| `root.set_match_mode` | UInt 4 (`mode.mode`) | HUD init 0x6136F0 |
| `root.set_tickets_required` | UInt `score` (5) | HUD init; modes 1, 2 and 4 use this, **not** `set_artifacts_required` (that is mode 0) |
| `root.set_tab_message` | String `st_start_match_welcome_message_7` | HUD init (0x613DB2) |
| `root.show_pregame` / `root.set_pregame` | Boolean / String "<text> MM:SS" | states 0, 1, 8: `st_waiting_for_players`, `st_final_countdown`, `st_final_countdown` with the time left (0x7B0C80, 0x7B0D70, 0x7B1180) |
| welcome message | `st_start_match_welcome_message_7`, 6000 ms (0x618CF0, `root.set_parameterized_message`) | first update after the pregame ends |
| `root.set_game_progress` | UInt score team 0, UInt score team 1, UInt colour 0, UInt colour 1 | every change (0x616410); colour 0xFFFFFF own team, 0xFF0000 enemy (0x622800); a client option swaps the pairs so the own team is first |
| `root.set_match_time` | String "MM:SS" ("" at 0) | `match_time - elapsed`; in sudden death `draw_additional_time - elapsed since its start` (0x78503E..0x785084) |
| `root.show_round_restart` | Boolean | true in state 6 when a new round will follow (`!0x785CF0`), 0x616D70 |
| `root.set_round_restart_time` | String "MM:SS" | state 6, what is left of `time_before_new_round` (0x7B0F90, 0x616E00) |
| `root.show_fade_animation` | Number 1.0 (s), Boolean true | entering state 7 (flag bit 3), 0x612F60 |
| `root.set_last_players` | UInt own alive, own total, enemy alive, enemy total | modes 1 and 4 (0x6165F0); the alive counts are non-zero only in sudden death (`rule+0x388/9`) |
| `root.show_game_message` | String text, UInt 4000, Boolean enemy | spawned (all), "You picked the artifact" (the picker only), delivered: own / enemy text with the flag true for the enemy's, destroyed (all); 0x6E1690, 0x6E16C0, 0x6E1780 |
| `root.show_artifact_carry` | Boolean | mode 4 and the viewed player carries it (0x61BAC0) |
| `root.update_objects` | Array of `{id, type, pos_x, pos_y}` (pos_y = -z) | modes 0 and 4 only (0x6D2EB0): own base `base` / `base_highlighted` (when I carry), id 1000 + team; `base_enemy`; the artefact `minimap_match_artifact`, id 4000, **only while it lies in the world** (spawned or dropped) |
| `root.set_respawn_time` | from hud state +4 (total wait) and +8 (respawn time) | the local player's entry of `rule+0x280` / `+0x230` |
| `root.list_update_player` | object with `carry_item` Boolean, ... | player list |
| player icons | object with `carry_artifact` Boolean, `team`, `player_id`, ... | 0x61A470, see 4 |

Texts (0.46c2 English): "Artifact has spawned", "You picked the artifact", "Your team have delivered the
artifact", "The opponent team delivered the artifact", "Artifact destroyed", "Round restarts in",
"Awaiting match start:", "Awaiting other players:". A drop has no message, only the sound.

## 8. `mode.anomalies` and `timeout_anomaly_core`

- Rule 149 `timeout_anomalies_rule_core` is a separate rule class; its cook 0x7055C0 reads exactly the
  six `mode.anomalies` keys (`max_active_anomalies_count`, `spawn_anomalies_delay_s`,
  `respawn_anomalies_timeout_s`, `try_spawn_anomaly_interval_s`, `try_spawn_anomaly_variation_s`,
  `block_new_anomalies_spawn_radius`) and it owns the map's `timeout_anomaly_core` objects
  (0x7A2F10). PROVEN.
- It is **not** Artifact Hunt's: 149 and the same `mode.anomalies` block are in this build's Team
  Deathmatch config too (`level_04_day_tdm.options`), and the map's 212 `timeout_anomaly_core` objects
  are ordinary map objects. PROVEN from the configs.
- **The artefact does not depend on it.** Rule 97 never touches rule 149 or an anomaly; the victory
  artefact is the item `gameplay/victory_items/artefact.options`, placed at `victory_item_spawners`.
  PROVEN (no such reference in 0x7839F0..0x786010 or the state code). The strings `artefacts_enabled`,
  `artefacts_first_spawn_time_sec` etc. (0x7BC400) belong to the other, anomaly-born artefacts.

## 9. The saved state (world snapshot)

Rule 97's `serialize` is vtable slot 18, 0x7844B0; `deserialize` slot 19, 0x784C80 (mirror image);
links are resolved after a load by slot 20, 0x784EB0. PROVEN. In order:

1. u32 rng state (`+0x1F8`)
2. u8 score team 0, u8 score team 1
3. u8 0xFF if the artefact has no carrier, else 0
4. only if no carrier, the artefact (0x7979F0): u8 0xFF if it lies in the world, else 0;
   - not in the world: u8 carrier id (0xFF = none);
   - in the world: u32 +0x1FC (time put there), u32 deadline +0x200, 64 bytes transform, 12 bytes linear
     velocity, 12 bytes angular velocity, 2 x u32 of its rigid body, then its usable part (u8 count +
     that many player ids, 0x791BE0)
5. container 0's usable part, container 1's usable part (0x791BE0 each)
6. u8 "delivered this tick" (0xFF / 0)
7. the state machine (0x8451A0): u8 index of the current state (= its id), then the state's own data:
   u32 start time for every timed state; `waiting_for_artefact` adds u32 its random timeout; states 4,
   9, 10 write nothing
8. u32 mask + one u32 per set bit: respawn time per player (`+0x230`, -1 = none)
9. u32 mask + one u32 per set bit: death time per player (`+0x2D0`)
10. u32 mask + one u8 per set bit: a per-player kit / profile value (`+0x320`, 0x17 = none)
11. per team, per respawn point: 2 x u8 (the point's two triggers' "occupied" bytes)
12. u8 respawns enabled (`+0x220`, 0xFF / 0)
13. u32 players still in (`+0x380`), u32 players that entered (`+0x384`)
14. u8 sudden death state (`+0x224`: 0, 1, 2); if not 0: u32 its start time (`+0x228`)

A carried artefact is saved by the **carrier**: the player's item container (0x74EB00) writes the
carried item's id byte and then the item's serialize (u8 0, u8 carrier id, the carry effect's state).
After a load 0x797D30 finds the carrier again and re-applies the modifiers.

## 10. Not settled

- The use key's generic flow (reach, press or hold, how `can_use` candidates are picked): not traced.
  Only the artefact's and the container's own slots were read; they hold no timer.
- What exactly the round start resets (0x799860 and the item visitor 0x7B1200..0x7B1350).
- The client flags of state bits 0 and 1 (0x69F8AE..0x69F8ED) and the value behind `artifact_visible`.
- Whether the carrier keeps every weapon action: no restriction was found, but the search was not
  exhaustive (127 functions touch a +0x1F0 field; about 15 were read).
- The 0.46c2 hud movie's side of the calls (what it draws for them) was not looked at.
- Spawn protection after a respawn (0x7854D8..0x7854E5 writes `now + point value` and 3000) was not
  followed.

## 11. Functions identified (0.46c2, decoded exe)

| address | name | confidence |
|---|---|---|
| 0x73F210 | `artefact_hunt_rule_core_cook` static constructor (class id 97) | proven |
| 0x73F290 | `artefact_hunt_rule_core_cook::translate_query` (requests `artefact.options`) | high |
| 0x73F730 | cook: on subresources ready (spawners, containers, respawn points) | high |
| 0x73FC30 | cook: build states and transitions | proven |
| 0x740E20 | cook: make the "picked" state (core); client 0x5D39A0 | proven |
| 0x5D3560 / 0x5D3870 | `artefact_hunt_rule_cook`: request / store the 7 notification sounds | proven |
| 0x7839F0 | `artefact_hunt_rule_core::artefact_hunt_rule_core` | proven |
| 0x783E20 | `::set_game_world_core` (slot 11) | high |
| 0x783EC0 | `::resolve_links` (link_resolver slot 0) | high |
| 0x784020 | rule start: container ids, first state, respawn point triggers (slot 15) | medium |
| 0x7840E0 | `::on_event` (slot 7) | proven |
| 0x7842A0 | `::on_player_entered_match` (slot 8) | high |
| 0x784350 | `::on_player_left_match` (slot 9) | high |
| 0x7844B0 | `::serialize` (slot 18) | proven |
| 0x784C80 | `::deserialize` (slot 19) | proven |
| 0x784EB0 | after-load link fix-up (slot 20) | high |
| 0x784EF0 | `::fill_match_hud_state` (slot 21); client 0x69F7D0 | proven |
| 0x7850F0 | `::rule_completed` (slot 22) | high |
| 0x785130 | `::logic_tick` (callback stored at rule+0x1E4) | proven |
| 0x7857B0 | spawn one player at a point | high |
| 0x7858C0 | select respawn points for a team | high |
| 0x785AC0 | "can this dead player be given a point" | medium |
| 0x785C40 | mark all players for respawn (new round) | proven |
| 0x785CF0 | "match decided" (score, time left, sudden death) | proven |
| 0x785D80 | "enough players" | proven |
| 0x785DF0 | "current state timed out" | proven |
| 0x785E10 | "match time over" | proven |
| 0x785E70 / 0x785E80 | "artefact has / has no carrier" | proven |
| 0x785E90 | "delivered this tick" | proven |
| 0x785EA0 | "artefact not in the world" | proven |
| 0x785EC0 | "timed out and match decided" | proven |
| 0x785EF0 | completion text (slot 23) | proven |
| 0x785F10 | recount the teams' remaining players | proven |
| 0x785F80 | sudden death: wiped team gives the other +1 | proven |
| 0x785770 | leading team (0, 1, 2 = draw) | proven |
| 0x7B0BA0 | `artefact_hunt_state_with_timeout::initialize` (freeze flag, start time) | proven |
| 0x7B0C20 / 0x7B0C80 | `waiting_for_players_state` initialize / hud | proven |
| 0x7B0CD0 / 0x7B0CF0 / 0x7B0D70 | `match_countdown_state` initialize / finalize / hud | proven |
| 0x7B0DC0 | `waiting_for_artefact_state::initialize` (random wait) | proven |
| 0x7B0E90 | `artefact_spawned_state::initialize` (random spawner) | proven |
| 0x7B0F20 | `artefact_dropped_state::initialize` | proven |
| 0x7B0F70 / 0x7B0F90 | `artefact_brought_state` initialize / hud | proven |
| 0x7B0FF0 | `transition_to_new_round_state::initialize` | proven |
| 0x7B1020 / 0x7B1130 / 0x7B1180 | `round_countdown_state` initialize / finalize / hud | proven |
| 0x7B1430 / 0x7B1450 | `finished_state` / `finished_successfully_state` initialize | proven |
| 0x844F10 / 0x844F70 / 0x8451A0 | fsm add transition / tick / serialize | high |
| 0x69F560 | `artefact_hunt_rule::artefact_hunt_rule` (client) | proven |
| 0x69F870 | client rule tick: flag edges, sound notifications | proven |
| 0x69FA40 | play notification n | proven |
| 0x69FB50 | `artefact_picked_game_state::deserialize` (keeps the carrier's team) | high |
| 0x796B50 | `victory_artefact_core::victory_artefact_core` | proven |
| 0x797280 | `::spawn` (slot 9); client 0x6E1690 | proven |
| 0x797320 | `::drop_or_put(time, put)` (slot 10); client 0x6E16C0 | proven |
| 0x7980E0 | `::drop(time)` (slot 6) | proven |
| 0x797930 | `::deactivate` (slot 11); client 0x6E1780 | proven |
| 0x7979F0 / 0x797BE0 | `::serialize` / `::deserialize` (slots 7, 8) | proven |
| 0x797D30 | after-load: find the carrier again | high |
| 0x797E00 / 0x797ED0 / 0x797EF0 | usable `can_use` / `use_info` / `use_execute`; client execute 0x6E1750 | proven |
| 0x797FB0 | usable finish: takes it out of the world | high |
| 0x7980F0 | artefact tick (ttl) | proven |
| 0x7982F0 / 0x798520 | put into the world at a transform | high |
| 0x798830 / 0x7988E0 | carrier modifiers on / off | proven |
| 0x798960 | carrier effect on | medium |
| 0x7B0A00 | `victory_artefact_container_core` constructor (reads `team`) | proven |
| 0x7B0AF0 / 0x7B0B30 / 0x7B0B70 | container `can_use` / `use_info` / `use_execute` | proven |
| 0x712100 | victory item spawner entry parser | proven |
| 0x7B2700 | `victory_item_event_manager::on_event` (awards), via 0x6D5A80 | high |
| 0x7B2960 / 0x7B29F0 | `is_solo_delivery` / share the delivery | high (by 0.26g's 0x334D00 / 0x335110) |
| 0x72BE90 | `base_player` slot 9: death (drops the item) | high |
| 0x726820 | `base_player`: weapon and action input (Drop key) | high |
| 0x72D700 | `base_player::can_sprint` | high |
| 0x72C0D0 | `base_player` tick with input (freeze byte) | high |
| 0x688360 | input mapper: key actions -> input bits | proven |
| 0x7477D0 | player parameter with modifiers applied to a base value | high |
| 0x7474A0 / 0x747550 | `player_params_modifiers_container::modifiers_list::add_modifier` / `remove_modifier` | proven (own strings) |
| 0x7055C0 | `timeout_anomalies_rule_core_cook`: read `mode.anomalies` | proven |
| 0x6136F0 | HUD init for a match (`set_match_mode`, `set_tickets_required`) | proven |
| 0x61CB60 | HUD update from the match hud state | proven |
| 0x616410 / 0x6165F0 / 0x616AF0 | `set_game_progress` / `set_last_players` / `set_match_time` | proven |
| 0x616D70 / 0x616E00 | `show_round_restart` / `set_round_restart_time` | proven |
| 0x616F00 / 0x616F90 | `show_pregame` / `set_pregame` | proven |
| 0x618BA0 | `show_game_message` | proven |
| 0x61BAC0 | `show_artifact_carry` | proven |
| 0x612F60 | `show_fade_animation` | proven |
| 0x61A470 | player icons (`carry_artifact`) | proven |
| 0x6D2EB0 | minimap objects (`update_objects`) | proven |
| 0x617D50 | `list_update_player` (`carry_item`) | proven |
| 0x5ADC10 | play music track n with a fade | medium |

Tools: Ghidra 12.1.4 headless (decompiler) and capstone; the scratch project and the decoded exe are in
the session scratchpad, not kept.
