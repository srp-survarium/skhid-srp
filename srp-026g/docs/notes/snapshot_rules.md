# 0.26g snapshot: the game rules and statistics blobs

**Version scope:** client **0.26g (build 2777, no PDB)**. Compared with what `servers/match-server/src/snapshot.rs` decodes for **0.21d**. Every address is a VA in the original 0.26g exe (image base 0x10000), read with `tools/re-kit/common/scripts/dis.py 026g`.

Tags: **PROVEN** = read in the 0.26g disassembly at the address given. **LIKELY** = strong inference, the missing step is named. **GUESS** = weak. Nothing here was checked against a live snapshot yet: none exists for 0.26g.

Field names are the client's own: each write passes its expression as an assert string (e.g. `m_team_points[0]`), quoted below.

Primitive writers (the size is the `resize(old + n)` in each): 0x1F3DE0 = **u8**, 0x80FA0 = **u8** (bool), 0x2F5DA0 = **u16**, 0x1F48A0 = **u32**, 0x804E0 = **f32** (4 bytes). All little-endian. PROVEN (0x1F3E81, 0x8104B, 0x2F5E41, 0x1F4941, 0x80581).

## 1. Where the blobs are

The uploaded body (client_patch.md §3.4) is `game_state_history_item::serialize(mask)` 0x324BE0. PROVEN (calls at 0x324CA7..0x324D62):

```
u32  active_clients_mask
per set bit, ascending: player_history_item          (u16 len + state, a flag, a bit stream; §7)
u16 len + bullet_manager_history_item                 item+0xB4690
u16 len + statistics_history_item                     item+0xB6AB8   <- §3
u16 len + game_objects_serialized_state               item+0xBBF30
u16 len + physics_world_history_item                  item+0xB7F08
u16 len + game_rules_history_item                     item+0xB76E0   <- §2
u16 len + game_effect_player_history_item             item+0xBFF80
```

- Each blob is an in-item buffer copied out as `u16 size` + bytes (e.g. rules: 0x324110, size at buffer+0x80C, data at +0x804). The rules buffer holds at most 0x800 bytes, the statistics buffer 0xC00 (the distance to the next buffer minus the 0x28-byte header). PROVEN (offsets); the overflow behaviour is not traced.
- The player blocks can be walked from the front (§7, added 2026-10-12): the blobs start where the last block ends. The older way still works for bytes that do not walk: the last u16 prefixes chain to the end of the body (effects, rules, physics, objects, statistics; 0.21d chained three), a candidate chain being confirmed by the checks in §2.7 and §3.4.
- `game_world_core` fills the blobs in `serialize` 0x2D2570: statistics by `[core+18FA8h]->vtbl+30h` (0x2D2644; skipped when the pointer is null, so the blob can be **empty**), rules by the loop at 0x2D2661. PROVEN.

## 2. The rules blob

### 2.1 Structure — PROVEN, the rule order too (2026-10-12)

```
for each rule in the core's rule vector (core+0x18824 .. +0x18828), in vector order:
    rule->serialize(writer)        vtable slot 15 (+0x3C), 0x2D2671..0x2D2686
u8  team_1_respawn                 blackboard, 0x323D60
u8  team_2_respawn
```

- The loader mirrors it: slot 16 (+0x40) per rule in the same order (0x2D2B20..0x2D2B5D), then the two blackboard bytes (0x323DC0). PROVEN.
- The two trailing bytes are new in 0.26g (`survarium::blackboard::serialize`, strings 0xA1791C `team_1_respawn`, 0xA17964 `team_2_respawn`): bools "this team may still respawn". Team Deathmatch clears one when the team's tickets reach 0 (§2.5). PROVEN.
- **Rule order: the mode config's `rules` order = timelimit (93), player_respawn (91), the mode rule (88 gather / 89 match_time_vi_spawn / 90 tdm / 480 research, §2.8), kd_stats (94). PROVEN (2026-10-12).** The core's vector is a plain copy of the range it is constructed with (0x2CFF74..0x2CFF90 → 0x2D4D70, no sort), and so is `pvp_match_core`'s (0x2D5603..0x2D5628); the cook requests one resource per `rules[i]` in config order (modes_maps.md §3). The last link: `pvp_match_core_cook::on_resources_loaded` 0x2CBC60 reads the query result as 0x2E0-byte entries (0 project, 1 game materials, 2 storm, then the rules), takes the rule count from the config's `rules` (0x2CBD53..0x2CBD85) and copies entry `3 + i` into `rules[i]` (0x2CBDA8..0x2CBE22), which goes straight to the `pvp_match_core` constructor (0x2CC0ED..0x2CC11A). The one sort nearby (0x2D4DD0, by `[obj+0x10]`, from 0x2D1D20) sorts the tick delegates at core+0x18840.., not the rules. Every shipped mode config lists the four in that order (2: 93, 91, 88, 94; 3: 93, 91, 89, 94; 4: 93, 91, 90, 94). 0.21d's order was gather, respawn, timelimit, kd, which was also its (hard-coded) request order.
- So the timelimit rule is the first 13 bytes and the respawn rule starts at byte 13; the signature of §2.2 is a check now, not a search.

### 2.2 timelimit_rule_core (class 0x5D) — 13 bytes, PROVEN (serialize 0x317F30, deserialize 0x317FD0)

