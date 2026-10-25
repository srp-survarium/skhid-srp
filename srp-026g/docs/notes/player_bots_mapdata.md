# Player bots: map data (0.26g)

The map knowledge server-driven bots need (`player_bots.md` §5), built as
the crate `crates/mapdata`: spawns and objectives, a walk grid with paths,
and line of sight, from the map projects in the game's `resources.db`. Tags
as elsewhere: PROVEN, LIKELY, GUESS. The match server loads the files
(`SRP_MAPDATA_DIR`, §6); no bot moves by them yet.

## 1. Dumping

The game's data never goes into this repository. A server loads one file
per project, written once from a `resources.db`:

```text
cargo run --release -p mapdata --bin mapdata-dump -- --db <resources.db> --out <dir>
cargo run --release -p mapdata --bin mapdata-dump -- --db <resources.db> --out <dir> --map 117 --map 87 --stats 0.25
```

- `--db` may come from `SRP_RESOURCES_DB`.
- No `--map` / `--project`: every playable project (72; the table is
  `survarium::v026g::maps`). `--map <id>` takes a `maps_dict` id,
  `--project level_01/day_4` a project.
- Output: `<dir>/level_01_day_4.mapdata` etc. 7.1-16.3 MB a project,
  817 MB for all 72. All 72 in 6.4 s with a warm disk cache (format 2:
  7.2 s, the same size within 0.1 %).
- `--stats <cell>` also builds the grid and the sight tree and prints
  counts and timings (§5).

For drawing (heat maps of saved worlds), `mapdata-layout --out <dir>
<file.mapdata>...` writes a top view per project: the top height and the
number of floors per 0.5 m cell, of the floors a spawn reaches only
(level_02 has an invisible walkable ceiling at 48 m over the whole map),
and the objects as CSV. Its output is game data as well.

Mode variants of one map place different props (level_01: 1157 / 1159 /
1015 collision objects), so a match loads the file of its own project.

## 2. Facts the crate rests on

- **Frame.** The map frame: left-handed, Y up, Z forward, positions at the
  feet. The physics frame is the map frame with z negated; the uploaded
  worlds' `m_last_pos` is in it, at the capsule's centre 0.9 m above the
  feet. Type `PhysicsPoint` converts. PROVEN (`player_bots.md` §4).
- **Placement.** World point = (local point × the `#[sx][sy][sz]` scale of
  the library name) × `create_rotation(rotation)` + position, row vectors,
  `create_rotation` = Rx·Ry·Rz (unit test). The object's own `scale` is not
  applied. All 43 spawns of level_01/day_4 stand -0.036..+0.120 m above
  the walker mesh this way, those of level_05/evening_2 within 0.15 m
  (tests on the game data). PROVEN.
- **Groups.** The walking body (group 4, mask 2) meets objects with
  `cgroup & 2`, a bullet (16, 8) `cgroup & 8`; an object meets a body when
  `cgroup & mask` and `cmask & group`. PROVEN.
- **Hard links.** Many meshes are hard links in the archive's index. The
  `resources-db` listing resolves a link to its target's payload under the
  link's own path; links to links would be counted in
  `Resources::unresolved_links` (0 in the shipped archive). PROVEN.
- **Walker behaviour** per face: `face_data` names a shading group, the
  model's `settings` give its game material, `game.materials` its
  `physic.walker_behaviour`: 0 ground up to 60°, 1 (`walk`) always ground,
  2 (`dont_walk`) never. Values and slope LIKELY (`player_bots.md` §5).
- **Primitives** (`exported_primitives`: box, sphere, cylinder, capsule)
  are turned into triangles (box exact, round shapes 12 sides × 6 rings),
  with the material of their `mtl`.
- **Body** (`grid::Body`): radius 0.38, stand 1.8, crouch 1.1, step 0.6
  (PROVEN); ground walked up to 50° (PROVEN, `impassable_slope`), faces up
  to 60° floors, climbs up 50-60° ground cut past a step; drop ≤ 5 m
  (`fall_params.harmless_height`); jump ≤ 1.05 m (LIKELY, from the live
  logs; was 1.3 m, GUESS). Since 2026-10-23, `player_bots_stuck.md`.
- **A missing `walker_collision`** (fruit trees, level 3's anomalous
  trees, a bench, a wheel: 0-60 placements a map) is stood in for by the
  model's `collision` for the walking body (`MapData::triangles`, LIKELY:
  `player_bots_stuck.md`).
- **Play area.** The walker mesh reaches far off the map (to -666 m); the
  grid covers the `levelmap` x/z box, widened by 5 m and to hold every
  spawn and objective. level_03's box is 2 m and level_06, 07 and 08 have
  none: there the area is the middle 96 % of the collision objects'
  positions with the objectives, widened by a further 15 m (with the
  objectives alone, level_06 mode 3 lost the ground between the bases).

## 3. The file

One per project, little endian; strings `u32 length` + UTF-8, lists
`u32 count` + items, vectors 3 × f32. The libraries are stored once each in
their own space (scale suffix applied), the placements separately; placing
all triangles at load takes about 10 ms.

```text
magic "SRPMAPD\0", u32 version (2)
string project
vec3 levelmap_min, vec3 levelmap_max          (zero when the project has none)
list spawns:      vec3 position, vec3 rotation, i32 team, i32 priority,
                  i32 point_id, string layer
list containers:  vec3 position, i32 team, i32 id, list shapes (use boxes)
list spawners:    vec3 position, vec3 item, string layer
list stations:    vec3
list zones:       u8 kind (0 resupply, 1 damage, 2 effect, 3 artefact
                  container), string name, i32 team (-1 none), vec3 position,
                  string damage_type, u8 has_damage, [damage], list shapes
list anomalies:   string name, vec3 position, list string zones,
                  energy, artefacts, list states
list libraries:   string name, u32 primitives, list vec3 vertices,
                  list [u32; 3] triangles, then one u8 walker behaviour
                  per triangle
list placements:  u32 library, vec3 position, vec3 rotation,
                  u16 cgroup, u16 cmask
magic "END\0"
shape:            u8 kind (0 sphere, 1 box, 2 cylinder), vec3 center,
                  3 × vec3 rotation rows, vec3 half (a sphere's radius is
                  half.x; a cylinder's radius half.x, half height half.y,
                  round about its local y)
damage:           f32 max_hit, f32 armor_piercing, u32 interval_ms,
                  u8 continuous, u8 parts, list (f32 d, f32 value) hit_curve
energy:           u8 enabled, f32 initial, decrease_speed, maximum, on_walk,
                  on_run, on_sprint, on_jump, on_shoot, on_explosion,
                  on_character_hit, on_character_kill, on_af_container_use
artefacts:        u8 enabled, f32 first_spawn_s, respawn_s, variation_s,
                  chance, u32 max_count, u32 containers
state:            u8 enabled, f32 threshold, u32 active_time_s,
                  f32 energy_on_exit, i32 select_priority, u8 shoot_trigger,
                  u8 zone_activity_trigger, list groups
group:            u8 enabled, u8 keep_all, u8 sequential, u32 min_active_s,
                  u32 max_active_s, u32 max_charged, u32 recharge_s,
                  list string zones (config order = the worlds' mask bits)
```

**Version 2** (2026-10-14) added cylinders (version 1 dropped them: 2 of 3
shapes of each level_03 electric zone, the moss, acid, stalactite, fruit
and level_07 zones), each damage zone's hit and each anomaly core's states,
groups, energy and artefact settings (config order; the client sorts the
states by threshold, highest first). A server refuses a version 1 file
("map data version 1, this build reads 2 only: write the file again with
this build's mapdata-dump"): read with defaults, its zones would have no
hit and no cylinders, and bots would walk into anomalies. Dump again.

A shape is placed as: the mesh's rotation and position, then the
object's scale, rotation and position (LIKELY; every shape of the shipped
maps has scale 1). Reading checks every count against the bytes left: a
damaged file is an error, never a panic.

## 4. API

```rust
let map = MapData::read(path)?;                       // or MapData::load(&mut resources, &materials, project)
let world = World::build(map, mapdata::CELL, Body::default());
let node = world.grid.locate(feet, 1.0)?;             // nearest floor within 1 m
let to_flag = world.grid.distances_to(&[goal]);       // one flood for every bot heading there
let (next, how) = to_flag.next_step(&world.grid, node)?;  // how: Walk, Drop or Jump
let path = world.grid.path(a, b)?;                    // A*, nodes and cost
let sees = world.sight.visible(eye_a, chest_b);       // bullet mesh
```

- `MapData`: `spawns`, `containers` (with use boxes), `battery_spawners`,
  `stations`, `zones` (a damage zone's `damage`: `max_hit`,
  `armor_piercing`, `interval_ms`, `continuous`, `parts`, `hit_curve`;
  `harmful()` is false for the zones that only show or sound),
  `anomalies` (`states` → `groups` → zone names, `energy`, `artefacts`);
  `walker_triangles()`, `bullet_triangles()`, `play_area(margin)`.
- `Shape`: `contains(p)`, `distance(p)` (0 inside), `bounds()`.
- `WalkGrid`: `locate`, `position`, `headroom` / `can_stand` (crouch-only
  floors), `links` / `incoming`, `path`, `distances_to` (cost from every
  floor to the targets) and `distances_from` (where a team gets),
  `floors_in(area)` (the floors under an x/z box); from the spawns,
  `reached` / `returns` / `in_pocket`, `pocket_of` / `pocket_floors` and
  `pocket_exits` (below, "Grid: pockets and jumps").
- `RayTree`: `visible(a, b)`, `ray_cast(origin, dir, max)`.

### The walk grid

Port of the prototype, with these changes:
- Cells are half-open: a mesh ending on a cell line stops before it, a wall
  standing on one goes to one column.
