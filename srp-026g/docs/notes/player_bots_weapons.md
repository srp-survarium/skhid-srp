# Player bots: the weapons top players use, and how they fire them

2026-10-23. What the best players of the 0.26g server carry at each equipment
level and how they fire each kind of weapon, and what the server's bots now do
with it (`servers/match-server/src/v026g/bots/guns.rs`, `reflex.rs`,
`tuning.rs`; the weapons' numbers from `crates/survarium/src/v026g/weapons.rs`).
Players are named by label only. Tags: PROVEN (read exactly in the data or the
configs), ESTIMATED (measured with a known error), LIKELY, GUESS.

## 1. Data and method

- **Recordings:** 631 recordings of the players' server.
  - Every PvP match with 6 or more players, started from 2026-10-16 19:55 to
    2026-10-22 23:36 UTC (4.9 GB, copied read-only).
  - 481 of them were played after weapon balance 1 (revision 21, live from
    2026-10-18 14:43 UTC). Those 481 are "rev21" below.
  - No match with server bots is counted.
- **Per match:**
  - The kit of each place, from the 0xE4 static info.
  - Every input of each client (50 ms ticks).
  - The uploaded worlds (about every 5 s): positions, statistics per kit, and
    the damage each place received from each inflictor.
  - 13 recordings (2 %) have no readable static info, so their kits are
    missing.
- **Players:**
  - Kills and deaths per place come from the last world's statistics.
  - The ranking is by kills a minute, then K/D, counting accounts with at
    least 8 matches and 90 minutes (222 accounts, 148 counted).
  - Everyone together scores 0.78 kills a minute.
  - Each life's band is its kit's equipment level as the lobby computes it:
    the highest `match_making_level` of the items in the kit
    (`session::v026g::matchmaking::kit_level`).
  - The top of a band is the 5 accounts with the most kills per minute alive
    in that band, out of those with at least 30 minutes alive in it.
  - "Top" in the firing tables is the overall top 10 together with the top 5
    of each band.
- **Trigger runs:**
  - A run is FIRE held over consecutive ticks with one weapon in hand.
  - The weapon in hand is weapon 1 at a spawn, then follows the weapon keys
    and the wheel. The weapon-balance study checked this model at 99.7 %.
  - **Distance** of a run, ESTIMATED:
    - Taken when exactly one enemy's received damage from the shooter rose in
      the world interval that holds the run's middle.
    - Positions are interpolated linearly between worlds, and the run is kept
      only when the nearest world is within 1.5 s.
    - So these are the runs that hit, and the distance is good to a few
      metres.

## 2. Top players and their weapons by band

Overall top 10, all 631 matches:

| Rank | Player | Kills / min | K/D |
|---|---|---|---|
| 1 | Y | 1.87 | 2.11 |
| 2 | AG | 1.87 | 3.09 |
| 3 | B | 1.80 | 1.91 |
| 4 | AH | 1.74 | 1.77 |
| 5 | AF | 1.61 | 1.71 |
| 6 | Z | 1.54 | 2.07 |
| 7 | AR | 1.52 | 2.65 |
| 8 | AI | 1.42 | 1.62 |
| 9 | AC | 1.41 | 1.25 |
| 10 | AE | 1.39 | 1.97 |

The rev21 subset gives the same accounts in a slightly different order.

Weapon 1 of the kits the top of each band used, as a share of minutes alive,
and gun kills by weapon:

| Band (sample) | Top of the band | Weapon 1 (share of time alive) | Gun kills of the top of the band | Everyone: weapon 1 |
|---|---|---|---|---|
| 4-5 (27 accounts, 815 min alive) | K, AK, AP, AB, O | Vityaz 47 %, Uzi 24 %, MP-153 17 %, TOZ-66 11 % | Vityaz 145, Uzi 90, MP-153 38 | Vityaz 25 %, Uzi 17 %, TOZ-122 13 %, Kiparis 11 %, MP-153 8 %, Remington 700 6 % |
| 9-10 (111 accounts, 12,187 min) | AG, AH, AF, AL, Z | AK-74N 47 %, PP-2000 30 %, Vityaz 12 %, SV-98 11 % | AK-74N 333, PP-2000 322, SV-98 103, Vityaz 98, Remington 700 74 | SV-98 17 %, AK-74N 11 %, PP-2000 9 %, Vityaz 8 %, AKMN 7 % |
| 9-10, overall top 10 | | AKMN 40 %, AK-74N 27 %, PP-2000 14 %, SV-98 8 % | | |
| 13-15 (74 accounts, 14,600 min) | Y, AG, B, AL, AA | AK-12 (premium) 45 %, Kriss Vector 12 %, HK416 10 %, VSK-94 9 %, FN P90 7 %, VAL 6 % | AK-12p 1,732, Kriss 463, VSK-94 418, HK416 354, P90 306, VAL 195 | AK-12p 30 %, Kriss 14 %, VSK-94p 10 %, VSK-94 5 % |

Notes on the table:

- **Band 4-5:** few big matches are played at this level, so the sample is
  small. The everyone column holds 452 gun kills.
- **Band 9-10, rev21:** the top players' kits shift to the AK-74N, VZ58,
  Vityaz and SR-2M, at 21-28 % each.
- **Weapon 2:** the top players carry a sniper or marksman rifle there: the
  Remington 700 or SV-98 in band 9-10, the VSK-94, SVD or Remington in band
  13-15. They rarely use it: weapon 2 is in hand 4 % of the time and fires
  2.6 % of the shots.
- **Not usable for bots:** the AK-74N and every `*_premium` weapon are
  premium items, and the bots carry no premium item (`loadouts.rs`).

### Recommended bot kits (weapon 1 + ammo; item `match_making_level` in brackets)

| Tier | Flanker | Support | Rusher | Camper |
|---|---|---|---|---|
| 4-5 | `vityaz` (4) + `ammo_9x19p_para` (2) | `ak_74u` (5) + `ammo_5.45x39_7n6m` (5) (kept) | `mp_153` (4) + `ammo_12mm_buck` (1) | `rem_700` (5) + `ammo_7.62x51_ap` (5); `mosin_nagant` for a free kit |
| 9-10 | `pp2000` (9) + `ammo_9x19p_7n31` (9) | `akmn` (6) + `ammo_7.62x39_7n23` (9), or `vz58` (9) + the same | `vityaz` (4) + `ammo_9x19p_7n31` (9); `spas_12` (7) + `ammo_12mm_buck` keeps a shotgun | `sv_98` (9) + `ammo_7.62x54_7n1` (9) |
| 13-15 | `kriss_vector` (14) + `ammo_45acp` (8) | `ak_12` (15) + `ammo_6.5x39` (15) | `fn_p90` (14) + `ammo_5.7x28` (9) | `vsk_94` (14) + `ammo_sp5` (12); a bolt action: `vssk` (15) + `ammo_12.7x54_vps` (15) |

- **Ammo:** each kit carries the ammo the players carry most with that weapon,
  counted over every kit in the recordings.
- **Shotguns:** no top player uses one at 9-10 or 13-15. A shotgun for the
  rusher at those tiers is a role choice, not what the data shows.
- **Weapons outside the test data:** the 0.29a3 weapons are in the players'
  game data, not in the test data (`resources/game-026g`). Tests of the tiers'
  kits need `SRP_TEST_NEW_WEAPONS_DATA`
  (`weapons::tests::every_weapon_of_the_players_data_reads`).

## 3. How the best players fire automatic weapons

All numbers are for runs that hurt one enemy, distance band by distance band.

**Run held, ms (q1 / median / q3), top players, all 631 matches:**

| Distance | SMG | Rifle | Machine gun |
|---|---|---|---|
| 0-10 m | 400 / 700 / 1050 | 350 / 700 / 1150 | 450 / 650 / 850 (7) |
| 10-20 m | 460 / 750 / 1200 | 350 / 700 / 1200 | 690 / 1150 / 1760 (16) |
| 20-35 m | 400 / 850 / 1400 | 300 / 700 / 1200 | 300 / 900 / 1460 |
| 35-60 m | 400 / 1000 / 1650 | 100 / 500 / 1100 | 100 / 175 / 1010 |
| 60+ m | 175 / 500 / 1275 | 100 / 150 / 650 | 100 / 150 / 150 |

**Taps (runs under 250 ms) among runs that hit, top players:**

| Distance | Rifle | SMG, all matches | SMG, rev21 |
|---|---|---|---|
| 0-10 m | 15 % | 12 % | 13 % |
| 10-20 m | 17 % | 11 % | 9 % |
| 20-35 m | 19 % | 12 % | 21 % |
| 35-60 m | 36 % | 16 % | 36 % |
| 60+ m | 61 % | 32 % | 65 % |

- **SMGs after weapon balance 1:** after revision 21, which cut the SR-2M's
  and the PP-2000's rate of fire, the SMG holds also fall past 35 m.
- **Everyone else** taps more at range: rifles 52 % at 35-60 m and 69 % past
  60 m.
- **Pauses between runs** (gaps under 2 s): 150 / 350 / 800 ms (SMG) and
  150 / 300 / 600 ms (rifle).
- **Per weapon:**
  - Low-kick SMGs are held longest at 35-60 m: Vityaz 1250 ms, PP-2000 1150,
    MSBS 1025, SR-2M 975.
  - The AKMN (150 ms) and the HK416 (200 ms) are tapped there.
  - The kick's rate alone (top recoil × rate of fire) does not order the
    weapons well. The category, SMG or rifle, does.
- **While firing:**
  - Down the sights on 78-86 % of the ticks within 10 m and 96-100 % beyond.
  - Crouched 16 % (machine guns 36 %).
  - A side key held 75 % (SMG) and 81 % (rifle) of the time.
  - The side key changes 1.9-2.2 times a second around a run.
  - Others: side key 64 %, changes 1.5 a second.
- **Reloads:** 41 % of the top players' SMG runs are followed by a reload
  within 3 s, median 300 ms after the run ends. They reload as soon as the
  burst is over, at about half a magazine (`TOP-PLAYERS.md`).

## 4. One round a press: marksman rifles, pistols, shotguns, bolt actions

**Press to press minus the weapon's cycle** (`60 / rpm` plus
`chamber_a_round_time` from the config), top players, ms (q1 / median / q3):

