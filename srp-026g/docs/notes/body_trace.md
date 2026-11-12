# The game's own bodies, step by step (0.26g)

The walk tick's port (`crates/mapdata/src/controller/tick.rs`) and its Unicorn harness were proven on worlds
and states we made. This note is the other side: every step the running game itself makes for every player's
body (`bullet_character_controller`), taken at the step, with the state it starts from, the walk vector it is
handed and the state it leaves. Tags: PROVEN, LIKELY, GUESS. Addresses are image VAs (build 2777, base 0x10000).
Written 2026-11-12, on the revision-56 exe (the code the players run). Tools:
`scripts/collision-world/step_trace.py`, `step_report.py`, `step_vs_recording.py`.

**Result in one line:** a replaying game steps every body once a 50 ms grid step with the whole 0.05 s, from
exactly the bytes its last whole step left (21,078 of 21,539 steps in two traces; the rest are a crouch or a
jump input, or a spawn), and what a client uploads in a world is the body one step before the world's time
(617 of 628 moving bodies).

## 1. How the trace is taken

`step_trace.py <pid>` attaches to the game as a debugger and puts four execute breakpoints into the debug
registers of every thread's context. No byte of the game's code or data is changed (the registers are the
thread's own). At a stop it reads the thread's registers and the controller's memory, runs the stop's one
instruction with that stop off (trap flag) and puts it back.

| stop | what is read |
|---|---|
| 0x2D1E20 `game_world_core::atomic_tick`, entry | edi: the match time the step ends at; [esp+8]: its length, ms |
| 0x7B7A9 `updateAction`, before its call of `player_step` | esi: the controller; [ebp+0Ch]: dt; the controller's +0h..+210h and +2C700h..+2C790h; updateAction's return address |
| 0x7B7B7 `updateAction`, after the call | the same bytes (the walk vector is cleared only after it) |
| 0x7F6D0 `bullet_character_controller::set_transform`, entry | eax: the new transform (the feet); [esp+4]: the controller; [esp]: the caller |

All four are the stock exe's bytes in revision 56 (`bullet_cast.md`: the controller differs only at 0x7C7D7 and
0x7E6E6..0x7E75C). The game is a worker's own, started hidden with a replay (`bw_run_replay.py --keep`, idle
priority, two CPUs). Cost: 29,036 stops in 29.9 s left the game at real time (598 grid steps); with the
`set_transform` stop, 58,746 stops in 44.9 s, the game made 43.15 s of match time.

A sampler that only reads memory from outside (the first try) cannot do this: the origin is rewritten at every
frame's look ahead (section 2), and the walk vector is cleared at the end of each `updateAction`.

## 2. The frame of a tick (PROVEN: the exe, and the two traces)

The traces are of one replay (Vector Laboratory, mode 4, 16 places, made from a local test recording): 30 s
and 45 s, 1,995 atomic ticks, 27,166 controller steps. (A third, 20 s with the tool as it is in the
repository, has the same frame: 6,242 steps, 5,785 of 5,921 whole steps on the chain.)

1. `game_world_core::tick_events` 0x2D21E0 ends a step at the next 50 ms grid time, the next input's time or
   the frame's time, whichever is first, and calls `atomic_tick` for it (`tools/re-kit/v026g/abi.tsv`).
2. In an atomic tick, for every player, `base_player::set_physics_controller_walk_vector` 0x2D9060 sets the
   controller's transform from the player's own place (`set_transform` from 0x2D911F: 15,114 calls) and hands
   it the walk vector (+20h: metres over this tick's dt).
3. `bullet_physics_world::tick(ms)` 0x75550 calls `btDiscreteDynamicsWorld::stepSimulation` 0x9B260 twice:
   (0, 0, 1/60), then (ms x 0.001, 3, 1/60). The rigid bodies move in Bullet's fixed 1/60 s sub-steps. The
   actions do not: **the game's `stepSimulation` calls `updateActions` 0x9B810 once, after the sub-steps, with
   the whole time step** (0x9B35E..0x9B375, when the time step is over 0; stock Bullet calls it in each
   sub-step). 0x9B810 calls every action's `updateAction(world, dt)` (vtable +4). Measured: 13,698 of 13,698
   steps return to 0x9B848; the dt is the f32 of ms x 0.001f in 27,166 of 27,166; a controller has at most one
   step an atomic tick.
4. `bullet_character_controller::updateAction` 0x7B730 (read whole): it calls `player_step(dt)` when dt is the
   controller's own dt (+2C704h: 0.05; 0x7B4A1 is the controller code's only store) or when the origin (+B0h) is not `m_last_pos`
   (+50h); a body that did not move in its last step is not stepped in a part. Then it clears the walk vector
   (+20h..+2Ch) and the air-control vector (+30h..+3Ch).
5. `player_step` 0x7BDA0 begins with pos (+40h) := `m_last_pos` (+50h) := origin (0x7BE46..0x7BE94) and ends
   (0x7C734..0x7C797) with origin := start + (pos - start) x dt / own dt; then pos and the surface normal are
   rounded to 0.001 (`round_up` 0x7B7F0, six calls). The origin is not rounded. (A second ending,
   0x7C681..0x7C732, blends otherwise; not read.)
6. `bullet_physics_world::tick` then puts overlapping controllers of unlike +2C781h back
   (`resolve_character_pairs`, abi.tsv: both to their transforms from before the tick, velocity 0,
   `m_colliding_with_player` set).
