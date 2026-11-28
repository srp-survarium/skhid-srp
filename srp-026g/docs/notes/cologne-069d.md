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
- exposure: 0.69d's range EV 5 to 11 as exposure profile 33008 of `three-maps-021-lighting@22`
  (resource-porter #109, key #119: floor 0.001628, ceiling 0.104167, key 0.3346; 0.1232 until @22,
  with cologne-069d@4's grading texture made for it), its sky 0.8589 of 0.26g's
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
  0.88. No wall or floor is missing. What differs: the river's water is 0.26g's own
  `water/water_l08_boloto` (murky brown, where 0.69d draws it clear and blue); the sky and the
  horizon are native Cologne's (no 0.69d dark band at the horizon, a hazier, brighter sky with
  more bloom in the views up through the bridge: those frames are up to 1.5x 0.69d's); the shade
  under foliage is a little warmer.

## The shade, the veil and the sun's glow (2026-11-28)

The owner's screenshots of map 543: the shade far darker than 0.69d's, the sky blown out to white
with a milky veil over the bridge and trees, blotches over dark walls looking out. Three causes, each
fixed as a rule of resource-porter's 0.69d builder (#118, `fix/cologne-069d-tone-key`) for every
0.69d map:

- **The exposure key.** 0.69d level_08's curve clips its white (it rises to 0.896 only), so the
  builder took the search limit 16 as its white: key 0.1232, as School had (#113). With #113's white
  the key is 0.2132, which still left Cologne's steep toe in the grading texture's first interval
  (0.26g encodes its curve before the lookup without sRGB's linear segment: 0 below 0.0008; the light
  0.69d shows at 11 levels was drawn at 5). Above the white's key the builder now takes the key whose
  texture draws 0.69d's grey nearest: 0.3346, 2.7 levels off at worst (6.9 with 0.2132). School keeps
  1.107, Rudnya 0.3367.
- **The bloom.** The port kept native Cologne's `bloom_intensity` 1.3: 0.26g lerps the frame toward a
  six-level blur picked per pixel by the lens dirt texture (`engine/lensdirt_rain`) by it, so every
  view was drawn as more than its own blur: the white veil over the girders against the sky, the
  dirt's blue blotches over the tunnel's walls, and a lift of the shade that hid the key's toe. 0.69d
  adds its 0.02 of a bloom of the light above its threshold; the port now takes the donor's 0.02.
- **The god rays.** Native Cologne's `god_rays_intensity` 1.65 laid a warm glow round the sun (80
  degrees up: every view up through the bridge); 0.69d's light shafts have no 0.26g program, and the
  template's are not them: none.

Checked with headless captures against the kept 0.69d frames, the eye adaptation held fast in the test
build only (the headless game runs at a few frames a second, and the native adaptation then took over
30 s, so the pose order changed the frames). Mean grey, 0.69d / before / after: under the bridge
(`in_indoor_favela_day_m3_1_0_wall`) 58.1 / 71.0 / 56.3; by the house (`in_ambient_light_0_1_0_out`)
59.0 / 72.0 / 58.8; up through the bridge (`in_indoor_favela_day_m3_1_0_up`) 64.4 / 100.4 / 64.9 and
(`in_ambient_light_m3_1_0_up`, at the sun) 92.5 / 135.5 / 94.1; out of the tunnel
(`in_indoor_favela_day_0_1_0_out`) 63.7 / 100.9 / 56.3. The veil, the blotches and the glow are gone.

Still differs:

- the shade 5 to 20 m away outdoors is about half 0.69d's light (the darkest tiles 10-17 where 0.69d's
  are 21-31; interiors and close shade match). The environment's probe multipliers are not it (set to
  1, nothing changes); 0.69d's height fog, which lifts what is far and dark and which the port does
  not draw, is the candidate;
- the sky up near the sun is whiter and about 1.5x 0.69d's: the cloud layer (0.26g's cloud lighting
  over 0.69d's texture; the sky is blue with the clouds off) and 0.26g's atmosphere.
