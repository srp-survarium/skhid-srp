# STALKER maps (team deathmatch)

Six STALKER multiplayer maps as one content pack, `stalker-maps@4`, the owner's
pick of 2026-10-24 on the map spot planner. Team deathmatch only (mode 4).
Where the work stands and how to continue: `stalker-maps-handoff.md`.
FFA points and mode-8 configs for Pool, Military Warehouses and Training Camp:
`stalker-ffa.md` (depends on the current FFA runtime; not released).

## Free for All only, for now (the owner, 2026-11-12)

"New stalker maps should only have FFA", "no TDM yet": players get Free for All
on Experimental Pool, Military Warehouses and Training Camp (maps_dict 42-44)
and no Team Deathmatch on any of the six. `content-edit/stalker-tdm-hold-1.json`
takes the six TDM rows (210-215) out of `maps_dict`, so neither srp nor the game
has them as maps: no room pool or create window, vote card, Play, clan match,
Player vs Bots, room with bots or Maps tab can list them (each lists the
dictionary's playable rows; a room saved with them is fitted without them).
Repair Depot, Station and Depository have no FFA and are not played.

Everything else stays: the six TDM configs, the projects with both teams'
spawns and stashes, their `modes.tsv`/`maps.tsv` rows, names, pictures and
minimaps; the FFA configs are copies on the same projects. Ids 210-215 stay
these configs' and are given to nothing else.

**To offer Team Deathmatch on them later:** delete `stalker-tdm-hold-1.json` and
its row of `release/content.tsv`. The list must stay the last that edits
`maps_dict` (a list that appends rows goes above it). Before that:

- The six maps need their top-down renders for the Maps tab
  (`scripts/damage-calc/assets/maps`), or `release/build-site` stops.
- Training Camp's Team Deathmatch project (`day_4`) has no walker wall:
  `training-camp-arena-1.json` walls Free for All's own `day_8` only, at the
  base's fence (`stalker-ffa.md`). Its teams' spawns are at the north and
  south gateways and its west stash pair (team 0's `stash2`, team 1's
  `stash5`, at about (-20.0, -9.6) and (-20.8, -9.8)) is 11 m outside the
  fence: Team Deathmatch there needs its own boundary first.
- Bots: no bots' map data for these maps (`release/bot-data.tsv`).

| maps_dict id | level (`map_id`) | project / HUD key | map | source | loading id |
| ---: | ---: | --- | --- | --- | ---: |
| 210 | 14 | `cs_mp_bath` | Experimental Pool · Бассейн | Clear Sky `mp_bath` | 30 |
| 211 | 15 | `cs_mp_military_1` | Military Warehouses | Clear Sky `mp_military_1` | 31 |
| 212 | 16 | `cs_training_camp` | Training Camp | Clear Sky `mp_training_camp` + summer meadow | 32 |
| 213 | 17 | `cs_mp_rembasa` | Repair Depot | Clear Sky `mp_rembasa` | 33 |
| 214 | 18 | `soc_ug_station` | Station | community SoC map `mp_{UG}station` | 34 |
| 215 | 19 | `soc_ug_depository` | Depository | community SoC map `mp_{UG}depository` | 35 |

- Ids: 210-215 are free in the live data (stock, patches, research-mode@3's
  145-149 and 184-203, the v0.11e maps' 206-209); the earlier local package's
  197-201 are Research's live rows, so these maps do not use them.
- Levels 14-19: the lobby names them (`RoomsText.LEVELS`, `BottomPanel`) and draws
  their room tiles with their loading picture (`RoomsLook.RENDER_ONE`).
- Names: English from the map; the other languages are translations to check.
  The game's texts are Windows-1251, so a German or Polish name with letters
  outside it takes the English one in the localization (the lobby's own tables
  keep them).
- Clear Sky's versions are the most complete: mp_bath 255,461 collision faces and
  145 rpoints with TDM teams (SoC: 228,998 and 15; CoP: 198,348 and 149);
  mp_military_1 497,056 and 170 (SoC: 383,577 and 103; a CoP version exists only
  in an untrusted patch archive). Station and Depository are community SoC 1.0006
  maps (`.xdb0`, xrLC 14, DM/TDM/AH); neither the archives nor the level files
  name an author or credit.

## Spawns and ammo stashes

From the map spot planner (its `spots` list, Claude's proposals the owner has
not edited yet): 12 spawns a team, and per team one safe stash and one in each
contested middle area. A spawn is registered for its own team only; a stash is
the native level_03 team-deathmatch `resupply_zone` of that team (the zone, its
collision box, the ammo decal and the `item/quest/container_closed` container)
moved so the container stands on the planned spot. The owner edits the spots on
the planner; the pack is then built again from the new list.

Every spot stands on the converted collision (checked in the built maps against
the floor under each one) and on ground a walker reaches without a ladder. That
check moved one Repair Depot stash 0.7 m onto solid floor and dropped one
Training Camp pair (its floor under the tower is not walkable in the build); the
planner has the same.

### Experimental Pool: the owner's rules (2026-10-26)

Experimental Pool no longer uses the planner's proposal. Its spots in
`ports/stalker-tdm/spots.json` follow these rules:

- Every spawn is on the second floor, 12 per team. Team 1 (server team 0) has 5
  in the south-west block and 7 in the south block (y 8.6-9.0). Team 2 (server
  team 1) has 6 in the north-west block and 6 in the north block (y 8.5).
- No spawn sees an enemy spawn. Each spawn is at least 0.9 m from a wall, has
  at least 3.8 m of headroom and is indoors. The enemy's second floor is at
  least 25 m away on foot.
- Roof sight lines were kept as few as the blocks allow. The south block's
  north windows face the north roofs across the pool, so 9 of team 1's spawns
  are in sight of some roof, about 320 m² of roof in all. Only 1 of team 2's
  is, from 9 m² of roof.
- Each team has one stash, outside on the grass behind its spawns:
  - Team 1's is south of the south blocks at (-14.88, 4.26, -13.62), 32 m on
    average from its spawns.
  - Team 2's is north of the yard at (-22.88, 4.18, 48.88), 81 m on average.
    The north blocks have no way out to the north except through the sunken
    passage, so team 2 walks about 50 m further. A stair from the north
    gallery down to the yard would even this out; it is not built.
- The hatch room under ladder clmbl#8 (south block, x -12.3..-4.5,
  z -8.7..-4.5) has no spawns. It holds team 1's way up to the roof. Team 2
  already has one: the table and planks in the north block's broken east
  end.

### Experimental Pool: team 1's way to the roof (the owner's pick, 2026-10-27)

The roof over the east half of the hatch room has caved in. The geometry is
listed under `cs-mp_bath` in resource-porter's `ports/stalker-tdm/maps.json`
(commit 823b2fe), and `make_spec.py` passes it to the importer.

- Five cut boxes take the roof slab out (y 13.14-13.62 only, so the walls
  stay). Together they give the hole a ragged edge.
- On and over the rim: broken slabs with rebar, and a roof slab hanging into
  the hole.
- On the floor: rubble round the ladder foot, broken slabs and a fallen slab
  leaning on the rubble.
- A 1.2 m plank ramp (28.7 deg) runs from a fallen slab (0.53 m up) over the
  wreckage onto the roof.
- Ladder clmbl#8 stays in the opening.
- Every piece is a native 0.26g model (`props/garbage/concrete_wreck*`,
  `concrete_wrecks`, `props/cover/concrete_tray_01`) with its own collision
  and walker collision. A tilted piece takes the prop's new `rotation`.
- Checked on the built map:
  - both doors reach the roof with a 0.6 m step, 50 deg and 1.8 m headroom;
  - no spot in the room or on its roof is a dead end;
  - the room's doors stay clear.

## The old builds' post-process (the owner, 2026-11-13/14)

The environments of the maps ported from v0.10b and v0.11e (these six, Vostok
v0.11, Chemical Plant v0.11) keep their source build's post-process values,
which 0.26g draws its own way. Three edit lists set seven of them, in every
mode's copy of the projects they name:

| value | the ports | stock day maps | now | list |
| --- | --- | --- | --- | --- |
| `image_grain_intensity`, `image_grain_update_frequency` | 0.08, 2 | 0.02, 1 | 0.02, 1 on all 16 projects | `ported-maps-grain-1.json` |
| `god_rays_intensity`, `god_rays_attenuation` | 1.0, 1.0 (the six STALKER maps) | 0.65, 2.6 (level_03) | 0.2, 3.0 on Military Warehouses, Experimental Pool and Training Camp | `stalker-maps-sun-1.json` |
| `lens_flares_multiplier` | 2 (the six) | 1 | 0.7 on the same three | the same |
| `ao_saturation` | 2 (the six) | 1 to 1.29 (level_03: 1) | 1 on the six's nine projects | `stalker-maps-ao-1.json` |

- Grain ("always noisy in the distance", "on almost all ported maps"): 0.26g
  adds it last, noise x colour^0.6 x intensity (`aberration.ps`), so it shows on
  the sky and the haze. The v0.21d and v0.69d ports have stock grain. Seen in
  the game (2026-11-14, captured frames before and after, the owner's
  settings): the sky's grain is stock's.
- The sun (on Military Warehouses: "you cannot look at that direction at
  all"; on Experimental Pool and Training Camp, the same weather: "fix them
  too"): a ray's strength is intensity x (1 - screen distance to the
  sun)^attenuation x the far pixels (`t_position.x` of 300 or more: the sky)
  between the pixel and the sun (`god_rays.ps`, `god_rays_mask.ps`); times the
  ray colour, it is added before the tone curve. Half a screen height above or
  below an open sun that is 0.23 with the ports' values, 0.05 on stock
  level_03 and 0.01 on Vostok v0.11, which has this same day weather (sun 38.0
  degrees up, azimuth 219.7, intensity 1.29) with rays 0.2 at 3.0 and flares
  x0.7. The three maps have Vostok v0.11's three values, each in its Team
  Deathmatch project and Free for All's copy. Seen in the game on Military
  Warehouses (2026-11-14, captured frames): toward the sun the rays washed
  the frame orange; with these values the sun has a halo and the sky and the
  buildings stay visible. Experimental Pool and Training Camp were not
  captured.
- Ambient occlusion ("on fence there are black stripes", "often those happen
  for some reason with this engine"; gone with `r_ambient_occlusion_quality 0`
  in the owner's game): the stripes are the occlusion's own sample pattern on
  a far flat wall, screen-aligned and about 6 pixels apart (Military
  Warehouses' east fence from (59.8, -8.9, 1.7): solid, facing away from the
  sun, no such lines in its textures). `ao_saturation` is an exponent in both
  occlusion shaders: (1 - occluded)^(2 x `ao_saturation`) x 1.1 at
  `r_ambient_occlusion_quality` 1 (`ssao_hemisphere_optimized.ps`), (1 -
  occluded)^(3 x `ao_saturation`) x 1.2 at 2 and 3
  (`ssao_hemisphere_stable_radius.ps`). The ports had 2, stock maps have 1 to
  1.29: at quality 1 an occlusion of 0.2 left 0.45 of the ambient light where
  level_03 leaves 0.70. The six maps have 1 now; the pattern itself is the
  engine's and stays at stock strength. Not proven in the game: a still
  captured frame shows the stripes neither before nor after (the occlusion's
  temporal filter settles), so whether stock strength hides them in play is
  the owner's to see. Their `ao_radius` 1.5 is a stock value
  (level_02 day; stock maps have 1 to 1.5) and stays. Vostok v0.11 (1.5) and
  Chemical Plant v0.11 (1.3) are above stock too and are not changed.
- Not changed: Repair Depot, Station and Depository, not played, keep rays 1.0
  at 1.0 and flares x2. All eight maps keep the old builds' vignette
  (`vignette_effect_power` 1, stock 0) and far depth of field
  (`use_bokeh_dof`, focus 100 m, region 50 m; stock day maps do not use it).

## Grass

Experimental Pool and Military Warehouses have the grass of their Clear Sky
levels (`level.details`), converted by the porter's recipe option
`detail_objects` (resource-porter `docs/soc-map-porting.md`, "Grass: detail
objects"). It is X-Ray's own grass: its models, its `build_details` texture,
and the positions X-Ray draws, at the same density and spread for each model,
on the collision floor. It is drawn by 0.26g's grass system (16 m patches,
`r_grass_lod1/2_distance`, the 100 m cull, `r_decorations_quality`), like
Training Camp's meadow.

| map | tufts | models | triangles LOD 0 / 1 | patches (draws per pass) | biggest patch |
| --- | ---: | ---: | --- | ---: | ---: |
| Experimental Pool | 125,844 | 13 | 1.08 M / 0.61 M | 263 | 809 tufts, 14,564 vertices |
| Military Warehouses | 77,721 | 14 | 0.61 M / 0.36 M | 295 | 536 tufts, 9,233 vertices |
| Training Camp (meadow) | 50,283 | 9 | 0.50 M / 0.10 M | 120 | 1,084 tufts, 32,520 vertices |
| native level_03 (most grass) | 90,879 | | | | |

- About 23,000 and 22,000 of them are within 20 m of the spots; the rest
  cover the levels' outer hills, which the grass system culls past 100 m.
- Normals are X-Ray's own detail normals (outward from the tuft's foot and
  mostly up); the cards' face normals left the tufts black under a high sun.
- Not carried over: X-Ray's per-slot sun, sky and colour terms (0.26g's grass
  has one colour per instance, fixed in the engine); 0.26g's own shadows
  light it.
