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

- `maps-021d-chemical@2`, resource-porter `tools/content/chemical_021d.sh`
  (branch `port/v021d-chemical`): donor geometry, local lights, probe volumes
  and cubemaps with native material schemas; native level_04's ammo crates at
  its spots, set on 0.21d's floor (3 cm). 151 shared `ported/v021d` paths keep
  the live bytes (`chemical_021d.keep-live.txt` says why).
- Rain and evening are four projects of their own pack, `maps-021d-chemical-weathers@2`, made from
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

A particle effect 0.26 also ships under the same path stays 0.26's (its
retunes are kept, as a material's finish is), unless 0.26's lacks one of the
donor's emitters: then part of the effect itself is gone and the donor's
effect is imported (resource-porter `survarium_026.rs`, `particle_emitters`).
0.26 dropped the second, green emitter of the anomaly's
`fx/level_4_anomaly/radial_smoke`, so `maps-021d-chemical@1` drew no green
haze over the toxic moss (held 150 s: 0.21d shows it the whole time, 15-17%
of the frame strongly green, the port never), and the smoke materials
`v021d-chemical-materials-1.json` tunes were not used by anything.
`maps-021d-chemical@2` imports the effect as
`particles/ported/v021d/fx/level_4_anomaly/radial_smoke` with both emitters on
the ported materials. Of the 16 particles the six 0.21 maps take from 0.26
by name, the rule matches only this one, so the five released maps do not
change; the other 15 differ from 0.21d in values only.

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
What that comparison left open, and where it stands (2026-11-19, the completion pass over released
revision 66: day pack `@2`, weather pack `@2`):

- **Rain: the anomaly's liquid was pale white** (`battery_3`, `roof_1_out`): fixed by the
  original forward fog (`v021d-forward-fog@5`): difference from 0.21d 40.9 -> 11.2 and
  28.3 -> 6.1.
- **Sunlight indoors** (day `in_18_wall`, evening `in_03_near`): 0.21d's own stale shadow map,
  not the port. Shot in a fresh 0.21d session (one camera a session, kept apart from the
  references), both rooms are unlit as in the port: difference 5.7 and 2.5 there, against
  22.0 and 31.6 between the kept shot and the fresh one. The kept shots were taken after the
  capture route, which leaves 0.21d's sun shadow map stale (as on School's gym wall).
- **Anomaly haze**: a port bug, fixed (the particle rule above). It is not an active state:
  held 150 s, 0.21d shows it all the time.
- **Roof metal** (day `in_31_near`): now 5.8 from 0.21d (was 20.6).
- **Water** (`in_32_wall`, every weather) still differs: 0.21d's water mirrors a bright
  sky and the pillars, the port's neither, at the rig's settings and the owner's. The two
  water programs do the same arithmetic (the reflection cube times 0.25, mixed with the
  screen-space reflection by its alpha); what differs is what the engine binds to them, so
  it is a renderer difference shared by every ported map's water, not a content fix.
