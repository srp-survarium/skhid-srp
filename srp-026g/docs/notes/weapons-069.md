# The 0.69d0 weapons (packs weapons-069, weapons-069-camo)

Six guns only retail 0.69d0 has, and the KS-23M's ammo, for release 41 (batch 4 of the
owner's weapon batches, 2026-11-01). Built by `scripts/weapons-069/make_pack.sh` with
resource-porter's shared weapon core, 0.69d0 as its donor (`port-weapons-069d`,
`ports/weapons-069d.json` and its sidecar; `docs/weapons-069d-026.md`). The camouflage masks are
pack weapons-069-camo (`camo.tsv`; `parts.tsv` leaves the cartridges and the KS-23M's loose shell
plain, and `--bare-masks` still gives each a mask, so no surface of a camouflaged gun fails to load
its `_aoac`), the configs take the camouflage and decal by the edit list `weapon-camo-069-1.json`.
Built over weapons-031 and weapons-046 (`make_pack.sh ... 7 2 weapons-031@5 weapons-046@5`): what
they add is reused, not added again (the MP5's bolt sounds the G3SG/1 plays are weapons-046's, the
G3SG/1's 7.62x51 in clips of 20 weapons-031's), so their rows come before this one's.

## Owner rules

- Stats fit the live balance ("we will balance them around the current balance"): each gun sits
  among the live guns of its class and calibre; 0.69d0's own numbers give only its shape against
  its 0.69d0 peer (fire rate, magazine, handling).
- Levels and factions follow `prices.rs`'s rule (rank = item level, less 5 for Renaissance and
  the Fringe; capped at the faction's last rank).
- No premium or skin variants; no suppressors.
- Modules: only a gun's structural parts are baked in (receiver parts, stocks, pistol grips,
  magazines, bolts, the bare barrel). Muzzle devices, foregrips and scopes stay separable for the
  weapon kit; their locators are below.

## The items

