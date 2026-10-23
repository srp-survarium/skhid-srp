# 0.26g game data vs 0.21d (resources.db)

**Version scope:** 0.26g `resources.db` (build "0.26g0", file dated Jan 17 2015, 6,285,533,906 B,
`E:\sur-res-pro\downloads\v026g\extracted\Survarium\game\resources.db`) compared with 0.21d
`resources.db` (April 2014, `E:\sur-res-pro\downloads\v021d\extracted\game\resources.db`).
Everything on this page is 0.26g vs 0.21d.

Labels: **PROVEN** = read from the 0.26g file (path and dump given); **LIKELY** = strong
inference from the data; **GUESS** = plausible, not checked. Nothing was run; the game files
were only read. Extraction went to the session scratchpad.

Dumps (UTF-8, Cyrillic de-mojibaked) are in `E:\sur-res-pro\tools\re-kit\v026g\data\`:
- `static_dictionary-026g.txt`, `skills_tree.txt`, `drops.txt`, `quests.txt`,
  `test_player_profiles.txt`, `static_game_parameters.txt`
- `items\gameplay_items_*.txt` (all 389 files under `gameplay/items/`)
- `game_modes\*.txt` (all `gameplay/game_modes/*.options`)
- `other\*.txt` (bundles, clans, awards, effects, player, hit_params, victory screen, ...)
- `localization\*.txt` (all six languages plus `flash_movies_localizations.options`)
- `projects\<level>_<variant>.server_objects.txt`: the `server_objects` section of each map
  `client_project`. `projects\map_object_counts.txt` holds the object counts for both versions.
- Tables: `items_dict-026g.tsv` (id, name, category, stack, premium, base_cost, cfg, NEW flag),
  `item_params_026g.tsv` (per item: faction, affinity, item_level, mm_level, rarity, weight,
  drop_weight, degrade, reliability, repair ratio, clip), `game_modes-026g.tsv`
  (maps_dict id ↔ mode file ↔ map/weather/mode/rules/params).

## 0. Binary config format

- **PROVEN:** the format is unchanged. srp-021d's `binary-config-cli print-binary-config`
  parses all 508 `gameplay/*` files, all 19 `localization/*` binaries and all 72 map
  `client_project` files with no error. The header is the same (24-byte root value,
  type/count u16; the dictionary root has 9 children instead of 8).
- **PROVEN:** strings are UTF-8 in both versions. The parser decodes them as Windows-1251 in
  both (`deserialize.rs`), so its Cyrillic output is mojibake. That is not a 0.26g change.
- The EULAs (`localization/*/eula`) are plain text, as before.

## 1. File list diff (counts, `resources_list.txt` of each version)

| area | 0.21d | 0.26g | added | removed | size changed | MB 0.21d → 0.26g |
|---|---|---|---|---|---|---|
| gameplay | 124 | 508 | 398 | 14 | 109 | 1.3 → 2.8 |
| projects | 99 | 155 | 92 | 36 | 63 | 533.5 → 66.9 (the .veg files shrank from 517 MB to 7 MB) |
| localization | 12 | 19 | 11 | 4 | 8 | 0.7 → 2.7 |
| flash_movies | 7 | 9 | 2 | 0 | 6 | 25.6 → 90.7 |
| textures | 4,546 | 18,876 | 15,566 | 1,236 | 29 | 2,523 → 5,384 |
| models | 16,982 | 26,549 | 11,593 | 2,026 | 2,296 | 359 → 841 |
| sounds | 7,782 | 14,450 | 9,841 | 3,173 | 217 | 153 → 209 |
| shaders | 19,237 | 14,746 | 14,544 | 19,035 | 74 | 394 → 271 |
| animations | 1,891 | 1,217 | 739 | 1,413 | 193 | 50 → 32 |
| material_instances | 1,925 | 2,341 | 445 | 29 | 1,485 | 26 → 32 |
| particles | 94 | 88 | 34 | 40 | 54 | 7 → 9 |
| total | 52,702 | 78,961 | | | | |

Removed from gameplay/projects/localization (all **PROVEN**):
- `gameplay/db_skills_tree`. It is replaced by `gameplay/skills/skills_tree.options`, a new format.
- `gameplay/effects/client/*`. Moved to `gameplay/effects/`; `effect_irradiation` is gone and
  `effect_storm` is new.
- `artefact_rattle`, `scope_m4a1_ironsight`, `ammo_12mm_buck2`, `wpn_test`.
- `localization/*/swear_filter_data_bin`, replaced by `swear_filter_data_a_c`.
- **All 18 `projects/level_0X/{day,evening,rain}/server_project` files.** 0.26g ships no
  `server_project` at all. Map projects are now `projects/level_0X/<weather>_<mode>/client_project`.

New in gameplay (**PROVEN**): `awards/unique_awards.options`, `bundles/*` (13 files),
`clans/clan_name.options`, `game_modes/*` (113 files), `item_categories`,
`skills/skills_tree.options`, `ui/victory_screen.options`, and items under `boosters/`,
`craft/`, `money/`, `reroll/`, `upgrades/camouflages/**`, `upgrades/decals/**` and
`vouchers/`, plus many new weapons and armour.

New UI (**PROVEN**): `flash_movies/replay_player.swf`, and `flash_movies/lobby_menu.swf.orig`,
a leftover copy 2.6 KB smaller than `lobby_menu.swf`. lobby_menu.swf grew from 16.5 MB to 39 MB.
New languages: german, polish. New file: `localization/flash_movies_localizations.options`.

## 2. db_static_dictionaries (`gameplay/db_static_dictionaries`, dump `static_dictionary-026g.txt`)

Top-level sections (**PROVEN**):

| section | 0.21d | 0.26g |
|---|---|---|
| items_dict | 83 | **384** |
| categories_dict | 24 | **29** |
| slots_restrictions | 77 | 77 (identical) |
| items_compatibility | 41 | **94** |
| factions_dict | 6 | 6 (identical) |
| boosters_dict | 14 | **removed** |
| perks_dict | 36 | **removed** |
| skills_dict | 5 | **removed** |
| shop_bundles_dict | — | **new**, 14 |
| maps_dict | — | **new**, 89 |
| match_maker_defines | — | **new** |
| match_levels_dict | — | **new**, 21 |

**srp-021d `Dictionary::parse` fails on 0.26g as written.** It calls `req("skills_dict")` and
`req("perks_dict")`, and requires `item_level` in every item. 0.26g has none of these.

### items_dict (PROVEN; table `items_dict-026g.tsv`)
- Item fields: 0.21d had `dict_id, name, item_category, is_stack, is_premium, item_level, cfg_name`.
  0.26g has **`base_cost`** in place of **`item_level`**. `item_level` moved to each item
  config's `parameters`, next to the new `match_making_level`.
- 384 items, ids up to 737. 82 of 0.21d's 83 ids are kept with the same name and cfg.
  - Removed: id 73 `ammo_12mm_buck2`.
  - Changed: id 13 `ak_74u` moves from category 15 to 8. Artefacts 54, 74 and 76 become
    `is_stack: true`.
- 302 new ids:
  - **50 camouflages** (category 25), **120 decals** (category 26), 4 vouchers (27),
    `generic_money` 214 (28), `premium_money` 215 (29).
  - **6 boosters as items** (ids 339–344, category 10, 1500 each).
  - New weapons: ak_74m 3, svd 63, rpd 77, sv_98 149, vepr 216, a_545 220, rpk 222, a9_91 303,
    vsk_94 305, ak_74n 306, val 307, vss 308, sr3m 309, sr2m 321, ump45 324, ak_12 363,
    ash_12 364, vssk 365, socom16 366, pp2000 367.
  - "_premium" weapon copies 332–338. New ammo, scopes `scope_pso1` 304 and `scope_giperon` 362,
    grenade `rdg-m` 219, and armour tiers 2–6 for every faction.
- `is_premium: true` on 123 items: most decals and camouflages, the vouchers, and 14 gear items
  (ak_74n, the *_premium weapons, scav_torso_4/legs_4, bm_torso_4/legs_4 and their premium copies).
- Category counts in 0.26g: 1:14, 2:8, 3:24, 4:10, 5:23, 6:14, 7:17, 8:15, 9:9, 10:10, 11:1,
  12:3, 13:10, 14:8, 15:10, 16:2, 17:5, 18:8, 19:2, 20:9, 21:1, 22:3, 24:2, 25:50, 26:120,
  27:4, 28:1, 29:1.
- `base_cost` gives a real price for each item (e.g. ak_74m 47500, ak_74u 12920,
  scav_oxygen_1 2380, 5.45x39 7N6M 60). **LIKELY** this is the shop price in generic money.
  Ammo `base_cost` is **per clip** (settled, see "Settled in step 4" below). srp-021d's
  hand-made `prices.rs` ladder is dropped for 0.26g (`v026g/prices.rs`).

### categories_dict (PROVEN)
- Ids 1–24 are kept; 25–29 are new, all with min/optimal 0.
- min/optimal counts changed for the weapon categories:
  - 8: 60/120 → 10/60
  - 13: 20/40 → 10/20
  - 14: 20/40 → 10/30
  - 15: 60/90 → 30/90
  - 16: 40/120 → 30/100
  - 17: 20/40 → 10/40
- The meaning of 25–29 comes from the items in each: 25 camouflage, 26 decal, 27 voucher,
  28 generic money, 29 premium money.
- New `gameplay/item_categories`: `item_category_booby_traps: 11`, `item_category_grenades: 24`.

### slots_restrictions (PROVEN)
- Identical to 0.21d: 77 pairs and **23 profile slots (0–22)**, so `PROFILE_SLOTS = 23` still holds.
  - Slot 0 → cat 1, 1→2, 2→3, 3→4, 4→5, 5→6, 6→7.
  - Slots 7 and 10 → {8, 13–17}.
  - Slots 8, 9, 11, 12 and 19–22 → {9, 18–21}.
  - Slots 13–18 → {10, 11, 24}.
- Categories 12 (artefacts), 22 (scopes) and 25–29 have no profile slot. **LIKELY** cosmetics
  attach to an item rather than a slot, since armour configs have `camo_available` and
  `decal_available`.

### items_compatibility (PROVEN)
- Removed pairs: (7,3) and (81,77). Both now appear in the reverse order or under new ids.
- 55 new pairs, listed in the dump.
- Dangling ids, with no item: 79 and 101 (dangling in 0.21d too), and **735, 738, 739, 740,
  741**, which are not in items_dict. The server should skip unknown ids.

### factions_dict (PROVEN)
- Identical: ids 1–6 with the same level thresholds and names.
  - Scavengers and Black Market: 0/100/420/990/1840/3000.
  - Renaissance and Edge: 11 levels up to 35250.
  - Scientists and Mercenaries: one `st_temp` level.

### boosters / perks / skills dictionaries (PROVEN removed)
- 0.21d `boosters_dict` (id → prop_id 100+id) is gone. Boosters are now items (339–344,
  `gameplay/items/boosters/*.options`) with named `*_modifier` parameters.
- Perks have **no data anywhere** in 0.26g: no `perks_dict`, and no "perk" string in any
  gameplay file or in the English localization.
- `skills_dict` is gone. Skill ids now live only in `skills/skills_tree.options` (§3).

### New dictionary sections (PROVEN)
- `shop_bundles_dict`: id 1–14 → `gameplay/bundles/*.options`.
  - 1 premium 3d, 2 1w, 3 1m, 4 3m, 5 6m, 6 1y, 7 1d.
  - 8 blackmarket_starter, 9 ak74n, 10 izh27, 11 sr3m, 12 vityaz, 13 vss, 14 scavenger_starter.
  - The bundle files carry **only UI** (name/description/icon). The contents and price are not
    in the client: **LIKELY** server-side.
- `maps_dict`: id → `cfg_mode_name` (a `gameplay/game_modes/` file). See §6.
- `match_maker_defines`:
  - `items_levels_modifiers` (21 rows: modifier_value 0..20 ↔ item_level ranges 0–15).
  - `elo_modifiers` (6 rows: elo 100–10000 → 0, then 1000-wide bands up to 15000 → 5).
  - `slots_weight` (23 × 1).
  - `wait_modifier: 1`.
  - **LIKELY** inputs for the server's matchmaking (gear level + ELO).
- `match_levels_dict`: 21 rows of `money_k`, `experience_k` and `reputation_k`, rising with
  the row (money 1 → 9.5+, exp and rep 1 → 1.65+). **LIKELY** match-result multipliers per
  match level (= mm level of the gear), used by the server.

## 3. Skills (`gameplay/skills/skills_tree.options`, dump `skills_tree.txt`)

**PROVEN:** a completely new format. 0.21d's tree was 5 skills × 20 levels of
`boosters{id,value}`, with id-1 = player modifier index and values in percent.

0.26g has `skills_tree[26]`, one entry per skill:
- `id`, `levels_amount`;
- `preconditions{branch_points, antiskills[]}`;
- `modifiers[levels]{<name>_modifier: value}`, using **modifier names, not ids**, and
  **fractions, not percents**;
- `ui{position[col,row], name, description, icon, postfix, invert_sign, skill_descriptions}`.

Two branches: 101–113 (weapon handling) and 201–214 (body/stamina).

| id | modifier (per level) | levels | branch_points | antiskill |
|---|---|---|---|---|
| 101 | aim_time −0.02 | 5 | 0 | |
| 102 | movement_speed_when_aiming +0.01 | 5 | 0 | |
| 103 | chamber_a_round +0.04 | 3 | 5 | |
| 104 | change_weapon_speed +0.075 | 2 | 5 | |
| 105 | breath_holding_time +0.15 | 3 | 10 | |
| 106 | dispersion_from_hip −0.02 | 5 | 10 | |
| 107 | dispersion_movement_influence −0.02 | 5 | 10 | |
| 108 | reload_speed +0.04 | 3 | 10 | |
| 109 | ammo_bags +1 | 1 | 15 | |
| 110 | injury_penalty −0.1 | 5 | 15 | |
| 111 | use_two_double_handed_weapons 1 | 1 | 20 | 113 |
| 112 | move_speed_weapon_penalty 1 | 1 | 15 | |
| 113 | disable_second_weapon 1 + pocket_slots 1 | 1 | 20 | 111 |
| 201 | stamina_spending_speed_skill −0.02 | 5 | 0 | |
| 202 | overweight_penalty −0.05 | 5 | 10 | |
| 203 | grenade_throw_power +0.1 | 2 | 5 | |
| 204 | stamina_regeneration_speed +0.02 | 5 | 0 | |
| 205 | stamina_boost_amount 0.1, interval 0, time 100 | 2 | 15 | |
| 206 | meele_hit_ultimate 1 | 3 | 50 | |
| 207 | fall_damage −0.15 | 2 | 10 | |
| 208 | movement_speed +0.01 | 5 | 10 | |
| 209 | max_carried_weight +2 | 5 | 5 | |
| 210 | meele_hit_range 1 | **0** | 50 | |
| 211 | stamina_loss_on_hit −0.05 | 5 | 15 | |
| 213 | can_sprint 1 | 1 | 20 | 214 |
| 214 | max_carried_weight_ultimate 1 | 1 | 20 | 213 |

Consequences:
- srp-021d `skills.rs` (booster id → modifier index, ÷100, 20 levels) **does not apply**.
- `branch_points` = points spent **in the rows above** of the same branch, and `antiskills` =
  mutually exclusive skills: **PROVEN** in the lobby's skills window (see "Settled in step 4").
- The 0.21d 20-float `player_params_modifiers_enum` profile block probably grew or changed.
  The 0.26g skill tree alone names about 25 new modifiers. The enum order must come from the
  0.26g exe: **open**.

### Modifier names across all gameplay configs (PROVEN, counted over dumps)
- 0.21d had 26 names; 0.26g has 64.
- Removed: `device_slots_modifier`, `multiplicative_movement_speed_modifier`,
  `quick_slot_size_modifier` (now `pocket_slot_size_modifier` / `pocket_slots_modifier`).
- New on every item: `aim_time`, `breath_vibration`, `melee_damage`, `respawn_time`,
  `pocket_slot_size`, `pocket_slots`, plus `money_bonus`, `experience_bonus` and
  `reputation_bonus`.
- New skill-only names are in the table above.
- New UI props 123–133 (`static_game_parameters`):
  - `st_multiplicative_reliability/respawn_time/breath_vibration_modifier`;
  - `st_money/experience_modifier`;
  - `st_scav/bm/ren/edge_rep_modifier`;
  - `st_inv_stopingpower`.

## 4. Item configs (`gameplay/items/**`, dumps `items\*.txt`, table `item_params_026g.tsv`)

### Changes for every item (PROVEN)

New `parameters` keys on every item kind:

| key | example value |
|---|---|
| `item_level` | 0–16 |
| `match_making_level` | |
| `rarity[]` | gear: 4 levels 0..3; stacks: a `(min,max)` pair, e.g. ammo (60,150) |
| `degrade_per_use` | |
| `degrade_per_match` | |
| `degrade_per_death` | |
| `available_modifications_mask` | 40-char bit strings |
| `available_defects_mask` | 20-char bit strings |
| `type` | string such as `wpn_aslt`, `arm_tors`, `ammo`, `drugs`, `trap`, `grenade`, `vch_name` |
| `default_modifications[]` | modification ids, e.g. armour 1050 |

Gear also gains `reliability` and `repair_cost_ratio`.

Removed keys:
- `type_mask` (replaced by `type`);
- **`drop_value`**, which srp-021d drops use: there is no drop_value in 0.26g;
- `quick_slot_item_size`;
- ammo `clips_in_bag` (now `required_bags`) and `ammo_type`.

### Durability, repair and modding: all new (PROVEN)
- `degrade_*` and `reliability` per item.
- `modifications.options` has 158 modifications (0.21d had 36). Each has
  `modifiers{path[], value, type}` with a scalar value (0.21d's was a list), `lobby_info`,
  `type_mask` and `type_requirements_mask`.
- It also has 20 **defects**, each with `fix_cost_general`, `fix_cost_gold`, `fix_time_general`
  and modifiers.
- `defect_slots` gives condition thresholds 0.9/0.5/0.1 with probabilities.
- How the masks, the types and the slots are read, and what the client knows of the file: items.md §1.
- `craft/craft.options`:
  - `craft_price_general[16]` (400 … 20000) and `craft_price_premium[16]` (2 … 100), by item level;
  - `craft_probabilities_general/premium`.
- `reroll/reroll.options`: `reroll_price_general/premium[16]` and
  `reroll_modificator_multiplicator 1.1`.
- **LIKELY** "craft" means upgrading an item (adding a modification), which the lobby string
  "Can't upgrade damaged item" supports. Reroll re-rolls a modification.
- Vouchers (`vouchers/*`) have `voucher_type` and `ttl_in_hours`: repair kit, rename account,
  drop card, 1-day premium.

### Armour (PROVEN)
- `hit_params/<bodypart>/hit_types/<type>` keeps only `armor`.
  - `reduce` is gone (it is also gone from `human_hit_params`).
  - The `blunt` hit type is gone (7 types instead of 8).
- `parameters`: `reduce`, `arms_reduce`, `back_reduce` and `ano_reduce` are gone. `armor`,
  `arms_armor`, `back_armor` and `ano_armor` keep new values. Example: scav_torso_1 armor
  0.2→0.1, ano_armor 1→0.1.
- `additional_slots{artefact_slots, device_slots}` is gone from armour.

### Weapons (PROVEN)
- New:
  - a `melee` sub-config;
  - `stopping_power`, `stamina_damage`, `aim_time`, `aim_offset`, `min_damage_amount` and
    `min_damage_distance` in parameters;
  - a new recoil model (`min/max_{top,left,right,shake}_recoil`, `recoil_power`, PID
    `compensation_*_gain`), which replaces 0.21d's first-shoot/queue-shoot parameters.
- The `damage{}` block is gone; its values sit in `parameters`.
- Many values were retuned. For example ak_74u: base_dispersion 1.8→0.3, bullet_damage
  0.4→0.5, max_damage_distance 40→15, reload 3.5→3.167.

### Weights of the test loadout items (PROVEN, `parameters/weight`)
- Unchanged for most items: ak_74u 2.7, aps 1.2, spas_12 4.4, toz_34 3.2, toz_66 1.8,
  bm_torso_3 10, ren_torso_1 10, ren_boots_1 1.2, ren_legs_1 2.1, scav_oxygen_1 8,
  scav_resp_1 0.5, scav_torso_1 3, scav_torso_2 6, scav_gloves_1 0.2.
- Changed: **ren_helmet_1 1.7→1.4**, **scav_back_1 4→3**.
- Ammo: `clip_weight` for 5.45 7N6M 0.5→0.25 with clip_size 50→30. 12 mm clip_size 20→15.
- `drop_weight` changed on every item (10 → 80–160).
- `default.player`: `max_carried_weight` 25→30, `min_carried_weight` 15→20, `ammo_bags_count`
  4→3, and `quick_slot_pocket_size 10` replaced by `pocket_slots_count 1` / `pocket_slot_size 1`.

### test_player_profiles (PROVEN, `test_player_profiles.txt`)
- Same format: 10 profiles of 13 names each. Every name exists in 0.26g items_dict.
- Contents changed:
  - profile 0 now has edge_torso_5, scav_oxygen_1, edge_gloves_2, ak_74u + 5.45 ammo;
  - the others use ren_*_4/6, edge_helmet_5, toz_66/toz_34, bm_torso_3, scav_torso_1/2.
- Summed weight of the gear alone (no ammo), per profile: 0.21d 25.3 kg each; 0.26g 30.5,
  30.0, 36.1, 24.6, 17.0, 24.0, 20.0, 24.0, 24.0, 24.0. With one clip of each of the four
  ammo (`clip_weight`): 32.35, 32.3, 38.4, 26.9, 19.3, 26.3, 22.3, 26.3, 26.3, 26.3.
- Profiles 0 and 2 are over the new 30 kg `max_carried_weight` on gear alone, and profile 1
  with its ammo. `v026g/loadouts.rs` drops these three (corrected in step 4: this page said
  only profile 2).

## 5. drops, quests, static_game_parameters, other configs

### drops (`gameplay/drops/drops.options`, dump `drops.txt`): PROVEN, restructured
- 0.21d had a flat structure: `slots_open`, `drop_amount`, `item_category_probabilities`
  (nested), `item_level_probabilities` and `modifier_probabilities`.
- 0.26g has **four drop tables**: `generic_drop`, `new_player_drop`, `daily_bonus_drop` and
  `drop_card_drop`. Each has:
  - `item_level_probabilities[21][16]`;
  - `item_category_probabilities[]{drop_weight, categories[]}`. The generic table: weight 60 →
    category 0 (= nothing?), 5 → cosmetics 25/26, 5 → vouchers 27, 9 → generic money 28,
    1 → premium money 29, 10 → armour 1–7, 10 → weapons 8/13–17;
  - `rarity_points[5]`;
  - `item_level_independent_categories[]` (10, 11, 24–29).
- Also new:
  - `playtime_dependence{play_period 60, games_without_drop_bonus_coef, modifications_steps[4], drop_item_probabilities_bonus[{games,bonus}]}`;
  - `game_mode_params{high_threshold 0.6, low_threshold 0.2}`.
- srp-021d `drops.rs` must be rewritten. Item value now comes from `item_level` and `rarity`,
  not `drop_value`.

### quests (`gameplay/quests/quests.options`, dump `quests.txt`): PROVEN
- Root: 0.21d had `quest_givers`, `tasks[271]`, `types[11]` and `quests[271]`. 0.26g has only
  `quests[369]`.
- Quest entry keys are the same: `awards, daily, faction, faction_representative, id, params,
  preconditions, priority, ttl_minutes, type, ui`.
- `ui.task_description` is removed; `ui.task_descriptions[]` now holds string pairs.
- Award types are unchanged: 1 = reputation (param = faction), 2 = money (param 0).
  Precondition types are unchanged: 0 cooldown, 1 min rep, 2 max rep.
- **The quest `type` enum was renumbered.** Map by quest name:

  | goal | 0.21d | 0.26g |
  |---|---|---|
  | bring_x_items | 6 | 6 |
  | steal_box | — | **7** |
  | find_artifacts | 10 | 10 |
  | entrap_enemies | — | **11** |
  | kill_enemies | 12 | 12 |
  | kill_enemy_x | 15 | removed |
  | grenade_enemies | — | **16** |
  | meele_kill | — | **17** |
  | headshots | 16 | **18** |
  | killstreak | 19 | **21** |
  | win_x_games | 28 | **30** |
  | kd_ratio | 29 | **31** |
  | accuracy | 30 | **32** |
  | win_score | 32 | **34** |
  | defend_items | 33 | **35** |

  srp-021d `quests.rs::kind` constants must change.
- Ids run from 10101 to 41113. Factions: 63/64/121/121 quests.
- The localization objective keys lost their per-level suffix: `st_*_obj_N` became `st_*_obj`.

### static_game_parameters (dump `static_game_parameters.txt`): PROVEN
- Same structure. `ui_properties` grows from 46 to 57 (props 123–133 added).
- prop 8 renamed `st_inv_ano_reduce` → `st_inv_ano_armor`, plus minor min/max/direction edits.

### Other (PROVEN)
- `awards/unique_awards.options`: one-off awards for `tutorial_completed` (money 1000 +
  type 0 amount 250), `game_world_tutorial_completed`, `survival_*_notes_completed` and
  `tutorial_change_name`. Award type 0 is **LIKELY** experience; type 2 param 1 is **LIKELY**
  premium money (gold).
- `clans/clan_name.options`: `account_name_regex`, `clan_name_regex` and a homoglyph table.
  Account names allow Cyrillic, Latin, Ukrainian and German letters and `-_+`, with a space
  only inside the name.
- `ui/victory_screen.options`: 3 score categories, and `event_categories[28]`.
  **LIKELY** `match_stats_events` grew from 24 to 28.
- `victory_items/default.options`: `spawn_parameters` moved into the mode files
  (`vi_spawn_time`, `vi_spawn_variation`).
- `blackmarket_trap`: damage 0.85→0.6, `defuse_by_hit: ranged`.

## 6. Maps and modes (summary; details in the sibling `modes_maps.md`)

- **PROVEN:** 8 maps (0.21d had 6). Weather variants:
  - level_01–07: day, evening, rain;
  - level_08: day, evening, prerain (`night_2/3` mode files exist, but there is no
    `projects/level_08/night_*`).
- Each weather has a project per mode: `<weather>_2`, `_3`, `_4`. That makes 72
  `client_project` files and no `server_project`.
- Names: 01 Rudnya, 02 School, 03 Vostok Radar Station, 04 Chemical Plant, 05 Tarakanovsky
  Fort, 06 Mamayev Kurgan, 07 Vector Laboratory, 08 Cologne.
- **PROVEN** `gameplay/game_modes/*.options` has
  `mode{mode, name, parameters, ui{map_name, mode_name, description, loading_screen_id, icon}, match_making_params{map_id, map_weather_id, map_name}, rules[]}`.
  - Mode 2 "Protective device" (gather batteries): `victory_items_count 10`, rules 93,91,88,94.
  - Mode 3 (VI spawn): `spawn_points_count 3` (2 on level_08), `vi_spawn_time 60`,
    `vi_spawn_variation 10`; rules 93,91,89,94.
  - **Mode 4 "Team Deathmatch"** (new): `tickets_count 100`, or 150 on levels 05/08;
    rules 93,91,90,94.
  - All three: `match_time 900`, `respawn_time 8`, `countdown 20`, `wait_time 5`,
    `wait_player_percent 0.8`.
  - Weather ids: 1 day, 2 rain, 3 evening, 4 night, 5 prerain.
  - Test modes: tt5, artefact_testlevel, tdm_test, loopback_client (rule 92);
    `gather_server_resources` (mode 0, id 142).
  - maps_dict has ids with no file: 114 `level.options`, 143–151 (`*_day_5`, `test/dtb*`,
    `primitive_1`).
- **PROVEN** server objects now come only from `client_project`:
  - `server_objects{respawn_points, victory_items_containers, victory_item_spawners, victory_item_spawn_points}`;
  - `client_objects` with `damage_zone`, `generic_anomaly_core`, `artefact_container`,
    `effect_zone`, and new `resupply_zone` (2 per map) and `victory_items_spawn_box` (mode 3).
  - Mode-4 projects have no containers or spawners.
  - `team_base_points` appears nowhere.
  - Counts per project are in `projects\map_object_counts.txt`.

## 7. What the 0.26g server must re-extract or re-handle

| srp-021d embedded file (`resources/binary-config-examples/`) | 0.26g counterpart | status |
|---|---|---|
| `db_static_dictionaries_021d` | `gameplay/db_static_dictionaries` (0.26g) | **changed**: new parser (no skills/perks/boosters; `base_cost` in place of `item_level`; bundles, maps, mm defines, match levels) |
| `test_player_profiles_021d.options` | `gameplay/test_player_profiles.options` | same format, new contents |
| `drops_021d.options` | `gameplay/drops/drops.options` | **rewrite** (4 tables, rarity, playtime bonus) |
| `quests_021d.options` | `gameplay/quests/quests.options` | parser mostly reusable; **type enum renumbered**, 4 new goals, no `tasks`/`types` |
| `db_skills_tree_021d` | `gameplay/skills/skills_tree.options` | **rewrite** (named modifiers, fractions, preconditions/antiskills, 26 skills) |
| `v021d/item_params.rs` (generated) | item configs | regenerate: no `drop_value`/`clips_in_bag`; add item_level, mm_level, rarity, degrade, reliability, repair_cost_ratio, default_modifications, required_bags |
| `prices.rs` (hand ladder) | `items_dict.base_cost` | replace with data |
| — | `items/modifications/modifications.options` | **new**: modifications 158, defects 20, defect slots |
| — | `items/craft/craft.options`, `items/reroll/reroll.options` | **new**: craft/reroll prices by level |
| — | `bundles/*`, `shop_bundles_dict` | new: ids and UI only; contents are server-side |
| — | `game_modes/*`, `maps_dict` | new: map/mode ids for the match options |
| — | `match_maker_defines`, `match_levels_dict` | new: matchmaking level and reward multipliers |
| — | `awards/unique_awards.options`, `clans/clan_name.options` | new: tutorial awards, name rules |

New server features implied by the data (**LIKELY**):
- **Two currencies**: generic money and premium money (gold) appear as items 214 and 215, and
  defect fixes and craft have `*_gold` / `*_premium` prices.
- **Item wear and repair**: degrade per use/match/death, reliability, repair_cost_ratio, the
  repair voucher, and defects with fix cost and time.
- **Upgrades**: modifications, craft and reroll.
- **Cosmetics**: camouflages and decals, applied to items.
- Item levels 0–16 and ELO-based matchmaking.
- Clans, with registration cost and invites (59 new `st_clan_*` strings).
- Daily bonus drops and drop cards.
- Tutorial awards.
- Premium bundles 1 day to 1 year.
- TDM mode.

## Settled in step 4 (2026-10-12; code in `crates/survarium/src/v026g/`)

Evidence: the 0.26g exe (VAs) and `flash_movies/lobby_menu.swf` decompiled with FFDec
(`tools/re-kit/common/ffdec`; class and line of the decompiled ActionScript given).

- **Ammo `base_cost` is per clip, not per round. PROVEN for the client's own use of it;
  LIKELY for the server's price list.**
  - The exe gives `base_cost` (dictionary_item+0x230, read at 0x2C919F) to the lobby as the
    item descriptor's `cost` (0x286950; member name string 0xA015C4).
  - The buy/sell dialog's price is for one slider step, and ammo's step is its `clip_size`
    (`MessageAsk.update_slider`: `_step = clip_size`; `onSliderUpdate`:
    `slider / _step * _price`).
  - Selling uses the descriptor's `cost` with that step (`InventoryList.sellObject`:
    `cost / 4` a step). So the client treats `base_cost` as a clip's price. The shop price
    (0x8A) goes through the same dialog and step, so the list's cost is a clip's too.
  - The buy dialog sends its count **in steps** (`MessageAsk`: `count = slider / _step`), as
    0.21d did.
  - Sanity: per clip a 5.45 clip is 60 next to a medkit at 20; per round it would be 1800.
- **Other price rules (PROVEN, what the lobby shows):** sell = `cost / 4`, times
  `condition / 100` for an item that is no stack; repair =
  `(1 - condition / 100) * repair_cost_ratio * cost` + the defects' `fix_cost_general`
  (`MessageRepair.init`); a premium item's `cost` is gold and is multiplied by the premium
  exchange rate (service price 4) first. This answers half of open question 3: premium
  gear's one cost field is its gold price.
- **Price list `level` and `faction_id` (PROVEN):** `GangShop.setupShopItems` reads
  `faction_id` as the seller and `level` as the reputation level needed
  (`canBuy = _unlockedLevel[seller] >= level`); **seller 0 is always buyable**. One dict id
  may be listed under several sellers. Which level an item needs is not in the client; srp
  uses `item_level` (Scavengers, Black Market) and `item_level - 5` (Renaissance, Edge)
  [LIKELY: gear levels are 1..5 and 6..15, the factions have 5 and 10 levels above 0, and it
  reproduces the January 2015 wiki ranks in `v021d/shop_ranks.md`].
- **Who decides the seller and the rank: the server alone (2026-10-12).** The exe keeps the
  0x8A records as a flat array and gives each to the lobby as `{faction_id, level, dictId,
  cost}` (0x223190); `setupShopItems` files the item under the record's seller with the
  record's level as its lock, and the item's own config (`faction`, `item_level`) is not
  read for the shop. PROVEN. Two records of one dict id are two shop items, one a seller:
  the descriptor's `sellers` array gets both (`noSeller` / `push`), the seller tabs test
  each (`InventoryList` `isSeller`), an owned item shows a seller's icon only when it has
  one seller (`InventoryList.createItem`), and `_itemsBuyDict[dictId]` is set when any of
  them sells to the player. PROVEN in the ActionScript, not yet seen live. A buy names the
  shop item's seller; with none named the exe names the first record's (0x288BC0).
- **The exceptions and the supplies' sellers (2026-10-12, `prices.rs` `EXCEPTIONS`, from
  `progression_sources.md` §3, wiki revisions of January and February 2015 and patch
  notes):** 9A-91 and VSK-94 at Edge 8 and 9 (configs say Renaissance); the chemical set
  and the Amber respirator at Scavengers 2; Fort 17 at Renaissance 2, APS at Edge 2; the
  "Marodeur" container at Black Market 4 (config: Scavengers); ammo, drugs, the trap and
  grenades by the factions at ranks, an item two factions sold having two records; the
  boosters and the 9x19 7N21 (on no list) stay with seller 0. **Since 2026-10-13 ammo
  needs no rank** (the owner's choice, as the game from 0.27a on; `AMMO_NEEDS_RANK`): its
  records keep their sellers and carry level 0, which the lobby shows as buyable at any
  reputation (`canBuy = _unlockedLevel[seller] >= level`). A buy goes through any seller
  of the item whose rank the account has [srp's choice].
- **The factions' camouflages, the Vityaz and the Lancaster (2026-10-17, the owner's rules;
  `prices.rs`):**
  - **A faction's own camouflage is sold by that faction, at no rank, for its `base_cost` in
    silver.** A faction's own camouflage is one of category 25 whose config's `type` is
    `stk_camo` (the game's "stock" camouflage), that is not premium and has a `base_cost`
    (`is_shop_camo`). Its seller is the folder its config is filed in under
    `gameplay/items/upgrades/camouflages/`: `scavengers` the Scavengers, `black_market` the
    Black Market, `renesance` Renaissance (`CAMO_SHOPS`); the configs' own `faction` is 0, so
    the folder is the only thing in the data that names the shop. The record's level is 0.
    A camouflage is no stack: a step is one item, and the buy dialog pays silver for what is
    not premium (`MessageAsk`: `_money_type = isPremium`), so the price is `base_cost` as
    for any other silver item. The live data (stock with patches 12, 14 and 17) has eight,
    each item level 1 and drop weight 0, so nothing gave them before: Black death 154
    (8,000), Ifrit 162 (3,000), Denim 163 (1,000) of the Black Market; Oakley 169 and
    Huntsman 170 (10,000 each) of Renaissance; Ghost 184 (8,000), Quilted fabric 185
    (1,000), Seeker 187 (3,000) of the Scavengers. The same folders hold 19 drop camouflages
    (`drp_camo`, `base_cost` 1) and premium ones (`prm_camo`, sold one by one for gold,
    `premium.md`): their `type` keeps them out. The wiki of January 2015 lists some of the
    eight at ranks (Quilted fabric 1, Seeker 3, Ghost 5; `progression_sources.md` §3); not
    applied, the owner's rule is no rank. A shop camouflage in a folder no faction sells from
    (a new patch's, or one of the Edge's) stops srp at start, naming it (`tables::check`),
    so none is left out of the shop unseen.
  - **The New Year 2015 camouflage stays unsold** (`camo_new_year`, 737; `prm_camo`, not
    flagged premium, `base_cost` 0): an event camouflage, unavailable by the owner's
    decision. Its price of 0 would make it free.
  - **The PP-19-01 Vityaz (19) and the MP-27M Lancaster (108) are sold to everyone**: seller
    0, level 0 (`EXCEPTIONS`; their configs name no faction, so the rule sold them nowhere).
    The Lancaster for its `base_cost`, 2,500; **the Vityaz for 25,000, the owner's price**
    (`OWNER_PRICES`; its `base_cost` is 12,000 and stays so in the data). Both still drop,
    and their premium copies stay in gold bundles 12 and 10. The VSS, the SR-3M and the
    SOCOM 16 stay unsold. The shop shows the record's cost (`GangShop.setupShopItems`:
    `item.cost = p[i].cost`), but the item's descriptor keeps the game's (`descrData.cost`
    is set from the record only when the exe gave none): an owned Vityaz shows 12,000, sells
    for a quarter of it times its condition (at most 3,000; the lobby shows it so and the
    server pays it so, `prices::sell_price` takes `base_cost`), and is repaired by it. So
    buying for 25,000 and selling back cannot make silver. PROVEN in the ActionScript and
    the code; not yet seen in a running game, nor the camouflages in the factions' tabs.
  - **The starting weapons are sold to everyone** at the owner's prices (`OWNER_PRICES`,
    seller 0, level 0; the game prices all three at 0): the Mosin carbine and the PPSh-41
    1,000 each (2026-10-18), the TT-33 500 (2026-10-23: "it should be there and cost
    500"; before it, it was in no shop, so no Buy anywhere). Selling one back pays a quarter
    of the game's 0. Their free ammo is listed at no cost.
- **What a new account owns (2026-10-12, `loadouts.rs`):** the TT-33, the Mosin carbine and
  the PPSh-41 with a clip of each free ammo (7.62x25, 7.62x54R): the wiki of February 2015
  names the three as every new player's, and they are the only weapons and ammo with
  `base_cost` 0 and item level 0. PROVEN (wiki of the time). The first kit wears the TT-33
  and its clip as before; the rest lies in the inventory [srp's choice: what a new player
  wore is not known]. Accounts made earlier are not given the items.
- **Skill rules (PROVEN in the UI, `survarium.char.CharWindow`):**
  - `branch_points`: the skills of a branch are walked by row (`ui.position[1]`), then id,
    adding up the points of the skills before; the walk stops at the first skill whose
    `branch_points` is more than that sum; reached rows unlock
    `floor(branch_points / 5) * 5`, at most 25 (`getBranchUnlockedLevel`, `SKILL_LEVELS 25`,
    `POINTS_PER_LEVEL 5`). So a row needs its `branch_points` spent in the rows above it.
  - Skills 206 and 210 (`branch_points` 50) can never be bought.
  - `antiskills` lock each other while one has points (`handleAntiskills`).
  - Points available = level - Σ points (exe `fill_char_info` 0x224420, as lobby.md §2.12).
  - That the original server checked the same rules is LIKELY.
- **The 68 modifier names**: re-read from the exe's table 0x928448 by script; they match
  static_info.md §3.1. `stamina_regeneration_speed_modifier` is the exe's spelling (item
  configs also carry a misspelt `stamina_regenation_speed_modifier`, which the exe's table
  does not have).
- **Carried weight (PROVEN UI):** ammo weighs `count / clip_size * weight`, other stacks
  `weight * count` (`AmmoShareControll` 211, 217); ammo's `weight` is its `clip_weight`
  (LIKELY, exe 0x286BA1).
- **Drops are server-only (PROVEN negative):** the exe has no `rarity_points`,
  `item_level_probabilities` or `playtime_dependence` string. srp's reading of the tables
  (all LIKELY, reasons in `v026g/drops.rs`): category 0 = no drop; only items with
  `drop_weight` > 0 drop (cosmetics of type `drp_*`; premium weapon copies have 0);
  `rarity_points` are band bounds (`[0, 1, 1.6, 1.9, 2]` = 50/30/15/5 % for rarities 0..3;
  the new player's table always gives rarity 3, the daily bonus rarity 4 = 50 gold, the
  fifth pair of `premium_money`); a stack's `rarity` pair is its count range (ammo: 2 to 5
  clips), gear's `rarity` list its rarity levels. This answers open question 5 as far as the
  data goes.
- **LZMA2 stored chunks vs the 32 KiB dictionary:** the client's `Lzma2Decode` (0x3EE810)
  uses the output buffer as the dictionary (`dic = dest`, `dicBufSize = destLen`, stores at
  0x3EE84B / 0x3EE84F: PROVEN), so a 64 KiB stored chunk is no concern (LIKELY for the
  chunk copy itself, which is stock LZMA SDK code not traced here). Python's liblzma decodes
  srp's streams with `dict_size` 0x8000 (test `a_real_decoder_reads_the_streams`).

## Open questions
1. 0.26g has no `server_project`. If 0.26g matches are still lockstep (client simulation, as
   in 0.21d), the client must build its world from `client_project.server_objects` +
   `client_objects`. This needs a check in the 0.26g exe; it is not in the data.
2. The `player_params_modifiers` enum in 0.26g: its order and size, and how named skill and
   item modifiers map onto the profile floats the server sends. This needs the exe.
3. ~~Is `base_cost` for ammo per round or per clip?~~ Per clip (settled above). Premium
   items' `base_cost` is gold (settled above); what the original server charged for the
   non-premium bundle weapons and for cosmetics is still open. Who sold ammo and
   consumables (their configs name no faction): settled 2026-10-12 from the wiki of the time
   (above).
4. Bundle contents and prices, and the premium durations of the bundles: not in the client. **2026-10-12: the contents and durations are in the localization's bundle texts (premium.md §1); the prices are srp's.** **Later the same day: the bundle prices and the 90 days of premium are sourced (premium.md §1).**
5. Meaning of the `rarity` arrays: gear has 4 levels; stacks have (min,max) counts? Also the
   `rarity_points` in drops, and `category 0` in the drop category tables.
6. Ids 735 and 738–741 in compat, and maps_dict ids 114 and 143–151, have no item or file.
   Probably content stripped from the client (**GUESS**).
7. Whether `match_levels_dict` row = the match's mm level (0–20), and how the client shows it.
8. Award type 0 / param semantics in `unique_awards` (experience? gold?). **2026-10-12: settled, premium.md §5.**
9. 28 victory-screen event categories versus 0.21d's 24 `match_stats_events`: which 4 are new
   (likely steal_box, grenade, melee, entrap kills, going by the new quest goals; **GUESS**).
