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

## Checked (2026-11-28)

- A quick content build of this branch for revision 76 (`tools/test-build 76 --shared --slow`) over
  the live patches 12..75: it builds (the ammo and lamp lists, the Keeper's counts, the maps_dict
  counts). Its memory budget: Cologne (v0.69) 11.4 MiB of grass world, 1.1 MiB for its largest
  project (day_3), 0.9 MiB of dictionary, 13.4 MiB in all (native Cologne 12.8, Vostok (v0.69) 23.9;
  bounds 32 and 12 MiB): no `MEMORY_KNOWN` row. Players mount 57 patches with the next release's.
- Headless captures straight into each map (`scripts/capture/capture.py`, exe patch 176, no server
  or input), 1280x720 at the reference rig's settings: 543 130 pictures (three games), 544 20, 545
  20, 546 14. Every game loaded and exited 0; the game logs have no missing material or texture,
  only the one `cook of 'unused string' ... failed` line every map has.
- Against 0.69d (its own `level_08/day_tdm` and `day_br` loaded by the rig's `ref069.py`, 136 poses
  shot once and kept in `donor-refs/069d/level_08_*`): the port's light is 1.07 of 0.69d's (median of
  the frames' mean luma; quartiles 1.00 and 1.13), interiors 1.09, the horizon 0.95, the ground
  0.88. No wall or floor is missing. What differs: the river's water (below, "The river"); the sky and the
  horizon are native Cologne's (no 0.69d dark band at the horizon, a hazier, brighter sky with
  more bloom in the views up through the bridge: those frames are up to 1.5x 0.69d's); the shade
  under foliage is a little warmer.

## The river (2026-11-28)

The owner, on 0.69d's `ground_03` (the ground from 3 m, 21.74 16.44 -8.84, yaw 99.5, pitch -28.1;
the pose's map 263 is the id this port had before 543): "water is not drawn at all". 0.69d draws a
clear teal river with the bed under it; @3 drew the stones a little dim and brown, nothing more.

- **Cause.** 0.69d's river is the plane `level/08/moss/back_niz1`, surface `do2sg` (y 14.743, x -129
  to 120, z -396 to 582), with 0.69d's `water/water_l08_boloto` (scattering colour 0.2 0.4 0.5,
  extinction 0.75). 0.26g ships a material of that name, native Cologne's swamp on the same plane
  (fog 0.040 0.075 0.034, density 0.4, one-sided, a night probe that ships nowhere), and the
  converter drew namesake water with 0.26g's material as it is. Native Cologne keeps that plane
  mostly under its ground; 0.69d lowered the riverbed, so the plane draws over the whole river, but
  as a thin dark film: no colour of its own, no reflection. It is drawn: the same material with a red
  fog turns the whole river red.
- **Fix** (resource-porter `fix/cologne-069d-water`, on `port/cologne-v069`; `docs/porting-water.md`):
  the manifest's `donor_water` gives 0.69d's water 0.69d's look on its 0.26g namesake. From both
  shaders (0.69d `forward_water_stage_default.ps` adds `scattering colour x sun illuminance` with
  depth; 0.26g `forward_water.ps` mixes an unlit fog colour by depth and adds a probe's reflection):
  fog colour = linear scattering colour x the port's sun illuminance x 0.14, density = extinction x
  0.47 (fitted on this river against 0.69d's frames), reflection = the map-wide probe. For
  Cologne: `v069d_cologne/water/water_l08_boloto`, fog 0.194 0.604 0.696, density 0.3525,
  `v069d_cologne/environment_probes/map_wide`. Two files change: that material (new) and
  `back_niz1`'s settings.
- **What 0.26g cannot draw:** 0.69d's sun glints (0.26g's water has no sun term; a reflection cube
  with the sun in it changed no frame), and 0.69d's sharp drop from clear shallows to opaque teal
  (0.26g's depth fog is flat past 2.5 m): the shallows are a little more teal, the deep water a
  little less opaque, and from above the water is more saturated, less silvery than 0.69d's.
- **Checked** (headless `capture.py`, map 543, the test-76 build with a delta patch of the two files,
  local pack `private/content-packs/cologne-069d-4-water`, not listed). Columns are 0.69d / before / after.
  `ground_03`: frame luma 98.9 / 57.5 / 96.4, mean difference from 0.69d 48.8 before, 32.8 after; the deep water
  rgb 38 88 99 / 33 20 4 / 43 82 91. Seen from above, after: `ground_02` 114.7 (0.69d 116.9), `ground_05` 100.0 (115.1),
  `ground_10` 110.1 (122.1), `ground_11` 112.5 (116.8). The pack's row and version here are unchanged (@3);
  the fix joins the next build of the pack.
- Rudnya (v0.69) (`water_l1_poison`) and Vostok (v0.69) (`water_l04_shallow`) take the same path:
  0.26g's namesake as it is. Their manifests do not set `donor_water`; their packs are unchanged.

