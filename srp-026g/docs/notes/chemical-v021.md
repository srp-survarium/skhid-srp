# Chemical Plant v0.21

The sixth 0.21d map (`level_04`), ported the way School, the Fort and Rudnya
were ([three-maps-v021.md](three-maps-v021.md)), in all three weathers.

| Weather | Protective device | Team Deathmatch | Project |
| --- | --- | --- | --- |
| day | 266 | 267 | `v021d_level_04/day_2`, `day_4` |
| rain | 268 | 269 | `v021d_level_04/rain_2`, `rain_4` |
| evening | 270 | 271 | `v021d_level_04/evening_2`, `evening_4` |

Map number 31 (`st_level_31_name`, `st_srp_map_chemical_plant_v021`: the native
name with " (v0.21)"); the lobby draws it with native Chemical Plant's renders
(`RoomsLook.RENDER_LEVEL`) and its loading screens (5 day, 10 rain, 11 evening).
Ids over 255 need exe patch 154.

## The packs

- `maps-021d-chemical@1`, resource-porter `tools/content/chemical_021d.sh`
  (branch `port/v021d-chemical`): donor geometry, local lights, probe volumes
  and cubemaps with native material schemas; native level_04's ammo crates at
  its spots, set on 0.21d's floor (3 cm). 151 shared `ported/v021d` paths keep
  the live bytes (`chemical_021d.keep-live.txt` says why).
- Rain and evening are four projects of their own pack, `maps-021d-chemical-weathers@1`, made from
  the authored day source by the same builder
  ([the weather README](../../scripts/v021d-weather/README.md)). 0.21d's rain
  has two hanging lamps its day has not; `v021d-weather-geometry-1.json` adds
  them with their breakable fixtures, as School's rain.
- `three-maps-021-lighting@8` carries the three profiles (markers -32287 day,
  -32289 rain, -32291 evening); `v021d-forward-fog@5` guards them too. Both are
  the five maps' `@7` and `@4` with nothing but Chemical Plant's guard branches
  added to every program (128 and 1904 files; compared instruction by
  instruction after taking the three markers' branches out).
- `v021d-renderer-2.json`: exe patch 130's keys, as `v021d-renderer-1.json`
  gives the other five maps (the six client projects, `flora/grass_l04`, the
  two `water_l04_*` materials).
- `v021d-chemical-materials-1.json`: the anomaly smoke's numbers as 0.21d has
  them (`radial_smoke`'s frame sequence at 0.5, the template's 1 played it
  twice as fast; `radial_smoke` and `smoke_sample` glowing at 1; `green_smoke`
  without the template's emissive map). Numbers only: a flipped switch can ask
  for a shader variant 0.26g never shipped.

`release/content.tsv` applies `v021d-chemical-environment-1.json` (the shader
marker, cloud coordinates, the original engine's black/white height-ambient
defaults), 19 breakable lamps a day project in `breakable-lamps-1.json`, and
the map's rows in `maps.tsv`, `modes.tsv`, the music, vegetation and Keeper
lists. The 0.21 ambient and particle-light rule of exe patch `ambient_021`
takes every level 1 to 6.

## Importer rules

0.21d's anomaly smoke (`fx/level_4_anomaly/green_smoke`, `radial_smoke`,
`smoke_sample`) is a forward particle; 0.26's material of the same path is a
lit one. A same-path native material is the template only when it has each
donor render stage that carries visibility; otherwise the donor material is
converted like one 0.26 does not have (here on `level_4_anomaly/bugs`, a
forward particle) and stays 0.21's.

A particle material's transparency texture keeps 0.26's file where the
full-size image is the same (the anomaly's `bug` and `spori`, the candle
light): 0.21's small mips raise alpha, and 0.26's particle pass would draw
pale squares with them (the reason `v021d-particle-alpha` was dropped).

## Comparison with 0.21d (2026-11-18)

Generated poses a set (interiors, roofs, lights, horizon, ground, mode objects,
spawns, four sun views), 0.21d shot once and kept, the port on a quick build
with lighting@7, both at the rig's 0.21 quality settings. Static render
geometry: 51.1 MiB (Protective device) and 51.0 MiB (Team Deathmatch) in every
weather, of the pool's 88.

| Set | Pairs | Flagged | Serious |
| --- | --- | --- | --- |
| day | 96 | 4 | 3 |
| rain | 93 | 13 | 8 |
| evening | 96 | 8 | 5 |

All six projects load with no missing shader. The day sun views match 0.21d's.
Open, none of them fixed here:

- **Rain: the anomaly's liquid is pale white** (`battery_3`, `roof_1_out`); it is
  teal in 0.21d's rain, in native 0.26 rain at the same cameras, and in the
  port's day and evening. Lighting quality 0 or 1 makes no difference. The
  material (`anomaly/anomaly_liquid`) is 0.26's own with 0.21d's diffuse;
  0.21d's rain fog is dense and white (Mie far distance 60 m against the
  native rain's 1200), which is where to look.
- **Sunlight missing indoors**: day `in_18_wall` (one sun patch on the floor)
  and evening `in_03_near` (a room the low sun floods in 0.21d is unlit): the
  sun's shadow covers them, as on School's gym wall.
- **Water** (`in_32_wall`, every weather): 0.21d's water mirrors the sky at the
  far edge, the port's does not (water surface lighting is 0.26's).
- **Roof metal** (day `in_31_near`): 0.26's finish is glossier and bluer than
  0.21d's matte rust (the native-finish rule of these ports).
- The anomaly's green haze and bright spores in 0.21d's rain and evening shots
  are its active state at the time of the shot (the day shots of both builds
  have neither); the port's static captures show it idle.
