# Fall damage

The owner's rule and what it changes. Data: `gameplay/players/default.player`
`player.fall_params`; the change ships as a resources.db patch
(`scripts/content-edit/character.tsv`, `player/fall_params/fatal_damage`).

- **Decision (the owner, 2026-10-19):** `fatal_damage` 1.0 as in 0.27: the
  0.27d0 value, kept by every later build (0.28a2, 0.28d0, 0.31a2, 0.46c2,
  0.69d: `i32:1`). 0.20e to 0.26g have `f32:1.5`. Nothing else changes.
- **0.27's changes only (the owner, 2026-10-20):** this takes what 0.27 changed
  in falls and nothing a later version (0.30 and on) changed. A fall value that
  differs only from 0.28 on is not taken here; a later version's changes would
  be a decision of their own.
- **Values:** `fatal_damage` 1.5 -> 1 (0.27 writes `i32:1`; the patch keeps the
  field's f32 and writes 1.0, which the loader reads alike: it takes i32 or
  f32). Unchanged: `harmless_height` 5, `fatal_height` 10, `gravity` 9.8,
  `soft_landing_height` 1.8, `noiseless_height` 1.
- **Who computes it:** the client, in the lockstep simulation (handler
  0x2d8220; the loader 0x2d6600 reads `fall_params`). srp does not simulate
  falls, and its server data reads nothing of `fall_params`.
- **Formula:** landing speed v = sqrt(2·g·h). Up to `harmless_height`
  (v ≤ 9.9 m/s) nothing; from there to `fatal_height` (v = 14 m/s)
  D = fatal_damage · (v − 9.9) / (14 − 9.9); higher, D = fatal_damage.
  D is dealt as a blunt hit to each foot (foot blunt -> leg ×1, pain ×0.8).
  `fatal_damage` goes through the player's modifier 30, `fall_damage`
  (apply_modifier 0x2d5ae0, multiplicative): fatal_damage · max(0, 1 + sum);
  the skill (207, −0.15 a level, 2 levels) makes it ×0.7 (PROVEN).
- **What it means:**

  | height | v (m/s) | D stock (1.5) | D new (1.0) |
  |---|---|---|---|
  | 5 m | 9.9 | 0 | 0 |
  | 6 m | 10.8 | 0.35 | 0.23 |
  | 7 m | 11.7 | 0.66 | 0.44 |
  | 8 m | 12.5 | 0.96 | 0.64 |
  | 9 m | 13.3 | 1.24 | 0.82 |
  | 10 m and more | 14.0 | 1.5 | 1.0 |

  - damage starts just above 5 m in both (the formula: any v over 9.9 m/s);
  - lethal from full health: about 6.8 m stock, so about 7.8 m new (a fall kills
    at the same D, about 0.6, whatever `fatal_damage` is).
- **Bots:** their map data takes a drop of at most `harmless_height` (5 m),
  unchanged.
