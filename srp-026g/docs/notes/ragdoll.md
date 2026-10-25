# Ragdoll on death (research, phase 1)

The owner (2026-10-25): "implement proper ragdoll for dying": a killed player falls as a
physical body instead of (or blended from) the canned death animation. This page is what
0.26g has for it, what lockstep allows, and the options. No patch yet. Tags as in the other
notes: **PROVEN** (read in the exe at the address given), **LIKELY**, **GUESS**. Retail
never shipped ragdolls (the owner), so later builds were not searched.

## Summary and recommendation

- **0.26g has no ragdoll, dormant or not.** Its Bullet build has rigid bodies, capsules and
  a constraint solver, but no joint class at all (no `btTypedConstraint` subclass, no
  `ragdoll` / `physics_shell` string). The physics world still has an `add(bt_constraint*)`
  slot that hands a constraint to Bullet, with nothing in the exe that makes one.
- **Each body part already has a collision shape:** `base_character_full.skinned_model/hit_targets`
  has 19 shapes (capsules, boxes, a sphere) on 16 bones of the one human skeleton. There
  are no joint limits anywhere in the data.
- **A corpse is outside the simulation's physics:** at a death the player's controller and
  hit model leave the Bullet world, and the hit model's bones stop following the animation.
  So a ragdoll that only changes the drawn bones touches nothing a match computes.
