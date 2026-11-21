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
  `Level9_evn`, `Level9_rain`; its tiles are London's by weather), and the HUD London's minimap:
  the map's bounds did not change with the rework.
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
- **Files:** of the models the three projects place, 48 are players' own, 247 are imported under
  `ported/v044a` (players have another form of the name: other materials, another mesh or
  collision), 44 are new. Of the textures 514 are players' own (some the same image in another
  encoding), 111 are another image than players' file of the name and go in under
  `ported/v044a`, 48 are new. 0.44a5's surfaces carry statistics chunks (110 to 113) that 0.26g's
  do not have: they are not carried. A model's `hq_collision` is not carried either: 0.26g's
  executable has no reader for it.
- **Materials:** 119 are players' own, 139 imported, 10 new. 30 took a smaller switch set, because
  no material players have uses theirs and 0.26g cannot compile a shader program (it stops with
  "cannot find shader"): the vertex-blend, parallax and translucency variants of London's own
  surfaces. The donor's own programs for them are the next step (`near-builds.md`, "Materials and
  shader programs").
- **The environment:** 104 settings from 0.44a5's first key frame. Its height fog (`fog_density`,
  `fog_color`, `fog_base_height`, `fog_falloff_height`), its sky fog and the settings of its later
  sun and cloud model have no 0.26g setting and are listed, not imitated.
- **Forward effects** (fires, blowing sand, cloth): players' own particles and materials where
  they have one of the name; `london-forward-effects.md` is the proposal for drawing the donor's
  exactly.

## Checked