| Item | Dict id | Config | Lobby cell | Kill feed |
|---|---|---|---|---|
| AKMSU | 2290 | `wpn_akmsu` | weapons [17, 5] | 96 |
| FAMAS | 2291 | `wpn_famas` | weapons [18, 5] | 97 |
| H&K G3SG/1 | 2292 | `wpn_g3sg1` | weapons [19, 5] | 98 |
| Mauser M.712 | 2293 | `wpn_mauser_m712` | weapons [20, 5] | 86 |
| KS-23M | 2294 | `wpn_ks_23m` | weapons [21, 5] | 85 |
| PPSH-41 "Drum" | 2295 | `wpn_ppsh_drum` | weapons [22, 5] | 33 (the PPSh's) |
| 23x75R, smallshot | 2253 | `ammo/ammo_23mm_r` | ammo [1, 1] (the 12x70 smallshot's, as 0.69d0) | - |

- Kill feed icons are 0.69d0's own numbers and images (`icon_<n + 1>.png`): hud bitmaps
  `CombatLogWeapon85/86/96/97/98` (`CombatLogItem` finds an icon past the clip's frames by its
  class name), and the lobby's `KillFeedWeapon<n>` from them (`make_lobby_weapon_icons.py`).
- Lobby icons are 0.69d0's own (`weapon_<y>_<x>.png` of its `ui_desc.icon`), which 0.69d0 draws
  diagonally: the port levels them (mirrored, turned by the art's axis, muzzle right) and fits
  them to the cell ([weapons-069-icons.png](weapons-069-icons.png): before, renders of the
  diffuse alone; after). The 23 mm round shows the 12x70 smallshot's cell, as 0.69d0 does.
- Texts: `st_srp_weapon069_<name>_name/_desc` and `st_srp_ammo069_23mm_r_name/_desc` in the ten
  localization tables.

## Rigs, clips and sounds

The newest animations and sounds (the owner): every gun plays 0.69d0's own states, clips and sound
mixes, but the FAMAS's idle and fire (the Ash-12's, which 0.69d0 plays too). Where 0.26g has an
older file under the same name, the port moves 0.69d0's to a name of its own (the donor sidecar's
`renames`, resource-porter's `donor_069d/renames.rs`), so the older one stays the live guns':

- clips: 0.69d0's `ak/` (the AKMSU's) to `akmsu_069/`, its `ppsh/` (the Drum's) to `ppsh_drum/`;
- sound mixes: `weapons/<name>` to `weapons/<name>_069` for akm_n, show_hide_aimed_states (draw
  and holster), m4a1, vssk (the FAMAS's reload), socom16, mp5 and saiga_12 (the G3SG/1's),
  rem_870 and remington_870 (the KS-23M's), ppsh (the Drum's and the Mauser's fire), fort_17
  (the Mauser's reload), and the layers they share: benelli_m4, hk416, pp2000, vepr, vityaz.

- AKMSU: 0.69d0's own AK rig, clips and hand points (its left hand on its own forend; the
  AKS-74U's grip sat 3.6 cm behind it), its reload of eight sounds, the AKM firing mix.
- FAMAS: the live Ash-12 rig and its idle and fire clips (0.69d0 plays them too), 0.69d0's draw,
  holster (with their sound) and reload; the M4A1 firing mix.
- PPSH-41 "Drum": 0.69d0's own PPSh rig, clips and hand points: the drum grip, the left wrist on
  the drum's front (the live PPSh's points held the box magazine's place).
- G3SG/1, Mauser, KS-23M: their own 0.69d0 rigs and clips. The KS-23M is a pump as the
  Remington 870 (start, one shell, finish, chambering); 0.69d0's "load into an empty chamber"
  start and the Mauser's aimed shots are 0.69d0-only and left out. The Mauser's empty reload
  plays the slide at its fifth event (sound 6: 0.69d0's clip leaves its events' indices at 255).
- The KS-23M's fire sounds are four (the live Remington 870's), not 0.69d0's eight: its fire
  state lists them twice (unsuppressed, then a suppressed half that repeats them). Every entry of
  an item config counts against the 16 KB stack arena the client rebuilds an upgraded item in,
  which nothing bounds (`create_binary_config_buffer`, exe 0x358AA0); at 587 entries the
  camouflaged KS-23M overflowed it and crashed every match at load (play41c, 2026-11-01). srp's
  game-data check refuses an item config over 565 entries, the largest stock one's
  (`tables::MAX_CONFIG_ENTRIES`).
- A donor state keeps 0.69d0's type as 0.26g numbers it: the two-clip reload (tactical and empty)
  of the FAMAS, G3SG/1, Mauser and drum is 0.26g's 314; the Mauser's two-clip idle and fire 313
  and 315.
- Sights: 0.69d0's aim locator, which 0.69d0 places at the eye; 0.69d0's configs carry
  `aim_offset` on no weapon, so a gun keeps its template's (the KS-23M the Remington 870's 0.03:
  at 0 its aim sat at the receiver's top and the receiver filled the lower screen; the Drum the
  PPSh's 0.01, its aim at the live PPSh's height). The Mauser sets 0 (its aim was within 2 px;
  the APS's -0.1 is the APS model's).

## Stats (live peers beside each)

Damage, pierce: per bullet x 100. Price: the dictionary's `base_cost` (the shop's, but for the
owner's prices). Factions: 1 Scavengers, 2 Black Market, 3 Renaissance, 4 Fringe Settlers.

| Gun | Damage | Pierce | RPM | Mag | Level | Faction (rank) | Price |
|---|---|---|---|---|---|---|---|
| **AKMSU** | 45 | 40 | 625 | 30 | 6 | 3 (1) | 19,000 |
| AKM | 45 | 40 | 600 | 30 | 6 | 3 (1) | 20,900 |
| AKS-74U | 50 | 30 | 650 | 30 | 5 | 2 (5) | 12,920 |
| **FAMAS** | 50 | 32 | 900 | 30 | 14 | 1 (5) | 100,000 |
| H&K 416A5 | 55 | 34 | 850 | 30 | 14 | 2 (5) | 120,000 |
| M4A1 | 50 | 32 | 750 | 30 | 12 | 2 (5) | 75,000 |
| **G3SG/1** | 64 | 40 | 240 | 20 | 15 | 2 (5) | 80,000 |
| SOCOM 16 | 60 | 40 | 300 | 10 | 15 | - | 75,000 |
| SVD | 100 | 50 | 250 | 5 | 15 | 3 (10) | 68,250 |
| **Mauser M.712** | 35 | 25 | 580 | 10 | 6 | 3 (1) | 13,000 |
| APS | 35 | 20 | 500 | 20 | 6 | 4 (2) | 12,830 |
| TT-33 | 30 | 25 | 300 | 8 | 0 | - (owner's 500) | 0 |
| **KS-23M** | 120 | 10 | 180 | 4 | 7 | 3 (2) | 26,000 |
| Remington 870 | 110 | 10 | 180 | 4 | 4 | 2 (4) | 12,350 |
| Benelli M4 | 120 | 10 | 280 | 6 | 7 | 4 (2) | 28,350 |
| **PPSH-41 "Drum"** | 30 | 25 | 1000 | 71 | 2 | 1 (2) | 6,000 |
| PPSH-41 | 30 | 25 | 1000 | 35 | 0 | - (owner's 1,000) | 0 |

Why each sits there:

- AKMSU: the AKM's round and damage (0.69d0: the same 0.38), 0.69d0's 4 % faster fire, the
  AKS-74U's short-barrel spread and falloff (0.3, 15-40 m), 2.9 kg; the AKM's level and seller.
- FAMAS: 5.56x45 at the M4A1's damage (0.69d0: 0.35 against its 0.36 and the H&K's 0.35),
  0.69d0's 900 rpm and 3-round burst, a slower reload (4.0 s, 0.69d0's +33 % on the H&K); level 14
  beside the H&K, sold by the Scavengers (0.69d0's) beside the MSBS Radon.
- G3SG/1: 7.62x51 semi-auto as the SOCOM 16 (the closest live gun), 0.69d0's +6 % damage, its
  -20 % rate and burst of 3; 20 rounds, the real gun's and 0.69d0's (the owner, 2026-11-01:
  over the SOCOM's 10); 5.2 kg; level 15, Black Market (0.69d0's).
- Mauser M.712: an automatic pistol as the APS (0.69d0: the same 0.42 damage), 7.62x25's pierce
  (the TT-33's), 0.69d0's +17 % rate and half the APS's magazine (10), 1.7 kg; the APS's level,
  sold by Renaissance (0.69d0's has no seller).
- KS-23M: a pump as the Remington 870, 0.69d0's +7 % damage (3.2 against 3.0, rounded to the
  live semi-autos' 1.2), its +25 % chambering (0.75 s) and +5 % reload; level 7 with the Benelli
  and the SPAS, Renaissance (0.69d0's) at rank 2.
- PPSH-41 "Drum": the PPSh-41 with 0.69d0's 71-round drum, its slower reload (+25 %: 4.6 s) and
  aim (0.3 s), 3.9 kg; level 2 from the Scavengers (the PPSh is the starting gun, sold to all).
- The G3SG/1's round: weapons-031's 7.62x51 in clips of 20 (2254, shared with the SCAR-H). A
  default kit's three ammo bags hold three clips of a round: of the live 7.62x51 (clips of 10) 30
  rounds, under a magazine of 20 and a reload. Every gun's rounds are checked so
  (`port-weapons-069d`): the drum's 71 and a reload of 7.62x25 fit 150, the KS-23M's 4 and a reload
  45.
- 23x75R, smallshot: 0.69d0's twin of its 12/70 smallshot, so the live 12x70 smallshot's
  numbers (10 pellets of 0.2, armour piercing 0.2, 15 a clip) and price (20, the shop's 10);
  sold by Renaissance (`AMMO_PRICES`, `EXCEPTIONS`).

## Attach points (bind frame, metres, from the bone)

For the weapon kit (muzzle devices at `muzzle_point`; `barrel_point` is the bare barrel's end,
`fire_point` where 0.69d0's own device fired from).

| Gun | Locator | Bone | Position | 0.69d0's module left out |
|---|---|---|---|---|
| AKMSU | muzzle_point | offset | 0, 0.0473, -0.3738 | akmsu_m24x15_muzzlebrake |
| AKMSU | fire_point | offset | 0, 0.0473, -0.3961 | |
| FAMAS | muzzle_point | offset | 0, 0.0537, -0.3623 | famas_default_muzzlebrake |
| FAMAS | fire_point | offset | 0, 0.0537, -0.4077 | |
| FAMAS | scope_picatinnyrail_point | offset | 0, 0.1290, -0.1072 | |
| FAMAS | grip_picatinnyrail_point | offset | 0, 0.0200, -0.1831 | |
| G3SG/1 | muzzle_point | offset | 0.0003, 0.0411, -0.6106 | g3a3_default_muzzlebrake |
| G3SG/1 | fire_point | offset | 0.0003, 0.0411, -0.6290 | |
| G3SG/1 | scope_picatinnyrail_point | offset | 0.0003, 0.0758, -0.0365 | |
| G3SG/1 | grip_picatinnyrail_point | offset | 0.0003, 0.0219, -0.2944 | |
| G3SG/1 | tactical_picatinnyrail_point | offset | 0.0003, 0.0239, -0.4443 | |
| KS-23M | muzzle_point | offset | 0, 0.0457, -0.3565 | |
| KS-23M | scope_picatinnyrail_point | offset | 0, 0.0663, -0.1405 | |
| Mauser M.712 | muzzle_point | barrel | 0, 0.0002, -0.1333 | |
| PPSH-41 "Drum" | fire_point | offset | 0.0003, 0.0408, -0.5656 | (no muzzle slot) |
| PPSH-41 "Drum" | scope_picatinnyrail_point | offset | 0.0007, 0.0598, -0.3754 | |

## The card

Damage, pierce, aim, stopping power, spread, fall-off start, rate, magazine (prop 134) and weight,
as the core writes every ported gun's.

## Not in this pack

- Scope fit variants (`fitn_*`): no scope takes these guns yet.
- The weapon kit's records (#893, not merged). The camouflage masks paint each texture's large
  parts (weapon_camo.py's suggestion, no premium to read them from), not yet picked by the owner.
