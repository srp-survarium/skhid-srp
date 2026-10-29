# Player vs Bots for every level: the bots' equipment tiers

The owner's decisions of 2026-10-23. Code: `survarium::v026g::loadouts` (`BotTier`, `TIER_4_5`,
`TIER_9_10`, `TIER_13_15`, `RusherKit`, `bot_kits`, `bot_of`), the lobby's `pvb.rs` and `play::launch` /
`play::server_bot`, the match server's `bots::guns` (`GUNS`, `Arms`), `bots::recoil` (`KICKS`),
`bots/mod.rs` (`Bot::pick_kit`, `Bot::new_life`, `Bots::with_tiers`) and `game::driven`. Tags as in
the other notes: PROVEN (code or data read), LIKELY, GUESS.

## 1. Every level may play against bots

- Until 2026-10-23 the lobby refused a player whose equipment level (`matchmaking::gear_level`, the
  client's own, of the kits it can play) was over `SRP_PVB_MAX_GEAR`, default 5: code 1
  (`PvbCode::GearTooHigh`). `SRP_PVB_MAX_GEAR` now defaults to 15, the highest level there is, so
  nobody is refused for a level; an operator can still set a lower most.
- The lobby movie has no level check of its own (PROVEN, `MatchMakingSettings.as`: `pvbWhy` greys the
  entry only by the code the server sends, and by the squad and clan-team boxes). It shows code 1's
  text with the numbers the server sends (`PVB_MAX_GEAR_DEFAULT` "5" only when the server sends none,
  which it always does). So the movie was not changed and needs no release.

## 2. The tier: from the player to the match server

- `play::launch` takes the highest `gear_level` of the match's players (a match against bots has one;
  a party or a room would take its highest member's) and `BotTier::of_gear`: 0-5 → `Low` (bots of
  level 4-5), 6-10 → `Mid` (9-10), 11-15 → `High` (13-15). The owner's examples: 3 → 4-5, 8 → 9-10,
  12 → 13-15. The log says "N bots with equipment of level <tier>".
- `play::server_bot(n, team, role, equipment)` gives each bot its role's three kits of the tier
  (`loadouts::bot_kits`) in its profile (`MatchProfile::from_loadouts`). The equipment is the tier and
  the kit its rusher wears (`BotEquipment`, §3.1): the lobby's setting `SRP_PVB_RUSHER_KIT`.
