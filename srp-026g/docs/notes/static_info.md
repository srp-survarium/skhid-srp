# 0.26g match static info (S→C 0xE4) vs 0.21d (S→C 0x51)

**Version scope:** client **0.26g build 2777** (Jan 2015, no PDB) compared with client **0.21d build 2010** (Apr 2014, PDB). All addresses are image VAs (base 0x10000) in the named build.

**Tags:**
- **PROVEN**: read in 0.26g code at the cited VA, and matched to the cited 0.21d PDB symbol and VA.
- **LIKELY**: a strong inference from code or data.
- **GUESS**: a weak inference.
- **UNKNOWN**: not determined.

**Sources:** the 0.26g listing (`d26.txt`), the 0.21d labelled disassembly, and 0.26g `resources.db` (`gameplay/db_static_dictionaries` and `gameplay/game_modes/*.options`, dumped with `dbgrab` + `cfgdump`). Baseline: `srp-021d/docs/notes/match_start_spec.md` §1–2.

---

## 0. Headline changes

1. **The envelope is new.** 0xE4 = `u8 local_player_index` first, then a **raw LZMA2 stream** that holds `match_options`. In 0.21d the index was the *last* byte and nothing was compressed. PROVEN (0.26g 0x27B880..0x27B928).
2. **Match settings are no longer on the wire.** These used to be sent in 0.21d: level/project name, game mode, battery count, respawn delay, match duration, start fraction, waiting time, countdown, and the loading picture. They now come from data. A single **`u8 map_id`** selects an entry of `maps_dict` (in `gameplay/db_static_dictionaries`). That entry names a `resources/gameplay/game_modes/<cfg>.options` config, which supplies all of them, including the list of rule resources. PROVEN (0.26g `items_dictionary::map_descriptor_by_id` 0x21F910; the maps_dict loader 0x2C994A..0x2C9D75; `pvp_match_core_cook` translate_query 0x2CC270).
3. **All 20 profiles are always sent** (fixed `player_profile[20]`), not N. The per-player squad bytes are always 20 too. PROVEN (0x1F0E70 loop to +0x577B0; byte loop to +0x57840).
4. **The profile is completely new.** It is no longer "one loadout of 23 slot items + 20 modifier floats". It is now:
   - team, name, a new **clan abbreviation** string, and the protection u32;
   - **up to 3 loadouts** ("kits"). Each kit has a state, an id, a name, 23 slots (u16 item index + u16 count) and a **68-float modifier block with a 128-bit mask**;
   - an **item list (≤ 69 items)** that the kits' slots index into.

   The per-item serialize mode comes from the item dictionary's `is_stack`. The 0.21d per-slot mode table (0x877168) is gone. PROVEN (0x1F1070, 0x1F1120, 0x1F1350, 0x1F13D0).

---

## 1. Comparison table: whole message

| # | 0.21d 0x51 (`process_static_match_info` 0x1A2DDC) | 0.26g 0xE4 (handler 0x27B880, assert/log strings name it `network_client::process_static_match_info`, `network_client_processing.cpp` lines 0x193..0x198) | status |
|---|---|---|---|
| envelope | none: `match_options::deserialize` 0x1F59A4 reads the payload directly | `u8 local_player_index`, then **all remaining bytes = raw LZMA2 stream**. It is decoded by `lzma2_compressor` vfn+4 = 0x357EB0 → `Lzma2Decode` 0x3EE810 into a 0x578A0-byte stack buffer. The decode result is **not checked**. Then `match_options::deserialize` 0x1F0E70 runs on the decoded bytes | **CHANGED** PROVEN |
| local index | **last** `u8`, after the N profiles; profile +5B0 = (i == idx) at 0x1A2E3B | **first** `u8` (read at 0x27B890); profile +0x45F0 = (i == idx) for i < N at 0x27BE4E..0x27BE75; `on_match_ready` copies the matching i into network_client+0x1F1C44 (0x27AD7B..0x27AD89; ctor default 0xFF at 0x219AE4) | **MOVED** PROVEN |
| trailer | none | none (the reader is set to the end of the message at 0x27B8F5..0x27B900) | same |

**LZMA2 parameters.** The `lzma2_compressor` object at network_client+0x4C is built at 0x219A0C → ctor 0x357DD0 (RTTI `.?AVlzma2_compressor@vostok@@`, `compressor_lzma2.cpp`). Its fields:
- dictionary 0x8000 (32 KiB);
- level 9, lc 4, lp 0, pb 2, fb 273 (encoder side only).

The decoder's dictionary-property byte is derived from the dictionary size (`(2|p&1)<<(p/2+11) >= 0x8000` → **prop 6**). It is a constructor constant, not a wire byte. The stream is **raw LZMA2 chunks**, with no .xz/.7z header and no property byte. PROVEN.

**Server shortcut.** LZMA2 allows *uncompressed* chunks:
- the first chunk is `01 <u16 BE size-1> <≤64 KiB data>`;
- further chunks are `02 <u16 BE size-1> <data>`;
- the stream ends with `00`.

