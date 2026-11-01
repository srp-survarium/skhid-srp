# Weapons from 0.46c2 (pack weapons-046)

Release 41, batches 2 and 2b (the owner, 2026-11-01): ten guns of retail 0.46c2 (April 2017) that live
Skhid lacks, as new items. No premium or skin copies. Every number below is a proposal: the owner
balances "around the current balance".

| Gun | Item | Dict | Class (template) | Ammo |
|---|---|---|---|---|
| H&K MP5 | `mp5a1` | 2270 | SMG, 15 (Vityaz) | 9x19 (53, 71, 368) |
| SVU | `svu_a` | 2271 | sniper, 13 (SVD) | 7.62x54 7N1 (148) |
| L85A2 | `l85a2` | 2272 | assault, 8 (M4A1) | 5.56x45 (2201) |
| Fort-401 | `fort_401` | 2273 | machine gun, 16 (PKP) | 7.62x51 belt of 100 (2275) |
| Pancor Jackhammer | `jackhammer` | 2274 | shotgun, 14 (Saiga-12K) | 12/70 (2205, 72) |
| CZ 75 | `cz_75` | 2280 | pistol, 17 (Fort 17) | 9x19 (53, 71, 368) |
| MP-443 Grach | `mp443` | 2281 | pistol, 17 (Fort 17) | 9x19 |
| Beretta 93R | `beretta93r` | 2282 | pistol, 17 (APS) | 9x19 |
| Glock 17 | `glock_17` | 2283 | pistol, 17 (APS) | 9x19 |
| Colt M1911 | `colt_m1911` | 2284 | pistol, 17 (Fort 17) | .45 ACP (323) |

## Rules

- **Balance around the live guns** (the owner, 2026-11-01). Each gun's stats are its peer's live
  values times the donor's ratio to that peer in 0.46c2 (donor x live peer / donor peer): damage, rate
  of fire, magazine, reload and aim time. 0.46c2 puts armour piercing on the ammo, so a gun takes its
  peer's piercing and stopping power. Recoil, dispersion, sway and damage fall-off are the template's,
  the live peer's. A pistol in 9x19 has no live peer of its calibre: it takes the 9x19 SMGs' damage
  (0.40) scaled by its donor's ratio, and the Fort 17's (or APS's) handling; the Colt takes the UMP45's
  .45 ACP damage.
- **Levels and factions** as `prices.rs` sells them: Scavengers and Black Market items 1 to 5 (rank =
  level, capped at 5), Renaissance and Edge 6 to 15 (rank = level - 5). The long guns keep their
  donor's faction; the pistols (no faction in 0.46c2) take Renaissance or Edge at level 6 and 7, the Colt the Black Market beside the Colt Python. A donor one level above its
  peer in 0.46c2 sits one level above it here, within 15.
- **Fire modes** are the donor's: the L85A2 auto, single and burst of 3; the Jackhammer automatic only
  (0.46c2's, on the Saiga's `wpn_asgn`); the Beretta 93R bursts of 3 and single shots; the Glock 17
  automatic and single. The SVU is `wpn_cara` in 0.46c2: here the SVD's `wpn_aslt`, single fire.
- **Ammo** is live ammo, but for the Fort-401: a default kit holds 3 bags, so a 100-round box fed the
  rifle 7.62x51 (10 a unit) starts short and never reloads. It takes `ammo_7.62x51_belt` (2275): the
  live 7.62x51's config in a belt of 100, 2 bags, 2.5 kg, level 14, as the PKP's 7N13 belt (2204):
  dictionary category 21, 300 a belt (srp 150), sold by Edge at rank 9 (the Fort-401's seller). The
  other guns' units fill a kit: 9x19 and .45 come in 50, buckshot 20, slugs 15, 5.56 30, 7N1 10.
- **Models as 0.46c2 shows them.** A few 0.46c2 guns carry parts as default attachments (static models
  at a locator): the MP5's forend, the L85A2's carry-handle sight, forend and flash hider, the
  Fort-401's sight and muzzle brake, the SVU's muzzle brake, and the Glock 17's frame, slide, barrel,
  magazine, sights and back plate. They are baked into the gun's model as surfaces of their own,
  named `<part>_<surface>` (`muzzle_*` for the devices), on the locator's bone (the Glock's slide on
  the slide's bone, its magazine on the magazine's).
- **Camouflage and decals** as the live weapons (client patches 44 and 47): `weapon-camo-046-1`
  gives the ten configs `camo_available`, `decal_available` and their `data` table; their masks are
  the pack `weapons-046-camo` (`scripts/weapons-046/camo.tsv`), on the parts `weapon_camo.py`
  suggests: there is no premium copy to learn the painted parts from, so the picker
  (`scripts/weapon-camo`) chooses others when the owner wants.
- **Kill feed:** the guns' own kill-log icons (0.46c2's 75, 77 to 80, 82 to 84; the Glock's 95 from
  0.69d0, as 0.46c2 gave it the APS's; the Colt M1911's 99, drawn from its lobby cell, as no build has
  one: 0.46c2 gives it the PM's 16, 0.69d0 the CZ 75's 82), in the hud and the lobby's tooltips; the
  hud finds each `CombatLogWeapon<icon>` by name, so a ported weapon adds a bitmap, no list. The port
  refuses two guns on one new icon, and a gun's own image on a live gun's number.
