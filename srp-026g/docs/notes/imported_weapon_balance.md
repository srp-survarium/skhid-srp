# Imported weapons: Zubr +8 balance proposal

Local balance pass based on `009b2dabe` (#947 over main's #948 and #949), 2026-11-02.
Not released or playtested. The changes are in `scripts/content-edit/weapon-stats.tsv` and
`ammo.tsv`; their existing manifest rows build the configs and card values. No pack was
assembled or revision reserved. A `next` simulation row invalidates older replays when released.

## Decisions

- Keep AK-12 and PKP unchanged as top-level references. Tune the ten imported primary weapons
  intended for the upper pool, with the L85 subsequently moved to the lower pool.
- Groza: base penetration **55** (AK-12 40), **66 with SP-6**, **60.5 with SP-5**.
  Damage 50 -> 42, magazine stays 20, full/minimum-damage distances 23/65 -> 15/35 m,
  minimum damage fraction 0.6 -> 0.5. SP-5 extends those distances by 1.5, so the actual
  best-ammo curve matters. This is an explicit armour-specialist decision, not a claim
  that retail 0.69 used these penetration units. At 42 damage it needs two helmet hits
  with either SP-5 or SP-6; 44 damage would accidentally give SP-5 a one-shot helmet kill.
- L85A2: equipment **12 -> 10**, matchmaking level **12 -> 10**, damage stays 48,
  penetration stays 32; 725 rpm, 25/60 m falloff, much softer kick and sideways variation.
  It trades close-range damage output for control and reach. Price/faction are unchanged.
- MP5 (5), AKMSU (6), PPSh Drum (2), KS-23M (7) remain in the level-10 pool.
  MP5, AKMSU and Drum keep their imported damage, penetration and rate of fire.
  Lower-pool guns are not required to beat Zubr +8. The rest of a player's kit must also
  fit the room's cap; [rooms.md](rooms.md) defines the <=10 and 11–15 pools.
- **Pistols remain weak backups. All six keep their imported damage, penetration and RPM.**
  Their handling differs: CZ precision/reach, MP-443 faster aim, Beretta bursts,
  Glock light automatic with stronger kick, Colt heavy individual shots/small magazine,
  Mauser fast but only ten rounds. A Mauser magazine still cannot kill this target's torso.
- KS-23M: damage 120 -> 160, penetration 10 -> 18; its own 23 mm ammunition's penetration
  multiplier 0.2 -> 1.0. No shared shotgun ammo changes. Still two *complete* pellet hits
  against this torso, a 0.75 s pump and only four shells. This deliberately retains its
  lower-pool role instead of making it a guaranteed Zubr one-shot.
- Jackhammer: damage 93 -> 105, penetration 10 -> 30, short 8/25 m falloff, floor 35%.
  Two complete close-range buckshot hits can kill; partial pellets cost shells. Its ten-round
  automatic drum is its niche. The stronger penetration is a deliberate upper-pool choice.
- G3SG/1: damage 64 -> 90, penetration 40 -> 50, 40/110 m falloff, floor 65%; retain
  240 rpm and 20 rounds. Three torso hits up close, with a substantial kick increase;
  it earns a sustained marksman role beside the SVU's faster, five-round precision bursts.
- FAMAS: remove the ASh-12 template's heavy stopping/stamina damage (50 -> 15), keep
  900 rpm but damage 50 -> 47 and earlier 15/45 m falloff. Its fire rate has a range
  and ammunition-efficiency cost. VHS-2 keeps its 850 rpm, gains 25/55 m reach and control.
- Fort-401 and Minimi gain usable sustained fire: Fort 43 damage / 43 penetration /
  750 rpm / 30–65 m; Minimi 44 / 38 / 800 / 30–60 m. Their 100-round belts, reload
  times and handling remain distinct. SVU cadence falls from 333 to 300 rpm, with
  faster aim, a shorter damage plateau and stronger shot kick.

## Measurement contract

Stock weapons, best **compatible** ammo at each distance, full-health Zubr +8: torso 98,
helmet 83. No weapon rolls, artefacts, drugs, cover or pre-existing injury. The +8 is the
additive armour modification 1004, not an invented eight-step upgrade multiplier.

`audit.js` calls the existing damage calculator's `bulletAt` and `simulate`: both damage
and penetration fall with bullet speed, ammunition scales the distances and floor, and
head and torso have their actual distinct health pools. The current balance/armour/ammo/
modification TSVs supply the mutable values. `reference.json` contains extracted body,
ammo compatibility and drag metadata, with source hashes; it is analysis data, never
server gameplay data. `collect_inputs.py` reproduces it from the calculator and the
three imported packs, using their actual dictionary operations.

TTK below is an **ideal single-magazine firing-time lower bound**, first shot at t=0:
`(hits - 1) * max(60 / rpm, chamber_time)`. Aim/draw, bullet flight, tick scheduling,
semi/burst re-trigger delays, recoil-induced misses and reloads are excluded. `reload`
means the required shots exceed the magazine, not an estimated reload duration.
The pump interval is at least its chamber time. In-game timing/accuracy requires a
playtest; these tables do not claim an empirical win rate.

Shotgun cells assume every pellet hits the named part, even at long distance: they are
optimistic bounds, **not practical shotgun range**. The audit also outputs half-pellet
shots needed. Kills per magazine are `floor(capacity / shots_to_kill)`. The 50% column
below budgets one miss per hit; it is not a probabilistic expected kill count.

## Torso TTK across distance

Seconds, best ammo selected independently at each distance. Zubr +8 throughout.

### Upper pool and references

| Gun | Level | 0 m | 10 m | 20 m | 30 m | 50 m | 75 m | 100 m |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| AK-12 | 15 | 0.369 | 0.369 | 0.369 | 0.554 | 1.108 | 1.108 | 1.108 |
| Groza | 13 | 0.257 | 0.257 | 0.257 | 0.343 | 0.857 | 1.029 | 1.029 |
| SCAR-H | 15 | 0.288 | 0.288 | 0.288 | 0.384 | 0.672 | 0.864 | 0.864 |
| VHS-2 | 14 | 0.353 | 0.353 | 0.353 | 0.424 | 0.847 | 0.988 | 0.988 |
| Minimi | 14 | 0.375 | 0.375 | 0.375 | 0.375 | 0.750 | 1.125 | 1.125 |
| SVU-A | 15 | 0.200 | 0.200 | 0.200 | 0.200 | 0.400 | 0.400 | 0.600 |
| Fort-401 | 14 | 0.400 | 0.400 | 0.400 | 0.400 | 0.640 | 1.040 | 1.040 |
| Jackhammer | 13 | 0.200 | 0.400 | 0.600 | 0.800 | 1.400 | 1.400 | 1.400 |
| FAMAS | 14 | 0.333 | 0.333 | 0.400 | 0.600 | 1.000 | 1.000 | 1.000 |
| G3SG/1 | 15 | 0.500 | 0.500 | 0.500 | 0.500 | 0.500 | 0.750 | 1.000 |
| PKP | 15 | 0.369 | 0.369 | 0.369 | 0.369 | 0.831 | 1.108 | 1.108 |

### Level-10 pool primaries

| Gun | Level | 0 m | 10 m | 20 m | 30 m | 50 m | 75 m | 100 m |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| MP5 | 5 | 0.525 | 0.525 | 0.600 | 0.900 | 1.425 | 1.425 | 1.425 |
| L85A2 | 10 | 0.497 | 0.497 | 0.497 | 0.497 | 0.910 | 1.324 | 1.324 |
| AKMSU | 6 | 0.384 | 0.384 | 0.480 | 0.672 | 1.152 | 1.152 | 1.152 |
| KS-23M | 7 | 0.750 | 0.750 | 1.500 | reload | reload | reload | reload |
| PPSh Drum | 2 | 0.720 | 0.720 | 0.840 | 1.200 | 1.920 | 1.920 | 1.920 |

### Pistols (no AK-12 parity requirement)

| Gun | Level | 0 m | 10 m | 20 m | 30 m | 50 m | 75 m | 100 m |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| CZ 75 | 6 | 1.200 | 1.200 | 1.200 | 1.800 | reload | reload | reload |
| MP-443 | 6 | 1.200 | 1.200 | 1.600 | 2.200 | reload | reload | reload |
| Beretta 93R | 6 | 0.828 | 0.828 | 0.931 | 1.345 | reload | reload | reload |
| Glock 17 | 7 | 1.120 | 1.120 | 1.280 | 1.920 | reload | reload | reload |
| Colt M1911 | 3 | 1.385 | 1.385 | reload | reload | reload | reload | reload |
| Mauser M.712 | 6 | reload | reload | reload | reload | reload | reload | reload |

## Before/after and magazine efficiency

Close torso TTK uses 0 m. Magazine columns show perfect hits / one miss per hit.
Head TTK is the same Zubr +8 target at 0 m, with its helmet.

| Gun | Torso before -> after | Head after | Kills/mag 0 m | Kills/mag 30 m | Kills/mag 50 m |
|---|---:|---:|---:|---:|---:|
| AK-12 | 0.369 -> 0.369 | 0.092 | 6 / 3 | 4 / 2 | 2 / 1 |
| Groza | 0.343 -> 0.257 | 0.086 | 5 / 2 | 4 / 2 | 1 / 0 |
| SCAR-H | 0.288 -> 0.288 | 0.096 | 5 / 2 | 4 / 2 | 2 / 1 |
| VHS-2 | 0.353 -> 0.353 | 0.071 | 5 / 2 | 4 / 2 | 2 / 1 |
| Minimi | 0.450 -> 0.375 | 0.075 | 16 / 8 | 16 / 8 | 9 / 4 |
| MP5 | 0.525 -> 0.525 | 0.075 | 3 / 1 | 2 / 1 | 1 / 0 |
| SVU-A | 0.180 -> 0.200 | 0.000 | 2 / 1 | 2 / 1 | 1 / 0 |
| L85A2 | 0.554 -> 0.497 | 0.083 | 4 / 2 | 4 / 2 | 2 / 1 |
| Fort-401 | 0.429 -> 0.400 | 0.080 | 16 / 8 | 16 / 8 | 11 / 5 |
| Jackhammer | 1.000 -> 0.200 | 0.000 | 5 / 2 | 2 / 1 | 1 / 0 |
| CZ 75 | 1.200 -> 1.200 | 0.200 | 2 / 1 | 1 / 0 | 0 / 0 |
| MP-443 | 1.200 -> 1.200 | 0.200 | 2 / 1 | 1 / 0 | 0 / 0 |
| Beretta 93R | 0.828 -> 0.828 | 0.207 | 2 / 1 | 1 / 0 | 0 / 0 |
| Glock 17 | 1.120 -> 1.120 | 0.160 | 2 / 1 | 1 / 0 | 0 / 0 |
| Colt M1911 | 1.385 -> 1.385 | 0.231 | 1 / 0 | 0 / 0 | 0 / 0 |
| AKMSU | 0.384 -> 0.384 | 0.096 | 6 / 3 | 3 / 1 | 2 / 1 |
| FAMAS | 0.333 -> 0.333 | 0.067 | 5 / 2 | 3 / 1 | 1 / 0 |
| G3SG/1 | 0.750 -> 0.500 | 0.000 | 6 / 3 | 6 / 3 | 6 / 3 |
| Mauser M.712 | reload -> reload | 0.207 | 0 / 0 | 0 / 0 | 0 / 0 |
| KS-23M | reload -> 0.750 | 0.000 | 2 / 1 | 0 / 0 | 0 / 0 |
| PPSh Drum | 0.720 -> 0.720 | 0.180 | 5 / 2 | 3 / 1 | 2 / 1 |
| PKP | 0.369 -> 0.369 | 0.092 | 20 / 10 | 20 / 10 | 10 / 5 |

## Recoil, kick and screen shake

These are separate settings. Shot kick is the raw up range and signed `right - left`
range; sideways standard deviation is `sqrt((left_width² + right_width²) / 12)`.
`Power` is recoil_power; it is not multiplied into the tabulated first-shot kick.
The existing engine's accumulation and PID recovery apply during firing. PID gains
and shake time scale are unchanged; screen-shake bounds are listed separately.
No single scalar here is a simulated hit rate. Binary value types are preserved.

| Gun | Power | Up kick | Signed side kick | Side SD | Screen shake |
|---|---:|---:|---:|---:|---:|
| AK-12 | 0.5 | 1.5–1.5 | -0.40…0.40 | 0.163 | 0.8–1 |
| Groza | 0.65 | 1.2–1.5 | -0.50…0.50 | 0.204 | 0.4–1 |
| SCAR-H | 0.7 | 1.7–2 | -0.60…0.60 | 0.245 | 0.5–1 |
| VHS-2 | 0.55 | 1.3–1.6 | -0.30…0.30 | 0.122 | 0.3–1 |
| Minimi | 0.8 | 1.7–2 | -0.40…0.40 | 0.163 | 0.5–1 |
| MP5 | 0.45 | 1–1.1 | -0.20…0.20 | 0.082 | 0.3–1 |
| SVU-A | 1 | 2–3 | -1.00…1.00 | 0.408 | 0–0 |
| L85A2 | 0.4 | 0.9–1.1 | -0.20…0.20 | 0.082 | 0.2–0.4 |
| Fort-401 | 1 | 1.5–2 | -0.30…0.30 | 0.122 | 0.4–1 |
| Jackhammer | 1 | 2–2.8 | -0.40…0.40 | 0.163 | 0.8–1 |
| CZ 75 | 0.4 | 0.7–0.85 | -0.10…0.10 | 0.041 | 0–0 |
| MP-443 | 0.5 | 0.8–1 | -0.25…0.25 | 0.102 | 0–0 |
| Beretta 93R | 0.55 | 0.8–1 | -0.30…0.30 | 0.122 | 0–0 |
| Glock 17 | 0.6 | 1.1–1.4 | -0.40…0.40 | 0.163 | 0–0 |
| Colt M1911 | 0.7 | 1.3–1.6 | -0.30…0.30 | 0.122 | 0–0 |
| AKMSU | 0.8 | 2–2.2 | -0.60…0.60 | 0.245 | 0.6–1 |
| FAMAS | 1 | 1–1.8 | -0.50…0.50 | 0.204 | 0.4–1 |
| G3SG/1 | 0.8 | 3–4 | -0.60…0.60 | 0.245 | 0.9–1 |
| Mauser M.712 | 0.6 | 1–1.2 | -0.40…0.40 | 0.163 | 0–0 |
| KS-23M | 1 | 3–3.5 | -0.80…0.80 | 0.327 | 0–0 |
| PPSh Drum | 0.55 | 1–1 | -0.35…0.35 | 0.143 | 0.4–1 |
| PKP | 0.85 | 2.5–3 | -0.50…0.50 | 0.204 | 0.8–1 |

## Earlier-build evidence and placement

The checked retail 0.69d0 config is summarized in `scripts/weapon-balance/donor-069.tsv`:
its damage, cadence, magazine, effective distance, equipment tier and aim-recoil pattern.
Its source dump SHA256 is recorded there. Retail tiers 1–5 do **not** map mechanically
onto our 1–15 levels; its effective distance is not our two-point damage falloff.
Likewise `material_pierce` is not the 0.26 armour-penetration stat. No retail 0.69 TTK
against our Zubr has been invented.

L85 is tier 4 against AK-12 tier 5, first pattern magnitude 0.30 against 1.25, with
725 against 650 rpm: preserve its controllability, place it at the level-10 ceiling.
MP5 tier 3 and Drum tier 1 support keeping those lower; AKMSU and KS-23 stay at their
existing 6/7 on our roster because a compact AK and a four-shot pump can serve that pool.
KS-23 is actually retail tier 5: its lower placement here is our deliberate decision,
not a historical-tier claim. SVU and Fort were retail tier 4 but remain upper-pool here
because of their damage/precision and belt-fed capacity. Groza's very high penetration
is explicitly the owner's preference and is compensated in damage, reach and capacity.

The initial imports' peer-based decisions are documented in [weapons-031.md](weapons-031.md),
[weapons-046.md](weapons-046.md) and [weapons-069.md](weapons-069.md). This pass supersedes
those documents' initial balance numbers, keeping their asset/animation/ammo provenance.

| Gun | 0.69 tier | Damage | RPM | Mag | Effective m | First kick | Mean pattern kick |
|---|---:|---:|---:|---:|---:|---:|---:|
| AK-12 | 5 | 0.39 | 650 | 30 | 50 | 1.25 | 2.116 |
| Groza | 4 | 0.39 | 750 | 20 | 35 | 0.45 | 1.5955 |
| SCAR-H | 5 | 0.4 | 625 | 20 | 55 | 0.7 | 1.8405 |
| VHS-2 | 5 | 0.35 | 850 | 30 | 45 | 0.72 | 1.85433 |
| Minimi | 5 | 0.36 | 800 | 100 | 70 | 0.89 | 1.9593 |
| MP5 | 3 | 0.41 | 800 | 30 | 30 | 1.16 | 1.45 |
| SVU-A | 4 | 0.95 | 250 | 10 | 120 | 2.35 | 2.6 |
| L85A2 | 4 | 0.38 | 725 | 30 | 50 | 0.3 | 1.72967 |
| Fort-401 | 4 | 0.36 | 800 | 100 | 75 | 1.08 | 1.59815 |
| Jackhammer | 5 | 1.6 | 300 | 10 | 20 | 6.5 | 7.4 |
| CZ 75 | 2 | 0.46 | 350 | 16 | 25 | 0.73 | 0.88 |
| MP-443 | 2 | 0.46 | 350 | 18 | 25 | 0.72 | 0.89 |
| Beretta 93R | 4 | 0.46 | 600 | 20 | 25 | 0.64 | 0.83 |
| Glock 17 | 5 | 0.4 | 350 | 17 | 20 | 1.75 | 2.15 |
| Colt M1911 | 4 | 0.62 | 350 | 7 | 30 | 1.34 | 1.4 |
| AKMSU | 4 | 0.38 | 625 | 30 | 40 | 2 | 2.204 |
| FAMAS | 5 | 0.35 | 900 | 30 | 45 | 0.72 | 1.92533 |
| G3SG/1 | 5 | 0.9 | 240 | 20 | 120 | 10 | 10.221 |
| Mauser M.712 | 3 | 0.42 | 700 | 10 | 25 | 0.87 | 1.06 |
| KS-23M | 5 | 3.2 | 500 | 4 | 16 | 18 | 18 |
| PPSh Drum | 1 | 0.38 | 800 | 71 | 30 | 1.03 | 1.6131 |
| PKP | 5 | 0.4 | 650 | 100 | 85 | 1.32 | 1.83815 |

## A reason to pick each gun

| Gun | Niche | Cost |
|---|---|---|
| AK-12 | Top-level benchmark | Unchanged |
| Groza | High penetration, compact assault rifle | 20 rounds, sharper falloff and slow bullets |
| SCAR-H | Hard-hitting battle rifle | 20 rounds, deliberate kick |
| VHS-2 | Accurate medium-range automatic | Less penetration than AK-12 |
| Minimi | Fast sustained fire | Long reload and movement penalty |
| MP5 | Level-10 pool: mobile, controlled SMG | Weak against heavy armour |
| SVU-A | Quick follow-up sniper | Five rounds and substantial kick |
| L85A2 | Level-10 pool: steady rifle with good reach | Slower close-range TTK than AK-12 |
| Fort-401 | Stable belt-fed gun with good reach | Heavy and slow to aim |
| Jackhammer | Automatic room-clearing shotgun | Pellet accuracy, kick and sharp falloff |
| CZ 75 | Precise backup with slightly longer reach | Weak pistol damage |
| MP-443 | Fast-drawing, 18-round backup | Less precise than CZ 75 |
| Beretta 93R | Fast three-shot emergency bursts | Weak rounds and trigger cadence between bursts |
| Glock 17 | Light automatic backup | Pronounced kick, weak rounds |
| Colt M1911 | Harder-hitting individual pistol rounds | Seven-round magazine and heavy kick |
| AKMSU | Level-10 pool: quick compact rifle | Short reach and strong kick |
| FAMAS | Close-range high fire rate | Fast ammunition use, recoil buildup and early falloff |
| G3SG/1 | Long-range sustained marksman fire | Slow cadence and large kick |
| Mauser M.712 | Fast automatic backup | Ten weak rounds; cannot kill Zubr +8 torso in one magazine |
| KS-23M | Level-10 pool: heavy close-range pump | Four shells, slow pump, unreliable against heavy armour |
| PPSh Drum | Level-10 pool: 71-round sustained fire | Weak penetration, slow reload and aim |
| PKP | Top-level sustained-fire reference | Unchanged |

## Verification and remaining work

- Eight audit checks: armour, AK-12 distance breakpoints, compatible ammo, pump timing,
  partial pellets, a magazine that cannot kill, bullet reach, Groza helmet breakpoints and complete coverage.
- All 20 imported weapon configs and four imported ammo configs round-trip in memory
  through the real content table writer, retaining every value type and matching card text.
- Content editor unit suite passed. No Rust code or executable patch changes.
- No full release build, push, PR, deployment or live playtest was performed. Before pushing,
  run the repository's required full self-test/Unicorn/clippy/content build gates. The actual
  release follows the owner's normal approval at that moment.

Reproduce the detailed 440-row torso/head audit (ten distances for 22 weapons):

```sh
node srp-026g/scripts/weapon-balance/audit.js > /tmp/weapon-balance.tsv
node srp-026g/scripts/weapon-balance/audit.test.js
python3 srp-026g/scripts/weapon-balance/check_content.py "$SKHID_PACKS_DIR"
```

For before/after, extract `weapon-stats.tsv` and `ammo.tsv` from `009b2dabe`, then pass
those paths as the first and second arguments to `audit.js`. Keep both old tables: the
KS-23 ammo changed too. The generated TSV records the selected ammo at each distance,
shots to kill, magazine capacity, kills, one-miss-per-hit budget, and half-pellet results.