So the server needs no LZMA encoder. LIKELY: this is standard LZMA2 format, and the decoder is the stock LZMA SDK `Lzma2Decode` (0x3EE810 checks prop ≤ 40 and builds LZMA props with lc+lp = 4).

---

## 2. `match_options` fields: 0.21d vs 0.26g

`match_options`:
- 0.21d: 0xC4F4 bytes = `player_profile[20]` (stride 0x9D0) + fields; ctor 0x1F5916.
- 0.26g: **0x578A0** bytes = `player_profile[20]` (stride **0x45FC**) + fields; ctor 0x3183A0. Defaults: +577B0 = 0xFFFFFFFF, +577B5 = 0xFF, +577B6 = 0, the 28 floats = 0.0, strings +57840/+57880 empty.

In network_client it is `vfn40()+0xC`. vfn40 = 0x219970 returns `[nc+0x1F1C48]`, a match object whose vtable is 0xA083D8.

| 0.21d wire order (0x1F59A4) | 0.21d field | 0.26g wire order (0x1F0E70) | 0.26g field | status |
|---|---|---|---|---|
| — | — | **1** | `player_profile[20]`, each via 0x1F1070 (§3), **always 20** | **NEW position / count.** 0.21d sent N profiles after the options. PROVEN 0x1F0E81..0x1F0EA2 |
| 1 u8 | C444 level_id (loading picture) | — | from config `mode.ui.loading_screen_id` (read at 0x2204AF) | **REMOVED** from wire. PROVEN (no wire read; config key ref) |
| 2 str | C460 project name (class 0x41) | — | from config `mode.match_making_params.map_name`, e.g. `level_01/day_2` (read at 0x2CC41A..0x2CC454, class **0x40**) | **REMOVED** PROVEN |
| 3 u8 | C448 game_mode | — | config `mode.mode` (2, 3, 4) | **REMOVED** PROVEN (HUD reads `mode` at 0x234D1D → `root.set_game_type`) |
| — | — | **2** u8 | +577B4 **map_id** = `maps_dict[].id`. Looked up by `items_dictionary::map_descriptor_by_id` 0x21F910 (linear search; unknown id → "There is no map with id [%u]" and NULL). Logged as "Map : %s (%d)" | **NEW** PROVEN |
| 4 u8 | C44C player_count N | **3** u8 | +577B5 N (≤ 20) | same meaning, **moved**. PROVEN. Logged as "Players : %d". The match vfn34 0x279610 derives the player-mask byte count `(N-1)/8+1` |
| 5 u8 | C45B battery_count | — | config `victory_items_count` (0x2F24ED, 0x234E18) | **REMOVED** PROVEN |
| 6 u8 | C44D respawn s | — | config `respawn_time` (0x31677A) | **REMOVED** PROVEN |
| 7 u16 | C44E match_duration s | — | config `match_time` (timelimit 0x317E32) | **REMOVED** PROVEN |
| 8 u32 | C440 seed | **4** u32 | +577B0. Logged as "Match id : %d". **It is the anomaly seed**: `generic_anomaly_cook` vfn 0x1F6680 returns it, the same as 0.21d `generic_anomaly_cook::seed` 0x15A16A reading C440. It is also copied to the player object (0x20289E). Other readers, all seeds too (PROVEN): the game world's constructor hands it to the object at +0x108 (0x2D01D7), the victory items mode stores it with each item (0x2F2679), a mode object keeps it at +0x140 (0x3139DF), the player's model loader copies it into the creation struct (0x20289E). The same value must go to every client of a match. **It must not be the match id the lobby's `82` gives**: `network_client::connect_to_match` 0x21BEB0 returns at once when the two are equal (lobby.md §2.2), and the field is cleared only by the constructor (0x3183D2), so a client holds the id of its last match until it is started again. srp sends `registry::static_id(match_id)` (the match id with the top bit set; match ids have it clear) | **MOVED + renamed.** PROVEN (seed role, both builds; the compare) |
| 9 f32 | C454 start_fraction | — | config `wait_player_percent` (0x317E9E) | **REMOVED** PROVEN |
| 10/11 u8,u8 | C458+C459 waiting minutes | — | config `wait_time` (0x317E56) | **REMOVED** PROVEN |
| 12 u8 | C45A countdown s | — | config `countdown_time` (0x317E7A) | **REMOVED** PROVEN |
| 13 u16 | C450 (unused) | **5** u16 | +577B6 "Match level". Logged as "Match level : %d"; **no other reader** (displacement grep) | **NEW** u16. PROVEN read; meaning LIKELY match-making level (cf. `match_levels_dict`). Send 0 or the MM level |
| 14 24×f32 | C480 score weights | **6** **28**×f32 | +577BC..+5782C score weights. `score += round(value × w[e] × k)`, k = 0.01 for events 6, 0x11, 0x12 (0x26DBA2..0x26DBDE), the same formula as 0.21d `base_game_statistics_handler::on_event` 0x19C3D2 | **CHANGED count 24 → 28.** PROVEN. Events 0..23 are LIKELY the 0.21d enum; 24..27 are UNKNOWN |
| 15 N×u8 | C4E0 squad_id[i] | **7** **20**×u8 | +5782C squad_id[20] (HUD `squad_id`/`squad_state` 0x2352A3..0x235367; same-squad test 0x23989F) | **CHANGED count N → 20.** PROVEN |
| — | — | (not on wire) | +577B8 "Match maker id" u32: logged only, never written except by the allocator | not sent. PROVEN (negative grep) |
| — | — | (not on wire) | +57840 "Mode" str[0x40] and +57880 map name str[0x20]: written only by the ctor (cleared). +57880 replaces the config project name only when two query flags are set (offline/test path, 0x2CC3D9..0x2CC40C) | not sent. PROVEN |

