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
