# Cologne (v0.69)

0.69d's Cologne (its `level_08`, day) as a map of its own beside 0.26g's Cologne, in content pack
`cologne-069d@3`. The owner's asks: "let's do them" (2026-11-16, with School, Rudnya v0.21 and
Cologne); on 2026-11-28: "start work porting the next map". Made by the same 0.69d rules as Rudnya
(v0.69), Vostok (v0.69) and School (v0.69) (`rudnya-069d.md`, `vostok-069d.md`), with no per-map
tuning.

| maps_dict id | level (`map_id`) | project | mode | loading id | HUD key |
| ---: | ---: | --- | --- | ---: | --- |
| 543 | 30 | `v069d_level_08/day_4` | 4 (team deathmatch) | 21 | `level_08` |
| 544 | 30 | `v069d_level_08/day_2` | 2 (Protective device) | 21 | `level_08` |
| 545 | 30 | `v069d_level_08/day_3` | 3 (Protective device with charging stations) | 21 | `level_08` |
| 546 | 30 | `v069d_level_08/day_5` | 5 (Research) | 21 | `level_08` |

- Ids over a byte (exe patch 154). The early start of 2026-11-16 (`feat/cologne-v069`, pack
  `cologne-069d@1`, never listed) took 262 and 263, which `maps-021d-weathers@2` has had since
  patch 65. @2 took 282-285, which the open #2225 (Chemical Plant v0.11's evening and rain) claims,
  and 286-289 are `feat/v011-variants`' (the v0.11 weather variants, no PR yet); @3 is the same files as
  543-546. No stock db, published patch (12 to 75), content pack (`private/content-packs` and every
  work folder's), branch of skhid-srp or resource-porter, or open PR of either names 543-546 as any
  id, or level 30, on 2026-11-28; the maps' ids below them run to Research's and Free for All's
  500-511. School (v0.69) has 256, 257, 299, 116 and
  level 27, Vostok (v0.69) 258, 259 and 28, the Vostok v0.11 weathers (#2207) 272-275, the firing
  range (#2098) level 21; Artifact Hunt's and Freeze Tag's next ids (`mode-maps/make.py`) start past
  350 and 450.
- Names: `st_level_30_name` and `st_srp_map_cologne_v069`, each key's native name (`st_level_08_name`,
  `st_srp_map_cologne`) with " (v0.69)" in each of the twelve languages. The lobby draws level 30 with
  native Cologne's pictures (`MapRenders.tsv`, `LIKE 30 Level8`) and its loading screen (21); the HUD
  draws native Cologne's minimap (the same layout). The loading theme is level_08's (`music-1`). The
  modes' rows are native Cologne's (`modes.tsv`: 150 tickets, 2 charging stations), and so is its `maps.tsv`
  row (spawn zone 3000 ms with no fade; no artefact search, as Cologne has no artefact containers).
- Ammo only at the bases (`cologne-v069-ammo-1.json`, `scripts/base-ammo/make.py`): 0.69d's two
  mid-map crates (`logic/resupply/neutral/first` at 2.4 33.8 26.5, `second` at 6.5 36.3 59.4) go
  whole; `logic/resupply/team1` (-61.4 33.0 17.5) is team 0's base and `team2` (69.1 33.2 44.1)
  team 1's, each refilling its own team alone. 9 client objects, 6 collision objects and 3 visuals
  less a project.
- Breakable lamps (`breakable-lamps-cologne-v069-1.json`, `scripts/breakable-lamps/make_edits.py`):
  3 in every mode, the stock `props/light/green_lamp`; none has a light of its own. Native
  Cologne's battery props bring no lamp. `v069d_cologne/` is among the ports' prefixes, though no
  fixture of it is a lamp (its `props/light/bonefire_01` is a fire).
- The Keeper's counts (`artefact-keeper-1`): 457, 481, 451, 467 client objects (day_2 to day_5).
- maps_dict counts +4 in `artefact-hunt-1`, `blackout-mode-1`, `ffa-1`, `ffa-ported-1`,
  `v011e-vostok-modes-1`, `v021d-school-fort-modes-1` (and `artefact-hunt/make_pack.py`'s
  `MAPS_DICT_COUNT`). School (v0.69) moves the same counts by its own four: the second to land adds
  the other's.
- Not yet in Blackout, Free for All, Freeze Tag or Artifact Hunt (one `scripts/mode-maps/make.py`
  run for the v0.69 maps), not in `water-steps` (wading in the river), no bots' `.mapdata`, no site
  render.

## What 0.69d changed against 0.26g's Cologne

Bases A and B and the favela re-cut, base B's gates, the bridge without its underside (0.26g's
`bridge_down_*` and `holder` are not in 0.69d), the bridge's fence cover moved, corrugated sheets
over (-61, 33, 14) and (-49, 33, 14), charging racks and a lab table on the bridge. Static geometry:
28.0 MiB (team deathmatch), 29.8 MiB (Protective device), 31.1 MiB (charging stations), 30.8 MiB
(Research) of the 88 MB pool; native maps 43-48.

## The conversion (resource-porter `port/cologne-v069`, #110)

`ports/survarium-069d-cologne-to-026g.json` is Vostok's and School's manifest for level_08: the
native `level_08/day_4` template (the same coordinates), the donor's team-deathmatch spawns
(`logic/modes/team_deathmatch/`, 133: team 0 72, team 1 61), native Cologne's under-bridge damage
zone (`logic/underbridge/`, `map.native_objects`) in every mode, and native Cologne's day_2, day_3
and day_5 layouts as maps 544, 545 and 546 (`map.modes`; Research's project from published patch 30).
Two native battery spawners of Protective device, `victory_item_spawner(23)` and `(2)`, stood 0.11 m
over native Cologne's floor and 0.45 m over 0.69d's, which lacks what they stood on: down by 0.34 and
0.33 m. Built from the converter School (v0.69) @5 was made with (`port/school-v069`: Rudnya's and
Vostok's rules, #106's probe reflections, the sound emitters in the form they play).

The rules, as Vostok's:

- light: the donor's 74 probe volumes with their own cubemaps and SH, the largest
  (`level_08/day/_global_3212`) also as the map-wide sphere; 60 local probes marked for exe patch
  203's tiers, 3 left unmarked (layer 2, or a falloff power other than 1); its ambient lights as
  0.26g ambient lights (165 adding, 58 brightening, 1 darkening; one it fades out farther than every
  spawn left out); its sun (0.9 at 80 degrees, tint 1.0 0.824 0.675, through its atmosphere);
- exposure: 0.69d's range EV 5 to 11 as exposure profile 33008 of `three-maps-021-lighting@20`
  (resource-porter #109: floor 0.001628, ceiling 0.104167, key 0.1232), its sky 0.8589 of 0.26g's
  and 0.69d's vignette (`vignette_intensity` 0.12, k 1.2), and the scene's own colour grading
  texture from its grading and curve;
- materials by the two builds' shading (gloss, Fresnel 0.04, metal maps made texel by texel);
- the donor's 6 sound emitters: 4 collections (`scapes/level_08/random_noises`,
  `right_side_birds`, `left_side_birds`, `devices/wind_mills/wind_mill_random_squeak`) and 2 singles
  (the frogs), each in the form it plays, all of them 0.26g's own files.

What 0.26g cannot draw is left out with a note, as on Rudnya: detail normals, fuzziness, 0.69d's
sky and height fog (the sky and distance fog are native Cologne's).