| Kind | Within 20 m | 20-35 m | 35-60 m | 60+ m |
|---|---|---|---|---|
| Marksman rifles (SVD, SKS, VSK-94's one-round copies) | -20 / 30 / 160 | 30 / 110 / 210 | 110 / 210 / 680 | 280 / 510 / 1140 |
| Bolt actions | 480 / 1100 / 1400 (few) | 620 / 1070 / 1430 | 760 / 1190 / 1780 | 920 / 1320 / 2030 |
| Semi-automatic shotguns | 0 / 0-50 / 100-400 | 0 / 100 / 310 | 50 / 150 / 525 | |
| Pistols | -50 / 0 / 50 | -50 / 0 / 140 | | |

Two barrels: press to press 300 / 350 / 540 ms, against a 250 ms cycle.

- **Holds:** a press is held 100-200 ms whatever the weapon, at every
  distance. Nobody holds a bolt action.
- **Bolt actions per weapon**, press to press for the top players:
  - SV-98: 3.4 s (cycle 2.77).
  - Remington 700: 2.95 s (cycle 1.83).
  - Mosin: 2.95 s (cycle 2.18).
  - TOZ-122: 2.4 s (cycle 1.5).
- **The bolt's time, LIKELY:** no player presses sooner than 0.4 s past the
  config's cycle.
  - The card's rate of fire is `60 / (60 / rpm + chamber_a_round_time)`,
    floored, for all eleven bolt and pump weapons.
  - This is consistent with the bolt taking `chamber_a_round_time`, or the
    Mosin's 65-frame clip (0.37 s longer).
- **Engagement distances** (runs that hit, top players): SMG 17 / 26 / 38 m,
  rifle 21 / 33 / 45, marksman 32 / 46 / 68, bolt 45 / 65 / 85 (SV-98 52 /
  80 / 96), shotguns 12 / 19 / 27, pistols 11 / 15 / 22.
- **Stance with a bolt action:**
  - Down the sights 95-100 % (others 78-91 %).
  - Crouched 20 % (others 28 %).
  - A side key held 51 % (others 30 %): the best snipers move between shots.
- **Marksman rifles:** crouched 32 %, a side key held 51 %.

## 5. What the bots did, and what they do now

**Before:** five weapons in a table in code (`guns::GUNS`), one burst length
for every automatic weapon (0.5-1 s at any distance), and a bolt action
pressed at its cycle plus 50 ms. With a bolt action the bot never
side-stepped, and only a bolt action crouched.

**Now:**

- **Every weapon from the game data.**
  - `survarium::v026g::weapons` reads every weapon's config:
    - rate of fire and fire modes;
    - magazine and the states it has (reload kind, bolt);
    - `reload_time`, `reload_start_time`, `reload_end_time` and
      `chamber_a_round_time`;
    - show and hide times, the kick block, and the damage distances.
  - `guns::guns()` and `recoil::kicks()` are built from it, so a tier's
    weapon (SV-98, Kriss, VSK-94, ...) is fired by its own numbers. No table
    of weapons is kept in code.
  - Two clip ratios that the server's data does not hold stay as named
    constants: the TT-33's empty reload and the TOZ-34's two barrels.
  - The Mosin's times now come from the config: bolt 1.8 s, round-by-round
    reload 1.5 + 0.5 a round + 1.5 s.
- **Handling by kind** (`guns::Handling`, `reflex::FireTuning`, under
  `aim.combat.fire` in the tuning file):
  - **Burst:** an automatic weapon, with the SMG or rifle rows by its category.
    - Each burst is a tap (100-200 ms) with the row's chance, else a hold
      drawn in the row's range.
    - SMG: taps 11 / 20 / 36 / 65 % up to 20, 35 and 60 m and beyond; holds
      400-1150, 450-1350, 450-1500 and 300-900 ms.
    - Rifle and machine gun: taps 16 / 19 / 36 / 61 %; holds 400-1200,
      400-1200, 300-1100 and 250-650 ms.
    - Pause 150-700 ms.
  - **Semi:** pressed again after the cycle plus a wait by distance: 0-150 ms
    within 20 m, 30-210 to 35 m, 110-500 to 60 m, 280-1000 beyond.
  - **Bolt:**
    - One press of at most 100 ms, never held.
    - The next press after the cycle, `press_gap_ms` and a re-aim of
      600-1700 ms.
    - Always down the sights.
    - Side-steps between shots for half its targets, still from 300 ms before
      the press until the shot.
    - Fires out to 120 m; other weapons to 80 m.
  - **Barrels:** the second press 50-300 ms after the cycle.
- **Crouch to fire**, drawn per target: automatic 0.16, machine gun 0.35,
  marksman 0.3, bolt 0.25 (`sniper_crouch_chance`), others 0.15.
  - A marksman rifle side-steps for half its targets; the other weapons always
    side-step.
- **Fight distances** (`band_*` in `reflex::CombatTuning`), from the top
  players' engagement distances (section 4):
  - SMG 5-25 → 10-35 m, rifle 10-45 → 15-50, bolt action 30-80 → 40-95,
    shotgun 0-12 → 5-25; pistol 0-20 unchanged.
  - A bot keeps 15 m off an enemy shotgun unless it carries one itself, and
    closes on an enemy sniper to 30 m.
- **Reloads** (section 3): a partly spent magazine is reloaded once no enemy
  has been seen for 400 ms (was 1 s) and at least 40 % of it is fired (was
  30 %), nearer the top players' half a magazine.
- **Moving while firing** (`strafe_walking`, on): the side step also runs
  while the bot walks to or from its target, not only while it stands; a
  walk along its view weaves to the side. A run stays straight. The top
  players move on 84.5 % of the ticks while finishing a kill, the others on
  66.6 %.
- **No shot through a teammate** (`friends.rs`, the owner 2026-10-23: bots
  killed teammates, bots and players, standing between them and the enemy).
  The game has friendly fire, and the aim's wall test looked at the world
  only. In the saved worlds of the 257 player-against-bots matches of
  2026-10-20..23 the bots made 27,088 kills and 7,250 team kills (21 % of
  their kills; event 4, counted to the killer: the killer's own death rarely
  falls in the same world, 291 of 7,356, a teammate's nearly always does).
  Of the 4,234 team kills of a bot with one teammate's death in that world,
  3,671 (87 %) had the victim within 1.5 m of the line from the killer to an
  enemy beyond him a second before (median 0.24 m off the line, 5.8 m from
  the killer), against 26 % of the killer's other teammates within 12 m.
  The player died ~160 times (9 % of his deaths) in a world where a bot of
  his team made a team kill. `received_damage` has no entries for a
  teammate's damage (none in any of these worlds), so team damage short of
  a kill is not counted there. Every weapon kind did it; team kills of all
  its kills by the kit's weapon (the kit in use a world before): SMG 18 %,
  rifle 17 %, shotgun 31 %, the camper's sniper rifle 34 % (GUESS: it
  fires from behind its team). Now a bot does not fire while a living teammate's body (the
  shins to the head, 0.35 m wide, 0.6 m for a player, whose body is
  reckoned) comes within the margin of the view's line or the line it turns
  to, short of the target and 2 m past: the margin grows by the weapon's
  spread (`aim.friends.spread_deg`: 3° automatic, 1.5° semi-automatic,
  0.75° bolt, 4° shotgun); it steps to the side away from him instead. The
  test harness now lets a shot hit a teammate (it skipped them): 6 Team
  Deathmatch matches of 6 a side, off then on, made 547 kills and 181 team
  kills (985 team hits), then 696 kills and 10 team kills (85 team hits).