- The match server reads the equipment and the role back from the profile it has of each place:
  `loadouts::bot_equipment_of` (the bot whose kits hold exactly the profile's kits' items; else, a
  profile of an older kit, the lowest tier's role by weapon 1). `game::driven` passes the roles to
  `Bots::new` and the equipments to `Bots::with_tiers`, which arms each bot with its tier's kits
  (`guns::Arms::kits_of`). Nothing else travels: the profile is the contract, so what a bot carries is
  what it fires.

## 3. The tables

One `RoleGear` a role in each tier (`weapon`, `ammo`, `armour` by slot): the one place to change a
bot's weapon or armour. Every kit also has its pockets (`loadouts::bot_pockets`: two medkits, two bandages, two bear traps, the radio where a pocket is left; the support has one RGD-5 in place of the traps where a fourth pocket is open). The
tests check every kit made of them: the slots, the tier's levels (the kit's level is its highest
item's `match_making_level`, `matchmaking::kit_level`), the ammo fits the weapon, the pockets, the
carry limit, four different ammo types in a tier, and the ammo at the most.

Item levels and masses from the server's game data (`items_dict`, the item configs; PROVEN). A kit's
mass is what the lobby counts (`Loadout::weight`: ammo `rounds / clip_size × clip_weight`), with a
life's ammo. The carry limit is 30 kg with no skills (`default.player`) and 40 kg with the bots' skills (209 at five). Of the owner's kit (2026-10-26) one kit, the 13-15 camper's second, is over 30 (30.2 kg): the bots' skills carry it, and none is near 40.

| Tier | Role | Weapon 1 (level) | Ammo (level), a life | Armour (level) | Kits: mass (kg) |
|---|---|---|---|---|---|
| 4-5 | flanker | `vityaz` (4) | `ammo_9x19p_para` (2), 250 | `bm_torso_1` (1), `bm_back_1` (3), `bm_legs_5` (4), `bm_boots_1` (1) | 18.1, 16.55, 19.1 |
| 4-5 | support | `ak_74u` (5) | `ammo_5.45x39_7n6m` (5), 120 | `bm_helmet_1` (3), `bm_torso_4` (3), `bm_back_1`, `bm_legs_5` | 18.5, 19.8, 19.4 |
| 4-5 | rusher | `mp_153` (4) | `ammo_12mm_buck` (1), 75 | `scav_torso_1` (1), `bm_back_1`, `bm_legs_5`, `scav_boots_1` (1) | 19, 18, 16.45 |
| 4-5 | camper | `rem_700` (5) | `ammo_7.62x51_ap` (5), 40 | `bm_helmet_1`, `scav_torso_2` (3), `bm_back_1`, `bm_legs_5` | 20.6, 19.7, 21 |
| 9-10 | flanker | `pp2000` (9) | `ammo_9x19p_7n31` (9), 200 | `edge_helmet_3` (8), `ren_torso_3` (8), `bm_back_1` (3), `edge_legs_2` (9), `edge_gloves_2` (9), `edge_boots_2` (9) | 20.8, 23, 24.2 |
| 9-10 | support | `akmn` (6) | `ammo_7.62x39_7n23` (9), 120 | `ren_helmet_2` (9), `ren_torso_3` (8), `bm_back_1`, `bm_legs_5` (4), `ren_gloves_1` (9), `edge_boots_2` | 22.95, 20.75, 24.35 |
| 9-10 | rusher | `spas_12` (7) | `ammo_12mm_buck` (1), 60 | `edge_helmet_3`, `edge_hunter_torso_1` (6), `bm_back_1`, `bm_legs_5`, `edge_gloves_2`, `edge_boots_2` | 23.6, 20.2, 22.4 |
| 9-10 | camper | `sv_98` (9) | `ammo_7.62x54_7n1` (9), 40 | `ren_helmet_2`, `ren_mask_2` (9), `ren_torso_3`, `bm_back_1`, `ren_legs_2` (9), `ren_gloves_1`, `edge_boots_2` | 27.95, 26.55, 24.35 |
| 13-15 | flanker | `sr3m` (14) | `ammo_sp6` (14), 150 | `edge_helmet_4` (12), `edge_torso_5` (14), `bm_back_1` (3), `ren_legs_5` (14), `edge_gloves_5` (14), `edge_boots_6` (15) | 26.15, 26.85, 28.85 |
| 13-15 | support | `ak_12` (15) | `ammo_6.5x39` (15), 150 | `edge_helmet_5` (14), `ren_mask_5` (14), `edge_torso_5`, `bm_back_1`, `ren_legs_5`, `edge_gloves_5`, `edge_boots_5` (14) | 28.9, 28.2, 27.8 |
| 13-15 | rusher | `spas_12` (7) | `ammo_12mm_buck` (1), 75 | `edge_torso_5`, `bm_back_1`, `ren_legs_5`, `edge_gloves_5`, `edge_boots_6` | 28.7, 26, 26.7 |
| 13-15 | camper | `vsk_94` (14) | `ammo_sp5` (12), 100 | as the support's | 29.1, 30.2, 29.5 |

### 3.1 The rusher's kit

The shotgun rusher lost its fights (K/D 0.52 at 13-15 and 0.55 at 9-10 against 1.08 to 1.48 for every
other role in the owner's eight matches); no player of kit level 9-10 carries a shotgun (14 of 10,339
lives) and the SMG is what 12 and more players of both tiers carry most (41 % of the alive time at 9-10,
22 % at 13-15). The rusher of the two higher tiers can wear an SMG of its tier; the switch is **off by
default** (`control`, the old table) until a bench train turns it on (`RusherKit`, the lobby's
`SRP_PVB_RUSHER_KIT`, the bench's `SRP_BENCH_RUSHER`). The RK arm sweep (smoke preset, 16 maps, 10 a
side, seeds 1-8) found `9-10=alt,13-15=smg` the best setting (`fighting-record/look-rk-sweep.md`):

| `RusherKit` | 9-10 | 13-15 |
|---|---|---|
| `smg` | `sr2m` + `ammo_9x21` (150), `ren_helmet_2`: 19.6 kg | `fn_p90` + `ammo_5.7x28` (200), `edge_helmet_5`: 25.7 kg |
| `alt` | `ump45` + `ammo_45acp` (150): 21.5 kg | `kriss_vector` + `ammo_45acp` (200): 26.6 kg |
| `slug` | `spas_12` + `ammo_12mm_slug` (45), `ren_helmet_2`: 23.4 kg | the same (60), `edge_helmet_5`: 28.3 kg |
| `helmet` | `spas_12` + buckshot (45), `ren_helmet_2`: 23.1 kg | the same (60), `edge_helmet_5`: 27.9 kg |
| `control` (the default) | the old kit: `spas_12` + buckshot (45), `edge_helmet_3`: 21.8 kg | the old kit: `spas_12` + buckshot (60), no helmet: 25.9 kg |

`control` is the old table exactly (what the switch gives when unset); `helmet` changes the helmet alone,
`slug` the ammo on top of it, `smg` the weapon on top of that. The lowest tier has only its table.
The flanker's third kit is the rusher's weapon with the flanker's armour (`BotRole::kit_roles`: P90 or
SR-2M, 200 or 150 rounds at its bags). `fn_p90` and `kriss_vector` are 0.29a3 weapons that come with
the live patches: the repo's test data (`resources/game-026g`) lacks them, so with that data the
13-15 rusher wears the old kit (a line says so; `RusherKit::worn_in`), and the tests that need them
run on the server's game data (`SRP_GAME_DATA`, the `on_the_live_data_` tests, each run alone).

