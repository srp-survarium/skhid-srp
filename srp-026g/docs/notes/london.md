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

## Campfires burn (the owner, 2026-11-15)

"Fireplaces which don't do damage" (`2026-11-15-20-43-24.png`, the campfire by the skeleton). London
has two lit bonfires (`props/light/bonefire_02_on`, each with its `fx/lights/bonfire` particle), at
(0.88, 14.06, 18.73) and (14.04, 10.35, 4.87), the same in its five projects. The port never had a
damage zone for them (its seven zones were the anomalies', which `london-1` removed).

`port-campfires-1.json` (`scripts/port-fires/make_edits.py`, from the stock
`projects/level_05/day_4/client_project` and London's projects as the manifest leaves them) appends
what Training Camp got (`stalker-maps.md`, `training-camp-fires-1`): level_05's
`logic/fire_damage/fire_collision`, here two 0.8 m spheres, one 0.16 m over each bonfire, and
`logic/fire_damage/fire_damage_zone` naming it (fire, 0.1 to both hips and both feet every 500 ms).
The game's client applies the damage in its own simulation; the server needs no code. Each world
saves one damage zone more, so it is a `next` simulation change.

The nearest spawn is a Free for All point 6.9 m from a fire; the nearest battery spawner 4.9 m.
London's nine barrel fires (eleven in `day_3`) and four skyscraper fires stay harmless: they are the
same flame particle over a barrel or over nothing, and no barrel or stove burns in the stock maps.

The 0.21 Mamayev Kurgan (`v021d_level_06`) has the same gap on seven bonfires. It is not in the list:
one of them, at (-46.9, 17.94, 64.94), stands 0.98 m from `respawn_point(6)`, and is the one the stock
map leaves without a zone.

Not played in a game.