- Matte (the owner, 2026-11-14: "grass which is too glossy";
  `stalker-grass-matte-1.json`): the two maps' grass material
  (`ported/cs/<map>/details`) is stock `flora/grass_l03_alt` with the grass's
  colour texture as its roughness map, `constant_roughness_min` 0.3, max 1.
  A material's "roughness" is its gloss in 0.26g (stock glass 0.7 to 1, cloth
  and skin 0.1, `flora/grass` 0.01): `gbuffer_pass.ps` writes 1 - (min +
  range x the map's green), and the sun's highlight in `composition.ps` takes
  that as its GGX roughness (alpha = 0.5 s^6 + 0.1 s). Stock's own map is near
  black, gloss 0.04: a broad highlight of about 0.04 x the sun. The colour
  texture gives 0.46: a sharp one of about 3 x the sun, and with every normal
  up a whole patch took the sun's colour where the player looked toward the
  sun and down. Both ends of the range are 0.04 now, stock's average. Seen in
  the game on Military Warehouses (2026-11-14, captured frames at the owner's
  two places and settings): the sun-coloured patches are gone, the grass keeps
  its colour. Training Camp's meadow grass (`ported/v010b/flora/grass_l03_alt`,
  also 0.3 over its colour texture) is not changed.
- The other four maps take the same option. Repair Depot's level has
  349,153 tufts at the same density (thin it with `density`); Training Camp
  has its meadow already; Station and Depository need their level folders
  again.

