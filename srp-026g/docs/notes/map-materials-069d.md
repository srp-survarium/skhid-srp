# Map surfaces as 0.69d has them (edit list map-materials-069d-1)

The owner, 2026-11-16, of the wrong map surfaces found in the sound survey (the Chemical Plant's
tracks; concrete rooms that count as dirt): "let's not change things unless 0.69 does so". So a
face changes only where 0.69d's own data differs from 0.26g's.

- **How a face gets its material:** `collision_shape_cook` (0x82590) reads the model's `settings`;
  a face's name in `face_data` names `game_material_settings/<name>/game_material_id`. A name
  without an entry, or with id 0FFFFh, is 3FFFh after the mask (`get_triangle_material` 0x3570A0),
  which `material_pair` steps and hits as `default`. PROVEN (`stalker-maps.md`, `footsteps.md`).
- **The Chemical Plant's tracks.** 0.26g places two collision meshes on the same track bed:
  `props/railroad/railroad.model` (cgroup 10, cmask 20h: bullets and the walker; every face
  `metall_tank`) and `railroad_01..03` (8/16 and their walker meshes 2/4: rails
  `metall_thick_walkable`, sleepers `concrete_walkable`). The higher face wins, so a step along
  the track played a tank, rail steel, concrete and dirt in turn and a bullet into the bed hit a
  tank. They cover the same bed (horizontal area at the tracks: 26 / 120 / 193 m² of railroad.model
  against railroad_03 26, railroad_02 120, railroad_01 192). 0.69d has no `railroad.model`
  (neither drawn nor collision) and gives railroad_01..03's rails (`coll_metal_thick_s`)
  `metall_tank`. Here: railroad.model's collision object leaves the 15 level_04 projects (its
  drawn model stays, the same bed as railroad_01..03's), and the rails' id is 55.
- **Faces with no material that 0.69d names** (the models' settings, both builds):
  - Radar's `garage_inside`: `coll_metall_thick_s` metall_thick (2,066 faces), `coll_concrete_s`
    concrete (1,832), `coll_gravel` gravel (430); `coll_metall_thin_s` metall_thin and
    `coll_wood_silent_s` wood_silent, which its collision does not use, too;
  - the Fort's `center_part_01_floor`: `coll_gravel_mSG` gravel (1,965), `coll_moss_s` ground (949),
    its walker face `walker_coll_s` ground;
  - Vector's `centre_lab_dome`: its one face name, `default` (17,153 faces), has no entry in 0.26g;
    0.69d's entry is the material default (11).
  The names stay as they are ("not assigned or deleted"): the cook reads the id.
- **Left as they are, the same in 0.69d:** Radar's `arsenal_in` (no collision of its own), Mamayev's
  `monument` (no material in 0.69d either) and `statue_ground` (0.69d's is another mesh, without a
  material), the Fort's `out_wall_base_a_2` (none in 0.69d), London's `building_07_3` (not in 0.69d)
  and `building_06_1_indoor_01`'s `stucco_07_s` (none in 0.69d).
- **What a match computes otherwise** (`simulation_changes.tsv`, `next`): a bullet meets other
  values there (game.materials: armor, ricochet, pierce dispersion, width):
  - the track bed: no more `metall_tank` over the bed (armor 20, ricochet 1.2, width 0.1); the rails
    were `metall_thick_walkable` (armor 0.2, ricochet 1, width 1) and are `metall_tank`; the
    sleepers `concrete_walkable` (armor 0.23) where the bed's tank face was higher;
  - garage_inside, center_part_01_floor and centre_lab_dome: 3FFFh (no entry in the material table)
    becomes metall_thick (armor 0.2), concrete (0.23), gravel (20), ground (20, width 10) and
    default (2);
  - the walker: the capsule no longer stands on railroad.model's faces (cgroup 10 is the walker's
    too) but on railroad_01..03's walker meshes, as in 0.69d. The walker bits (`get_walker_behaviour`
    0x356D00 reads the face's top two bits; LIKELY set from the material's `walker_behaviour` at the
    cook) of the newly named faces are 0, as 0FFFFh's are; the rails' go from 1 to 0 on a mesh the
    walker does not meet (8/16). Only the removed bed changes where the walker stands.
- **What a player hears:** steps along the tracks play concrete on the sleepers and the tank on
  the rails, as 0.69d plays them, no dirt or rail steel in between; the garage, the Fort's floor and
  the dome step and are hit as metal, concrete, gravel and ground instead of dirt.
- **0.69d assigns more that this does not take** (asked only for the rooms above): fruit bushes'
  `default` faces as foliage_dry, `container_closed`, `perehod*`, `bridge_big_burned_01` and
  `level_05_out_wall_l_corner`'s walker faces (concrete, concrete_walkable, ground_walkable,
  default), `antenna_base`, `plafon_lamp*`, `lamp`, `podvesnoy_low*` (wood, metal, glass,
  plastic); 76 faces it names otherwise than 0.26g.
