# STALKER FFA maps and Training Camp's fenced arena

Experimental Pool and Military Warehouses have 24 authored FFA spawns each,
Training Camp 26. Training Camp keeps players inside its base's fence (a
walker wall behind the fence, a truck in each of its three gateways), with
its raised islands, and gives its FFA spawns 500 ms of protection. Pool and
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
| Training Camp | `v010b_cs_training_camp/day_4` | 212 / 44 | 17 / 33 | 12.67 m; 3D from island points (point 20 is 22.7 m above point 17) |

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
Points are at least 12 m apart horizontally. Pool and Military Warehouses are
planned from scratch, 24 each, and rerunning reproduces their rows byte for
byte.

Training Camp is not planned again. Its rows of the committed table
(`--points`) are its points: the planner loads Free for All's own project,
`day_8`, as the layers give it, keeps every row that is still fit there (inside
the fence, floor, body, harmful zones, the stored facing) at its place and id,
and gives the ids of the others new places (below). Run on the table it wrote,
it changes nothing. To plan after a change of `day_8` itself (the wall, a
truck), the last layer is a quick `release/build-content` patch that has it.

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
6 m: three towers, the hub, catwalks, five islands) are reached exactly as
before, all inside; the nearest is 1.34 m from the fence (the north-east upper
island). The dead ends the grid has are the old ones, none new: parts of the
west island and of a platform by the hub that are left only by a jump down.

### The spawns

The centre has elevated walkways 15-30 m above the ground: three towers, a
central hub, catwalks between them, and five flat raised platforms, the islands
of `training-islands.tsv` (a box and floor height range each, reviewed from the
planner's standing floors). Spacing is in 3D when either point is on an island,
so a spawn on the ground under an island does not take it (the owner,
2026-11-13).

Revision 57 had 24 points spread over the crater. Revision 58 moved every other
perimeter one inside the fence and added five on the islands, 29 (the owner,
2026-11-13: "set more spawns on training camp on islands", "remove half spawns
on perimeter", "put them in the center"). Seven of those 29 stood outside the
fence: 1, 3, 5, 9, 12, 13 and 20 (6, by the crater wall then, is inside it).
The planner gave their ids new places inside the fence, 2 m or more from its
line: the candidate farthest from every point so far, while one stays 12 m from
all of them. Four fit. Spacing goes before count (the owner): three ids had no
place, and ids stay 0..n without a gap (the game takes a point by its id; a
config with a hole has none to give), so the last three points took them,
standing where they stood. 26 points, ids 0 to 25.

| Id | Was | Is |
| --- | --- | --- |
| 1 | (60.2, -32.6), south-east of the fence | (54.185, -0.771, 21.155), the north-east corner |
| 3 | (42.2, 51.2), north | (-6.565, -0.828, 21.155), the west side |
| 5 | (-24.1, 6.9), west | (15.185, -0.829, -5.845), west of the hub |
| 9 | (-17.6, -23.8), south-west | (60.185, -0.847, -8.845), the east strip |
| 12 | (-8.8, 27.2), 0.2 m outside the north-west corner | the former 28: `(-4.315, 22.158, 9.155)`, west island |
| 13 | (9.2, 57.2), north | the former 27: `(28.185, 17.926, 18.655)`, north-east lower island |
| 20 | (24.2, -44.6), the south road | the former 26: `(50.435, 21.864, -12.595)`, east island |
| 26, 27, 28 | the islands above | gone (now 20, 13, 12) |

The other 19 are where they were. Three of them stand within 2 m of the fence
(7 at 1.0 m, 15 at 0.6 m, 23 at 1.0 m): the 2 m rule was for the unseen crater
wall and is kept for new places; these face away from a fence a player sees.

| Island | Floor height | Standing area | Points |
| --- | --- | --- | --- |
| west | 22-25 m | 122 m² | 24 `(4.685, 24.885, -2.845)`, 12 |
| north-east upper | 22-23 m | 68 m² | 25 `(31.435, 22.507, 33.905)` |
| north-east lower | 18-19 m | 54 m² | 13 |
| south-east | 20-22 m | 115 m² | 8 `(30.435, 21.321, -23.845)` |
| east | 21-22 m | 66 m² | 20 |

| Training Camp | Outside the fence | Ground inside | Raised | Total |
| --- | --- | --- | --- | --- |
| Revision 57 | 14 | 8 | 2 | 24 |
| Revision 58 | 7 | 15 | 7 | 29 |
| This change | 0 | 19 | 7 | 26 |

Point 19 is on the east tower's ramp. The nearest two are 9 and 17, 12.67 m.
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
unchanged TDM data, native field types, dictionary preservation, island spawns,
the closed wall behind the fence and the trucks (drawn and solid at their rows'
places, the pack made again and compared with the manifest's pin), then tests
and lints the planner. `build-content`
checks full manifest composition and actual dictionary registration; `ffa-native`
covers the native timing loader and existing FFA/late-join/world checks.

### The fenced arena (2026-11-13)

Not played, and not seen in the game: every check below is offline, on a quick
`build-content 59`, and the pictures are software renders of the files
(`private/renders/training-camp-fence/` of the main checkout).

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
