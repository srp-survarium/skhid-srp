# London (v0.42)

The reworked London as a map of its own beside London (v0.32) (the owner, 2026-11-20: "I think we
want london 0.32 in the game and london 0.42 in the game"; "let's finish london", "both versions").
Update 0.42 (2016-05-11) moved both bases in, cut the outskirts and changed the tower's inside; no
build of 0.35 to 0.43 is archived, so the donor is 0.44a5 (build 3808, 2016-09-20), the first
archived build with the rework (`private/renders/london-history`). Content pack `london-v042@1`,
made by resource-porter's `build-survarium-near-map` from 0.44a5's own projects
(`ports/survarium-044a5-london-to-026g.json`, `docs/near-builds.md` there).

| maps_dict id | project | mode | weather | loading id |
| ---: | --- | --- | --- | ---: |
| 290, 291, 292 | `v044a_level_09/day_2`, `day_3`, `day_4` | 2 (Protective device), 3 (its charging stations), 4 (Team Deathmatch) | day | 41 |
| 293, 294, 295 | `v044a_level_09/rain_2`, `rain_3`, `rain_4` | 2, 3, 4 | rain | 43 |
| 296, 297, 298 | `v044a_level_09/evening_2`, `evening_3`, `evening_4` | 2, 3, 4 | evening | 42 |

- Level 32 (`match_making_params.map_id`), `st_level_32_name` and `st_srp_map_london_v042`
  ("London (v0.42)" in each language), build `v042` in `modes.tsv` (`london.md`, "Name and build").
  Ids 290 and up, level 32 and loading pictures 41 to 43 were free on main, in every published
  patch and in every pack of `private/content-packs` on 2026-11-20.
- The lobby shows London's own pictures for it (loading pictures 41 to 43 are `Level9_day`,
  `Level9_evn`, `Level9_rain`; its tiles are London's by weather). The HUD has a row of its own
  for `v044a_level_09`: 0.44a5's own minimap of the reworked map (its `map_09_minimap.png`, the
  cut outskirts hatched; `flash/hud/bitmaps/minimap_v044a_level_09.png` at the movie's 512 px) in
  London's frame, which 0.44a5's HUD has too (size 140, centre 0, -5). The wiki's top-down render
  is in the same frame (`scripts/damage-calc/assets/maps`).
- 0.44a5 has no mode 2 (the game dropped it in 0.42). Each weather's `_2` is its mode 3 project
  without the four charging stations and what they own (their props, lights, sounds, cables and
  boxes); the 24 battery places they owned are mode 2's.
- Each weather is 0.44a5's own project of that weather (`day_N`, `rain_N`, `evn_N`), so the three
  are one consistent build.

## What the conversion keeps, and what it leaves out

One pass over the donor's projects, by rule (the pack's `report.json` has every count):

- **Left out by decision:** anomalies (`timeout_anomaly_core`, 96 of them) with their groups: each
  one's damage zones, shapes and artefact places (192 zones, 144 shapes, 96 artefact places).
  0.26g has no object of the kind.
- **The fires burn as the donor's:** its eight fire damage zones (`logic/fire/fire_damage_zone`,
  fire, 0.1 to both hips and both feet every 500 ms) stand in no anomaly's group and come over
  with their shapes, with the fields 0.26g's own fire zone has.
- **Kinds 0.26g has no object of:** `portal_system` (1), `primitive_geometry` (26),
  `sound_environment` (6), `winner_point` (16).
- **Files:** of the models the nine projects place, 47 are players' own, 248 are imported under
  `ported/v044a` (players have another form of the name: other materials, another mesh or
  collision), 44 are new. Of the textures 514 are players' own (402 of them the same image in
  another encoding), 111 are another image than players' file of the name and go in under
  `ported/v044a`, 49 are new. 0.44a5's surfaces carry statistics chunks (110 to 113) that 0.26g's
  do not have: they are not carried. A model's `hq_collision` is not carried either: 0.26g's
  executable has no reader for it.
