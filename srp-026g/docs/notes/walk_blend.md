# The walk vector and the 0.3 s blend in the 0.26g client

Read in the executable (virtual addresses, image base 0x10000; names in `tools/re-kit/v026g`; the animation
mixer's functions matched by shape to their named twins of the 0.21d build). PROVEN where an address is given,
unless said otherwise. Counted on a replayed match (16 bodies, 220,994 steps; `docs/notes/exactness_rig.md`). See
also `turning_and_jumps.md`, `sprint_state.md`, `crouch_logic.md`.

## The walk vector

`base_player::set_physics_controller_walk_vector` 0x2D9060: the animation tree's object position
(`get_object_transform` 0x39F360) minus the body's origin; when the controller is not in a jump (+2C70Ch clear)
and its horizontal length is at least `max_speed` 0x2DC010 times the step, it is cut to that (0x2D91AC..0x2D923F).
**The cap is on the blended sum, once, never on a clip.**

## A step's move

- Each clip with root motion adds up its own root displacement since the last reset
  (`accumulate_object_movement` 0x39F860). The object's move is the plain weighted sum of those, no normalisation
  (`bone_matrices_computer::get_object_transform` 0x3B2D20 -> `mix_translations` 0x3B1B60), each weight the one at
  the END of the tick (0x39F8BC). After the physics step the sums are reset to the body
  (`assign_physics_transform` 0x2D93A0).
- **A transition's weight is linear in time**: (1 - u) x old + u x new with u = elapsed / duration
  (the weight calculator's 0x3B0CD0; `linear_interpolator::interpolated_value` 0x395360 is `t / duration`); at the
  duration the target stands and the transition is removed (0x3B0BE0). No easing: the only other interpolators
  (fermi) are the IK solvers'.
- **The duration is 0.3 s** (0x921804) for every stand, crouch and sprint movement; the short jump uses 0.10 and
  the sprint jump's prepare `jump_prepare_interval_length` (0.040 in `startup.cfg`).
- **A second change during a blend nests**: the new transition starts from the old weights with their transitions
  still running (0x3A4580 / 0x3A4490). It neither restarts nor snaps.
- **Clips of one time-synchronised group share their phase, and the group's rate itself blends.** The run and
  sprint movements are in a group (+4Ch = 0, 0x336093, 0x33670A); the on-site clip is in none (-1, 0x335B21). The
  field reaches the tree's node +40h (0x394CE7, 0x399CE0, 0x397C84), which only the group's code reads (0x3A0190,
  0x39FD70, 0x3A28D0). The phase is kept by interval (a step), not by clip: a member's time is its interval's
  length over the leader's (0x39FF6E); the run clips' intervals are 12 and 12 frames, the left clips' 11 and 13, the
  sprint's 9 and 9, the crouch's 15 and 15. A clip joining a group starts in phase with the old leader, and its time
  scale goes
  from the old rate to its own over the same interpolator (`new_time_scale` 0x3A28D0). The group's time advances in
  10 ms pieces, the time scale read at each piece's end (0x39FD70, 0x3B1450): a change of rate runs (k - 0.4) / 6
  over 7 steps, not k / 6 over 6.
- **Mid-frame bakes**: at an animation event of the mask 0x1DF (`need_new_transform` 0x3A0260) the tick fixes the
  blended move so far and resets the sums (`n_ary_tree::tick` 0x3A19F0, 0x3A1A87, 0x3A1AE6). The events: an
  interval's end of any clip of the body (bit 4; a movement clip has two a cycle, a step each), a clip's end (8,
  10h), a clip's start (80h, with 1 for a new one), the end of a weight transition (100h) and a rate's sign change
  (40h); a clip's sound or `shoot` knot (20h) is not one. A frame with such an event has one step of a blend up to
  about 4 % of the speed off.
- **An aim change is two nested rate ramps.** The step ending 50 k ms after the AIM input moves at
  vA + (vB - vA) x [1 - (1 - u(k)) (1 - u(k - n))], u(j) = clamp((j - 0.4) / 6, 0, 1) for j >= 1, else 0;
  n = ceil(T / 50 ms) - 1, T the gun's aim time (`aim_time` x max(0, 1 + modifier 49)). The first ramp is the
  selection the key asks, the second the one selection the weapon asks when its aim transition arrives. Every
  selection takes the aimed (or unaimed) clip of the direction at once: the builder's aiming argument is the
  weapon's flag +5E8h (0x2E1110, 0x335F35). T over 0.30 s leaves the plain ramp.

## The order in a frame

The player's update to the newest input's time (0x2DA9A0), for each queued input time t: `base_player::tick`
0x2DAA60 (the animation ticks to t, the clips re-selected at the previous input's time when the keys changed: the flag
+33Eh is set by `apply_new_input` itself when the key word changes, 0x2D8995, and also by a change of the carried
weight, the stagger, a leg's term, a hit animation, and once by the weapon, at the tick its aim transition arrives:
`transition_helper::tick` 0x32FE40 returns 1 only then, 0x32FEC2; the flag 0x2E0A59), then
`apply_new_input` 0x2D86E0 at t; after the loop the walk vector (0x2DAA2B), then the physics step. So **the step
ending at t moves by the blend of the inputs up to t - 50 ms, and is capped by the keys of input t** (`max_speed`
asks whether the body sprints after input t is applied).

## Counted

| case | events | result |
|---|---|---|
| clean starts (at rest, then a move key, level ground) | 28 | k / 6 of the speed on step k, median error 0.0000 m/s (0.632 1.263 1.895 2.527 3.159 3.790) |
| clean stops | 82 | 1 - k / 6 |
| their one-step-off steps | 19 and 84 | each one mid-frame bake |
| forward <-> front diagonals, back <-> rear diagonals | 315 | the linear blend, exact |
| side <-> forward or back | 68 | the linear blend off by a median 0.060 m/s; with the group's rate blend 0.019 |
| sprint start | 210 | 0.1198 0.2784 0.4417 0.6098 0.7827 0.9603 of the change; the rate blend with a measured rate ratio 1.212 fits to 0.0001 m/s |
| sprint release, the step ending the release's input | 201 | at the walk cap in 192 (5.287 -> 3.731 m/s): the cap is input t's |
| a second change during a blend | 495 (4,455 steps) | nested layers: median error 0.0001 m/s (kept frozen 0.118, snapped 0.036) |
| an aim change with the move keys held (aim off 20, aim on 1) | 21 | two nested rate ramps n ticks apart: 20 within 0.0023 m/s on every step (the other starts inside an earlier blend); the plain ramp is off by 0.011 (n = 4) and 0.033 (n = 3) |

The sprint start's rate ratio is 1.2121 from the clips themselves: the sprint clip has 18 keys (0.6 s, two
intervals of 9 frames, its root 3.2727 m) against the run clip's 24 (0.8 s, two of 12, 2.8 m), played at 5 / 5.5
against 1.

## The movement clips' own speeds

A clip's play rate is desired / default of its class (`load_movement_speed` 0x2F0B00, `movement_speed` 0x2DBB50) x
the speed factor F, and the body moves at the clip's root speed x that rate, cut at the cap (3.5 x F, sprinting
5.0 x F; `max_speed` 0x2DC010). Where the root's speed is not the config's `default`, the body's speed is not
`desired`. Read from the clips (the decoder checked on 103 fire clips' lengths and on the traced steps):

| gait | root speed, m/s | default, desired | the body's speed / F | steady steps on it |
|---|---|---|---|---|
| run forward and its diagonals | 3.5 | 3.5, 3.5 | 3.5 | 17,295 of 17,585 |
| run back and its diagonals | 3.2 | 3.2, 3.0 | 3.0 | 62 of 77 |
| run left, right | 2.2440 | 2.25, 2.5 | **2.4933** | 490 of 499 |
| aimed forward, back | 3.5, 3.2 | 3.5, 3.0; 3.2, 2.5 | 3.0, 2.5 | 312 of 481 (the others in the aim's two ramps) |
| aimed left, right | 2.2440 | 2.25, 2.0 | **1.9947** | 61 of 89 |
| crouch forward | 2.2 | 2.2, 2.5 | 2.5 | 3,016 of 3,059 |
| crouch back | 1.95 | 2.0, 2.0 | **1.95** | 58 of 58 |
| crouch left | 1.41 | 1.4, 2.0 | **2.0143** | 141 of 141 |
| crouch right | 1.4586 for 29 frames of 30, then still | 1.4, 2.0 | **2.0837**, still for a 30th of each cycle | 16 steps at 2.0837 |
| crouch and aim: forward, back, left | 2.2, 1.95, 1.41 | 2.2, 2.0; 2.0, 1.5; 1.4, 1.5 | 2.0, **1.4625**, **1.5107** | |
| sprint and its diagonals | **5.4545** (3.2727 m in 0.6 s) | 5.5, 5.0 | **4.9587** | 10,909 of 11,214 |

The sprint is under its cap: the cap does not bite on a steady sprint.

## Against the bots' model (`bots/kin.rs`, `body.rs`)

The same: the k / 6 ramp over 6 steps, nested layers, the cap after the blend, starts and stops. Different: the cap
of a step is chosen there by the keys that move it (input t - 50 ms), in the client by input t's, 1.555 m/s on the
step ending a sprint release (78 mm); no rate blend of a synchronised group (a sprint start 0.06 to 0.09 m/s fast
for six steps, about 24 mm; side changes a median 0.060 m/s); a rate change of one clip takes 7 steps on
(k - 0.4) / 6; no mid-frame bakes (up to 7 mm on one step).

## Not read

Which node leads a group when transitions overlap; which clip's interval end is the
bake of a start (the tree's node list at a start is in no trace). 0x3FE260 is only a function object's call
operator, not a callback of its own.
