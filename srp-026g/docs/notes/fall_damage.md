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