`match_options` wire size after decompression = `sum(profiles) + 1 + 1 + 4 + 2 + 112 + 20` = profiles + **140** bytes.

---

## 3. `player_profile` static block

| 0.21d `deserialize_static` 0x1F5C52 | 0.26g 0x1F1070 (+ 0x1F1120) | status |
|---|---|---|
| u8 team → +5AC (u32) | u8 team → **+45EC** (u32, ctor default 3 at 0x2F7CAF) | same. PROVEN. HUD: team == local team → ally (0x234FF5) |
| bool → +5B0 (overwritten with is_local) | — | **REMOVED** from wire; is_local +45F0 is set only by the handler. PROVEN |
| str name → +8 (64-byte buffer) | str name → +8 (buffer +8..+0x47 → **≤ 63 chars**; plain memcpy, **no length check**) | same. PROVEN |
| — | **str clan abbreviation** → **+0x48** (buffer +0x48..+0x56; ≤ 14 safe, ≤ 15 tolerated because the next read overwrites +0x57). HUD shows `"[%s] %s"` (clan, name) when non-empty (0x23506F..0x23509F) | **NEW.** PROVEN wire; "clan abbr" LIKELY |
| u32 → +9C8 (new-player protection: factor = max(0, 1 − 0.1·v)) | u32 → **+45F4**, used as `max(0, 1 − 0.1·v)` (0x332BAA..0x332BE7, const 0x921948 = 0.1) and `== 0` (0x2F4E2A) | same. PROVEN. srp sends 10 |
| `deserialize_slots` 0x1F5D08: u32 mask over 23 slots + items | replaced by 0x1F1120: **kits + item list** (below) | **CHANGED** PROVEN |
| u32 mask over 20 f32, each added to a zeroed base | per kit: **16-byte mask** over **68** f32 (below) | **CHANGED** PROVEN |

**Not on the wire:**
- +0x45F1 (bool) and +0x45F8 (u32). They are copied by the lobby (0x284E64..0x284E90), and +45F1 is set to 1 at 0x284BFC.
- The 0.21d trailing u32 that lobby `36 02` appends after the profile; that belongs to the lobby message, not to the profile.

### 3.1 0x1F1120: kits ("loadouts") and items: wire order

```
u8  K                      -> +0x57    number of kits, capacity 3 (no check: K > 3 overflows)
u8  M                      -> +0x1564  number of items, capacity 69 (0x45) (no check)
K x kit:                                kit k at profile+0x58+k*0x704 (ctor 0x2F7BB0)
    u8  state  (->u32)     -> kit+0x700  ctor default 2
    u32 kit_id             -> kit+0x6FC
    str name               -> kit+0x00  (64-byte buffer, <= 63)
    u32 slot_mask          (23 slots, bits 0..22; loop count 0x17 at 0x1F11DF)
    per set slot i (ascending):
        u16 item_index     -> kit+0x40+i*4   index into this profile's item list
        u16 count          -> kit+0x42+i*4
    (unset slot: item_index = 0xFFFF)
    u64 mod_mask_hi        (bits 64..127)   } 16 bytes, high qword FIRST
    u64 mod_mask_lo        (bits 0..63)     }
    per set bit b (b = 0..67 ascending): f32 -> kit+0x9C+b*4 (added to 0 after the block is zeroed)
M x item:                               item m at profile+0x1568+m*0xB4 (ctor 0x2F7C10)
    see 3.2
```

Evidence: 0x1F112A..0x1F12F5 (kits) and 0x1F12FB..0x1F1339 (items). The mask layout is PROVEN from the bitset shift helper 0x1EFFD0: the constant one = {hi = 0, lo = 1}, shift moves bits from lo into hi, and the `and` pairs wire bytes 0..7 with hi. Kit fields:
- `kit_id` +0x6FC: compared with an id at 0x227C79 and 0x284B4D. LIKELY the kit id.
- `name`: sent to Flash `root.setup_player_profiles` as `name`.
- `state`: sent to Flash as `state`, with `level` and `hint_text` (0x222010, strings 0x9ED668/0xA01498/0xA014A0/0xA014AC).

**`state` matters.** `pvp_match_core_cook` translate_query requests one **player** resource (class 0x54, `gameplay/players/default.player`) for **every kit with state == 0** of every player i < N (0x2CC684..0x2CC7C7). The lobby skips its validity check for state 2 (0x2CA9F0, 0x22201B), and the ctor default is 2. So **send state 0 for every kit the player may use in the match.** The values 1 and 2 are UNKNOWN; GUESS: 2 = locked / not bought.

