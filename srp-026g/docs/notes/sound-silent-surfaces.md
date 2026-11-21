# Silent surfaces: every live map surface that plays nothing for some interaction

The owner, 2026-11-16: "better figure out and find all surfaces which don't make any sound", and
for each, "what they did to them to 0.69". A companion of `sound-gaps.md` (the survey); research
only, nothing built, played or released.

Marks: PROVEN (read in the code or computed from the data), LIKELY, NOT VERIFIED.

## 0. What this covers, and the short answer

- **Live data:** the stock `resources.db` and patches 12..62 of `release/patches.tsv` (revision 62
  is live: step-sounds-1, the dry-leaves retune #2125, wading #2121 and the step contact
  #2128/#2131 are in it). After revision 62 main has only `level06-tunnel-top-1` (Mamayev walker
  boxes, no material change). **#2134** (open, grenade bounces from 0.69d) is applied on top only
  where a column says "#2134"; every other number is the same with or without it. **#2136** (open,
  materials on three 0.69d rooms) is not applied; section 5 says what it would change.
- **Maps:** every playable project of the 21 live maps: the nine stock levels, Vostok and Chemical
  Plant v0.11, Vostok and Mamayev Kurgan v0.21, Rudnya v0.69, and the seven STALKER maps (four
  offered, in Free for All only; Repair Depot, Station and Depository are in the data and not
  offered). No Dirty Bomb map is live (`release/content.tsv` has none).