| # | size | field | notes |
|---|---|---|---|
| 1 | u32 | `m_state_start_time_ms` (+0x12C) | **absolute** match ms; 0xFFFFFFFF until `on_match_ready` |
| 2 | u32 | `m_current_time_in_ms` (+0x130) | the rule's last tick = the item's own time |
| 3 | u32 | `m_players_mask` (+0x134) | players that have entered (0xE9) and not left |
| 4 | u8 | `(u8)m_current_state` (+0x148) | 0 inactive, 1 waiting for players, 2 countdown, 3 in process |

- **Changed against 0.21d:** the two times are written raw. 0.21d's serialize took a time base and wrote `t + base` (0x1F5588, 0x1F55C2), which `snapshot.rs` undoes with `TIME_OFFSET = 1e9`. In 0.26g serialize has one argument (`ret 4`) and writes the fields as they are. So `current == item_time` (not 1 000 000 000) is the signature, and `start` is an absolute time.
- The start rule and the meaning of the states: modes_maps.md §10.

### 2.3 player_respawn_rule_core (class 0x5B) — variable, PROVEN (0x316F10 → base 0x316C90)

| # | size | field | notes |
|---|---|---|---|
| 1 | u32 | `players_mask` | bit i set when `m_player_respawn_times[i]` ≠ 0xFFFFFFFF, i < 20 |
| 2 | u32 × popcount | `m_player_respawn_times[i]` | absolute ms, ascending i |
| 3 | u32 | `m_respawn_random.seed()` (+0x1A0) | |
| 4 | 2 × u8 per respawn point | `m_inside_objects_count` twice (`players_checker::serialize`) | three groups of points (pointers at +0x188/+0x18C/+0x190, counts at +0x194/+0x198/+0x19C), every point of every group in order |

- Length = 8 + 4·popcount(mask) + 2·P, P = the number of respawn points the rule loaded for the map and mode.
- **P is every entry of `server_objects.respawn_points` of the mode's project. PROVEN (2026-10-12):** `player_respawn_rule_cook` 0x2C6910 reads `server_objects` → `respawn_points`, allocates `0x1A8 + total·0x38`, counts team 0 and team 1 (0x2C69B4..0x2C69E9), lays the three groups out as team 0, team 1, team 2 (0x2C6A12..0x2C6A21) and fills each in config order (0x2C6A33..0x2C6A9D); no filter on layer or `is_temporary`. On the wire: team 0's points, then team 1's, then team 2's, each `u8` of the checker at point+0x28, then of the one at +0x2C (LIKELY allies in `ally_zones`, enemies in `enemy_zones`: constructor 0x2F4580).
- srp counts P in each map's project, from the server's game data (`maps::MapMode::respawn_points`, read when the table is derived; no level is known to the code). In 0.26g's projects P is by level, the same for every weather (`tools/re-kit/v026g/data/projects/*.server_objects.txt`); London (level_09, skhid's patch 14) has 20 in each of its three modes:

| level | modes 2 and 3 (team 0 / team 1) | mode 4 |
|---|---|---|
| level_01 | 39 (22/17) | 43 (26/17) |
| level_02 | 30 (15/15) | 31 (15/15 + 1 of team 2) |
| level_03 | 28 (14/14) | 22 (11/11) |
| level_04 | 36 (20/16) | 75 (20/55) |
| level_05 | 29 (14/15) | 29 |
| level_06 | 30 (14/16) | 30 |
| level_07 | 31 (16/15) | 31 |
| level_08 | 26 (13/13) | 26 |
- The deserializer of the base reads the mask, then one u32 per set bit, and sets the others to 0xFFFFFFFF (0x316D50). PROVEN.
- 0.21d: `snapshot.rs` never decoded the respawn rule (it sat between the gather head and the timelimit).

### 2.4 gather_victory_items_rule_core (class 0x58, mode 2) — variable, PROVEN (0x313CA0)

| # | size | field | notes |
|---|---|---|---|
| 1 | u32 | `m_spawner_random.seed( )` (+0x140) | |
| 2 | u8 | `m_team_points[0]` (+0x144) | batteries in team 0's container |
| 3 | u8 | `m_team_points[1]` (+0x145) | |
| 4 | u16 | `items_mask` | bit i set when battery i has **no carrier** (`[item+8] == 0`) |
| 5 | per set bit | the battery's own state (`victory_item_core::serialize` 0x301470) | variable, see below |
| 6 | per container (2) | `victory_items_container_core::serialize` 0x31CE40 | `u8 users`, users × `u8 owner.id`, `u32 m_victory_items.size()`, then that many `u8` battery ids |

- Fields 1–3 are the 6 bytes `snapshot.rs` reads as `GATHER_HEAD` for 0.21d (`team_points = rules[4], rules[5]`): **same layout, but no longer at the start of the blob** if the rule order is as §2.1 says.
- **A battery's state** (`victory_item_core::serialize` 0x301470, reader 0x301660; PROVEN 2026-10-12). The rule writes it only for a battery with no carrier (0x313D84..0x313D94), so the user byte is always 0xFF here:

| size | field | notes |
|---|---|---|
| u32 | `m_spotted_mask` (+0x1A4) | |
| u8 | `m_user ? m_user->id : -1` | 0xFF inside the rules blob |
| u8 | container id or 0xFF | the index into the rule's container vector |
| | **in a container: the record ends here (6 bytes)** | |
| 64 | `m_last_safe_transform` (+0x1C4) | 4×4 f32 (writer 0x301DE0) |
| 64 | `m_transform` (+0x204) | 4×4 f32; the position LIKELY its floats 12..14 (not checked against a live value) |
| 12 + 12 | `linear_velocity`, `angular_velocity` | float3 each (0x81A70) |
| f32 | `deactivation_time` | |
| u32 | `activation_state` | writer 0x3028A0 |
| u8 n, n × u8 | `m_usable_object_users.size()`, then `i->owner.id` | 0x2F8460 |

  A free battery is `6 + 160 + 1 + n` bytes. A cleared mask bit is a carried battery; who carries it is in that player's block (`m_carried_item`, §7).
- **A container** (0x31CE40, reader 0x31CEE0; PROVEN): `u8 users`, users × `u8 owner.id`, `u32 m_victory_items.size()`, that many `u8` battery numbers. Two on every map.
- **The container id is not the team** (PROVEN, data): it is the map's `victory_items_containers[..].id`, and `team` says whose it is. Container 0 is team 0's on level_02 and level_07 and team 1's on level_01, 03, 04, 05, 06, 08, in modes 2 and 3 and every weather; London's container 0 is team 1's. srp reads it in each map's project (`maps::MapMode::container_teams`). `m_team_points` are by team.
- **The results code does not need this rule when there are statistics:** the same team points and the winner are in the statistics tail (§3.3), which has a fixed position. With an empty statistics blob the rule's own `m_team_points` are the standing (`Decoded::standing`).

### 2.5 match_time_vi_spawn_rule_core (class 0x59, mode 3) — variable, PROVEN (0x3149B0)

| # | size | field | notes |
|---|---|---|---|
| 1 | u32 | `m_spawner_random.seed( )` | |
| 2 | u8 | `m_team_points[0]` | |
| 3 | u8 | `m_team_points[1]` | |
| 4 | u32 | `m_available_victory_items_count` (+0x170) | batteries placed so far (A) |
| 5 | u16 | `items_mask` | over the first A batteries only |
| 6 | per set bit | battery state, as §2.4 | |
| 7 | per container | container state, as §2.4 | |
| 8 | u32 | `m_match_start_time_ms` (+0x168) | absolute |
| 9 | u32 | `m_next_spawn_time_ms` (+0x16C) | absolute |
| 10 | u32 | `m_victory_items_spawn_time_ms` (+0x174) | |
| 11 | u32 | `m_victory_items_spawn_variation_ms` (+0x178) | |
| 12 | u8 | `m_match_started` (+0x17C) | |