## 6. The aim at skill 1, the tiers' skill, the pull against the kick

The owner, 2026-10-23: skill 1 is now the top players' best quarter, not
their median.

- **Aim at skill 1** (`tuning::AimTuning`):
  - `error_deg` 1.5 → 0.9.
  - `wander_deg` 1.3 → 0.8.
  - `settle_s` 0.15 → 0.10.
  - `reaction_ms` 180 → 120.
  - The noticing ×0.8 → ×0.6.
  - Skill 0's numbers are unchanged.
- **Target taken between thinks:** an enemy noticed between two thinks is now
  taken as the target at the next tick (`Reflex::tick`).
  - Noticing is the track's own time, and a think came only every 200-400 ms.
  - Without this, the chain could not get under one think interval.
  - Still only a seen, noticed enemy in range.
- **Duel chain** (`reflex::tests::duel_numbers_by_skill`: a target strafing
  at 2.5 m/s, 20 m away, 60 seeds):

  | Skill | Sighting → first shot, ms (q1 / median / q3, least) | Yaw off the chest while firing (median) |
  |---|---|---|
  | 0 | 850 / 950 / 1050, 750 | 1.79° |
  | 0.6 | 550 / 650 / 700, 450 | 1.25° |
  | 0.75 | 500 / 550 / 600, 350 | 0.96° |
  | 0.85 | 450 / 500 / 550, 300 | 0.80° |
  | 1 | 350 / 400 / 500, 250 | 0.57° |

  - Before this change: 550 ms and 0.93° at skill 1, 950 ms and 2.03° at
    skill 0.
  - The top players: 400 / 650 / 1050 ms, about 1.0° of their own hands'
    error.
  - The test holds the fairness: never a shot within 250 ms of a sighting,
    never an error of 0.