- A jump up is a wall to erosion (a crate's, a low wall's face): the floor
  at its foot goes and the jump starts a cell back
  (`player_bots_stuck.md`: Cologne's workbench). A jump lands only where
  two more floors are walked on along it (no fence's top).
- Erosion keeps a floor that is at least the radius (less 1 cm) from the
  near edge of a wall cell: `k` cells from it, `(k - 0.5) × cell`. That is
  1 round at 0.25 m cells; the prototype's `ceil(radius / cell)` = 2 rounds
  shut a 1 m diagonal door on level_01/day_4 and left a spawn in a 4 m²
  pocket.
- A drop or jump whose landing was eroded lands up to the rounds of erosion
  further on (a body dropping beside a wall lands clear of it).
- Paths cost metres plus 0.5 for a drop and 1.5 for a jump (tuning).

**Use 0.25 m cells** (`mapdata::CELL`). 0.5 m cells shut 1 m doors: on
level_02 and level_04 the teams then reach 13-64 of the spawns and A*
finds 0-1 of 25 paths.

### Grid: pockets and jumps

The grid's build marks, from the spawns (`World::build` passes them;
`build_kept(..., seeds, spawns)`):
- **reached**: forward over every link from the spawns' floors (within
  1.5 m of a spawn, as the bots' tactics find them);
- **returns**: a walk gets from the floor into the main area, the largest
  strongly connected set of reached floors. Not "back to a spawn":
  level_05's spawns are left by a drop, and that made 97,292 floors and all
  53 objectives a pocket;
- **pocket**: reached and not returns, labelled by its lowest floor
  (`pocket_of`, `pocket_floors`). A floor that is not a pocket and links
  into one is a pocket's edge.

With no spawn located every floor is reached and returns. The marks cost
a way nothing (below: the bench). A jump of 0.95-1.05 m costs 4 m more
(`MARGINAL_JUMP_PENALTY`): the client's jump tops at 0.998 m (PROVEN), and
the grid links jumps up to 1.05 m.
`pocket_exits(floor)` lists a pocket's ways out that are no link: floors
that return within 1 m across and 0.2-1.35 m up, with headroom over the
take-off, one per landing (the jump's distance was never measured: the 1 m
is a GUESS). Each says what it takes (`ExitJump`): `Standing`, the jump (up
to 0.998 m, or across a gap); `Sprint`, only the sprint jump (up to 1.35 m:
a run-up with sprint held, any weapon, never with a battery; no bot's jump
picks it yet); `Steep`, a rise within a step into a neighbour cell: a walk
up it would be a link, so the grid cut it as a climb onto steep ground (the
client's `step_down` refuses that too), and a jump onto it is not proven.
`pocket_entries(pocket)` lists the links into a pocket from its edge,
mostly drops: the take-offs a body keeps off when the pocket has no way
out. The digest, the file (`GRID_ALGO` 5) and `check()` carry the marks.

The client's step (`step_up` 0x7C900, `step_down` 0x7E1B0,
`impassable_slope` 0x7DE10, re-kit): a step of up to 0.6 m (`s_step_height`)
with no slope test; a surface steeper than 50° is refused only when the feet
ended higher. Not used by the grid: "Grid: steep step-ups (parked)"
below says why.

Re-linking jumps up to 1 m beside walls (to open pits) was tried and
dropped: no table changed. Cologne's pit by the bridge pillar leaves by a
1.23 m jump onto the tyre (z 18.62), a sprint jump's height; the cells
between were never floors (the pillar).

The table per map (`tests/grid_table.rs`, 0.25 m cells; pockets by
their surest way out, `pocket_exits` from the pocket's lowest floor):

| project | floors | reached | pocket floors | pockets | out by jump / sprint only / steep only / none | objectives unreached | in a pocket | jumps | of them 0.998-1.05 m |
|---|---|---|---|---|---|---|---|---|---|
| level_01_day_2 | 426,363 | 378,194 | 15 | 5 | 5 / 0 / 0 / 0 | 3 of 39 | 0 | 5,469 | 365 |
| level_01_day_4 | 427,238 | 379,323 | 15 | 5 | 5 / 0 / 0 / 0 | 0 of 0 | 0 | 5,353 | 356 |
| level_02_day_2 | 468,474 | 384,373 | 20 | 6 | 3 / 2 / 0 / 1 | 2 of 53 | 0 | 6,103 | 768 |
| level_02_day_4 | 465,251 | 383,224 | 20 | 6 | 3 / 2 / 0 / 1 | 0 of 0 | 0 | 5,972 | 735 |
| level_03_day_2 | 581,303 | 246,613 | 49 | 6 | 4 / 0 / 0 / 2 | 2 of 52 | 0 | 5,722 | 325 |
| level_03_day_4 | 585,481 | 246,816 | 49 | 5 | 2 / 1 / 0 / 2 | 0 of 0 | 0 | 5,669 | 336 |
| level_04_day_2 | 381,364 | 337,238 | 255 | 13 | 5 / 3 / 0 / 5 | 5 of 35 | 0 | 5,922 | 765 |
| level_04_day_4 | 378,643 | 336,360 | 255 | 13 | 5 / 3 / 0 / 5 | 0 of 0 | 0 | 5,855 | 762 |
| level_05_day_2 | 188,042 | 110,870 | 385 | 6 | 1 / 1 / 0 / 4 | 0 of 53 | 0 | 2,762 | 142 |
| level_05_day_4 | 188,533 | 111,569 | 385 | 6 | 1 / 1 / 0 / 4 | 0 of 1 | 0 | 2,750 | 149 |
| level_06_day_2 | 694,496 | 216,303 | 5,059 | 29 | 16 / 4 / 0 / 9 | 0 of 34 | 0 | 6,688 | 579 |
| level_06_day_4 | 685,586 | 211,898 | 4,755 | 25 | 15 / 1 / 0 / 9 | 0 of 0 | 0 | 6,445 | 516 |
| level_07_day_2 | 240,106 | 141,417 | 1,950 | 10 | 3 / 5 / 0 / 2 | 2 of 41 | 0 | 6,554 | 353 |
| level_07_day_4 | 240,901 | 142,310 | 1,950 | 10 | 3 / 5 / 0 / 2 | 0 of 0 | 0 | 6,437 | 335 |
| level_08_day_2 | 128,859 | 118,411 | 710 | 9 | 5 / 1 / 0 / 3 | 3 of 24 | 0 | 2,942 | 447 |
| level_08_day_4 | 128,181 | 117,938 | 607 | 8 | 4 / 1 / 0 / 3 | 0 of 0 | 0 | 2,845 | 435 |

162 pockets in all: 80 leave by the jump, 30 only by the sprint jump, 52
have no way out within 1.35 m. Every pocket with a steep exit has a jump
out too. Of the 26 with none on the day_2 maps, 12 are pits (every floor
that returns within 2 m is 1.5 m or more higher); the other 14 have one
lower, 1-2.8 m across. Of the 9 of those looked at cell by cell, each had a
wall, a void or a crouch-only space between (the grid makes no jump into
crouch-only space: the client's is not proven). At the base the table was the same but for level_05: 97,292
pocket floors and 53 objectives in a pocket (the spawn rule above). No
jump link lands 3-4 cells along its direction on any map.

The bench's stand-in body (`bots/tests.rs`, `stand_in_jump`) jumps as the
client does: by a grid jump link only within 0.998 m, else onto the highest
floor up to 0.998 m within 0.6 m ahead (GUESS) on its walk's line; the
sprint jump (1.30 m) is in a test only, as no bot's jump picks it yet.

The bench (`real_bot_cost`: the 16 projects, 4 v 4 and 10 v 10, 180 s, 10
batteries; 16 seeds, 512 pairs; `with − without`, * beyond 2 SE). It is
deterministic per seed: a seed run again gives the same decisions.

| | stuck notes | delivered | carriers killed | K/D team 0 / 1 | falls on route |
|---|---|---|---|---|---|
| the stand-in's jump, against the base | +1.15 ± 0.19* | −0.09 ± 0.03* | −0.045 ± 0.047 | −0.04 ± 0.04 / +0.01 ± 0.03 | −1.12 ± 0.23* |
| the marginal-jump cost, pockets' marks, against the stand-in's jump | −0.56 ± 0.19* | +0.07 ± 0.03* | −0.049 ± 0.050 | +0.06 ± 0.04 / −0.02 ± 0.02 | +0.42 ± 0.20* |
| with the pocket costs too, against without them | −0.01 ± 0.08 | −0.02 ± 0.02 | −0.033 ± 0.020 | −0.02 ± 0.01* / +0.01 ± 0.01 | +0.10 ± 0.07 |
| with the marginal-jump cost, against without it (pocket costs in both) | −0.69 ± 0.20* | +0.04 ± 0.04 | −0.100 ± 0.049* | +0.05 ± 0.04 / −0.02 ± 0.02 | +0.41 ± 0.20* |

- The stand-in's jump is a measurement change: the bench's body no longer
  makes the 0.998-1.05 m jumps the client cannot, so bots that took them
  stall (more stuck notes, fewer deliveries).
- The marginal-jump cost makes the gain: ways go round a jump at the
  client's limit when the way round is short, so fewer bots stall at one;
  more of them go down planned drops instead (falls on route).
- **The pocket costs were measured and removed**: 30 m more into a pocket
  and 1 m onto its rim moved no target, and team 0's K/D was a little worse
  with them. Pockets are small and routes seldom pass one. A cost there
  needs its own evidence (unstuck's bench) before it comes back.
- Quick ends, flips, idle and team kills (friendly fire: 6 over the 512
  runs with the stand-in's jump, 3 with this) did not move beyond noise.

On the combined tip with unstuck's team-wide stuck and fall spots (#383)
and the batteries' 0.6 s put (#384), 512 pairs:

| | stuck notes | delivered | carriers killed | K/D team 0 / 1 | quick ends / bot-min | falls on route |
|---|---|---|---|---|---|---|
| the stand-in's jump, against the tip | +0.95 ± 0.19* | −0.10 ± 0.04* | −0.037 ± 0.052 | −0.01 ± 0.04 / −0.01 ± 0.03 | +0.004 ± 0.012 | −0.77 ± 0.22* |
| this PR, against the tip with the stand-in's jump | −0.37 ± 0.16* | +0.04 ± 0.03 | −0.047 ± 0.048 | +0.01 ± 0.04 / +0.01 ± 0.03 | +0.018 ± 0.013 | −0.01 ± 0.18 |

With the team's fall spots the extra falls on route are gone, and
deliveries keep within noise of the gain without them. Team kills over the
512 runs: 4 at the tip, 4 with the stand-in's jump, 9 with this PR (within
noise of each other).

### Grid: steep step-ups (parked, 2026-10-24)

**The rule tried.** A walk up onto a steep face (a floor on a 50-60°
walker triangle) was cut once the climb rose more than 0.3 m (GUESS) over
the walkable ground it began on, against the step's 0.6 m today. Ground
steep only by its neighbours' heights kept the step. The basis was the
client's `step_down` (0x7E1B0), which refuses a tick that lands on a facet
steeper than 50° with the feet higher (`impassable_slope` 0x7DE10, PROVEN).
Cutting both kinds (faces and grade) at 0.3 m was rejected first: it made
a new pocket with no way out on level_04 (71 floors).

**Where it fails.** It held the grid's hard limits on every map (day 2, 3
and 4): no objective unreachable, no new pocket with no way out, no
pocket's surest exit worse. Most maps lost 20-160 reached floors.
level_06 (day 2 and 3) lost 4,638, nearly all of them one slope (x 47-80,
z 119-135, y 31-39), entered only by walks up steep ground of 0.15-0.54 m
each. Players stand on that slope: 998 positions in the server's 5 s
worlds, 116 lives, 438 of its cells (non-bot matches, Oct 12-21).
Cutting only where the face and the grade are both over 50° keeps the loss
(4,583 floors, 114 lives).

**They walk up, not hop.** Of the same matches, 76 have a recording (every
input, per tick; the sprint key agrees with the worlds' sprint flag 96 % at
no lag, 82 % at ±1 s). 30 times a player shows on the slope 5 s after
being below it at its south edge (12 players, 10 matches). They rise 0.9-5.2
m, 26 of them 2 m or more, with 0 jump presses in 2 climbs, 1 in 20 and 2-5
in 8 (the same players press as often in the 5 s before). A jump rises
0.998 m at most, so at least 26 climbs walked most of the way up faces the
rule calls too steep. A hop link would not match that.

**The follow-up (client RE).** The normal `step_down`'s sweep meets on
level_06's slope at x 60-65, z 118-128. Either the collision there is not
the walker mesh's 50-60° faces, or the sweep's normal at the convex edges
is more upright than either facet (LIKELY). The rule can come back once
that is known.

**Strips, dropped.** A cost of 0.5 m (GUESS) on each walk onto a strip
floor (1-2 cells between walls, voids or cut steep faces: 950-3,700
reached floors a map) was benched with the rule above, against it alone
(8 seeds, battery and team deathmatch, no tier): falls on route +2.8 ±
0.7 and +2.1 ± 0.6 a match, jumps pressed +6.6 ± 1.7 and +3.1 ± 0.8, stuck
notes +0.8 ± 0.5 and +0.6 ± 0.4. Per floor, a 10 m strip cost 20 m, so ways
went round strips by drops and jumps. If it comes back: a flat cost per
strip crossed (or on any link onto one), with a target measure for the
strip cases (CG-8) first.

### Grid: no way planned into a pocket with no way out (2026-10-25)

**The rule.** A pocket whose surest exit is none (no jump the body makes
leads out, `pocket_exits`) marks its floors `NO_EXIT`. No way is planned
over a link into such a pocket from a floor outside it, whatever the link
(a drop, or a walk down a step that does not walk back): `WalkGrid::plans`,
and its cost is infinite. The link stays, so a body knocked or overshooting
over the edge is in the pocket, and ways inside it and out of it are still
planned. The regions and the tactics' floods (the escape flood, the exit
field, the floors walkable from the spawns) skip the same links, so no spot
or way out lands in one. A battery lying there has no stand (no floor of
the pocket is walkable from the spawns) and keeps the pocket's take block
(`collect::POCKET_BLOCK`). `GRID_ALGO` 8 (6 and 7 were unreleased
variants), `TACTICS_ALGO` 3.

**The grid table** (every map, day 2, 3 and 4): 79 shut pockets, none
planned into (`shut_planned` 0, asserted), 7,733 floors no longer planned
onto (level_07 1,600 a map, level_05 382, level_06 167-355, the rest 5-244).
No objective in a shut pocket or behind an unplanned way; objectives
unreached 19, as before.

**Players there.** Of the human positions in the server's 5 s worlds (the
audit of the erosion work, non-bot matches), 308 stand on floors now
unplanned, 14 lives. 243 of them (5 lives, 3 matches) are level_05_day_4's
pocket of 372 floors around (-10.4, 9.1, -22.8), a pit 1.2-1.6 m below the
ground west of it. In the recordings (the worlds' positions, every input)
each of the 5 drops in from the west, crouches there (most ticks) and never
leaves: 4 die in it, 1 is in it at the match's end. So the grid's "no way
out" holds for players too; the pit is a firing spot, not a way. The rest
are 2-38 a map (level_03, 04, 06, 08).

**The routes check** (the stored human routes, walked by the planner):
legs with no way 12 → 10, points lost 141 → 143, jumps 86 → 86, length
+0.1 %. The two lost points are one point of level_05's day 2 and day 3
routes at (-7.8, -21.3). Route points are 2-D, and `HumanRoute::on_floors`
puts one on the lowest floor near it: here the pit (8.5-9.0 m), under the
floor players walk (10.3-10.6 m; of the players' positions within 2 m, 180
of 181 on day 2, all 132 on day 3, 430 of 481 on day 4 stand on it). Before,
bots were routed into the pit and the leg out had no way; now the point is
unwalkable and dropped.

**Route points on a planned floor (2026-10-25).** `on_floors` now takes the
lowest floor a way is planned to (none in a shut pocket). The routes check,
from #402's tip: points lost 143 → 141 (level_05's two points come back on
the walkway at 10.5 m), legs with no way 10 → 10, jumps 86 → 86, length
+9 m. Three level_07 points (days 2, 3, 4, around (-7, -2) and (5, -2)) move
from a shut pocket at 2.5 m to 5.8-9.7 m and stay dropped, as before;
players there stand at about 21 m (139 of 154, 26 of 26 and 74 of 79
positions), so "the lowest floor" is the wrong rule for those 2-D points
(the routes' follow-up). Bench (seeds 1-16, after side, against #402's
tip): every map but level_05 and every TDM match decision-identical; per
seed run, batteries 4-5 falls on route 63.8 → 62.6 (z −2.2) and kills
240.0 → 238.8 (z −2.2), 9-10 planned drops 329.2 → 328.4 (z −2.0), the rest
within 2 SE. On level_05 (32 matches a cell) falls on route −19, 0, −4 and
legs with no way +116, −382, −969 by tier; no time in pockets either side.

**Bench** (c8bd1281 against 54ececcf, the base before #398/#399/#401,
batteries and team deathmatch, tiers 4-5, 9-10 and 13-15, seeds 1-16,
after side): decisions differ on every cell. Per seed run, past 2 SE:

| cell | change |
|---|---|
| batt 4-5 | kills 236.5 → 240.0 (z +2.9); quick ends 327.1 → 334.8 (z +2.5) |
| batt 13-15 | idle −49 s (z −2.7); legs with no way +323 (z +2.2) |
| tdm 4-5 | K/D of team 0 +0.03 (z +2.4) |
| tdm 9-10 | flips −3.6 (z −3.1) |

Everything else is within 2 SE: falls on route (z −1.1 to +1.2), planned
drops, time and deaths in pockets (batt 13-15 pocket deaths −0.19, z −1.9),
give-ups, deliveries, carriers killed, team kills and stuck. batt 4-5's quick
ends are the one guard past 2 SE, of about 60.

**Follow-ups.** Route points with a height (level_07, above).
Grid gaps: level_07
(82, -25); level_06_day_4 (13.6, 108.6); level_03 (-16, -15); passages
narrower than the body. The erosion by body height (floors beside a fall
kept, with a cost on them) is rebuilt on top of this rule: benched alone it
raised falls on route on every cell, from route legs along the edges and
from planned drops off them.

### Route points with the players' height: benched, shelved (2026-10-25)

Play 17's route points carry no height; on stacked floors the server puts a point on the lowest floor round it, often under the floor the players walked. The 3D table gave a point the players' feet height (an optional `heights` per row, no format change), and the server put it on the floor nearest that height (within 3 m; radius 2 m, then 5 m).
- **Heights from real matches.** For each point, every human's positions within 5 m in all recorded matches on that map and day, any match size, the humans of bot matches included, and positions filled every 1 m between 5 s samples of one life on one level (15 m at most, feet within 0.5 m, a floor there). Levels split at 2 m gaps; the lives' majority picks, the lower level when within 60:40. 993 of 1,321 route points got a height; 121 of the 122 PvB points with none before were filled, the last has no positions. The table and the players' data stay in `E:\sur-res-pro\private\bot-routes\work-20261025-3d` (not in any repo).
- **Walk check** (without → with heights, 231 routes, tip 1dee0543's tree): lost points 141 → 122, legs with no way 10 → 1, jumps 78 → 83, legs through a damage zone 162 → 114; every pick but 2 has a floor within 1.5 m of its height.
- **Bench** (battery + Team Deathmatch × 3 tiers, 16 seeds, 96 pairs, the table against routes.json on 1dee0543): the target, legs with no way, +181 ± 180 (z +1.0, no gain); falls on route -11.4 ± 1.9 (z -6.0), zone entries -9.3 ± 1.6 (z -5.7), reversals -6.4 ± 2.7, idle -0.30 ± 0.14 points; but stood with a goal and no route +0.16 ± 0.06 points (z +2.9) and delivered -0.36 ± 0.17 (z -2.1), guards worse beyond 2 SE. Shelved: no choice of target ships it.
- **What the benches found on the way**, fixed for every table: spot plays sent bots to spots with no way to them (#417: Cut, Crossfire and HoldExit keep only reachable spots), and Cut's point jumped a leg on when a route point moved 0.3 m across `cut_progress` (#421: the cut is where the leg crosses it). Before them the table's no-way rose +1,200 (z +3.8), then idle +0.37 points (z +3.0).
- **Open:** with the table, Crossfire (Team Deathmatch, +9 %) and Collect (batteries, +4 %) stand more with a goal and no route, though neither reads the routes; where the fights move is the likely cause, not traced.

### Grid: edges and drops priced, a band at the lips (b4, 2026-10-25)

**The rules.** On top of the shut-pocket rule (`GRID_ALGO` 11; 9 and 10 were
unreleased variants, so stored grids rebuild):

- **Erosion by body height:** walls are solid at the body's height. The floors
  beside a fall deeper than the harmless drop (5 m) are kept, and those within
  the body's radius + 0.25 m of one are `near_fall`; a walk onto one costs
  `FALL_EDGE_PENALTY` (0.5 m) more.
- **Drops:** a drop costs `DROP_DESCENT_PENALTY` (1 m) more per metre it falls,
  about the walk down a stair it replaces. Without it, the edge cost is dodged
  by dropping off the edge: planned drops rose 27-56 a cell.
- **The band at the lips:** floors within the body's radius + 0.25 m of a lip
  (a drop of 1 m or more) are `near_lip`. A walk onto one costs `LIP_PENALTY`
  (0.25 m) more. That is a cost per floor entered, never a block: a narrow
  walkway or an open-sided stair inside the band is still the way, and a drop
  link never pays it, so a drop's take-off is walked to across the band for
  about 0.75 m.
- **pull_straight:** a line pulled straight crosses no lip, near-fall or
  near-lip floor that the search kept off. The exception is the edge floors
  leading in to a drop's take-off.

**The erosion changes floors, not just costs.** Every bench number below is
from binaries that include it. The grid's hard limits, checked at the PR's
commit against base a6fac9de on all 24 projects:
- **No plan into a shut pocket:** 0 on every map. No objective is in one or
  unplanned.
- **Objectives unreached:** 19 → 15.
- **Reached floors:** 5.80 M → 5.94 M.
- **Pockets with no way out:** 79 → 86, none planned into:
  - level_03 +1, level_04 +1 and level_07 +3 (each day);
  - level_05 −2 (each day), level_06_day_4 −1 and level_08_day_3 −1.
- **Surest exits:** one pocket's got worse, level_06 day 2 and 3 at (46, 30,
  101). It went from 5 floors with a sprint exit to 1 floor with none.
- **Human positions** (the servers' 5 s worlds):
  - on planned floors +10,140;
  - unreached −5,587;
  - with no floor near −4,555;
  - on reached but unplanned floors 308 → 310 (lives 14 → 15).
- **The three small shut pockets first logged with the erosion,** at level_07
  (82, −25), level_06_day_4 (13.6, 108.6) and level_03 (−16, −15): all still
  shut and none planned into. Human positions on their reached floors: none on
  level_07 or level_06_day_4; on level_03, 3 (one life) on day 2 and 2 (one
  life) on day 4. They stay grid gaps (follow-ups). Whether a bot that falls
  in gets out is for unstuck's sweep.
- The erosion's commit subject still reads "(WIP)"; that is wording only.

**Why the band.** Every unplanned fall of the bench is off a lip of 1-5 m:
1,410 of 1,410 on #402's tip, and 3,095 of 3,097 with the drop rule. The
`near_fall` band, at falls deeper than 5 m, meets almost none of them.

**Where the falls were** (#402's tip, an event per fall, batteries and team
deathmatch, three tiers, seeds 1-4): 539 started on floors the A* chose, 498
where pull_straight's line left them, 133 off the leg and 240 with no route.
That is even in every cell, so both the search's cost and the line test
matter.

**Bench** (the tip a6fac9de merged; against base-a6fac9de, both presets, three
tiers, seeds 1-16, 96 of 96 runs). Per seed run, base / without the band /
with it:

| cell | falls off the route | planned drops | time in pockets, s | delivered |
|---|---|---|---|---|
| batt 4-5 | 59.5 / 60.1 / 47.9 | 315 / 310 / 315 | 134 / 54 / 19 | 75.0 / 76.1 / 74.6 |
| batt 9-10 | 61.9 / 61.6 / 45.1 | 322 / 314 / 314 | 165 / 19 / 25 | 73.6 / 74.0 / 74.4 |
| batt 13-15 | 60.6 / 56.9 / 46.2 | 332 / 320 / 325 | 101 / 39 / 1 | 70.7 / 73.4 / 72.9 |
| tdm 4-5 | 60.6 / 67.1 / 42.3 | 262 / 219 / 240 | 101 / 78 / 56 | |
| tdm 9-10 | 56.9 / 63.4 / 40.2 | 269 / 225 / 240 | 72 / 109 / 46 | |
| tdm 13-15 | 56.5 / 63.1 / 44.0 | 276 / 223 / 244 | 160 / 80 / 35 | |

- **Falls with the band:** −11.6 to −18.3 a cell against base (z −4.4 to
  −9.3).
  - The A*'s falls drop most (batteries 21-25 → 14-15, TDM 28-30 → 17-18).
  - The line's falls drop most in TDM (18-22 → 10-12).
  - Falls within 2 m of a take-off drop too (18-22 → 9-15), so the band does
    not trade planned drops for falls beside them.
- **Without the band**, TDM's falls rose 6.5 a cell (z +1.7 to +3.1). The A*
  walked down rough steps and along lips that cost 0.25 where drops cost more.
  On level_06_day_4, (−2.5, 29, 105) and (47.5, 23, 60) went from 0.5-1.0 to
  5-6 falls a seed; with the band they are at 4.4 and 3.5.
- **Objectives:**
  - deliveries are level or up (batt 13-15 +2.2, z +2.5);
  - carry and put-in times are unchanged;
  - legs with no way are down (z −2.0 to −2.3 in three cells);
  - give-ups are down in every cell.
- **Watch:**
  - batt 13-15 carriers killed +2.4 (z +1.5), with the deliveries;
  - batt 4-5 stuck +6.6 against the variant without the band (z +1.8).
- **Planned drops against players** (a seed, three tiers, without the band →
  with it):
  - S21 30.8 → 30.8 and S22 23.8 → 23.8, where players go down most (4.5 and
    7.4 % a sample);
  - S18 44.6 → 7.4, a spot players hardly stand on;
  - level_05's S01 49.6 → 80.4, S02 36.2 → 62.6 and S05 46.2 → 89.1, where
    players walk (they go down 2.4-3.0 % a sample).
  - The level_05 drops are safe. Drops are at most the harmless 5 m
    (`fall_params.harmless_height`), so they do no damage. Bots killed within
    5 s of landing: S01 0.9 %, S02 2.8 %, S05 0.1 %, against 3.4 % for all
    other planned drops. Bots drop where players walk; safe.

**Tried and dropped** (each against the rules above without the band, the same
base and seeds):

- **The drop at 0.5 m a metre:** planned drops came back (+21 to +42 a cell)
  and the falls did not drop.
- **`LIP_PENALTY` 0.5:** batteries' falls rose 11-14 a cell while its planned
  drops fell 20-24. With the take-off floor dearer, routes walked the floors
  beside it and bots fell there.
- **A walk down steeper than walkable ground priced as a drop:** TDM's falls
  did not move, and its planned drops rose 15-17.
- **Strafing kept to the way:** while aiming on an edge leg, the keys took turns
  between the two nearest of the eight ways, against the nearest way alone,
  which is up to 22.5° off and leaves the body up to 0.36 m beside the line.
  Its sign was mixed: falls +6 in batteries 9-10 and 13-15 (z +2.1 to +2.5),
  −5 in TDM 4-5 (z −2.5). The slow strips it was for (the fall-spot study's
  S10, S12) had 38 falls in 24 runs.

**Follow-ups.**
- A downward feeler in the motor, for the off-leg falls (4-6 a cell); the
  motor does not see the grid.
- Route points with a height: they are re-checked on `GRID_ALGO` 11.

## 5. Measured (2026-10-14, release build, one thread)

0.25 m cells, all 72 projects; the day_4 project of each map, and one
battery map:

| project | spawns | file | walker / bullet tris | floors | grid | tree | team floods | A* (5×5 spawns) | ray |
|---|---|---|---|---|---|---|---|---|---|
| level_01/day_4 | 43 | 9.5 MB | 230k / 457k | 852k | 0.45 s, 49 MB | 0.08 s, 25 MB | 57 ms | 38 ms, 1.20× straight | 1.9 µs |
| level_02/day_4 | 31 | 9.4 MB | 153k / 466k | 2,288k | 0.61 s, 122 MB | 0.08 s, 25 MB | 74 ms | 58 ms, 1.22× | 1.3 µs |
| level_03/day_4 | 22 | 9.4 MB | 150k / 561k | 672k | 0.29 s, 38 MB | 0.10 s, 31 MB | 37 ms | 14 ms, 1.14× | 1.8 µs |
| level_04/day_4 | 75 | 14.9 MB | 298k / 814k | 1,260k | 0.49 s, 69 MB | 0.14 s, 46 MB | 55 ms | 47 ms, 1.25× | 2.1 µs |
| level_05/day_4 | 29 | 10.2 MB | 267k / 691k | 228k | 0.27 s, 15 MB | 0.12 s, 42 MB | 16 ms | 5 ms, 1.14× | 1.8 µs |
| level_05/evening_2 | 29 | 10.3 MB | 269k / 699k | 228k | 0.27 s, 15 MB | 0.12 s, 42 MB | 16 ms | none (one-way) | 1.7 µs |
| level_06/day_4 | 30 | 15.9 MB | 633k / 810k | 1,482k | 0.58 s, 80 MB | 0.15 s, 46 MB | 40 ms | 34 ms, 1.41× | 1.3 µs |
| level_07/day_4 | 31 | 12.4 MB | 390k / 1,025k | 339k | 0.24 s, 19 MB | 0.18 s, 54 MB | 23 ms | 15 ms, 1.22× | 2.3 µs |
| level_08/day_4 | 26 | 7.1 MB | 158k / 518k | 1,328k | 0.58 s, 71 MB | 0.09 s, 27 MB | 22 ms | 14 ms, 1.10× | 1.8 µs |

- Over all 72: grid 0.23-0.63 s, tree 0.07-0.19 s, file read 5-16 ms, a
  team's flood 15-81 ms, A* 5-60 ms, a ray (5-150 m, eye to chest) 1.3-2.4
  µs. 20 bots × 19 targets × 2 rays × 10 Hz ≈ 2 % of a core.
- Every spawn stands on a floor (located within 1 m) on all 72.
- Reach (with jumps): on 60 projects each team's spawns reach every spawn.
  On level_01 and level_05 in modes 2 and 3 the bases are one-way (team 0
  of level_01 reaches 30 of 39 spawns; on level_05 neither team reaches
  the other's), as the prototype found for level_05 (players' word:
  plausible).
- level_01/day_4 at 0.25 m: each of the 43 spawns reaches every other one
  (with or without jumps). At 0.5 m: 34 of 43 in one piece without jumps,
  as the prototype; with jumps each team reaches all 43, but 5 spawns sit
  in small pockets (behind narrow doors) that the 0.5 m grid enters and
  cannot leave.
- level_05/evening_2 at 0.25 m: each team reaches all 50 batteries.
- Grid memory was 52 bytes a floor (8 links of 4 bytes dominated); level_02
  needed 122 MB at 0.25 m. Since 2026-10-16 the links are packed (§6,
  "Lighter maps"): 28 bytes a floor, 64 MB on level_02.

Tests on the game data: `SRP_RESOURCES_DB=<resources.db> cargo test -p
mapdata --release -- --nocapture` (skipped without the variable).

## 6. On the match server

`match-server` `v026g::bots::maps`: a store of built worlds shared by
every match.

- `SRP_MAPDATA_DIR=<dir>` (default off): the `<project>.mapdata` files,
  e.g. `level_01_day_4.mapdata`, written by the dump command of §1 (no
  `--map`: all 72 projects, 817 MB). Unset, or a project with no file or a
  file that fails to load: bots keep the stage 0/1 script (a warning once
  per project, until a restart).
- `SRP_MAPDATA_KEEP=<n>` (default 1): built worlds kept besides those
  running matches hold.
- `Maps::request(project)` at match creation starts the build on a thread
  of its own; `Maps::get(project)` is `Absent(reason)`, `Loading` or
  `Ready(Arc<World>)` and never waits. Builds run one at a time. A world
  dropped from the store while a match still holds it comes back without
  a build. One info line per build (time, MB), one warning per failure.

Measured 2026-10-14 through the store (release, file read + grid + tree;
`real_map_sizes` in `maps.rs`); MB is the grid's, the tree's and the
meshes' vectors summed:

| project | ready in | grid | tree | world |
|---|---|---|---|---|
| level_01/day_4 | 0.53 s | 46 MB | 23 MB | 79 MB |
| level_02/day_4 | 0.71 s | 116 MB | 23 MB | 149 MB |
| level_03/day_4 | 0.43 s | 36 MB | 29 MB | 74 MB |
| level_04/day_4 | 0.69 s | 66 MB | 43 MB | 124 MB |
| level_05/day_4 | 0.43 s | 14 MB | 39 MB | 63 MB |
| level_06/day_4 | 0.76 s | 76 MB | 43 MB | 135 MB |
| level_07/day_4 | 0.47 s | 18 MB | 51 MB | 81 MB |
| level_08/day_4 | 0.69 s | 67 MB | 25 MB | 99 MB |

- Process working set (Windows): loading level_01/day_4 alone peaks at
  145 MB and ends at 111 MB (the summed 79 MB plus the test process, the
  file and the placed triangles while the build runs). The eight above in
  turn with one kept: 326 MB at the peak.
- **Cache policy.** All 72 kept would be about 7 GB (100 MB a world on
  average over the eight above); the 0.26g host has
  955 MB. So the store keeps the last used world only (`KEEP` = 1), builds
  one at a time, and a match's own `Arc` keeps its world while it runs.
  With 72 projects the next match is rarely on the last map, and a
  rebuild costs 0.3-0.8 s off the match thread. Several matches at once
  on big maps still need 150 MB each: GUESS that a small host holds two.
- The build shares the host's one core with the match loop for up to
  0.8 s; how much a running match feels it is not measured.

Deploying (described only; nothing deployed): dump on a PC with the game
(§1), copy the directory to the server's data volume (e.g. a `mapdata/`
beside `logs/` and `recordings/`), and set `SRP_MAPDATA_DIR` to it as the
container sees it. The files are the game's data: in the private data
volume only, never in an image or this repository. A file added for a
project no match has asked for yet is used; one whose project was already
found missing or broken needs a restart.

### The bots' cost on the real maps (2026-10-14)

`real_bot_cost` (`bots/tests.rs`, `SRP_MAPDATA_DIR`, `SRP_BOT_ROUTES`):
the soaks' stand-in game on a real map, the bots polled every 5 ms as the
match loop does with nobody sending (every datagram polls every match too:
the recorded matches of 6-10 players bring 100-200 a second), each part of
a poll timed. Measured on a fast desktop core; the 0.26g host's core is
about 2.6 times slower (the same Python loop: 0.26 s here, 0.66 s there).

**Before** (commit f8fcd38): on level_06/day_2 four bots a side took 2.5
cores (247 % of a core), a poll up to 0.64 s, all of it in route searches:
421 of 440 searches failed, 75 ms each. A search for a goal with no way
(a bot on a floor island, a goal behind a drop, a battery in their spawn
area) floods everything it reaches, up to 1.5 million floors, and each bot
asked again every 0.5 s; even a search that found its way took 4-14 ms on
average, up to 0.16 s (`real_search_budget`). The bots never met: no shot
in 240 s.

**What changed:**
- `WalkGrid::pieces` (union-find over the links at the build): floors in
  pieces no link joins have no way, answered at once (`may_reach`).
- `WalkGrid::search`: the A* keeps its arrays on the thread between
  searches, marked by a search number, so a search costs what it expands,
  not the grid's size (it allocated and filled 2 × 4 bytes a floor, 18 MB
  on level_02, every time); a floor met again by a cheaper way is skipped
  when its stale heap entry comes up (it was expanded again).
- `mapdata::regions` (hierarchical path finding, as HPA* and navigation
  meshes do it): 8 m blocks cut into regions of floors joined by walk
  links both ways and of one label; regions link where floors do (one way
  for a drop or jump). 2,500-5,800 regions a map. A search goes by the
  regions first, then the floors' A* inside the corridor of regions and
  one ring of neighbours, then three rings more, last the whole grid.
- The tactics label every floor with what `Tactics::allowed` asks (the
  zones' set, the sides' spawn areas) and cut the regions by the labels, so
  the regions' search keeps off what the floors' does, exactly (Detour's
  query filters and area flags are the same idea). `allowed` reads the
  label instead of measuring to every spawn point for every floor.
- A poll makes at most 2 searches for all the match's bots (the others
  plan at the next poll, 5 ms on); the second search through a bot's stuck
  spots is made only when it has any.