**Slot second u16 = count.** It is compared with the item's +8 (`amount_in_storage`) and clamped (0x2FCC5F..0x2FCC8D). A slot with count 0 is skipped (0x2FD452). The first u16 indexes the item list (0x2036D9 → `items_dictionary::item_by_id(item[idx].dict_id)` at 0x203742). PROVEN.

**Modifier names (68).** Index i ↔ name table at 0x928448 (a copy at 0x9C9BE8). The skill/item config loader 0x2C840F..0x2C846C fills `float[i] = parameters[name[i]]` for i < 0x44. PROVEN mapping.

- **0–19, same order as 0.21d** `player_params_modifiers_enum`:
  - 0 dispersion, 1 aiming_speed, 2 health_regeneration_speed, 3 stamina_regeneration_speed, 4 movement_speed;
  - 5 max_carried_weight, 6 pain_health, 7 artefact_search_time, 8 anomaly_damage, 9 engineer_use_time;
  - 10 engineer_success_chance, 11 device_damage, 12 splash_radius, 13 reload_speed, 14 artefact_slots;
  - 15 **pocket_slots** (0.21d device_slots), 16 ammo_bags, 17 **pocket_slot_size** (0.21d quick_slot_size), 18 recoil, 19 stamina_spending_speed.
- **New in 0.26g:**
  - 20 movement_speed_when_aiming, 21 chamber_a_round, 22 change_weapon_speed, 23 breath_holding_time, 24 dispersion_movement_influence;
  - 25 use_two_double_handed_weapons, 26 overweight_penalty, 27 grenade_throw_power, 28 stamina_boost_time, 29 stamina_boost_interval;
  - 30 fall_damage, 31 can_sprint, 32 max_carried_weight_ultimate, 33 drug_using_time, 34 drug_activity_time;
  - 35 critical_regen_duration, 36 pain_regeneration_speed, 37 use_artefact, 38 artefact_cooldown, 39 artefact_duration;
  - 40 artefact_power, 41 incoming_weapon_damage_in_anomaly, 42 artefact_synergy, 43 armored_trap, 44 friendly_trap;
  - 45 can_defuse_traps, 46 traps_explode_after_defuse, 47 spot_enemy_traps, 48 spot_artefacts, 49 aim_time;
  - 50 friendly_anomaly, 51 toxic_damage, 52 pain_damage_after_hit_value, 53 pain_damage_after_hit_time, 54 second_chance_damage;
  - 55 second_chance_ramaining_health, 56 dispersion_from_hip, 57 drug_amount, 58 drug_hit_coeff, 59 melee_damage;
  - 60 respawn_time, 61 breath_vibration, 62 move_speed_weapon_penalty, 63 stamina_boost_amount, 64 disable_second_weapon;
  - 65 injury_penalty, 66 stamina_loss_on_hit, 67 stamina_spending_speed_skill.

  All names carry the suffix `_modifier`.

### 3.2 Item descriptor (0.26g 0x1F1350 → 0x1F13D0) vs `inventory_item_descr::deserialize` 0x1F5B53

| 0.21d (mode from table 0x877168: slots 0..7 and 10 = mode 0, others = 2) | 0.26g (mode = `items_dictionary::item_by_id(dict_id)->is_stack`, dictionary_item+0x22A, set from config key `is_stack` at 0x2C9199; unknown id → treated as stack (1)) |
|---|---|
| u16 dict_id (+0C) | **u32 instance_id (+0)** — items are found by it (0x277AC4 lookup, upgrade code) |
| u32 item_id (+08) | **u16 dict_id (+4)**; **if 0 the item ends here** (6 bytes total, the u32 is discarded) |
| mode 0/2: u16 condition_or_stack (+0) | **u32 (+8)**: `condition` for non-stack, `amount_in_storage` for stack (names from `player_profile::serialize_dynamic` 0x2F7CC0 asserts) |
| mode 1/2: u32 amount (+4) | **non-stack only:** u32 `repair_time` (+0xC), u32 `reroll_time` (+0x10) (names: lobby.md §2.6). The reader has a mode argument that drops `repair_time` when it is not 0; the profile reader passes **0** (`push 0` at 0x1F1317), so static info carries both. PROVEN (checked 2026-10-12, 0x1F13E8..0x1F1416) |
| u8 n, n × u32 modifications | u8 n1 (≤ 8, inline capacity 0x40/8), n1 × {**u32 modification_id, u8 state**} → +0x14 vector (stored as `state != 0`, 0x1F1456; state 0 = a modification, else a defect; the client looks the id up in the table of its kind and crashes if it is unknown, for both states: items.md §2) |
| — | u8 n2 (≤ 3, inline capacity 0x48/0x18), n2 × {**u32 upgrade_id, u32, u64, u8**} → +0x60 vector. **NEW.** Upgrades are LIKELY: `process_detach_upgrade` 0x275330 searches this vector by its first u32 (0x2775B0). The fields: the cosmetic's item id, its dict id, the decal editor's u64, the slot (items.md §2) |