- **Tiers** (`SkillTuning::tiers`; JSON `"skill": {"tiers": {"9-10":
  {"start": .., "min": ..}}}`):
  - Starts: 4-5 0.6, 9-10 0.75, 13-15 0.85.
  - Floors: 0.3, 0.5, 0.65.
  - Most: 1 for every tier.
  - The tier reaches the bots through `Bots::with_tier(BotTier)`; the default
    is 4-5.
  - `start` and `min` are used for a tier with no row. With the default rows,
    a file's `skill.start` therefore no longer changes the start.
- **Pull against the kick:** `recoil_pull` (skill 1) is 1.0 and
  `recoil_pull_low` (skill 0) is 0.5.
  - The top players took back about all of the kick: their base pitch sat 1.4
    / 3.5 / 5.8 / 5.6° under the chest over a burst, while their hits rode the
    kick (`TOP-PLAYERS.md`).
  - What is left over the chest in a burst at 20 m, 250 ms on (ESTIMATED with
    the bot's own model of the client's kick, `kick_left_by_skill`):

    | Skill | AK-74U | AKMN | RPD | Uzi | PP-2000 |
    |---|---|---|---|---|---|
    | 0 | 1.1-1.2° | 1.6-1.7° | 1.5-1.6° | 0.7-0.8° | 0.5-0.7° |
    | 0.6 | ~0.3° | 0.4° | 0.4° | 0.2° | 0.2° |
    | 1 | 0 | 0 | 0 | 0 | 0 |

## 7. Tests

**New tests:**

- `weapons::tests::the_weapons_are_read_from_their_configs`.
- `guns::tests::times_from_the_configs`: every weapon's handling, the times.
- `reflex::tests::a_bolt_action_is_pressed_once_a_cycle_and_never_held`.
- `reflex::tests::each_weapon_is_fired_by_its_kind`.
- `reflex::tests::a_burst_falls_with_the_distance`.
- `reflex::tests::the_pull_against_the_kick_grows_with_the_skill`.
- `signals::tests::each_tier_starts_at_its_skill_and_never_falls_below_its_floor`.
- `the_duel_numbers_are_a_top_players_at_the_best_skill`, now with the 250 ms
  and non-zero error checks.

**Changed tests:**

- `an_automatic_weapon_fires_in_the_top_players_bursts` follows the rows.
- `lost_battery_run` keeps the fixed bursts its scenario needs.

## 8. Not verified

- **No real match:** no real match has been played with these bots.
- **Bolt time:**
  - Whether the 0.26g client works the bolt in `chamber_a_round_time` or in
    the clip's length is not traced. The re-aim covers both.
  - The empty-reload and two-barrel ratios of the other weapons are the TT-33's
    and the TOZ-34's.
- **Distances:** ESTIMATED from 5 s worlds, and only for runs that hurt one
  enemy.
- **Band 4-5:** the sample is small (7 accounts with 30 minutes).
- **0.29a3 weapons:** these weapons run only with the players' game data. The
  repository's test data has none of them.
- **Bolt still time:** `bolt_still_ms` (300 ms) is a choice, not measured.

## 9. The 0.26g bullet dispersion (2026-10-23)

Read in the 0.26g executable (image base 0x10000; the functions are in
`tools/re-kit/v026g/`) and checked against the live configs. The numbers are
read by `weapons::Dispersion` and `weapons::CharacterDispersion`.

**The cone (PROVEN, `get_dispersed_bullet_direction` 0x2E1D50):**

```
D = base_dispersion × ammo.dispersion  +  (C + W) × (aim progress == 1 ? 0 : 1)
θ = frac(N(0, 0.5)) × D / 2   about a random axis across the barrel
```

- **D is the full apex angle in degrees.** |θ| has median 0.31·D/2, 80th
  percentile 0.58·D/2, and never passes D/2.
- **C, the character part:** ramps linearly to a target `(aimed ? aim_multiplier
  : from_the_hip_multiplier) × state value`. It rises at `growth_speed` and
  falls at `speed_of_aiming` (degrees a second). The state values are
  `player.character_dispersion_params` in `default.player` (bots use the same
  file, LIKELY):

  | state | idle | walk | run | jump | crouch | crouch walk |
  |---|---|---|---|---|---|---|
  | value (hip and aimed alike) | 5 | 10 | 20 | 20 | 4 | 8 |

  The state comes from the keys, not the velocity: moving is
  (forward xor back) or (left xor right).
- **W, the bloom:** each shot adds `one_shoot_dispersion_amount` (capped at
  `max_dispersion`); it falls at `speed_of_aiming`.
- **The aim:** the aim progress reaches 1 exactly `aim_time` after AIM is
  pressed (`parameters/aim_time`, 0.2 to 0.35 s). From then on the whole
  dynamic part counts 0: only `base_dispersion` is left (0.05 to 1°),
  **strafing or not**. Releasing AIM brings it all back at once.
- **Modifiers:** skills and weapon modifications scale these numbers by
  `max(0, 1 + m)`. All are 0 for a player without skills.

**Uzi and AK-74U (base 0.3, aim_time 0.25 s), the full cone D:**

| case | D |
|---|---|
| aimed for 0.25 s, still or strafing | 0.3° |
| from the hip, standing | 5.3° |
| from the hip, strafing (or in the first 0.25 s of the aim) | 10.3° |
| from the hip, crouched | 4.3° |
| sprinting or jumping | 20.3° |

The snipers (base 0.05, hip ×4) are 20° wide from the hip even standing; the
Mosin's hip is ×2.

**Chance to hit a 0.5 × 0.7 m torso with exact aim** (Monte Carlo of the rule
above):

| D | 10 m | 20 m | 30 m | 50 m |
|---|---|---|---|---|
| 0.3° (aimed in) | 100 % | 100 % | 100 % | 100 % |
| 5.3° (hip, standing) | 88 % | 56 % | 40 % | 25 % |
| 10.3° (hip, strafing) | 57 % | 31 % | 21 % | 13 % |
| 20.3° (sprint, jump) | 32 % | 16 % | 11 % | 7 % |