### 3.2 The respirator (batteries, 2026-10-27; `SRP_PVB_MASKS`, off by default)

The cloud and gas zones (School's fruit cloud, Vostok Radar Station's gas, Chemical Plant's moss, Mamayev Kurgan's acid) hit one part, the
`face`, with toxic damage, which goes to the infection pool (no regeneration, death at 0). Only a face item
covers it: `scav_resp_1` 0.30, `ren_mask_2` 0.05, `ren_mask_3` 0.50, `edge_resp_1` 0.50, `edge_resp_3` 0.65. The tier tables' kits wore none
(4-5), `ren_mask_2` in one role (9-10) and `ren_mask_5` in two (13-15), while players in the bot-free live recordings
(`batteries-record` `human_masks_by_tier.py`, kits of exactly the tier's levels, the top players) wear:

| tier (kit levels) | kits | no mask | most worn | next |
|---|---|---|---|---|
| 4-5 (4-5) | 143 | **66 %** | `scav_resp_1` 31 % | `bm_mask_1` 3 % |
| 9-10 (9-10) | 1,246 | 22 % | `ren_mask_3` 22 % | `scav_resp_1` 18 %, `ren_mask_2` 17 % |
| 13-15 (13-15) | 295 | 30 % | `edge_resp_3` 21 % | `ren_mask_3` 17 %, `ren_mask_2` 12 % |

`SRP_PVB_MASKS=top` puts the most worn one (`BotTier::top_mask`) in the mask slot of every role of 9-10 and 13-15; the lowest tier has none,
because most of its top players wear none. The kit carries it as `BotEquipment::mask: Option<Mask>`, set once where the profiles are made
(`RusherKits::masks`, `Masks::mask_of`); a profile says it back (`bot_equipment_of`).

Every kit is under the 30 kg a player carries with no skills (the heaviest, `slug` at 13-15, is 28.3
kg), and the bots' skills lift their capacity to 40 kg, so the weight costs a bot at most 0.415 of the
20 % of speed a full load costs (`bot_skills`, the rusher's own kit 0.285 at 13-15 and 0 at 9-10 with
the SMG). The SMG kits are lighter than the old one: 19.6 against 21.8 kg at 9-10, 25.7 against 25.9 kg
at 13-15 (the 2 kg helmet included).

Why these:
- **Pockets.** A player has one pocket of one unit (`default.player`); the bots need three or four of
  two (medkits, bandages, traps or a grenade, the radio; one pocket a type, `one-pocket-a-type.md`).
  `bm_back_1` (+1 pocket, +1 size) in every tier, and `bm_legs_5`, `ren_torso_3`, `bm_torso_4` or
  `edge_hunter_torso_1` (+1 pocket each); 13-15: `edge_torso_5` and `ren_legs_5` (+1 pocket each),
  with `bm_back_1` and not `edge_back_5` (+1 size only), which makes a fourth pocket. Four pockets in
  13-15 for every role, and in 9-10 for the rusher (`edge_hunter_torso_1`) and the support
  (`ren_torso_3`); 4-5 and the other 9-10 roles have three, and 4-5's support four. `ren_back_1` (+2 and
  +2, 8 kg) would give pockets of three, but the match server's medkit plays count two a life
  (`plays::heal`), so it is not used. The owner's choice of 2026-10-26 (`SRP_BOT_KIT=1` turns it on; off
  unless set, the kit is that of before it): bandages for the pain pool, traps for the placing habit, one grenade for the
  support where a pocket is free, the radio only where a pocket is left, and skill 109's extra ammo
  bag (`bot_skills::AMMO_POUCH`: 102, 104 and 108 on the way, none of them aim, damage or protection).
