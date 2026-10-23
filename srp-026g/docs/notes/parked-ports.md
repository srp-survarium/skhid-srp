# Parked ports: maps, weapons and equipment built or half-built, not shipped

Ports made with resource-porter (its branches named below) that are not live. Map porting wound
down after revision 22 (battery modes and Team Deathmatch only); equipment was paused by the
owner on 2026-10-17 (the masks of #269 excepted). What each one needs before it could ship, so a
port can be picked up without archaeology. Where each stands now: the status board.

## Maps

### S.T.A.L.K.E.R. multiplayer maps 197-201 (Pool, Training Camp, Repair Depot, Sports Center, Fire Station)

- Team Deathmatch maps from S.T.A.L.K.E.R.'s multiplayer; resource-porter branch
  `soc-mp-pool-026`. The owner: "looks amazing".
- Being worked on when parked: the lamp lights' banding, and every ladder rebuilt as a ramp under
  the game's real slope limit.
- Open:
  - walk tests: the ramps, firing (NaNs), the minimap marker, the loading screens;
  - stripes on the pool's window glass;
  - Training Camp and Repair Depot: players may walk out past the borders;
  - the loading-screen renders: the owner's approval;
  - keep or drop mp_pool (197): the wanted "Pripyat pool" was Sports Center;
  - a release needs the hud, lobby and localization changes merged with the other packs.
- Later candidates: the X-Ray maps of the ladder census (Rostok skipped).

### Duga (0.69d level_11) as map 205, Team Deathmatch

- Recipe `duga-069d-dm`, resource-porter branch `duga-069d-to-026` (local only: push it first).
  Round 2: the DDS pitch fix, sealed cave edges, matte cave water. Its own 0.69d loading screen,
  name and minimap.
- Fixed-camera load works; never walked in the game.
- Open:
  - vertex data is 79.2 MB of the 88 MB static pool: tight for a match;
  - draw calls about 45 % over native level_07's worst spawn;
  - some anomalous trees look pale;
  - anomalies and artefacts not ported;
  - the server needs mode-4 rows for 205.

### Radar (0.20e) day map 196, and the growing track fog

- 194-196 were the early day versions of Chemical (0.11e), Vostok Radar (0.11e) and Radar
  (0.20e). The first two shipped as the v0.11 maps (revision 22, maps-v011e); Radar did not.
- The faithful package looked right (graphics closed). The gameplay fill (no chargers, the
  owner's) adds a growing track fog, battery deposits and ammo crates; to check in the game:
  - when the fog's timer starts (map load or match start), its spread, the damage's lag behind
    it, fps;
  - the batteries and crates; team 1's tunnel kit sitting on the floor.
- Before a release: its original loading screen, srp's fixture rows (`026g-mode-timers.tsv`,
  `026g-result-modes.tsv`).
- The growing track fog is a dynamic-anomaly idea: calibrate it here, then consider it for other
  maps.
- Parked issues: moss and wall stripes on low mips (lead: a DDS pitch field off by one that makes
  0.26's streamer read the mip tail one byte early), the silent second-floor walkways on
  Chemical (resource-porter's step log, `step-log-hook`, built, never run), the faithful ambient
  sound curve.

### The other 0.21d maps (levels 01, 02, 04, 05)

- Imported with levels 03 and 06 (RLS Vostok and Mamayev Kurgan, live since revision 20) but
  never checked under the native lighting.

### Mamayev Kurgan's battery spawns favour team 0 (live map; the owner: "fix later")

- 25 of its 32 battery spawns are nearer team 0's container: mean walk 86 m against 107 m.
- Worst (team 0 vs team 1): #25 25 vs 102 m, #27 33 vs 99, #13 60 vs 138.
- RLS Vostok is even (98 vs 93 m).
- Accepted as they are (2026-10-17): spawns inside anomaly zones; RLS Vostok #13 slightly in a
  truck bed and #31 0.25 m up; the containers' `position` fields and their swapped labels
  (faithful to 0.21d, unused by the game).

## Weapons

### H&K 416A5 (763) with its EOTech 552 (819) and 5.56x45 ammo (841)

- Ported from 0.31a2 with its own animations. Fringe Settlers, item level 9, 45,000 silver; the
  ammo sold by the Fringe Settlers at rank 4 for 70 / 35. Moved out of revision 17, then not
  picked up again.
- The ammo price rows: skhid-srp branch `feat/weapons-hk416-ammo` (reverted from #146).
- Open: recoil 1.0 (the faction's automatics have 0.6-0.8); never fired, reloaded or aimed
  through the EOTech in a match.
- First, perhaps: ammo prices from the data (the seller of the weapons that fire it, half its
  `base_cost`), so new ammo needs no srp code.

### Candidates

- MSBS Radon (0.31a2, dict 926, `msbs_carbine`): the HK416's 5.56 ammo and reload sounds; its
  generic grenade, melee and recoil states remapped to `weapons/ak/*`.
- From the weapon survey: M4A1 (the HK416's animations), OTs-14 Groza, vz. 58 P, Valmet Rk.62,
  FN SCAR-H, PKP Pecheneg (needs 7N13 ammo), Saiga-12K / Vepr-12, Kedr / AEK-919K.
- Builds up to 0.31a2 use 0.26g's formats; a 0.69d-only weapon needs a skinned-model converter.

## Equipment

### Duty and Freedom exoskeleton follow-ups

- Should owners of the old one-piece body get the separate gloves and boots? A proposal is owed.
- Freedom's armour values are provisional.

### Snow-20 (1817) and Delta 2 (1818) masks

- Built with the other 0.69d masks (resource-porter `feat/face-items-069d`), taken out of #269
  by the owner (2026-10-21). Snow-20: 0.69d 2062, holiday set; its "thermal vision" has no
  gameplay effect in 0.69d (a mask with a lit diode, the emissive untested). Delta 2: 0.69d 1913,
  league premium.
- Never put through the pose checks. Proposed: Snow-20 Renaissance level 4, 25 / 5, 0.6 kg,
  3,300, drop 120; Delta 2 500 gold, no drop, 35 / 5. The owner's: the seller and the price.
- Hard parts (Delta 2's night-vision goggles) are pulled like cloth under helmets.
- The "with glasses" variants 2096-2099: not requested.

### The closed "Keeper" helmet (1820 `edge_helmet_5_closed`)

- resource-porter `feat/keeper-helmet-closed`. v2: visor at 82 degrees, slid 3.5 cm forward and
  0.8 cm down, grille on the eye line, its own texture with the visor's baked shadow lifted,
  the hinge stem rebuilt, a forehead flap.
- Open:
  - the visor sits far in front of the face, the arms stretched (or a tighter fit with the
    grille slightly above the eyes);
  - the lighter patch on the dome has hard edges; a dark wedge above the pivot;
  - keep or drop the flap;
  - face coverage: the mask slot taken with face protection, or masks allowed and clipping;
  - drop rate (two Keeper helmets double its drops);
  - the icon's angle hides the grille (icon cell [14, 6]); the lobby not checked.

### Dropped by the owner

- The STK2 Novice field jacket (1822), 2026-10-19: only the knit cap (1821) ships. Built
  (resource-porter `feat/novice-stk2`); its collar went into the chin.
