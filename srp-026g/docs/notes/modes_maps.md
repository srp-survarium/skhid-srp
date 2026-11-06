# 0.26g maps, game modes and rules data vs 0.21d

**Version scope:** client **0.26g build 2777** (exe Jan 14 2015, no PDB; `resources.db` dated Jan 17 2015, 78,961 entries, `E:\sur-res-pro\downloads\v026g\extracted\Survarium\game\resources.db`) vs client **0.21d build 2010** (Apr 2014, PDB; `E:\ssrp\Survarium-0.21d-srp\resources.db`, 52,702 entries). VAs are image VAs (base 0x10000 in both exes).

Tags: **PROVEN** = read in the data (resource path cited) or in exe code (VA / string cited). **LIKELY** = strong inference. **GUESS** = weak inference.

Method: entries pulled with `dbgrab` into the scratchpad, binary configs printed with srp's `binary-config-cli print-binary-config` (parses 0.26g configs unchanged). Disassembly: scratchpad `m26/d26.txt` (0.26g), `tools/re-kit/common/scripts/dis.py 021d` (0.21d). Nothing in the game folders was changed; nothing was run.

---

## 1. Summary table (0.26g vs 0.21d)

| Item | 0.21d | 0.26g | Status | srp-021d must change |
|---|---|---|---|---|
| How a match names its map+mode | `match_options` C460 = project string `level_0X/<day\|rain\|evening>`, C448 = mode, plus numeric params (C44D respawn, C44E time, C454, C458/9, C45A, C45B) | one **u8 map_id** = an id in `maps_dict` (`gameplay/db_static_dictionaries`) → `resources/gameplay/game_modes/<cfg_mode_name>` which holds mode, rules, all timings and the project name. PROVEN (0x2C99C7 `cfg_mode_name`, 0x2C9A13 `resources/gameplay/game_modes/`; u8 map_id in `match_options::deserialize` 0x1F0E70 per sibling `static_info.md`) | **changed** | `MatchOptions::new(map, variant, mode, seed)` and the `MAPS`/`VARIANTS` tables are replaced by a list of maps_dict ids (§3). Mode, timings, battery count, respawn time are no longer server-chosen in options (LIKELY; see open Q1 for the 28 f32 / 20 u8 fields) |
| Maps | level_01..06 | level_01..**08** (07 "Vector Laboratory", 08 "Cologne" new). PROVEN (`projects/level_07/*`, `projects/level_08/*`, `st_level_07_name`, `st_level_08_name`) | **new: 2 maps** | add to map table |
| Weather variants | day, rain, evening | day, rain, evening; level_08: day, evening, **prerain** (no rain). `map_weather_id` 1 day, 2 rain, 3 evening, 4 night, 5 prerain. PROVEN (`gameplay/game_modes/level_08_*`) | changed | per-map variant list |
| Project per variant | one `projects/<lvl>/<var>/server_project` + `client_project` shared by all modes | **one client_project per variant per mode**: `projects/<lvl>/<var>_<mode>/client_project`, mode ∈ {2,3,4}. **No `server_project` anywhere** (only `gameplay/game_modes/gather_server_resources.options` matches "server"). PROVEN (resources_list) | **changed** | project names are now e.g. `level_01/day_2`; srp's `level_0X/day` would not exist |
| Mode 0 capture_enemy_base, 1 capture_neutral_base | in the enum, no map data, play as mode 2 | **no config uses them** (only test `loopback_client` and `gather_server_resources` have `mode: 0`). PROVEN (data); enum names unknown (no PDB) | **gone (data)** | drop 0/1 from `SRP_MATCH_MODES` |
| Mode 2 gather_victory_items ("Protective device", batteries) | rule classes 0x5C timelimit, 0x59 gather, 0x5B respawn, 0x5D kd | rules `[93,91,88,94]` = timelimit 0x5D, player_respawn 0x5B, gather 0x58, kd 0x5E. PROVEN (§4) | same mode, **class ids renumbered**, params moved to data | none on the wire (client picks classes); results model: 10 batteries, 900 s |
| Mode 3 match_time_vi_spawn | batteries appear every 30 ± 10 s (`gameplay/victory_items/default.options`), only test use, broken by rollback (`battery_spawn.md`) | **reworked and shipped**: 60 ± 10 s, `spawn_points_count` 3 (2 on level_08) charging stations (`victory_item_spawn_points`, layer `mode_3`), 10 batteries; rule serializes `m_next_spawn_time_ms` and `m_team_points[2]`. PROVEN (data + strings 0x3149E7.., 0x314A21) | **changed** | srp's 0.21d battery-spawn client patch is LIKELY unnecessary; result model needs team points |
| Mode 4 Team Deathmatch | — | **new**: rules `[93,91,90,94]`, `tdm_rule_core` (class 0x5A), `tickets_count` 100 (150 on level_05 and level_08). PROVEN | **new** | add mode 4: winner by tickets, no batteries; result model |
| Mode 5 | — | referenced only: `maps_dict` ids 145..151 (`level_0{1,4,6,7,8}_day_5`, `test/level_06_dtb1/2`) and 143/144 (`test/primitive_1`, `test/dtb`), 114 (`level.options`) **have no file** in resources.db. PROVEN (dict vs resources_list) | **not shippable** | never offer these ids |
| Rule classes in exe | timelimit, gather_victory_items, match_time_vi_spawn, player_respawn, single_player_respawn, kd_stats | timelimit_rule_core, gather_victory_items_rule_core, match_time_vi_spawn_rule_core, **tdm_rule_core**, player_respawn_rule_core + player_respawn_rule_base, kd_stats_rule. `single_player_respawn_rule` string gone. PROVEN (refs26/refs21) | changed | — |
| Match timings | server-chosen per match (srp: 600 s, countdown 10 s, start at 0 %) | data, identical in all 74 shipped configs: `match_time` 900 s, `wait_time` 5, `countdown_time` 20 s, `wait_player_percent` 0.8, `respawn_time` 8 s. Read by `timelimit_rule_core` (strings at 0x317E32..0x317E9E) and respawn rule (0x31677A). PROVEN | **changed** | the match server cannot shorten the countdown or start with 1 player any more unless match_options still overrides (open Q1) — **a lone tester waits for 80 % of the roster** (or the 5 min `wait_time`) |
| Map objects for the rules | `server_project.server_objects` (10 kinds incl. anomalies, artefacts, damage zones, `team_base_points[0]`) | `client_project.server_objects` holds only `respawn_points`, `victory_items_containers`, `victory_item_spawners`, `victory_item_spawn_points`; anomalies, artefact containers, damage/effect zones and new `resupply_zone`s are in `client_objects`. PROVEN | changed | relevant only if srp simulates the world |
| Item/match dictionaries | items, perks, boosters, skills, factions | adds `maps_dict[89]`, `match_maker_defines`, `match_levels_dict[21]`, `shop_bundles_dict`; perks/boosters/skills dicts gone from this file. PROVEN (`gameplay/db_static_dictionaries`) | changed | matchmaker data available (§6) |
| Score events | 24 (`match_stats_events_dict_enum`) | 28 (`gameplay/ui/victory_screen.options` `event_categories[28]`), matching sibling's "28 f32" in match_options. LIKELY | changed | `SCORE_WEIGHTS` needs 28 entries |
| New world element | — | "storm" (`reinforcement_storm_game_effect_*`, `gameplay/effects/effect_storm.options`) requested by `pvp_match_core_cook` as class 0x6D (string `storm` 0xA0F18C at 0x2CC525). PROVEN | new | none (client data) |

