# Vostok Radar Station (v0.69)

0.69d's Vostok Radar Station (its `level_03`, day) as a map of its own beside 0.26g's Vostok, in
content pack `vostok-069d@2`. The owner's asks: "let's do them" (2026-11-16, with School, Rudnya v0.21
and Cologne); on 2026-11-22: port it properly, as Rudnya (v0.69) was, "rebuild and check". Made by
the same 0.69d rules as Rudnya (v0.69) @4 (`rudnya-069d.md`), with no per-map tuning.

| maps_dict id | level (`map_id`) | project | mode | loading id | HUD key |
| ---: | ---: | --- | --- | ---: | --- |
| 259 | 28 | `v069d_level_03/day_4` | 4 (team deathmatch) | 2 | `level_03` |
| 258 | 28 | `v069d_level_03/day_2` | 2 (Protective device) | 2 | `level_03` |

- Ids over a byte (exe patch 154). 256-257, 260-263 and level 27 were taken or claimed for the other
  ports of 2026-11-16; 258-259 and level 28 were free on main, in every published patch and in every
  pack of `private/content-packs` on 2026-11-22.
- Names: `st_level_28_name` and `st_srp_map_vostok_v069`, the native name with " (v0.69)" in each of
  the twelve languages. The lobby draws level 28 with native Vostok's pictures
  (`MapRenders.tsv`, `LIKE 28 Level3`) and its loading screen (2); the HUD draws native Vostok's
  minimap (the same layout). The loading theme is level_03's (`music-1`).
- Not yet in Blackout, Free for All, Freeze Tag or Artifact Hunt: the last two come with School
  (v0.69) in one run of `scripts/mode-maps/make.py` once both are merged.

## After the owner's first game (2026-11-22: "it is fine")

- "remove ammo crates and make them like in Rudnya": ammo only at the bases, as Rudnya (v0.69)'s
  `rudnya-ammo-1`: `vostok-v069-ammo-1.json`, made by `scripts/base-ammo/make.py` (each team's base
  is the crate nearest its spawns and refills that team alone; every other crate goes whole, its
  zones, decal, collision geometry, models and their collision). 0.69d's two mid-map crates
  (`logic/resupply/new_resupply/1`, `/2`) are gone; `logic/resupply/team1` is team 0's base,
  `team2` team 1's.
- "return the gaseous smoke anomaly in the bunker": native Vostok's rust anomaly came over whole
  but for its smoke, ten particles of its own folder (`fx/smoke*`: `fx/level_3_anomaly/rust_smoke`
  and `rust_smoke_dark`); `map.native_objects` takes them too. 0.69d's ground under them is within
  0.1 m of native's at nine of the ten.
- "make lamps breakable for Vostok": `breakable-lamps-vostok-v069-1.json`, by
  `scripts/breakable-lamps/make_edits.py`, which now reads a 0.69d port's render visuals by
  `lib_name` (they have no `visual_name`) and its `v069d_<map>/` fixtures: 4 lamps in Team
  Deathmatch (two with their light), 7 in Protective device (its native battery props' three).
  No 0.69d room probe holds a lamp: no probe or fill light goes out with one.
- Both lists run right after `rudnya-ammo-1`, before the Keeper (its counts stay: the smoke adds
  ten objects, the ammo list takes ten).

## What 0.69d changed against 0.26g's Vostok

A new checkpoint (kpp1), the outer and inner fences re-cut, terrain water, debris round the
officers' building, 183 more plants and 34 more cover props. Static geometry: 45.4 MiB (team
deathmatch), 47.5 MiB (Protective device) of the 88 MB pool; native maps 43-48.

## The conversion (resource-porter `port/vostok-v069`)

`ports/survarium-069d-vostok-to-026g.json` is Rudnya's manifest for level_03: the native
`level_03/day_4` template (the same coordinates), the donor's team-deathmatch spawns
(`logic/modes/team-deathmatch/`, 154: team 0 70, team 1 84), native Vostok's anomalies
(`logic/anomaly/`, `map.native_objects`) in both modes, and native Vostok's battery layout as map 258
(`map.modes`: its 28 spawns, 50 battery spawners, 2 containers and their props).

The rules, as Rudnya @4's:

- light: the donor's 7 probe volumes with their own cubemaps and SH, the largest also as the
  map-wide sphere; its ambient lights as 0.26g ambient lights (113 adding, 20 darkening; the ones it
  fades out farther than every spawn left out); its sun (0.85 at 38 degrees, through its atmosphere);
- exposure: 0.69d's range EV 10 to 12 as exposure profile 33003 of `three-maps-021-lighting@12`
  (floor 0.026042, ceiling 0.104167, key 0.0854), metered as 0.69d meters, and the scene's own colour
  grading texture from its grading and curve;
- materials by the two builds' shading (gloss, Fresnel 0.04, metal maps made texel by texel). New for
  Vostok: a material with no metalness map and a constant metalness above 0.1 is a metal surface too,
  its maps made with that value in every texel (the barbed wire, `tiled/metal/metal_egoza`, 0.7); at
  0.1 or less it stays a dielectric, as before;
- the donor's 6 sound emitters (5 sounds); its one emitter of a sound neither build ships left out.
- Vostok 0.69d has no `sun_tint`, no particle effects and a plain terrain material.

What 0.26g cannot draw is left out with a note, as on Rudnya: detail normals (36 materials), parallax
(6), fuzziness and UV sequences (2 each); 0.69d's sky and height fog (the sky and distance fog are
native Vostok's).