- `m_team_points` are the same two bytes, at the same offsets of the same base class, updated by the **same** event handler and completion rule as mode 2 (vtable 0xA15CAC slots 7 and 18 = 0x313A50 and 0x314230, the gather rule's). Mode 3 differs only in when batteries appear. PROVEN.

### 2.6 tdm_rule_core (class 0x5A, mode 4) — 12 bytes, PROVEN (0x316340, deserialize 0x3163D0)

| # | size | field |
|---|---|---|
| 1 | u16 | `m_team_tickets[0]` (+0x110) |
| 2 | u16 | `m_team_tickets[1]` (+0x112) |
| 3 | u32 | `m_alive_players_mask` (+0x120) |
| 4 | u32 | `m_active_players_mask` (+0x124) |

- Not serialized: the per-team alive counts (+0x11C/+0x11D, recomputed from the alive mask, 0x3161F0) and the completion time (+0x12C).
- The tickets are also in the statistics tail as `m_team_points` (§3.3).

### 2.7 kd_stats_rule (class 0x5E) — 5 bytes per roster place, PROVEN (0x323870)

Per player i < N (N = match options +0x577B5, stored at rule+0x1B0 by the constructor 0x3237B4): `u16 m_kd_stats[i].kills`, `u16 m_kd_stats[i].deaths`, `u8 m_kd_stats[i].online`. Same 5 bytes per player as 0.21d (`KD_LEN`).

**Decoding the rules blob for results** (with the LIKELY order):

```
[0..13)            timelimit: check bytes 4..8 == item_time and state <= 3
[13..)             player_respawn (8 + 4·popcount + 2·P)
...                the mode rule (modes 2, 3: walked battery by battery, §2.4)
[len-2-5N .. len-2) kd_stats, N records
[len-2 .. len)     team_1_respawn, team_2_respawn (each 0 or 1)
```

- Mode 4 has no variable part besides the respawn rule, so `len == 13 + (8 + 4·popcount(respawn mask) + 2·P) + 12 + 5N + 2` gives P and confirms the layout; the tdm rule is then the 12 bytes before the kd stats.
- Modes 2 and 3: nothing in the blob says P, and the gather rule after the respawn rule has no fixed length. `Rules::decode` takes the P (smallest first) with which the gather rule reads exactly to the kd stats, its containers holding as many batteries as the two team points say and each of those batteries naming that container. The match server compares the P found with the map's (§2.3) and logs a difference.
- Mode 5: the research rule (§2.8) is 21 + 96·Z bytes, Z its own byte 20 (3 on every map: 309 bytes), so, as in mode 4, the length gives P.

### 2.8 The research rule (class 480 = 0x1E0, mode 5) — 21 + 96·Z bytes (exe patch 53, `rr_write` / `rr_read`; added 2026-10-22)

Not 0.26g's: srp's own rule for Research (`research_mode.md`), loaded by the configs'
`rules [93, 91, 480, 94]`, so between the respawn rule and the kd stats. Little-endian.

| at | size | field |
|---|---|---|
| 0 | u32 x2 | each team's points, millionths (0 to `resources_count` x 10^6) |
| 8 | u16 x2 | the points shown, whole (rounded): what the HUD and the statistics' tail have |
| 12 | u32 | the end: match ms, 0xFFFFFFFF none; the tick + 3000 when a team's points reach the goal |
| 16 | u32 | roster places entered and not left |
| 20 | u8 | Z, the units (3) |
| 21 + 96z | u32 | unit z's progress, 0 to 1 000 000 000 (a whole capture) |
| +4 | u8 x4 | the team taking it, its owner (2 = nobody), players of team 0 inside, of team 1 |
| +8 | u32 x2 | those players by roster place, a mask a team |
| +16 | u32 x20 | each roster place's mark: its share of the capture when it went in, 10^9 = all |

- `rule_completed(t)`: the end is set and `t >= end` (as the gather rules; modes_maps.md §11).
- The rule writes the statistics' tail (§3.3) each tick in play: `m_team_points` the shown points,
  `m_winner_team` the leader (2 for equal points). No battery records (§3.2) in mode 5.
- `rr_read` refuses, and `Research::read` with it: a unit count other than the map's, a progress
  over 10^9, a team over 2, inside counts that are not their masks' bits, a place over 19.
- srp: `snapshot::Research` and `ResearchUnit` (read from the kd stats back, the most units first;
  written too), the standing from its shown points (else the tail's), the end from its end time
  when its points have the goal (`rules::research_finished`).

## 3. The statistics blob

`shared_statistics::serialize` 0x2F5AC0 (reached through the thunk 0x26DB40, `this + 8`). PROVEN:

```
N x player_shared_statistics            N = match options +0x577B5 (statistics+0x31E0)   §3.1
V x victory item status                 only when a gather rule exists (modes 2, 3)       §3.2
11-byte tail                                                                              §3.3
```

**Changed against 0.21d** (`snapshot.rs::decode_statistics`): every part. 0.21d had 24 u16 counters per player, two `{u8 count, 5 bytes}` lists, 5 + 2 + 4 + 7 bytes, then 7 curing bytes per player, per-battery records and 2 bytes of team points. 0.26g has 28 events **per kit**, bit-masked and mostly one byte wide, no faction-kill counts, no curing records, and a longer tail that includes the winner.

### 3.1 Per player — `player_shared_statistics::serialize` 0x335750, PROVEN

In memory a player has `events[28][3]` (u16; the second index is the "preset" = the kit 0..2 the player was using) and a damage record at +0xA8.

```
3 x (preset p = 0, 1, 2):
    u32  events_mask                    bit e set when events[e][q] > 0 for a preset q <= p, e < 28 (see below)
    for e in (0,1,2,3,4,5,7,8,9,10,11,12,13,14,15,16,19,20,21,24,25,27), if bit e:   u8  events[e][p]
    for e in (6,17,18,22,23,26), if bit e:                                           u16 events[e][p]
3 x (preset p):
    u32  inflictors_mask                bit i set when received_damage[i][p] > 0, i < 20
    per set bit: f32 received_damage[i][p]
u32  receivers_mask                     bit i set when last_hit_ms[i] > 0, i < 20
per set bit: u32 last_hit_ms[i]
u32  last_killed_enemy_ms
u8   last_hit_enemy_id
u8   killstreak                         ("u8( killstreak )")
f32  newbee_bonus_player_action_coef
u16  longest_killstreak
```

- The one-byte and two-byte event lists are the tables at 0x920588 (22 entries) and 0x9205E0 (6 entries), read in table order. PROVEN. Note the order: all one-byte events of a preset first, then its two-byte events.
- **The mask is not started again for each preset** (PROVEN, corrected 2026-10-14). It is cleared once (0x33575D), each preset's pass only sets bits in it (0x335778..0x3357A9) and it is taken back from its stack slot after it is written (0x3357CD). So preset 1's mask has preset 0's bits too, and preset 2's has all three: a player who used one kit all match has the same mask three times, and a zero for each of its events under the second and the third. Seen in every record of the worlds of match 0x69f8c0ba. The reader takes any mask. `snapshot.rs` reads this, and its writer (`PlayerStatistics::write`; only the scripted client and the tests write statistics) keeps the mask across the kits as a game does. Only this mask is kept so: the damage masks that follow are each preset's own (cleared inside the preset loop, 0x3353E7), and so are a battery's (§3.2, 0x334E24). PROVEN.
- A one-byte event is truncated to 8 bits on the wire (`u8( events[...] )`); the reader zero-extends (0x3358E6). So counts above 255 of those events wrap in a snapshot. PROVEN.
- A player's total for an event is the sum over the three presets. LIKELY (the scoreboard code was not read; the per-kit split is what the wire carries).
- Which event number is which: 0.21d's `match_stats_events_dict_enum` had 24; 6, 17, 18 are still two-byte (the hundredths events), and 22, 23 (bullet fire / hit in 0.21d) are two-byte as before, which fits the old numbering being kept. 24..27 are new (26 is two-byte). **Settled (2026-10-12, results.md §9 and below):** 0, 3, 4, 8, 9, 10, 13, 15 are PROVEN the 0.21d events (the kill handler 0x2F4EB0 counts them by those numbers), 11 and 12 too (`finish_match` 0x2F4CF0); **25 is a grenade kill, 27 the end-of-match event of a player whose protection value is 0**.
- **Events 24 and 26** (PROVEN, the victory item event function 0x334AC0, on "put into a container" by player P; the names are srp's, the game has none). Per battery the statistics keep `owner_team` (+0xF8; 3 = never delivered), `first_finder` (+0xFC) and the last player who took it out of a container (+0xFD, set at 0x334B4D):
  - **24, "solo delivery"** (0x334C46): +1 when 0x334D00 is true: the battery was fresh and P its first finder, or its owner was the enemy team; and no other player of P's team has a carried distance > 0 on it. The quest type 6 `bring_x_items` ("Find and bring batteries") counts it, not event 20: `game_quest_progress_calculator::refresh_quest_ui_data` 0x2A2FD0, case 0x2A30A3 (PROVEN). In 0.21d that quest counted event 20.
  - **26, "stolen and brought home"** (0x334C7F): +1 when `owner_team` is the enemy team and the last one to take it out is P. That last one is the one byte of the record no world carries (§3.2): a game that started after the taking out does not count it (match.md §28). Two bytes on the wire, but counted by ones and weighed × 1.0 (only 6, 17 and 18 are × 0.01: 0x26DBA2, 0x26DC86). The quest type 7 `steal_box` ("Steal a battery from the enemy base and bring it to your base") counts it: `refresh_quest_ui_data` 0x2A2FD0, case 0x2A30F6 (PROVEN; the whole table of what each quest type counts is in `crates/survarium/src/v026g/quests.rs`).
  - In the same code: 19 = first pick-up of a battery (0x334B2C); 20 = put into a container (0x334B5E); at a delivery 6 (never owned), 17 (the owner was the enemy) or 18 (own team) is shared out by carried distance (0x335110). Elsewhere: 22 = bullets fired, added by value (0x2F5519); 23 = a bullet hit on an enemy (0x2F54A3); 5 = artefact (0x2F5606); 7 at 0x2F57F5; 21 at 0x2F598C. A player's score adds the three presets of an event (0x26DC9D). So 0..13, 15 and 17..23 are PROVEN by where they are counted; 14 and 16 have no counting site left (LIKELY dead).
- Everything is self-describing, so a player record can be walked forward without knowing its length.

### 3.2 Per battery — 0x334DC0, PROVEN

Only when the statistics object has a gather rule (`[+0xA38] != 0`; modes 2 and 3). V = the rule's battery count (its item vector's size; 10 in every shipped config, also in mode 3 before all are placed).

```
3 x (preset p):
    u32  players_mask                   bit j set when carried_distances[j][p] > 0, j < N
    per set bit: f32 item_status.carried_distances[j][p]
f32  item_status.nearest_positions[ team_1 ]
f32  item_status.nearest_positions[ team_2 ]
u8   item_status.owner_team
u8   item_status.first_finder
u8 x 2 progress[team_1], [team_2]      exe patch 92 (its own state, not the record's): 0 untouched in the life, else 10 m steps + 1
u8   held count                        exe patch 92: the carriers holding steps toward their own team's container
per held: u8 cell, u8 steps            cell = place x 3 + kit, < 60, rising; steps >= 1; paid at the delivery
```

Mode 4 has no such records. The class is `victory_item_event_manager` (its strings, 0xA19D68; the object is `shared_statistics`+0x2760, the ten records from its +0x20, 0x100 bytes each). The reader is 0x334FB0.

- **Not saved: who took the battery out of a container last** (PROVEN, 2026-10-14). The record's byte +0xFD is the roster place of the player whose "taken out of a container" event came last (event type 1, written at 0x334B4D), and event 26 is counted only when it is the player who puts the battery in (0x334C74). The writer ends with `first_finder` (+0xFC, 0x334F5A) and the reader with the same byte (0x3350CA): a load leaves +0xFD as that game had it. Nothing else of the record is left out (+0xF0 bytes of distances, the two positions, `owner_team` as a dword at +0xF8, `first_finder`; +0xFE and +0xFF are never used by the game; exe patch 92 saves its own state of the battery after `first_finder`), and of a player's record only the word at +0x1F4, which nothing reads.
- **`first_finder` and +0xFD are set to 0xFF by the record's constructor and by nothing else** (PROVEN, 2026-10-14). The constructor is 0x3347F0, given to the array constructor at 0x2F4B7C in `shared_statistics`'s constructor 0x2F4B40. The reset between matches (0x2F4C20, called by the handler's 0x26D880 when a match's connection ends, 0x27B4C9) calls 0x3348A0, which zeroes the distances and sets `owner_team` to 3 and writes neither byte. The handler is a member of the network client, not of a match (+0x1EE8D8; the client's constructor 0x2199A0 makes it at 0x219AB0, and `game::create_network_client` 0x1EBFC0 makes the client). So a game that goes from one battery match to the next with the same network client keeps both bytes of the last one; seen (match.md §28). When the lobby makes a new client was not traced (`lobby_menu::on_render_scenes_ready` does it only with a byte at +0x5869 of the object it has at +0x9C set, 0x221C1D). `first_finder` is saved, so every game of a match takes the bytes of the game that gave the first world.

### 3.3 The tail — 11 bytes, PROVEN (0x2F5B08..0x2F5BCB; reader 0x2F5C22..0x2F5C84)

| # | size | field | meaning |
|---|---|---|---|
| 1 | u16 | `m_team_points[0]` | modes 2, 3: batteries in team 0's container (copied from the rule's u8 at 0x313AA5). Mode 4: team 0's tickets (0x316257). Mode 5: team 0's shown research points (§2.8) |
| 2 | u16 | `m_team_points[1]` | same for team 1 |
| 3 | u8 | `m_team_victory_items[0]` | modes 2, 3: the same count, kept by the statistics' own handler 0x2F5570 (+1 for the team of the player who puts a battery in, 0x2F5590; −1 for the other team when one is taken out, 0x2F55C1). 0 in mode 4 |
| 4 | u8 | `m_team_victory_items[1]` | same |
| 5 | u8 | `(u8)m_winner_team` | the **current** leader: 0 = team 0, 1 = team 1, 2 = draw; **3 until a rule's first event** (0xFF from the constructor 0x2F4BCD and the reset 0x2F4C37, then 3 from the handler's match init 0x26D95A). Rules in modes_maps.md §11 |
| 6 | u32 | `m_active_in_match_players_mask` | mode 4: the tdm rule's active mask (0x3165B5, 0x31660E). Modes 2, 3: nothing writes it and neither constructor nor reset sets it: not to be used |

- The rules write these fields on every event, so the tail of the last snapshot before the end is the result. **Compute the winner from the team points with the rules of modes_maps.md §11**; the byte is 3 before the first event.
- When the points are valid (PROVEN 2026-10-12): modes 2 and 3 from the start (0 from the constructor, then written only by the container event, in step with the rule's own pair); mode 4 from the start of play (the tdm rule writes tickets and winner on its "match started" event, 0x3160D8 → 0x3161F0), 0 during waiting and countdown.
- The tail is the last 11 bytes of the blob whatever N and V are, so the results code can read team points and winner without parsing §3.1 at all.

### 3.4 Decoding and checks

1. Chain five u16 prefixes from the end of the body; the statistics blob is the one before them.
2. Empty statistics blob: the client had no statistics object; take the rules blob only (as 0.21d).
3. Otherwise walk N player records (§3.1), then V battery records (V = 10 in modes 2/3, 0 in mode 4), then 11 bytes: the walk must end exactly at the blob's end. That is the same exact-fit test `snapshot.rs` uses to pick the right chain.
4. Cross-check: modes 2/3 `m_team_points` equal the rule's two u8; mode 4 they equal `m_team_tickets`; `m_winner_team` follows from them.

## 4. What `snapshot.rs` must change

| part | 0.21d code | 0.26g |
|---|---|---|
| section chain | 3 from the end (rules, objects, statistics) | 5 from the end (effects, rules, physics, objects, statistics) |
| `EVENTS` | 24 u16 | 28, per kit, masked, u8 or u16 (§3.1) |
| player record | fixed fields + two lists | §3.1 |
| curing records | 7 bytes a player | gone |
| battery records | mask, distances, 8 + 2 bytes | §3.2 (per kit) |
| statistics tail | 2 bytes | 11 bytes with the winner (§3.3) |
| rules: gather head | at offset 0 | after timelimit and respawn (LIKELY); not needed for results |
| rules: timelimit | 13 bytes before the kd stats, times offset by 1e9 | 13 bytes, LIKELY first, **absolute times** |
| rules: kd stats | last 5·N bytes | 5·N bytes before the **2 blackboard bytes** |
| mode 4 | — | tdm rule 12 bytes (§2.6) |

## 5. Not traced

- Settled 2026-10-12: the rule order (§2.1), P (§2.3), the battery and container states (§2.4), the tail's writers and first values (§3.3), events 24 and 26 (§3.1), the player block's envelope and head (§7).
- A carried battery's own state (inside its carrier's player block), and victory item event types 2 and 3.
- Whether the position is where §2.4 puts it in `m_transform`.
- The bullet manager, game objects, physics and effects blobs (not needed for results).
- Of a player's state: the effects after the head, a weapon after its magazine, the damage model's records (§7).

## 6. Code (added 2026-10-12)

`crates/survarium/src/v026g/match_msgs/snapshot.rs` reads and writes all of
this: `Sections` (the split; it chains **six** prefixes from the end, the
bullet manager's too, which leaves fewer false splits than five),
`Statistics` (§3, exact fit), `StatisticsTail::of` (§3.3 alone), `Rules`
(§2: the timelimit first, kd stats and the two flags from the end, the tdm
rule as the 12 bytes before the kd stats; since 2026-10-12 also `Respawn`,
`Gather` with its `Battery` and `Container` states and mode 3's `Timed`),
`decode` (picks the split) and `crc` (what 0xDF reports). `Sections::walk`
splits a body from the front (§7); `candidates` falls back to the chain
from the end. The byte tables at 0x920588 / 0x9205E0 were re-read from the
exe (22 and 6 entries, as §3.1 lists them). Mode 4's standing prefers the
tdm rule's tickets: the statistics' `m_team_points` are 0 until play
begins while the rule holds 100. Modes 2 and 3 take the statistics' points
and, with none, the gather rule's.

`match_msgs/players.rs` (§7): `walk` / `blocks`, `animation_len`,
`PlayerState` (the head), `Supplies` (what a player has left; the match
server puts it into each `PlayerResult`). A map's respawn points and its
containers' teams: `maps::MapMode::respawn_points`, `container_teams`, read in
its project when the map table is derived (they were hand tables of
`match_msgs/rules.rs` before, levels 1 to 9; the projects give the same).

## 7. The player blocks (added 2026-10-12)

Read for the supplies a player has left, which 0.21d's server took from its
snapshots too (`servers/match-server/src/supplies.rs`). PROVEN unless
tagged; no 0.26g snapshot was seen.

### 7.1 A block — `player_history_item::serialize` 0x33FDE0 (called at 0x324D16), reader 0x33FEE0 / 0x33FD80

| # | size | field | where |
|---|---|---|---|
| 1 | u16 | `(u16)m_buffer.size()` = L | 0x33FD24; read 0x33FD90 |
| 2 | L | the player state (§7.3) | memcpy 0x33FD57 / 0x33FDC3 |
| 3 | u8 | `animation_tree != 0`: 0x00 or 0xFF | 0x33FE16; read 0x33FF1C |
| 4 | variable | if set: the animation state (§7.2) | 0x33FE31 → 0x395CA0 → 0x397250; read 0x395D00 → 0x3A99C0 |

- match.md §3 called 0x33FEE0 the writer: it is the reader.
- The item's second per-player buffer (+0x1028; `has_effects_presenter`, `client_serializable_effects_count` and what `base_player::serialize` writes through its second writer) is not uploaded: 0x33FDE0 writes only the buffer at +0x1004.

### 7.2 The animation state's length — `decode_data` 0x3A9900, 0x3A9580, 0x3A9720, bit reader 0x3A8980

A bit stream, most significant bit first, with no length field:

```
16 bits  T                     bit length of the tree part (patched in by 0x396CD0)
T bits   the tree part
times    (decode_times_in_ms_impl 0x3A9580)
    10 bits N
    if N != 0:  1 bit mode, 32 bits base
        if N > 1:  5 bits w
            mode 0:  N x w bits
            mode 1:  U in width(N) bits, (U - 1) x w bits, N x width(U) bits
floats   (decode_floats_impl 0x3A9720)
    1 bit mode, 10 bits N
    mode 0:  N x 32 bits
    mode 1:  U in width(N) bits, U x 32 bits, N x width(U) bits
pad to a byte
```

`width(n) = max(1, bit_length(n))` (0x3A961E, 0x396120). The reader moves the outer stream on by exactly these bytes right after the two tables (0x3A9988..0x3A999A), before it reads the tree, so the length needs no tree decoding. The same routines are in 0.21d (by PDB name): this walk lands on the sections of all 1222 snapshots of `srp-021d/logs/snapshots`.

### 7.3 The state's head — `game_world_core_client::serialize` 0x3218E0, `base_player::serialize` 0x2DAF60 (reader 0x2DB490)

M = the profile's item count (the static info's item list).

| offset | size | field | where |
|---|---|---|---|
| 0 | u8 | `current_index`: which of the place's up to three player objects this is. LIKELY the kit in use (each item header's `player_profile_id`, owner+0x115, indexes the profile's kits: 0x2DB753..0x2DB766) | 0x321902 |
| 1 | u8 | `target_index` | 0x32191F |
| 2 | M × u32 | `player_profile::serialize_dynamic` 0x2F7CC0 (reader 0x2F7D40): for each item of the profile, `items[i].amount_in_storage` if the dictionary's `is_stack`, else `items[i].condition`. No mask | |
| 2+4M | bool ×3 | `m_is_alive`, `m_has_to_die`, `m_need_to_select_animations` (0x00 / 0xFF) | 0x2DAFA1.. |
| +3 | f32, f32 | `m_look_pitch`, `m_rotation_y` | 0x2DB001, 0x2DB021 |
| +11 | 4, u32 | the player input's 4 raw bytes, `actions_mask` | 0x2C5C26, 0x2DB060 |
| +19 | u32 | the game effect player's `m_current_time_in_ms` | 0x2DDAF6 |
| 4M+25 | u32 | `serializable_effects_count` E | 0x2DDB49 |
| 4M+29 | E effects | 41 fixed bytes each, then virtual parts: not traced | 0x2DDBD2..0x2DDD86 |

- `amount_in_storage` is what the player did not take into a slot: at a spawn each slot gets `min(the slot's count, amount_in_storage)` and the storage loses it (0x2FCC4A..0x2FCCBD). So what is left of a stack item = storage + the amounts in the slots + the rounds in the magazines, and a dead player has the storage.

### 7.4 After the effects (walkable only with E = 0 and no battery carried)

`u32` hit effects H, each `bool is_exists` and then 8 or 17 bytes (0x3402C0); `u8` current and target active object; 29 bytes of stamina (0x329310); then the **inventory** (0x2FE360): `f32 m_carried_weight`, `bool`, `u8 m_carried_item id or 0xFF` (then that battery's own state, not traced), `u8 m_artefact_slots_count` A (if A ≠ 0: `u8 slots_begin`, A × u8), `u8 m_active_slot`, `u8 m_grenade_slot`, then **the items by ascending slot 0..22** (0x2FE50A..0x2FE53B). After the inventory: jump type, speed, the damage model with each body part's `m_health` (0x32AE00, sizes not traced), the character controller if alive, spotting, recent hits, `bool m_respawn_disabled`, the last native field. Client patch 78 (`artefact_carry`, `carry.save` at the serialize epilogue 0x2DB47E, read back at 0x2DBAB5) then appends the carry's 32-byte snapshot, version byte 2 first (`artefact_carry::carry::Carry::snapshot`), so a state's last 32 bytes are the carry's and `m_respawn_disabled` is the byte before them; `Motion::read` (§7.5) drops them first and refuses a state without a valid one.

An item — `inventory_item::serialize` 0x2FFDF0:

| size | field |
|---|---|
| u16 `m_amount` or f32 `m_condition` | u16 if the item's dictionary entry `is_stack` |
| u8 `player_id` | 0xFF without an owner; then nothing more of the header |
| u8 `player_profile_id` | the owner's kit |
| u8 `m_slot_id` | LIKELY the slot number (item+0x254) |

Ammunition, grenades, traps, boosters, armour: the header only (5 or 7 bytes). A **weapon** (`weapon_core::serialize` 0x2E3130) goes on from its header's start h: h+7 u32 seed, h+11 u32 `m_last_tick_time_in_ms`, h+15 bool `m_load_ammo_on_next_activate`, **h+16 u8 `m_ammo_in_magazine`**, h+17 `m_fire_queue_type`, h+18 `m_current_ammo_id`, h+19 bool `m_is_round_chambered` only for some weapons (weapon+0x5F0), then recoil, dispersion, the FSM (not traced to the end). Medkit: bool `m_active` and more; oxygen tank: bool, u32.

### 7.5 What the server reads

- `players::walk`: every block's end (§7.1, §7.2), exact.
- `PlayerState::read`: the head (§7.3), exact, given M from the static info.
- `Supplies::read`: per profile item `stored` (the head's u32); for the kit `current_index`, each slot's item is looked for after the head by its three id bytes `[roster place, kit, slot]`, in slot order, with an amount no larger than the slot's count before them (a stack) or a finite f32 ≥ 0 (anything else): `in_hand`, `condition`, and for an item whose config `type` is `wpn_…` the byte at h+16, `magazine`. LIKELY (that `player_id` is the roster place; that nothing else in a state has these bytes in this order). `inventory_read` says whether anything was found.
- `Motion::read` (`match_msgs/motion.rs`; the bots' fixes in the match server, `srp-worlds`): the tail from the end, after dropping client patch 78's 32-byte carry snapshot (§7.4); a state without a valid snapshot is not read.
- `Supplies::left`: a stack item's total as §7.3 says; a magazine's rounds go to the ammunition in the slot after the weapon's, or the one after that (LIKELY: the lobby's kit layout; which of the two is loaded, `m_current_ammo_id`, is not read; a chambered round is not seen).

## Correction (2026-10-12): the bools of the rules section are 0x00 and 0xFF

`kd_stats_rule`'s `online` (§2.7) and the blackboard's two respawn flags at the end of the section (§2.1) are written as 0x00 or 0xFF, like the bools of a player's block (§7).

- PROVEN in the exe: the bool writer 0x80FA0 stores `neg al; sbb al,al` of the byte it is given (0x81038..0x81061), so 0x00 or 0xFF. `kd_stats_rule::serialize` 0x323870 writes two u16 (0x2F5DA0) and calls it for `online` (0x3238E7); the blackboard's serializer calls it for `team_1_respawn` (0x323D8A) and `team_2_respawn` (0x323DB2).
- The client's readers take any byte but 0 as true (`test` / `setnz`: the kd rule 0x323952, the blackboard 0x323DCA and 0x323E0B). `Rules::decode` is stricter on purpose: where the parts of the section end is found by what reads.
- `Rules::decode` took 0 and 1 only until this day and read the rules of no live world (432 dumps of four matches tried, none read). It takes 0x00, 0xFF and 1 now and writes 0xFF. All 897 dumps of five live matches read (four battery matches, one Team Deathmatch); no byte but 0x00 and 0xFF is in these places in any of them.
- What the match server does with the rules, and so did in no live match until then (`game.rs`):
  - a Team Deathmatch's end by its rule, and its standing from the rule's tickets and alive mask (`observe`);
  - the second start of a player back in a Team Deathmatch in play (match.md §21): `first_load_loses_the_standing` answers "not known" without the rules, and the kept logs have not one such start;
  - following the clients' own time rule, state and start, from the countdown on;
  - the warning when a world's respawn points are not the map's;
  - kills and deaths of the results from the kd rule, in place of the statistics' counters;
  - refusing an initial world whose kd rule has a kill or a death.
- Compared in those 897 dumps before the change went live: the kd rule's kills and deaths are the statistics' counters in every one; the mode rule's points (batteries, tickets) are the statistics' in every one; the respawn points are the maps'; the time rule's countdown and start of play are, to the millisecond, what the server's own copy logged in each of the five matches. Where the server had another source the two agree, so the ends and results of those matches would have been the same.
- Read again in the exe on the same day, because these paths were about to run with real clients for the first time: the tdm rule's bytes (`serialize` 0x316340: u16 for +0x110 and +0x112, u32 for +0x120 and +0x124), its end (0x316470: a team's tickets 0 and its alive count 0 set the completion to now + 3000), `rule_completed` (0x316520: completion < time), the leader (0x316550) and the time rule's reader (0x317FD0: start, current, mask, state). They are as §2.2, §2.6 and modes_maps.md §11 say.
- Not seen live yet: a Team Deathmatch that ends by its rule, and a second start.
