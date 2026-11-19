# London (level_09)

Skhid's London is 0.31a2's, ported in base patch 14; Research's day_5 comes from
research-mode@3. The owner's rules:

- No anomalies, damage zones or artefacts in any mode.
- Use the owner's map-editor spawn placement: ten points per team, the same in all four modes.
  Team1 (server team 0, west) is on building_06_1's first floor, placed to match team2;
  team2 (server team 1, east) is on building_10's first floor, the owner's eight points plus
  two beside them. Preserve a point's facing when it was moved without turning it.
- These replace the earlier proposal beside the ground-floor doors. The walking distance to
  the battery container is 44–65 m for team1 and 68–74 m for team2.
- Remove the old outdoor spawn-protection zones; keep each point's own 6-second invulnerability.
- Do not include the accidental prop moves from the editor.

Source and checks: `scripts/content-edit/london-1.json` and its README section. The server data,
bots' generated map files and website maps must be built from the same released content.

## Rain and evening (pack london-weathers, maps 276-281)

London's donor is 0.31a2 (`resources.03.db` of its 22 volumes holds `projects/level_09`): it ships
`day`, `evn` and `rain`, each in modes 2 to 6. No earlier build on disk has London (0.27d0, 0.28a2,
0.28d0 have levels 1-8); 0.46c2 has it again under other project names. Skhid's day is 0.31a2's
`day_2/3/4` as patch 14 converted it (not made in these repositories) with skhid's edits on top.

| Weather | Protective device | its chargers | Team Deathmatch | loading picture |
| --- | --- | --- | --- | --- |
| rain (`rain_N`, weather 2) | 276 | 277 | 278 | 28 (`Level9_rain`) |
| evening (`evening_N`, weather 3) | 279 | 280 | 281 | 27 (`Level9_evn`) |

Each weather project is the shipped day with 0.31a2's own day-to-weather difference laid over it
(resource-porter `src/weather_delta.rs`, `examples/weather-delta-pack.rs`, its manifest
`/mnt/hdd/Projects/london-weathers/london-weathers.json`):

- client objects matched by `full_name`: 149 day ambient lights give way to the weather's 163/164,
  160 probes take the weather's cubemaps (`texture` and `textures.<project>`), the lights' colours
  and ranges change; rain drops the sand and cloud particles, evening adds two cloud layers, two
  ground sand streams, one probe and one spot light; a camera `track` and the two
  `sound_environment`s have no 0.26g object and stay out;
- render visuals and collisions matched by content within 1 mm: the stand lamps become lit ones;
  `srp_lamps`' visual indices follow;
- fields skhid edited on the day (five probes' radius and side width) keep the edit;
- the environment: the shipped day's settings plus native 0.26g `level_01` rain/evening's, each set
  from 0.31a2's first key frame, as patch 14 made the day's from native `level_01/day_2`'s names.
- `mie_fog_base_height` is 0.31a2's (100 rain, 50 evening) plus 2^-10: London's tone-curve marker
  (three-maps-021-lighting@9), which no authored height has; the fog it drives does not change.

The conversion rules are patch 14's own, read off the shipped day: `is_enabled` is a light's
`enabled` and is dropped elsewhere, an ambient light's colour is packed ARGB (`round(c*255)`), a
converted object takes the field set of a shipped object of its kind. Converting 0.31a2's day with
them gives the shipped day back except where skhid edited it (decal and probe placements, rotations
written in another form).

The source day is what players have (patch 51's London projects): revision 66's full content build
found every London day file unchanged against the manifest, so it is the manifest's own output.
Six new maps raise the pinned `maps_dict` counts of five edit lists by six; a PR adding maps after
this one renumbers again.