7. Between the steps and the next atomic tick's entry `base_player::assign_physics_transform` 0x2D93A0 (abi.tsv:
   it takes the controller's transform into the player's own, +40Ch) sets each controller's transform once
   more (`set_transform` from 0x2D94C7: 14,054 calls; whether at the end of this atomic tick or before the next
   is not read).

**Whole steps and parts.** Of the 1,995 atomic ticks 1,461 are whole grid steps (50 ms, grid time to grid
time) and 534 are parts that start on the grid and end at the frame's time: a look ahead for the picture.
After a part `base_player::deserialize` 0x2DB490 puts every body back (`set_transform` from 0x2DB939: 1,231
calls for 82 parts of 15 bodies) and the whole step is made from the grid state. No step was split in two (a
part starting off the grid: 0 of 1,995). `tick_events` does split at an input's own time, so a live game's own
player may have split steps: not measured (LEAD).

**The chain of whole steps.** A whole step starts with exactly the bytes the whole step one grid time before
left (origin, `m_fall_and_slide_velocity`, `m_jumping`, the six flags, the two normals, the ceiling slide, the
dims) in 8,533 of 8,717 and 12,545 of 12,822 steps. The others differ in the flags (166, 247: a crouch input
between the steps), in `m_jumping` (13, 24: a jump began) or in the origin (5, 6: all at spawn places). Nothing
else changes a body between two steps, so a traced whole step is a closed case: the state before, the walk
vector, the state after.

**A part against its whole step** (first trace): of 1,640 parts that start from the same bytes with the same
input (walk vector x dt / own dt), 1,621 compute the whole step's end pos and 1,627 leave the origin at
dt / own dt of the whole step's way. The 19 and 13 others are not read (LEAD; the second ending is the first
suspect). Parts are never the simulation's truth in these traces.

## 3. The controller's fields (PROVEN: the exe's own names, the strings of `serialize` 0x7F7F0)

| offset | name | |
|---|---|---|
| +20h, +30h | (walk vector, air-control vector) | handed in before a step, cleared after it |
| +40h | (pos) | the step's working position; rounded to 0.001 at the step's end |
| +50h | `m_last_pos` | where the last step began; the position a world carries |
| +B0h | (the ghost's origin) | the capsule's centre; the feet are (radius + half height) below |
| +200h | `m_input_is_in_crouch` | |
| +201h | `m_logic_is_in_crouch` | |
| +202h | `m_can_stand` | |
| +203h | `m_capsule_is_in_crouch` | |
| +204h | `m_is_sprinting` | |
| +205h | `m_colliding_with_player` | |
| +2C704h | (own dt) | 0.05 |
| +2C70Ch | `m_jumping` | |
| +2C710h | `m_supporting_surface_normal` | rounded to 0.001 at the step's end |
| +2C720h | `m_blocking_ceiling_normal` | |
| +2C730h | `m_blocking_ceiling_slide_velocity` | |
| +2C770h | `m_fall_and_slide_velocity` | |

`serialize` writes them in this order: `m_last_pos`, `m_jumping`, +200h, +201h, +203h, +202h, +205h, the
velocity, the two normals, the ceiling slide, +204h; the origin is the player's own to save
(`base_player::deserialize` sets it with `set_transform`).

## 4. Against the recording

`step_vs_recording.py` lays the bodies the real clients uploaded (`srp-rec-table`'s `rec_worlds.csv`, three
decimals) against the replaying game's own bodies at the same match time.

- **A world of time W carries the body of W - 50** (PROVEN; `StepInput`'s doc in `bots/kin.rs` had it as
  LIKELY). By construction: the block's position is `m_last_pos`, which `player_step` sets to where the step
  began. Measured on the first trace (the game at real time): of 628 rows of a moving body, 617 are the
  trace's origin one grid step before the world's time within the table's rounding; 8, 15 and 11 fit two
  steps before, the world's time and one step after.
- **A replay is not the match.** 14 of 844 rows of the first trace are off that relation: two places in a
  jump (the same x and z, 0.35 m apart in height), one place by 2 to 5 cm for 2.5 s, one place 90 m away
  after a respawn. In the second trace (the game slower than real time) 236 of 1,233 rows are off, 171 by
  over a metre: bodies after a respawn stand elsewhere than the live game had them, and one place walks 1 to
  12 cm off for 12 s. So a replay's trace is the exe's own tick on real states with real inputs, which is
  what the port and the harness are checked against; what a live client's body did is in the recording's
  uploads, not in the replay.
- **Holes.** At one or two grid times a trace has no whole step for any controller (60750 and 70850 ms in the
  first, 50650 ms in the second: other times in two runs of one replay), and bodies stand elsewhere after it
  (one by 90 m, at the place the recording has). GUESS: the replay player sets a world there instead of
  stepping. Not read.

## 5. What it is for, and what it does not cover

- Every traced whole step is a test case with the game's own answer: about 290 a second from a 16-player
  replay, on the game's own collision world (`collision_world.md`: a capture of the same run). The port and the
  Unicorn harness can be run on the state before and the walk vector and laid against the state after, bit by
  bit.
- The live game's own player is not traced yet (split steps at input times, section 2). The tool attaches to
  any worker's test game the same way.
- LEAD: `set_transform` takes the feet and adds (radius + half height) along the up axis; the player's place is
  the feet. The origin goes through that subtraction and addition every tick. In the traces it came back to
  the same bits every time (heights 18 to 26 m); whether it does where the height crosses a power of two is
  not checked (GUESS: it can differ by one unit in the last place).
