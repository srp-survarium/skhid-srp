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
   taken again at once (it had been up only past `QUICK_FAIL_MS`), with
   the search found for it. Now (`mod.rs`): a play that took too long is
   set aside like one that failed quickly, and its found route, its
   "no way" and its pending search are dropped.

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

LIPRESULT

### 8.6 Measured

The bench (`real_bot_cost`), 8 seeds, maps level_06/2, 01/2, 03/3,
02/2, 04/2, 05/2, 06/4, 08/2, 4v4 and 10v10: 128 pairs, each one
(map, size, seed) before and after; the mean of each, and the mean of the
paired differences with its standard error. "Before" is f4c312ac.

| | before | routes and stuck (8.1-8.3) | and the ledge (8.4) | ledge vs routes |
|---|---|---|---|---|
| pushing, s a match | 770 | 67 | 19 (-751 +- 56) | -48 +- 5 |
| idle, s a match | 1042 | 379 | 284 (-758 +- 57) | |
| idle while fighting, s | 48 | 25 | 11 | |
| batteries delivered | 3.20 | 4.54 | 5.07 (+1.88 +- 0.21) | +0.53 +- 0.11 |
| carry time, s | 24.5 | 27.9 | 25.3 (+0.0 +- 0.8) | -2.5 +- 0.7 |
| stuck notes | 9.4 | 62.2 | 27.8 | -34 |
| of them, keys held | 0 | 53.2 | 20 | |
| reversals | 26.5 | 47.6 | 45.4 (+18.9 +- 1.4) | |
| stood with a goal, % | 4.8 | 7.9 | 7.2 | |
| grid searches | 923 | 1405 | 1392 | |
| failed searches | 73 | 188 | 130 | |

Bots that walked into walls before now notice and turn (the stuck notes
and reversals go up as the pushing goes down: a reversal is a recovery's
back-off or a re-plan after a fall). More searches: a bot off its route
plans again instead of pushing.

### 8.7 Left

- **No drop link where a face lies on a cell boundary** (8.4): the grid
  has no way down there at all (`mapdata`).
- **Reversals and standing with a goal went up** with the fixes (more
  re-plans, more "starved" and "ended" ticks): fewer seconds lost, but
  more turns a human would not make.
- **A partial route that runs out** before its search ends: the bot
  stands until the search ends (a few polls).
- **A goal under a ramp** plans to the nearest floor, which may be the
  ramp's top or its foot: whichever is nearer, not whichever the play
  meant.
