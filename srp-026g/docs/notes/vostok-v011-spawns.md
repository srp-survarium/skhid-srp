# Vostok v0.11: v0.26g spawn layout

The owner requested the v0.26 layout in v0.11 on 2026-11-04, with an explicit
check for coordinate differences. `vostok-v011-spawns-1.json`, after `maps.tsv`
in the content manifest, changes `server_objects/respawn_points` in
`projects/v011e_level_03/day_2/client_project` and `day_4/client_project`.

| Project | Before | After (stock level_03) |
| --- | --- | --- |
| day_2, Protective Device | 43 (21 team 1, 22 team 0) | 28 (14 each) |
| day_4, Team Deathmatch | 43 (21 team 1, 22 team 0) | 22 (11 each) |

Positions, full Euler rotations, priority (1), team order and point IDs match
the corresponding stock v0.26g project exactly as float32 values. Existing
records are reused within each team; surplus records are removed. Names and
other record properties stay, including old zone references, six-second spawn
protection and the absence of ally/enemy check-radius keys. No collision,
render, objective or base-zone objects are moved. The `next` simulation entry
records the changed spawn selection and respawn-rule size.

## Coordinate check

The source is stock `resources.db`; the target is the published v0.11 port
through base patch 23 (all manifest base layers mounted for collision models).
443 day_2 render placements have the same asset name (ignoring the port prefix),
position, rotation and scale in both maps. The minimaps' 0.23 m framing difference
is not a world-coordinate offset.

Using mapdata's walker collision triangles, all 50 source positions across both
modes have a floor in v0.11. The greatest difference between source and target
floor heights is 0.000219 m. Every point locates a walkable grid node within
0.3 m horizontally, on a 0.125 m grid with the default player body. Sampled
horizontal clearance (16 directions at four heights) is at least 0.410 m;
upward rays at the centre and eight points on a 0.38 m radius find at least
2.895 m headroom. These are offline geometry checks, not a native-client
capsule sweep or a claim of completed playtesting. No offset was applied.

A binary edit/readback check confirmed exact source positions, rotations,
teams, IDs and priority, retained six-second protection, and semantic equality
of every other project field. The owner will inspect the positions in a local
room with 19 idle accounts before release.

`release/build-content 46 --out /tmp/vostok-spawn-review/content` passed on
2026-11-04 (local validation only; no revision reserved or released). The
full-manifest output also passed the spawn readback and unrelated-field checks.