Mode table: the 0.21d per-slot `slot_serialize_mode_enum` table at 0x877168 = `0×8, 2, 2, 0, 2×12` (slots 0..7 and 10 = mode 0, the rest = 2). **0.26g has no such table.** The tables at 0x9266A0 and 0x925FA0 are slot *sublists* (pairs index→slot), not modes. PROVEN.

---

## 4. Resource classes and game modes

`pvp_match_core_cook` translate_query: 0.21d 0x1C7889 vs 0.26g **0x2CC270** (vtable 0xA0F1A0 slot 7; cook class 0x5F set at 0x2CBB69).

| request | 0.21d class | 0.26g class | source in 0.26g |
|---|---|---|---|
| level project | 0x41 (name C460) | **0x40** (name = config `match_making_params.map_name`, max 31 chars) | 0x2CC41A..0x2CC478 |
| game_material_manager | 0x52 | **0x51** | 0x2CC4F7 |
| `storm` | — | **0x6D (new)** | 0x2CC525 |
| rules | fixed: 0x59 or 0x5A (if C448 == 3), 0x5B, 0x5C, 0x5D | **from config `mode.rules[]`, each element = class id** (empty name) | 0x2CC573..0x2CC676 |
| players | N × 0x55 `gameplay/players/default.player` | **Σ over i < N of (kits with state 0) × 0x54** same path | 0x2CC684..0x2CC7C7 |

Rule cook class ids in 0.26g. Each is set by the cook's static ctor and named by RTTI; the vtables were found through RTTI COLs.

| class | 0.26g cook (ctor) | 0.21d class |
|---|---|---|
| 0x58 (88) | gather_victory_items_rule_cook (0x2F2390) | 0x59 |
| 0x59 (89) | match_time_vi_spawn_rule_cook (0x2F2400) | 0x5A |
| 0x5A (90) | **tdm_rule_cook** (0x2F41E0) | — new |
| 0x5B (91) | player_respawn_rule_cook (0x2C66A0) | 0x5B |
| 0x5C (92) | **single_player_respawn_rule_cook** (0x2C6710) | — new |
| 0x5D (93) | timelimit_rule_cook (0x2F49F0) | 0x5C |
| 0x5E (94) | kd_stats_rule_cook (0x2CCD90) | 0x5D |
| 0x5F | pvp_match_core_cook (0x2CBB50) | 0x5E |

All PROVEN.

**Game modes** (config `mode.mode`; there are no enum strings in the 0.26g exe):

| mode | rules in config | config parameters | UI |
|---|---|---|---|
| 2 (`st_mode_02`), gather victory items | 93, 91, **88**, 94 | match_time 900, wait_time 5, respawn_time 8, victory_items_count 10, countdown_time 20, wait_player_percent 0.8 | `root.set_artifacts_required` |
| 3 (`st_mode_03`), batteries spawn over time | 93, 91, **89**, 94 | as mode 2, plus vi_spawn_time 60, vi_spawn_variation 10, spawn_points_count 3 | |
| 4 (`st_mode_04`), **team deathmatch with tickets** (new) | 93, 91, **90**, 94 | tickets_count 100 (no victory_items_count) | `root.set_tickets_required` 0x234DD1 |
| 0 | test configs only (loopback_client: rule 92; gather_server_resources) | | |

The 0.21d values 0 and 1 (capture_enemy_base/capture_neutral_base) do not appear in the 0.26g data. The meanings of the mode values are LIKELY, from the rule cook names and the config keys.

**Usable map_ids** (from `maps_dict`): config present **and** project present in `resources.db` (72 projects: `level_0[1-8]/{day,evening,rain|prerain}_{2,3,4}`).

| map | day 2 / 3 / 4 | evening 2 / 3 / 4 | rain (level_08: prerain) 2 / 3 / 4 | loading_screen_id d / e / r |
|---|---|---|---|---|
| level_01 | 61 / 62 / 117 | 63 / 64 / 118 | 65 / 66 / 119 | 0 / 7 / 6 |
| level_02 | 67 / 68 / 120 | 69 / 70 / 121 | 71 / 72 / 122 | 1 / 9 / 8 |
| level_03 | 73 / 74 / 123 | 75 / 76 / 124 | 77 / 78 / 125 | 2 / 3 / 4 |
| level_04 | 79 / 80 / 126 | 81 / 82 / 127 | 83 / 84 / 128 | 5 / 11 / 10 |
| level_05 | 85 / 86 / 129 | 87 / 88 / 130 | 89 / 90 / 131 | 12 / 14 / 13 |
| level_06 | 91 / 92 / 132 | 93 / 94 / 133 | 95 / 96 / 134 | 15 / 17 / 16 |
| level_07 | 97 / 98 / 135 | 99 / 100 / 136 | 101 / 102 / 137 | 18 / 20 / 19 |
| level_08 | 103 / 104 / 138 | 105 / 106 / 139 | prerain 109 / 110 / 140 | 21 / 23 / 25 |