**What this means for the bots:** a strafing bot hits as well as a still one,
provided it holds AIM for the weapon's `aim_time` before the first shot and
keeps holding it. Before this, 35-40 % of the bots' shots left before their
sights were up, and those hit 9-11 % against 18-24 % aimed in (the bench with
the cone on, `SRP_BENCH_SPREAD=1`). `CombatTuning::aim_in_first` holds the
shot until the sights are up, unless the hip cone of the bot's stance fits the
target at that distance (`hip_cone_share` × the half cone against
`body_half_m`). Results: `player_bots_behaviour.md` §2.5.

**Not traced:** how the client poses the weapon bone that the shot starts from;
recoil (look pitch + weapon+0x478) climbs bursts on top of the cone, LIKELY for
bots too. The bench models the cone but not the recoil.

## Bots check teammates against a grenade's blast at the press and the hold

**The fault (Cologne CG-7):** a bot looked for teammates within
`grenade_friend_m` (6 m) of the landing point only when it chose to throw.
The RGD-5 then flies and lies for its fuse, 4 s, and a teammate who walked in
during that time was caught. The team's push was also timed from the press,
not from the blast.

**The grenade, from its config** (`gameplay/items/weapons/rgd-5.options`,
`data` block, read by `weapons::grenade_named`): `ttl_ms` 4000,
`splash_radius` 10 m, `splash_max_damage` 1.3, `splash_min_damage` 0.05,
`explode_on_contact` false. With an even falloff the blast does under half a
body's health from 6.4 m out, which is why `grenade_friend_m` stays 6 m (GUESS).

**What the bots do now** (`CombatTuning::grenade_recheck`,
`grenade_lead_s`, `grenade_push_after_blast`):

- At the choice, at every tick of the press and at the let-go, each living
  teammate counts as the line from his feet to where his velocity takes him
  in `grenade_lead_s` (1.5 s, GUESS: the time an advancing teammate needs to
  close half the blast's radius). If that line passes within
  `grenade_friend_m` of the landing point, the bot lets go of the button
  before the throw starts. In 0.26g releasing bit 23 before the weapon reaches
  state 6 cancels the throw (`player_bots.md`), so the grenade stays in hand.
  The bot notes `holds a grenade back at ...: teammate N`.
- At the let-go, the blast falls at the press time plus the config's `ttl_ms`,
  and the team's push starts then, not 1.5 s after the press.
- The fight tally counts, at each blast, the living teammates within 3, 6 and
  10 m, and the thrower within 6 and 10 m. The bench has no grenade physics
  (the clients simulate the grenades), so these are the places at the blast,
  not damage.

**Bench** (paired, `real_bot_cost`, 180 s, the 6 battery maps, 4v4 and 10v10,
`SRP_BENCH_SPREAD=1`, seeds 1-16, one binary. "Off" is
`grenade_recheck=false,grenade_push_after_blast=false`. Means per seed over 12
matches, ± 1 SE of the paired difference, * beyond 2 SE):

| | tiers 4-5 off → on | tiers 9-10 off → on | tiers 13-15 off → on |
|---|---|---|---|
| grenades thrown | 14.12 → 13.06 (−1.06 ± 0.86) | 8.94 → 9.06 (+0.12 ± 0.64) | 11.12 → 9.25 (−1.88 ± 1.11) |
| held back | 0 → 0.62* | 0 → 0.62* | 0 → 0.38* |
| teammates within 3 m of a blast | 0.31 → 0.19 | 0.12 → 0.00 | 0.06 → 0.06 |
| teammates within 6 m | 0.81 → 0.38 (−0.44 ± 0.30) | 0.62 → 0.12* | 0.69 → 0.12* |
| teammates within 10 m | 2.12 → 1.69 | 1.88 → 0.88 (−1.00 ± 0.54) | 1.94 → 1.06* |
| the thrower within 6 m | 0.88 → 0.44* | 0.44 → 0.38 | 0.75 → 0.19* |
| the thrower within 10 m | 2.38 → 1.12* | 1.19 → 1.06 | 2.00 → 0.81* |
| team hits (all weapons) | 1.81 → 1.88 | 1.69 → 1.62 | 1.31 → 1.31 |
| team kills (all weapons) | 0.19 → 0.19 | 0.12 → 0.19 (+0.06 ± 0.06) | 0.12 → 0.06 |
| kills | 246.5 → 249.1 (+2.6 ± 2.2) | 269.9 → 272.0 (+2.1 ± 2.0) | 281.5 → 284.7 (+3.2 ± 2.6) |
| K/D team 0 / team 1 | 0.98 / 1.05 → 1.01 / 1.02* | 0.97 / 1.08 → 0.96 / 1.08 | 0.99 / 1.05 → 0.99 / 1.05 |
| delivered | 74.94 → 75.69 (+0.75 ± 0.50) | 75.06 → 75.25 | 72.44 → 73.19 (+0.75 ± 0.60) |
| carriers killed | 17.81 → 17.81 | 19.19 → 19.12 | 20.94 → 21.12 |
| quick ends | 416.1 → 409.6 | 412.2 → 408.4 | 416.9 → 421.6 |
| flips | 152.5 → 150.6 | 153.6 → 152.2 | 149.7 → 154.8 (+5.1 ± 2.8) |
| stood % | 6.07 → 5.96 | 5.78 → 5.80 | 6.04 → 6.02 |

Teammates at the blast within 6 m fall by about three quarters at every tier.
The thrower is near his own blast half as often, because the push now waits
for the blast. No guard moves beyond 2 SE against the change. The K/D shift at
tiers 4-5 is between the two sides, and both run the same bots, so it is a side
effect of the seeds, not a gain. Team kills from grenades can't be told apart
here: the bench has no blast damage, and the team-kill counts above are from
bullets.

Test: `reflex::tests::a_grenade_is_held_back_from_a_teammate_walking_into_its_blast`,
`weapons::tests::the_bots_grenade_is_read_from_its_config`.

## Bots: the fire gates of each tick, tallied on the bench and traced

**What it measures (Cologne CG-2: bots with an enemy in their sights and no
shot).** Each tick the reflex records every gate that held its shot
(`reflex::gate`), not only the first:

- **The bot's own gates first:** getting unstuck (the reflex did not run),
  the play's word, a jump held (by its source: route, hop, unstick), a use in
  reach.
- **Then the fire step's, in its order:** a throw, a threat's look, no seen
  target, the view off by more than `within_deg`, the reaction, a reload or
  the switch, no round, the range, the hip cone too wide with the sights not
  up (running, or the aim-in under way), a teammate in the line,
  `clear_shot` (no part shows, or the view's or the aim's ray hits the
  world), the pause between bursts, the trigger between presses, a burst
  cut short.

Nothing decides by the record. `clear_shot`'s rays are cast as before, only
when the other gates let the shot go, unless `Reflex::gate_tally` asks for
every gate. Tracing, a view held 300 ms on a seen target (within
`within_deg` + 2°) with no shot, or a burst cut, is noted with every gate's
value: the time to the first shot, the weapon busy and why, rounds, distance
and range, the aim-in against `aim_ms` + margin, `hip_fits`, the pause, the
teammate, which ray hit and how far, the lift, and the looked part's age. At
most one note per target a second.

**The bench's tally** (`fight_tally::GateTally`, on 10054fd4's on-view
episodes) counts the ticks a bot's view was within 4° of a visible enemy
within 40 m, free to fire, and no shot left. It counts them by the first gate
that held the shot and by every gate. Two gates of the bench's own sit in the
order: `other_target` (the reflex aimed at another enemy than the one in
view, right after `no_target`) and `none` (every gate passed and the game
counted no shot). `SRP_BENCH_GATES=1` tests every gate each tick;
`SRP_BENCH_AIM=name=json,...` sets `AimTuning` as `SRP_BENCH_COMBAT` does
`CombatTuning`.