---

## 2. Maps (0.26g)

PROVEN from `projects/*` and `gameplay/game_modes/*.options` (`match_making_params`, `ui`).

| map_id | dir | name (`st_level_XX_name`, English) | variants with a client_project | in 0.21d |
|---|---|---|---|---|
| 1 | level_01 | Rudnya | day/rain/evening × _2 _3 _4 | yes |
| 2 | level_02 | School | same | yes |
| 3 | level_03 | Vostok Radar Station | same | yes |
| 4 | level_04 | Chemical Plant | same | yes |
| 5 | level_05 | Tarakanovsky Fort | same | yes |
| 6 | level_06 | Mamayev Kurgan | same | yes |
| 7 | level_07 | Vector Laboratory | same | **new** |
| 8 | level_08 | Cologne | day/evening/prerain × _2 _3 _4 | **new** |
| 9 | level_09 | London (skhid's `resources.db.patch_14`, ids 181 to 183) | day × _2 _3 _4 | — |
| 14 | v069d_level_01 | Rudnya (v0.69) (skhid's pack `rudnya-069d`, ids 204 and 216-218, `rudnya-069d.md`) | day × _2 _3 _4 _5 | — |
| 88 | test/tt5, test/tdm_test, test/artefact_testlevel | test maps | **no `projects/test/*` in the db** | — |

- `lobby_scene/day` still exists (client only). PROVEN.
- 0.21d `projects/<lvl>/<var>/server_project` files (18) are all gone. 0.26g has 72 level client_projects = 8 maps × 3 variants × 3 modes. PROVEN.
- `gameplay/game_modes/level_08_night_2.options` / `_night_3.options` (maps_dict ids 107/108) name `level_08/night_2`/`night_3`, **which have no project** → would fail to load. PROVEN (data); never offer them.
- `loading_screen_id` (0..25) and `map_weather_id` now come from the mode config (`ui.loading_screen_id`, `match_making_params.map_weather_id`), replacing srp's `MAPS[].pictures` level_id byte. PROVEN (data); whether match_options still carries a picture byte: see sibling static_info.md.

### Per-project rule objects (0.26g `client_project.server_objects`, 0.21d `server_project.server_objects`) — PROVEN

| map | 0.21d resp. pts (t0/t1), spawners, containers | 0.26g mode 2: resp (t0/t1), spawners, containers | mode 3: resp, stations, spawners | mode 4: resp (t0/t1) |
|---|---|---|---|---|
| level_01 | 24 (11/13), 37, 2 | 39 (22/17), 37, 2 | 39, 4, 24 | 43 (26/17) |
| level_02 | 20 (10/10), 51, 2 | 30 (15/15), 51, 2 | 30, 4, 24 | 31 (15/15 + **1 point with team 2**) |
| level_03 | 22 (11/11), 45, 2 | 28 (14/14), 50, 2 | 28, 3, 18 | 22 (11/11) |
| level_04 | 22 (11/11), 33, 2 | 36 (20/16), 33, 2 | 36, 4, 24 | 75 (20/55; 39 in layer `tdm`) |
| level_05 | 20 (10/10), 30, 2 | 29 (14/15), 50, 2 | 29, 5 (4 in layer mode_3), 30 | 29 (14/15) |
| level_06 | 20 (10/10), 32, 2 | 30 (14/16), 32, 2 | 30, 4, 24 | 30 (14/16) |
| level_07 | — | 31 (16/15), 39, 2 | 31, 3, 18 | 31 (16/15) |
| level_08 | — | 26 (13/13), 22, 2 | 26, 2, 16 | 26 (13/13) |
| level_09 (London) | — | 20, 2 (container 0 is team 1's) | 20 | 20 |

- Every map has ≥ 13 respawn points per team in every mode, so the 0.21d "≤ 10 per team" guidance still holds; mode 4 has no containers or spawners. PROVEN (counts); team cap LIKELY 10 (20 profiles in match_options per sibling).
- Respawn point gained `invulnerability_time` (6), `enemy_check_radius` (1.2), `ally_check_radius` (0.8), `is_temporary`. PROVEN (`projects/level_01/day_3/client_project`).
- New `resupply_zone` per team (`resupply_time_in_ms: 3000`, `team`). PROVEN (`client_objects` of level_01/day_2).
- Mode 3 `victory_item_spawn_point` (layer `mode_3`): `spawners_weight`, attached `spawn_boxes` and `spawners` (e.g. 3 boxes, 6 spawners), charging-station models (`new_mode/2/energy_charging_*`). PROVEN.
- Anomalies/artefacts shrank on level_01: 0.21d 7 anomaly cores / 27 artefact containers / 21 damage zones → 0.26g 2 / 16 / 2 (now in `client_objects`). PROVEN.

---

## 3. Game modes (0.26g) and the ids a server sends

All 74 shipped level configs, PROVEN from `gameplay/game_modes/<name>.options` and `maps_dict` in `gameplay/db_static_dictionaries`:

| mode | rules (class ids) | parameters (all maps unless noted) | UI | maps_dict ids |
|---|---|---|---|---|
| 2 batteries | 93, 91, 88, 94 | match_time 900, wait_time 5, countdown_time 20, wait_player_percent 0.8, respawn_time 8, victory_items_count 10 | `st_mode_02_name` "Protective device", icon 0 | day/evn/rain: L1 61,63,65 · L2 67,69,71 · L3 73,75,77 · L4 79,81,83 · L5 85,87,89 · L6 91,93,95 · L7 97,99,101 · L8 103,105 (night 107 broken), prerain 109 |
| 3 timed batteries + stations | 93, 91, 89, 94 | as mode 2 + vi_spawn_time 60, vi_spawn_variation 10, spawn_points_count 3 (level_08: 2) | `st_mode_03_name` "Protective device" (same text as 2), icon 0 | L1 62,64,66 · L2 68,70,72 · L3 74,76,78 · L4 80,82,84 · L5 86,88,90 · L6 92,94,96 · L7 98,100,102 · L8 104,106 (night 108 broken), prerain 110 |
| 4 Team Deathmatch | 93, 91, 90, 94 | match_time 900, wait_time 5, countdown 20, wait_player_percent 0.8, respawn_time 8, **tickets_count 100** (level_05, level_08: 150) | `st_mode_04_name` "Team Deathmatch", icon 2 | day/evn/rain: L1 117,118,119 · L2 120,121,122 · L3 123,124,125 · L4 126,127,128 · L5 129,130,131 · L6 132,133,134 · L7 135,136,137 · L8 138,139, prerain 140 |
| 5 Research (skhid's, not 0.26g's: `research_mode.md`) | 93, 91, **480**, 94 | 0.27d's: match_time 900, wait_time 5, countdown 20, wait_player_percent 0.8, respawn_time 10, **resources_count 100**, capture_time_in_s, resource_per_s (x 1.25, the owner 2026-10-22), restore_multiplier 0.5, the units in `capture_zones` | `st_mode_05_name` "Research", icon 1 | 0.26g's rows 145-149 (`level_0{1,4,6,7,8}_day_5`, no config in 0.26g) and 184-203: levels 1 to 8 in day, evening, rain (level 8 prerain), London day; the content pack `research-mode` |
| test | — | `test/tt5` (11): mode 2, 600 s, 2 batteries, wait 1, countdown 1, 10 %; `test/artefact_testlevel` (45): mode 2, 2 batteries, respawn 1; `test/tdm_test` (115): mode 4, 90 s, 50 tickets | | maps lack projects |
| 0 loopback | 92 only | respawn_time 1 | map `test/tt5/day` | 141 `loopback_client` |
| 0 none | — | empty | | 142 `gather_server_resources` (LIKELY a server-side preload list, unused by the client) |

- Config shape: `mode{ parameters{…}, ui{loading_screen_id, icon, description, mode_name, map_name}, name, match_making_params{map_weather_id, map_id, map_name = project}, rules[4], mode }`. PROVEN.
- Every config's `name` is `level_0X_<n>` for day and `level_0X_<evn|rain|night|prerain>_<n>` otherwise. PROVEN.
- English texts: mode 2/3 "Storm is approaching… find batteries… restore the protective stations… the team that collects most batteries wins"; mode 4 "every death takes one point from the team's reinforcement pool; most reinforcement points at the end wins". End messages `st_finish_match_mode_0{2,3,4}_{winner,loser}_message` ("…collected all the batteries", "Your team was eliminated"). PROVEN (`localization/english/localization`).
- What the client does with the config: `pvp_match_core_cook::translate_query` (0x2CC270) reads `mode` → `match_making_params.map_name` (≤ 31 chars) and requests it as **class 0x40** (0x2CC467; 0.21d class 0x41 at 0x1C7944), requests game materials (class 0x51, 0x2CC4FC), `storm` (class 0x6D, 0x2CC52A), then **one query per `rules[i]` with class = rules[i]** (loop 0x2CC579..0x2CC676), then `gameplay/players/default.player` (class 0x54, 0x2CC6E5). PROVEN. In 0.21d the rule classes were hard-coded (0x59 or 0x5A by `C448 == 3`, then 0x5B, 0x5C, 0x5D; 0x1C79D8..0x1C7B6C). PROVEN.
- There is an alternative project source at match_options+0x57880 used when two flags are set (0x2CC3D9..0x2CC415). GUESS: replay / editor path; not needed by a server.

---

## 4. Rule resource classes — PROVEN

0.26g: static cook objects get their class at obj+8 and their vtable at obj (core cooks), the client then swaps in client vtables (static init 0x1E99DD..0x1E9AEA).

| class | 0.26g rule (core ctor / size) | registration | 0.21d class |
|---|---|---|---|
| 0x58 (88) | gather_victory_items_rule_core (0x313970, 0x168) | 0x2F23A9 / client vt 0x9FD844 | 0x59 (vt 0x8199D8, 0x151633) |
| 0x59 (89) | match_time_vi_spawn_rule_core (client wrapper 0x206380 → 0x3147A0, 0x1F0) | 0x2F2418 / 0x9FD878 | 0x5A (vt 0x819A08, 0x151677) |
| 0x5A (90) | **tdm_rule_core** (0x315FF0, 0x130; reads `tickets_count` 0x31603F) | 0x2F41F9 / 0x9FD8AC | — |
| 0x5B (91) | player_respawn_rule_core (0x316EC0; reads `respawn_points`, seeded) | 0x2C66B9 / 0x9FACE4 | 0x5B (vt 0x819A40, 0x1516BB) |
| 0x5C (92) | player_respawn_rule_base only (0x316700, reads `respawn_time`, no respawn points) | 0x2C6729 / 0x9FD918 | 0.21d single_player_respawn_rule: LIKELY equivalent; its 0.21d class not checked |
| 0x5D (93) | timelimit_rule_core (0x317DA0, 0x150) | 0x2F4A09 / 0x9FD944 | 0x5C (`timelimit_rule_cook` ctor 0x162776) |
| 0x5E (94) | kd_stats_rule (0x323790, 0x1B8) | 0x2CCDA9 | 0x5D (vt 0x819A68, 0x151723) |
| 0x5F | pvp_match_core_cook (sibling `match.md`) | 0x2CBB69 | — |
| 0x1E0 (480) | skhid's research rule (exe patch 53, mode 5; `research_mode.md`) | the game's constructor, after its own cooks | — |

Other shifts seen on the way: project 0x41 → 0x40, game materials 0x52 → 0x51 (LIKELY: same request order), default.player 0x55 → 0x54 (LIKELY).

Serialized rule state (strings next to each `::serialize`), PROVEN:
- timelimit_rule_core: `m_state_start_time_ms`, `m_current_time_in_ms`, `m_players_mask`, `(u8)m_current_state` — same as 0.21d.
- gather_victory_items_rule_core: `m_spawner_random.seed()`, **`m_team_points[0..1]`**, `items_mask`.
- match_time_vi_spawn_rule_core: the above + `m_available_victory_items_count`, `m_match_start_time_ms`, **`m_next_spawn_time_ms`**, `m_victory_items_spawn_time_ms`, `m_victory_items_spawn_variation_ms`, **`m_match_started`**. 0.21d did not serialize the next spawn time, the root of its rollback bug (`srp-021d/.../battery_spawn.md`). LIKELY fixed in 0.26g.
- tdm_rule_core: `m_team_tickets[0..1]`, `m_alive_players_mask`, `m_active_players_mask`.
- player_respawn_rule_base: `players_mask`, `m_player_respawn_times[i]`; player_respawn_rule_core adds `m_respawn_random.seed()`.
- kd_stats_rule: `m_kd_stats[i].kills/.deaths/.online` (`online` new).

---

## 5. Battery data

- 0.21d `gameplay/victory_items/default.options` had `parameters.spawn_parameters{spawn_time 30, spawn_variation 10}`; 0.26g's has only `parameters.movement_speed_modifier: -0.05` (carrier slowed 5 %), 3rd-person animations, `data.mtl_idx 55`. Spawn timing moved to the mode config (`vi_spawn_time`/`vi_spawn_variation`). PROVEN.

---

## 6. Match-option-like and matchmaker data in the client db

- Per-mode timings and counts: §3 (identical across maps except tickets and spawn_points_count). PROVEN.
- No max players / team size field in any mode config or dictionary. PROVEN (negative, all 79 configs + dictionaries).
- `db_static_dictionaries.match_maker_defines`: `items_levels_modifiers[21]` (item level → modifier 0..20), `elo_modifiers[6]` (elo 100..15000 in 1000 bands → 0..5), `slots_weight[23]` (all 1), `wait_modifier 1`. PROVEN. The client itself computes a player's level from these (0x2CA9B0, 0x2F79A0): `matchmaking.md` §1. `wait_modifier` is not read by the client (the string is not in the exe).
- `match_levels_dict[21]`: per match level `money_k` 1 → 13.59, `reputation_k`/`experience_k` 1 → 1.95. PROVEN. LIKELY the reward multipliers by match level (results/lobby side).
- `gameplay/ui/victory_screen.options`: 3 score categories (kills, help, game events) and `event_categories[28]`. PROVEN.

## 7. Data that looks server-side

- `gameplay/game_modes/gather_server_resources.options` (mode 0, no rules, no params) and maps_dict id 142. LIKELY a dedicated-server resource-gathering entry; the client never needs it.
- `client_project.server_objects` (respawn points, battery containers/spawners/stations): named "server" but loaded by the client, since 0.26g is still client-side lockstep (sibling `match.md`). PROVEN that it is in the client project; a relay-only srp server does not read it.
- `match_maker_defines`, `match_levels_dict`: matchmaker/lobby inputs shipped to the client. PROVEN used by the client for display: the equipment level, the search range and the level's multipliers in the lobby (`matchmaking.md` §1, §2).

---

## 8. Server work implied (srp-021d → 0.26g)

1. **Map/mode choice:** replace `MAPS × VARIANTS × modes` (`crates/survarium/src/v021d/match_start.rs`, `servers/match-server/src/server.rs::new_game`) with a whitelist of maps_dict ids: mode 2 = 61,63,65,67,…,105,109; mode 3 = 62,…,106,110; mode 4 = 117..140. Exclude 11, 45, 107, 108, 114, 115, 141..151.
2. **`SRP_MATCH_MODES`:** accept 2, 3, 4; drop 0/1 (no data). Default could become 2,4 (and 3 once checked live).
3. **Static info:** send u8 map_id instead of the project string + mode + params (sibling `static_info.md` has the layout). Server-side copies of timings (srp `Setup.countdown_seconds`, `duration_seconds`, `batteries`) must come from the config table above: 900 s, countdown 20 s, 10 batteries, respawn 8 s, tickets 100/150.
4. **Start of match:** `wait_player_percent 0.8` and `wait_time 5` are data now. The countdown starts when `floor(0.8 × N)` roster places have entered (§10): size the roster to the players present and it starts with the first 0xE9s; a roster of 1 or 2 starts on the first entered player. Padding the roster with absent places can block the start, and after 5 minutes of waiting the rule reports the match as completed.
5. **Results:** mode 4 winner = more tickets left (tickets fall by deaths); mode 3 adds `m_team_points`. srp's results/game model (`servers/match-server/src/game.rs`) needs a TDM path. Score weights: 28 events.
6. **Battery-spawn client patch** (`../srp-021d/client-patches/battery_spawn.asm`): LIKELY not needed for 0.26g; verify live.
7. **Teams:** keep teams 0/1; ignore the stray team-2 respawn point on level_02 mode 4.

## 9. Open questions

1. Do the 28 f32 / 20 u8 fields of 0.26g `match_options` (sibling) override any config timing (e.g. countdown, wait percent), as 0.21d's C454/C45A did? **Answered for the timelimit and tdm rules: no.** Their constructors read `mode.parameters.*` from the config only; the one match-options value the timelimit rule takes is N (+0x577B5) (§10, §11). `respawn_time` was not re-checked.
2. The 0.26g `game_mode` enum names (no PDB): are 0/1 still capture-base, and what is mode 5 ("dtb")? Only configs referencing it exist in maps_dict, no files.
3. Does the client fail or skip when maps_dict names a missing config (ids 114, 143..151) at startup (`items_dictionary_cook::on_map_configs_loaded` ~0x2C9D24)? It evidently shipped this way, so LIKELY skipped, not checked.
4. Exact semantics of mode 3 `spawn_points_count` vs the 2..5 `victory_item_spawn_points` per map (choose N stations by `spawners_weight`?), and what `m_team_points` count. Not traced in code.
5. TDM end condition: **answered in §11** (a team with no tickets and no player left alive, or 900 s).
6. Class 0x5C (respawn base) in 0.21d terms: is it the old `single_player_respawn_rule`? Its 0.21d class id was not looked up.

---

## 10. When a match starts: `timelimit_rule_core` (added 2026-10-12) — PROVEN unless tagged

Read in the 0.26g exe: constructor 0x317DA0, `on_match_ready` 0x317EE0, `tick` 0x317F10, `logic_tick` 0x318100, enter/leave 0x3180C0 / 0x3180E0, `rule_completed` 0x318360 (vtable 0xA06E2C slots 13, 8, 9 and 18; `tick` is a delegate stored at rule+0x110/+0x114). 0.21d counterpart: `timelimit_rule_core` 0x1F549E..0x1F590F (`v021d/match_start_spec.md`).

**Parameters** (constructor, all from the mode config's `mode.parameters`, 0x317E18..0x317EC9):

| field | source | value in every shipped config |
|---|---|---|
| +0x138 match time, ms | `match_time` × 1000 | 900 000 |
| +0x13C waiting limit, ms | `wait_time` × **60 000** (minutes) | 300 000 |
| +0x140 countdown, ms | `countdown_time` × 1000 | 20 000 |
| +0x144 fraction, f32 | `wait_player_percent` | 0.8 |
| +0x14C N, u8 | **match options +0x577B5** = the roster size `u8 N` of static info (0x317E0A) | set by the server |

Nothing else comes from the match options: the server chooses N and nothing more (0.21d sent the fraction, the waiting minutes and the countdown in the options).

**States** (+0x148; the same number is copied to `game_world_core`+0x18FE4):

| state | entered | left |
|---|---|---|
| 1 waiting for players | `on_match_ready(t)`: start := t (0x317EE0) | to 2 when **the entered mask is not empty and `popcount(mask) ≥ floor(N × fraction)`** (0x318246..0x318271; popcount 0x317D40, `vostok::math::floor` 0x1E800); start := t. Log "Countdown: [%d]Match begins in %d seconds" |
| 2 countdown | | to 3 when `t ≥ start + countdown` (0x31813C..0x31814D); start := start + countdown; the rules get event 0 "match started" (0x2D1EA0). Log "Match started [%d]". **Never back to 1**, whoever leaves |
| 3 in process | | — |

- **The mask** (+0x134) is the set of players that have entered and not left: bit set by `on_player_entered_match` (0x3180C0), cleared by `on_player_left_match` (0x3180E0), each followed by a `logic_tick`. Players enter through the server's 0xE9 (match.md §2.2 step 7). It is **not** the roster and not the connected clients: the server decides it with its 0xE9 / 0xEA messages.
- **The fraction is of N**, the roster size the server put in static info, and the product is rounded **down**: N = 1 → 0, 2 → 1, 3 → 2, 4 → 3, 5 → 4, 6 → 4, 8 → 6, 10 → 8, 16 → 12, 20 → 16 players needed (0.8 as an f32 is slightly above 0.8, so N × 0.8 never falls below a whole number). The "mask not empty" test makes N = 1 need its one player.
- **So:** a match whose roster equals the players present starts its countdown as soon as 80 % of them (rounded down) have entered, not when all have; a **solo roster of 1 starts on that player's 0xE9**, and so does a roster of 2. Then 20 s of countdown (data, the server cannot shorten it), then 900 s of play.
- **`wait_time`** is only used by `rule_completed(t)` (0x318360): in state 1 it returns true once `t − start ≥ 300 000`; in state 3 once `t − start ≥ 900 000`; in state 2 never. So a match that waits 5 minutes for players reports "completed" without having started. What the caller does with it (the end-of-match path) was not traced: LIKELY the normal match end.
- For the server: size N to the players it will really enter. Padding the roster with absent places makes the start need `floor(0.8 × N)` real players, and a lone tester in a roster of 3 or more would sit in "waiting" until the 5-minute limit.
- Serialized state (13 bytes, absolute times): snapshot_rules.md §2.2.

## 11. How a match ends and who wins (added 2026-10-12) — PROVEN unless tagged

A rule ends the match by answering true to `rule_completed(t)` (vtable slot 18). The time limit is §10 (state 3 for 900 s). The mode rules add their own end, and they keep the current leader in the statistics (`m_winner_team`: 0 = team 0, 1 = team 1, 2 = draw; `m_team_points[2]`), which is serialized in the snapshot's statistics tail (snapshot_rules.md §3.3). The consumer of `rule_completed` and of the winner (results screen, end messages) was not traced.

### Modes 2 and 3 (`gather_victory_items_rule_core`; mode 3's `match_time_vi_spawn_rule_core` shares these functions)

- Event 2 "battery put into / taken out of a container" (handler 0x313A50, vtable slot 7 of both classes): `m_team_points[team]` +1 or −1 (u8 at rule+0x144 / +0x145), ignored once the rule has completed.
- Statistics after each change (0x313AA5..0x313ADC): `m_team_points[0..1]` := the two counts; `m_winner_team` := 0 if points0 > points1, 1 if less, **2 if equal**.
- **End:** when either team's count equals the number of batteries (the item vector's size = `victory_items_count`, 10), completion time := t + 3000 ms (0x313AE2..0x313B34); otherwise it is reset to "none". `rule_completed(t)` = completion time ≤ t (0x314230).
- **On time** (900 s): the timelimit rule completes; the winner is whoever has more batteries then, a draw if equal (the statistics value above).
- Mode 3 vs mode 2: the same `m_team_points`, handler, end and winner; only the battery placement differs (client_patch.md §4). The count to reach is all 10 batteries in both, although in mode 3 they appear over time.

### Mode 4, Team Deathmatch (`tdm_rule_core`, vtable 0xA15DA4)

- State: `m_team_tickets[2]` (u16, both start at `tickets_count` from the config, 0x31603E..0x316078), alive mask, active mask, per-team alive counts.
- **A death** (event 1, data `{u8 killer, u8 victim}`, handler 0x316090), with T = the victim's team:
  - tickets[T] already 0: the victim is removed from the alive mask (he is out);
  - otherwise tickets[T] −1, **except for a kill by a teammate** (killer ≠ victim and same team), which costs nothing. A suicide or a death without a killer (0xFF) costs a ticket (0x31614E..0x31618C);
  - when tickets[T] is now 0, the blackboard flag `team_(T+1)_respawn` is cleared (0x3161AE..0x3161C8; the flags are in the snapshot's rules blob, snapshot_rules.md §2.1). LIKELY: the respawn rule then stops respawning that team, so the death that takes the last ticket is the last one that still respawns (the respawn rule's reading of the flag was not traced).
- A player who leaves is removed from the alive mask (0x3165D0).
- **End:** when a team has **0 tickets and no player alive**, completion time := t + 3000 ms (0x316470); `rule_completed(t)` = completion time < t (0x316520). Or the time limit (§10).
- **Winner** (0x3161F0 writes it to the statistics after every event; the same rule again at 0x316550): more tickets wins; **equal tickets and not zero → draw (2)**; both zero → the team with more players alive, a draw if equal. So on time the team with more tickets left wins.
- Statistics: `m_team_points[0..1]` := the tickets (0x316257..0x316278), `m_active_in_match_players_mask` := the active mask.
- With `tickets_count` 0 in a config both respawn flags are cleared at match start (event 0, 0x31609B..0x3160D1): elimination without respawns. No shipped config uses it.

### Mode 5, Research (skhid's rule 480, exe patch 53; added 2026-10-22)

Not 0.26g's: 0.27d's rules as `research_mode.md` has them, the rule's bytes in snapshot_rules.md §2.8.

- Each team earns points while it owns units (millionths in the rule, shown whole). **End:** when a
  team's points reach `resources_count` (100), the end time := t + 3000 ms; `rule_completed(t)` =
  end set and end ≤ t (as the gather rules). Or the time limit (§10).
- **Winner:** more shown points; **equal → draw (2)**, on time as at the goal.
- Statistics, each tick in play: `m_team_points[0..1]` := the shown points, `m_winner_team` := the
  leader. No battery records, no active mask.
- srp: the end is the rule's own end time (`Research::end`) when its points have the goal; the
  standing the rule's shown points, else the tail's (`snapshot::Decoded::standing`).

### What the results code needs from a snapshot

Only the 11-byte statistics tail: `m_team_points[2]` (batteries, tickets or Research's points) and `m_winner_team`; and for the phase, the timelimit rule's 13 bytes (snapshot_rules.md §2.2, §3.3).

## Correction (2026-10-12): §11, the gather rule's completion

- "The consumer of `rule_completed` … was not traced": the world's save asks every rule (slot 18) and writes the answer into the history item at +0xC0BBA (0x2D271D..0x2D2755); nothing in the client reads that byte. The end of a match is the server's.
- "Ignored once the rule has completed" has a second half: at the completion the rule also clears its byte +0x160, and from then on no container can be used (match.md §26). Neither is taken back by a rollback, which is why the tenth battery does not go in while anybody else sends inputs.

## 12. What srp reads of the maps (added 2026-10-16)

srp knows no level, id or count of these sections beforehand: it derives
them from the server's game data when the map table is first used
(`crates/survarium/src/v026g/maps.rs`, `maps::derive`), so a map a patch adds
is played as soon as the server runs on data with it.

- **Playable** (`MapMode::is_playable`): the data has the id's config
  (`maps_dict` → `gameplay/game_modes/<cfg_mode_name>`), its `mode.mode` is
  one srp plays (2 to 5), and the data has its project
  (`match_making_params.map_name`). No range of levels: on 0.26g's data that
  is the 72 ids of §3; with London 75 (181, 182, 183); with the Research
  pack 25 more (mode 5; a mode-5 id also needs its `resources_count`).
- **From the project** (`server_objects` of `client_project`, which the server
  data keeps of every project: `resources_db::server::server_objects_of`):
  `MapMode::respawn_points`, every entry of `respawn_points` (§2's table, the
  number of pairs the respawn rule writes, snapshot_rules.md §2.3), and in
  modes 2 and 3 `MapMode::container_teams`, `victory_items_containers[..].team`
  by `id`. A playable id whose project has no respawn point, or (modes 2 and 3)
  not two containers of ids 0 and 1, one of each team, stops srp at start.
- **Names** (srp's web page): `ui.map_name` and `ui.mode_name` of the config
  (`st_level_0N_name`, `st_mode_0N_name`) in the game's English and Ukrainian
  localizations (`localization/<language>/localization`, kept in the server
  data; `survarium::v026g::localization`), else English, else the project's
  folder.
- Checked: on the test data (0.26g) the projects give the hand tables srp had
  before for every level 1 to 8 (`maps::tests::the_projects_give_what_srp_had_by_hand`);
  on 0.26g with skhid's patches 12 and 14 the playable ids are the 72 and 181
  to 183, every value as before, London 20 respawn points in each mode and
  container 0 team 1's, named "London" / "Лондон" (an ignored test:
  `SRP_TEST_RESOURCES_DB=<…>/resources.db SRP_TEST_SKHID_PATCHES="<…>/resources.db.patch_12
  <…>/resources.db.patch_14" cargo test --release -p survarium -- --ignored
  the_live_data`). A made-up
  map of a level 12 with Team Deathmatch only is played in mode 4 alone, with
  its project's respawn points, and drawn by the matchmaker
  (`maps::fixtures`).