- **Upgrades** (`loadouts::BOT_UPGRADES`, the owner's of 2026-10-26; on with `SRP_BOT_UPGRADES=1`, off
  unless set). The safe set only: gear weight, carry capacity, movement speed and stamina spent; never an aim
  time, a range or a bolt's chamber time. One upgrade a bot at 4-5, two at 9-10 and 13-15, at the
  strengths the players of the tier have (live matches: speed 4 / 6 / 12 % of pieces at 1.8 / 2.7 / 2.7,
  weight 6 / 10 / 15 % at 0.8 / 1.6 / 1.8, carry 4 / 3 / 6 % at 1.9 / 1.5 / 2.2). Flanker and rusher:
  movement speed on the torso (+3 % at 4-5, +4 % above) and, at 9-10 and 13-15, stamina spent on the
  boots (-6 %); support and camper: carry on the legs (+2 kg at 9-10, +3 kg at 13-15) and gear weight
  on the torso (-8 % at 4-5, -10 % above). The clients apply them (the profile's item carries the
  ids); the match server's speed prior reads the speed, the carry and the weight
  (`bot_skills::profile_movement_level`). **Weapon and gloves upgrades**
  (`loadouts::BOT_WEAPON_UPGRADES`, the owner's of 2026-10-27, the same switch): none at 4-5; at 9-10 the
  rusher and flanker reload 10 % faster (type 5, `2042`) and the support and camper carry a weapon 8 % lighter
  (type 7, `2061`); at 13-15 reload 12 % faster (`2043`) plus gloves with +10 % reload speed (type 26, `1142`),
  and 10 % lighter (`2062`). `Gun` follows them (`guns::gun_with`, `Gun::of_kit`): the weapon's mods by the
  client's rule (`modifications::apply`: the values of a key add, then one multiplication, floor 0; PROVEN),
  the reload speed of the skills and worn gear as `reload_time / (1 + sum)` (LIKELY), and the aim time of the
  bots' skill 101 (-10 %, PROVEN) so the sights come up when the client's do. Off, every `Gun` is the table's.
  Worn gear that carries reload speed of its own counts too: `edge_gloves_2` (the 9-10 rusher and flanker) has
  +8 % by default. The weapon's own `movement_speed_modifier` (+0.05 / -0.05) is in the speed prior.
- **Weapons.** What the top players used in matches of 6 players or more (the live server's match
  logs, 2026-10), where it is a stock item that fits the tier; else the strongest common pick of the
  kind. 4-5: the Vityaz (SMG, 4), the MP-153 (semi-automatic shotgun, 4), the Remington 700 (bolt
  action, 5) with `ammo_7.62x51_ap`; the support keeps the AK-74U (the lowest assault rifle, 5). 9-10:
  the AKMN with `ammo_7.62x39_7n23` (the top players' assault rifle there), the PP-2000 (the highest
  SMG, 9), the SV-98 (the only sniper weapon of 9-10); the rusher the SR-2M (§3.1; the Vityaz would
  share the PP-2000's 9x19). 13-15: the SR-3M (compact assault rifle, 14), the AK-12 (15), the VSK-94
  (the sniper category's marksman rifle, 14, automatic), the rusher the FN P90 (§3.1; ported item, not
  in the tests' game data (`resources/game-026g`), so its tests run on the server's).
- **Ammo of its own.** Each kit's ammo differs from the bot's other kits' (four ammo types in each
  tier), so a kit picked when one runs out is full.

## 4. Each kit's ammo, maxed

- **The most a kit takes** (PROVEN in the lobby movie, `AmmoShareElement.as`): an ammo type gets at
  most `int(bags × clipsInBag) × clipSize` rounds, `clipsInBag = 1 / required_bags`, `clipSize` its
  `clip_size`, the bags other ammo slots use taken off; in one slot or spread. Bags: the player's
  `ammo_bags_count` (3) plus the worn items' `ammo_bags_modifier` (LIKELY the exe's count: the lobby
  gets it from the exe, `set_ammo_bags_count`). Each bot kit puts that most in slot 8
  (`loadouts::ammo_max`, `BOT_AMMO_SLOTS`). A test checks every kit. (Before: three slots of 200
  rounds, 600, three times the lobby's most for the Uzi.)
- **What a spawn gives** (PROVEN, read 2026-10-23): `inventory::setup_from_profile` (0x2FCB70) gives
  each stack slot `min(slot count, the profile item's storage)` and takes that from the storage
  (0x2FCC5F..0x2FCCBD; the storage is the item's `amount_in_storage`, `static_info.md`); the removal
  before a spawn (`unload_to_profile` 0x2FD2D0) puts the magazine and what is left back into the
  storage (`client_patch.md` §3.20). So every life starts with the slot's rounds while the storage
  has them, and only the rounds fired are gone. Each kit's items are its own
  (`MatchProfile::from_loadouts`), so a kit chosen for the next life (0xDE → 0xF3; the kit byte must
  be under 3, 0x321790) brings its own storage. What limits a "fresh full reserve" is only that
  storage.
- **The storage** is `BOT_AMMO_LIVES` (6) lives' worth of each kit's slot [choice: finite, so a bot
  that fought through a kit switches]: 18 full lives over three kits. The lobby writes it as the
  ammo item's `condition_or_stack` (`SlotItem::stored`).
- **The pocket stacks' storage** (medkits, bandages, traps, the support's grenade, the radio) is the same
  mechanism: a spawn fills a pocket slot with `min(slot, storage)` and a death gives back what is left
  (`setup_from_profile` 0x2FCB70, `unload_to_profile` 0x2FD2D0, PROVEN), so with the storage at the stack's count a bot's two
  medkits or the support's one RGD-5 were for the match. Since 2026-10-29 every pocket stack has `BOT_POCKET_STORE` (100)
  in storage (`PocketStore::PerLife`, the owner: "storage put 100+ should be enough"), so every life starts with the
  stack. It is the bots' tuning switch `kit.pocket_store` (off: the count in storage, c3dc7241's profiles), read where a
  match is made (`MatchRegistry::with_bot_pocket_store`) and, in the bench, by each team (`bench_roster`). A resupply box
  tops pocket stacks up from the storage too (`inventory::resupply` 0x2FDC10 -> `inventory_item::restock` 0x2FFCE0, PROVEN,
  `fact-b-resupply.md`), so a bot at a box has its grenade or medkits back mid-life; a human with storage gets the same. The
  bench's stand-in does not model that for the stacks (play 27, off by default); the client test settles it.