- **Recommended: option C3.** A ragdoll drawn on each client that the simulation never
  sees: a small position-based solver (XPBD, 11 to 13 capsule bodies) in the `.srpx`
  arena, as `no_std` Rust (the sync-core plan). It only reads the level from the existing
  Bullet world, with read-only sweeps against static groups, and adds no Bullet object.
  It runs once a frame on the logic thread after the simulation. In `player::mt_draw` it
  overwrites the bone matrices before the renderer gets them.
  It starts from the death animation's pose and speed after a short blend-in, so the fall
  begins as the canned clip and ends as physics. The local player's own body keeps the
  canned animation on the death screen (patch 46's camera rides its head), at least at first.
- **Cheap fallback, option D1:** keep the canned clip and only fit its last pose to the
  ground (one drop and tilt of the root). This fixes bodies floating over ledges and stairs,
  but it is not a ragdoll.

## 1. What 0.26g does on a death today

```
  sim tick (lockstep, every client)                    frame (logic thread, after the sim)
  ---------------------------------                    -----------------------------------
  damage -> player::on_player_death 0x1F28E0           game_world::tick 0x1F77E0
     |                                                    |-- game_world_core::move 0x2D2E40
     +-> base_player::remove_alive(p, 1) 0x2D99C0         |      (the simulation, replays too)
     |      +-> deactivate_physics 0x2D9980               |
     |      |     world.remove(hit body)  vt+38h          +-- for each place: player::mt_draw 0x2474A0
     |      |     controller.remove 0x7F520                    dead: tick anim player +750h
     |      +-> +33Ch := 0 (hit bones frozen)                   bones := [+128h] vt+44h (stack)
     |                                                          weapon at bone "Weapon" x +40Ch
     +-> dead state activate 0x338330                           update_skeleton 0x22490 (bones)
            index := random(count) (sim random)
            death[index] / death_hud[index] play on +750h
```

- **The death clip (PROVEN, death-poses.md):** `player_logic_dead_state::activate` 0x338330
  takes `index = (x * count) >> 32` from the weapon's selector random. The record of
  `death` / `death_hud` is at +590h / +A20h, the count at +594h. The pair at an index
  comes from 0x338280. 0x338540 builds the state's animation expression from it. The
  index (+30h, `m_animation_index`) and `m_is_ready_to_be_deactivated` (+2Bh, set by `on_animation_end` 0x3384E0 at the clip's end) are serialized
  (0x3383E0 / 0x338430): **the clip is the simulation's**. Since death-poses-1, rifles have
  `on_site_left` / `on_site_right` and pistols `on_site_left`.
- **What the corpse is drawn from (PROVEN, mt_draw 0x24751C..0x247543):** for a dead player
  (+33Ch clear) mt_draw ticks the animation player at +750h up to the player's time +324h
  (0x3917D0). It then gets the bone matrices into a stack array (`[+128h]` vtable+44h at
  0x2476AB, count in `[ebp-1Ch]`, array in `esi`/`[ebp-14h]`). It puts the held item at
  bone "Weapon" (0x1113A0 finds it by name) times +40Ch (0x247763), and gives the array
  to `scene_renderer::update_skeleton` 0x22490 at 0x24780A. The decal pose (patch 45's
  notes, 0x2476F8) already overwrites the array in place before that call; a ragdoll
  would do the same.
- **The thread (PROVEN by call order, 0x1F7C37 then 0x1F7DCE):** `game_world::tick` runs
  the simulation (`move`) and then mt_draw for each place, on the logic thread. mt_draw
  queues render commands. So code in mt_draw runs after the simulation, on the thread that
  steps the Bullet world, never during a step.
- **Death camera:** first person at the corpse's eyes, the head bone's transform at
  player+44Ch (patch 46, death-camera.md). The exe also keeps 0.10b's third-person death
  camera console values (`death_camera_yaw` / `_pitch` / `_distance`); the death screen
  does not use them.
- **Corpse lifetime (LIKELY):** the place's one player object is the corpse until its
  respawn re-spawns that object (respawn rule 0x316820 -> spawn 0x316DA0). That is 8 s by
  the mode's options, less with `respawn_booster`, and to the match's end in Team
  Deathmatch with no tickets. So there are at most 19 other corpses, one per place.

## 2. Ragdoll support in the engine

- **Bullet in 0.26g (PROVEN, RTTI):** `btDiscreteDynamicsWorld`,
  `btSequentialImpulseConstraintSolver`, `btRigidBody`, `btCapsuleShape`, `btBoxShape`,
  `btCompoundShape`, `btBvhTriangleMeshShape`, `btGhostObject`, `btSoftBodyWorldInfo` (the
  soft bodies themselves are gone). Vostok wrappers: `bt_dynamic_rigid_body`,
  `bt_static_rigid_body`, `bt_animated_rigid_body`, `bt_character_controller`.
- **No joints (PROVEN, by absence):** no RTTI for `btTypedConstraint` or any subclass (hinge,
  cone-twist, point-to-point, 6-dof, slider), and none for vostok's `bt_constraint`. No
  string `ragdoll`, `physics_shell`, `anim_joint`, `bodies`, `joints`.
- **The dormant slot (PROVEN, vtable 0x9E2EBC):** `bullet_physics_world` +24h = 0x75B00
  `add(bt_constraint*)` calls `btDiscreteDynamicsWorld::addConstraint(c+4, false)` (its
  vtable+30h). +30h = 0x75B20 `remove(bt_constraint*)` jumps to `removeConstraint` (+34h).
  +28h / +34h = 0x75AF0, a bare `ret 4`: the rope (soft body) add and remove, compiled out.
  The order is MSVC's for the 0.10b `world` overloads (`physics/world.h`). Nothing calls
  +24h. The door exists, but nothing makes a key for it.
- **0.10b's "physics shell" (vostok sources):** `collision/shell_creation_utils_inline.h`
  reads a `bodies[]` table with `anim_joint` and per-body `collision`. The Maya plug-in has
  body and joint nodes. In 0.10b only the editor's `object_solid_visual::activate_physics_shell`
  uses it, for ODE-era props. Nothing of it reaches 0.26g's data (no `shell` / `joints`
  resource; only bullet-casing paths match).
- **Bullet in the simulation (PROVEN):**
  - `bullet_physics_world::tick` 0x75550 steps `stepSimulation(ms * 0.001, 3, 1/60)` with
    the simulation's elapsed time (the first argument), after a zero step.
  - It then pushes apart overlapping character controllers (+104h array, count +FCh).
  - The world's state is in each history item (`physics_world_history_item::serialize`
    0x324410, from `game_state_history_item::serialize` 0x324D4C): it is hashed (CRC of
    the item, match.md section 2.3) and restored on a rollback.
  - **Anything put into this world is part of lockstep.**