**Bench:** both presets (`batt`, `tdm`), seeds 1-8, 96 matches a tier,
`SRP_BENCH_GATES=1`, b222a338 + this change. Shares of the on-view ticks with
no shot, first gate / any gate:

| gate | batt 9-10 | tdm 9-10 | Cologne (level_08 day_4) 9-10 | all tiers, both presets (first) |
|---|---|---|---|---|
| aiming_in | 25.0 / 37.7 | 24.4 / 37.4 | 25.0 / 38.6 | 22.7-26.3 |
| no_target | 13.7 / 13.7 | 13.8 / 13.8 | 15.0 / 15.0 | 13.3-14.0 |
| reload | 12.7 / 15.0 | 12.5 / 14.9 | 10.6 / 12.8 | 12.4-13.6 |
| running | 8.7 / 14.4 | 8.7 / 14.7 | 7.4 / 12.0 | 7.6-9.3 |
| other_target | 7.3 / 7.6 | 8.2 / 8.5 | 8.3 / 8.7 | 6.7-8.2 |
| trigger | 6.0 / 6.2 | 5.6 / 5.9 | 4.9 / 5.1 | 5.5-6.1 |
| none | 5.5 | 5.3 | 4.9 | 5.3-5.6 |
| view_ray | 4.4 / 16.7 | 4.3 / 17.1 | 4.4 / 18.9 | 3.9-4.8 |
| reaction | 3.8 / 5.8 | 3.9 / 6.1 | 4.5 / 7.6 | 3.4-3.9 |
| friend | 3.1 / 8.7 | 4.0 / 11.4 | 5.1 / 13.8 | 3.0-4.1 |
| hidden | 2.4 / 10.1 | 2.1 / 9.9 | 2.5 / 12.1 | 2.0-2.4 |
| within | 2.0 / 3.6 | 2.1 / 3.7 | 2.4 / 4.7 | 1.5-3.8 |
| pause | 2.0 / 2.2 | 1.9 / 2.0 | 1.4 / 1.6 | 1.8-2.1 |
| aim_ray | 1.6 / 6.6 | 1.4 / 6.4 | 1.4 / 6.5 | 1.3-1.8 |
| threat | 1.2 | 1.0 | 1.7 | 1.0-1.3 |
| rounds, unstick, cut | under 1 | under 1 | under 1 | under 1 |
| range, play, jump, using, throw | 0 | 0 | 0 | 0 |

**What it says for the next fixes:**

- **The aim-in is the first gate:** a quarter of the held ticks, and with
  running another 8-9 %. The hip cone does not fit and the sights are not
  up yet. This is the aim-in restart the plan put first.
- **No seen target (13-15 %) and another target (7-8 %)** come next: the
  view is on an enemy the reflex has not noticed, or it aims at another. That
  is the noticing and target choice, not a fire gate.
- **Reloads** hold 12-14 %.
- **The friend gate** is first only 3-5 % of the time, though a teammate is
  in the line in 8-14 % of the held ticks: it is last in line, as planned.
- **The walls:** the view's ray is blocked in 16-19 % of the held ticks and
  the aim's in 5-8 %, but the rays are the first gate only 5-7 % of the
  time: something else holds the shot first.
- **The jump hold, the play's word and the range never hold a shot here:**
  the on-view measure counts only bots free to fire within 40 m. The 80-106 m
  ticks of the Cologne report are outside it.

**No behaviour change:** the decision digests (inputs and notes) of the gate
runs match b222a338's base runs (`coord/base-b222a338`) line for line,
`SRP_BENCH_GATES=1` and all. With no tier set, the bench plays as tier 4-5:
the two give the same counts.

Test: `reflex::tests::the_fire_gates_name_what_held_the_shot_and_change_none`.

## Bots: the aim held through a target lost a moment, and the restarts named

**The restarts, measured first.** PR A's `aiming_in` (a quarter of the
on-view ticks with no shot) was mostly a restart: the reflex lets go of the
aim when its target drops out of `Memory::seen` and starts the aim-in from
nothing when he is back. Three gates of the record now name the restarts
(`reflex::gate::LOST`, `REACTION_AGAIN`, `AIMING_AGAIN`; "a moment" is
`LOST_MS`, 1 s):

- **`aiming_again`**, the aim-in under way at the target the bot had already
  aimed in on: 16-17 % first, against 7-10 % for a fresh aim-in.
- **`lost`**, no seen target, the one lost under a second ago: 11 %. Most of
  it is `know`'s noticing wait (400 ms) started again on a target out of
  sight for a moment. A target never seen is `no_target`, 2.5-3.5 %.
- **`reaction_again`**, the reaction waited again at a target lost a moment:
  1.5 %.

The record decides nothing: with the flags off, the decision digests match
the base's.

**`CombatTuning::aim_carry` (on).** The aim stays pressed, and its time runs
on, through a threat's look and while the target is lost for at most
`aim_carry_ms` (GUESS: `LOST_MS`). Seen again, the sights are up at once.
`aiming_in` + `aiming_again` as the first gate: 23-27 % → 2-4 %.