## 5. Switching kits (the match server)

- A bot keeps, for each kit, the rounds its storage has left (`Bot::kits`); a life starts with
  `min(a life's rounds, the storage)` (`Bot::new_life`), as the client does.
- `Bot::pick_kit`, after play has begun: only when the present kit has less than a magazine left for
  the match; then the other kit with the most rounds; when every other kit has less than a magazine
  too, that one only if it has more rounds than the present one; none with no round. Chosen once and
  sent as the place's kit choice (0xF3, as a player's 0xDE is relayed) ahead of its next spawn
  (`take_kit_choices`); it does not go back and forth while the present kit runs down. Any number of
  kits.
- Out of a life's ammo with storage left, a bot is dry until its next spawn in the same kit: the
  reflex lets no shot go, takes cover rather than peeking, and the plays give it no fight play (hunt,
  push), while it still collects and carries batteries (`plays` `dry`, match 0x5402a730). It carries
  no pistol (the owner's of 2026-10-16) and has no melee or suicide input.

## 6. The new weapons' facts (`bots::guns`, `bots::recoil`)

From each `wpn_<name>.options` (magazine, rate, first fire mode, reload time, show and hide, its
states) and the clips its states name (`animations/single/weapons/<folder>/item/stand/3rd_view/`,
lengths read as `srp-0100b`'s `combat/pose.rs` does; the Mosin's read back as `guns.rs` had them).
PROVEN (read); the timing rules are `guns.rs`' (LIKELY).

| Weapon | Rounds | A minute | First mode | Reload | Other |
|---|---|---|---|---|---|
| `pp2000` | 20 | 750 | automatic | magazine 2.8 s | show/hide 0.5 s |
| `ak_74m` | 30 | 650 | automatic | magazine 3.167 s | the `ak` clips |
| `sr3m` | 20 | 900 | automatic | magazine 3.167 s | |
| `ak_12` | 30 | 650 | automatic | magazine 2.8 s | modes -1, 1, 3 |
| `vsk_94` | 20 | 600 | automatic | magazine 3.367 s | its first mode is automatic |
| `sv_98` | 10 | 220 | one round | round by round: start 48 f, 0.5 s a round, end 62 f | bolt `chamber_a_round` 95 f |
| `spas_12` | 8 | 240 | one round | round by round (the `mp153` clips): start 47 f, 0.375 s a shell, end 34 f | fire state 315: `shoot` 20 f and `shoot_end` 20 f |

**GUESS:** the SPAS-12's fire state 315 was not read in the exe. The bot takes `shoot_end` as the
action worked after each shot (667 ms before the next press), so it never presses before the gun can
fire; at worst it fires a little slower than it could. The kick of each (`KICKS`) is its config's
`recoil` (PROVEN).

## 7. Not verified

- Nothing of this was run with a real game: the tiers' kits reaching a client, the kit choice
  (0xF3) with three kits, a bot's life refilled from its storage.
- The ammo bags a kit has (the modifiers' sum) is LIKELY, as is the pockets' (`loadouts::pockets`).
- The production game data has the weapon balance patches (`simulation_changes.tsv` 21, 26, 27)
  over the test data the tests read; they change damage, piercing and the A-545's rate, none of the
  numbers above.