- **Bone shapes (PROVEN, data):** `models/character/human/base/base_character_full.skinned_model/hit_targets`
  is the only `hit_targets` in the 0.26g db. Each target has `animation_bone`, `type`
  (0 sphere, 1 box, 2 cylinder, 3 capsule; vostok `primitive_type`), `position`,
  `rotation`, `scale` (capsule: radius, height; box: half extents) and `hit_param`. They
  are made into `btCompoundShape`s by `new_compound_shape_from_hit_targets_config` 0x71080
  and `new_bt_element_joint` 0x70F40, inside `collision::animated_object`:

  ```
  bone          type     scale (m)               bone           type     scale (m)
  RightUpLeg    capsule  r 0.102 h 0.276         Spin           box      0.12 0.17 0.164
  LeftUpLeg     capsule  r 0.102 h 0.276         Spin (back)    box      0.043 0.158 0.165
  RightLeg      capsule  r 0.086 h 0.382         Chest          box      0.113 0.161 0.187
  LeftLeg       capsule  r 0.086 h 0.382         Chest (back)   box      0.047 0.163 0.165
  RightFoot     capsule  r 0.059 h 0.146         Neck           cylinder 0.069 0.056 0.069
  LeftFoot      capsule  r 0.059 h 0.146         Head           sphere   r 0.1
  RightArm      capsule  r 0.065 h 0.256         Head (face)    box      0.045 0.072 0.07
  LeftArm       capsule  r 0.065 h 0.256         RightHand      box      0.05 0.06 0.042
  RightForeArm  capsule  r 0.052 h 0.256         LeftHand       box      0.052 0.061 0.043
  LeftForeArm   capsule  r 0.052 h 0.256
  ```

- **The skeleton (PROVEN, data):** `animations/skeletons/scavengers_01.skeleton`, 69 bones
  (base_character_full's `skeleton` names it):

  ```
  Root -+- Hip -+- Spin - Spin_1 - Chest -+- Neck - Head (Jaw, LeftEye, RightEye)
        |       |                         +- L/R Clavicle -+- L/R Shoulder
        |       |                                          +- L/R Arm (_Roll) - ForeArm (_Roll)
        |       |                                                 - Hand - 15 finger bones
        |       +- L/R UpLeg (_Roll) - Knee - Leg - Foot - Toe
        +- Weapon, Pistol, Item
  ```

  No shape for Hip (the pelvis) or the clavicles; Weapon / Pistol / Item hang from Root,
  not from the hand. LIKELY every player model uses this skeleton and these targets (the
  player cook formats `resources/models/%s.skinned_model/hit_targets`, and only
  base_character_full has one).

## 3. Determinism: what a ragdoll may touch

| Question | 0.26g | Evidence |
|---|---|---|
| Does a corpse block players? | No: its controller leaves the world at the death | `deactivate_physics` 0x2D9980 (controller remove 0x7F520), from `remove_alive(p, 1)` 0x2D99C0, from `on_player_death` 0x1F28E0. PROVEN |
| Does it stop bullets or grenades? | No: the hit model's body (`[+498h]+12Ch`, group 40h, mask 11h when alive) leaves the world | same call, world vtable+38h; `activate_physics` 0x2D9930 is the add. PROVEN, the field's meaning LIKELY |
| Do its hit shapes follow the pose? | No: `recompute_damage_collision_bones` 0x2DA5B0 returns 2 at once while +33Ch is clear, which remove_alive sets | PROVEN |
| Is the death clip in the simulation? | Yes: index and end flag serialized; the end callback runs on +750h, which mt_draw also ticks | 0x3383E0, 0x3384E0, 0x24752B. PROVEN |
| Battery and items | Dropped by the simulation's own states at the player's simulated transform, never at a bone | LIKELY (victory item states are serialized, battery_pickup.md); check in phase 2 |
| What is hashed | The history item's serialize (players, bullets, statistics, objects, physics world, rules, effects) | match.md section 2.3. PROVEN |

The rules for a ragdoll follow from this:

- **Visual only.** It is computed on each client from what that client draws and never
  written into anything serialized or hashed: not the player (+40Ch, +44Ch, +750h), not the
  Bullet world, not the hit model. Two clients may show a corpse in different poses, which
  is allowed, because a corpse collides with nothing in the game.
- **The original keeps running.** mt_draw's tick of +750h must stay: the dead state's
  `m_is_ready_to_be_deactivated` (+2Bh, serialized) is set from that animation's end. The ragdoll replaces only
  the matrices given to `update_skeleton`, as the decal pose does.
- **No Bullet object in the simulation's world**, not even one that collides with nothing:
  the world is stepped by every tick and replay. Its objects' order and islands are part of
  what each client computes alike, and a body added outside the history would survive a
  rollback that should have removed it.
- **Read-only queries of that world are allowed**, on the logic thread outside `move`:
  ray / convex sweeps against the static groups. They change no state (Bullet's ray stacks
  are scratch).
- **The battery and loot stay where the simulation puts them.** A ragdoll that slides far
  would part the body from its battery, so the root's travel is capped (GUESS: 1 m from the
  death position, to tune).