## Bushes rustle (the owner, 2026-11-14)

"Bushes should have rustling sound just like on normal survarium maps": walking
through a bush of a ported map was silent. Pack `stalker-bushes@2` and
`content-edit/stalker-bushes-1.json` give Experimental Pool, Military Warehouses
and Training Camp the foliage step of stock's bushes. Built and read back
(below); **not heard in the game yet**.

### What rustles in stock 0.26g (the exe and the data, read statically)

- A step is the walk animation's sound event (`crouch_steps.md`):
  `player_equipment_sound_effect::on_foot_step` 0x2A6520 ->
  `step_manager::on_step` 0x248AD0 (a landing too, 0x23FBE0). `on_step` casts
  one ray from 1 m above the foot straight down, 2 m long, with filter group
  30h and mask 8 (0x248AED..0x248B58, `ray_test` 0x75E20). The closest hit's
  triangle gives a game material (`bt_static_rigid_body::get_triangle_material`
  0x3570A0); the pair of the foot's material and it (0x306D50) gives the sound.
  Nothing else plays a pair of a foot material: `material_pair`'s other seven
  callers are the bullet's four and the thrown things' marks (0x248160,
  0x248720, 0x248980). PROVEN.
- An object is tried by a ray when its group has a bit of the ray's mask and
  its mask a bit of the ray's group (`needsCollision` 0x75BD0). PROVEN.