The suffix _2/_3/_4 is the mode.

**Do not use these ids:**
- 107/108 (level_08 night): the configs exist but `projects/level_08/night_*` does not.
- 11, 45, 115, 141: test projects that are absent.
- 114 and 143..151: their configs are missing ("Wrong map config. Map id = %u", 0x2C9D18).

PROVEN (data dump + loader).

**Important:** `maps_dict` lives in the client's **local** `gameplay/db_static_dictionaries` (loaded by `items_dictionary_cook` 0x2C8926/0x2C994A), not in a lobby message. So map_ids are fixed per client build. PROVEN.

---

## 5. Roster model

**Still a fixed roster set up by static info.** LIKELY, and partly PROVEN:
- `player_profile[20]` is always deserialized; N (+577B5) bounds every per-player loop: HUD, query_players 0x27AEFC (via match+0x577C1), translate_query, and `on_match_ready` 0x27AD2A.
- All N players' kits are loaded as player resources at level load (0x2CC684).
- The local player is found by scanning i < N (0x27AD7B).
- The ED/EE match messages also loop over N (0x27C3DE).

The player-mask width is now `(N-1)/8+1` **bytes** (match vfn34 0x279610 → match+0x23D0E8). 0.21d had a fixed u32 mask, so masks in other messages (snapshot, enter/leave) are probably variable-length (LIKELY; see the snapshot/roster notes).

Late joiners must still be in the original N; there is no message that adds a profile. LIKELY (no other caller of 0x1F0E70 besides the handler and journal replay 0x1F096C).

**Constraints:**
- N ≤ 20. Above that, profile[i] indexing overruns; HUD loops trust N.
- local_index < N. Otherwise nc+0x1F1C44 stays 0xFF. The effect is LIKELY a null player/crash, as in 0.21d.

---

## 6. Byte-exact layout of 0.26g static info

All values are little-endian unless noted.

```
E4                          message id (transport/order envelope as for every other match message)
u8   local_player_index     < N
LZMA2 raw stream, to the end of the message, decoding to <= 0x578A0 bytes:
    20 x player_profile:
        u8   team                       0/1
        u8   len, name[len]             len <= 63
        u8   len, clan_abbr[len]        len <= 14 (0 = none)
        u32  protection                 (0.21d +9C8; 10 = none)
        u8   K   kits                   <= 3
        u8   M   items                  <= 69
        K x kit:
            u8   state                  0 = usable in match (a player resource is loaded only for state 0)
            u32  kit_id
            u8   len, name[len]         len <= 63
            u32  slot_mask              bits 0..22
            per set slot ascending: u16 item_index (< M), u16 count
            u64  modifier_mask bits 64..127   (only bits 64..67 meaningful)
            u64  modifier_mask bits 0..63
            per set bit ascending: f32 value
        M x item:
            u32  instance_id
            u16  dict_id                0 -> item ends here (empty)
            u32  condition | amount_in_storage   (dictionary is_stack decides; condition x 10000, lobby.md §2.6)
            if !is_stack(dict_id): u32 repair_time, u32 reroll_time
            u8   n1 (<= 8),  n1 x { u32 modification_id, u8 state }
            u8   n2 (<= 3),  n2 x { u32 upgrade_id, u32, u64, u8 param }
      (unused roster places: an empty profile = 00 00 00 00000000 00 00 -> 9 bytes)
    u8   map_id                 maps_dict id (§4 table)
    u8   N                      roster size, <= 20
    u32  match_id               = anomaly/player seed; = lobby match id
    u16  match_level            logged only
    28 x f32 score weights      events 0..27; k = 0.01 for 6, 17, 18
    20 x u8  squad_id           per roster place; 0 = no squad
    [srp's block]               "SRPX", then {u8 tag, u8 len, len bytes}... (srp, 2026-10-23):
                                the stock reader stops at the squads; exe patch 63 reads tag 1,
                                team damage {f32 max_amount, max_ratio, return_ratio}
                                (team-damage.md); sent only when SRP_TEAM_DAMAGE is not off
```

**Note on `is_stack`.** If `dict_id` is unknown to the client's `items_dict`, the reader treats the item as stack (`mode = 1` at 0x1F139F), so it does not read the two extra u32. The server must use the **same** `is_stack` as the client's `db_static_dictionaries` `items_dict`, or the stream desyncs.

Example. Client 0 of 2, map 61 (level_01 day, mode 2). Player 0 "alice" has 1 kit, with one item in slot 7 that has no modifiers; the other 19 profiles are empty.

```
E4 00
01 HH LL                         LZMA2 uncompressed chunk, size-1 big-endian, then the data:
  00 05 'alice' 00 0A000000 01 01        team 0, name, no clan, protection 10, K=1, M=1
    00 01000000 03 'kit' 80000000          state 0, kit id 1, name, slot 7 set
    0000 0100                              slot 7 -> item 0, count 1
    0000000000000000 0000000000000000      no modifiers
    11223344 <dict u16> 64000000 00000000 00000000 00 00   (non-stack weapon; the condition here is 100 raw = 0.01 %, a whole item is 1 000 000: lobby.md §2.6)
  01 00 00 00000000 00 00                profile 1: team 1, empty (any real player needs a kit)
  (18 x 9 bytes of empty profiles)
  3D 02 <match_id u32> 0000 <28 x f32> <20 x u8>
00                               LZMA2 end marker
```