**`CombatTuning::reacquire_keep` (off).** A target taken again within
`aim_carry_ms` keeps what the bot had of him (the error settled so far, the
reaction already waited, the draws for him), where he was a fresh one (the
error three times the skill's, a new reaction). It reads
`Memory::in_sight`, a narrow exception to the noticing wait: only for the
reflex's own last target, lost under `aim_carry_ms` ago, still in live sight
(seen, `sight_since` set, seen within 400 ms). Off by default, it never runs:
the default digests match aim_carry's alone.

**Bench, one-sided** (team 1 alone with the change, `SRP_BENCH_ONE_SIDE`,
seeds 1-16, both presets, against fc01ace3). Each flag alone, change ±
standard error, `*` beyond 2 SE:

| cell | aim_carry K/D (team 1) | kills / deaths (team 1) | hit % | reacquire_keep K/D |
|---|---|---|---|---|
| batt 4-5 | 1.05 → 1.23, +0.18 ± 0.04* | +11.1 / −7.4 | −2.3* | +0.02 ± 0.05 |
| batt 9-10 | 1.06 → 1.25, +0.19 ± 0.04* | +12.3 / −7.6 | −1.8* | +0.03 ± 0.02 |
| batt 13-15 | 1.05 → 1.33, +0.27 ± 0.03* | +22.9 / −9.1 | −2.7* | +0.02 ± 0.04 |
| tdm 4-5 | 1.00 → 1.18, +0.18 ± 0.02* | +12.9 / −8.4 | −2.5* | +0.03 ± 0.03 |
| tdm 9-10 | 1.00 → 1.24, +0.23 ± 0.04* | +15.8 / −11.6 | −2.6* | +0.03 ± 0.03 |
| tdm 13-15 | 1.00 → 1.29, +0.29 ± 0.05* | +23.3 / −11.3 | −2.2* | +0.04 ± 0.03 |

- **aim_carry's guards:**
  - The time moving while aimed went up 0.4-1.0 points, so the bots still
    strafe.
  - Team kills, team hits, quick ends, flips and carriers lost are flat
    within 2 SE.
  - Its shots went up 10-15 % and its hit rate fell 2-3 points: more shots
    at the sights' first moments. What part of the fall is the fresh
    target's tripled error on a target taken again is not shown.
- **Delivered in batt 13-15** fell 2.2 ± 1.1, on team 0, the side without
  the change:
  - Team 0's deliveries: −3.5 ± 0.7. Team 1's: +1.3 ± 0.9.
  - Team 1's carriers lost (+0.3 ± 1.3) and time per carry (+0.08 ± 0.07 s)
    are flat.
  - Team 1 wins the carry fights; its carriers do not stop to fight.
- **Stood with a goal and no route, in batt 13-15:**
  - +0.23 ± 0.11 points for both teams together.
  - By team: +0.19 ± 0.16 (team 0) and +0.27 ± 0.22 (team 1), neither
    beyond 2 SE.
  - It counts bots with a goal and no route, not standing to aim. The aim
    carry changes it only through where the fights leave bots waiting.
- **reacquire_keep's guards:** carriers lost by team 1 moved +2.0 ± 0.7 in
  batt 4-5 and −1.8 ± 0.7 in batt 9-10, which reads as noise. The rest are
  flat. No measurable effect, so it stays off.

Tests: `reflex::tests::the_aim_is_held_through_a_target_lost_a_moment`,
`a_target_taken_again_a_moment_after_keeps_what_it_had_of_him`,
`another_enemy_in_sight_still_waits_his_noticing`;
`know::tests::in_sight_is_a_live_sight_before_its_noticing`.

## Bots: the fire range by weapon, and the line looked at again while firing

**The trigger's range, today:** 80 m for every weapon (`AimTuning::range_m`) and
120 m for a bolt action (`FireTuning::bolt_range_m`).

**What the configs say: under the players' use in every class.**
- **Where the damage stops falling (`min_damage_distance`):**
  - 30 m for the shotguns;
  - 40 m for the SMGs, the pistols (the Magnum 50), the AK-74U and the 9A-91;
  - 50 m for the other rifles, the machine guns, the VSS and the VSK-94;
  - 100 m for the bolt actions, the SVD, the SKS and the SOCOM 16.
- **That is not where a weapon stops:** each hit past it still does
  `min_damage_amount` of its damage.
- **The aimed-in cone** (`base_dispersion`, 0.05-2°) keeps a fifth of its shots
  on a torso past 200 m.
- So neither term is used: the range comes from the players alone.

**The players' range (owner's rule: the 2 best of each match with 12 or more
people).**
- **Matches:** 402 of the 631 players' recordings (302 of them rev21). In each,
  the 2 best by kills, then K/D.
- **Runs:** every trigger run of theirs. Its distance is to the living enemy
  nearest their aim, within 5°, taken over up to 10 ticks of the run (positions
  between worlds, both within 1.5 s).
- **Sensitivity:** at a 3° cone or an 8° cone, and also counting runs that pass
  within 2 m of an enemy (the "2 m" column).

p95, m. Hit runs in brackets: runs that hit one enemy, at his distance.

| class | runs (matches) | p50 / p90 / p95 / p99 | 3° / 8° / 2 m p95 | hit runs, p95 (n) |
|---|---|---|---|---|
| rifles (cat 8) | 4520 (208) | 38 / 72 / 90 / 124 | 93 / 89 / 85 | 65 (1942) |
| SMGs | 1482 (122) | 32 / 70 / 90 / 131 | 92 / 90 / 80 | 58 (652) |
| shotguns | 377 (19) | 26 / 64 / 91 / 126 | 89 / 89 / 89 | 49 (143) |
| automatic marksman (VSS, VSK-94) | 773 (43) | 44 / 75 / 84 / 130 | | 74 (188) |
| SVD | 547 (18) | 67 / 105 / 112 / 123 | | 104 (157) |
| bolt actions | 844 (55) | 58 / 94 / 105 / 132 | 105 / 107 / 105 | 105 (140) |
| machine guns | 17 (4) | too few | | |
| pistols | 48 (7) | too few | | |

**What the numbers carry:**
- **Close fights are under-counted.** Positions between worlds (about 2.25 s
  apart) are off by a few metres, a wide angle up close. Only 30 % of the hit
  runs at 0-15 m fall within 5°, against 85 % at 60 m or more. So the table
  reads long.
- **There is no line-of-sight test.** The enemy nearest the aim may be behind a
  wall. Of the runs within 5°, 583 of 8615 are past 90 m, and only 87 of those
  hit. So the upper tail mixes real long shots with fire at enemies out of view.
- **The hit runs are a sure floor, but short:** they count no missed long shot.

**In the code** (`CombatTuning::range_by_weapon`, off):
- The trigger's range is the class's p95 (`range_floor_*_m`, MEASURED), never
  past today's ceiling: 80 m, or 120 m for a bolt action.
