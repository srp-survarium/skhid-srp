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
  sprint movements are in a group (+4Ch = 0, 0x336093, 0x33670A); the on-site clip is in none (-1, 0x335B21; that
  +4Ch is the group is LIKELY). A clip joining a group starts in phase with the old leader, and its time scale goes
  from the old rate to its own over the same interpolator (`new_time_scale` 0x3A28D0). The group's time advances in
  10 ms pieces, the time scale read at each piece's end (0x39FD70, 0x3B1450): a change of rate runs (k - 0.4) / 6
  over 7 steps, not k / 6 over 6.
- **Mid-frame bakes**: at an animation event (mask 0x1DF, `need_new_transform` 0x3A0260) the tick fixes the blended
  move so far and resets the sums (`n_ary_tree::tick` 0x3A19F0, 0x3A1A87, 0x3A1AE6): a frame with such an event has
  one step of a blend up to about 4 % of the speed off. Which events is not read (LIKELY the clips' interval ends).

## The order in a frame

The player's update to the newest input's time (0x2DA9A0), for each queued input time t: `base_player::tick`
0x2DAA60 (the animation ticks to t, the clips re-selected at the previous input's time when the keys changed), then
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

The sprint start's rate ratio 1.212 (the sprint clip about 0.75 of the run clip's length) is measured, not read:
the clips' lengths, root curves and event times are resource data.

## Against the bots' model (`bots/kin.rs`, `body.rs`)

The same: the k / 6 ramp over 6 steps, nested layers, the cap after the blend, starts and stops. Different: the cap
of a step is chosen there by the keys that move it (input t - 50 ms), in the client by input t's, 1.555 m/s on the
step ending a sprint release (78 mm); no rate blend of a synchronised group (a sprint start 0.06 to 0.09 m/s fast
for six steps, about 24 mm; side changes a median 0.060 m/s); a rate change of one clip takes 7 steps on
(k - 0.4) / 6; no mid-frame bakes (up to 7 mm on one step).

## Not read

The clips themselves; which node leads a group when transitions overlap; the aim-on curve (neither shape fits it:
LIKELY a second re-selection); what sets the re-select flag on a change of state (0x3FE260).