- **The level's own files** are 0.44a5's: its three vegetation layers (`projects/v044a_level_09/
  vegetation/*.veg`, the grass and camelthorn the vegetation object draws), also where players have
  a file of the name (the old London's layers; the first pack took those for players' and drew
  no grass).
- **Materials:** 116 are players' own, 142 imported, 10 new. 1 038 of 0.44a5's own g-buffer and
  vertex programs that players lack and 0.26g can feed are in the pack; 31 materials whose switch
  set needs a program 0.44a5's archives do not hold for 0.26g took the nearest set players have
  (the vertex-blend, parallax and translucency variants of London's own surfaces; `report.json`,
  `degraded`).
- **Forward effects** (fires, blowing sand, the cloud sheets): drawn as 0.44a5 draws them, by client
  patch 169 (`client_patch.md` section 3.91: DXT1 transparency, uv tiling, the distortion of the
  emissive map alone, the particles' probe light) and the shared pack `forward-effects@1` (its
  programs), which this pack requires. The materials keep the donor's forward switches and fields
  (resource-porter's client fields), so nothing of a forward effect is dropped; the forward fields
  still left out (`uv_distortion_method`, its amplitude and frequencies, the emissive sequence) are
  editor fields 0.44a5's executable has no string for.
- **The environment:** 104 settings from 0.44a5's first key frame. Its height fog (`fog_density`,
  `fog_color`, `fog_base_height`, `fog_falloff_height`), its sky fog and the settings of its later
  sun and cloud model have no 0.26g setting and are listed, not imitated.
- **The evening's camera track** (`track`) is carried: only the console command `demo_play` flies
  it (`game_world::demo_play` 0x1F9800, in 0.26g as in 0.44a5); its key values keep the editor's
  ints and floats (`release/build_content.py`'s allowlist, as 0.26g's lobby track mixes them).
- **Walker collisions:** 0.44a5 ends a walker collision's id in `/walker`; each takes its
  collision's name (`walker_<name>`). Before, the mode-3 projects crashed at load: 0.26g's battery
  rule looks `walker_<name>` up and uses it unchecked (0x3156EC); the content build checks it since
  #2334.
- **Battery places** write every list 0.26g's editor writes, empty or not: the same rule reads each
  station's `attached_objects/collision_geometries` unchecked (0x3155FB), and three of 0.44a5's four
  stations link no static, so the mode-3 projects crashed at load after the walker fix too (the
  minidump: `binary_config_value::operator[]` on the missing key). #2334 checks it too.

## Checked

- Loaded headless on patch 169's exe (#2340) with `forward-effects@1` and this pack, no sound device
  (`work-london-032-042/rig/load.sh`): all nine projects exit 0 with no "cannot find shader" assert;
  `day_4`, `rain_4` and `evening_4` at 28 views each, the six mode-2 and mode-3 projects at one.
- Compared with 0.44a5 itself at the same 28 views in each weather (`private/renders/london-v042/`,
  `rain/`, `evening/`): the owner's ten FFA picks, his London points of 2026-11-19, and the map's own
  fires, sand vortices and sky (eyes found on the port's collision). Day: 1.0 to 4.5/255 at most views;
  the effect views up to 16.4 (moving particles, and the grass below). Rain: the port darker,
  most visibly the glass towers' reflections. Evening: 1.0 to 8.3 but for those towers (37).
- The grass is there (0.44a5's layers) but darker than 0.44a5's: 0.44a5 lights foliage through with
  a direct translucency term of the environment (`direct_translucency_color`, `_intensity`,
  `sun_dir_vs_normal_factor`; London's day 0.57 / 0.56 / 0.40 at 1.2), which 0.26g's executable has
  no setting for. Drawing it so needs a client patch, as the forward effects did.
- Breakable lamps: `breakable-lamps-v042-1.json` (the lamp rule over the pack's projects, `ported/v044a`
  fixtures among its prefixes): 6 tube lamps and 8-9 kerosene lamps a project; build-content's lamp
  check passes.
- Not yet: a game window.