- **Short answer:** after revision 62 **no ground a player stands on steps silently** except the
  limiter faces of the two old Vostoks (124 m²) and an anomaly on a tree on Mamayev (18 m²). What
  stays silent is everything else a surface does: **bullets** on gratings, moss and dry leaves (5
  materials, 33,300 m² of hittable collision), **grenade bounces** on 26 face materials (23 empty pair rows; 21 materials with #2134), 195,000 m²,
  **gun-stock hits** on 26 materials (every concrete, brick, ground, grass and stone wall), and
  **bodies, dropped weapons and artefacts on every surface** (no code asks for a sound). 0.69d has
  every one of these gaps too, except five grenade materials (the ones #2134 ports) and a handful of
  faces it gave another material (section 5).

**The ten silent surfaces players meet most** (area x the map's play count x how often the
interaction happens: bullets most, then grenades, then the gun stock; section 3 has the method):

| # | surface | silent for | where, m² | 0.69d: what it did |
|---|---|---|---|---|
| 1 | Chemical Plant loading stations' gratings (metal_arm_gran_not_path) | bullet hit, pierce, ricochet (no puff, no hole), grenade, gun stock | Chemical Plant 5,285 | nothing: pair rows empty, faces the same |
| 2 | Cologne's moss (grass_soft) | bullets (puff and hole stay), grenade, gun stock | Cologne 3,841 | nothing: pair rows empty, its re-exported moss still grass_soft |
| 3 | Concrete, brick, ground, grass, asphalt, gravel, stone, glass, plastic... walls and floors | gun stock and battery swing | every map, 2.40 million of 3.12 million shootable | nothing: the same 26 empty rows, no recording for them in either build |
| 4 | Vector's lab and yard dry leaves (foliage_dry) | bullets (no puff, no hole), grenade, gun stock | Vector 1,052 | nothing |
| 5 | Tree trunks and bushes (wood_tree, _walkable, _no_walk) | grenade | Vostok's pines, Mamayev's dead trees, Vector's and the Fort's bushes; 61,022 | nothing (but London `building_03`'s wood -> wood_thin, 576) |
| 6 | Plastic and glass | grenade | London, Vector, Cologne, Chemical Plant; 35,884 | nothing (but Mamayev `ground_3`'s glass -> grass, 1,480) |
| 7 | Cars, tanks, gas cylinders, hard glass, wooden bridges | grenade | London's buses, the T-80s, the Chemical Plant track bed; 9,788 | **fills it**: `weapons/f1_metal_hit` / `f1_surface_hit` (#2134 ports it) |
| 8 | Mud, cloth, water, paper, rubber | grenade | Rudnya's and Chemical Plant's mud, Cologne's tents, the Fort's water; 25,259 | nothing (but Chemical Plant `moss4`'s mud -> grass, 710) |
| 9 | Vostok's chemical bunker gratings, the old Vostoks' fences | bullets, grenade, gun stock | Vostok 237; Vostok v0.11 / v0.21 3,114 / 1,318 | nothing |
| 10 | Mamayev's ground leaves (foliage_dry); its v0.21 port's ground | bullets, grenade, gun stock | Mamayev 385; Mamayev v0.21 3,601 | nothing |

Lower: Chemical Plant v0.11's gratings (13,157 m², 3 matches), Cologne's ICE cars' leaves (205),
the limiters of the old Vostoks (595, also steps), Rudnya's border grass (section 6).

## 1. Who plays a surface sound (PROVEN, the original exe)

`material_pair` (0x306D50) has eight callers; five play a sound.

| interaction | caller | master material | pair field |
|---|---|---|---|
| step: walk/run and soft landing | `step_manager::on_step` 0x248AD0 (landing: 0x23FBE0) | others 10 `actor`, own 42 `actor_local` | `collision_sound` |
| step: sprint and hard landing | same | others 53, own 56 | `collision_sound` |
| step: crouch | same | others 54, own 57 | `collision_sound` |
| bullet hit | `bullet::process_collision` 0x32EBD0 | 7 `bullet` | `collision_sound` (first mark of the bullet) |
| bullet pierce | `bullet::process_front_face_piercing` 0x32E4F0 | 7 | `piercing_sound` |
| bullet ricochet | `bullet::process_reflection` 0x32E970 | 7 | `reflection_sound` |
| (bullet exit) | `bullet::process_back_face_piercing` 0x32E740 | 7 | decal and particle only |
| grenade bounce: RGD-5, smoke grenade | `grenade::on_collision` 0x248980 | 45 `grenade` | `collision_sound` |
| (grenade, trap) | 0x248720 (grenade, LIKELY its landing decal), `booby_trap::on_collision` 0x248160 | 45 | decal only (LIKELY: no sound call): **the trap plays no pair** |
| melee: gun stock, battery swing | the melee bullet through `process_collision` | 74 `melee_blunt` | `collision_sound` |

- There is **no knife** in 0.26g (no knife item, model or config in the live data).
- **Nothing else asks for a pair**: a body or ragdoll landing, a dropped weapon, an artefact or any
  other item makes no surface sound anywhere. The `artefact` master (14) exists with 43 slave rows,
  all empty, in both builds, and no code reads it. 0.69d adds two thrown masters, `nut` (83) and
  `snowball` (86), and no body or drop master.
- **Which material a face plays as** (0x306D50, PROVEN): the material table's entry for the face's
  id; an id with no entry (null) is `default` (11, the manager's +1030Ch); an entry with a
  `base_material` plays as the base, one level only; then a missing pair row (null) falls back to
  (master, `default`). A pair row that **exists with an empty sound** is silent. So:
  `ground_walkable` 58 -> ground, `ground_not_walkable` 65 -> `ground_walkable` -> no row ->
  default; 3FFF (a face whose name the model's settings do not know) -> default; and **the ids 75
  and 77, which 0.26g does not have, -> default** (section 6).

## 2. Every material on live collision, and which interactions are silent

Area per material over all 21 maps (per map: `private/renders/sound-gaps/material-census.csv`).
**Shootable**: faces a bullet meets (cgroup 8, cmask 10h/20h) inside each map's minimap box
(`levelmap_min/max`), background models out. **Walkable**: the reachable standing room (a flood fill
from the spawns, 25 cm cells, 1 m climb, 6 m drop), each cell's step ground by revision 62's rule
(the game's ray 1 m over to 1 m under the foot; on a miss the walker face the capsule rests on;
an unnamed one takes the named hittable surface 10 cm over to 30 cm under it, else `default`).
The walkable column includes tops of invisible walls the fill can reach (Cologne's border boxes,
36,750 m², all 3FFF); no one stands there.

Each interaction cell is "0.26g / 0.69d": yes = a sound, **no** = silent. Steps: "all" = all six
foot materials play. #2134 changes the grenade cell where it says so.

| id | material | plays as | shootable m² | walkable m² | steps 0.26g / 0.69d | bullet hit | pierce | ricochet | grenade | melee |
|---|---|---|---:|---:|---|---|---|---|---|---|
| 9 | ground | ground | 154,537 | 88,650 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | **no** / no |
| 10 | actor | actor | 141 | 18 | **none** / crouch only | yes / yes | yes / yes | yes / yes | **no** / no | yes / yes |
| 11 | default | default | 138,646 | 5,055 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | **no** / no |
| 13 | wood_silent | wood_silent | 105,409 | 7,104 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | yes / yes |
| 15 | body | body | 132 | 15 | all / all | yes / yes | yes / yes | yes / yes | **no** / no | yes / yes |
| 16 | wood_squeak | wood_squeak | 27,151 | 7,702 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | yes / yes |
| 17 | metall_thin | metall_thin | 100,045 | 4,833 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | yes / yes |
| 18 | metall_thick | metall_thick | 389,538 | 20,706 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | yes / yes |
| 19 | concrete | concrete | 911,545 | 86,423 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | **no** / no |
| 20 | asphalt | asphalt | 48,506 | 32,664 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | **no** / no |
| 21 | stone | stone | 36,647 | 8,060 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | **no** / no |
| 22 | gravel | gravel | 69,197 | 44,463 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | **no** / no |
| 23 | mud | mud | 5,519 | 6,169 | all / all | yes / yes | yes / yes | yes / yes | **no** / no | yes / yes |
| 24 | foliage_coniferous | foliage_coniferous | 28,075 | 150 | all / own sprint silent | yes / yes | yes / yes | yes / yes | **no** / no | **no** / no |
| 25 | grass | grass | 323,569 | 257,116 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | **no** / no |
| 26 | ground_forest | ground_forest | 2,348 | 1,501 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | **no** / no |
| 27 | water | water | 2,954 | 1,063 | all / crouch silent | yes / yes | yes / yes | yes / yes | **no** / no | **no** / no |
| 28 | plastic | plastic | 18,286 | 1,459 | all / all | yes / yes | yes / yes | yes / yes | **no** / no | **no** / no |
| 29 | glass | glass | 17,598 | 1,044 | all / all | yes / yes | yes / yes | yes / yes | **no** / no | **no** / no |
| 30 | paper | paper | 3,698 | 558 | all / all | yes / yes | yes / yes | yes / yes | **no** / no | **no** / no |
| 31 | brick | brick | 192,338 | 6,988 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | **no** / no |
| 32 | deep_water | deep_water | 1,546 | 1,435 | all / all | yes / yes | yes / yes | yes / yes | **no** / no | **no** / no |
| 33 | cloth | cloth | 6,746 | 1,033 | all / all | yes / yes | yes / yes | yes / yes | **no** / no | **no** / no |
| 34 | rubber | rubber | 4,406 | 567 | all / all | yes / yes | yes / yes | yes / yes | **no** / no | yes / yes |
| 35 | wood_thin | wood_thin | 12,856 | 689 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | yes / yes |
| 36 | rabitz | rabitz | 30,551 | 493 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | **no** / no |
| 37 | restrictor | restrictor | 595 | 124 | **none** / crouch only | **no** / no | **no** / no | **no** / no | **no** / no | **no** / no |
| 39 | glass_hard | glass_hard | 271 | 8 | all / all | yes / yes | yes / yes | yes / yes | **no**, #2134 yes / yes | **no** / no |
| 40 | metal_arm_gran_pass | metal_arm_gran_pass | 12,574 | 178 | all / walk, sprint silent | **no** / no | **no** / no | **no** / no | **no** / no | **no** / no |
| 41 | metal_arm_gran_not_path | metal_arm_gran_not_path | 10,650 | 124 | all / walk, sprint silent | **no** / no | **no** / no | **no** / no | **no** / no | **no** / no |
| 43 | wood_tree | wood_tree | 55,226 | 3,217 | all / all | yes / yes | yes / yes | yes / yes | **no** / no | yes / yes |
| 44 | foliage_deciduous | foliage_deciduous | 146,524 | 9,022 | all / own sprint silent | yes / yes | yes / yes | yes / yes | yes / yes | **no** / no |
| 46 | metal_car | metal_car | 4,949 | 688 | all / all | yes / yes | yes / yes | yes / yes | **no**, #2134 yes / yes | yes / yes |
| 47 | glass_car | glass_car | 23 | 3 | all / walk, sprint silent | yes / yes | yes / yes | yes / yes | **no** / no | **no** / no |
| 48 | metal_gas_tank | metal_gas_tank | 76 | 7 | all / all | yes / yes | yes / yes | yes / yes | **no**, #2134 yes / yes | yes / yes |
| 49 | foliage_dry | foliage_dry | 5,255 | 2,328 | all / all | **no** / no | **no** / no | **no** / no | **no** / no | **no** / no |
| 50 | wood_bridge | wood_bridge | 254 | 42 | all / all | yes / yes | yes / yes | yes / yes | **no**, #2134 yes / yes | yes / yes |
| 51 | grass_soft | grass_soft | 4,194 | 2,980 | all / all | **no** / no | **no** / no | **no** / no | **no** / no | **no** / no |
| 55 | metall_tank | metall_tank | 4,238 | 647 | all / all | yes / yes | yes / yes | yes / yes | **no**, #2134 yes / yes | yes / yes |
| 58 | ground_walkable | ground | 20,272 | 16,890 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | **no** / no |
| 60 | grass_walkable | grass | 57,429 | 50,849 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | **no** / no |
| 61 | mud_walkable | mud | 1,936 | 1,689 | all / all | yes / yes | yes / yes | yes / yes | **no** / no | yes / yes |
| 62 | gravel_walkable | gravel | 17,171 | 13,448 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | **no** / no |
| 63 | schiefer | schiefer | 6,794 | 767 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | **no** / no |
| 65 | ground_not_walkable | default | 24,708 | 1 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | **no** / no |
| 66 | brick_walkable | brick | 2,617 | 370 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | **no** / no |
| 67 | concrete_walkable | concrete | 1,730 | 797 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | **no** / no |
| 68 | wood_tree_walkable | wood_tree | 1,245 | 421 | all / all | yes / yes | yes / yes | yes / yes | **no** / no | yes / yes |
| 70 | wood_tree_no_walk | wood_tree | 4,551 | 41 | all / all | yes / yes | yes / yes | yes / yes | **no** / no | yes / yes |
| 71 | metall_thick_walkable | metall_thick | 1,260 | 141 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | yes / yes |
| 72 | metall_thin_walkable | metall_thin | 1,944 | 272 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | yes / yes |
| 73 | metall_thick_not_walkable | metall_thick | 1,306 | 147 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | yes / yes |
| 75 | (0.69d's `grass_walkable_nowalk`) | default | 1,653 | 138 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | **no** / no |
| 77 | (0.69d's `sand`) | default | 15,851 | 6,495 | all / all | yes / **no** (sand) | yes / no | yes / no | yes / no | **no** / no |
| 3FFF | no material | default | 81,891 | 87,166 | all / all | yes / yes | yes / yes | yes / yes | yes / yes | **no** / no |

Notes on the table:

- 0.69d columns compare the pair rows (same ids in both builds). The grenade column's 0.69d "yes"
  is its `weapons/f1_surface_hit` / `f1_metal_hit` composites (#2134 takes all 21 rows: 16 replace
  0.26g's `f1_hit_*`, 5 are new). For 75/77 the 0.69d column is 0.69d's own material (London's
  terrain has the same faces there, section 6).
- **No material (3FFF), what it resolves to:** `default` for everything: steps play the plain dirt
  step, bullets `ground_hits` with the dirt puff, grenades the ground bounce; the gun stock is
  silent. Its 87,166 m² of walkable: 28,948 under the game's ray (hittable faces without a material:
  Vostok's garage and arsenal, London's `building_07_3`, Mamayev's monument), 21,455 on walker faces
  with nothing named near (Chemical Plant's `central_factory` 3,331 and `level_border` 1,780, the
  two v0.11 maps' borders, the bunkers of the Vostoks 665 each, School's `school_basement` 664, the
  Fort's `base_a` 553), 36,750 on editor walker boxes (Cologne's border).
- The STALKER maps' faces are mostly `default` (11) itself: their X-Ray materials that the port
  could not map (Military Warehouses 43,095 m², Depository 44,830, Repair Depot 21,334 shootable):
  every interaction plays the dirt sound but the gun stock.

## 3. The silent cells, where they are, and what 0.69d did

The full list (6,062 cells: map, model, material, interaction, area, and the 0.69d column) is
`private/renders/sound-gaps/silent-surfaces.csv`. Below, by interaction. "Matches" is the map's
count in the replay index (`~/games/skhid-replays/index.tsv`, 160 PvP matches of revisions 53..56,
two players: the only play counts kept locally; it predates Free for All and the STALKER maps, which
are given 1). The CSV's `exposure` is area x the map's share of those matches x a weight for how
often the interaction happens (step 1, crouch 0.3; bullet hit 1, pierce 0.3, ricochet 0.2;
grenade 0.05; gun stock 0.01): a ranking aid, not a measurement. Vostok is `level_03` (the
survey's Radar).

### 3.1 Bullets: 5 materials, 33,300 m² (hit, pierce and ricochet alike)

The same five pair rows are empty in 0.69d (`bullet` x restrictor, both gratings, foliage_dry,
grass_soft), and its `process_collision` (0x2002A5930) has the same three tests (sound only on the
first mark, byte +80h 0, count +84h 0). grass_soft keeps a puff and a hole; the other four have
neither (the bullet vanishes without a trace).

| map (matches) | model | material | shootable m² | 0.69d: what it did (the pair row is empty in 0.69d for all) |
|---|---|---|---:|---|
| Chemical Plant (13) | `level/04/loading_station_01.model` | metal_arm_gran_not_path | 2,999 | same material |
| Chemical Plant (13) | `level/04/loading_station_02.model` | metal_arm_gran_not_path | 2,286 | same material |
| Cologne (23) | `level/08/moss/mossmoss_01..06.model` | grass_soft | 3,841 | grass_soft (0.69d re-exported the moss: `coll_grass_soft` grass_soft) |
| Vector (29) | `level/07/centre_lab.model` | foliage_dry | 709 | same material |
| Vector (29) | `level/07/base_v_outdoor.model`, `centre_lab_out.model` | foliage_dry | 343 | same material |
| Cologne (23) | `level/08/ice3_train/ice3_*.model` (the ICE cars' leaves) | foliage_dry | 205 | same material |
| Mamayev Kurgan (4) | `level/06_v1/...` ground | foliage_dry | 385 | same material |
| Vostok (19) | `level/03/bunker_chemical.model` | metal_arm_gran_not_path | 237 | same material |
| Chemical Plant v0.11 (3) | `ported/v011e/level/04/loading_station_01/02`, `pipeline_part_06`, `admin_building`, `fences` | gratings 40/41 | 13,157 | not a 0.69d model; pair empty there |
| Vostok v0.11 (3), v0.21 (3) | `ported/.../level/03/fence_outer_part2..5`, `bunker_chemical` | gratings 40/41 | 3,114 / 1,318 | same |
| Mamayev Kurgan v0.21 (2) | `ported/v021d/level/06/terain1`, `object_1`, `ground_1_*` | foliage_dry | 3,601 | same |
| Vostok v0.11 / v0.21 (3) | `ported/.../level/03/blocks`, `garage` | restrictor | 312 / 283 | same |
| Rudnya v0.69 (2) | `v069d_rudnya/level/01/terrain/terrain_game_08` | grass_soft | 320 | the donor's own |
| School (19) | `level/02/anomaly_fruit.model` | grass_soft | 33 | **0.69d: grass** (bullets play the grass hit) |

**0.69d for every bullet gap: the same gap** (data and code), but for School's anomaly fruit, whose
face 0.69d made `grass`.

### 3.2 Steps: two materials, 142 m²

| map (matches) | model | material | walkable m² | 0.69d |
|---|---|---|---:|---|
| Vostok v0.11 / v0.21 (3) | `ported/.../level/03/blocks`, `garage` | restrictor | 66 / 58 | pair empty for walk and sprint too; 0.69d gives crouch `step/crouch` (crouch_wear vk 0.43 + crouch_step vk 0.31, spl 60) |
| Mamayev Kurgan (4) | `level/06_v1/anomaly_06_ontree1`, `anomaly_root_01` | actor | 18 | same |

Whether a player can stand there is LIKELY not (limiter tops and an anomaly's shape; the fill
reaches them). Everything else a body stands on plays a step for all six foot materials and both
landings. Two places still step without sound, neither a material: in the air (the sweep touches
nothing) and walking down stairs where the step-down sweep has no contact (walk 4, 6 of 137 steps,
`footsteps.md`). 0.69d is silent in both too (its controller ground is the same sweep).
**0.69d's step code** differs only by what revision 62 already copies (the controller ground on a
ray miss, the sound at the foot); its step *data* differs a lot (section 4).

### 3.3 Grenade bounces (RGD-5, smoke grenade): 26 materials (23 pair rows), 21 with #2134

195,140 m² of shootable collision is silent for a bounce; 185,352 m² with #2134.

The pair rows below are empty in 0.69d too unless said; the bounce handler's code was not compared
with 0.69d's (NOT VERIFIED; #2134 is data only).

| material | shootable m² | where (largest, m²) | 0.69d |
|---|---:|---|---|
| wood_tree, _walkable, _no_walk | 61,022 | Mamayev v0.21's dead trees 7,952; Vostok's pines 1,789+1,303+865; Vector's `flora/vine/bush_small` 2,906 and the Fort's 1,337 (wood_tree_no_walk) | same gap; `bush_small`: 0.69d's settings drop its `default` face (NOT VERIFIED, section 5); London `building_03`'s `coll_wood` wood_tree -> **wood_thin** (576 m²: the bounce plays) |
| foliage_coniferous | 28,075 | Vostok v0.11's pines 8,868+5,364+4,317+3,328 | same |
| plastic | 18,286 | London `building_05_1` 1,898, `gherkin_floor_*` 2,786; Vector `base_a` 1,809, `base_v_lab_1` 828; Cologne `favela_2/3` 1,584, ICE cars 837 | same |
| glass | 17,598 | London `building_05_2` 1,988; **Mamayev `ground_3` 1,480** (a face named `grass` given glass, section 6); Chemical Plant `central_factory` 1,084 | same; Mamayev `ground_3`'s face is **grass** in 0.69d (the ground bounce) |
| metal_arm_gran_pass / _not_path | 23,224 | Chemical Plant stations 5,285; its v0.11 port 13,157 | same |
| cloth | 6,746 | Cologne `bridge_hospital` 678, tents 333; London indoor 319 | same |
| mud, mud_walkable | 7,455 | Rudnya `terrain` 1,600; Chemical Plant `moss3/4` 1,299; Vostok `terrain_all` 921 | same pair; **Chemical Plant `moss4`'s `anomaly_moss_base_sg` is grass in 0.69d** (710 m²); 0.69d's Vostok terrain is one `coll_ground_s` ground mesh (re-exported) |
| foliage_dry | 5,255 | as bullets | same |
| rubber | 4,406 | Rudnya and Rudnya v0.69 `atp_awning` 530 each, School `boilernaya` 281 | same |
| grass_soft | 4,194 | Cologne's moss | same |
| paper | 3,698 | Rudnya `culture_center_top` 669 (both Rudnyas), Vector 303 | same |
| water | 2,954 | Mamayev v0.21 `object_3` 1,063; the Fort's water 599 | same |
| deep_water | 1,546 | Rudnya v0.69's pond 1,546 | same |
| restrictor, actor, body, glass_car | 891 | old Vostoks' limiters, Mamayev's anomalies, cars' windows | same |
| **metal_car** | 4,949 | London's buses and cars, Rudnya's UAZ | **0.69d fills it**: `weapons/f1_metal_hit` (composite: `grenade_hit_metal_close` collection, 15 samples, vk 0.8, spl 60, 0-15 m + `grenade_hit_metal_distant`, 10 samples, vk 0.8, spl 60, 15-25 m; preset 0.69d's `vss_main`). **#2134 ports it** |
| **metall_tank** | 4,238 | Chemical Plant `railroad.model` 683, the T-80s 417 a map | **0.69d fills it**: `f1_metal_hit`. #2134 |
| **glass_hard** | 271 | School `greenhouse` 198 | **0.69d fills it**: `weapons/f1_surface_hit` (`greande_hit_close`, 33 samples, vk 0.5, spl 60, 0-5 m + `grenade_hit_distant`, 10 samples, vk 1.0, spl 60, 5-25 m). #2134 |
| **wood_bridge** | 254 | the Fort's bridges, Cologne `hoist` | **0.69d fills it**: `f1_surface_hit`. #2134 |
| **metal_gas_tank** | 76 | gas and oxygen cylinders | **0.69d fills it**: `f1_metal_hit`. #2134 |

0.26g's own bounces (for comparison): `weapons/f1/f1_hit_ground` / `_wood` / `_metal` collections,
3 samples each, vk 0.5 / 0.4 / 0.3 (grenade-bounce-1), spl 20, preset `material_hit`.

### 3.4 Gun stock and battery swing (melee_blunt 74): 26 materials

Silent on ground, default (and so 3FFF, 65, 75, 77), concrete, asphalt, stone, gravel, grass,
ground_forest, brick, schiefer, rabitz, glass, glass_hard, glass_car, plastic, paper, cloth, water,
deep_water, foliage (all three), grass_soft, both gratings, restrictor: 2.40 million m² of the
3.12 million m² shootable, every wall of concrete or brick (London `building_03` 45,038 m², Chemical
Plant `admin_building` 41,345, ...). It plays only on body/actor, wood (4), metal (5 incl. car,
gas tank, tank), mud and rubber. **0.69d: the same 26 empty rows, and no melee recording for
stone, concrete or ground exists in either build** (`sounds/single/material/melee_hit/`: body,
car metal, gas tank, metal thick and thin, mud, rubber, wood). 0.69d only drops the melee dust
particles (#2134's note).

### 3.5 Bodies, ragdolls, dropped weapons, artefacts

Silent on every surface in 0.26g (no caller, section 1). 0.69d: no pair master for them either
(its 12 masters are 0.26g's 10 plus `nut` and `snowball`); its code for a body or drop landing was
not read (NOT VERIFIED). The dying player's `death_animation` composite (fall + scream) is
surface-independent in both.

## 4. The subtler silences

### 4.1 The mixer: what can drop a pair that exists

- **`spl_value` drops nothing (PROVEN for the sound module).** `calculate_hdr_audio_frame`
  (0x5C7A0) writes each propagator's HDR level (+3Ch, +40h: from the `.spl` preset's `spl` curve,
  or `spl_value` - 20 log10 d) and the scene's window top (+300h), and nothing in 0x58000..0x64000
  reads them back. The voice is made while the distance gain (+4Ch, the `volume` curve, or 2/d) is
  over 0.01 (0x5CA87) and let go under it when `can_be_displaced` (0x5CACD). So a pair is never
  dropped for a low `spl_value`. This is what `weapon-reverb-layers.md` says too.
- **What does drop a sound:** the 64 voices. Patch 96 gives a refused sound the voice of the
  quietest displaceable one under half its loudness (gain x the sample's amplitude). The quietest
  own steps are the first to go under fire, and with a quiet single layer they are also hard to
  hear at all:

| own step (42 walk / 56 sprint / 57 crouch) on | 0.26g: sound, layers, peak x vk | where (walkable) | 0.69d |
|---|---|---|---|
| body, cloth, glass_hard | `material/default/step/default_steps` (glass_hard `material/glass/step/default_steps`): one layer, vk 0.02, spl 10, not displaceable: walk -35.2 dBFS, crouch -43.2 (own walk on concrete -27.7, metal -23.9) | cloth 1,033 m² (London indoors 500, School 22), body 15, glass_hard 8 | `step/run/default` (default_steps vk 0.06 + `run_wear` vk 0.15, spl 60): -25.0; crouch `step/crouch` (crouch_wear 0.43 + crouch_step 0.31): -21.8 |
| ground_forest | `material/grass/step/grass_step`, vk 0.03: -30.7; crouch -38.6 | Mamayev `terain1` 1,480 | `step/run/default` -25.0 |
| mud, mud_walkable (the goo) | `material/mud/step/mud_steps`, vk 0.1, spl 15, displaceable: -20.4; sprint the same (step-sounds-1); crouch -28.4 | Rudnya `terrain` 1,454, Cologne `back_wood_01/02` 2,263, Chemical Plant `moss3/4` 1,060, Vostok terrain 1,662 | `step/run/mud` (mud_steps vk 0.33 spl 20 + run_wear vk 0.15 spl 60): -20.4; sprint `step/sprint/mud_sprint` (steps_walk_sprint 0.4 + mud_steps_sprint 0.2): -12.8; crouch `step/mud_crouch` -17.4 |
| water, deep_water | single layers, vk 0.1 / 0.35 | the Fort, Mamayev v0.21, Rudnya v0.69's pond 1,435 | `step/run/water` (water_steps vk 0.9 + run_wear): -15.9; deep_water the same as 0.26g |
| foliage_coniferous / deciduous | single layers, vk 0.2 / 0.1 | 150 / 9,022 m² (STALKER maps' bushes) | the same rows (0.69d did not change them; own sprint empty there) |

  These are the 0.26g own-step pairs with no gear layer (`material/steps_walk`,
  `steps_walk_sprint`, `sprint_clothes`). **The goo (mud) is silent for another reason, PROVEN
  later** (`sound-gaps.md` 1.9): mud's own-step samples are the only mono ones, and a mono sample
  on a hud (first-person) sound gets a low-pass at frequency 0 (`notify_listener2`
  0x5C666..0x5C6AE). So own steps on mud are silent on every map; the others in this table play.
- **Not displaceable** (they never lose a voice): 0.26g's walk and crouch layers of default,
  concrete, stone, grass, gravel, foliage_coniferous, water, body, cloth, glass_hard; the metal,
  mud, rubber and squeaky-wood impacts (`sound_voices.md`).

### 4.2 Missing or empty files (PROVEN: 12 interactions x 43 slaves, both builds)

Every sound a live or 0.69d pair names resolves: no missing composite or collection, no collection
with zero items, every sample has its `.raw_options` and an `.ogg`, none under -80 dBFS or shorter
than 20 ms. Over all 1,035 live collections and composites: none with zero items; five name samples
without a `.raw_options` (they play with the defaults: vk 1, spl 60, displaceable, preset
`default`), none a surface's: `weapons/ak_12/ak_12_set_ready` (1), `weapons/svd/svd_shutter_out`
(3), `weapons/environments/level_01/automatic/outdoor/rudnya_outdoor_one_shot` (5),
`weapons/vssk` composite's `layers/vssk_bass`.

## 5. What 0.69d did, gap by gap (the owner's rule: change only where 0.69d does)

- **Pair rows:** of all the silent cells above, 0.69d fills exactly five: grenade x metal_car,
  metall_tank, glass_hard, wood_bridge, metal_gas_tank (#2134 ports them, with the other 16 grenade
  rows). Every bullet, melee, artefact and step gap row is empty in 0.69d too. For steps 0.69d
  is *more* silent than revision 62: gratings and car glass for walk and sprint, own sprint on
  foliage, crouch on water (revision 62 plays them).
- **Faces 0.69d gave another material** (its models' settings against live 0.26g's, same face
  names; the ones that touch a gap here):
  - School `anomaly_fruit` grass_soft -> grass (bullets play): 33 m²;
  - Mamayev `ground_3` glass -> grass: 1,480 m² shootable, 1,000 walkable (0.26g plays glass for
    steps and bullets there, nothing for grenades);
  - Chemical Plant `moss4` mud -> grass: 710 m² (grenade plays, gun stock goes silent);
  - London `building_03` wood_tree -> wood_thin: 576 m² (grenade plays);
  - Vector/Fort `flora/vine/bush_small`: 0.26g's settings give its face `default` wood_tree_no_walk
    (no grenade sound); 0.69d's settings have no `default` entry (an unnamed face would play
    `default`, the ground bounce): NOT VERIFIED which faces its collision has;
  - #2136's rooms: Vostok `garage_inside` (5,456 m² of 3FFF -> metall_thick, concrete, gravel,
    metall_thin, wood_silent), the Fort `center_part_01_floor` (ground, gravel), Chemical Plant
    `perehod*` walker faces -> concrete_walkable, `antenna_base` -> wood_thin, the ventilation
    `podvesnoy_low*` -> plastic, metall_thick: all from 3FFF (dirt) to a named sound, none from
    silence except the gun stock on their metal and wood faces.
  - Others it changed that touch no gap: the railroad rails metall_thick_walkable -> metall_tank
    (the owner kept 0.26g's, #2136), Chemical Plant pipelines and tanks metall_thin/thick ->
    metall_tank, the Fort's uranium crystals stone -> crystal (a 0.69d material), Cologne's
    `db_423` metal_*_walkable -> metall_thick, sandbags' walker ground_walkable -> ground.
- **Code:** bullets the same; melee the same (a bullet); steps: the controller ground and the
  sound at the foot (revision 62 has both), the gear layer and per-room layers are data. Grenade
  bounce code not compared.

## 6. Against the earlier survey (`sound-gaps.md`)

- **Rudnya's silent grass is the background, not the field.** The survey (#5, 2) put 6,087 m² of
  grass_soft on Rudnya's terrain ("a shot into the grass of Rudnya makes no sound"). It is the face
  `grass_back_s`: 0 m² inside the minimap box, 594 m² within 25 m beyond it, 5,493 m² 25 to 100 m
  beyond. Rudnya v0.69: 320 m² inside (`terrain_game_08`), 3,968 within 25 m (2,858 of it
  `terrain_game_*`), the rest farther out, nearly all `terrain_bg_*`. The big silent bullet surfaces in play are the Chemical Plant's gratings and
  Cologne's moss. Likewise Mamayev's foliage_dry is 385 m² inside the box (the survey's 1,608
  counts the outside).
- **0.69d-only material ids are on a live map.** The survey: "no live 0.26g face uses them".
  London's terrain (`level/09/terrain/terrain_all*`, base patch 14) has faces of id 77 (0.69d's
  `sand`: 15,851 m² shootable, 6,495 walkable) and 75 (`grass_walkable_nowalk`, 1,653). 0.26g has
  no such material; `material_pair` plays them as `default` (dirt), PROVEN (0x306D5D). In 0.69d
  sand steps as `default` and **its bullet, grenade and melee rows are empty**: on London's sand
  0.26g plays a hit where 0.69d is silent.
- **Mamayev `ground_3` is glass** (a face named `grass`), 1,480 m² shootable: it plays glass for
  steps and bullets. 0.69d made it grass. Not in the survey.
- **Steps (survey #2, #3):** closed by revision 62; no step pair is empty but `actor` and
  `restrictor`, and the census finds no reachable walker-only floor without a ground.
- **The goo's cause:** not `spl_value` 15 and displacement (the survey's follow-up): a mono sample
  on a hud sound is low-passed to silence (`sound-gaps.md` 1.9).
- Confirmed: the five empty bullet rows and that 0.69d has them too (3.3 of the survey); the 23
  empty grenade rows and 0.69d's five; the 26 empty melee rows; no body or drop pair.

## 7. How it was made (reproducible)

Scratch scripts (not kept in the repo; the session scratch `ss/py/`): `pairs.py` (every master x
slave row of live `material.pairs` (patch 62) and 0.69d's, each sound resolved to its samples:
layers, `volume_k`, `spl_value`, `can_be_displaced`, peak by ffmpeg `volumedetect`), `census.py`
over `level.py` of `private/ladder-work/step-sounds-build/census` (each project's collision placed,
the materials through the models' `settings`, the reachable standing room and revision 62's step
ground), `faces69.py` (each gap model's `settings` in 0.26g and 0.69d), `report.py`/`final.py`
(the tables and the two CSVs). Projects: `day_4` for the stock, v0.11, v0.21 and v0.69 maps and
the three STALKER maps not offered; `day_8` (Free for All's own copy) for the four offered STALKER
maps. Loudness is "peak x vk" as in the survey (dBFS at 1 m, room layers out).