- **Rollback and resync:** every frame, after `move`, the ragdoll checks the player's state
  in the world as it now stands (section 5). A rollback or a loaded world that makes the
  player alive drops the ragdoll at once. A death re-simulated as the same death (same key)
  keeps it.

## 4. Options, ranked

```
  rank  option                                   determinism   cost        looks
  ----  ---------------------------------------  ------------  ----------  ------------------
   1    C3 own XPBD ragdoll, reads level only    safe          medium      proper ragdoll
   2    D1 ground-fit of the canned last pose    safe          small       canned, no floating
   3    D2 canned clip + limb IK to the ground   safe          small-med   canned, limbs settle
   4    C2 second Bullet world + own joints      safe          large       proper ragdoll
   5    C1 bodies in the simulation's world      UNSAFE        -           rejected
   -    A  dormant engine ragdoll                none exists   -           -
   -    B  port retail's                         none shipped  -           -
```

### C3 (recommended): a visual-only ragdoll of our own, reading the level from Bullet

- **What:** a position-based rigid-body solver (XPBD: positions and orientations, distance
  and swing/twist limits, a few substeps). It has 11 bodies: pelvis, torso (Spin to Chest),
  head (with Neck), and 2 each of thigh, shin, upper arm and forearm (hand fixed to the
  forearm). Feet and hands can be added as bodies later (13).
- **Level contacts:** each moving capsule is swept against the static world with
  `bullet_physics_world::object_query` 0x76920 (vtable+50h; a convex sweep with a group
  and a mask), or ray_test 0x75E20 for the capsule ends. The shapes come once from
  `create_primitive_shape` 0x7A390. No body is added to the world. Corpses do not
  collide with each other or with the living (no corpse does today).
- **Start (Dying, blended from the clip):** the canned clip plays for a short blend-in
  (GUESS: 0.25 s). Its pose and the speed between the last two frames' matrices give each
  body its start state, so the body begins to fall the way the clip falls (left / right).
  An impulse along the killing shot is a later step (where the game keeps the last hit's
  direction is unknown yet).
- **Drawn:** each frame the solved bodies give the driven bones' matrices. Every bone that
  is not driven (fingers, toes, rolls, Knee, Clavicle, Jaw, eyes) keeps its pose relative
  to its nearest driven ancestor, taken from the animation. The `Weapon` bone keeps its
  transform relative to RightHand from the death on, so the gun stays in the hand.
  Dropping the gun as a body of its own comes later.