- The machine guns and the pistols have too few runs for a number; they keep
  the ceiling (`NO_FLOOR_M`).
- **Only the bolt actions would change: 120 → 105 m.** Every other class's p95
  is past 80 m. That is too small to bench, so the flag stays off.
- **No change with it off:** at its defaults this PR's decision digests match
  2841b08b's, 12 of 12 (batt and tdm, tiers 4-5/9-10/13-15, seeds 1-2).
- **The Cologne report's 80-106 m fire with a Vityaz** cannot come from the
  trigger (80 m). It was fire at another target than the one measured.

**The bench's ranges** (the new `ranges` line of the fight tally: shots by
weapon class and 10 m, hits taken and deaths by team and distance):
- **Shotguns:** at the defaults, 11 % of the bots' shotgun shots are at 60 m or
  more, and 8 % of those hit (32 % under 60 m).
- **The other classes:** rifles, SMGs and bolts put 12-15 % of their shots at
  60 m or more, hitting 7-8 % there.

**Fire at the bot's own target with no line**, at the defaults: 2.0-3.0 % of
those shots leave with nothing of him showing, and 6-8 % meet the world short
of him (with the spread on, some of those are the cone clipping his cover).

**The line looked at again while firing: tried, and dropped.**
- **The idea:** a bot whose target had moved half his width since the last look
  looked at his parts again before the next shot, not only every
  `LOOK_AGAIN_MS` (100 ms).
- **The bench:** team 1 alone with it, against 2841b08b, seeds 1-8, both
  presets, tiers 4-5/9-10/13-15. The tally now counts these shots by the
  shooter's team.
- **The shots with nothing of him showing:**
  - Team 1's share stayed at 2.0-3.0 %, as team 0's did.
  - Team 1 against team 0: +0.18 ± 0.07 points over 48 matches, and
    +0.29 ± 0.27 in 4 control matches without it.
- **The shots that met the world short of him:** +0.03 ± 0.11 points.
- **Its K/D** was flat, at worst −0.06 ± 0.03 (batt 13-15), and fails the
  early stop.
- **So those shots do not come from a stale look:** the parts are already looked
  at every 100 ms, ahead by the target's speed. It is what the bot sees against
  where the stand-in has him at the shot. Not followed here.

**Base:** benched against 2841b08b, the commit this was built on, then merged
with 54ececcf (#396).

## Bots: the reloads measured, and a reload at the lull tried and dropped

**The measure** (the reflex's `ReloadNote` of its last reload, read by the
bench's `ReloadTally`; nothing decides by it). By team and weapon class:
- why each reload started: empty with a target seen (out of sight first, or
  where it stands), empty with none seen, or tactical (`reload_idle_ms`);
- the share of the magazine left;
- an enemy on view (the on-view episodes' 4°) within 1 s before;
- the time from its end to the next shot;
- deaths while reloading;
- and the on-view ticks with no shot that the reload held (the `RELOAD` gate),
  by why the reload started.

**The bots at the defaults** (939b2cca, 48 matches: batt and tdm, tiers
4-5/9-10/13-15, seeds 1-8):

| class | reloads | empty, in view | empty, none seen | tactical | an enemy on view 1 s before | share left (median) |
|---|---|---|---|---|---|---|
| bolt actions | 3,605 | 4 % | 3 % | 93 % | 56 % | 0.60 |
| rifles | 6,578 | 12 % | 8 % | 80 % | 43 % | 0.43 |
| SMGs | 5,892 | 13 % | 10 % | 76 % | 47 % | 0.40 |
| shotguns | 14,218 | 37 % | 25 % | 38 % | 67 % | 0.00 |

- 13 % of the reloads end in the bot's death (4,056 of 30,293).
- The on-view ticks the reload held: 34 % in an empty-in-view reload, 27 % in
  an empty one, 38 % in a tactical one. The last start with nobody seen, and
  then an enemy walks into view.

**The players** (the owner's rule: the 2 best by kills, then K/D, of each of the
402 players' matches with 12 or more people; rounds counted from each world's
magazine, the shots in between from the weapon's rate, about ±1-2 rounds):

| class | reloads | before empty | share left, the ones before empty (median) | an enemy within 30° 1 s before | from the last shot, median |
|---|---|---|---|---|---|
| rifles | 6,144 | 96 % | 0.53 | 69 % | 400 ms |
| SMGs | 2,491 | 91 % | 0.45 | 70 % | 150 ms |
| bolt actions | 366 | 84 % | 0.60 | 65 % | 4,200 ms |
| VSS, VSK-94 | 734 | 89 % | 0.50 | 70 % | 250 ms |
| SVD | 196 (unstable) | 92 % | 0.60 | 71 % | 1,150 ms |
| shotguns | 158 (unstable) | 89 % | 0.50 | 70 % | 550 ms |

The machine guns (12) and the pistols (24) are too few.

**A reload at the lull: tried, and dropped.**
- **The idea** (`reload_lull`): reload as the players do, at the lull after a
  burst. That is the target down or lost, no target seen for 250 ms (GUESS), and
  the rounds left at most the players' median share for the class (above). It
  was never done while the aim was carried through a target lost a moment.
- **The bench:** team 1 alone with it, against the same build with it off,
  seeds 1-8, both presets, tiers 4-5/9-10/13-15. It reloaded about 60 times a
  match at the lull.
- **The target did not move.** Team 1's empty-in-view reloads per kill stayed
  flat, and went the wrong way at batt 4-5: +0.05 ± 0.03. The `RELOAD` share of
  the first gates was flat too, at most +0.4 ± 0.3 points.
- **A guard was worse beyond 3 SE:** team 1's deaths while reloading at batt
  13-15, +5.6 ± 1.5. K/D was −0.02 ± 0.01 at batt 9-10, flat elsewhere.
- **So it fails the early stop.** Most reloads are tactical already, and
  empty-in-view ones are a fifth of them, so a lull reload only adds reloads an
  enemy can walk into. The earlier-lull variant waited on empty-in-view reloads
  dominating; they do not, so it was not tried.
- **What stands out is the shotguns.** 62 % of their reloads come with the
  magazine out, and 37 % of all their reloads have an enemy in view. Among the
  players, a shotgun is out at 11 % of reloads. Not followed here.

**No change:** at the defaults, this PR's decision digests match 939b2cca's,
48 of 48 (batt and tdm, tiers 4-5/9-10/13-15, seeds 1-8).
