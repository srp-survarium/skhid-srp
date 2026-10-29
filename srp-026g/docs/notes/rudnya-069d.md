# Rudnya (v0.69)

0.69d's Rudnya (its `level_01`, day, team deathmatch) as a map of its own beside 0.26g's
Rudnya, in content pack `rudnya-069d@1`. The owner's ask (2026-10-29): port it "with the
same sharp look".

| maps_dict id | level (`map_id`) | project | mode | loading id | HUD key |
| ---: | ---: | --- | --- | ---: | --- |
| 204 | 14 | `v069d_level_01/day_4` | 4 (team deathmatch) | 30 | `level_01` |

- Ids: 204 is free (stock, patches and packs; Duga's local package used 205); level 14 and
  loading id 30 are the next free on main. The parked branch `feat/stalker-maps` claims
  levels 14-19 and loading ids 30-35 too: the second of the two to land renumbers.
- The look: every imported texture at its full size with all its mips, the authored LOD0
  geometry kept (only far LODs and terrain tiles simplified); materials on 0.26
  templates, which have no metalness, parallax, detail maps or 0.69d's exposure and
  colour grading. The sky, fog, sun colour, global probe and ambient sounds are native
  Rudnya's (the two maps share coordinates), the sun's direction 0.69d's.
- Spawns: 0.69d's team-deathmatch points as they are, 405 (team 0: 313, team 1: 92);
  each point's own invulnerability time and check radii. No anomalies, artefacts or
  0.69d ambient lights.
- Names: `st_level_14_name` (the game's texts) and `st_srp_map_rudnya_v069` (the rooms and
  the Matches window), "Rudnya (v0.69)" in each language as the v0.11 ports name theirs.
- Lobby: the rooms' tile and the loading picture are 0.69d's own loading image for its
  TDM Rudnya, `Level_v069d_rudnya` (`flash/lobby_menu/bitmaps/`). The HUD draws native
  Rudnya's minimap: the layout is the same.

## Building the pack

resource-porter (branch `feat/rudnya-069d`), the 0.69d volumes as a `.split` list:

```sh
vostok-port build-survarium-069d-map --manifest ports/survarium-069d-rudnya-to-026g.json \
  --donor-db donor-069d.split --target-db <stock 0.26g resources.db> --output <bundle>
vostok-port pack from-bundle --base-db <stock 0.26g resources.db> --bundle <bundle> \
  --output <packs>/rudnya-069d-1 --id rudnya-069d --version 1
```

## Before a release

- An in-game look and a playtest: no 0.69d map has been played on Skhid yet (Duga was a
  local package).
- The site: a top-down render for /maps and /damage
  (`scripts/damage-calc/assets/maps/tools/build_all.py`), or `release/build-site` stops.
- Bots: no `.mapdata` and not in Player vs Bots.