- **Hooks (2):**
  - **step:** once a frame after `move` in `game_world::tick` (after 0x1F7C37, before
    the place loop 0x1F7D89). It runs the state machine for every place, steps the
    moving ragdolls with the frame's real time, and does the queries.
  - **apply:** in mt_draw after the bones are computed (after 0x2476AB, before 0x247716
    reads the "Weapon" bone). For a place in Ragdoll or Settled, it overwrites the
    array. The culled path (0x247811) skips the apply; the step still runs.
- **Where the code lives:** an `ARENA` row in `link_026g.py`, `no_std` Rust (branch
  `feat/sync-core`'s toolchain) or C. Floats in plain IEEE f32, `libm` for sqrt and
  trigonometry. The solver is a pure function `(state, frame) -> state` with no engine
  pointers, so it is unit-tested natively on Linux (falls, joint limits, settling,
  a rollback). The engine calls (`object_query`, `ray_test`, the bone lookup 0x1113A0)
  go through `engine_026g.tsv` rows.
- **Data:** the radii and lengths from `hit_targets` (above), read by the build into a table
  in the arena. The pelvis capsule is ours (between the UpLeg tops, r ~0.12). Masses from
  a standard human table (~80 kg). The joint limits are authored once for the one
  skeleton: neck and shoulders and hips as swing cones with twist ranges, elbows and knees
  as hinges, the waist a small cone. That is about 10 rows.
- **Cost (GUESS):** about 1,000 lines of Rust (the solver, the contacts, the state machine,
  the bone writer), 2 small NASM entries, Unicorn checks of the two sites. Phase-2
  reading (section 6).
- **Performance (GUESS):** the solver is microseconds per ragdoll. The cost is the
  queries: 11 sweeps per moving ragdoll per frame. A ragdoll settles in about 2 s and then
  costs nothing, so 20 corpses at once means at most 220 sweeps in the frames after a
  grenade, and a handful in steady play. A hard budget caps it (sweeps per frame,
  oldest-first), and a ragdoll over budget waits a frame: no frame hitch for a corpse.
- **Risks:**
  - The bone array's space (model or world) and layout are not read yet.
  - The model's culling box follows the player's transform, so the travel cap keeps the
    body inside it.
  - `object_query`'s callback and result layout (closest_ray_result) need reading.
  - The local player's own body: patch 46's camera at +44Ch would ride a tumbling head.
    So the local player keeps the clip at first (an owner question).

### D1: fit the canned last pose to the ground

- At the clip's end (the dead state's +2Bh), one ray down from the pelvis and two more
  (head, feet) give a height and a slope. The whole pose is moved down onto the ground
  and tilted to the slope, at most ~0.5 m and ~30 degrees, eased in over ~0.2 s.
- One hook (mt_draw apply) and 3 `ray_test`s per death. Same visual-only rules,
  hardly any cost. Fixes the floating corpse on ledges, stairs and slopes, but it is
  not a ragdoll.

### D2: canned clip with limbs settled by IK

- D1, plus two-bone IK for each arm and leg toward the ground under them (a ray each).
- Little more code than D1; the clip's character is kept, and limbs stop hanging in the air.
- Still the same 2-3 poses on every death.

### C2: a second, draw-only Bullet world with our own joints

- A second `bullet_physics_world` (ctor 0x74950, `initialize` 0x74B20), the level's static
  shapes added again as new collision objects (Bullet shapes can be shared, objects
  cannot), and corpse bodies via `create_dynamic_rigid_body` 0x357100.
- No joint class exists, so the joints would be our own `btTypedConstraint`-compatible
  objects (the solver's getInfo1 / getInfo2 virtual interface, Bullet 2.7x layouts matched
  byte for byte). That is about the size of Bullet's cone-twist joint, plus world setup
  per map and teardown.
- The same visual-only rules, but far more to get wrong than C3, and a second
  broadphase of the whole level in memory.

### C1: bodies in the simulation's world (rejected)

- Even with a group and mask that nothing collides with, the bodies are stepped in every
  tick and replay, take part in the world's object arrays and islands, and are not in the
  history: a rollback would keep a ragdoll the restored world never had.
