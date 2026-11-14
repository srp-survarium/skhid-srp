# STALKER FFA maps and Training Camp's fenced arena

Experimental Pool and Military Warehouses have 24 authored FFA spawns each,
Training Camp 26. Training Camp keeps players inside its base's fence (a
walker wall behind the fence, a truck in each of its three gateways), has its
FFA spawns on its raised floors (islands, the towers' stairs, cabins; two on
the ground), and gives them 500 ms of protection. Pool and
Military Warehouses have in-game panorama pictures in room tiles and loading
screens. All original TDM spawn positions are preserved. Half of Pool's and
Military Warehouses' perimeter FFA spawns (7 each) are moved to their centre
afterwards, ids kept, by `ffa-center-1` with the other maps (`ffa.md`), so
their day_8 positions are not all ffa-spawns.tsv's.

This integrates the previously unmerged `stalker-maps@6` pack, which supplies
six TDM ports. The other three are Repair Depot, Station and Depository. Only
the three requested maps receive FFA configurations. Team Deathmatch on all
six is not offered yet (`stalker-tdm-hold-1.json`; `stalker-maps.md`, "Free
for All only, for now"). The pack is immutable:
`pack.json` SHA-256 is
`ceb6ae661dda93e24f62d5c59a5cfba68cc9d4e3cd2e34d2eba1b52d99bd17cb`.
Its source and porter history are in `stalker-maps.md` and
`stalker-maps-handoff.md`.

| Map | Project | TDM / FFA dictionary IDs | Level / loading picture | Minimum horizontal separation |
| --- | --- | --- | --- | --- |
| Experimental Pool | `v010b_cs_mp_bath/day_4` | 210 / 42 | 15 / 31 | 15.52 m |
| Military Warehouses | `v010b_cs_mp_military_1/day_4` | 211 / 43 | 16 / 32 | 39.71 m |
| Training Camp | `v010b_cs_training_camp/day_4` | 212 / 44 | 17 / 33 | 9.21 m, in 3D: its points are spaced so (point 2 is 22.3 m above point 10, 2.75 m beside it) |

The remaining ports use levels 18–20 and pictures 34–36. Rudnya keeps level 14
and picture 30. The mode tables, movie classes, minimaps and ten localization
catalogs agree with these IDs. The Keeper pool includes every new project;
dictionary append-count guards include the six TDM registrations.

## Source edits and regeneration

`scripts/stalker-maps/ffa-spawns.tsv` contains positions and engine rotations.
`make_ffa_edits.py` generates `content-edit/ffa-stalker-1.json` over the source
pack and a composed dictionary that already contains its six TDM rows. The
following `ffa-stalker-radius-1.json` retunes all 74 FFA points to a 20 m enemy
radius. They have a 1 m ally radius, priority 1 and empty team collision zones.
Pool and Military Warehouses keep two seconds of spawn protection.

The FFA configs copy their final TDM configurations after table retunes, use
mode 8 and rules 93/91/94 without team rule 90, and set 30 kills and at most ten
players. This does not change main's spawn-selection algorithm, including its
five-candidate fallback when no point passes the clear test.

From `srp-026g`, using the dev shell:

```sh
cargo run --release --manifest-path scripts/stalker-maps/ffa-points/Cargo.toml -- \
  --db "$STOCK" --patch "$PATCH_12" ... --patch "$PATCH_LAST" \
  --project v010b_cs_mp_bath/day_4 \
  --project v010b_cs_mp_military_1/day_4 \
  --project v010b_cs_training_camp/day_4 --points scripts/stalker-maps/ffa-spawns.tsv \
  --plots "$OUT/plots" > "$OUT/ffa-spawns.tsv"
python3 scripts/stalker-maps/make_ffa_edits.py --pack "$PACK" --dictionary "$DICTIONARY"
python3 scripts/stalker-maps/make_truck_pack.py --pack "$PACK" --out "$SKHID_PACKS_DIR/training-camp-truck-1"
python3 scripts/stalker-maps/make_training_arena.py --native "$NATIVE_BORDER"
```

Review generated points before replacing the committed TSV. The planner loads
actual walker geometry and reachable standing floors, checks a 0.38 m radius /
1.8 m standing body, ground within 0.5 m, a six-metre walkable facing and a 2 m
margin from harmful zones. Stored rotations use Vostok's positive-yaw convention.
Pool's and Military Warehouses' points are at least 12 m apart horizontally:
they are planned from scratch, 24 each, and rerunning reproduces their rows
byte for byte.

Training Camp is not planned again. Its rows of the committed table
(`--points`) are its points: the planner loads Free for All's own project,
`day_8`, as the layers give it, and keeps a row at its place and id when it is
still fit there (inside the fence, floor, body, harmful zones, the stored
facing) and stands where a point may: on a raised floor with a way down, or,
two of them, on the ground. The ids of the others take new places (below). Run
on the table it wrote, it changes nothing. To plan after a change of `day_8`
itself (the wall, a truck), the last layer is a quick `release/build-content`
patch that has it. Its log says, for every raised point, its cheapest way down
to the ground (length, drops, jumps, the floors passed); with
`FFA_POINTS_CANDIDATES` set, it lists every safe candidate too.

## The fence, its trucks and the islands

The owner, 2026-11-13: "update training camp map to close entrances in fences
and reduce gaming area even more to just inside of the fence", "we can close
entrances with trucks that are already on the map". Free for All's own project
`day_8` only; `day_4` is never edited. Revisions 57 and 58 held players inside
the crater round the base (a wall on its rim, `training-arena.tsv`, which is
now only the outline the HUD minimap is framed on: `stalker-maps.md`).

### The fence

The base's fence is corrugated sheet about 3.2 m high (its top at y = 2.45) on
posts 5 m apart, barbed wire over it, an octagon of about 73 x 67 m. `training-fence.tsv` is its line:
the posts in order, read off the level's collision, 49 points. It is closed
but for three gateways (checked on the walk grid: with the gateways shut and
nothing else added, no floor outside is reached from inside by walking,
crouching, a 1.3 m jump or a 5 m drop):

| Gateway | Posts | Post to post | Clear | Closed by |
| --- | --- | --- | --- | --- |
| north | (6.65, 37.07), (11.85, 37.19) | 5.2 m | 3.1 m between the sheets' bent ends | a truck through it, nose in |
| north-east | (45.82, 33.41), (49.53, 29.80) | 5.2 m | 3.3 m between the bent ends | a truck through it, nose in |
| south | (19.57, -29.43), (29.60, -29.42) | 10.0 m, its two leaves open inwards | 4.8 m east of the level's own truck | the level's truck (its nose against the west leaf) and a truck beside it, askew, between its nose and the east post |

The east side of the fence lies outside the old crater wall: the strip
x 55..64.7 between them is flat ground and is now played.

### The trucks

Training Camp has one truck, a GAZ-66 with a box body, standing in the south
gateway. It is not an object of the project: the level is one mesh in chunks,
and the truck is baked into it. Its picture is the whole of render chunk
`training_camp_104` (the only surface of material `shader_0107`, texture
`veh/veh_gaz66`; 4,708 triangles); its collision is 4,227 triangles spread over
collision chunks 005, 006, 046, 054 and 055 among the ground's and the gate's.
The level's other vehicles (a BTR, a UAZ, a field kitchen) are not trucks and
are not used.

`make_truck_pack.py` makes the pack `training-camp-truck@1` from
`stalker-maps@6`'s files (each pinned by its sha256): the model
`models/srp/training_camp_truck`, that truck alone.

- Render: chunk 104's vertices and triangles, all of them.
- Collision: every triangle of the five chunks whose three corners are
  vertices of the picture (the two meshes differ by 2 micrometres of rounding
  at most; the nearest other triangle is decimetres off), with its game
  material as the level has it (`metall_thick`; `cloth` for the tyres).
- Both in the truck's own frame: the origin on the ground under the middle of
  its four wheels, +z out of its nose, +y out of its roof (the level's truck
  leans 1.9 degrees and points 17.3 degrees west of north). Normals, tangents
  and binormals turn with it. It is 7.55 m long, 2.8 m wide (3.06 m over the
  mirrors) and 3.08 m high.
- The material instance and textures stay `stalker-maps@6`'s. `pack.json`
  names each file's sha256, so the manifest's pin is of the contents too.

`training-trucks.tsv` places three (position, rotation), and
`make_training_arena.py` adds each to `day_8` twice: drawn (`render_visuals`,
after the level's own truck) and solid (`collision_objects`, walkers and
bullets as the level's collision is, after the wall). Each stands on the
ground under its wheels (the plane through the ground at the four, lowered so
that none hangs: 0 to 3 cm into the ground at the north and south, 10 cm under
two wheels at the north-east, where the ground is not flat).

| Gateway | Position | Rotation | Nose | Nearest fence or truck (3D) |
| --- | --- | --- | --- | --- |
| north | (9.183, -0.993, 38.889) | (-3.0898, -0.027, 3.1171) | south, into the base; 1.8 m inside the posts' line | 0.42 m each side of the cab |
| north-east | (48.884, -0.995, 32.87) | (3.1229, 0.7769, -3.1339) | south-west, into the base; 1.8 m inside | 0.43 m each side |
| south | (26.96, -0.816, -28.11) | (0.041, 0.5752, -0.0594) | 33 degrees west of north, 15.7 degrees off the level's truck; its nose 2 m ahead of that truck's | 0.20 m from the level's truck's front corner, 0.21 m from the east post |

A body is 0.76 m wide: none of these gaps lets one through. Under a truck
there is 0.4 to 0.9 m, less than a crouched body. With the trucks and without
the wall, the walk grid has no way out on the ground. The three ways it finds
climb a truck's nose (the north-east one, the level's own, the one beside it)
by jumps of about 1 m (bumper, bonnet, cab roof) and cross over it. The wall
is what stops those.

### The wall

`training-camp-arena-1.json` copies the stock native level-border model through
the content editor and replaces its primitives with 49 boxes, one a side of
`training-fence.tsv`'s line, 1 m thick, their middle 0.6 m outside the line:
the inner face is 0.1 m behind the fence, so a player meets the fence, never
the wall. Each spans y = -30..50 and overlaps its neighbours by 0.75 m. A
walker-only collision object references it (`cmask=4`, `cgroup=2`); bullets are
unaffected. This is game content, not a server-side map table.

In the north and north-east gateways the line runs post to post, through the
truck. In the south gateway it leaves the posts: along the open west leaf to
the level's truck, across that truck's nose (0.15 m behind its bumper's
front), then through the second truck to the east post. The pockets south of
it (between the west leaf and the level's truck, and between the two trucks)
are outside: a player who had climbed a cab and dropped into one could not
have left it.

On the built `day_8` (the walk grid of every floor, flooded from the 26
spawns): no floor outside the line is reached. A player who climbs a truck's
nose stands on its cab against the wall. The raised floors (1,812 m² above
6 m: three towers, the hub, catwalks, the islands) are reached exactly as
before, all inside; the nearest is 1.34 m from the fence (the north-east upper
island). The dead ends the grid has are the old ones, none new, left only by a
jump down: a hollow inside the west island's body, under its top (79 m² of
floor, which this grid enters by one 2.4 m drop), and a low island by the
north-west tower (21 m²).

### The spawns

The base's raised floors, 6 to 30 m over its ground, 1,726 m² of them a
walker reaches standing:

- three towers, hollow concrete silos with a flat top at y = 28.5. A wooden
  walkway of steps winds round each from the ground to its top, a second
  flight of stairs winds inside it, and doors with landings join the two;
- two cabins on each tower, round wooden huts with 2.75 m of standing floor
  across: the lower one hangs by the walkway at y = 15.5, the upper one
  stands a short bridge from the top, at y = 28.4;
- floating islands with ground on top. A walker reaches eight of them, over
  catwalks and bridges from the towers (the table below). The big one in the
  middle, under the hub (y = 14 to 17), and the other small ones are reached
  by no floor;
- the hub's platform at about y = 29, over the middle island, where the
  catwalks of the three towers meet.

Two tables name the floors a spawn may stand on, reviewed from the planner's
standing floors and from renders of the level: `training-islands.tsv` (a box
and a floor height range for each island) and `training-towers.tsv` (a circle
and a height range: a tower's stairs, from 6 m up to under its top, and each
cabin's floor). A place is a cabin, else an island, else a tower's stairs. The
towers' tops, the catwalks, the bridges and the hub's platform are in neither.

Revision 57 had 24 points spread over the crater. Revision 58 moved every other
perimeter one inside the fence and added five on the islands, 29 (the owner,
2026-11-13: "set more spawns on training camp on islands", "remove half spawns
on perimeter", "put them in the center"). The fence, this change's first step
(never released), took the seven that stood outside it: 26 points, ids 0 to
25, 19 of them on the ground inside the fence and 7 raised.

The owner on those, 2026-11-14: "make sure all spawns are on islands or on
ladders or in rubkas", "so that people don't only spawn on ground" (the
ladders are the towers' stairs, the rubkas their cabins); then "some can be on
ground, but only a few", "and very rare". As it is read here (the numbers are
not the owner's): two points stay on the ground, every other one stands on a
raised floor of the tables, 26 if they fit 8 m apart, fewer rather than
nearer. Spacing is in 3D for every pair: a spawn on the ground under a raised
floor does not take it (the owner, 2026-11-13), and up there "it is fine for
players to spawn close".

The planner, on `day_8`:

1. A row on a raised floor keeps its place and id when it is still fit and
   its floor has a way down: the seven did. Of the rows on the ground, the two
   farthest apart stay, 3 and 9 (73 m, the west side and the east strip).
2. A new place is a standing floor of the tables, reached from the spawns and
   2 m inside the fence, that passes every point's checks (the body, harmful
   zones, a level 6 m walk ahead: on the stairs that leaves their landings and
   flat runs) and has a way down: the ground (a floor under y = 1 inside the
   fence) is reached from it on the walk grid, by walking, crouching, a 1.3 m
   jump or a drop of 5 m at most. A floor a walker cannot leave alive is no
   place.
3. The ids of the other 17 ground rows take the new places, the lowest id
   first: the cabins first (they are the smallest floors, and the place
   farthest from every point is never in one), then the place farthest from
   every point so far, while one stays 8 m from them all.
4. Ids no place is left for go, the last points taking them: spacing before
   count, and ids stay 0..n without a gap (the game takes a point by its id; a
   config with a hole has none to give). All 17 had a place.

26 points, ids 0 to 25: 2 on the ground and 24 raised, 11 of them on islands,
10 on the towers' stairs (6 on a landing of the walkway outside, 4 on the
stairs inside) and 3 in cabins. The nearest two are 2 and 20, 9.21 m.

| Id | Place | Stands on | Over the ground | On foot to the ground, walking only | Before |
| --- | --- | --- | --- | --- | --- |
| 0 | (17.185, 15.548, -20.845) | cabin, the lower one of the south-west tower | 16.3 m | 35 m: the south-west tower's stairs | ground (19.9, 6.2) |
| 1 | (4.685, 15.548, 15.905) | cabin, the lower one of the north-west tower | 16.3 m | 35 m: the north-west tower's stairs | ground (54.2, 21.2) |
| 2 | (47.435, 28.397, -6.845) | cabin, the upper one of the east tower | 29.2 m | 70 m: a catwalk, the east tower's top, the east tower's stairs | ground (27.2, -20.8) |
| 3 | (-6.565, -0.828, 21.155) | the ground |  |  | here |
| 4 | (7.685, 6.288, -8.095) | stairs of the south-west tower, its walkway outside | 7.1 m | 14 m down its stairs | ground (42.2, -26.8) |
| 5 | (45.935, 26.302, 18.905) | island north-east-small | 27.1 m | 65 m: a catwalk, the east tower's top, the east tower's stairs | ground (15.2, -5.8) |
| 6 | (6.185, 10.977, 31.655) | stairs of the north-west tower, its walkway outside | 11.8 m | 23 m down its stairs | ground (57.2, 6.2) |
| 7 | (5.185, 25.475, -18.595) | stairs of the south-west tower, inside it | 26.3 m | 50 m down its stairs | ground (15.2, 36.2) |
| 8 | (30.435, 21.321, -23.845) | island south-east | 22.1 m | 81 m: a catwalk, the south-west tower's top, the south-west tower's stairs | here |
| 9 | (60.185, -0.847, -8.845) | the ground |  |  | here |
| 10 | (47.435, 6.071, -4.095) | stairs of the east tower, its walkway outside | 6.9 m | 14 m down its stairs | ground (39.2, 24.2) |
| 11 | (52.685, 11.004, 9.155) | stairs of the east tower, its walkway outside | 11.8 m | 24 m down its stairs | ground (6.2, -14.8) |
| 12 | (-4.315, 22.158, 9.155) | island west | 23.0 m | 92 m: a catwalk, the south-west tower's top, the south-west tower's stairs | here |
| 13 | (28.185, 17.926, 18.655) | island north-east-lower | 18.7 m | 100 m: a catwalk, island north-east-upper, a catwalk, the north-west tower's top, the north-west tower's stairs | here |
| 14 | (6.935, 25.514, 27.905) | stairs of the north-west tower, inside it | 26.3 m | 50 m down its stairs | ground (-2.8, 6.2) |
| 15 | (3.435, 11.167, -21.595) | stairs of the south-west tower, its walkway outside | 12.0 m | 24 m down its stairs | ground (-8.1, -8.8) |
| 16 | (0.935, 27.895, 18.155) | island north-west-small | 28.7 m | 64 m: the north-west tower's top, the north-west tower's stairs | ground (24.2, 24.2) |
| 17 | (39.935, 20.345, -17.095) | island south-east | 21.1 m | 90 m: a catwalk, the south-west tower's top, the south-west tower's stairs | ground (47.9, -12.1) |
| 18 | (48.185, 20.970, 0.905) | stairs of the east tower, inside it | 21.8 m | 41 m down its stairs | ground (30.2, -5.8) |
| 19 | (38.685, 16.222, 6.155) | stairs of the east tower, its walkway outside | 17.0 m | 34 m down its stairs | here |
| 20 | (50.435, 21.864, -12.595) | island east | 22.7 m | 53 m: a catwalk, the east tower's stairs | here |
| 21 | (11.435, 20.306, -12.595) | stairs of the south-west tower, inside it | 21.1 m | 38 m down its stairs | ground (12.2, -26.8) |
| 22 | (-4.565, 24.342, -0.345) | island west | 25.1 m | 85 m: a catwalk, the south-west tower's top, the south-west tower's stairs | ground (8.4, 21.2) |
| 23 | (37.685, 22.148, 26.655) | island north-east-upper | 22.9 m | 93 m: a catwalk, the north-west tower's top, the north-west tower's stairs | ground (30.2, 36.2) |
| 24 | (4.685, 24.885, -2.845) | island west | 25.7 m | 75 m: a catwalk, the south-west tower's top, the south-west tower's stairs | here |
| 25 | (31.435, 22.507, 33.905) | island north-east-upper | 23.3 m | 87 m: a catwalk, the north-west tower's top, the north-west tower's stairs | here |

"Walking only" is the way with no drop and no jump on the walk grid of every
floor of the built `day_8`; the planner's cheapest way, which takes drops of
up to 5.0 m as short cuts, is 13 to 86 m. "Before" is where the id stood
after the fence's first step.

| Raised floor | Floor y | Standing floor | Safe places | Points |
| --- | --- | --- | --- | --- |
| island west | 22.2-25.5 | 122 m² | 233 | 12, 22, 24 |
| island north-east-upper | 21.8-23.1 | 68 m² | 113 | 23, 25 |
| island north-east-lower | 17.8-19.0 | 54 m² | 112 | 13 |
| island south-east | 20.1-22.2 | 115 m² | 202 | 8, 17 |
| island east | 21.4-22.1 | 47 m² | 101 | 20 |
| island north-west-small | 27.5-28.1 | 26 m² | 28 | 16 |
| island north-east-small | 26.2-27.5 | 27 m² | 15 | 5 |
| island north-west-low | 15.4-16.1 | 21 m² | none: no way down | |
| the north-west tower's stairs | 6.0-27.5 | 228 m² | 37 | 6, 14 |
| the south-west tower's stairs | 6.0-27.5 | 228 m² | 34 | 4, 7, 15, 21 |
| the east tower's stairs | 6.0-27.5 | 230 m² | 46 | 10, 11, 18, 19 |
| the six cabins | 15.5 (lower), 28.4 (upper) | 6 m² each | 31 | 0, 1, 2 |
| tops, catwalks, bridges, the hub's platform | 15.5-30.3 | 522 m² | in no table | |

The east island's box ended at the tower before and held its bridge and a
piece of the tower's walkway; it ends at the island now. Three cabins have no
point: the east tower's lower one is 4 m from point 19 and the south-west
tower's upper one 6 m from point 24, which stay, and the north-west tower's
upper one has no place that passes the checks.

No spawn stands where a walker cannot get down. The north-west-low island, by
that tower's lower cabin, is such a place: it is in the table so that the
rule, not its absence, says so. The other dead end of the fence's check is no
part of the west island's top but a hollow under it (`### The wall`), which
the planner's grid does not enter: the island's three points stand on its
top, which a catwalk joins to the south-west tower.

| Training Camp | Outside the fence | Ground | Raised | Total |
| --- | --- | --- | --- | --- |
| Revision 57 | 14 | 8 | 2 | 24 |
| Revision 58 | 7 | 15 | 7 | 29 |
| The fence, first step (not released) | 0 | 19 | 7 | 26 |
| This change | 0 | 2 kept (3 and 9) | 24: 7 kept, 17 new | 26 |

Two of the 26 points are on the ground, 7.7 %. How often a spawn lands on one
is not that share: the game takes any point with no other player within 20 m
(`ffa.md`), and the two ground points, at the base's west and east edges, are
the ones most often free. The crowding model of `ffa.md` (offline, not play;
40,000 respawns a case; the 20 m taken in 3D, which is assumed of the game's
test), one player respawning while the N - 1 others live, N = 10 / 16:

| The others stand | A spawn lands on the ground | No point is safe (the fallback) |
| --- | --- | --- |
| on the ground, anywhere inside the fence | 2.6 % / 1.1 % (19 + 7 before: 29 % / 12 %) | 0 / 0 (0 / 0) |
| half so, half on random spawn points | 10 % / 12 % (41 % / 24 %) | 0.0 % / 2.6 % (0.0 % / 0.1 %) |
| on random spawn points, all of them | 30 % / 47 % (53 % / 40 %) | 0.3 % / 9.6 % (0.0 % / 0.6 %) |
| anywhere in the points' box, heights ignored (`ffa.md`'s B) | 17 % / 19 % (81 % / 82 %) | 12 % / 53 % (5 % / 40 %) |