- The tactics (hazard marks, labels, regions, exits; 0.15-0.3 s on a big
  map here, 0.4-0.8 s on the host) are built on a thread of their own
  (`game/driven.rs`), not on the match loop; the bots stand until they
  come, as they stand until the world is built.

**Same answers** (`real_route_search`, 300 pairs of floors 10-150 m apart
a map, with and without 8 round "anomalies" of 6 m on the way): the
regions say "no way" exactly when the whole grid's A* does (all 4,800
pairs), and a way found costs 0.1-1.2 % more on average than the whole
grid's cheapest (its string is pulled straight after either way). The
bases' exits are worked out once a map with the whole grid's search, as
before. The three soaks and the real-map route and cut tests pass with the
same kills, hits and batteries.

**After**, the same runs (120 s of play; % of the fast core):

| map | 4 v 4 | 10 v 10 | worst poll (10 v 10) |
|---|---|---|---|
| level_06/day_2 | 0.20 % (was 247 %) | 0.76 % (34.7 % with the first fixes) | 46 ms (was 0.64 s) |
| level_01/day_4 | 0.54 % | 1.62 % | 20 ms |
| level_03/day_3 | 0.31 % | 1.90 % | 17 ms |

So a 10 v 10 of bots takes about 2-5 % of the host's core. A search took
1-2 ms on average and 46 ms at worst (a long corridor on level_06).
`real_bot_cost` prints the searches by stage.