---

## 7. What srp-021d code must change (server work implied)

**`crates/survarium/src/v021d/match_start.rs` → a 0.26g `static_info`:**
- Put the local index **first**, then LZMA2-wrap the rest. Uncompressed LZMA2 chunks are enough: split at 64 KiB, the first control byte `0x01`, then `0x02`, end with `0x00`.
- Drop from the wire: `level_id`, `project`, `mode`, `batteries`, `respawn_seconds`, `duration_seconds`, `start_fraction`, `waiting_minutes`, the `0` byte, `countdown_seconds` and `u16 0`. Replace them with `u8 map_id` chosen from the §4 table. The mode and timings follow from it, and a custom value needs a client data change.
- New field order: 20 profiles, u8 map_id, u8 N, u32 match_id (seed), u16 match_level, **28** f32 weights (extend `SCORE_WEIGHTS` by 4; GUESS 0.0), **20** squad bytes (pad with 0).
- Always emit 20 profiles; pad unused places with the 9-byte empty profile.
- `MAPS`/`VARIANTS` become map_id tables: variants `day/evening/rain` (level_08: `prerain`), suffixes _2/_3/_4 = modes 2/3/4.
- The server's own rule timings must come from the `game_modes/*.options` of the chosen id (match_time 900 s, wait 5 min, countdown 20 s, respawn 8 s, wait_player_percent 0.8, victory_items_count 10, tickets 100). Note that wait_player_percent is 0.8, not the 0.0 srp used for 0.21d: the server cannot lower it without a data change. The start rule must be re-checked in the 0.26g timelimit (0x317E32..).

**`crates/survarium/src/v021d/lobby.rs` `write_profile_static`.** Lobby profile messages in 0.26g call the same 0x1F1070 (0x27027D, 0x271A81, 0x274BF1). So these change too:
- `u8 team`; **no bool**; `str name`; **new** `str clan_abbr`; `u32 protection` (10).
- Then `u8 K`, `u8 M`, the kits, and the item list as §3.1/§3.2, instead of `u32 slot mask + items + u32 20-bit modifier mask + floats`.
- `write_item` becomes: `u32 instance_id, u16 dict_id, u32 condition_or_amount, [!is_stack: u32, u32], u8 n_mods, {u32 id, u8 flag}…, u8 n_upgrades, {u32,u32,u64,u8}…`. Not `u16 dict, u32 id, u16, [u32], u8 n, u32…`.
- `slot_carries_amount` (the 0x877168 table) is obsolete: use the dictionary's `is_stack` (`items_dict` in 0.26g `db_static_dictionaries`).
- `skills.rs` `MODIFIERS` = 68 with a 128-bit mask (high qword first). The new names are §3.1; the skill tree data (`gameplay/skills/skills_tree.options`) must be re-read for 0.26g.
- The account model needs kits: up to 3 named kits per character, each with state/id/name, 23 slots of (item index, count), and its own modifier block, all sharing one ≤ 69-item list.

**Match core (if the server hosts or mirrors it):**
- Each player's usable kits each become a player resource (class 0x54). Whether the snapshot/roster carries one entry per kit or per player is UNKNOWN; this matters for snapshot work.
- Rule class ids shifted (§4). Mode 4 TDM and the `storm` resource (0x6D) are new.

---

## 8. Open questions

