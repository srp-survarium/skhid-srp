# Player vs Bots for every level: the bots' equipment tiers

The owner's decisions of 2026-10-23. Code: `survarium::v026g::loadouts` (`BotTier`, `TIER_4_5`,
`TIER_9_10`, `TIER_13_15`, `bot_kits`, `bot_of`), the lobby's `pvb.rs` and `play::launch` /
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
- `play::server_bot(n, team, role, tier)` gives each bot its role's three kits of the tier
  (`loadouts::bot_kits`) in its profile (`MatchProfile::from_loadouts`).
- The match server reads the tier and the role back from the profile it has of each place:
  `loadouts::bot_of` (the bot whose first kit holds exactly the profile's first kit's items; else, a
  profile of an older kit, the lowest tier's role by weapon 1). `game::driven` passes the roles to
  `Bots::new` and the tiers to `Bots::with_tiers`, which arms each bot with its tier's kits
  (`guns::Arms::kits_of`). Nothing else travels: the profile is the contract.

## 3. The tables

One `RoleGear` a role in each tier (`weapon`, `ammo`, `armour` by slot): the one place to change a
bot's weapon or armour. Every kit also has `BOT_POCKETS` (two medkits, the radio, two RGD-5). The
tests check every kit made of them: the slots, the tier's levels (the kit's level is its highest
item's `match_making_level`, `matchmaking::kit_level`), the ammo fits the weapon, the pockets, the
carry limit, four different ammo types in a tier, and the ammo at the most.

Item levels and masses from the server's game data (`items_dict`, the item configs; PROVEN). A kit's
mass is what the lobby counts (`Loadout::weight`: ammo `rounds / clip_size × clip_weight`), with a
life's ammo. The carry limit is 30 kg with no skills (`default.player`); every kit is under it.

| Tier | Role | Weapon 1 (level) | Ammo (level), a life | Armour (level) | Kits: mass (kg) |
|---|---|---|---|---|---|
| 4-5 | flanker | `uzi` (2) | `ammo_9x19p_para` (2), 200 | `bm_torso_1` (1), `bm_back_1` (3), `bm_legs_5` (4), `bm_boots_1` (1) | 17.0, 15.2, 17.1 |
| 4-5 | support | `ak_74u` (5) | `ammo_5.45x39_7n6m` (5), 90 | `bm_helmet_1` (3), `bm_torso_4` (3), `bm_back_1`, `bm_legs_5` | 18.35, 19.9, 19.0 |
| 4-5 | rusher | `toz_34` (1) | `ammo_12mm_buck` (1), 60 | `scav_torso_1` (1), `bm_back_1`, `bm_legs_5`, `scav_boots_1` (1) | 17.0, 16.9, 15.1 |
| 4-5 | camper | `mosin_nagant` (0) | `ammo_7.62x54r_old` (0), 30 | `bm_helmet_1`, `scav_torso_2` (3), `bm_back_1`, `bm_legs_5` | 19.0, 18.35, 19.9 |
| 9-10 | flanker | `pp2000` (9) | `ammo_9x19p_7n31` (9), 150 | `edge_helmet_3` (8), `ren_torso_3` (8), `bm_back_1` (3), `edge_legs_2` (9), `edge_gloves_2` (9), `edge_boots_2` (9) | 19.2, 20.6, 22.5 |
| 9-10 | support | `ak_74m` (10) | `ammo_5.45x39_7n10` (10), 120 | `ren_helmet_2` (9), `edge_torso_2` (9), `bm_back_1`, `bm_legs_5` (4), `ren_gloves_1` (9), `edge_boots_2` | 23.55, 22.35, 25.95 |
| 9-10 | rusher | `spas_12` (7) | `ammo_12mm_buck` (1), 45 | `edge_helmet_3`, `edge_torso_3` (8), `bm_back_1`, `bm_legs_5`, `edge_gloves_2`, `edge_boots_2` | 21.8, 18.5, 19.9 |
| 9-10 | camper | `sv_98` (9) | `ammo_7.62x54_7n1` (9), 30 | `ren_helmet_2`, `ren_mask_2` (9), `ren_torso_3`, `bm_back_1`, `ren_legs_2` (9), `ren_gloves_1`, `edge_boots_2` | 26.65, 24.15, 22.75 |
| 13-15 | flanker | `sr3m` (14) | `ammo_sp6` (14), 120 | `edge_helmet_4` (12), `edge_torso_5` (14), `edge_back_5` (14), `ren_legs_5` (14), `edge_gloves_5` (14), `edge_boots_6` (15) | 23.45, 24.25, 26.05 |
| 13-15 | support | `ak_12` (15) | `ammo_6.5x39` (15), 120 | `edge_helmet_5` (14), `ren_mask_5` (14), `edge_torso_5`, `edge_back_5`, `ren_legs_5`, `edge_gloves_5`, `edge_boots_5` (14) | 27.6, 26.8, 26.6 |
| 13-15 | rusher | `spas_12` (7) | `ammo_12mm_buck` (1), 60 | `edge_torso_5`, `edge_back_5`, `ren_legs_5`, `edge_gloves_5`, `edge_boots_6` | 25.9, 23.3, 24.1 |
| 13-15 | camper | `vsk_94` (14) | `ammo_sp5` (12), 80 | as the support's | 26.6, 27.6, 26.8 |

Why these:
- **Pockets.** A player has one pocket of one unit (`default.player`); the bots need three of two
  (medkits, the radio, grenades; one pocket a type, `one-pocket-a-type.md`). 4-5 and 9-10: `bm_back_1`
  (+1 pocket, +1 size) and one more pocket (`bm_legs_5`, `ren_torso_3` or `bm_torso_4`). 13-15:
  `edge_back_5` (+1 size), `edge_torso_5` and `ren_legs_5` (+1 pocket each). `ren_back_1` (+2 and
  +2, 8 kg) would give pockets of three, but the match server's medkit and grenade plays count two a
  life (`plays::heal`, `reflex` `grenades`), so it is not used.
- **Weapons.** The strongest common pick of each kind at the tier: 9-10 the PP-2000 (the highest SMG,
  9), the AK-74M (10), the SV-98 (the only sniper weapon of 9-10); 13-15 the SR-3M (compact assault
  rifle, 14; no SMG is above 9), the AK-12 (15), the VSK-94 (the sniper category's marksman rifle, 14,
  automatic). The rusher keeps a shotgun, its personality's range: the SPAS-12 is the highest there is
  (7); the kit's armour makes its level. Another agent researches what top players used; its list
  changes only the tables.
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