**Memory before 2026-10-16:** the 0.26g container is limited to 512 MB
and uses about 190 MB; a world was 63-149 MB and its build peaked near
twice that; a match's tactics added 4 bytes a floor of labels, 4 of
regions and 2 of hazard marks (up to 23 MB on level_02); the A* arrays
the match thread keeps are 12 bytes a floor of the largest grid searched
(27 MB).

### Lighter maps (2026-10-16)

The owner's rule: as little CPU and memory as can be, and the bots never
decide worse (only better). So every change below was kept only once it
was shown to leave every decision as it was.

**The proof.**
- `mapdata::digest`: `WalkGrid::digest`, `RayTree::digest` and
  `TacticsBase::digest` give what a structure answers as one number (floats
  by their bits, links as target and kind). Pinned on synthetic maps, so a
  change to a build fails a test until its `*_ALGO` is raised.
- `every_world_builds_the_same_twice` (`crates/mapdata/tests/real_maps.rs`):
  each of the 72 projects builds the same twice; its digests are the
  baseline the later steps had to match (they did, all 72).
- The decisions digest (`bots/tests.rs`, `Decisions`): every input the
  bots make and every note, as one number, in the three whole matches and
  in `real_bot_cost` (4 maps, 4 v 4 and 10 v 10, 437,000 inputs). The bots
  read no clock, so two runs give the same digests; every step below gave
  the same digests as before it.