- The physics world's level is the project's `collision_objects` and nothing
  else: `project_cooker_simple::create_game_objects` 0x22C010 asks for each
  entry's `lib_name` as a collision resource with its `cgroup` and `cmask`
  (`static_collision::insert` 0x2F8850 adds the body with them), and for each
  `render_visuals` entry as a drawn model only. The exe has no `hq_collision`
  string: a model's `hq_collision` is not the game's (an editor's mesh).
  PROVEN.
- A triangle's material: `collision_shape_cook` (0x82590, 0x829E0) reads
  `<lib_name>/vertices`, `indices`, `face_data` and the model's `settings`; a
  face's name in `face_data`'s dictionary names
  `game_material_settings/<name>/game_material_id`. PROVEN.
- The sounds: `game_materials/material.pairs` gives foliage_deciduous (44)
  `material/foliage_2/step/foliage_2_step` for the walker's own steps
  (actor_local, 42), `..._m` and `..._sprint_m` for the others' (actor 10,
  actor_sprint 53), the crouched ones of `crouch-steps-1`; **the walker's own
  sprint and landing (actor_local_sprint, 56: every boot's
  `sprint_1st_view_material_id` and `jump_1st_view_material_id`) have none**:
  sprinting through a bush, he hears no step of his own for the steps that
  meet a leaf, in stock too; the others hear his. These collections are named
  by nothing but the pairs (the stock db's bytes).
- Which bushes: of the 808 stock collision meshes, five bushes have faces of
  foliage_deciduous, their whole `collision`: `models/flora/vine/bush_big`,
  `bush_low`, `bush_wide`, `bush_corner_in`, `bush_corner_out` (768 to 1,642
  triangles; cgroup 8, cmask 16), each with a smaller `walker_collision`
  inside. Tarakanovsky Fort has 147 of them and Vector 319: a walker goes into
  their leaves and the ray meets a leaf before the ground (of 3,040 standing
  places sampled at the Fort's vine bushes, 235). The free-standing bushes
  (`flora/bush_s_*`: School 82, Chemical Plant 5; `flora/fruit_2/yablo_kust_*`:
  Rudnya 71, the Fort 43) are render visuals with no collision object: silent
  in stock, as the ported ones were. Vector's `level/07/base_v_born_place` has
  1,848 foliage faces of its own, and one fir (`flora/elka_green_05`) is
  foliage_coniferous.
- Why the ports were silent: resource-porter drops X-Ray's passable collision
  triangles (`materials\bush`, `bush_sux`; `src/soc/build.rs`): 3,328 on
  Experimental Pool, 8,373 on Military Warehouses, 866 on Training Camp.

### What the pack is

`scripts/stalker-bushes/make_pack.py` (its docstring has the rule) makes, from
`stalker-maps@6`'s own files (its `pack.json` and each map's inputs pinned by
sha256), one model a map, `models/srp/<level>_bushes`: `settings` and a
`collision` mesh alone, every triangle of game material foliage_deciduous (44).

- Its triangles are the leaves the map draws: the level chunks' surfaces of
  foliage material (`flora`) with a branch or needle texture
  (`trees_vetkagreen1/3/5`, `trees_vetkabig`, `trees_vetkasux2`, `trees_sosna`,
  `trees_sosna_sux`, `trees_dub`, `trees_elka`), a leaf's two drawn sides once.
- Every other drawn surface is on a list of what is not leaves: bark, stumps
  and Training Camp's planks (`wood_collect`), the four other kinds of
  material, one surface drawn with no material. A surface on no list stops the
  script with its name.
- Only what a step can meet: a triangle is kept when a point of it is at most
  1.2 m above a floor of the level's collision (the ray starts 1 m above the
  foot), and is cut off 1.2 m above the highest such floor. A floor faces up
  (the level's ground has its normal up in every triangle) and is no tree's
  wood: the port gives X-Ray's trunks, branches and planks one game material,
  wood_silent, and a branch's top would keep the crown over it (846 leaves on
  Experimental Pool); leaves over a plank floor are kept only where another
  floor is in reach. The leaves over a walker's head and the crowns are not
  in it.