- **Lobby icons:** 0.46c2's diagonal renders turned level and mirrored (0.26g draws a gun muzzle
  right), in a sixth column of the weapon atlas (cells 1 to 10).
- **Clips on their own skeleton.** The lobby holds a gun with `preview_item`, now the gun's own idle
  (the template's was another gun's: the PKP's on the Fort-401, the PM's on the pistols, which
  crashed the lobby in bone_matrices_computer at every sign-in). The port refuses a config whose clip
  animates bones of another skeleton (a state's item clips and `preview_item` on the gun's, every
  other clip on the player's), and any resource key naming nothing.
- **Sights by the model's own `aim` locator:** 0.46c2 has no `aim_offset`, and the template's
  (the SVD's -0.11, the PKP's 0.12, the pistols' -0.1) put the SVU's sight 30 px and the Fort-401's
  post 17 px low: the port leaves it out, as for 0.31a2.
- **Sounds carry their loudness preset.** A sound's `.raw_options` names an SPL preset
  (`sounds/spl/<preset>.spl`); 0.46c2's Beretta 93R and Glock 17 shutters name
  `main_3rd_view_suppressed`, which 0.26g had not, and the CZ 75's shot (the Colt's too) plays the
  Beretta's shutter. The sound cook then read a null config (exe+0x62731) and every match with one of
  the four crashed at load. The pack ships 0.46c2's preset; the port carries a sound's preset or stops.
- **Pistol hands are the live pistols'.** A hands_only clip is additive on the base pose of the
  gun's `user_animations` file. 0.26g's pistol base (`pistols_new.user_animations`) is a rifle pose;
  the live pistols' tt33 hands move the right hand and the Weapon bone into a pistol grip, 0.46c2's pm
  hands (made for 0.46c2's pistol base) move them by nothing: the MP-443 showed no hands and its gun
  at the rifle's place. The five pistols take the template's (Fort 17, APS) hands for show, fire,
  reload, hide and idle; their own item clips stay. The long guns keep 0.46c2's hands (rifle base).
- **Rarity:** the Glock 17's `base_rarity_level` is 0 (0.46c2's daily-quest Glock had 3, a gold name).
- **Decals:** the same default as the live pistols (`weapon-decals-1`: an empty `data/decals`; the
  size is client patch 44's, 6 cm to start, one for every weapon), so the pistols' look large.

## Stats

The new guns beside their closest live peers.

| Gun | Faction / level | Price | Damage | Pierce | RPM | Mag | Reload | Aim | Weight | Why |
|---|---|---|---|---|---|---|---|---|---|---|
| **MP5** | Black Market 5 | 15,000 | 38 | 20 | 800 | 30 | 3.37 | 0.20 | 2.6 | Vityaz x 0.46c2's 0.94 damage, 800/700 rate; one level above it |
| Vityaz | none 4 | 12,000 | 40 | 20 | 700 | 30 | 3.37 | 0.25 | 3.0 | |
| UMP45 | Edge 8 | 27,720 | 55 | 25 | 600 | 25 | 3.7 | 0.25 | 2.5 | |
| **SVU** | Edge 15 | 75,000 | 93 | 50 | 333 | 5 | 3.1 | 0.35 | 4.4 | SVD x 0.93 damage, 200/150 rate; 0.46c2 puts it a level above the SVD, capped at 15 |
| SVD | Renaissance 15 | 68,250 | 100 | 50 | 250 | 5 | 3.1 | 0.35 | 4.5 | |
| **L85A2** | Scavengers 12 | 72,000 | 48 | 32 | 650 | 30 | 3.0 | 0.30 | 3.8 | M4A1 x 0.96 damage, 650/750 rate; the M4A1's level in both builds |
| M4A1 | Black Market 12 | 75,000 | 50 | 32 | 750 | 30 | 3.2 | 0.25 | 3.0 | |
| MSBS Radon | Scavengers 12 | 75,000 | 50 | 32 | 750 | 30 | 3.0 | 0.30 | 3.7 | |
| **Fort-401** | Edge 14 | 95,000 | 39 | 45 | 700 | 100 | 6.9 | 0.36 | 6.5 | PKP x 0.86 damage, 750/700 rate, 5.5/8 reload; a level below it, as in 0.46c2 |
| PKP | Renaissance 15 | 125,000 | 45 | 45 | 650 | 100 | 10 | 0.40 | 7.0 | |
| RPK-74M | Edge 11 | 69,300 | 50 | 30 | 600 | 45 | 3.17 | 0.35 | 5.0 | |
| **Jackhammer** | Black Market 13 | 85,000 | 93 | 10 | 300 | 10 | 2.56 | 0.30 | 4.6 | Saiga x 0.78 damage, 300/350 rate, 10/5 mag; a level above it; automatic only |
| Saiga-12K | Edge 12 | 75,000 | 120 | 10 | 350 | 5 | 2.8 | 0.30 | 3.6 | |
| **CZ 75** | Edge 6 | 13,500 | 40 | 20 | 300 | 16 | 1.4 | 0.20 | 1.0 | 9x19 damage (the 9x19 SMGs'), the Fort 17's rate and its 2.0/2.3 reload; the APS's level (both 3 in 0.46c2) |
| **MP-443** | Renaissance 6 | 13,500 | 40 | 20 | 300 | 18 | 1.4 | 0.20 | 0.95 | as the CZ 75, 18 rounds |
| **Beretta 93R** | Edge 6 | 14,000 | 34 | 20 | 580 | 20 | 1.0 | 0.20 | 1.1 | APS x 0.96 damage, 700/600 rate; bursts of 3; the APS's level (both 3 in 0.46c2) |
| **Glock 17** | Renaissance 7 | 15,000 | 36 | 20 | 375 | 17 | 1.25 | 0.20 | 0.7 | APS x 1.04 damage, 450/600 rate; automatic; a level above the APS, as in 0.46c2 |
| **Colt M1911** | Black Market 3 | 4,500 | 55 | 25 | 260 | 7 | 1.4 | 0.20 | 1.1 | .45 ACP as the UMP45; 0.46c2 puts it beside the Colt Python |
| PM | Scavengers 2 | 2,630 | 35 | 20 | 250 | 8 | 1.6 | 0.20 | 0.75 | |
| Colt Python | Black Market 3 | 4,280 | 60 | 30 | 120 | 6 | 3.5 | 0.20 | 1.1 | |
| Fort 17 | Renaissance 6 | 12,830 | 35 | 20 | 300 | 12 | 1.6 | 0.20 | 0.75 | |
| APS | Edge 6 | 12,830 | 35 | 20 | 500 | 20 | 1.3 | 0.20 | 1.2 | |

Drop weights follow the level, as the peers': 130 at level 3, 110 at 5, 100 at 6 and 7, 80 at 12, 70 at
13, 50 at 14, 40 at 15.

## Muzzles (for the muzzle device step)

In 0.46c2 a device is part of the mesh (the Fort-401's, nothing to remove) or a default part, baked
here as `muzzle_<surface>` (the SVU's, the L85A2's): the muzzle step can leave that surface out. `muzzle_point` is the
barrel's end (where 0.69d0 mounts a device), `fire_point` the end of the muzzle with any built-in device;
0.26g's `barrel_point` is put at `fire_point` (the porter's `donor_046.rs`). Metres, the model frame.

| Gun | muzzle_point (0.46c2 = 0.69d0) | fire_point 0.46c2 | fire_point 0.69d0 | Built-in device |
|---|---|---|---|---|
| MP5 | 0, 0.0401, -0.3579 | -0.3735 | -0.3735 | none (the 3-lug barrel) |
| SVU | 0, 0.0166, -0.4173 | -0.5952 | -0.5952 | an 18 cm muzzle brake fused into the mesh: skip |
| L85A2 | 0, 0.0700, -0.3486 | -0.4344 | -0.3578 | flash hider (0.69d0 module `enfieldl85a2_i05x28_muzzlebrake`) |
| Fort-401 | 0, 0.0632, -0.6210 | -0.6833 | -0.6331 | muzzle brake (0.69d0 module `fort401_i05x28_muzzlebrake`) |
| Jackhammer | none | -0.3510 | -0.3510 | none |
| CZ 75 | 0.0062, 0.0401, -0.1564 | the same | the same | none |
| MP-443 | 0, 0.0441, -0.1549 | the same | the same | none |
| Beretta 93R | 0, 0.0445, -0.1792 | the same | the same | none |
| Glock 17 | 0, 0.0266, -0.1529 | the same | (0.69d0's `glock_17_sandy`: a barrel module) | none here (its barrel is a baked part, `barrel_*`); 0.69d0's default has a clamp brake |
| Colt M1911 | 0.0001, 0.0268, -0.1691 | the same | the same | none |

## Not in this pack

- Premium and skin copies (the owner: none).
- Scope variants (`fitn_*`): the guns aim with their iron sights. The SVU's template is the SVD; it does
  not take the SVD's PSO-1 (placed for the SVD's mount), and its lobby icon has 0.46c2's PSO-1 erased.
- The weapon kit (#893, not merged): when it is, these configs take its data as the live guns do.
- Muzzle devices (the muzzle step, above).