**What changed.**
- `TacticsBase` (shared by matches) holds what depends on the map and the
  hazard margin only: the hazard marks, the spawn areas (a byte a floor,
  was 4 with the zones' set), the regions, the exits. `Tactics` is a
  match's view of it: its own zones' clocks (anomalies) and human routes.
- The store (`bots::maps`) hands out one `TacticsBase` per map for the
  default margin, built on the map's build thread: no tactics thread per
  match. A match tuned to another margin builds its own on the shared
  world, as before.
- The grid and the sight tree are `Arc`s, found by the key of what they are
  built from (`mapdata::grid_key`, `sight_key`): the 72 projects have only
  29 grids and 30 trees, and the weather variants of a level now share
  them. The map's meshes are dropped after the build (nothing read them).
- The grid's links are packed: the linked floor's index in its cell (a
  byte) and 4 bits of kind and reach, 28 bytes a floor instead of 56.
  Building all 72 got faster too (62 s to 48 s here).
- Built grids, trees and tactics are kept on disk (`mapdata::built`,
  `SRP_MAPDATA_BUILT`, default `<SRP_MAPDATA_DIR>/built`) and read instead
  of built. Each file is named by its key (another build's file is never
  found), checked index by index and by its digest (a damaged one is
  refused, built again and written over; never a panic). The server writes
  them itself, so they are what the host builds. The tactics' key holds the
  map file's digest and the grid's key: a tactics file is never read onto
  another grid.

**Measured here** (release, fast desktop; the host is about 2.6 times
slower):

| | before | after |
|---|---|---|
| grid, level_02/day_4 | 128 MB | 64 MB |
| tactics, level_02 | 23 MB a match | 16 MB a map, shared |
| a map ready (world + tactics) | 1.1 s + 0.39 s a match | 0.13 s read |
| all 72 projects ready | 57 s built | 9.7 s read |
| built files on disk | | 2.4 GB (29 grids, 30 trees, 72 tactics) |
| the bots' polls, level_02 10 v 10 | 102 ms a second | 90 ms a second |
| a test process loading level_01/day_4 through the store (peak working set) | 146 MB, 0.69 s (world only) | first load 146 MB, 1.45 s (world, tactics, files written); then 62-83 MB, 0.17 s |

**Goals no allowed floor leads into** (School, level_02). Several of
School's battery spawners lie inside the fruit toxic cloud (the anomaly in
the middle of the map) or the stalactite radiation; the bots keep out of
the cloud's low levels always, so those batteries cannot be reached. The
regions' search lets the goal's own region through (the goal floor is
always let through), so it found a corridor, the floors' search inside it
failed, and the whole grid was flooded to prove "no way": about 80 ms,
again every half second while Collect kept that battery (224 of the 228
whole-grid searches of a 10 v 10, 94 % of the bots' time).
`WalkGrid::entered_from` now answers it first: a goal is reached only from
a floor that links into it and is let through (or the start); with none
there is no way, the same answer, at once. School 10 v 10: 90 to 11.6 ms
a second, worst poll 124 to 19 ms; level_01's worst poll 99 to 16 ms. The
decisions digests are the same.

**Batteries inside a hazard are left** (a decision change, the owner's
word of 2026-10-16). Before, Collect took such a battery, found no way and
the bot stood still till `give_up_s` (120 s). Now Collect leaves a free
battery no allowed floor leads into (`Ctx::reachable`, the same check), and
a Fetch gives up at once when a hazard closes round its battery (the
cloud's energy rose). Batteries so left with the zones always on: School 14
of 51 (8 in the cloud's levels 1-2, 6 in the stalactites' radiation; 6 more
in levels 3-5 only while the energy is high), level_06/day_2 5 of 32 (acid,
fire), level_03/day_3 6 of 18 (the timed spot in the rust gas). Failed
searches, School 10 v 10: 2,012 to 618. Taking them with little damage, as
players do, is for later.

**Floors no body gets to are dropped** (a decision change, the owner's
word of 2026-10-16). The grid covers the `levelmap` box and 5 m more, and
counts every face up with room above. On School only 17 % of its 2.29 M
floors were reached from a spawn: 58 % lay on `level_02_border` (the
invisible boundary: walls with lids at 35 m and 48 m and a plate at -9 m,
on purpose, not a leftover), 20 % on the ground outside it, 3 % on roofs
one drops from, 1.5 % in shut rooms (one wall from open floor), the rest on
props' tops. `WalkGrid::build_kept` (step 5 of the grid's build) keeps the
floors reached from the map's `grid_seeds` (spawns, batteries, containers,
stations), those leading into them (roofs) and what those link to; the
seeds are in `grid_key` and `GRID_ALGO` is 2.