- X-Ray's own bush triangles would be the truer set (the level's `level.cform`);
  Clear Sky is not installed on this machine any more, so the drawn leaves are
  used.

| map | leaves drawn | within a step of a floor | triangles | where a step rustles |
| --- | ---: | ---: | ---: | ---: |
| Experimental Pool | 20,666 | 5,059 | 6,945 | about 630 m² |
| Military Warehouses | 50,271 | 20,503 | 28,772 | about 3,900 m² |
| Training Camp | 4,212 | 2,011 | 2,704 | about 340 m² |

The last column: standing places of a 1 m grid over the level's collision
where the step's ray meets a leaf first. That is roughly two thirds of the
ground where leaves are within a walker's height: the other steps there sound
of the ground, as at the Fort's bushes.

`@1` (never released) took a triangle for a floor by the size of its normal's
y alone: undersides counted and branches' tops did, and leaves high in the
crowns were kept (9,098, 31,833 and 3,242 triangles).

### Where it stands in the project, and who meets it

`stalker-bushes-1.json` adds the model as one collision object of each map's
Team Deathmatch project (`day_4`), after the level's own chunks, before
`ffa-projects-1` in `release/content.tsv`: Free for All's project is a copy
and has it too. Each entry says one file must take it (`expected_files`, new
in `content_edit.py`: an `add_entry` whose anchor is gone was passed over):
the build stops at a project the object cannot be placed in, it cannot leave
the bushes out silently. **cgroup 8, cmask 32**, a pair no object of the game
had (every project's are 2/4, 8/16 and 10/20):

| who asks | its group / mask | meets the bushes |
| --- | --- | --- |
| a step (`on_step` 0x248B58) | 30h / 8 | yes: the foliage step |
| a bullet (`check_collision` 0x32D625) | 10h / 48h | no |
| a blast's rays, the lobby's and the warm-up's cameras | 10h / 8 | no |
| a grenade's body, a battery's | 818h / 8, 0A18h / 418h | no (each side's group must have a bit of the other's mask, 0xB3EDC) |
| a walker's body, the foot placing, carried things | 824h / 3, 24h / 2 | no |
| the use ray (`detect_usable_object`) | 114h / 8Ah | no |
| a trap being placed (0x3064D2, 0x3066E5) | 404h / 202h, 414h / 20Ah | no |
| a spot (`apply_new_input` 0x2D87EB) | 0FFFFh / 80Ah | yes, as a hittable thing that is not walkable: the ray goes on |
| a dead body and its dropped gun (the ragdoll patch's level ray) | 414h / 20Ah | no: they fall through the leaves (it was 434h; `ragdoll.md` section 19) |

The rows are every call of the physics world's ray test, ray query and
penetration test in the game's code whose filter is pushed as a number, the
bodies the notes know, and the patches' own rays. A requester that takes its
filter from elsewhere would not be in it; none was found.

- Bullets, grenades and blasts pass as before: no foliage hit effect either
  (the Fort's bushes have one, and turn each bullet through them by up to the
  round's `pierce_dispersion_angle`, `try_pierce` 0x32D960; cmask 16 would do
  the same here, with a grenade bouncing off the leaves).
- A spot's walk stops once it has met both something hittable (8) and
  something walkable (2). The level's chunks are both (10) and stop it as
  before. A sight line through a bush's lowest 1.2 m that also crosses
  walker-only collision (on these maps Training Camp's alone: its crater
  boundary in Free for All, its bonfires' walker meshes) now stops there,
  where it went on: the one thing a match computes otherwise, hence the words
  in `simulation_changes.tsv`.
- The mesh is inside the level's collision box, so the physics world's box
  (which the level's groups 8 and 2 make) is the same.
- The ragdoll patch's level ray had bit 20h through the foot placing's group
  24h, and a body and its gun would have come to rest on the leaves. Its
  group is 414h now (`LEVEL_GROUP`, `rust/patches/src/ragdoll.rs`): of every
  group and mask in the live data, the bushes' are the one pair that 434h met
  and 414h does not (`ragdoll.md` section 19).

### Not done

- Repair Depot, Station and Depository (not played): a row each in the
  script's `MAPS` once they are.
- The walker's own sprint and landing on leaves are silent (stock's pair,
  above): a `collision_sound` for pair 56/44 in `material.pairs` would give
  them one, on the Fort's and Vector's bushes too.
- Bot map data and the site's renders do not read it.

To make it again: `python3 -I srp-026g/scripts/stalker-bushes/make_pack.py
--packs $SKHID_PACKS_DIR --out $SKHID_PACKS_DIR/stalker-bushes-2
--edits srp-026g/scripts/content-edit/stalker-bushes-1.json` (the same bytes
every run: `pack.json` sha256 `a0b9ea98...d167`, the script's pin and the
manifest's). `tools/check stalker-bushes` makes it again from the pinned source
pack and compares the pack and the list; `stalker-bushes-manifest` holds the
manifest's rows (the `stalker-maps` pack it is made from, this pack, the list
before `ffa-projects-1`) to the script's pins, without making anything.

## Ladders

0.26g cannot climb. Every ladder stays as the source has it: no ramp, lid or
blocker is added and nothing is removed, so whatever only a tall ladder reaches
stays out of reach for now (the owner decides later). There are no low ladders
(every rise is 2.9 m or more). "Needed" means the ladder is the only way to an
area; the rest are joined on foot, lead to a few square metres, or have no floor
at an end. Position is the ladder's x, z; heights are its foot and top (y).

**Experimental Pool** (9): needed: clmbl#0 (17.67, −6.57; 4.4 → 13.7 m),
clmbl#5 (−13.98, 29.92; 8.8 → 13.4), clmbl#6 (−38.52, 26.0; 4.4 → 13.3),
clmbl#8 (−4.96, −7.54; 9.0 → 13.4), each the only way up to the upper floors and
roofs (1,507 m², 15 source spawns); clmbl#4 (20.83, 36.36; 4.4 → 7.6) to a 62 m²
area. Not needed: clmbl#1, #2, #3 (joined on foot), #7 (a 15 m² spot).

**Military Warehouses** (14): needed: clmbl#0 (71.68, −58.67; −8.9 → −0.5) and
clmbl#1 (13.19, −58.58; −8.9 → −0.6), the only ways up to a 675 m² area with 2
source spawns; clmbl#2 (−77.12, 39.04; −11.3 → −7.8) to a 503 m² area. Not needed:
#4-#9 (joined on foot or a 0 m² landing), #3, #10-#13 (no floor at an end).

**Training Camp** (1): clmbl#0 (21.75, 3.22; 16.6 → 20.2) to a 22 m² walkway with
one source spawn. The earlier package's slab over its rungs is not kept.

**Repair Depot** (2): clmbl#0 (−4.76, −33.8; 0.5 → 15.9) to an 18 m² ledge and
clmbl#1 (42.07, 18.87; 4.4 → 7.6) to a 15 m² ledge, no spawns: neither matters.
The earlier package's slabs are not kept.

**Station** (6): needed: clmbl#0 (131.36, −6.76; −12.2 → −8.5) 65 m²,
clmbl#1 (157.52, −8.09; −12.0 → −8.3) 49 m², clmbl#2 (118.0, −38.71; −12.4 → −8.0)
97 m², clmbl#5 (231.72, 23.85; −12.5 → −8.5) 314 m² with 2 source spawns. Not
needed: #3 (a 4 m² spot), #4 (26 m²).

**Depository** (6): needed: clmbl#1 (22.92, −16.14; 13.4 → 16.9) 35 m²,
clmbl#2 and #3 (4.63, 43.75 / −8.09, 33.39; 13.2 → 19.7) 173 m², clmbl#4
(15.6, −1.9; 13.5 → 17.9) 251 m² with 4 source spawns, clmbl#5 (17.48, 12.7;
18.0 → 21.4) 170 m². Not needed: #0 (no floor at its top).

## How it is built (resource-porter branch `stalker-maps-026`)

```
scripts/stalker-tdm/port-map.sh MAP [LEVEL_FOLDER [TEXTURES_FOLDER]]   # one map's v0.100b bundle, then the pack
scripts/stalker-tdm/build-pack.sh [--version N]                       # every map: donor, spec, 0.26g, pack, minimaps
```

- `port-map.sh` converts a level with `build-soc-map` (recipes
  `ports/cs-mp_bath-to-0100b.json`, `cs-mp_military_1-…`, `cs-training_camp-…`
  with the meadow addon, `cs-mp_rembasa-…`, `soc-ug-mp_station-…`,
  `soc-ug-mp_depository-…`), from the installed game or from a level folder read
  over the game's archives (a `base` + `overlay` source the porter opens as one),
  and writes every source file's sha256 (`sources/MAP.sha256`) so a re-export can
  be compared.
- `build-pack.sh` composes the six bundles into one v0.100b donor, matches every
  converted material to its template, writes the spec from
  `ports/stalker-tdm/maps.json` (ids, names, loading ids, probes) and
  `ports/stalker-tdm/spots.json` (the planner's spots), imports the maps into
  0.26g (`build-survarium-maps-026`, which now takes spawns by team, ammo stashes,
  the level number, a loading id and names by language) and makes the pack with
  `pack from-bundle`, checked over every live patch. One pack: the maps share
  converted textures and materials.
- Nothing in the steps is edited by hand: a level the owner changes in the SDK
  goes through the same commands.

### Re-exporting a map from the SDK

| map | SDK | files the porter reads back |
| --- | --- | --- |
| Experimental Pool, Military Warehouses, Training Camp, Repair Depot | the Clear Sky SDK (the CS level format) | `level`, `level.geom`, `level.cform`, `level.game`, `level.spawn`, and the level's own `terrain\*.dds` |
| Station, Depository | Shadow of Chernobyl SDK 0.4 (xrLC 14, SoC 1.0006) | the same, and the map's own `textures\` (terrain, intro and map pictures) |

The rest (shaders, materials, shared textures) comes from the installed game.
The spawns do not come from the level: they are the planner's.

## In this branch

- `srp-026g/flash/lobby_menu`: levels 14-19 in the rooms' and matches' name
  tables, their room tiles (`RENDER_ONE`), their loading pictures
  (`LobbyMenu._load_images` 30-35, classes `Level_<project>`).
- `srp-026g/flash/hud`: their minimaps (`Minimap._levels`, `_minimapProps`,
  classes `minimap_<project>`). Training Camp's ground runs to the level's
  edge (400 x 400 m), so the renderer's default cut it 45 m around the TDM
  spawns (a figure-8). Its minimap is framed on the crater wall of revisions
  57-58 + 15 m and shows all the ground there, fading out inside the Tab
  map's 460 px (the owner's pick, 2026-11-13): `render_minimap.py --map 212
  --frame-outline srp-026g/scripts/stalker-maps/training-arena.tsv`
  (resource-porter `fix/training-camp-minimap`). Free for All's wall has since
  moved in to the base's fence (`stalker-ffa.md`); the frame has not: the
  fenced base is 73 x 67 m of the frame's 159 m, its middle 9 m east of the
  frame's. Repair Depot's south edge has
  the same 45 m circles; it is not played.
- `tools/actionscript` adds a bitmap the original movie lacks from the tree's
  `bitmaps/<class>.png` (lossless) or `.jpg`; `srp-026g/scripts/stalker-maps/ui_images.py`
  writes them from the porter's minimap render and a loading picture.
- Loading pictures and room tiles: Experimental Pool, Military Warehouses and
  Training Camp are frames of the game itself, captured on 2026-11-14 (the
  owner: "rerender preview images, since they are low quality and don't have
  grass"): a headless game at 2560 x 1440, every graphics setting at its
  highest, a fixed camera of 100 degrees across, the maps as PR #1793 leaves
  them (the grass, stock film grain and ambient occlusion). The earlier
  pictures were 1920 x 1080 frames from before the maps had grass, with the
  old builds' grain. Camera (x, eye height y, z; yaw 0 looks +z, 90 looks +x;
  pitch up):

  | map | pose | position | yaw | pitch |
  | --- | --- | --- | ---: | ---: |
  | Experimental Pool | `poolside-se` | (4, 6.5, 4) | -60 | -4 |
  | Military Warehouses | `road-tower` | (-8, -5.5, -8) | 3 | -2 |
  | Training Camp | `base-se2` | (62, 19, -50) | -47 | -7 |

  The frame is scaled to cover 1920 x 798 and its middle band kept, as
  `ui_images.py` cuts it; the existing classes serve both loading screens and
  room tiles. Repair Depot still uses the earlier package's in-game shot; the
  two underground maps still use top-down placeholders.
- `release/content.tsv`: the pack row `stalker-maps@4`. @3 has the grass
  with flat-lying normals (black tufts under a high sun); @2 also has the
  planner's spots on Experimental Pool; @1 has no grass.

## Still to do before a release

- An in-game test of each map (local game): spawns, stashes, minimap frame,
  loading picture, names.
- The site: a top-down render per project in the calculator's maps
  (`scripts/damage-calc/assets/maps`, README) before `release/build-site`, or the
  Maps tab build stops. The three played in Free for All have theirs (of
  `day_8`); Repair Depot, Station and Depository need one before they are
  played.
- Bots: `mapdata-dump` for the new maps (`docs/notes/player_bots_mapdata.md`).
- The live pools: `SRP_MATCH_MAPS` / `SRP_ROOMS_EXTRA_MAPS` say whether the maps
  are in the match pool or rooms only.