- **Release:** the resources.db patch, made by `scripts/content-edit/content_edit.py`
  from `scripts/content-edit/character.tsv` (with `default.player` from the
  live server game data) and `release/build-patch`; the server game data built
  again with that patch (srp reads `default.player` for the carry limits, and the
  server data must be the one the players' patches make); and a `next` row of
  `crates/session/src/v026g/simulation_changes.tsv` (a match plays out
  otherwise), so recordings from earlier revisions are no longer watchable
  after the release, as for every simulation change.

## How speed counts

The owner (2026-10-20): "one problem, is that fall is dependant on speed and
speed can still somehow be additive to something in a very confusing ways".
Read from the 0.26g exe (no game run); the labels are in `tools/re-kit/v026g`.

- **Only the vertical speed counts (PROVEN).** The character controller
  (step_down 0x7e1b0) calls the landing callback with v = −(velocity · up) on
  the tick it touches ground, then zeroes the velocity;
  on_physical_controller_landing 0x2d8220 turns v into D. Horizontal speed
  never enters: the velocity (controller +0x2C770) gets only gravity, the
  jump's vertical motion and zero on landing; walking and sprinting move the
  character by the walk step, not by the velocity.
- **Heights and speeds agree (PROVEN):** the loader takes
  sqrt(2 · gravity · height) for both heights, and the same `gravity` 9.8 is the
  controller's own gravity (base_player::base_player 0x2d6990 passes it in).
- **One tick is 0.05 s (PROVEN):** gravity adds 0.49 m/s a tick; v is the speed
  at the start of the landing tick, so it comes in steps of 0.49 m/s and D in
  steps of about 0.12. While the fall is slower than 2 m/s (its first 5 ticks)
  the controller drops 0.05 m a tick. The frame rate plays no part (lockstep).
- **Walking or sprinting off, fatal_damage 1.0 (the tick model above):**

  | drop | v (m/s) | D |
  |---|---|---|
  | 4 m | 8.8 | 0 |
  | 5 m | 9.8 | 0 |
  | 5.5 m | 10.3 | 0.10 |
  | 6 m | 10.8 | 0.21 |
  | 7 m | 11.8 | 0.45 |
  | 8 m | 12.7 | 0.69 |
  | 9 m | 13.2 | 0.81 |
  | 10 m and more | 14.2 | 1.0 |

  Lethal from full health (D about 0.6) from about 8.0 m (7.8 m above is the
  smooth formula).
- **What adds to v:**
  - **a jump off a ledge (LIKELY):** in a jump (base_player::jump 0x2d7920)
    the height follows the jump animation, with no gravity, and a landing then
    never hurts (step_jump_down 0x7d0f0 has no callback). The jump ends when the
    animation does, not on the ground (jump_logic_state_landing::initialize
    0x3440e0 -> end_jump 0x7f050), and end_jump keeps the downward speed. Off a
    ledge the fall so starts with the animation's last speed v0: as a drop
    v0²/2g higher (2 m/s +0.2 m, 3 m/s +0.46 m, 4 m/s +0.8 m). 5 m: walking 0,
    jumping with v0 3 m/s 0.11. v0 is the animation's, not measured.
  - **steep slopes (LIKELY):** a fall onto ground too steep to stand on (up to
    about 78° for a straight fall) is a landing with the full vertical speed;
    the velocity keeps its part along the slope, the slide goes on
    (update_slide_velocity 0x7fea0), and walkable ground at its bottom is a
    second landing with the slide's vertical speed. A long steep slide can hurt
    at the bottom; a steep roof can hurt twice. The slide speeds are not
    computed here.
- **What does not add (PROVEN unless said):**
  - sprinting or running off: 4 m walking = 4 m sprinting, 8.8 m/s, 0;
  - movement speed modifiers (armour, backpack, battery carry, skills,
    boosters), stamina: they set the horizontal step only
    (set_physics_controller_walk_vector 0x2d9060; the controller drops its
    vertical part outside a jump). Not settled: whether they speed up the jump
    animation (v0 above);
  - crouching, crouch-landing: the handler reads only v and the modifier;
  - stairs and small drops: every landing on walkable ground zeroes the
    velocity, nothing carries over;
  - explosions, pushes: nothing else writes the velocity; the physics tick
    (0x757f1) zeroes it when two players overlap, so landing on a player's head
    ends the fall without a hit;
  - ladders, moving objects: not read.
- **0.27 and later:** 0.27d0 and 0.28d0 `fall_params`, `jump_params`,
  `physics_controller`: the same keys and values, `fatal_damage` 1 the only
  change. 0.27d0 exe: the same loader, handler (0x2df330: linear between the two
  speeds, modifier 30) and controller (−velocity · up): the formula did not
  change (PROVEN). 0.28d0 exe: the controller is rebuilt, the landing still
  passes −velocity · up and zeroes the velocity (0x5f4bc9); its handler not
  read.
- **Options:**
  - horizontal speed: nothing to do, it already does not count;
  - data only (resources.db patch, server data, `next` row): move
    `harmless_height` / `fatal_height` / `fatal_damage`; the table shifts, the
    quirks stay;
  - jump: end_jump sets the vertical speed to 0, not min(v, 0) (a few bytes at
    0x7f0a7). A jump off a ledge then counts from where the jump ended, like
    walking off. Risk: the fall restarts at 0.05 m a tick, a short hang after a
    jump;
  - steep slopes: no callback unless the ground is walkable (step_down
    0x7e660). Only the bottom of a slide counts. Risk: falls onto steep roofs
    hurt less;
  - by height: keep the highest point since the last landing in the controller
    and pass sqrt(2 · g · drop) instead of v. The plainest rule ("a fall of N
    metres does X"), the biggest change: a cave in step_down and new controller
    state in its serialize / deserialize (0x7f7f0 / 0x7fb10: late join,
    recordings), so the most desync risk;
  - every exe option is a client patch and a simulation change: a `next` row
    of `simulation_changes.tsv`, earlier recordings no longer watchable.

## Split ticks (patch 72)

The owner (2026-10-25): "the fall damage problem is back… it seems to be
something stacking up"; "you can now die from falling 1 meter"; "let's fix it".
Read from the 0.26g exe; client_patch.md section 3.56 has the addresses.

- **The cause (PROVEN):** the world steps to the time of each precise input
  inside a tick: a press or a release of fire, by any player, off the 50 ms
  grid (0xDD). Every game cuts the same ticks. Each part steps every moving
  character controller: the body moves its share of the tick, but the free
  fall took a whole tick of gravity (0.49 m/s) at every part. A tick in k
  parts so gave k ticks of speed, and the landing hurt as from a fall k times
  higher. More players clicking means more parts: a 10v10 fight of
  semi-automatic weapons can cut most ticks in several.
- **What it meant (the tick model, fatal_damage 1.0):**

  | drop | every tick whole | a quarter of ticks in 2 | every tick in 2 | every tick in 6 |
  |---|---|---|---|---|
  | 1 m | 0 | 0 | 0 | 0.23 |
  | 3 m | 0 | 0 | 0.14 | 1.0 |
  | 5 m | 0 | 0.26 | 0.88 | 1.0 |
  | 6 m | 0.21 | 0.51 | 1.0 | 1.0 |

  About 0.6 kills a player at full health; less kills one already hurt (the
  feet's blunt hit takes pain x 0.8).
- **Not the cause (PROVEN):** the history (the controller's velocity and
  flags are in each history item and restored on a re-simulation; a frame's
  step past the newest item is redone from it), real time (gravity's dt is
  the tick constant), revisions 32 and 33 (nothing of theirs touches the
  physics).
- **The rule (patch 72):** a fall gains speed for the time it fell. The free
  fall takes gravity x the step's dt (at most the tick), so a tick adds
  0.49 m/s however many parts it is stepped in. Whole ticks are bit for bit
  as before: the "Walking or sprinting off" table above holds again in every
  fight. A simulation change.
- **Unchanged:** how the world cuts its ticks (also the cost of a big fight's
  simulation: a separate decision), the landing handler, `fall_params`, the
  jump's end speed and the steep slopes above.
- **Follow-up, not done:** cap the landing speed at sqrt(2 · g · the height
  fallen since the last ground), the start kept in the controller. The
  controller has 12 bytes nothing writes (+2C784h..+2C78Fh of its 0x2C790);
  the start must be in its serialize (rollback), so the world's controller
  block grows from 67 to 71 bytes and srp's readers of the player blocks
  (`match_msgs/motion.rs`) change with it. Only if low falls still hurt with
  patch 72.

## A part of a tick moves a body at rest (patch 166)

The owner (2026-11-20, a 16-player match of revision 68): "there are lots of freezes there", "it was
happening for everyone". The frames did not stall and the match server saw no late game: players
stood still with their keys held, through bursts of fire. Read from the
0.26g exe; `client_patch.md` section 3.89 has the addresses.

- **The cause (PROVEN, stock):** the world steps to the time of each precise input inside a tick
  ("Split ticks" above), and `bullet_character_controller::updateAction` (0x7B730) ran `player_step`
  for a step shorter than a tick only when the body had moved in its step before (its position against
  `m_last_pos`, which `player_step` sets at its start). Either way it then zeroes the movement asked
  for the step (the desired walk and air control vectors). So in a tick stepped in parts a body at rest
  took nothing from its player: no walk, no strafe, no jump, no crouch or standing up, until a tick
  that was whole. A body already moving went on, until a part moved it by nothing (a wall, a stop,
  a part too short to walk); then it waited the same way. Every game computes the same, so nothing
  desynced and no input came late: everyone saw the same players stuck, themselves too.
- **How it showed:** with few precise inputs a tick in ten is cut and a start is a tick late at
  most. In a fight, or through a burst of precise inputs, few ticks are whole. The match's recording:
  through its bursts of precise inputs 2 % of the ticks were whole (91 % in the rest of the match),
  and in the worlds uploaded then 64 % of the walking players' samples (forward held, not crouched,
  not aiming) had no speed, against 2 % outside (sprinting: 16 % against 0.3 %).
- **`player_step` already does a part right (PROVEN):** it takes the step's dt, makes the tick's whole
  step from the walk asked for (walk / dt is the speed; x 0.05 the tick's move; under 3 m/s a longer
  probe scaled back) and moves the body dt / 0.05 of it (0x7C13B, 0x7C603, 0x7C734); and it returns at
  once for a body with nothing to do (`can_skeep_player_step` 0x7C7E0: no walk vector, no velocity,
  no slide, not jumping, no push). The game asks for a displacement of the step's own time, at most
  the player's speed x that time (`base_player::set_physics_controller_walk_vector` 0x2D9060). So a
  body moves a part's share in a part, and a tick's worth in a tick however it is cut. updateAction's
  shortcut for a part asked only whether the body had moved, not whether it was asked to.
- **The rule (patch 166):** every step runs `player_step`, a part as a whole tick (one branch,
  `STEP_PARTS` 0x7B76B). A body asked to move in a part of a tick moves by that part's share. A
  simulation change.
- **Nobody moves faster (checked, `check_step_parts_026g.py`, Unicorn, the game's own updateAction
  and player_step on open ground, 5 s each):** whole ticks are the original's bit for bit; a body
  already moving is the original's bit for bit however its ticks are cut; a body from rest, with one
  cut a tick, a burst's cuts or 50 cuts a second, is where whole ticks put it to a float's rounding
  (under 0.5 mm in 5 s walking at 3.74 m/s, sprinting at 5.28, slow at 1.4, strafing): the same
  distance, the same speed from the first tick. It is never ahead: a part asked for under 3.2 mm
  (`player_step`'s least walk, 1e-5 m2: 1 or 2 ms at 1.4 m/s) is not made, as in the original, so
  cuts can only lose distance (46 mm in 5 s, 0.7 %, at 1.4 m/s under a burst's cuts; the original
  stopped there until a whole tick). A body asked nothing is not stepped and not changed. Patch 168
  (below) makes those short parts too.
- **What else a cut tick skipped, and now runs:** all of it was `player_step`'s for a body at rest:
  the walk and the strafe, the jump's start, the crouch and the standing up (its capsule,
  `setup_crouch_state` at `player_step`'s start), the air control, and a slide that begins. Nothing
  else in the simulation tests a step's length against the tick: this compare (0x7B760) is the only
  one of a dt with the tick constant, and no code of the game's simulation or the physics branches on
  a step of exactly 50 ms.
- **Unchanged:** how the world cuts its ticks (and what a cut tick costs every game's simulation);
  `player_step` itself, so what a moving body did in a part before it does now: its steps up and
  down are the tick's, shared by dt, the slide's acceleration is a tick's at each part (steep slopes
  only), the landing is taken where a part lands.

## A part of a tick: its least walk and the speed read from it (patch 168)

Patch 166 runs every part of a tick. Two things a part still did otherwise than a whole tick, read
from the 0.26g exe and measured in a game (`client_patch.md` section 3.93 has the addresses):

- **The least walk (PROVEN):** `can_skeep_player_step` (0x7C7E0) skips a step whose walk asked is under
  3.16 mm (its square under 1e-5, 0x9CBA48). A part is asked its share of the tick's walk, so a part
  of 1 ms of a walk, or 1-2 ms of a slow walk, was skipped: the original then stood still until a whole
  tick (this is how moving players froze too), and with patch 166 the body lost that part's ground
  and its speed read 0 for the step.
- **The measured speed (PROVEN):** `base_player::assign_physics_transform` (0x2D93A0) stores
  m_linear_horizontal_speed (+334h) after every step: the step's horizontal move over its own ms
  (0x2D9545). A jump reads it as it begins (`player_logic_stand_jump_state::initialize` 0x337040 and
  `initialize_weight_for_on_site_animation` 0x337280: the moving jump's forward speed and its
  animation), and so does the anomalies' notice of a moving player (`notify_actions_subscribers`
  0x2D95B0). A walker's speed ramps by a step every tick and a step moves at the ramp's value at its
  end, so a part inside a tick reads more than the tick it is in: a jump begun in a cut tick flew
  faster than one begun in a whole tick (the original's own, through any cut; measured: 3.455 m/s in
  the air against 3.139-3.157 uncut, a jump begun at the end of a run-up's ramp).
- **The rule (patch 168, the Rust patch `tick_parts`, two hooks):** a part is asked, at the least walk's
  test, what its tick would be asked at its pace (the walk squared over (dt / 0.05) squared): every
  part of a walking body is made, and a body creeping under 63 mm/s is at rest in a part as in a tick.
  And only the step that ends a tick writes the measured speed (the world tells every tick delegate
  whether the step ends on a 50 ms boundary: `atomic_tick` 0x2D1E32; tick boundaries are multiples of
  50 ms, the world's base time being 50): a jump begun in a cut tick reads the speed of the tick
  before, as one begun in a whole tick does. Whole ticks are the original's bit for bit. A simulation
  change.
- **Checked (`check_step_parts_026g.py`, Unicorn, the game's own updateAction, player_step and
  can_skeep_player_step on open ground, 5 s each):** whole ticks bit for bit the original's; a body
  from rest under one cut a tick, a burst's cuts and denser cuts still is where whole ticks put it to
  under 0.4 mm in 5 s, walking, sprinting, slow and strafing, the slow walk under a burst's cuts
  included (it lost 46 mm with patch 166 alone, now +0.02 mm); with every tick in 50 parts of 1 ms
  (5000 steps) within 5 mm of whole ticks either way (float rounding, 0.1 %), where the original does
  not move at all; a body creeping at 50 mm/s stays at rest, whole or cut, and one at 80 mm/s walks
  its 0.4 m in 5 s through a burst's cuts. The speed store in 60 cases (five speeds, three values
  before, the byte 0 or 1 with garbage above it): written when the step ends a tick, kept in a part,
  the registers, the stack and the x87 stack as the original's.
- **In a game (revision 68 with patch 166, with and without 168, and the original; one headless game,
  a scripted walker, every step of the game's simulation traced, a world every tick; 32 s cycles,
  every tick of the walker's cycle cut in two of them, by one precise input in each tick or by a burst
  of them, the others uncut):**
  - No body faster: of 1308-1695 whole-tick and 5519-7434 part steps on the ground asked to walk in
    each run, none moved further than it was asked, in any build; the fastest part 4.402 m/s against
    4.401 for a whole tick (a sprint). The speed in the uploaded worlds tops each gait at its uncut
    value under every cut (walk 3.106, sprint 4.401, crouched 2.219, strafe 2.213 m/s).
  - The jump after a short run-up (a walker still speeding up): with 166 alone it flew at 3.476 m/s
    under cuts against 3.157 uncut (the original 3.457, patch 166 under a burst 3.520); with 168 at
    3.157 cut and uncut (its fastest air step 3.158 in a part, 3.157 in a whole tick), and the jump
    state read 2.588-2.589 m/s, cut or not, where with 166 alone it read a part's 2.59-3.02 under cuts
    (the original 2.948).
  - Full-speed jumps (1.5 s run-ups: walk, sprint, both strafes, standing, forward with jump from
    rest): the jump state read 3.106 (forward) and 2.213 (strafe) cut and uncut; heights 0.96-1.00 m
    against 0.99 uncut where the ground was the same; the fastest air step of a sprint jump 5.866 m/s
    in a whole tick and 5.89 in a part in every build, the original's too (the air sweep below).
  - Parts asked under the least walk and not made though their tick would walk: 98 (one input a
    tick) and 262 (bursts) with 166 alone, 131 on the walker's course; 4, 4 and 0-2 with 168, each
    a body against a wall, where its whole ticks move nothing either.
  - A standing jump stays where it stood (0 m) and a body asked nothing is not moved, cut or not.
  - The original under cuts through a whole cycle never starts to move (0 m).
  - Distances over a script part vary from cycle to cycle with where the walker begins it (walls,
    slopes), uncut as cut, so the per-step figures above are the measure.
- **Not changed (stated limits):** a jump pressed together with forward from rest rose 0.89 m in some
  cut cycles against 0.99 uncut, with 166 alone as with 168 (never higher). Timings inside a cut tick still differ from a whole tick's by under
  one tick: a key acts in the last part of its tick (inputs govern the step that ends at their time,
  0x2D22BC) and state changes go one per step (`vostok::ai::fsm::tick` 0x38CA20). Ramps are sampled at
  part ends, as in the original: a start covers up to about 8 cm less, a stop as much more. A jump's
  air step sweeps a whole tick's move and takes the part's share (`step_jump_forward` 0x7CB90), so
  near an obstacle a jump in parts can land elsewhere than one in whole ticks, as in the original.