Proof, `kept_grids_are_the_whole_grids_less_floors_no_body_gets_to` (all
72 projects): each kept floor is a floor of the whole grid at the same
place with the same headroom and the same links, every floor the seeds
reach is kept, every seed finds the same floor. So a way between kept
floors is as before. The decisions still changed on the real maps (not in
the three whole matches): `Tactics::spots` probes points round a place and
takes the floor `locate` finds there, preferring high ones, and `locate`
found floors no body gets to (a prop's top, a shut room); a bot then went
for a spot with no way (`SRP_TRACE_DECISIONS` showed the first such goal
on level_01, 36 s in). Now those probes land on floors a body reaches.
Failed searches, the same runs: level_01 4 v 4 205 to 128, 10 v 10 923 to
203; level_03 10 v 10 2,909 to 1,837; School 4 v 4 725 to 445, 10 v 10 618
to 535; level_06 about the same (2,007 to 2,021, 4,077 to 4,155).

| | whole grid | kept |
|---|---|---|
| School (level_02/day_2) floors | 2,291,518 | 476,544 (21 %) |
| grid, level_02/day_4 | 64 MB | 15 MB |
| tactics, level_02 | 16 MB a map | 3 MB |
| grids of all 72 projects (unshared) | 2,346 MB | 1,012 MB |
| kept share by level | | 01: 51 %, 02: 21 %, 03: 88 %, 04: 31 %, 05: 84 %, 06: 47 %, 07: 74 %, 08: 10 % |
| map_ready (tactics build), level_06 | 479 ms | 360 ms |

A process loading level_01/day_4 from built files peaks at 72 MB (62-83
before; other parts dominate there). The build itself still makes the
whole grid first, so a first load's peak is as before (146 MB).

**Batteries go in at the use box** (a decision change, 2026-10-16). The
bots took a battery home to the container's `position`; a use is a 2 m ray
from the eye that must hit the container's use box (`client_patch.md`,
`detect_usable_object`, PROVEN), and the position lies 0.1-2.1 m from the
box on level_01, 03, 07, 08 but 13-152 m on level_02, 04, 05, 06. On
level_06 both positions are in the fire pit, 4 m apart, a floor no allowed
floor leads into: every carrier's search was no way, and the 4 v 4 bots
stood without a route 99.7 % of their ticks (0 kills). Players' 783
put-ins (2026-10-14 worlds) are a median 4-6 m from the box, 22-56 m from
the position. `Container::use_point` is a metre under the box's centre (as
a body's feet: `locate` finds the floor beside the box, not its lid, and a
look a metre over it is at the box); on all 16 containers of 8 maps that
floor is 0.8-1.1 m from the box, the eye 0.8 m or less from it, and walked
to from the own spawn (the position: 7 of 16 not). The bots' containers
and the test game's puts and thefts use it. Failed searches, 180 s runs:
level_06 4 v 4 2,021 to 528, 10 v 10 4,155 to 1,446; level_05 4 v 4 1,139
to 27, 10 v 10 3,343 to 672; level_03 4 v 4 1,559 to 930. Batteries put
in: level_06 0 to 10 and 0 to 16, level_04 1 to 9 and 1 to 16. The bots'
time 0.17 to 0.35 % of a core (level_06 4 v 4): they walk and fight now.
The three whole matches (no use boxes) and level_01 (Team Deathmatch) are
unchanged. Still failing on level_06: batteries ringed by fire (a way only
through it; players go there).

