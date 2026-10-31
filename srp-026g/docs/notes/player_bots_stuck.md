# Player bots: where they got stuck (0.26g, 2026-10-23)

The owner's report: the server's bots get stuck at walls on many maps and
modes. This page is the evidence from the live server's logs, what the
causes were, what changed, and what is left. Tags as elsewhere: PROVEN,
LIKELY, GUESS.

## 1. Data

- **The live 0.26g host's daily logs** (`logs/srp-2026-10-12.log` ..
  `-23`, read only): the bots' lines (`player N (the server's bot) at T ms
  ...`: `is alive at`, `takes <play>`, `is stuck at [x, y, z]: <recovery>`)
  and the `Match ... made: map .. (<map>_<mode>, mode m)` lines. Bots
  played from 2026-10-15 15:12 UTC: **534 matches with bots, 457 of them
  with a stuck line, 46,720 stuck lines** (23,435 `BackOff`, 7,154
  `Sidestep`, 16,131 `GiveUp`). `SRP_BOT_TRACE_DIR` is not set there, so
  there are no decision traces: the play the bot ran is the last `takes`
  line before.
- **Maps and modes**: every level (01-08) in mode 2 (batteries) and 4
  (TDM); the bots' map files are the `day_2` / `day_4` projects. No bot
  match in mode 3 or on a research map was logged.
- **The map data** (`mapdata-dump` files of the shipped `resources.db`)
  and the project configs, to look at each spot; scratch tools (a grid and
  mesh probe, a classifier of the logged spots) were not kept.
- **20 recordings of player matches** (2026-10-14/15, 1,897 player
  minutes, all eight levels): the players' own keys, §6.

A stuck line is one bot at one spot; one bot caught for minutes writes one
every 5 s. So the lines are grouped as **incidents**: one bot life at one
3 m cell, 19,443 in all.

| map (project) | stuck lines | of them `GiveUp` |
|---|---|---|
| level_08 (Cologne) TDM | 9,824 | 5,701 |
| level_08 batteries | 7,595 | 4,025 |
| level_02 TDM | 6,467 | 2,188 |
| level_05 TDM | 4,388 | 497 |
| level_02 batteries, level_07 TDM | 3,481 each | 768, 953 |
| level_07 batteries | 2,543 | 1,064 |
| level_01 batteries | 2,302 | 22 |
| the other eight | 501-1,334 each | |

`GiveUp` is the third time at the same spot within 20 s: the bot did not
get away. Of the 46,720 lines, 1,214 runs are a body **frozen**: four or
more stuck lines in a row within 10 cm, for 20 s to several minutes.

## 2. How each spot was read

Each stuck point was put on the walk grid and the walker mesh of its map:
the nearest floor, the links round it (a jump up beside it, its height),
the walker mesh within the body's reach at knee, waist and head height, the
bullet mesh likewise, and the faces of 50-60° within 0.8 m. The classes,
by incidents (lines):

| class | incidents | lines | frozen runs |
|---|---|---|---|
| beside a ledge the grid jumps up (a crate, a desk, a low wall) | 5,022 | 25,221 | 984 |
| nothing of the walker mesh within reach ("free") | 6,817 | 9,143 | 15 |
| against a wall the grid knows | 3,616 | 5,076 | 25 |
| at the foot of 50-60° ground | 2,532 | 3,294 | 19 |
| against the bullet mesh only (no walker mesh there) | 513 | 1,630 | 87 |
| at a jump up of 1.05-1.3 m | 439 | 772 | 19 |
| at a lower jump | 216 | 354 | 6 |
| off the grid (no floor within 0.6 m) | 288 | 1,230 | 59 |

## 3. The causes

### 3.1 A ledge the grid jumps up is no wall to its erosion (PROVEN)

The grid erodes a floor whose side has no link; a side with a **jump** up
(0.6-1.3 m: a crate, a desk, a sill, a low wall) counted as a link, so the
floors at the foot of such a ledge stayed, nearer to its face than the
body's radius. A way along it, or between it and a wall, was one the body
does not fit. The largest class (26 % of the incidents, 54 % of the lines,
half of the frozen runs).

**Cologne (level_08), the players' report of 2026-10-23** ("bots from left
side team on Cologne map get stuck on this desk", two screenshots: the
hall with the rails, two bots against a blue-grey desk before a concrete
half-wall). The desk is `props/furniture/workbench_02` (scale 1.1, at
(-49.909, 32.017, 42.569), its walker box 0.92 m high, x -50.64..-49.14,
z 42.53..43.58). The half-wall's face is at z 41.89: the gap is 0.64 m, the
body 0.76 m. The grid had floors at z 42.375 all along the gap (the desk's
side a jump). In the logs: **13,888 stuck lines at that spot, all of them
the left team's bots (side 0, spawning at x ≈ -67), in 68 matches (40 TDM,
28 batteries)**, most of them frozen at (-50.966, 32.068, 42.307) for
minutes, through every recovery: the body wedged at the desk's corner with
the wall at its back. Its way out of the base east, to every objective,
passed there. PROVEN (the geometry from the game data, the body's frozen
point 0.39 m from the desk's corner and 0.42 m from the wall).

The same on level_07 at (-26, -4), (-37, -7), (25, -24); level_02 at
(20-27, 64); level_06 at (-35, 131) and others.

**Fix** (`grid.rs`, `erode`): a side whose link is a jump up is a wall: the
floor at the ledge's foot goes, and the jump onto it starts a cell back
(the relink pass: a walk to a floor gone that was a ledge's foot becomes a
jump past it). Tests: `a_body_walking_by_a_crate_it_could_jump_on_keeps_its_radius_off_it`
(made up), `cologne_no_way_between_the_workbench_and_the_half_wall` (the
real map, both projects; it fails with the old rule: floors along the
gap).

### 3.2 A jump lands on no room (LIKELY)

level_07 (-68, -12): a wall 1 m high and a cell thick, ground both sides;
the grid jumped onto its top and dropped down the other side. 1,713 lines
in 46 matches, bots jumping at it for minutes. The body (0.76 m) does not
stand on a 0.25 m top. **Fix** (`grid.rs`, `lands`): a jump lands only
where two more floors are walked on along it. Tests:
`the_top_of_a_thin_wall_is_no_landing`, `level_07_no_jump_onto_the_thin_wall`.

### 3.3 Jumps the client does not make (LIKELY)

The grid's jump reach was 1.3 m (a GUESS). The client's jump follows its
animation (`fall_damage.md`), not measured. Stuck incidents at a jump's
foot by its height, per 1,000 jump links of that height (the links counted
on nine of the projects): 4.5 at 0.6 m, 6.2 at
0.7, 2.3 at 0.8, 2.2 at 0.9, 3.8 at 1.0, 7.9 at 1.1, **16.6 at 1.2, 18.9 at
1.3**. level_02 (24, 64): a platform 1.24-1.49 m over rubble, 2,248 lines
in 40 matches, bots seen 0.8-1.0 m up in mid-jump and never on it. **Fix**:
`Body::max_jump` 1.05 m. Test: `level_02_no_jump_up_the_high_platform`. On
all 16 projects the bots play, each team still gets to the same enemy
spawns as before (the same counts).

### 3.4 Ground too steep (PROVEN limit, LIKELY effect)

The grid took ground up to 60° (LIKELY). The client: the character
controller's `impassable_slope` (0x7de10) refuses ground whose normal's y
is under a cosine set at start-up (0x84d780: `cos(0.8727)`, **50°**);
`need_slide_on_supporting_surface` (0x7bc20) slides a body standing still
from 40° (0x84d800: `cos(0.6981)`). PROVEN. 2,532 incidents at the foot of
50-60° faces (level_05's rubble most: 1,010 on its TDM project).

Taking 50-60° faces off the grid altogether cut level_06 in two (no team
got to the other's spawns): rough ground's facets are steeper than the
slope walked, and the body steps up them. **Fix** (`grid.rs`,
`cut_steep_climbs`): faces up to 60° stay floors; a walk up onto steep
ground (its face over 50°, or its height by its neighbours rising faster
than 50°) is cut once the climb is more than a step (0.6 m) over the
walkable ground it began on; a way down stays; the cut side is no wall to
erosion. Tests: `ground_steeper_than_fifty_degrees_is_not_walked_up` (a
45° ramp walked up, 55° not, a curb's steep face stepped). Not fully
explained: level_05's rubble at (-52, 12), where the mesh's average is
46° and bots still stood caught (§5).

### 3.5 Models without a walker collision (LIKELY)

Some placements of the walking body name a `walker_collision` the archive
lacks (fruit trees, level 3's anomalous trees, a bench, a wheel: 0-60 a
map); the grid saw nothing there. Of the 513 incidents with only the
bullet mesh in reach, most are by such trees: level_02's anomalous tree
`level/03/anomal_tree_02_` at (18.2, 15.6, 117.4) alone 300 (two bodies
frozen together in it for minutes at (18.83, 16.20, 118.69), nothing of
the walker mesh within 1.4 m), fruit trees on level_01 and level_02.
**Fix** (`project.rs`, `MapData::triangles`): the walking body meets the
model's own `collision` (the same scale) when its `walker_collision` is
empty. It takes 0-0.4 % of a map's floors and no spawn's reach. Not read:
what the engine loads for a missing one. Tests:
`a_missing_walker_collision_is_stood_in_for_by_the_models_collision`,
`level_02_the_anomalous_tree_is_in_the_way`.

### 3.6 "Stuck" while not walking (LIKELY)

The watch called a bot stuck when two worlds 1.5 s apart showed its body
moved under 0.4 m **while it had a route**, not while it walked: a bot
standing to aim, waiting for its search, or creeping with a route in hand
was "stuck" too, and backed off, jumped and dropped its play. 6,817
incidents (35 %) had nothing of the walker mesh within reach of the body;
Buddy and PairUp (a bot by its partner) lead them. **Fix** (`body.rs`,
`stuck.rs`): the reckoning keeps the way the bot's own keys moved the body
on the walk grid between two worlds (`BodyTrack::last_meant`, step by step:
a turning body's way, not its displacement); stuck needs under 0.4 m moved
while the keys meant at least 1.2 m. Tests:
`the_way_the_keys_meant_between_two_worlds_is_kept`,
`only_a_body_that_meant_to_walk_is_stuck`, the match end to end
(`with_the_map_the_servers_bot_sees_its_body_in_the_worlds_and_walks_smoothly`).

### 3.7 Recovery that tried the same way (backstop)

`BackOff` stepped back and to a random side, `Sidestep` to a side, and
`GiveUp` only stood: a body wedged in a corner tried the same way again
and again (the Cologne bots for minutes). **Fix** (`stuck.rs`,
`Stuck::escape`, `mod.rs`): each time again at a spot it gets off another
way: back and to one side, back and to the other, then to each side
straight; `GiveUp` steps too before it stands. Test:
`each_time_at_a_spot_it_gets_off_another_way`.

## 4. After

The grid's build changed (`GRID_ALGO` 4: built grids of before are not
used; the server builds and stores them again at first use). The map files
need no new dump. On the 16 projects the bots play, each team gets to the
same spawns as before; the grids lose 0.01-0.4 % of their floors.

Of the logged incidents, by class, the share whose spot changed on the new
grid (the body's cell is no floor now, or fewer floors more than 0.7 m up
are walked or jumped to within 3 m): beside a ledge 69 %, a jump of
1.05-1.3 m 89 %, off the grid 99 %, the bullet mesh 64 %, steep 38 %,
against a known wall 47 %, "free" 25 %. Not seen in a real game yet.

## 5. Left

- **The client's jump height** is not measured (1.05 m is from the logs).
  A real game, or the jump animation's curve, would settle it.
- **level_05's rubble** (-52, 12) and the like: bots stood caught where the
  grid's slope is under 50° on average; the client's contact with single
  facets is not modelled.
- **"Free" incidents** that are not a bot standing to aim: other bodies
  (bots following their partner closely), props that move; the new watch
  only stops calling a still bot stuck.
- **Diagonal corners** are eroded no more than before (a body slides round
  them).
- **No decision traces** on the host until 2026-10-23: `SRP_BOT_TRACE_DIR`
  gives each stuck line the route and the keys. The owner turned it on
  (`/srv/srp/data/bot-traces`, cut to `SRP_BOT_TRACE_MAX_MB`, 2000) with
  the release of these fixes.
- **A bot frozen in geometry** still has no way out but its recoveries
  and its death; the server cannot move a body.

## 6. How players move (the recordings), for the bots

From 20 recordings of player matches (2026-10-14/15, before bots; 1,897
player minutes over levels 1-8, modes 2-4), the keys each player held,
tick by tick. LIKELY (the relayed inputs; a tick's keys held until the
next input).

| | players |
|---|---|
| moving | 83 % of the time (78-88 % by map) |
| sprinting | 37 % (32-45 %) |
| crouched | 17 % (9-30 %), moving crouched 10 % |
| strafing (left/right held) | 41 % (32-51 %) |
| backing | 3 % |
| aiming down sights | 16 % (7-22 %) |
| jumps | 0.98 a minute (0.53-1.43), 94 % of them while moving |
| standing still ≥ 1 s | p50 2.0 s, p90 6.2 s a time |

Ranked by what they would do for the bots (each a suggestion, not built):

1. **Strafe while walking** (LIKELY high impact). Players hold a side key
   41 % of the time; a bot strafes only standing to aim
   (`mod.rs`, `over.strafe` with `aiming_still`), and walks with forward
   only (`motor.rs`, `keys_toward`). A side key with forward also slides a
   body along a face it would catch on. Suggest: `motor.rs` walks
   diagonally (forward plus a side) round a corner within a step of the
   route, and the reflex's strafe while it walks in a fight.
2. **Jump about once a minute, while moving** (LIKELY). Bots jump only at a
   route's jump link and to get unstuck (`motor.rs`, `jump_key`;
   `mod.rs`, unstick). Players jump 0.5-1.4 times a minute, almost always
   on the move: over low obstacles and to unstick themselves. Suggest:
   `motor.rs` presses jump when the reckoning (`BodyTrack::last_error`)
   shows the body short of the reckoned place by more than 1 m while the
   route ahead rises under 0.6 m (a lip, a rail).
   **Built** (`motor.rs`, `Motor::hop_key`, the numbers `Feel::hop_*`):
   while walking a route (`walk` and `walk_aiming`), a hop when a world
   has just put the body caught on rising ground. The signal is the
   reckoned feet going back along the way from one tick to the next: the
   keys never move them back, only a world does, by as much as the body
   was short of the reckoning (`last_error`, along the way), read from
   the poses the motor is given. It hops when that is at least 1 m and a
   fifth of the way reckoned since the world before, and the ground about
   the body climbs 0.25-0.9 m within 1.5 m: the floors the reckoning
   walked (it walks the grid; the route's corners are far apart, a lip
   between two is not in its heights), or the route's own heights. Not
   turning more than 0.35 rad, not within 1.5 m of a low leg or a jump of
   the route, not within 2 s of the route's jump, at most once in 4 s.
   A hop has to come at the first world that shows the catch: at the next
   the stuck watch takes over (1.5 s without moving). The player numbers
   (a jump a minute, on the move) are why it exists; the thresholds are a
   choice, not measured. Tests: `a_body_caught_at_a_lip_hops_once`,
   `the_hop_cools_down`, `flat_walking_never_hops`,
   `no_hop_under_a_low_floor_or_by_a_planned_jump`. Not seen in a real
   game.
   **No jump while the weapon fires** (PROVEN in the code, 2026-10-23):
   the jump goes through `weapon_user_animations_selector::jump_predicate`
   (0x2FBDC0), which asks the weapon in hand's `can_jump` (0x2E3A70, its
   vtable+64h) before `is_trying_to_jump` (0x2FC230, the jump bit of the
   actions). `can_jump` answers yes only for the weapon state ids 3 (idle),
   4 (reload) and 6 (chamber a round); the ids are each state's
   constructor's push to `weapon_core_base_state`'s (0x31EAB0, id at
   +124h): 1 show, 2 hide, 3 idle, 4 reload, 5 fire, 6 chamber a round.
   Aiming is no state (`m_aimed`, +5E8h) and is not asked: it does not
   stop a jump. A bot fired on through its jumps (the route's, a hop, the
   unstick's), so a jump pressed with the trigger down was dropped. Now
   (`mod.rs`, `jumping`): a jump with the trigger down now or within
   300 ms is held 300 ms with the trigger let go (`JUMP_HOLD_MS`; the
   round counted that tick given back, `Reflex::hold_fire`); otherwise it
   is the one tick it was. Whether 300 ms covers every weapon's fire state
   is not measured.
   **Traced:** with `SRP_BOT_TRACE_DIR` set, each jump is a line
   (`"jump"`: `route`, `hop` or `unstick`; `pos`, `firing`, `aiming`), and
   what followed it another (`"jumped"`, `after_ms`, `then`: `went on`
   after 3 s, `stuck: <recovery>`, `jumped again` or `a new life`;
   `moved_m`, `rose_m` from where it jumped). A hop that frees a body is a
   `went on` with some metres moved; one that does not, a `stuck` soon after.
   `scripts/bot-traces/jumps.py <trace dir> [--logs <host logs>]` reports
   them: by kind, how often and what followed; the jumps made while firing
   (refused by the game before the hold) against the others; and the
   spots where hops repeat. The player's game simulates the jump, so the
   server's simulated matches cannot show what a jump gains: only traces of
   real matches can.
3. **Short stops** (GUESS on impact). Players stand 2 s at the median, 6 s
   at the 90th percentile; bots look about 1-2 s at an arrival and stand to
   aim. Suggest: `tactics.rs`'s hold plays end in 2-6 s by that spread.
4. **Crouch by map** (GUESS). Players crouch 9 % (level_02 TDM) to 30 %
   (level_05 rain TDM) of the time. Bots crouch for low legs, sneaking and
   some fights. Suggest: a crouch share by map in `tuning.rs`.

Not read from the recordings yet: where players walk (their positions are
in the uploaded worlds, one every ≤ 10 s): the routes they favour by map
and mode, and how they pass the spots of §3 (whether they jump the
Cologne desk or go round it). The converter (`sur_convert`) already
gathers those worlds.

## 8. Under its own route, and goals on no floor (2026-10-23 audit, issues 1 and 5)

From the decision audit of the owner's three PvB matches of 2026-10-23
(issue 1: a carrier 273 s against a wall 8 m from its container in
0x4da3b51d; issue 5: bots standing with a goal, 0x5d506a22 bot 14).

### 8.1 A carrier under its own route (PROVEN)

The carrier fell 2.9 m off the walkway its route went along, onto the
floor below it, 0.7 m off the route on the ground plane. Three things
held it there:

1. **The route still said "on the way".** `Route::progress` measured the
   distance to the route on the ground plane only, under `OFF_ROUTE_M`
   (3 m), so the motor never answered `Walked::Off` and steered at the
   walkway above: into the wall under it.
   Now (`path.rs`): a route keeps the height of the floors it passes over
   along each leg (`floors`, a sample where the height changes by
   `FLOOR_SAMPLE_M` 0.1 m), and a leg whose floor at that point is more
   than `OFF_ROUTE_HEIGHT_M` (1.5 m) from the feet is not one the body is
   on. A body under its route is off it and plans again from its floor.
   A height per leg (its two ends) was not enough: a pulled leg over a
   ramp and a walkway has floors well off the line between its ends.
2. **The stuck watch saw nothing.** It calls a body stuck when its keys,
   reckoned on the walk grid, would have moved it 1.2 m. The grid had a
   wall there too, so the reckoned body stood as still as the real one.
   Now (`stuck.rs`): walk keys held in every world for `HELD_AFTER_MS`
   (3 s) with the body within `HELD_MOVED_M` (0.5 m) is stuck too,
   whatever the reckoning says. The note ends "(walk keys held, the body
   still)".
3. **"Took too long" changed nothing.** The audit said a retaken play
   kept its route: it does not (`end_play` clears it). But the play was
   taken again at once (it had been up only past `QUICK_FAIL_MS`). Setting
   such a play aside like a quick failure was tried and left out: with 1
   and 2 in, the carrier gets home without it (the test below passes
   with it off, and fails only with both 1 and 2 off), plays end "took
   too long" 3 times in 12 bench matches, and on the bench it moved
   nothing beyond its error (8.6).

Test: `a_carrier_fallen_under_its_walkway_route_gets_home` (`tests.rs`,
`walkway_world`: a walkway 3 m up with its container, a ramp down, a wall
under it). A carrier with a route along the walkway, standing on the
floor under it, delivers within 45 s (about 30 s). Before: it stood at
x 14.5 against the wall to the end.

### 8.2 Standing with a goal: goals on no floor (PROVEN for bot 14)

The audit guessed that searches dropped while a partner moved kept the
route from ever being found (LEAD). They were not the cause. In the bench
the "searching" ticks are about none and no search is dropped. The bot
stood because its goal was on no floor:

- Buddy's goal is 9 m (`close_m`) short of the partner, toward the bot,
  **at the partner's height**. With the partner up a ramp, or on a
  walkway, that point is in the air or inside the ramp.
- `WalkGrid::locate` looks within 3 m across and from 1 m above to 2.5 m
  below a point: nothing. The search failed at once, the route was set to
  none, and the bot stood (with no keys, so not stuck either).
- 0x5d506a22, bot 14 (Buddy of 12), 116-131 s: its goal had no floor at
  every second; it was 1.8-6 m under the floor its partner walked. The
  nearest floor within 3 m across at any height had a way to it every
  second.

Now (`path.rs` `goal_floor`, used by the drive): a goal on no floor plans
to the nearest floor within `GOAL_REACH_M` (3 m) across and
`GOAL_RISE_M` (6 m) above or below. A bot already on its goal's floor
plans nothing and has no "no way" counted against the play. Buddy itself
(`plays/buddy.rs`) is unchanged.

Tests: `a_goal_in_the_air_is_walked_to_the_floor_under_it` (a goal 4 m
over the field: before, it never had a route) and
`path::tests::a_goal_off_its_floor_plans_to_the_nearest_floor`.

### 8.3 The old route while the next is searched

The drive already walked the old route, or the part of the new one found
so far, while a search went on over several polls. The audit's "drop the
search and stand" was not what happened. What was missing was a trace:
now a bot whose goal moved on before its search finished notes "dropped
its route search to ... (n dropped since the last note)", at most once in
`DROPPED_NOTE_MS` (5 s).

Test: `a_bot_keeps_walking_its_route_while_its_next_search_goes_on`
(a search of 150 floors a poll, a goal that moves; the bot walks on and
is never without a route, and the dropped notes come 5 s or more apart).

### 8.4 Off a ledge, in the reckoning (PROVEN)

`body.rs` `slide` moves a reckoned body (the server's reckoning of a bot,
for the stuck watch, and the tests' stand-in game) a quarter cell at a
time, each step onto a floor walked to or dropped to from the one under
it. Off a sheer ledge there is none: erosion takes the floor at the foot
of the face (it is within a body's radius of the face), so the grid's
drop from the ledge's last floor lands two cells on, and the step in
between is over no floor. The body stood at the edge, or slid along it.

- In the stand-in, bots spawned on some level_06 ledges never left them:
  the play choice's standing spots (-23.3, 38.7, 143.4) and
  (97.2, 27.5, 136.1) were this and a goal on no floor (8.2). It was not,
  as first thought, the stand-in not jumping: no jump was near.
- In the server, a bot walking off a ledge had a reckoned body that stood
  still while its keys were held.

Now: a step that no floor holds goes down the drop link ahead of it whose
line passes within half a cell of where the step ends (`dropped_to`),
and a step at a slant tries walk-or-drop whole before sliding along an
axis, so a body walking into an edge at a slant goes over it, not along
it. Test: `body::tests::a_body_walks_off_a_ledge_to_the_floor_below`
(before: it stopped at the edge, 2 m up, and slid along it).

A face exactly on a cell boundary gets no drop link at all (the face at
x 10.0 in that test): grid follow-up, below.

### 8.5 Routes along a lip

Erosion keeps a body's radius back from walls and from a jump's face, but
not from a drop (`LinkKind::Drop => false` in `erode`), so the floors of
a walkway reach to its edge, and A* and the pulled straight line take the
last cell before it whenever it is no longer. Cologne (level_08 day 4),
the audit's walkway: its floors run z 25.1-27.1 at x -45.2, the route ran
at z 26.9, on the lip; a way 1 m in costs the same. The client's capsule
(radius 0.38) stays up until its centre is 0.24-0.29 m past the edge, so
a bot on the lip falls if it strays 0.3 m, which a bot turning a corner
or bumped by a teammate does.

In the owner's matches, the bots' drops of 1.2 m or more between two
worlds (positions.csv), by whether a walk-only way from the top to where
they landed exists within 3 times the distance plus 6 m (else they went
off a ledge), and how many of those off a ledge were back at the height
they fell from within 30 s (they fell off the way they meant to go):

| Match | a walk-only way (stairs, a ramp) | off a ledge | of them back up within 30 s |
|---|---|---|---|
| 0x130 (level_08 day 4) | 34 | 8 | 1 |
| 0x4da (level_08 day 2) | 73 | 22 | 6 |
| 0x5d5 (level_06 day 4) | 61 | 54 | 27 |

Now (`mapdata` `grid.rs`): a lip is a floor with a drop of `LIP_DROP_M`
(1 m) or more out of it (`WalkGrid::is_lip`), and walking onto one costs
`LIP_PENALTY` (0.25 m) more in `link_cost`: the one cost A*, the
distance maps and `next_step` all use, so they agree. A way along a
ledge costs double there and takes the floors a cell back where there are
any. And `path.rs` `pull_straight` pulls no line across a lip the search
did not take (cutting a corner over an edge). The grid itself is
unchanged (no `GRID_ALGO` bump): the cost is the grid's reading of it.

Test: `grid::tests::a_way_along_a_ledge_keeps_back_from_its_lip` (a
block 2 m high: the way along its edge had 33 of 33 floors on the lip,
now 4 or fewer; a 0.8 m drop makes no lip). The lip's cell overlaps the
block's top, so its centre (x 5.125) is even past the edge (x 5.1).

On a first bench (128 pairs, 8 seeds of 8 maps, with 8.1-8.4 in both;
the final one is 8.6), falls of a metre or more in a tick, off the route
(no drop of it there) and taken by the route:

| | without | with the lip cost | diff |
|---|---|---|---|
| falls off the route | 9.21 | 6.76 | -2.45 +- 0.40 |
| falls the route took | 24.9 | 25.2 | +0.24 +- 0.57 |
| batteries delivered | 5.07 | 5.24 | +0.17 +- 0.11 |
| carry time, s | 25.3 | 24.7 | -0.51 +- 0.49 |
| pushing, s | 18.8 | 17.1 | -1.7 +- 1.8 |
| stuck notes | 27.8 | 26.1 | -1.7 +- 0.9 |
| grid searches | 1392 | 1364 | -29 +- 17 |

The rest (idle, reversals, standing with a goal, failed searches) moves
within its error. The stand-in walks its keys on the grid, so a fall in
it is a route or a slide over an edge, not a stray: the client's strays
(a bump, a turn) should gain more than this shows.

### 8.6 Measured

The bench (`real_bot_cost`, the stand-in game of `tests.rs`): levels 1-8,
days 2 and 4, 4v4 and 10v10, 180 s, 10 batteries, seeds 1-16: 512 pairs,
each one (map, size, seed) with and without. Each part was measured off
with all the others in (a toggle, removed before the commit); the mean of
the paired differences `with - without` with its standard error, `*`
beyond 2 SE. A part is kept only if it moves its target beyond 2 SE and
no guard (kills, deaths, K/D, deliveries, carriers killed, plays ended
within 1 s, A-B-A flips, idle, standing with a goal) gets worse beyond
it, or the cost is small against the gain and explained.

**Everything** (all of 8.1-8.6 off, then on):

| | without | with | diff |
|---|---|---|---|
| pushing, s a match | 539 | 1.5 | -538 +- 25* |
| idle 5 s or more, % alive | 36.6 | 11.4 | -25.2 +- 1.0* |
| batteries delivered | 1.74 | 3.10 | +1.36 +- 0.11* |
| carry time, s | 26.9 | 24.0 | -2.9 +- 0.9* |
| kills | 17.2 | 24.6 | +7.4 +- 0.4* |
| deaths a bot-minute | 0.425 | 0.614 | +0.189 +- 0.009* |
| K/D team 0 / team 1 | 1.07 / 1.03 | 1.22 / 1.06 | +0.15* / +0.03 |
| carriers killed | 0.52 | 0.72 | +0.20 +- 0.06* |
| plays taken a bot-minute | 5.68 | 7.50 | |
| ended within 1 s, % of plays | 12.2 | 13.8 | +1.7 +- 0.3* |
| A-B-A flips, % of plays | 3.3 | 4.0 | +0.7 +- 0.1* |
| stood with a goal, % | 6.9 | 7.9 | +0.9 +- 0.2* |
| reversals | 25.3 | 37.3 | +11.9 +- 0.7* |
| falls off the route | 3.3 | 7.9 | +4.6 +- 0.2* |

Bots that stood against walls now walk, fight and carry: more kills and
deaths on both sides, more deliveries, more carriers killed (more of them
carry). More plays end within 1 s because more are taken: read in the
notes, they are the play choice switching (Crossfire to Roam or Buddy,
Collect to Collect when a nearer battery shows), 1 of 786 after a stuck
(follow-up below). More falls: a reckoned body now walks off a ledge
(8.4) instead of standing at it, and the lip cost and fall spots take a
part of them back.

**Each part** (with it minus without it, the rest in; its target in bold):

| | held keys (8.1.2) | route floors (8.1.1) | goal floor (8.2) | ledge (8.4) | lip (8.5) | fall spots (below) |
|---|---|---|---|---|---|---|
| pushing, s | **-42.6 +- 2.6*** | **-0.52 +- 0.20*** | -0.35 +- 0.16* | **-41.4 +- 2.4*** | -0.20 +- 0.20 | -0.13 +- 0.15 |
| stuck notes | +4.2* | **-1.2 +- 0.2*** | 0.0 | -37* | -0.3 | -0.2 |
| carry time, s | -0.74 +- 0.27* | +0.15 | 0.0 | -4.6 +- 0.6* | -0.25 | 0.0 |
| delivered | +0.06 | +0.01 | 0.00 | +0.40 +- 0.06* | +0.02 | 0.00 |
| falls off the route | +0.34 | -0.11 | -0.05 | +2.0* | **-2.30 +- 0.22*** | **-0.70 +- 0.16*** |
| idle 5 s+, % | -2.4 +- 0.3* | +0.13 | -0.16 | -3.4* | +0.17 | +0.27 |
| stood with a goal, % | +0.48 +- 0.12* | -0.11 | **-0.58 +- 0.10*** | -0.54* | -0.07 | -0.05 |
| kills | +0.24 | -0.26 | +0.04 | +3.0* | -0.22 | -0.28 +- 0.13* |
| deaths a bot-minute | +0.011 +- 0.005* | -0.007 | +0.004 | +0.085* | -0.005 | -0.005 |
| K/D team 1 | -0.02 | -0.08 +- 0.03* | -0.06 +- 0.02* | -0.07 +- 0.04* | -0.03 | +0.01 |
| carriers killed | +0.01 | -0.01 | +0.03 | -0.05 | -0.02 | +0.04 |
| ended within 1 s, % | +0.03 +- 0.14 | -0.14 | -0.05 | +0.97 +- 0.21* | -0.12 | -0.09 |
| A-B-A flips, % | -0.06 | +0.05 | +0.02 | -0.05 | +0.02 | -0.04 |
| reversals | +1.0 +- 0.3* | 0.0 | -0.3 | -2.4* | +0.1 | -0.3 |

K/D team 0 moved within its error for every part. The guards beyond
2 SE:

- K/D team 1 with route floors and the goal floor (-0.08, -0.06, at
  2.5-3 SE). Team 0 and kills stayed flat, so this is a shift between
  sides, not bots fighting worse.
- Held keys: +0.5 % standing with a goal and +1 reversal (the recovery's
  back-off and re-plan), against 43 s a match less pushing and 2.4 % less
  idle.
- The ledge: +1 % plays ended within 1 s, because more plays are taken.
  The ledge is most of the "everything" activity.

**The stand-in had no jumps.** A first run of this bench had held keys
cost +0.5 % plays ended within 1 s and +0.2 % flips. Its stuck notes
were the cause:

- 100 of 118 were within 1 m of a jump's floor.
- The two level_06 traps that held bodies longest ((110.3, 30.9, 130.2)
  and (-12.5, 37.0, 150.0)) can be left only by a jump link.

The stand-in never jumped (`floor_under` leaves out jump links). So the
bots' JUMP did nothing, the keys stayed held, and the watch was right to
call them stuck; in the server the body jumps.

Now the stand-in jumps. A JUMP press keeps a body in the air
`STAND_IN_AIR_MS` (600 ms). That is a GUESS from the motor's press 0.8 m
before the face and the game's jump of about 0.5 s. A step the body
cannot take in the air takes the jump link ahead of it (`jumped_to`).
Test: `a_stand_in_body_in_the_air_gets_up_a_ledge`. With it, held keys
no longer cost anything on plays ended within 1 s or on flips (above).

**Fall spots.** With route floors in, a bot fallen off its route plans
again from where it landed. On level_06 (16, 30, 102) the new route took
it back onto the same narrow beam, so route floors first had +0.6 falls
off the route.

Now a body that fell off its route marks the spot it fell from
(`stuck.rs` `Stuck::fell`, `path.rs` `Route::fell_from`):

- A fall off the route is `Walked::Off` with no floor of the route under
  or over the body within `OFF_ROUTE_HEIGHT_M`.
- The spot is the route's floor point above the body, within
  `FELL_FROM_M` (3 m) across.
- It is avoided from the first fall (an ordinary stuck spot only from its
  second), and kept `SPOT_KEPT_MS`.
- A spot is on one floor. It is near a point within `SPOT_M` across and
  `SPOT_HEIGHT_M` up or down (2 m, a GUESS: the falls off a route came
  down 2.5 m at the median), so the floor under it is not avoided.

A first version, which marked a spot only on the second fall, moved
nothing beyond its error; this one is the column above. Tests:

- `stuck::tests::a_spot_fallen_from_is_avoided_on_its_floor`;
- the `fell_from` asserts in
  `path::tests::a_body_falling_down_its_routes_drop_is_on_it`.

**Left out: setting a play that "took too long" aside** (8.1.3). Over
401 pairs (a run with it as a toggle) it moved nothing:

- pushing -0.01 +- 0.06;
- carry time -0.03 +- 0.06;
- delivered +0.00 +- 0.01;
- stood with a goal 0.00 +- 0.03;
- flips -0.05 +- 0.03.

With the route floors and held keys in, no play is retaken under a route
it cannot walk.

**Also left out: a held-keys rework**, in which only walk keys (not the
JUMP or crouch bits) count as held. Against the kept version it cost
+24 s of pushing.

Whole-match tests (`--ignored`, release, on the branch merged with
`feat/bots-tiers-combined` 8a50657b): all 8 pass, among them
`four_bots_a_side_fight_and_bring_batteries_to_a_win` and
`timed_batteries_are_waited_for_at_the_stations_and_brought_home`.

**On the current base.** The same bench, `feat/bots-tiers-combined`
8a50657b against it merged with this work, 512 pairs. The base's bench has
no `RouteTally` (pushing, idle and falls are this work's), and its stand-in
does not jump, so these are the guards and the deliveries:

| | base | with this | diff |
|---|---|---|---|
| batteries delivered | 1.99 | 3.50 | +1.52 +- 0.10* |
| kills | 17.0 | 24.8 | +7.8 +- 0.4* |
| K/D team 0 / team 1 | 1.03 / 1.02 | 1.23 / 1.11 | +0.20* / +0.09 |
| carriers killed | 0.56 | 0.84 | +0.29 +- 0.06* |
| ended within 1 s, % of plays | 12.0 | 13.8 | +1.8 +- 0.3* |
| A-B-A flips, % of plays | 3.4 | 4.0 | +0.6 +- 0.1* |
| stood with a goal, % | 6.2 | 7.7 | +1.5 +- 0.2* |

The "stood with a goal and no route" ticks are up in every play (most in
Collect, +0.5 points of goal ticks). These are short gaps between
re-plans: the time idle 5 s or more fell from 36.6 % of the time alive to
11.4 % (the "everything" table).

The bench, to run again. The binary is `cargo test -p match-server
--release --lib --no-run` in `srp-026g` under `nix develop`; give each
checkout its own `CARGO_TARGET_DIR`. In `srp-026g/servers/match-server`,
for seeds 1-16:

    export SRP_MAPDATA_DIR=<mapdata dump>
    export SRP_BENCH_MAPS=1/2,2/2,3/2,4/2,5/2,6/2,7/2,8/2,1/4,2/4,3/4,4/4,5/4,6/4,7/4,8/4
    export SRP_BENCH_SIDES=4,10 SRP_BENCH_SECONDS=180 SRP_BENCH_BATTERIES=10 SRP_BENCH_USE_LINE=1
    SRP_BENCH_SEED=<seed> <binary> --ignored --nocapture --exact v026g::bots::tests::real_bot_cost > s<seed>.log

There is no `SRP_BOT_ROUTES`. Each (map, size, seed) is a pair; the tables
are the mean of the paired differences with its standard error.

### 8.7 Left

- **Plays ended within 1 s by the play choice**: Crossfire to Roam or
  Buddy, Collect to Collect, about 4 % of plays taken. They are the
  choice switching (a play ran out, or a higher band came), not the
  routes (1 of 786 after a stuck): for the play choice and Crossfire.
- **No drop link where a face lies on a cell boundary** (8.4): the grid
  has no way down there at all (`mapdata`).
- **Reversals went up** with the fixes (re-plans, a recovery's back-off):
  fewer seconds lost, but more turns a human would not make.
- **K/D team 1** fell 0.06-0.08 (2.5-3 SE) with route floors and the goal
  floor while team 0 and kills stayed flat; worth a look per map.
- **A partial route that runs out** before its search ends: the bot
  stands until the search ends (a few polls).
- **A goal under a ramp** plans to the nearest floor, which may be the
  ramp's top or its foot: whichever is nearer, not whichever the play
  meant.
- **Route jumps fail 4 times in 5 in the client** (8.8): 42 of 199 first
  jumps got up. The motor jumps 0.8 m before the take-off on the
  approach's heading and retries wherever the body slid to. It needs a
  lined-up jump from the take-off, and no retry away from it. Cologne's
  battery 6 cost seven deaths this way. Done in 8.9.
- **The stand-in's jump** is a guess (600 ms in the air, up a grid jump
  link only): a jump the grid has no link for is still a wall in it.

### 8.8 Two spots of the #343 check (2026-10-23 evening)

The owner's four evening matches on `feat/bots-tiers-combined` without
this work (`pvb-343-check/analysis/REPORT.md` §3, items 1 and 7).

**0x6da44bbb bot 4, 285-375 s (level_01 day 2): caught.** It fell into
a trench 1 m deep and 0.75 m wide at (-49.1, 21.5, -82.0). The walk grid
has jump links up both of its sides; the bot jumped up one every second,
88 times. The jumps never got it up: every world showed forward held and
the feet within 0.08 m. The only answer was "Roam: made no headway" every
10 s.

The held-keys watch (8.1.2) calls this stuck 3 s in (a test from its
worlds in `walk_keys_held_and_no_move_is_stuck_without_the_reckoning`),
and the recovery steps back and to a side, then avoids the spot. Whether
that gets it out of this trench is not shown: in the stand-in every jump
up a grid link succeeds.

**0x1b8ee975, battery 6 at (5.1, 35.5, 60.1) (Cologne, level_08 day 2):
not this work.** Seven deaths in 106-345 s.

- The route to it jumps from a platform's north edge (y 34.4, z 55.9)
  up 0.9 m to a strip 0.75 m wide at z 56.4, over a gap with a long fall
  under it.
- Every death had the same trace:
  - a route jump 0.8 m before the take-off (x 7.6-7.7);
  - a second jump 1 s later, 1-1.5 m further west (x 6.1-6.6), still at
    the platform's height: the first hit the step's face, and the body
    slid west along it to the platform's end;
  - the second jump went on west, into the gap.
- The lip cost does not mark this edge: nothing under the gap is a
  floor within a drop, so there is no drop link and no lip. The fall
  spots do not help either: each bot died of the fall.

The cause is the motor's jump. In the four evening matches 42 of 199
first route jumps (21 %) got the body up (`went on` in the jump trace).
The others were jumped again 1 s later, short of the ledge. The motor
presses JUMP 0.8 m before the take-off, steering past it. Where the jump
leg turns from the approach, the body flies on the approach's way. Its
retry comes wherever the body slid to. A player lines up first and
jumps from the edge. This needs the real client to measure (in the
stand-in every jump succeeds), so it is a follow-up (8.7).

### 8.9 Route jumps: lined up at the take-off (2026-10-24)

**What the client does with a jump (LIKELY, from the code).**
`base_player::jump` (0x2D7920) sets the controller's jump flag (+0x2C70C).
The jump branch of `bullet_character_controller::player_step` (0x7BF80 to
0x7C13B) moves the body by (its velocity × dt + the walk vector of the
keys) × a scale (+0x2C704). It splits that into a vertical step
(`step_jump_up` 0x7CFC0, `step_jump_down` 0x7D0F0) and a horizontal one
(`step_jump_forward` 0x7CB90: a sweep that slides along what it hits). So
a jump goes the way the body faces and its keys push, and against a face
it slides along it.

**What the jumps came to (the owner's 7 matches of 2026-10-23,
`pvb-343-check` and `pvb-jumps-local`, `scripts/bot-traces/jumps.py`).**
A first try is a route jump with no route jump by the same bot in the
2.5 s before. It got up when the bot went on, or rose at least 0.4 m.

- 108 of 234 first tries got up (46 %). Aiming: 11 of 40 (27 %); not
  aiming: 97 of 194 (50 %).
- By the jump's rise: 0.5-0.8 m about half; 0.9 m 36 %; 1.0 m 24 %. Three
  first tries in four are on links of 0.9-1.05 m (the grid's `max_jump`
  is 1.05). The worlds in the middle of a jump show the feet's top at
  about 1.0 m.
- Some take-offs never work: 1b8 (14.5, 53.9) 0 of 20, (55.2, 43.6) 1 of
  19; others always do: (56.7, 42.9) 8 of 8, (4.1, 106.4) 9 of 9.
- Retries got 51 of the 124 failed firsts up: 29 at the first retry, 15
  at the second.
- A press makes a jump (the worlds 200-750 ms after it show the body in
  the air): route jumps standing 149 of 159, crouched 8 of 12; unstick
  jumps standing 68 of 76, crouched 6 of 23; hops crouched 3 of 11.
- Cologne's battery 6 (8.8): 550 ms after the press the body was 0.98 m
  up, but had gone 1.31 m west along the approach and 0.09 m north, the
  jump's way. It jumped high enough, the wrong way.
- 81 "jumped again" follow-ups had already risen: the server's reckoning
  did not jump, so it still had the body under the ledge.

**The change.**

- `motor.rs` `jump_ahead`: from 2.5 m before a jump's take-off
  (`LINE_UP_FROM_M`) the motor lines up. Off the jump's line it walks to
  the run-up, 0.8 m behind the take-off on the jump's line (0.4 m, or the
  take-off itself, where that is not clear). On the line (0.3 m to the
  side) it walks along the jump's way only while facing it within
  0.45 rad (`LINE_UP_GO`): where the jump turns from the approach it
  stands and turns first. No sway, sprint or crouch near the jump.
- `jump_key`: it presses on the line, facing the way within 0.2 rad
  (`JUMP_FACING`), moving along it (`Ahead::going`: not still walking
  back from the run-up), within 0.2 s × speed + 0.15 m of the take-off.
- Aiming (`walk_aiming`), on the line the view turns to the jump's way
  and the legs push forward only: a strafed jump goes sideways.
- A try that did not get up is tried again only from the run-up, never
  where the body slid to. After 3 tries (`JUMP_TRIES`) the jump is given
  up on: `Walked::JumpFailed`, and the take-off becomes a spot to keep
  clear of, as a fall's (`Stuck::failed_jump`). Tries are kept for 10 s
  over a route planned again to the same jump.
- `is_up`: up a jump is within 1.5 m of the landing, past the leg's
  middle, and under the landing's floor by at most 0.35 m (half the rise
  for a low jump). `arrived`: a route's end up a ledge is not arrived at
  from under it.
- `body.rs`: the reckoning jumps. A press keeps it in the air for 800 ms
  (`JUMP_AIR_MS`), and a step a ledge stops that walks into a grid jump
  link gets up it (`jumped_to`, the stand-in's). A world that shows the
  body still below puts it right; `Motor::under_jump` then takes the
  route back to the jump's leg instead of walking on from under it.
- `mod.rs`: no crouch or sprint while lining up. The trace's route jump
  lines say which try (`try`), the take-off and landing (`from`, `to`),
  how far short it was pressed (`before_m`), how far off the way it faced
  (`facing`) and whether it was crouched; the follow-up says when a world
  first showed it up (`up_ms`).

**Measured.** The stand-in game of the bench jumps any grid link it walks
into within 600 ms of a press, whichever way it faces: it cannot show the
jumps' success. The bench guards the rest (below). The client's first-try
share needs PvB matches played on this build (`jumps.py`, the "Route jumps
at the first try" section; before: 108 of 234).

The bench: 16 PvB maps, 4v4 and 10v10, 180 s, batteries 10, the use
line, `SRP_BOT_ROUTES` on both, seeds 1-12: 384 (map, size, seed) pairs,
this change against its base 0b2ae190. The mean of the paired
differences ± its standard error; * beyond 2 SE.

| | base | lined up | diff |
|---|---|---|---|
| kills | 18.94 | 19.17 | +0.23 ± 0.18 |
| deaths a bot-minute | 0.452 | 0.460 | +0.009 ± 0.005 |
| K/D team 0 / 1 | 1.28 / 1.14 | 1.30 / 1.14 | +0.02 ± 0.05 / +0.00 ± 0.05 |
| delivered | 3.51 | 3.58 | +0.07 ± 0.05 |
| carriers killed | 0.781 | 0.747 | -0.03 ± 0.06 |
| quick ends a bot-minute | 1.12 | 1.14 | +0.03 ± 0.02 |
| quick ends, % of plays taken | 14.95 | 15.28 | +0.33 ± 0.19 |
| flips a bot-minute | 0.250 | 0.248 | -0.002 ± 0.008 |
| idle 5 s+, % alive | 16.67 | 15.94 | -0.73 ± 0.33* |
| stood with a goal, % | 7.33 | 6.92 | -0.41 ± 0.18* |
| stuck notes | 9.07 | 7.90 | -1.17 ± 0.25* |
| falls off the route | 7.81 | 6.57 | -1.23 ± 0.22* |
| pushing, s a match | 1.89 | 1.03 | -0.86 ± 0.24* |

No guard is worse beyond 2 SE (deaths +1.8 SE, quick ends +1.7 SE).

**Left.**

- Links of 0.9-1.05 m are at the client's reach, and some take-offs never
  work even lined up. If the client still fails them, the grid's
  `max_jump` for route jumps should come down (a `mapdata` change: the
  grids are built with it), or such links should cost more.
- 0x6da44bbb bot 4's trench (8.8): the worlds show the body about 0.9 m
  above where the reckoning has it, a grid or reckoning mismatch there, not
  a jump.

### Team-wide stuck spots (2026-10-24)

**Why.** In the Cologne match 0x60aeff4f, bots of one team got stuck at a
spot, or fell off the route at an edge, where a teammate had already got
stuck or fallen. Each bot's stuck watch kept its spots to itself
(`Stuck`: avoided after 2 stucks, for 45 s), so each mate learned the same
spot again.

**The change** (`stuck.rs`, three `mod.rs` hunks).

- Each bot tells its team what it learned: every stuck note, and every fall
  off the route at the spot it fell from (`Stuck::tell`).
- At each commander tick (500 ms) the team merges those into its spots
  (`TeamSpots::learn`, merged by the stuck watch's `near`: 2 m round, 2 m
  high). A spot becomes avoided after a fall there, or after 2 stucks by
  any of the team's bots. Avoided spots are kept for the match, at most 64
  a team; past that, the least recently hit spot that is not avoided goes
  first.
- Every bot of the team routes clear of the avoided spots, as it does of
  its own (`Stuck::avoided_for`). A spot within 3 m of the bot's goal is
  exempt, so a goal next to an edge stays reachable.
- **Detour cap:** when the way round is long (over 1.5 × the straight
  distance plus 10 m), the route through is searched too. The way round is
  taken only if it is at most 1.5 × the way through plus 10 m
  (`stuck::worth_round`).
- All the numbers are GUESS consts in `stuck.rs`.

**Measured** (`real_bot_cost`, 16 maps × 4v4 and 10v10 × seeds 1-16 = 512
pairs, with `SRP_BOT_ROUTES`, against d8dcea55; ± is 1 SE, * beyond 2 SE).
The tally (`RouteTally` in `tests.rs`) counts a stuck note or fall within
`near` of a spot where a teammate had one before.

| | before | after | change |
|---|---|---|---|
| stuck at a mate's spot | 0.94 | 0.55 | -0.39 ± 0.07* |
| falls at a mate's fall spot | 1.91 | 1.13 | -0.79 ± 0.10* |
| falls off the route | 6.59 | 5.62 | -0.96 ± 0.15* |
| stuck notes | 7.99 | 7.62 | -0.36 ± 0.14* |
| pushing, s a match | 1.18 | 0.93 | -0.26 ± 0.12* |
| quick ends % takes | 9.33 | 9.09 | -0.23 ± 0.11* |

No guard is worse beyond 2 SE: K/D -0.01 ± 0.03 and +0.03 ± 0.02, kills
+0.01 ± 0.14, delivered -0.03 ± 0.03, carriers killed -0.05 ± 0.05, quick
ends a bot-minute -0.007 ± 0.011, flips +0.007 ± 0.005, idle -0.20 ± 0.17,
team kills 0.016 → 0.008. Route searches are up 2.8 % (the second search
when the way round is long).

**Left.**

- The first fall at a spot is not prevented. Falls with no route (fight side
  steps, pushes) are not learned. A long edge with several fall points is
  learned one point at a time. Grid's pocket entries can seed the team's
  spots at match start.
- A bot already in a pit with no route still stands. The stuck watch needs
  a route; getting out of pockets is a later change.

### Bots in damage zones (2026-10-24)

**Why.** In the Cologne match 0x60aeff4f (CG-18), bots walked into level_03's
electric pairs as they came on, stood in them with no goal, and backed out
of a zone the way they came when a chase re-planned their route mid-zone.
The routes checked a zone only at the moment of the search, so a zone that
came on before the bot got there was walked into. The way out of a zone took
the nearest exit with no thought of the goal, and found nothing when the
bot was deeper than its search.

**The change** (`hazards.rs`, `tactics.rs`, `stuck.rs`, `mod.rs`).

- **Arrival time.** A route takes a zoned floor only when its zones stay off
  for the pass (`PASS_MS`) from the bot's earliest arrival there: the
  straight distance from the start at sprint pace (`hazards::ARRIVE_MPS`,
  5 m/s). The region stage samples the zone's near and far distances, so it
  stays permissive.
- **Held routes.** Every 500 ms the route's next 15 m is checked the same way.
  When a zone ahead will be on when the bot gets there, the route is dropped
  and searched again (`Tactics::route_shut_ahead`). This is not done while
  the bot is on a zone's floors walking out.
- **Getting out toward the goal.** The way out of a zone weighs the goal:
  exits within 6 m of the nearest are scored on the walk to them (× 3) plus
  their distance to the goal. An exit on the far side counts only when the
  zone stays off for the walk at 3 m/s, or the zone is mild (0.05 of health a
  second or less; it was 5.0, read as health points, which made every zone of
  the maps but one mild: fixed 2026-10-25, below).
  Escapes from spawn-area marks stay nearest.
- **Deep in a zone.** When the search ends before an exit, the bot takes the
  match's exit field instead: a search from every unmarked floor back over
  the marked ones, built once a match on first need.
- **No goal.** A bot holding a spot, or standing at its goal, walks out of a
  zone that is on (or coming on), and holds the exit for 2 s.
- **Re-entries.** A second escape from the same zones toward the same goal,
  after having left them, within 30 s, marks that goal no way for a while
  (`Stuck::zone_escape`), as an unreachable goal is.
- **Unstick keys** do not step onto a zone's floors when another key set
  does not.
- All the numbers are GUESS consts.

**Tests** (`zone_runs` in `tests.rs`, the Cologne anomaly bench's runs):

| Run | Result | Stock |
|---|---|---|
| Only way, 6 s on / 6 s off | crosses in an off span, 0 s on | 1.2 s on |
| With a detour | 0 s on | 1.2 s on |
| A chase re-planned every 1.5 s | no back-outs, arrives at 17 s | 2 back-outs, 41.7 s |
| No goal, in the gap as it comes on | 0 s on | 5.9 s each span |
| Pushed into a small zone, no goal | 2.1 s (fixes once a second) | 27 s |
| Pushed 25 m into a big zone | out by the exit field, 8.5 s on, arrives | stood 37 s |

**Measured** (`real_bot_cost`, 16 maps × 4v4 and 10v10 × seeds 1-16 = 512
pairs, with `SRP_BOT_ROUTES`, against 639490d6 with the same zone tally; ±
is 1 SE, * beyond 2 SE). `RouteTally` counts living bot-seconds on a zone's
floors while it is on (sampled each world), entries onto zones' floors, and
re-entries into the zones last left within 30 s.

| | before | after | change |
|---|---|---|---|
| bot-s on a zone while on | 2.15 | 1.80 | -0.35 ± 0.13* |
| zone entries | 8.91 | 8.63 | -0.28 ± 0.16 |
| re-entries within 30 s | 1.78 | 1.76 | -0.03 ± 0.08 |

By map (all seeds, bot-s on a zone while on): level_03 day 2 36 → 4 and day
4 35 → 20, level_02 day 2 54 → 19, level_06 522 → 420; level_07 day 4 82 →
107, the one worse. Escapes: 975 back the nearest way, 201 on toward the
goal, none found nothing (the exit field).

Guards: K/D -0.02 ± 0.02 and +0.02 ± 0.02, kills +0.09 ± 0.10, delivered
-0.01 ± 0.02, carriers killed -0.06 ± 0.04, quick ends a bot-minute -0.004 ±
0.007, flips -0.001 ± 0.003, idle -0.08 ± 0.12, team kills 0.008 → 0.012
(+0.004 ± 0.004). Fall ticks are up +0.37 ± 0.16*. These are route drops, not
falls off: planned drops on routes that go round zones +0.28 ± 0.15, falls
off the route +0.09 ± 0.09 (no change). Stuck notes +0.08 ± 0.10.

CPU (the same 512 matches, 8 at a time on one machine, so noisy):

| | before | after |
|---|---|---|
| bots, ms a second (mean a match) | 18.61 | 19.45 (+4.5 %) |
| poll µs p99 / p99.9 (mean a match) | 2046 / 4263 | 2138 / 4367 |
| worst poll µs (p99 of matches' worst / worst) | 74159 / 251145 | 72186 / 134900 |
| worst think / ticks µs | 51754 / 251135 | 60401 / 134892 |
| grid searches, count / ms each | 681139 / 1.440 | 684190 / 1.478 |
| map_ready ms, level_03 day 2 / day 4 / level_06 day 2 / level_07 day 2 | 376 / 460 / 480 / 196 | 381 / 468 / 467 / 189 |

The exit field is built on first need during play, not at map_ready, so its
build is in the polls' worst times, which did not grow.

**Left.**

- Re-entries did not move. Most entries are of always-on, low-damage zones on
  levels 1 and 2, crossed on purpose when there is no other way. The
  re-entry mark acts only on a second escape toward the same goal.
- level_06 and level_07's big zones still hold bots for seconds: their
  bot-seconds are mostly escapes back the nearest way from a zone the route
  had to cross.
- The arrival check assumes a healthy bot at sprint pace. A bot slowed by a
  broken leg, no stamina or a hit's stagger gets there later than that; the
  4 s pass and the held-route check every 500 ms cover only part of it.

### Crossing weak zones on all match, and out of a ring of one (2026-10-25)

**Why.** On Mamayev Kurgan TDM (3D route table, 4 v 4, seed 2), a bot
holding the Cut spot slipped 1.9 m in one tick onto a patch of 92 floors
(about 6 m²) below it. The grid has no drop link there. A fire's soft damage
(`logic/anomaly_fire/fire_soft_damage`, 0.04 of health a second, on all
match) rings the patch. The planner shuts every zone's floors
(`Tactics::allowed_from`), so every search from there failed (NoWay at the
start), for 100 s. The zone escape runs only for a bot standing on a zone's
floor, and this one stood next to it. Players walk through a zone this weak.

**The change** (`crossing.rs`; `Tactics::start_crossing`, `ringed`,
`only_across_fixed`; `Hazards::only_fixed_at`; `collect::bears`; the motor's
route search in `mod.rs`).

- **When.** After the plain search: no way, where a walk over the regions
  does not get there with the zones on all match shut but does with the ones
  the bot may cross open (`only_across_fixed`). Or a way longer than 1.5 × the
  line + 10 m (`stuck::may_be_long`) while the line crosses such a zone; not
  again for 10 s to a goal within 10 m after one that kept the plain route
  (`crossing::LONG_RETRY_MS`, `Bot::long_kept`).
- **The search.** The same search again, over polls as ever, with the floors
  of zones on all match open: those whose walk across at their thinnest (the
  marks' margin each side) leaves the bot `CROSS_ABORT_LEFT` (`crossable`).
  Zones of 1.2 a second stay shut.
- **Taken** when `collect::bears` holds for the health the route costs at
  `CROSS_MPS` (a battery's crossing's rule, #428: at most half the health,
  leaving 0.4), and only begun at 0.7 health or more
  (`crossing::START_HEALTH`). Over a long plain route, only when that route
  is not worth it (`stuck::worth_round`).
- **A ring.** Else, from a patch of at most 100 m² that zones on all match
  wall (`Tactics::ringed`), the route is walked only to the first floor past
  the ring, toward the goal, if that walk leaves `CROSS_ABORT_LEFT`. It plans
  as ever from there.
- **Walked.** A route taken is a crossing (`Bot::crossing`): the motor does
  not escape off those zones' floors, nor drop the route for them ahead.
  Once the rest of it would leave under `CROSS_ABORT_LEFT` (hurt in a fight,
  held up in the zone), it is stopped and the escape walks the bot out
  (`crossing::still_borne`), as a carrier's crossing.
- **Goal moved mid-zone.** A crossing whose goal moves while the bot is on
  its zone's floors (a route leg's next point) is walked on till off them,
  then it plans to the new goal. Before, the escape took the nearest way
  out: on Mamayev Kurgan's batteries (10 v 10, tier 9-10, seed 5) one led
  down a 4 m drop into the 0.25 a second zone below, where the bot died.
- **A carrier's own way out yields.** Collect's line way out of zones
  (`Ctx::way_out`: to the nearest dry floor) is not asked while the motor
  walks a crossing or a ring's walk out (`BotFacts::crossing`). On Mamayev
  Kurgan's acid the two took turns a poll each, as the floor under the
  carrier switched `needs_crossing`: the route 8 m on toward home, the line
  to a floor 0.5 m up it never stepped onto. It stood 60 s there, 0.66 to
  0.38 health. One way at a time, the running one kept (arbitration, not
  blending; a behaviour tree's Running node): [steering](https://faculty.cc.gatech.edu/~surban6/2018sp-gameAI/lectures/2018_02_01_steering_continued.pdf),
  [behavior trees](https://en.wikipedia.org/wiki/Behavior_tree_(artificial_intelligence,_robotics_and_control)).
- **`MILD_HIT_PS`** (the escape's mild zone) is now 0.05 of health a second.
  Health and the zones' hits are fractions (`BodyView::health`, `[0, 1]`).
  At 5.0, every harmful zone of the maps but one (Mamayev Kurgan's 120 a
  second) was mild, the 0.8-1.2 a second ones too (Vostok Radar Station,
  Tarakanovsky Fort, Vector Laboratory, Cologne). Mild now: 0.015-0.04
  (Mamayev Kurgan, School).
- **Bench stand-in.** Its spawn put a body up to 2 m off a spawn point. On
  Rudnya 0.7 % of those offsets (one spawn 12 %) land on a floor below with
  no way to or from any spawn; a bot stood there all match. Now the offset is
  kept only on a floor that is reached and returns, else the spawn point's
  floor. Bad landings by map (a 21 × 21 lattice of the offsets): Rudnya 122
  of 17-19k, Mamayev Kurgan batteries 24, Cologne 23-25, Chemical Plant 6,
  Vector Laboratory 5, the rest 0.

**Best practice.** The usual way is area costs in one search: Detour's
[`dtQueryFilter`](https://recastnav.com/classdtQueryFilter.html) (exclude
flags shut an area, area costs only make it dear) and Unity's
[`NavMeshAgent.GetAreaCost`](https://docs.unity3d.com/ScriptReference/AI.NavMeshAgent.GetAreaCost.html)
(a dear area is still used when it is the only way). A second search is used
instead, for three reasons:

- The regions' graph is built once from the geometry, as
  [HPA*](https://papersdb.cs.ualberta.ca/~papersdb/view_publication.php?pub_id=585)'s
  abstract graph. Prices that change with the zones' state would need a cost
  hook at both levels and change every route.
- Health is a hard budget, which a cost alone does not keep. A constrained
  shortest path is NP-hard; the practice is to relax, then check the path
  ([Carlyle, Royset and Wood](https://faculty.nps.edu/mcarlyle/docs/CarlyleRoysetWoodCSP.pdf);
  [Lagrangian relaxation](https://linfo2266.readthedocs.io/en/latest/_static/slides/03-lagrangian-relaxation.pdf)).
  The second search is that at its cheapest: the zones' price infinite
  first (the search as before), then zero for the ones a bot may cross, the
  route checked against the budget.
- Every route a plain search finds stays the same.

The price: a weak zone that would save a modest detour is not crossed. A
bot moved off valid ground and replanned from where it is follows Detour's
crowd ([`checkPathValidity`](https://github.com/recastnavigation/recastnavigation/blob/main/DetourCrowd/Source/DetourCrowd.cpp)).

**Tests.** `crossing::tests`: the only way under a weak zone is crossed when
health bears it, not when too hurt or the zone too strong; a plain route is
kept with no second search where no zone is on all match; a ringed bot goes
out by its crossing, or, too hurt for that, out of the ring only, or stays.
`tactics::tests::a_mild_zone_takes_under_a_tenth_of_health_in_a_crossing_of_2_s`.

**Bench** (seeds 1-8, batteries and TDM, tiers 4-5, 9-10, 13-15, 3D table;
48 runs of 12 matches; base C = fbe66237 with the stand-in's spawn; the tip
ba77c815; with `SRP_BENCH_ZONE_DAMAGE=1`):

- Legs with no way: 766 → 569 a run (−198 ± 73). Kills, deliveries,
  carriers killed, team kills: unchanged within noise.
- Zone deaths a run 0.58 → 0.62 (+0.04 ± 0.07); by map, none beyond 2 SE
  (Mamayev Kurgan batteries 1 → 1, TDM 1 → 0 in 48 matches). Health lost to
  zones a run 0.54 → 1.28: crossings by design.
- Crossings a run: 126 searched, 40 taken, 6 rings walked out of, 74
  refused. Stretches in zones that left a body under 0.6 health: Mamayev
  Kurgan batteries 10 → 94 (78 begun under 0.7 health, 18 in a fight),
  TDM 25 → 43; the worst stretch took 0.72.
- The carrier's way out (bf6526b0, against ba77c815): Mamayev Kurgan
  batteries' stretches under 0.6 94 → 30 (ring walk-outs 37 → 0, Collect's
  straight walk 27 → 0), zone deaths 1 → 1; every other map's decisions the
  same; batteries 10 v 10 −0.47 ± 0.27 ms a second side by side.
- CPU against C, same load (0c5d9a2d): batteries 10 v 10 +0.48 ± 0.29 ms
  a second plain, +0.87 ± 0.34 with zone damage; TDM 10 v 10 +0.37 ± 0.31.
- The stand-in's spawn alone (C against fbe66237) moves Rudnya (no way
  8307 → 6244 batteries, 10536 → 4661 TDM), Mamayev Kurgan batteries
  (delivered 346 → 341) and Cologne TDM (no way 2210 → 614): those maps'
  earlier bench numbers were partly its artefact.

Open: an escape's way out runs over any floors, so it may drop into a
stronger zone below (`Tactics::escape`). Mamayev Kurgan batteries' 30
stretches under 0.6 left: walk-on 17, escape 8, crossings 5.

### Bots out of pockets (2026-10-24)

**Why.** In the Cologne match 0x60aeff4f (CG-3), bots that dropped into
level_08's pit A stood there for the rest of the match: the pit's rim is
0.90 m up and 1.0 m across, beyond the grid's jump links, so no route led
out, and the unstick keys tried the same walls. The grid knows such places
since #389: a pocket is a set of floors from which no route reaches the
spawns, and `pocket_exits` lists the ledges a jump could leave it by
(Standing up to 0.998 m, Sprint up to 1.35 m, Steep beyond).

**The change** (`escape.rs`, `path.rs`, `stuck.rs`, `tactics.rs`, `team.rs`,
`motor.rs`, `mod.rs`, `plays/mod.rs`).

- **Trying the ways out.** A bot in a pocket whose goal is outside it walks
  to an exit's take-off inside the pocket and jumps it: the route ends with
  a jump waypoint to the landing (`Route::then_jump`), so the motor lines it
  up with its run-up and presses as for a route's own jumps. Exits are tried
  in this order: those the team got out by before (`escape::Board`), then the
  Standing ones, shortest across first and then lowest (2 tries each), then
  the 2 lowest Sprint ones (1 try each). Steep ones are not tried. A try ends
  when the bot stands on a floor that reaches the spawns again (worked, and
  the team learns the exit), or fails after 12 s or when its route is dropped.
- **Giving up.** With no exit left to try, or 40 s after the first try, the
  bot gives up on the pocket and is marked trapped (`Ctx::trapped`). Its
  failed jumps while trying do not count as the motor's failed jumps. The
  escape is kept while the bot is in the pocket, its goal in it or not: a
  goal that moves in stops the try under way (not failed), and a bot that
  gave up stays so until it is out (a play's goal going in and out of the
  pocket made it give up again and again, handing its slot on each time).
- **No stuck watch while trying.** A try runs up to the rim and jumps; the
  stuck watch took that for stuck, noted it, marked a spot for the team and
  backed off or side-stepped against the try. While a try is under way the
  watch is off (`Stuck::reset`); the try has its own clock.
- **Slots.** A bot given up in a pocket lets its slot go, even inside
  `COMMIT_M`, and takes no slotted play (`team::assign_trapped`), so a
  teammate takes the battery or the spot. `Ctx::path_dist` is unchanged.
- **Hurt.** While the bot may not jump (`Hurt::may_jump`: a broken leg, a
  hit's stagger) the clocks of its try and of its giving up stand still, and
  the stuck recovery's back-off and side step press no jump.
- **Side steps.** The fight's side step does not step into a pocket the bot
  is not in that has no Standing exit (`Tactics::clear_step_for`), nor onto
  the band of a fall or a lip: with `Tuning::aim.combat.clear_step_sees_lip`
  (off by default; the bench's `SRP_BENCH_TUNING` turns it on) the step's end is never in a
  lip's band and its line crosses none unless the body starts in it, and the
  move tested is the one the keys make (`drive::side_step`: a weave is a
  diagonal). No step cleared: no weave, the body stands or walks straight.
- **For the plays.** `Ctx::trapped`, `teammate_trapped`, `in_pocket` and
  `escape_known(at, carrying)` (a way out the team knows, or a Standing
  exit; a carrier counts Standing exits only).
- Trace lines: `{"escape":{from,to,rise,across,jump},"try":n}`,
  `{"escaped":"worked"|"failed","exit":..}`, `{"escaped":"gave up"}`.
- All the numbers are GUESS consts.

**The stand-in's limit.** The bench's walker lands a jump that is not a grid
link only on a floor within 0.6 m ahead (a GUESS of the client's standing
jump), so it cannot land the exits 0.8-1.0 m across that most Standing
pockets have (the grid links jumps up to 1.05 m, so what is left is gaps;
the grid lists exits up to 1.0 m across, `POCKET_EXIT_ACROSS_M`). Pit A is
one of them. `SRP_BENCH_JUMP_AHEAD_M` sets the stand-in's reach for a
sensitivity row; the real reach is not measured, and whether bots get out
of those pockets is for the client recordings to show.

**Tests** (`pocket_runs` in `tests.rs`, a pit in an 80 × 40 yard, with
`max_jump` 0.6 so a 0.9 m rim has no link):

| Pit | Result |
|---|---|
| 0.9 m, Standing exits | out at 2.6 s, at the goal at 7.55 s |
| 1.2 m, Sprint exits | tries the 2 lowest once each, gives up |
| 1.6 m, no exit | gives up |
| Side step beside a 1.6 m pit | keeps out |

The ignored `pocket_sweep` drives a bot from each pocket of each bench map
to its team's spawn for 60 s, with the escape off and on (16 maps, pockets
counted per map). A pocket is labelled by the lowest exit the bot walks to
inside it (an exit on a floor reached only by a drop from elsewhere is not
the bot's):

| Pockets | count | out, escape off | out, escape on |
|---|---|---|---|
| Standing exits | 79 | 0 | 53 |
| Sprint exits only | 30 | 0 | 1 |
| Steep only | 1 | 0 | 0 |
| No exit | 52 | 0 | 0 |

Standing pockets by map, out with the escape on: level_01 10/10, level_02 2/6,
level_03 4/6, level_04 8/10, level_05 2/2, level_06 23/30, level_07 4/6,
level_08 0/9; out in 2.2-4.0 s on average. The 26 left have no Standing
exit nearer than 0.7 m across (level_08's, pit A among them, are 0.9-1.0 m),
beyond the stand-in's landing reach of 0.6 m, a GUESS of the client's
standing jump. They are to be checked again against the real jump reach
once it is measured.

Three level_06 pockets with a Standing exit 0.5-0.56 m across were not left
until the motor's arrival was fixed (`motor.rs`): a route whose last leg is
a jump to a landing within `arrive_m` (0.6 m) and under `ARRIVE_BELOW_M`
(0.5 m) up was "walked" with no press. Such a route is now arrived at only
on its landing (`ARRIVE_ON_LANDING_M`, 0.15 m). The fix is general (any route
ending in a near, low jump); alone it moved nothing in the bench (batt and
tdm presets, seeds 1-8, everything within 1 SE).

**Measured** (`real_bot_cost`, the change against 626db4b5 with the same
pocket tally, both sides run here; ± is 1 SE, * beyond 2 SE). `RouteTally`
counts living bot-seconds on a pocket's floors (`pocket bot-s`), deaths
there, and the escape's tries, exits that worked or failed, give-ups and
slots handed on. The stand-in's reach changes both sides, so each row is
its own pair: the batt and tdm presets, tier unset, seeds 1-16 (192 pairs
a preset).

| | reach 1.0 m (the grid's exits) | reach 0.6 m (the GUESS) |
|---|---|---|
| pocket bot-s, batt | 11.60 → 8.91, -2.69 ± 1.77 | 12.80 → 9.76, -3.04 ± 1.56 |
| pocket bot-s, tdm | 10.29 → 2.99, -7.29 ± 2.04* | 11.93 → 10.74, -1.19 ± 2.32 |
| pocket deaths, batt / tdm | -0.021 ± 0.010* / -0.073 ± 0.027* | -0.016 ± 0.009 / -0.010 ± 0.024 |
| tries / worked / gave up (sum), batt | 111 / 108 / 18 | 93 / 90 / 22 |
| tries / worked / gave up (sum), tdm | 88 / 35 / 4 | 204 / 10 / 19 |

Guards (batt; tdm), reach 1.0 m: K/D +0.02 ± 0.02 and +0.00 ± 0.03;
-0.01 ± 0.01 and -0.00 ± 0.01, kills -0.19 ± 0.14; +0.27 ± 0.12*, delivered
-0.03 ± 0.05, carriers killed -0.05 ± 0.05, quick ends a bot-minute
-0.064 ± 0.019* (better); 0.000 ± 0.004, flips -0.007 ± 0.004;
-0.000 ± 0.005, idle -0.12 ± 0.14; -0.50 ± 0.17* (better), team kills
0.000; -0.006 ± 0.006, stuck notes +0.05 ± 0.15; -0.11 ± 0.10, falls off
the route 0.00 ± 0.07; -0.05 ± 0.10. Falls on the route (drops of a metre
or more in a tick where the bot's own route has a corner a metre lower
within a metre of the landing: planned drops) are up in tdm, +0.30 ±
0.14*: +2.31 ± 0.55* in the 29 matches with tries (pocket bot-s -44.7 ±
10.7* there), -0.06 ± 0.12 in the 163 without. Bots out of pockets walk
routes that drop.

Reach 0.6 m: K/D -0.04 ± 0.03 and +0.04 ± 0.03; -0.01 ± 0.02 and +0.02 ±
0.02, kills +0.08 ± 0.12; +0.11 ± 0.11, delivered +0.05 ± 0.04, carriers
killed +0.02 ± 0.06, quick ends -0.046 ± 0.019* (better); -0.000 ± 0.003,
flips +0.001 ± 0.003; +0.001 ± 0.004, idle -0.13 ± 0.13; -0.01 ± 0.15, team
kills +0.006 ± 0.006 both, stuck notes -0.15 ± 0.19; -0.06 ± 0.13. Falls off
the route in batt +0.18 ± 0.09*: +1.24 ± 0.49* in the 21 matches with a
give-up, +0.03 ± 0.03 in the 138 with no pocket: tries at rims the stand-in
cannot land, bounded by the give-up clock.

Slots handed on by bots given up: 0-2 a seed in every row.

The 16-map bench (`SRP_BOT_ROUTES`, 16 maps × 4v4 and 10v10, seeds 1-16:
512 pairs) at reach 0.6 m: pocket bot-s 18.53 → 17.93 (-0.60 ± 1.18), jumps
pressed +1.80 ± 0.37*, stuck notes +0.07 ± 0.09, stuck at a mate's spot
+0.02 ± 0.04, flips -0.003 ± 0.003, quick ends -0.025 ± 0.009* (better),
carriers killed +0.049 ± 0.028, falls off the route +0.15 ± 0.06* (+0.43 ±
0.23 in matches with tries, +0.03 ± 0.02 without), pushing +0.56 ± 0.17*
(tries pushing at rims).

The hurt clocks never came into play in the bench (no pocketed bot was hurt
in a try; tries 0.5-1.1 a match): seeds 1-8 of batt and tdm against the same
build without them, every guard within 1.5 SE. `a_hurt_bot_spends_no_try_on_a_pit`
shows what they do.

The real jump reach is not measured: the change is judged on the owner's
recordings of the final test matches.


**Left.**

- Team spots and routes are not kept from pockets with no way out: routes
  never plan into a pocket for a goal outside it, and the grid's sink cost
  moved nothing in the bench. To measure first.
- A bot that gave up stays in the pocket. Killing itself is gated on the
  client recordings showing that bots still cannot leave with nothing else
  wrong. The 26 Standing pockets whose exits are all 0.7 m or more across
  are the evidence to check first.

### Jumps while hurt (2026-10-24)

The game takes no jump of any kind with a broken leg (a leg pool,
`Damage::part_health` 6 or 7, 0.5 full, at 0; about 8 s), and the 0.5 s
stagger after each hit slows the body by up to half (the sprint-jump RE,
`FINDINGS.md` §1.8, PROVEN). A bot pressed its route jumps and hops through
both: a press the game refuses still counted one of the jump's
`JUMP_TRIES`, and three gave the jump up.

- **The gate** (`motor.rs`, `Hurt`, `Pose::no_jump`): while `Hurt::may_jump`
  is false (a broken leg, or within `HIT_JUMP_HOLD_MS` 500 of the last hit)
  `jump_key` presses nothing and counts no try, and `hop_key` does not hop.
  The leg gate is PROVEN; the hit hold is a GUESS (the stagger stops the
  sprint, not the jump: a slowed body may land short).
- **Hops are no combat move:** `hop_key` jumps only walking a route, when a
  world puts the body back behind its reckoning while the ground climbs (a
  lip). No dodge under fire uses it, so the hit hold costs no fight.
- **What it reads:** the leg pools, and `m_last_hit_time_ms`
  (`Damage::last_hit_time_ms`, the damage model's field after the second
  chance, which the decoder already stepped over), from each trusted world
  (`game/driven.rs` → `Bots::hurt`). `Hurt::may_jump` is for other jumps
  too (unstuck's escape).
- **Stamina is left out.** The game takes no jump under 10 stamina, nor from
  0 until it is back over 20, but the world's state does not carry stamina:
  a server reckoning would be a GUESS on unknown rates (its regeneration,
  each hit's stamina damage). Open, for the sprint RE.

**The stand-in** (`bots/tests.rs`, a measurement change for every later
bench): one hit in 4 lands on a leg (`STAND_IN_LEG_SHARE`, GUESS), taking
0.25 of its 0.5 pool (`STAND_IN_LEG_HIT`, GUESS: one or two rifle hits empty
one); an empty leg refuses every jump for 8 s. A hit slows the body to 0.75
of its speed for 0.5 s (`STAND_IN_STAGGER`, GUESS between the game's 15-50
%). The leg is drawn by a hash, so the game's draws do not move. Each run
prints `jumps: pressed N refused N`.

**Bench** (`real_bot_cost`, presets batt and tdm, tier none, seeds 1-16:
192 pairs each; `with − without`, * beyond 2 SE; before: fc01ace3, which
decides as 0741a7c0):

| | jumps refused | jumps pressed | stuck notes | kills | K/D team 0 / 1 | delivered | carriers killed |
|---|---|---|---|---|---|---|---|
| the stand-in's hurts, against the tip, batt | | | −0.13 ± 0.31 | +0.41 ± 0.26 | +0.00 ± 0.08 / +0.03 ± 0.08 | +0.02 ± 0.09 | +0.19 ± 0.13 |
| the stand-in's hurts, against the tip, tdm | | | +0.20 ± 0.28 | −0.08 ± 0.28 | −0.00 ± 0.08 / −0.02 ± 0.05 | | |
| the gate, against the stand-in's hurts, batt | −0.109 ± 0.026* | −0.18 ± 0.11 | +0.09 ± 0.07 | −0.02 ± 0.06 | −0.01 ± 0.01 / +0.00 ± 0.01 | −0.01 ± 0.01 | −0.01 ± 0.02 |
| the gate, against the stand-in's hurts, tdm | −0.146 ± 0.030* | −0.14 ± 0.07 | +0.05 ± 0.06 | −0.02 ± 0.06 | −0.00 ± 0.01 / +0.00 ± 0.01 | | |

- The gate takes away 55 % of the jumps the game refuses (0.20 → 0.09 a
  match in batt, 0.27 → 0.13 in tdm). The rest are pressed before a world
  shows the leg broken: the bench's worlds come every second, 300 ms behind.
- No guard is worse beyond 2 SE: quick ends +0.003 ± 0.004 / −0.002 ±
  0.002, flips +0.002 ± 0.001 / −0.004 ± 0.002, idle 5 s+ +0.03 ± 0.06 /
  +0.03 ± 0.08, team kills 1 / 1 over the 192 runs each, as without.
- Such jumps are rare (0.2-0.3 a match, of 15-25 jumps), so stuck notes do
  not move. Beside the guards: falls on route −0.12 ± 0.06* in batt, falls
  off the route +0.08 ± 0.03* in tdm.
- The stand-in's hurts moved no row beyond 2 SE against the tip (the
  nearest: batt deaths a bot-minute +0.013 ± 0.007, reversals +1.42 ±
  0.71). Team kills over the 192 runs: batt 0 / 1 / 1, tdm 3 / 1 / 1 (tip,
  the stand-in's hurts, the gate).

### The client's wall slide, and the stand-in's capsule (A191, 2026-10-31)

The stand-in slid a body along every wall of the lattice at full speed and let it through every gap the grid has, so no bench showed
the stalls the live game has at door jambs (A66: the stand-in passed the 717 jamb on Chemical Plant at 4.8-5.9 m/s). The rule the
client moves a capsule by is `bullet_character_controller::player_step` 0x7BDA0 → `step_forward` 0x7D590 → `step_forward_impl` 0x7D2A0
(PROVEN, read from the exe, `get_wall_slide_vector` 0x7AC50 run with Unicorn: equal to the rule below at 0, 10, 20, 25, 26, 30, 45, 60
and 80 degrees):

- The move of the tick (velocity × dt) is swept as a capsule; with a contact, the body goes to the contact (the sweep's fraction `t`) and
  the rest of the move, `(1 - t)`, is slid: `unit(m - n (m·n) + K n) × |m| × f(c) × (1 - t)` for the unit move `m` and the wall's
  horizontal unit normal `n`, `c = |m·n|`, `f = 1` when `c < D`, else `(1 - c) / (1 - D)`. Then the part of the slide that points
  back against the first direction of the walk is taken off, a slide with `|v|² < 1e-5` ends the move, and up to three sweeps are made
  (depth 0 to 2), while `cc_wall_slide` (0xA7203F, default on) holds.
- `D` is `cc_wall_full_slide_dot` (0xA7AB14), `K` is `cc_wall_slide_bouncing_factor` (0xA7D5D8). The exe's defaults (0.9, 0.001) are not
  the game's: `startup.cfg` of `resources.db` sets `cc_wall_full_slide_dot 0.50` and `cc_wall_slide_bouncing_factor 0.080` (with
  `cc_wall_slide on`, `cc_wall_normal_via_ray on`, `cc_wall_normal_via_ray_offset 0.030`, `cc_move_step_margin 0.040`,
  `cc_stand_move_complex_test on`, `cc_step_height 0.80`). So a wall met within 60 degrees of its normal takes speed
  (30 degrees off the normal: 27 % of the rest of the move is slid; 45 degrees: 59 %; head on: all of it is lost), and a wall met
  within 30 degrees of its plane is slid along at full speed.
- The contact normal is the horizontal part of the sweep's (`character_move_sweep_callback::addSingleResult` 0x895F0); with
  `cc_wall_normal_via_ray` and a move more head-on than `D` the exe takes a face's normal from a ray instead (0x89B80, LIKELY: a ray
  along the move, 0.03 m past the contact). At a jamb's corner that is the difference between the round normal of the capsule's hemisphere
  and the face's: a body walking at the face is stopped, one 0.25 m beside its edge slides round it at about half speed.
- Not read: the Bullet sweep itself (`character_controller_move_step_tester::convex_sweep_test` 0x85A90 with the complex test on) and
  the second half of 0x89B80; the stand-in takes the sweep to be an exact circle of the capsule's radius 0.38 m in the walls'
  horizontal sections (GUESS: the complex test's shapes).

`tests/standin_capsule.rs` does this for the stand-in's bodies (`SRP_BENCH_STAND_IN_CORNER`, on unless `off`): the walker mesh's
faces that are not ground (steeper than the client's slope, or `dont_walk`) are cut by four planes 0.9-1.75 m over the feet (the step
height is 0.80: lower faces are steps), the body is a circle of radius 0.38 m in those sections, pushed out of a wall it overlaps (at
most 0.15 m a tick) and moved by the rule above; the lattice then walks it to where the rule put it (ledges, floors and the
sprint lift stay the lattice's and the intended move's). Tests-only: the bots' reckoning does not know it.

**Against live players (A191).** The hold is the capsule's radius: of the live alive samples of the 12+ human matches (the reach record's
`human/*.tsv`, 250 ms apart, on the ground), measured to the walker mesh's wall sections at the same four heights
(`live_wall_clearance` in `standin_capsule.rs`), the first percentile of the distance to a wall is 0.376-0.385 m on levels 02, 03 and 07
and the days of 04 (nearly every file), the fifth 0.41-0.44 m (players walk along walls). Levels 01, 06 and 08 have a thinner tail (first
percentile 0.13-0.38 m) and level 05 a thick one (0.03-0.23 m, 2-5 % of its samples under 0.30 m): faces the sections count as walls that
a player stands through (LIKELY thin or non-colliding faces; not read). The grid keeps its floors 0.3175 m from a wall (`exact_clearance`,
A66), so the stand-in's lattice put bodies where a live capsule never stands.

**717 door on Chemical Plant** (`door_corner.rs`, the A66 walk of fx-struct without the door gates, 12 starts that climb the ramp and
turn 68 degrees, both days; `SRP_BENCH_STAND_IN_CORNER` off against on):

| | closest jamb | ms under the controller's 0.41 m | ms under the radius 0.38 m | ms under 1 m/s | ms within 4 m of the door | stuck notes | reached |
|---|---|---|---|---|---|---|---|
| lattice (off) | 0.32 m | 1200 | 500 | 450 | 22500 | 0 | 12 of 12 |
| capsule (on) | 0.37 m | 200 | 50 | 1200 | 23800 | 0 | 12 of 12 |

The capsule keeps the body off the jamb (the 0.30 m crossing of the route becomes 0.37-0.52 m, the body slides round the corner) and the
pass takes 110 ms longer; it does not stall for long. So the wall slide alone does not hold a body at this door the way the live bots'
stalls 0.5-0.9 m south of its line were seen: that needs the bot's steering (A66's look-ahead into the jamb) on top. NOT shown here.

**Bench** (`real_bot_cost`, TDM and batteries, 10v10, tiers 4-5, 9-10 and 13-15, seeds 1 and 2, `SRP_BENCH_VIEW_ERROR=1`; one
binary, `SRP_BENCH_STAND_IN_CORNER` off against on; 6 runs a map and arm; contacts are the `contact::contacts` counter of A66, stuck the
route stuck notes):

| map | contacts off / on | contact seconds off / on | stuck off / on |
|---|---|---|---|
| level_01 day 2 | 288 / 266 | 243 / 225 | 58 / 94 |
| level_01 day 4 | 286 / 286 | 238 / 236 | 43 / 61 |
| level_02 day 2 | 601 / 605 | 565 / 539 | 162 / 237 |
| level_02 day 4 | 325 / 317 | 291 / 278 | 66 / 111 |
| level_03 day 3 | 339 / 278 | 285 / 243 | 46 / 93 |
| level_04 day 2 | 488 / 562 | 457 / 524 | 105 / 305 |
| level_05 day 2 | 569 / 464 | 498 / 389 | 84 / 123 |
| level_05 day 4 | 387 / 333 | 339 / 282 | 69 / 75 |
| level_06 day 2 | 499 / 500 | 451 / 437 | 156 / 206 |
| level_06 day 4 | 425 / 438 | 400 / 384 | 138 / 329 |
| level_07 day 4 | 670 / 661 | 637 / 553 | 224 / 266 |
| level_08 day 4 | 274 / 260 | 216 / 221 | 74 / 92 |
| all (72 runs) | 5151 / 4970 | 4620 / 4311 | 1225 / 1992 |

Of 4.1 million moves of walking bodies with the capsule on, 6.5 % met a wall, 2.0 % made under a tenth of the move (stopped), 3.2 % under
nine tenths (slowed), and 1.4 % began inside a wall's radius (pushed out, at most 0.15 m a tick). The bench takes as long as before (a
TDM run 328 s with and without). Stuck notes rise on every map (+63 % over the 72 runs), contacts barely move (−4 %): the walls that
catch the capsule are walls the lattice passed at the grid's slack. The new stuck places are, where looked at (level_04 day 2 and level_06
day 4), nooks live players do not use (no live samples within 1.5 m of two of four) or a corridor 1.1 m wide with floors the capsule does
not fit (`capsule_spot_report`); a live path that crosses one (level_06 day 4, −23 / 142, 18 samples over 1 m/s) stands 0.73 m from the
walls at its nearest.
