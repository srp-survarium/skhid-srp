# Turning and the jump in the 0.26g client

Read in the executable (virtual addresses, image base 0x10000; names in `tools/re-kit/v026g`). PROVEN where an address
is given, unless said otherwise; config values as the game data and `startup.cfg` have them. Counted on a replayed
match (16 bodies, 220,994 steps, 116 whole jumps; `docs/notes/exactness_rig.md`). See also `sprint_state.md`.

## Turning

- **The yaw follows the input at once.** `base_player::apply_new_input` 0x2D86E0 decodes the input's two angles and
  `apply_rotation` 0x2D8AD0 adds the first to the yaw (player+490h): no clamp, no rate, no smoothing; the animated
  body is turned in the same call. Counted: the yaw's change between two steps is the input's angle on 189,137 of
  189,137 step pairs; the largest turn in one 50 ms step is 52.3 degrees.
- **`max_angular_velocity` (720) and `smooth_linear_speed` (3.0) limit nothing.** They are console variables that no
  game code reads (the only references to their values are their own initializers; a scan of the whole file).
- **The walk direction lags the view by exactly one step, then takes the new heading whole** (the walk vector of the
  step ending at t is the root motion in the heading after the input stamped t - 50 ms): 8,490 of 8,490 turning
  steps within 0.5 degree. Within a step the path is straight.
- **The 0.3 s start/stop blend** is the animation mixer's linear interpolator (0x921804), built by every stand and
  crouch state when the keys change; a turn of the view alone does not start it.
- **Speed by direction**: each direction plays its own clip at desired / default times the modifiers; the front
  diagonals at the forward rate, the rear diagonals at the back rate, the sides at the side rate (0x2F0980). Counted
  against forward: back 0.859, sides 0.712, front diagonals 1.000, rear diagonals 0.859; aiming forward / back /
  side 0.860 / 0.714 / 0.570; crouched forward 0.714; sprint 1.417 (the sprint clip moves at 4.96, not 5.0, times
  the modifiers).

## The jump

1. **Entering** (`player_logic_stand_jump_state::initialize` 0x337040): the type from the keys' direction. Forward
   and the front diagonals are a **sprint jump** only when the body's last horizontal move (player+334h, m/s) is at
   least `max_speed` 0x2DC010 times `long_jump_speed_factor` (1.0; the cap is the walk's, the state being the jump's
   already), the sprint keys are held and `can_sprint` 0x2DBF40 is true; else a short jump. Counted: the rule gives
   the traced kind in 116 of 116.
2. **A short jump's travel** (0x337280 and the state's two clips): the on-site clip and the direction's clip are
   blended with the weights w and 1 - w, **w = 1 - min(max(1, v) / S, 0.99)**, v the body's speed at the press, S the
   `jump_on_site_landing` row (4 forward). So the ground speed in the air is the direction clip's travel (3.33 m/s
   forward) times min(max(1, v) / S, 0.99): 0.833 x v for a forward jump, at least 0.83 m/s with a direction key
   held and at most 3.30. Counted: within 5 mm/s in 100 of 100. (It is not the walk's speed carried on: a body at
   the 3.79 cap travels at 3.16.)
3. **A sprint jump's prepare**: the direction's sprint clip blended in over `jump_prepare_interval_length` (0.040 in
   `startup.cfg`; the exe's default is 0.1) and played until its present interval ends: under 80 ms.
4. **The take-off is an animation event**, not a timer: the clip's callback named "jump" (0x3434A0) spends the
   stamina (`amount_to_jump`, tested at the press, spent here) and calls `base_player::jump` 0x2D7920, its only
   caller, which sets the controller's jump flag. Counted: the first air step ends 200 ms after the press for a
   short jump (100 of 100) and 300 ms for a sprint jump (16 of 16); the air is 12 steps in 116 of 116.
5. **Which leg** a sprint jump takes is the run cycle's supporting leg at the press (the selector's flag, set at each
   interval end of the run clip, 0x2FC000). Counted: left 13, right 3.
6. **Turning in the air** turns the travel: the root motion is laid in the body's present heading each step (88 of
   88 jumps that turned more than 5 degrees).
7. **Landing and leaving**: the air ends when the take-off clip ends; each landing substate's `initialize` calls
   `end_jump` 0x7F050; nothing takes speed at the landing. The state is left (`is_ready_for_transition` 0x337330)
   when the landing substate is finished (the short landing at its clip's mark; the sprint landing at the end of its
   0.4 s clip's interval), or both legs break, or the player dies; the sprint is then the first transition tested,
   with no wait. Counted: after a short jump a sprint resumes one step after the first ground step; after a sprint
   jump not before 200 ms and by 600 ms, the body at the walk cap meanwhile (15 of 16).

## Walking off an edge

Leaving the ground without a press is not a jump. The selector's state machine (built at 0x2FAB70) enters its jump
state (kind 4: the constructor 0x336B70 writes it at +24h, 0x336B87) only by the three transitions 0x2FAFEF, 0x2FB1A7
and 0x2FB315, all with the predicate 0x2FBDC0: the jump input and the jump gate 0x2FC230, whose ground test needs the
ground. No other predicate of that machine tests the ground. So a body that walks off an edge keeps the state it had
(still, run or crouched walk) at any height: the shot's gate `fire_pred` 0x2E3CB0 refuses only the jump state
(0x2E3CCD) and `can_aim` 0x2E3AC0 tests no movement, so the sights and the shot stay open through the fall; there is
no landing clip (the landing substates are the jump state's), only the fall's damage at the landing
(`on_physical_controller_landing` 0x2D8220); and the cone keeps its still or moving term (the call 0x2E17A4 passes
the state's stance +20h and kind +24h to 0x331330 / 0x331560).

## Not read

Where the "jump" marks and the intervals sit in the clips (resource data: only the counts bound them); how the
animation player carries time over an interval's end; the frame order of the input, the selector, the physics step
and the animation (measured above, not read).