**Goals with no way left, and a way out of a zone** (decision changes,
the owner's word of 2026-10-16). The bench (`real_bot_cost`) now prints
the living bots' ticks with a goal and no route ("stood"), by play, and
the batteries delivered; `SRP_BENCH_SEED` gives other matches of the same
map and size. Three causes of standing, each fixed and measured on six
maps (level_01-06, 4 v 4 and 10 v 10, 180 s):
- *Goals no walk gets to.* `Tactics::spots` and `fine_goal` took any floor
  off hazards, so roofs one only drops from were spots (Station's spots
  prefer height). `TacticsBase` keeps a bit a floor for the floors a walk
  from a spawn gets to (`walkable`, `TACTICS_ALGO` 2; all floors on a map
  without spawns); spots, `fine_goal` and `Ctx::reachable` take only those.
  Stood: level_03 34 to 12 % and 29 to 6 %, level_02 15 to 3 % and 12 to
  4.5 %, level_04 13.5 to 3 % and 11 to 4 %, level_01 11 to 4 % and 10 to
  6 %.
- *A battery on a shelf.* School's at (43.8, 22.7, 92.3) lies on a shelf
  top past the jump; two on level_04 alike. `Tactics::stand_to_take`: a
  battery on a walkable floor is fetched as before; else the bot goes to
  the walkable floor whose crouched eye is nearest it within a use's reach
  (`USE_REACH_M` 1.8 of the client's 2.0 m ray) in a clear line, and takes
  it from there (School's: 0.7 m from the floor behind the shelf). Only
  level_02 and level_04 change; delivered 15 to 17 and 26 to 27 on
  level_02.
- *A battery ringed by fire* (level_06) passes `reachable` (its floor is
  fine) but every way in crosses the fire. A search that proves no way
  from a floor the bot may walk on is kept 20 s (`Bot::no_way_to`); Collect
  leaves the battery and a fetch gives up. And a body inside a zone's
  margin (pushed or stepped in) had no route to anything and stood in the
  fire: `Tactics::escape` walks it out to the nearest floor at least 1.5 m
  off whose neighbours are all allowed. On seeds 1-3, level_06 stood 28.5
  to 8.4, 17.3 to 4.1, 15.2 to 4.2 and 28.1 to 6.7 %; delivered 17 to 18,
  6 to 12, 19 to 21, 14 to 18; level_02, 03 and 05 the same or nearly.

The three whole matches are unchanged by all three. Not checked in a
game: whether the client takes School's shelf battery from the floor
behind it (the eye is 0.7 m from it, well within the ray).

Not changed: the decisions (proven above). The first load of a map on the
host still builds it (and writes its files); the build's peak (near twice
the grid) comes only then.

### Searches in slices (2026-10-16)

A route search ran to its end inside the poll that asked for it: the worst
took 115,000 floors expanded, 26 ms here, and a poll up to 81 ms (level_04
10 v 10), about 0.2 s on the host's core. Now a poll's searches share a
budget of floors expanded, as Detour's crowd shares its path queue's
iterations (`dtPathQueue::update`, `MAX_ITERS_PER_UPDATE`):
- `WalkGrid::start_search` and `SlicedSearch::step`: the A* set up once and
  run a few floors at a time, the same floors in the same order, so it ends
  with the same answer. A search under way holds its own arrays; the thread
  keeps two sets (12 bytes a floor each then, 8 since "Search memory"), one
  for a search left under way.
- `Regions::start_route_search` and `RouteSearch::step`: the regions'
  search the same way, each stage (the corridor, widened, the whole grid)
  in slices. Its corridor is a bit a region of its own, not the thread's
  marks.
- `Bots::poll`: at most `search_floors_per_poll` floors a poll for all the
  match's bots (`ThinkTuning`, default 10,000: about 2 ms here, 5 ms on the
  host), besides the 2 searches. A search past them is left under way, one
  a match, and goes on first at the next polls (`go_on_searching`); no
  other search starts meanwhile. Its bot walks the way so far (Detour's
  partial path, `finalizeSlicedFindPathPartial`: to the floor expanded
  nearest the goal) and takes the route at its next tick. A search whose
  bot left, whose goal moved or whose hazards changed is dropped. The floors
  are counted, not timed: the bots stay deterministic.

**Proof.** With no bound (`SRP_BENCH_SEARCH_FLOORS=max`), every decisions
digest is the one before (the three whole matches, level_01-06 4 v 4 and
10 v 10). A bound changes when a route comes, so the decisions differ; the
play was compared over ten seeds of each of the 12 real-map runs (120
matches of 180 s a budget):

| budget | stood | delivered | kills | failed searches | sliced |
|---|---|---|---|---|---|
| none | 5.4 % | 1,491 | 1,364 | 16,729 | 0 |
| 20,000 | 5.3 % | 1,481 | 1,396 | 15,700 | 8,291 |
| 10,000 | 5.5 % | 1,477 | 1,381 | 16,067 | 22,378 |

Per seed the totals spread: kills 114-164 (sd 15), delivered 137-159 (sd
7), stood 4.6-5.8 %; so the budgets play as no bound does. Without the
partial path (the bot stood till its next tick) three seeds gave kills 447
to 411 and 426 and delivered 441 to 429 and 430, every seed lower: that
version was not kept. The bots' time over the 120 runs: 2,052, 2,055 and
2,064 ms a second summed: no more work.

**Polls** (one run alone, the default seed, fast desktop core; the host
is about 2.6 times slower):

| run | worst poll, no bound | 10,000 | p99.9, no bound | 10,000 |
|---|---|---|---|---|
| level_04 10 v 10 | 80.7 ms | 7.0 ms | 8.1 ms | 3.7 ms |
| level_02 10 v 10 | 33.2 ms | 5.2 ms | 5.6 ms | 3.3 ms |
| level_03 10 v 10 | 31.1 ms | 9.7 ms | 8.8 ms | 4.3 ms |
| level_01 10 v 10 | 26.1 ms | 3.3 ms | 12.9 ms | 2.8 ms |
| level_06 10 v 10 | 13.4 ms | 5.7 ms | 5.2 ms | 2.8 ms |
| level_05 10 v 10 | 9.9 ms | 3.6 ms | 3.9 ms | 2.7 ms |

Every 4 v 4 run's worst poll is 2.7-3.7 ms (was 5-22 ms). What is left of
the worst is the commander's tick and the bots' thinks (level_03 10 v 10:
the commander 4.8 ms), not the searches.

### Search memory (2026-10-16)

The A* arrays the match thread keeps were 12 bytes a floor of the largest
grid searched (a mark, a cost, a parent), two sets while a search is left
under way, kept for the life of the process: 16.8 MB after a match on
level_06 (701,698 floors).

**What was measured first.** The plan was a table sized to the corridor
for the corridor stages, as Detour's node pool is sized to the search
(`dtNodePool`: a fixed number of nodes and a hash, not one per polygon).
One bench run of each of the 12 real-map runs, every corridor counted:

| | p50 | p99 | max |
|---|---|---|---|
| floors in the corridor's regions | 8-18 k | 50-154 k | 60-205 k |
| the same, share of the grid | | | 17-47 % |

Of the searches that got past the regions' search, 0-42 % a run went on
to the whole grid (level_01 4 v 4 none, level_06 4 v 4 133 of 314).

The widest corridors hold up to half the grid, and searches that go to the
whole grid (the stuck spots shut the corridor) come in every match, so the
whole-grid arrays are needed all match long anyway: a corridor table would
not lower the peak. Not built.

**What was built.**
- **8 bytes a floor, not 12.** The mark is 2 bytes (a search number; the
  marks are cleared every 32,766 searches, 1.4 MB written). The parent is
  2 bytes: the way back from the floor to the one it was reached from (that
  floor's index in its cell, the direction, the reach; `links_back`,
  `back`). The cost stays 4. The search's order and answers are the same.
  New arrays are zeroed by the allocator, so pages no search touches are
  not taken from the system. Level_06: 11.2 MB for the two sets, was 16.8.
- **Given back when no match has bots.** Each `Bots` holds its thread's
  arrays (`SearchMemory`); when the thread's last match with bots ends,
  `mapdata::free_search_memory` drops them and the region marks, and with
  glibc calls `malloc_trim(0)`. The log says how much went back.

**Why `malloc_trim`.** glibc raises its mmap threshold to the size of a
large block once it is freed (up to 32 MB), so the next such arrays come
from the heap and stay with the process when freed. Checked on WSL Ubuntu
(glibc 2.39) with the arrays of level_06 run as three matches on a thread
(`private/mapdata-digests/trim_check.py`): after the first match the freed
arrays stayed resident (23.0 MB held, 23.0 MB after the free) until
`malloc_trim`, which brought it to 12-14 MB. The trim also hands back what
a map's build freed on its own thread.

**Proof.** Every decisions digest is the one before, at the default
budget: the three whole matches and level_01-06 4 v 4 and 10 v 10.
The searches' time, the 12 real-map runs one process at a time, the
merged branch and this change in turns, four rounds each (64.9 M floors
expanded a round by both): 242-284 ns a floor expanded before, 236-287
after (means 269 and 265), the bots' time 144-170 and 137-171 ms a
second summed. The same within the rounds' spread: no more work.

### Straight lines (2026-10-16)

A route is the grid path pulled straight (`path::pull`): from each corner
the line is tried to the next floor, then the one after, until a line
fails; each try walks the whole line again, half a cell at a time.

**What it cost.** The bench now counts it (`pulled straight`). Over the 12
real-map runs it took 1.0-1.2 s of the bots' 21-25 s: about 5 % (the
searches 44-77 %). A route made about one check a floor (1.78 M checks for
1.83 M floors).

**Halving was not taken.** The plan was to look for the corner by halving
the stretch. But a line from a corner may fail to floor j and pass to
floor j + 1 (a wall's end, a hole the line just misses), so halving finds
another corner than the one-by-one tries: other routes, other decisions.
Grid string-pulling papers say the same of greedy smoothing (Han, Uras and
Koenig 2020, SoCS: greedy smoothing leaves turns in open space and is not
optimal), and fixing that means a different algorithm (Theta*, string
pulling on the grid), not a faster check. For about 2 % of the bots' time
it was not worth a change of play.

**What was built.** `WalkGrid::walks_straight` (mapdata), the same check
made inside the grid: it follows the line by the cells' whole-number
coordinates and the packed links, where the bots' check looked up every
candidate floor's place (its cell, a division, its height). The same
floors are tried in the same order with the same float sums, so the
answers are the same:
- 400,000 random lines on level_06, 01, 03 and 02, with and without holes
  shut: the same answers; 0.71-1.26 µs a check, was 1.09-1.84 (each check
  over every line in turns, the best of three).
- Every decisions digest is the one before (the three whole matches,
  level_01-06 4 v 4 and 10 v 10).
- The 12 real-map runs, three rounds each in turns: pulling 568-650 ms,
  was 984-1,166 (44 % less), the same 1,784,848 checks.

### Distance fields to the objectives: measured, not built (2026-10-16)

The plan: flood a distance field once to each fixed objective (containers,
stations, battery spawners), shared by every bot heading there, so those
bots need no search. Flow fields and Dijkstra maps pay when many agents
share one goal and the floors they may use stay the same; a change of
those floors means a new flood or a repair (D* Lite).

A bot's search is exact for one set of let-through floors: the zones shut
at that moment (they switch on and off), the side's spawn areas, and the
spots it got stuck at. So a field could only stand for searches with the
same goal floor, side, objective flag, set of shut zones and no stuck
spots. One bench run of the 12 real-map runs, every search keyed so (the
measuring patch is `private/mapdata-digests/measure-fields.patch`):

| run | searches | goals | most searches to one of the 10 costliest keys |
|---|---|---|---|
| level_06 10 v 10 | 1,212 | 327 | 47 |
| level_01 10 v 10 | 2,147 | 1,561 | 20 |
| level_03 10 v 10 | 1,824 | 1,020 | 25 |
| level_02 10 v 10 | 971 | 450 | 44 |
| level_04 10 v 10 | 1,373 | 682 | 57 |
| level_05 10 v 10 | 1,844 | 749 | 41 |

The bots' goals are many and spread (cover spots, crossfire places, cut
points, lying batteries), not the few objectives. Counting a field as one
flood of the grid and built only where its searches expanded more than
that (the best case, no flood again when zones switch): fields would save
nothing on 9 of the 12 runs and 3.7-5.2 % of the floors expanded on the
other three (level_06, 04 and 05 10 v 10). Against that, 2-4 bytes a
floor a field (0.4-2.8 MB each), a flood each time the zones' set changes,
and other routes (a flood breaks ties between ways of equal cost
otherwise than the A*). Not built.

### On host-like limits (2026-10-16)

The 12 real-map runs as the host runs them: a Linux build (glibc 2.37,
built in WSL, the toolchain and target on E:), in a systemd scope with
512 MB, no swap and a 100 % CPU quota, pinned to one core
(`private/host-measure/run-limited.sh`; checked inside: `memory.max`
536870912, `memory.swap.max` 0, `cpu.max` 100000 100000). The bench now
prints the process's resident memory on Linux: with the world (what the
build freed handed back first), with the bots' tactics, and the peak in
the match (`/proc/self/status`, the peak reset after the tactics).

| map | with the world | with the tactics | peak, 10 v 10 | bots, 10 v 10 | worst poll, 10 v 10 |
|---|---|---|---|---|---|
| level_06/day_2 | 124 MB | 144 MB | 145 MB | 13.7 ms a second | 8.0 ms |
| level_01/day_4 | 64 MB | 76-78 MB | 77 MB | 20.8 ms a second | 4.5 ms |
| level_03/day_3 | 73 MB | 93 MB | 96 MB | 24.6 ms a second | 9.4 ms |
| level_02/day_2 | 96 MB | 111 MB | 111 MB | 15.1 ms a second | 4.0 ms |
| level_04/day_2 | 96 MB | 107 MB | 109 MB | 20.5 ms a second | 7.5 ms |
| level_05/day_2 | 79 MB | 84 MB | 84 MB | 11.4 ms a second | 3.2 ms |

- No run was stopped and the scope saw no out-of-memory event. Its peak,
  360 MB, counts the page cache of the map files read and each world's
  build from the map file; the server reads built files instead (no build
  peak).