1. Non-stack item fields +0xC and +0x10: **answered**, `repair_time` and `reroll_time` (lobby.md §2.6); 0 for an item that is neither being repaired nor rerolled.
2. The modification flag (u8) in item vector +0x14, and fields 2–4 of the upgrade record (u32, u64, u8) in vector +0x60.
3. Kit `state` values 1 and 2 (only "0 = loaded in match" and "default 2" are known).
4. Score events 24..27 (the weight array grew from 24 to 28). There is no enum string in the exe. **Answered (2026-10-12):** what counts each is in snapshot_rules.md §3.1 (24 solo battery delivery, 25 grenade kill, 26 stolen battery brought home, 27 a new player's match). **The weights have no source** (PROVEN negative): the only code touching options+0x577BC is the reader (0x1F0EE5), the constructor's zeroing (0x3183BA) and the two score sums (0x26DBD6, 0x26DC77); no config has per-event numbers (`victory_screen.options` has categories only; `static_game_parameters`, the nine tables of `db_static_dictionaries`, quests, drops, awards and the mode configs were searched); no capture of an original match exists. So all 28 are srp's own, 24..27 at 0. A hint for one of them (LIKELY): `event_categories` puts the team kill (4) with the kills under "Enemies killed", so the original probably gave it a weight (a penalty).
5. `match_level` u16: confirm that nothing else reads it (the displacement grep found only the log). The "Match maker id" at +577B8 is never set.
6. Whether a match object/player exists per kit or per roster place, and how the players mask (now `(N-1)/8+1` bytes) indexes it. This decides the snapshot layout.
7. The 0.26g slot enum. Still 23 slots, but the per-slot meaning (which slot is primary/secondary/ammo/pocket) was not mapped; see the sublist tables 0x9266A0/0x925FA0.
8. Whether `wait_player_percent` 0.8 blocks the start with fewer players. **Answered (PROVEN, `timelimit_rule_core::logic_tick` 0x318100; details in modes_maps.md §10):** the rule leaves "waiting" when at least one player has entered and `popcount(entered mask) ≥ floor(N × 0.8)`, N = the roster size byte of the match options (+0x577B5, the `u8 N` of §6), the mask = players activated by 0xE9 and not yet left. It is the 0.21d rule with the fraction and the times taken from the mode config instead of the match options. A roster equal to the players present starts its 20 s countdown as soon as 80 % of them (rounded down) have entered; N = 1 and N = 2 start on the first entered player. Roster places that never enter count against the 80 %: with fewer than `floor(0.8 N)` entered the rule waits, and after `wait_time` = 5 minutes of waiting it reports itself completed (the match is over without having started; the consumer of that report was not traced: LIKELY).
9. The flags that make translate_query use the +57880 name override (offline/loopback/journal; not needed for network play).

## 9. Code (added 2026-10-12)

`crates/survarium/src/v026g/match_msgs/static_info.rs`: `MatchOptions` /
`MatchProfile` / `MatchKit` / `MatchItem` in, `StaticInfo::new` checks every
limit of §3 and §5 and writes §6 (stored LZMA2), `StaticInfo::payload(local
index)` is 0xE4's payload; `parse_options` / `parse_payload` read it back in
the client's order for tests. Unused roster places are written as 9 zero
bytes (team 0, not the ctor's 3; the client's loops stop at N).

## 10. Where skills and item modifications come from in a match (added 2026-10-12)

**Skills: the kit's 68 floats are the only source. Item modifications: the client applies them itself. The server must put the skills into the kit's block and nothing of the items.** The builders do that already (`session::v026g::wire::account_body` sums `SkillsTree::modifiers(account.skills)` into each kit; `lobby-server` `play::match_profile` copies it).

- **No skills list reaches a match.** The match profile is team, name, clan tag, protection, the kits and the items (§3.1): nothing else. PROVEN (the reader 0x1F1120).
- **Gameplay reads the kit's block directly.** Every use of a player modifier takes `block = profile [player+0x6B8] + kit [player+0x115] × 0x704 + 0xF4` (the kit's floats: +0x58 + 0x9C) and calls the getter 0x2D5AE0 with the modifier's index (for example 0x2E1B68..0x2E1B9A index 0x31, 0x2D701B..0x2D7031 index 0x24, 0x2DBBFB..0x2DBC1D, 0x304AFC.. index 0x0B). PROVEN.
- **The getter** 0x2D5AE0: `value = block[index] + Σ (a list of extra amounts for that index)`, then `base × (1 + value)` clamped at 0, or `base + value × k` for the indices of table 0x927E08. The lists are in the kit too (20 bytes for each modifier from block+0x110, up to the kit id at +0x6FC). PROVEN.
- **Who adds to those lists** (the only callers of the add function 0x2D5A90): the weapon (0x2E2C9C, 0x2E37E4), a carried battery (0x30177D) and the medkit / booster (0x33D892, 0x33DDC1). All item effects, added by every client from the same data; none is a skill. PROVEN (callers by direct call; no indirect use was looked for).
- So a skill counted in the block is counted once. Whether anything else writes the 68 floats after the reader: not searched exhaustively (LIKELY nothing: the lockstep needs every client to compute the same for every player, and only the options are shared).
- **Item modifications and defects**: `item_config_cook::translate_query` 0x2F1750 (the match's resource cook for a player's item) walks the item's `{id, state}` list, takes each id's row from the dictionary's modifications table (state 0: +0x140, count +0x144, 0x2F1802..0x2F1826) or defects table (else: +0x148 / +0x14C, 0x2F1865..0x2F1890), also each upgrade's `parameters.modification` (0x2F18D0..0x2F1934, table +0x150 / +0x154), counts their `modifiers` (0x2F1977) and builds the "item_config_cook: modifiers" list that is applied to the item's config. PROVEN: the client applies them in the match from its own tables; the server sends the ids only and must **not** add their values to the kit's block (they would count twice).
- Not tried live. A check: carried weight with skill 209, and a weapon with a modification, each against the lobby's numbers.

## Correction (2026-10-12): which kits get state 0

"Send state 0 for every kit the player may use in the match" is not enough: a kit of state 0 with nothing in the weapon slots 7 and 10 ends every game of the match when a player spawns with it (match.md §23). srp sends state 0 only for a kit that passes the client's own Play check (`rules::check_play`), and 2 (`KIT_NOT_LOADED`) for any other.
