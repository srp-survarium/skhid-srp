# Rudnya (v0.69)

0.69d's Rudnya (its `level_01`, day) as a map of its own beside 0.26g's Rudnya, in content
pack `rudnya-069d@3`. The owner's asks: port it "with the same sharp look" (2026-10-29);
"add batteries there", "and artefact spawners", "and research points and battery chargers",
"we know positions from 0.26 version", "might need to be changed a bit" (2026-10-30).

| maps_dict id | level (`map_id`) | project | mode | loading id | HUD key |
| ---: | ---: | --- | --- | ---: | --- |
| 204 | 14 | `v069d_level_01/day_4` | 4 (team deathmatch) | 30 | `level_01` |
| 216 | 14 | `v069d_level_01/day_2` | 2 (Protective device) | 30 | `level_01` |
| 217 | 14 | `v069d_level_01/day_3` | 3 (Protective device, charging stations) | 30 | `level_01` |
| 218 | 14 | `v069d_level_01/day_5` | 5 (Research) | 30 | `level_01` |

- Ids: 204 is free (stock, patches and packs; Duga's local package used 205); 206-209 are the
  v0.11e maps' and 210-215 the parked `feat/stalker-maps`', so the modes take 216-218. Level 14
  and loading id 30 are the next free on main. The parked branch `feat/stalker-maps` claims
  levels 14-19 and loading ids 30-35 too: the second of the two to land renumbers.
- The look: every imported texture at its full size with all its mips (0.69d's never exceed
  what 0.26g's stock maps use for the same kind), 0.26g's own file where 0.69d kept the same
  image (238 of 463 textures, 345 MiB: see below), the authored LOD0
  geometry kept with 0.69d's own LOD switching, the terrain at full density (simplified
  tiles left the blend overlays up to 2 m off: seams and holes in trenches); materials on
  0.26 templates, which have no metalness, parallax, detail maps or 0.69d's colour grading.
  35.6 MiB of geometry (team deathmatch), 40.9 MiB (charging stations); native maps 43-48.
- The light is 0.69d's (resource-porter `lighting.rs`): its probes re-encoded for 0.26 with
  their multipliers and full rotations, its fill lights as probe volumes, its darkening
  lights as 0.26 ambient lights (at least x0.25, no cylinders), its sun, clouds and the
  pond's mist; interiors raised to what 0.69d's exposure shows, the church, the tractor
  station's annex and the bus stop per area (`area_gains`). 0.26's atmosphere and the
  ambient sounds are native Rudnya's (the two maps share coordinates).
- Spawns (team deathmatch): 0.69d's team-deathmatch points as they are, 405 (team 0: 313,
  team 1: 92); each point's own check radii; their invulnerability time 6 s as the rest of the
  map (maps.tsv; 0.69d's 3 s until then). The owner may revisit it: these are free spawns, up to
  20 m from enemies.
- Names: `st_level_14_name` (the game's texts) and `st_srp_map_rudnya_v069` (the rooms and
  the Matches window), "Rudnya (v0.69)" in each language as the v0.11 ports name theirs.
- Lobby: the rooms' tile and the loading picture are 0.69d's own loading image for its
  TDM Rudnya, `Level_v069d_rudnya` (`flash/lobby_menu/bitmaps/`). The HUD draws native
  Rudnya's minimap: the layout is the same.

## Modes and artefacts

Native Rudnya's own layouts (the owner: "we know positions from 0.26 version"), each mode its
own project and options, made by the porter from the manifest (`map.modes`):

- **A mode project** is the ported scene plus what native Rudnya's project of that mode has over
  its team-deathmatch project (`level_01/day_4`): its props in every top-level list (render and
  collision, the charging stations' temporary ones), by model and position; its client objects
  by `full_name` (the containers' use and team zones, the stations' lights, sounds, particles,
  windmills, spawn boxes and cables). Its server objects are native's whole: spawns, battery
  chargers (`victory_item_spawners`), battery containers (`victory_items_containers`) and
  stations. Its options are native's with the port's level, loading screen and name.
  - 216 (mode 2): 37 chargers, 2 containers, 39 spawns (22/17).
  - 217 (mode 3): 4 stations with 24 chargers, 2 containers, 39 spawns (22/17).
  - 218 (Research): the 3 units of `research-mode@3` (its `level_01_day_5`: 0.27d's units and
    props), 43 spawns. Read from published patch 30, which carries that pack: the porter takes
    it with `--published-patch`, its sha256 in the manifest.
- **0.69d agrees:** its own battery project (`level_01/day_br`) has native mode 3's containers,
  stations and 24 chargers at the same positions, and native's battery props but one.
- **Artefacts, every mode** (team deathmatch too): native Rudnya's swamp anomaly as it is
  (`logic/swamp_anomaly/`, `map.native_objects`): two anomaly cores (Spring, Lifebone), 16
  artefact spawners in the swamp, its toxic damage zones.
- **Checked against 0.69d's geometry** (the walker mesh: the ground under each point, a body
  standing there, its headroom, reachability from the spawns, against native Rudnya's own): every
  spawn, charger, container, station, unit and artefact spawner, and every prop added. Changed:
  - team 2's container stack, a transmitter at (-21.66, 26.22, -83.26) (modes 2, 3): 3.4 m above
    0.69d's ground, which lacks what it stood on; left out with the cable hung from it
    (`new_container/2/wire_set(6)`), as 0.69d's own battery project leaves it out;
  - charger `logic/vip/mode_2/victory_item_spawner(30)` (mode 2): its battery lay 0.12 m under the
    floor of 0.69d's shed at (24.04, 21.66, -56.02); 0.22 m up, 0.10 m above that floor as on
    native Rudnya (24.04, 21.89, -56.02).
  - Left as they are: the station by the church and its charger 14 (mode 3) stand on rubble 0.3 m
    higher than native's, as in 0.69d's own battery project; Research unit C's props by the church
    on rubble that differs by up to 0.5 m (the props rest on one another); the six team-deathmatch
    covers near team 2's spawn (four half, two low), which native's battery modes do not have.
- **Battery respawn** 10 s in both battery modes (`battery-respawn-1.json`, as every map).

## What comes from 0.26g

0.69d kept most of 0.26g's level_01 textures and props, re-encoded. Of the 463 textures the
port imports (the probes aside), 415 have a 0.26g file at the same path:

- **238 are the same image** and the pack draws 0.26g's own file (resource-porter
  `reuse_native_textures`, its `native_same_image`): the same size, the mean colour of each
  4x4 block within 2 levels on average, no channel's mean moved by 1.5, 99% of blocks within
  12 levels. 10 are byte for byte, 101 decode alike, 127 are re-encodes (about 1 level).
  By kind: 80 diffuse, 113 normal, 39 gloss, 6 translucency maps; by use: 115 buildings'
  (tiled surfaces and level_01), 35 flora, 29 props, 29 vehicles, 11 decals, 9 terrain, 10
  others. 345 MiB the patch no longer carries.
- **177 differ** (235 MiB): 0.69d retouched them (gloss levels moved, colours, painted-in
  details; 12 larger than 0.26g's, 15 flora textures turned on their side): imported.
- 49 have no 0.26g namesake (63 MiB), and none matches another 0.26g texture by image.
- Sizes: none exceeds what 0.26g's stock maps use for the same kind (2048; terrain normal and
  gloss maps 1024), all DXT1/DXT5 with their mips as 0.26g's own: nothing is downscaled.
- Models: 46 are 0.26g's own (same collision, render bounds within 5 cm); the 209 imported
  ones with a 0.26g namesake differ in render geometry (0.69d's are denser), and about 85 of those
  share its collision. Materials are 0.69d's converted onto 0.26 templates (224, 5 MiB).
- Terrain collision: 0.69d's, the same as native Rudnya's. Its walker mesh lies a median 4 cm
  under the drawn ground (10 cm at the 90th percentile); the collision bullets hit follows it
  within 2 cm, and the ragdolls rest on that one (ragdoll.md section 17). East of the play area
  a mud field is drawn 30 cm above both, as in 0.69d and native Rudnya.

## Building the pack

resource-porter (branch `feat/rudnya-069d`), the 0.69d volumes as a `.split` list:

```sh
vostok-port build-survarium-069d-map --manifest ports/survarium-069d-rudnya-to-026g.json \
  --donor-db donor-069d.split --target-db <stock 0.26g resources.db> \
  --published-patch resources.db.patch_30 --output <bundle>
vostok-port pack from-bundle --base-db <stock 0.26g resources.db> --bundle <bundle> \
  --output <packs>/rudnya-069d-3 --id rudnya-069d --version 3
```

The pack's folder goes in `$SKHID_PACKS_DIR` (`private/content-packs/rudnya-069d-3`), where
`release/build-content` composes it from; `pack.json`'s sha256 is its row in
`release/content.tsv`. The skill `map-port` has the whole procedure.

## Checked against 0.69d

The reference rig (`private/rudnya-ref069/tool`), 112 fixed poses against 0.69d's own frames
(its shots taken once and kept; 7 poses with none checked from the port alone), region flags on
an 8 x 6 grid (`report-modes7`): no black volumes, missing walls or holes left. Mean frame
brightness, 0.69d / the port: church inside 57 / 51, nave 54 / 65, bell 48 / 45; under the
tractor station's annex roof 63 / 58; bus stop 54 / 67; school 54 / 70, houses 57 / 69 (the
port's interiors with sun run brighter). Left flagged: the power plant on the horizon and the
distance are darker and bluer (0.69d's haze over the map is not reproduced), the bus stop's
corner by its round window darker.

## Before a release

- An in-game look and a playtest: no 0.69d map has been played on Skhid yet (Duga was a
  local package); each mode's battery props, stations, units and artefacts in game.
- The site: its top-down render is `v069d_level_01` (`scripts/damage-calc/assets/maps`, built
  by `tools/build_all.py` from the pack over the db in native Rudnya's frame and play area).
- Bots: no `.mapdata` and not in Player vs Bots.
- Pools: in the match pool and every room's, as every playable map is (`SRP_MATCH_MAPS` is
  unset on live: every playable map of the game data); no `.env` change.
- Open: 0.69d's reeds and pond grass are golden; the port's are grey-green. The textures and
  material are 0.69d's, and 0.26's own material for the same reeds draws them the same: it
  is 0.69d's grass lighting (a shader change), not the conversion.