- A match adds at most 3 MB over its tactics (the search arrays and the
  bots' own state); the world and the tactics are the cost of a map.
- The bots' time on one core is what it was with all cores (one thread).
  On the host's core, about 2.6 times slower: 10 v 10 30-65 ms a second
  (3-6.5 % of the core), the worst poll about 25 ms.

**The memory budget.** The server used about 190 MB without bots
(2026-10-14). Each map that bots play adds its world and tactics, 84-145
MB, shared by every match on it, and the store keeps one more map past
those matches use (`maps::KEEP`):

| bot maps held | the server with them |
|---|---|
| one, the largest (level_06) | about 335 MB |
| one playing and one kept (level_06, level_02) | about 445 MB |
| two playing and one kept (level_06, level_02, level_03) | about 540 MB, past 512 |

So one match of bots at a time fits with room; two at a time on the large
maps, with a third map kept, would pass the container's 512 MB.

**The host itself** (read 2026-10-16 03:49 UTC, no bots on it): 955 MB in
all, 779 used, 176 MB available, 307 MB of its 2.3 GB swap file in use.
The server's container held 204 MB of its 512 MB, Caddy 19 MB; by
process, `dockerd` 258 MB, `srp` 187 MB, `containerd` 28 MB, the kernel's
slab 97 MB. A larger container limit would not help: one bots' map
(up to 145 MB) takes nearly all the memory the host has free, and a
second would push the server into swap whatever the limit. The limit
stays 512 MB. What could make room: `dockerd` at 258 MB is far over its
usual size (a restart or a look at its logs and caches, an action on the
live host), or one bots' map at a time on the server.

### One bots' map at a time (2026-10-16)

The owner's choice after the host check: at most one bots' map in memory.
The store (`bots::maps`) holds at most `SRP_MAPDATA_AT_ONCE` maps (default
1, `maps::AT_ONCE`), counting those loading, those matches hold and the
one kept after its match:
- A match on a map already in memory (another match on it, or the kept
  one) has it at once, as before.
- A match on another map, while the maps in memory are all held by
  matches or loading, waits (`MapState::Waiting`): its bots run the stage
  0 script, as they do while a map loads, the match asks again at each
  poll, and its bots take the map once it is loaded. The log says when it
  began to wait and when the map came.
- A kept map no match holds (by its tactics, or only by its world when a
  match built tactics of its own) is dropped to make room before the next
  is read, and the memory it freed is handed back to the system
  (`mapdata::trim_freed_memory`, `malloc_trim` with glibc).

So the server holds at most one map's 84-145 MB for bots. A second match
of bots on another map plays its first minutes, or all of it, on the
script; its bots never stand without a map. Test:
`one_map_at_a_time_waits_for_room_and_drops_the_idle_one` (waits while the
first map is held by its tactics or only its world, has the map in memory
at once, drops the kept map for the next).

### Guard spots see the container's box (2026-10-16)

The real-map bench showed play 18 (guard the lead) taken and ended about
every think: 27,609 takes over 96 matches of 180 s (12 runs, seeds 1-8),
64,707 over 96 of 600 s. Every end was `no spot with a sight line`, on
every map.

**Cause (proven).** A guard spot must see the container: a ray from its
crouched eyes to a metre over the container's place in the view, which is
the middle of the container's use box (`Container::use_point`, the box's
centre less a metre). On the real maps the container is a solid in the
sight tree, so no ray from outside ever reaches its middle. Measured with
a temporary probe on level_01, 04, 05 and 06: of 30-62 spots round each
container, none saw it; every blocked ray stopped either on a wall or on
the box itself (its first hit 0.00 m from the use box, 0.3-1.0 m short of
the middle; the boxes are about 2 x 1.7 x 1.3 m). With no spot, `start`
gave `NoSpot`, the first step failed, the commander was called at once,
and the play, still the best candidate (no time since it last ran
counts in its score), was taken again: the bot stood still while it
lasted, and its team's commander ran again at every failure.

**Fix.** `SpotQuery::sight_body`: the solid the sight target lies in. A
spot sees the container when the ray hits nothing or first hits the
container's use box (`Tactics::sees_body`, within 0.1 m of the box);
`Tactics::container_body` finds the box of the map's container used at
the view's point. Play 18 alone sets it (the only spot query aimed at a
container); the other plays' queries are as before. Test:
`a_solid_container_box_is_seen_on_its_faces` (fails without the fix).

**Play, same seeds before and after** (the bench line now prints the
points and the batteries stolen):

| | 180 s, before | after | 600 s, before | after |
|---|---|---|---|---|
| GuardLead taken | 27,609 | 319 | 64,707 | 178 |
| delivered | 1,584 | 1,588 | 2,528 | 2,526 |
| stolen | 1 | 1 | 28 | 19 |
| points at the end | 1,583 | 1,587 | 2,500 | 2,507 |
| kills (both sides) | 801 | 768 | 3,429 | 3,385 |

Every decisions digest of the three whole matches on the stand-in field
is the one before (its container is no solid); on the real maps level_06
4 v 4 and 10 v 10, level_03 10 v 10 and level_02 10 v 10 change. Kills
fell 4 % at 180 s and 1.3 % at 600 s, both sides counted: guards hold
crouched instead of roaming, and the extra commander runs are gone (on
level_02 10 v 10 at 600 s no bot guarded after the fix, and kills still
differ). Points and thefts, what the play is for, are as good or better.

## 7. Anomalies on the match server

`match-server` `v026g::bots::anomalies` runs every anomaly core of the map
on the match clock with the client's rules, and `bots::hazards` turns that
into which floors a bot may walk on when. The facts (client 0.26g, PROVEN
unless marked; the research of 2026-10-14 is private):

- Only two kinds switch. level_03: three electric cores, two zones each,
  strictly alternating, each on 40-60 s (drawn), about lethal per hit
  (0.6 × 2 feet). level_02: the fruit cloud, five levels by an energy that
  bodies moving in it raise (35-70 a second, LIKELY) and that falls 25 a
  second. Every other zone is on all match. Zones with `max_hit` 0 (sound,
  sight) are no hazard.
- Each core has its own LCG `x = x·0x08088405 + 1`, seeded with
  `static_seed ^ crc32(full_name)`; `static_seed` is the static info's id,
  `registry::static_id(match_id)` = `match_id | 0x80000000`. Ticks on the
  50 ms grid: energy, state, groups (a sequential group switches when its
  time comes and draws the next: `t + min + (x·(max−min))>>32` ms), then
  artefact spawns (four draws each, from the same stream).
- Each checked world holds every core's state in its objects section:
  `[u8 artefact pending, u32 next] f32 energy, u8 state, u32 finish, per
  group (u32 next, mask bytes), 5 bytes per active zone (u32 last hit, u8
  someone inside: 0 or 0xFF), u8 events, u32 random`. A core is found by
  its random value (the last known one stepped until it appears), then
  read backwards.

Checked against the saved worlds of 36 matches (17 on level_03, 19 on
level_02, every world; `real_worlds_agree_with_the_simulation`): the first
artefact times of one match come out as the research found them; every
core is found in every world (25,809 reads); the simulation from one world
to the next has the same zones and switch times (within 50 ms) in
99.86-99.92 % of the electric cores' worlds and in all of the static
ones'; the fruit's state differs in 3 % (players raised it). Never synced,
the two electric cores without artefacts keep the same zones on for whole
matches in 99.9 % of the worlds, the one with artefacts in 90.7 % (a
pickup draws unseen): hence the sync.

On real maps (`real_hazards`, 0.5 m margin): 1-11 harmful zones a map,
4,700-46,000 floors marked (level_08's river 520,000), 1-98 ms.

The match server does (wired): `Bots::tactics_ready` attaches
`Anomalies::new(&tactics.world.map, static_seed)` (`static_id(match_id)`),
`Bots::poll` advances it on the match clock, and each trusted world
(`Game::observe` -> `observe_driven` -> `anomalies_from_world`) syncs it.

Timed anomalies (exe patch 42, `dynamic_anomalies.md`): energy states
charging 3000 (20 s) -> damage 2000 (4 s) -> artefact 1000 -> after the
pick-up 50 (60 s) -> idle 0. The clients' rule charges an idle one (the
server never predicts that: an idle core stays idle in the simulation, its
zones off); a world that shows it charging gives its damage 20 s on, and a
world after the pick-up turns it off. The simulation follows a state's end
on the first tick past it, and chooses the next state on the tick after,
as the client does (`Sim::idle` no longer skips that tick; a state's end
used to be taken on its own tick). Test:
`a_timed_anomaly_follows_the_worlds_and_is_never_predicted_to_charge`.
`mapdata-dump --patch <resources.db.patch_n>` (repeatable, mount order)
dumps maps that only patches have (London) or change.

Rules for bots (`hazards`): a zone of a timer group is active by the
simulation, its on spans widened by 1 s on each side; a route may cross it
only while it stays off for `pass_ms` (4 s). A zone of an energy core that
a body moving inside would switch on within 10 s (the fruit's lvl1 at 20
and lvl2 at 400) is always active; its higher states' zones (1000 and up)
follow the energy the worlds give, falling between worlds. Every other
zone is always active, as is every zone before `attach`.

## 8. Not done, not proven

- The match server loads the files (§6), but no bot moves by them yet.
- The grid against the client's real movement (step, slope, jump height,
  narrow doors, the one-way bases) is not checked in a game.
- Grid memory: done (§6, "Lighter maps"); 15 MB on level_02 (floors no body gets to dropped).
- Files are not compressed (`lzma-rust2` is in the workspace).
- The objectives' reach is only checked for level_05 batteries; a
  container's use box is a trigger shape, not a floor (`Shape::contains`
  tells whether a point is inside).
- The yaw of a spawn (`math::forward`, `math::heading`) follows the
  engine's `create_rotation_y`; how a client's input yaw maps to it is not
  checked.
- Anomalies: the cylinder's axis and half height are LIKELY (the editor's
  drawing); a zone counts for a body when it comes within the margin of a
  standing body's axis (GUESS: the client's contact test is not read); the
  fruit's energy rises (per tick or per second, explosions) are not
  simulated; artefact pickups are not seen (their draws come back with the
  next world); a hit curve is read as straight lines between its keys (the
  game's are Hermite); the receiver's armour is not read, so the damages
  are raw.