- Making them part of the history would make the ragdoll lockstep. The world grows, its
  hash covers it, every client must compute it bit for bit, and older replays break.
  That is a large simulation change for something that only needs to look right.

## 5. The state machine (one per place)

```
                 sim: dead(K)                    blend-in done
     Alive ----------------------> Dying{K, t} -----------------> Ragdoll{K, bodies, t}
       ^                              |    ^                          |        |
       |                              |    | sim: dead(K2), K2 != K   |        | at rest 0.5 s,
       |                              |    +--------------------------+        | or t > 6 s
       |                              |    (also from Settled)                 v
       +------------------------------+-----------------------------------  Settled{K, pose}
             sim: alive (respawn, or a rollback / loaded world undid the death)
```

- `enum CorpseVisual { Alive, Dying { key, t }, Ragdoll { key, bodies, t }, Settled { key, pose } }`.
  Each state carries only its own data, there are no flags, and one function
  `(state, Frame) -> state` is total over the frame's two facts: the player in the
  current world (`Alive` or `Dead(key)`) and the frame's time.
- **The key K** names one death: the place and a counter that the world restores on a
  rollback. LIKELY the player's death count in the statistics (snapshot_rules.md
  section 3); the dead state's start time would do as well. Read once a frame into a
  typed value (parse, don't validate).
- **Local player:** the place of the local player in first person stays at `Alive`/`Dying`
  pass-through (the clip as today) unless the owner wants the camera on the ragdoll.
- **Replays** (replay_watch.md) go through the same frame function; a seek back is a
  rollback.

## 6. Open for phase 2 (read before building)

1. The bone array mt_draw gives `update_skeleton`: space (model or world), order (skeleton
   index), and how +40Ch places the model. One Unicorn run of mt_draw for a dead player.
2. `object_query` 0x76920: arguments (shape, from and to transforms, results vector,
   group, mask) and the result's fields. Which groups are the level's static geometry.
3. The death key: where the world keeps a player's deaths or the dead state's start time.
4. Where the battery is dropped at a death (to confirm the simulated transform).
5. The model's culling bounds for a corpse that moves.
6. Whether the last hit's direction is kept on the player, for the impulse.
7. Owner questions: (a) the local player's own body: canned clip (default) or a ragdoll
   with the camera on its head; (b) the gun stays in the hand or drops; (c) C3 now,
   or D1 first as a quick fix.

## Addresses (re-kit rows added in this change)

| VA | Name | Row |
|---|---|---|
| 0x3B480 | `vostok::collision::animated_object::update` (bones begin, end; `ret 8`) | inferred.tsv, abi |
| 0x338280 | `survarium::player_logic_dead_state::get_death_animations` (the `death` / `death_hud` pair at an index) | inferred.tsv |
| 0x338540 | `survarium::player_logic_dead_state::selected_animations` (the state's expression) | inferred.tsv |
| 0x2D77C0 | `survarium::base_player::on_player_death` (the core's twin of 0x1F28E0) | inferred.tsv |
| 0x75550 | `bullet_physics_world::tick`: arguments and step | abi |
| 0x75B00, 0x75B20 | `bullet_physics_world::add` / `remove` of a `bt_constraint` | abi |
| 0x2D9930, 0x2D9980 | `base_player::activate_physics` / `deactivate_physics` | abi |
| 0x2D99C0 | `base_player::remove_alive` | abi |
| 0x2DA5B0 | `base_player::recompute_damage_collision_bones` | abi |
| 0x1F28E0 | `player::on_player_death` | abi |

Already in the kit and used above: 0x338330 dead state activate, 0x3383E0 / 0x338430 /
0x3384E0 its serialize / deserialize / end, 0x2474A0 mt_draw, 0x1F77E0 game_world::tick,
0x22490 update_skeleton, 0x75E20 ray_test, 0x76920 object_query, 0x70F40, 0x71080,
0x714F0, 0x71600 (the hit model's makers), 0x324410 physics history item.
