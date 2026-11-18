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
- Rain and evening are four more projects of `maps-021d-weathers@3`, made from
  the authored day source by the same builder
  ([the weather README](../../scripts/v021d-weather/README.md)). 0.21d's rain
  has two hanging lamps its day has not; `v021d-weather-geometry-1.json` adds
  them with their breakable fixtures, as School's rain.
- `three-maps-021-lighting@7` carries the three profiles (markers -32287 day,
  -32289 rain, -32291 evening); `v021d-forward-fog@3` guards them too.

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
