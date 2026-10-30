# Rudnya (v0.69)

0.69d's Rudnya (its `level_01`, day) as a map of its own beside 0.26g's Rudnya, in content
pack `rudnya-069d@1`. The owner's asks: port it "with the same sharp look" (2026-10-29);
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
- The look: every imported texture at its full size with all its mips, the authored LOD0
  geometry kept (only far LODs and terrain tiles simplified); materials on 0.26
  templates, which have no metalness, parallax, detail maps or 0.69d's exposure and
  colour grading. The sky, fog, sun colour, global probe and ambient sounds are native
  Rudnya's (the two maps share coordinates), the sun's direction 0.69d's.
- Spawns (team deathmatch): 0.69d's team-deathmatch points as they are, 405 (team 0: 313,
  team 1: 92); each point's own invulnerability time and check radii. No 0.69d ambient lights.
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

## Building the pack

resource-porter (branch `feat/rudnya-069d`), the 0.69d volumes as a `.split` list:

```sh
vostok-port build-survarium-069d-map --manifest ports/survarium-069d-rudnya-to-026g.json \
  --donor-db donor-069d.split --target-db <stock 0.26g resources.db> \
  --published-patch resources.db.patch_30 --output <bundle>
vostok-port pack from-bundle --base-db <stock 0.26g resources.db> --bundle <bundle> \
  --output <packs>/rudnya-069d-1 --id rudnya-069d --version 1
```

## Before a release

- An in-game look and a playtest: no 0.69d map has been played on Skhid yet (Duga was a
  local package); each mode's battery props, stations, units and artefacts in game.
- The site: a top-down render for /maps and /damage
  (`scripts/damage-calc/assets/maps/tools/build_all.py`), or `release/build-site` stops.
- Bots: no `.mapdata` and not in Player vs Bots.