So a ground spawn is rare while players are on the ground, and common when
they all stay up by the spawns: the raised points, 9 m and more apart, share
the enemies near them, and the ground points are what is left. The same makes
a respawn with no safe point likelier than before at 16. With the same 24
raised points and the two ground points nearest the middle (0 and 18)
instead, the half-and-half case gives 4 % / 3 % on the ground and
0.1 % / 4.8 % with no safe point; with no ground point, 0.4 % / 7.4 % with no
safe point.

`day_8` still carries Team Deathmatch's 24 points, as every Free for All copy
does; Free for All takes team 2's only. Most of them are at the gateways,
outside the wall, and one (team 0's point 2) is where the south gateway's
second truck stands.

Nothing else of the mode is outside: `day_8` has no zones, crates, batteries or
stations; its four campfires are inside the fence; its one anomaly is the
Keeper's hidden pool at y = -1000.

## Millisecond protection

The arena edit sets `invulnerability_time_ms = 500` on Training Camp's 26 FFA
points, with legacy seconds zero. Original TDM timing is unchanged.

The client hooks `respawn_point_core::load` (0x2F4580) at 0x2F4628, before the
native seconds-to-milliseconds conversion. Safe Rust parses a nonnegative
signed-range integer once, writes the existing +0x20 duration and takes the
generated exit to 0x2F4633. Missing or malformed optional values retain native
seconds behavior. This adds no serialized field. The site, field, generated
hooks, arena lock and re-kit ABI evidence accompany the change.

The content and client hook must ship together (they did, in revision 57). A
`next` simulation-change row records the fence's wall, its trucks and the
points. No feature revision is reserved.

## Validation and local playtest

`tools/check stalker-maps` checks generated edits against the source pack,
unchanged TDM data, native field types, dictionary preservation, Training
Camp's points (each on a floor of the two tables as its row says, two on the
ground, 8 m apart in 3D), the closed wall behind the fence and the trucks (drawn and solid at their rows'
places, the pack made again and compared with the manifest's pin), then tests
and lints the planner. `build-content`
checks full manifest composition and actual dictionary registration; `ffa-native`
covers the native timing loader and existing FFA/late-join/world checks.

### The fenced arena (2026-11-13)

Not played, and not seen in the game: every check below is offline, on a quick
`build-content 59` of that day, when 19 of the 26 points stood on the ground
(the next section moves them), and the pictures are software renders of the
files (`private/renders/training-camp-fence/` of the main checkout).

- The patch: the truck model's seven files, the border's primitives and
  `day_8`. Read back: `day_4` byte-identical to live; in `day_8` the 24 Team
  Deathmatch points and everything but the three lists unchanged; team 2's
  ids 0 to 25 at the table's places with 20 m and 500 ms, 19 of them the live
  records, ids 12, 13 and 20 the live 28, 27 and 26 but for name and id; three
  truck visuals and three truck collision objects (`cmask=20`, `cgroup=10`,
  as the level's) after the wall's one; 49 wall boxes.
- The planner on that patch, from revision 58's table: the committed table,
  byte for byte. From the committed table: all 26 kept.
- The walk grid of that `day_8` (every floor; walking, crouching, 1.3 m jumps,
  5 m drops), flooded from the 26 spawns: 89,927 floors, none outside the
  fence's line; the same 28,987 raised floors and the same 1,599 dead-end
  floors as on live.
- `the_released_free_for_alls_play_on_their_own_projects` on its server game
  data: map 44 has 26 solo points on its own project.

What a walk grid and a software render cannot show is left for the game: how
the trucks sit and are lit, whether a cab can be climbed (the grid's jump is a
guess of 1.3 m), and the engine's own collision with the truck's mesh.

### The raised spawns (2026-11-14)

Not played, and not seen in the game: every check below is offline, on a quick
`build-content 59` of this branch with main (`261e3411b`) merged in, and the
pictures are software renders of the files
(`private/renders/training-camp-raised-spawns/` of the main checkout).

- The build passes. `day_8` read back with the resource porter's config reader
  (not the edit tool's): team 2's ids 0 to 25 at the table's places and
  facings, 20 m, 1 m and 500 ms each; the 24 Team Deathmatch points and every
  other value as the same tree builds them without this change (17 points'
  position and rotation differ, nothing else). `day_4` is byte for byte that
  build's: six environment values of main's lists (#1793) apart from live,
  and no list of this change names it.
- Each of the 26 against the built collision, apart from the planner (rays on
  the walker triangles, the walk grid of every floor): collision within 5 cm
  under its feet, 1.8 m clear over it, the body's 64 side rays clear, a
  standing floor, and by the two tables the floor its row says.
- The way down: from each of the 24 raised points the ground is reached by
  walking alone, no drop and no jump, in 14 to 100 m (the table); so it is on
  the fence's check's grid too, whose cells lie 0.1 m aside.
- The planner on that patch, from the committed table: all 26 kept, the table
  byte for byte. From the table of the fence's first step, on a build of this
  tree with those points: this table. Experimental Pool's and Military
  Warehouses' rows byte for byte both times.
- The walk grid of the fence's check (`### The wall`), flooded from the new
  spawns: the same 89,927 floors, none outside the fence's line, and the same
  1,599 dead-end floors, none with a spawn.
- `the_released_free_for_alls_play_on_their_own_projects` on its server game
  data: 33 maps, map 44 with 26 solo points on its own project.

Left for the game: how a spawn inside a tower, on a walkway's landing and in a
cabin looks and feels (light, the first step out), whether the steps of the
stairs walk as the grid's 0.6 m step says, and the game's own clear test with
players above and below a point (taken here as 20 m in 3D).

### The crater arena of revision 57

The tested local content was read back from the built archive: exact authored
positions/facings, all 72 IDs, 20 m radii, Training-only 500 ms and unchanged TDM
points. Both panorama JPEGs appeared byte-for-byte in the lobby movie. The
loaded walker mesh blocked 1,200 crossing rays at segment interiors and joins,
at five heights and in both directions; all 48 Training spawns were inside.

The owner played the corrected build in two 1280×720 windows on 2026-11-12 and
approved merge. The initial fullscreen run exhausted available GPU memory;
windowed mode displayed the lobby and match. This establishes a local playtest,
not exhaustive balance testing or a release. Integration onto current main
preserves its newer FFA fallback and lobby changes; its check results are
recorded in the PR.

The completed local Training Camp match is in the private world corpus at
`local/stalker-ffa-crater/rec026g_20261112-202431_2533279d.srprec` (45,082 bytes,
SHA-256 `1a81bf6c94e558b79fec02c98aacbf8bef4af601d7646115eeef2e8603c2de7c`).
The server logged the completed recording after both updated clients entered;
all 15 saved worlds passed the combined-tree server check. The native/host
comparison agreed on all 1,044 sampled worlds and their mutated copies on the
integration with main `4bebbb538`. Main's later executable-size precheck
(`6baf30627`) was integrated and the arena relinked; focused final native and
Rust results are recorded in the PR.

The combined-main quick content build passed (`tools/check build-content`,
key `59385548a82b`). Its archive SHA-256 is
`c430744415f0bcba6e5c6019b2d647e57703d0872ab8f92f30c9ecf6a1035f44`;
server-data SHA-256 is
`0f57d94b704dae55a3598c52cf150aed520e3bb7e718ce09bcd69a4d219c371b`.
Readback and the 1,200 walker-crossing checks also passed on this archive.
These are local test artifacts, not a release.

Two unrelated checks also fail on untouched main (`ffe92a0ba`): the localization
harness lacks Calibri's infinity glyph for the existing pocket-units string,
and HUD-options fails the existing `configs.start` registration assertion.
The same failures were reproduced in a clean detached baseline checkout;
no validation rule was loosened.

The full historical corpus is not green: 22,705 of 44,233 worlds passed and
21,528 were refused, mostly for older respawn counts plus invalid object states.
The additional full baseline comparison was stopped after resource contention;
these corpus failures remain a validation limit, not a claimed baseline pass.
The new Training Camp recording has no refused worlds.
