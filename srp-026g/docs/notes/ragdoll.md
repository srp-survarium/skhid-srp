# Ragdoll on death

The owner (2026-10-25): "implement proper ragdoll for dying": a killed player falls as a
physical body instead of (or blended from) the canned death animation. Sections 1 to 6 are
phase 1: what 0.26g has for it, what lockstep allows, and the options. Section 6 now holds
the phase-2 findings, sections 7 to 10 the build (patch 76). Tags as in the other notes:
**PROVEN** (read in the exe at the address given), **LIKELY**, **GUESS**. Retail never
shipped ragdolls (the owner), so later builds were not searched.

**The owner's decisions (2026-10-25):** option C3, the full ragdoll, no ground-fit step
first; the local player's own body ragdolls too, with patch 46's camera smoothed on its
head; the gun drops as a body of its own; the fall starts from the canned clip by a drive
whose strength fades (the Bodycam study's recipe, section 7). All Rust client code stacks on
the Rust framework PR (`feat/rust-framework`): the solver here is a pure crate; the game-side
wiring (section 8) is rewired through the framework's hook template and bindings.

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
  begins as the canned clip and ends as physics. (Phase 1 kept the local player's own body
  canned; the owner chose a ragdoll for it too, the camera smoothed on its eye.)
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
- **Death camera:** first person at the corpse's eyes: player+44Ch (patch 46,
  death-camera.md). Phase 2: +44Ch is bone **"Item"** times +40Ch, written by the
  simulation (`computed_head_transform` 0x2DA4B0), not the head bone and not mt_draw
  (section 6). The exe also keeps 0.10b's third-person death camera console values
  (`death_camera_yaw` / `_pitch` / `_distance`); the death screen does not use them.
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
| Battery and items | Dropped by the simulation's own states at the simulation's "Weapon" bone (its own animation player +750h, not the drawn bones) | PROVEN in phase 2 (section 6, item 4) |
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
- **The key K** names one death: the place and a value the world restores on a rollback.
  Phase 2: the life's spawn time, player+328h (`m_inserted_time_in_ms`, serialized), since
  a life has one death (section 6, item 3). Read once a frame into a typed value (parse,
  don't validate).
- **Local player:** ragdolls too (the owner); the camera follows its ragdoll's eye
  (section 8).
- **Built** (section 7): the place's state is `Alive`, `Body { death, slot }` or
  `Canned { death }`; a slot of the pool of 20 is `Free`, `Dying { clip_t, captured }`,
  `Ragdoll { ragdoll, clip }` or `Settled { rig, parts, gun, head_ground, since }`. The
  owner's four states are the place and its slot together; each fact lives once.
- **Replays** (replay_watch.md) go through the same frame function; a seek back is a
  rollback.

## 6. Phase 2: what was read (2026-10-25)

Every finding is in the re-kit (`tools/re-kit/v026g`, the rows of this change).

1. **The bone array (PROVEN).** mt_draw's array (0x2476AB: `[+128h]` vt+44h, the item's
   `update_bones_matrices` 0x2FF600) holds vostok `float4x4`s: rows i, j, k (the bone's
   axes) and c (its place), row vectors (v' = v M), 64 bytes each. Each is the bone's
   **absolute pose in model space** (`convert_to_object_matrices` 0x3B2C90 walks the tree
   from the identity). World = M_bone x M(+40Ch), as mt_draw does for the hand at 0x247763.
   Order: the cooked skeleton's (`add_bone` 0x3ACDA0: a bone's children together, in the
   file's order), less the root `object_movement`: array index = skeleton index - 1. The
   skeleton is `[[player+F290h]+10Ch]`: +108h the count (70), +110h the bones, 1Ch each
   (+0 name, +4 parent bone, +8/+0Ch children). So names and parents are read at run time.
   The array is an alloca of mt_draw (base [ebp-14h], count [ebp-1Ch]); update_skeleton
   0x22490 copies it into its command (at most 80 bones), so an overwrite must come before
   0x24780A. The clip's root motion is not in the bones: for a dead player the simulation
   moves +40Ch by it (`assign_physics_transform` 0x2D93A0).
2. **The level's queries (PROVEN).** `ray_test` 0x75E20 (ECX the world, then the result
   28h, origin, direction (not normalized: the end is origin + direction x length),
   length, group, mask, delegate, unflipped; `ret 20h`). The result: +0 the point, +0Ch the
   fraction, +10h the normal (toward the ray's start when the last argument is 0), +1Ch the
   object (0: no hit; then the rest is not written). **The level alone: group 24h, mask 2**
   (the foot IK's filter: walkers' geometry, no players, hit models, grenades or items).
   `object_query` 0x76920 is a convex sweep but **not usable**: its callback keeps every
   hit (no closest), its vector grows through an allocator, and a hit on a bare convex
   object dereferences a null shape info (0x77403). A sweep would call
   `btCollisionWorld::convexSweepTest` 0xA36E0 with a stack `ClosestConvexResultCallback`
   (vtable 0x9E2FA8), as the foot IK does (0x778B0); the ragdoll uses rays (a few
   microseconds each, a sweep 10 to 100). The physics world is player+350h; queries run on
   the logic thread after `move` and change nothing but Bullet's scratch stacks.
3. **The death and its key (PROVEN).** Dead in the current world: `+33Fh`
   (`m_has_been_inserted`) set and `+33Ch` (`m_is_alive`) clear; +33Ch is serialized and
   restored by a rollback (0x2DAFA1, 0x2DB4E8). The key: the life's spawn time **+328h**
   (`m_inserted_time_in_ms`, set at the spawn 0x2DAC43, serialized 0x2DB2FE): one death a
   life, the same when the death is simulated again. No death time is kept anywhere (the
   dead state serializes +2Bh and +30h only). A rollback that turns death A into death B in
   the same life keeps the key; the kd rule's deaths (`kd_rule+112h+place*8`) would tell
   them apart if that ever matters.
4. **The battery (PROVEN).** Carried, its transform is the simulation's bone "Weapon" x
   its offset x the carrier's +40Ch (0x344E40, every tick); at the death it drops there
   (0x300E50, 0x300900) as a body in the simulation's world. The drawn bones never touch it.
5. **The cull (PROVEN).** The renderer's box follows the bone origins by itself (0x15A590
   grows the instance's box by every bone's place), so a moving ragdoll needs no box.
   mt_draw's own cull is the problem: a player not seen the last frame (+0F4E2h) and
   holding an item skips the bones (0x24757C..0x24758C). The ragdoll's cull hook draws a
   corpse of ours every frame (section 8).
6. **The last hit (PROVEN): not kept.** The player keeps no hit point, direction or bone;
   the kill event has no position. Only hit_impl has them, at 0x2D9EFC: `[ebp+0Ch]` the
   hit_info (+0 the part's name, +24h the world hit point, +44h the killer's place),
   `[ebp-4]` the bullet (+1Ch its velocity, LIKELY). A capture hook there (writing only the
   arena, keyed by place and +328h; a re-simulated death writes the same) would give the
   impulse and the headshot's fade; not built (section 10).
7. **Threads (PROVEN).** With the console bool `mt_player_draw` (0x1F1D9D2) set,
   game_world::tick runs mt_draw as a task per place (0x1FB610, 0xFE810) and waits for all
   of them (0x10D4E0) before going on. So the draws may run at once on several threads,
   after the step: each touches only its own slot.
8. **The eye (PROVEN).** update_camera gives the view's update (0x2422B0, at 0x1F364B,
   patch 46's death_view in our exe) EAX = &player+44Ch in the first person (0x1F356E,
   0x1F35A5); both ways join at 0x1F3643. +44Ch is bone "Item" x +40Ch, recomputed by the
   simulation (`computed_head_transform` 0x2DA4B0) and by a deserialize; it must not be
   written. The ragdoll gives the view a matrix of its own instead.

## 7. The solver (`srp-026g/crates/ragdoll`, `no_std`)

A pure crate: no engine pointer, no allocation, unit-tested on the host. The numbers are
the Bodycam study's recipe (the owner, 2026-10-25), proposals to tune in the game.

- **Bodies (11):** pelvis (between the UpLegs), torso (Spin to Neck), head (Neck to Head
  and beyond), thighs (UpLeg to Leg), shins (Leg to Foot), upper arms (Arm to ForeArm),
  forearms (ForeArm to Hand and beyond). Capsules along each segment of the death pose,
  the radii from `hit_targets`. Masses (the recipe's, given for 80 kg; they sum to
  69.1 kg): head and neck 6.5, torso 17, pelvis 12, upper arm 2.2, forearm and hand 1.8,
  thigh 8, shin and foot 4.8. A thin limb's spin about its own axis is boosted to a quarter
  of the other two (the usual stabilizer).
- **Joints (10) and limits** (compliance 1e-6): neck cone 45 degrees, twist 50; waist cone
  35, twist 25; hips 100 forward, 30 back, 45 sideways (an ellipse), twist 30; knees a
  hinge -3..140; shoulders cone 110, twist 70; elbows a hinge 0..145. Limits are vectors
  in the two bodies' frames made from the death's pose (up; forward from the toes; left
  from the legs), never stored angles. Bodies that share no joint collide (capsule against
  capsule), except pairs that overlap at the death (an arm across the chest).
- **The start:** the bodies take the motion the player had: the last two drawn skeletons'
  difference over the frame's time, at most 8 m/s and 20 rad/s. A slot captures the
  game's skeleton at two draws (`Dying`) before its ragdoll starts. Every body starts on
  the plane the player stood on (its transform's place, up).
- **The drive:** each joint turned toward its target, a fraction 1 - exp(-h 40 s) of the
  way a substep, with the relative spin damped by s x 0.5 (at least 2.5 %, the joints'
  friction). The strength s fades per group: s0 (1 - smoothstep(t0, t0 + T, t)), floored at
  0.03. No world pin: only joint angles. The target is the death clip's relative pose (the
  game keeps ticking +750h, so the clip is drawn every frame), except a body shot's legs.
- **A body shot stands on its legs, then folds** (the owner, 2026-10-25: "similar to
  Bodycam, where the model tries to still stand on its legs when it drops"; the numbers in
  `crates/ragdoll/src/tuning.rs`): the legs hold at full strength for `LEG_HOLD` 0.25 s,
  then fade over `LEG_FADE` 0.8 s; the waist and neck fade over `TRUNK_FADE` 0.4 s, the
  arms over `ARM_FADE` 0.35 s, so the upper body slumps while the legs still carry it. The
  legs drive toward a supporting stance, not the clip: the knees bent `KNEE_BEND` 15
  degrees, each thigh toward its foot's place, which is under its hip moved
  `BALANCE_ASSIST` 0.8 of the way toward under the centre of mass (feet `STANCE_HALF_WIDTH`
  0.1 m apart): a balance assist that fades with the legs. No stagger step. After the legs'
  fade their weak floor follows the clip as every other joint's. At most 0.05 rad a
  substep of stance turn, so a hard landing does not throw the legs back up. A headshot
  fades everything in 0.06 s to 0 (limp at once); a blast starts at 0.4 and fades in
  0.15 s (weak tone, thrown).
- **The impulse** (when the cause is known): 2 m/s on the hit body (3.5 heavy), 0.4 on every
  body along the shot; a headshot 2.75 m/s on the head; a blast 3 to 6 m/s away and 1.5 up.
  0.26g keeps no last hit (section 6, item 6): the cause is `Unknown` until a capture hook.
- **A frame:** dt capped at 1/30 s, 6 substeps of one iteration each: integrate (g 9.81,
  damping 0.05/s linear, 0.3/s angular), joints, drive, limits, collision between bodies,
  the level and friction (static 0.9, kinetic 0.7, no bounce), the velocities. A body in
  the level already leaves it by at most 6 mm a substep (pushed out at once, it is thrown).
- **The level:** one query per moving body a frame, from its leading end along its motion
  with the ground below it added (v dt plus the radius and 5 cm): the plane found is kept
  for the substeps and the frames after while the body rests. A fast body (1.5 m/s at its
  ends) asks from both ends; a body whose leading end finds nothing asks from the other,
  and loses its plane only when neither has ground (a body half over a ledge stays on it).
  The game's budget is 160 rays a frame for all ragdolls; a body not asked keeps its plane.
- **Settled:** no capsule end moved 1.5 cm in 0.4 s, or every body slower than 0.05 m/s
  (the solver's creep between a limb, the ground and a leg); or every capsule end slower
  than 0.35 m/s for a second (a limb that still drifts, two constraints fighting with one
  iteration a substep); at the latest after 5 s. Near
  rest on the ground (every end below 1 m/s) the bodies are slowed at 8/s. A step that goes
  non-finite falls back to the last finite poses and settles. The pelvis stays within
  2.5 m (horizontally) of where the player died.
- **Drawn:** a body's bone is the body times its offset at the death; every other bone keeps
  its place on its parent from the death (fingers, rolls, clavicles, toes); the root stays.
  The gun drops: a capsule (0.35 m half length, 0.04 radius, 3.5 kg) along the "Weapon"
  bone's axis nearest the body's forward, with the hand's motion; the "Weapon" bone
  follows it, so the game draws the held item where the gun lies. Its own bounds are not
  read yet (a rifle's size for every item).
- **The camera:** the eye ("Item") kept relative to the head at the death; followed by
  critically damped springs (12 rad/s on its place, 9 on its forward), its up level (no
  roll), its pitch within 55 degrees, 22 cm above the ground the head lies on at least.
  Patch 46's drag still turns it.
- **The pool:** 20 slots. A new death with none free takes the oldest settled corpse's
  slot (that corpse is drawn as the game draws it again); with none settled, the death
  keeps the game's clip (`Canned`).

## 8. The game side (patch 76; to be rewired through the Rust framework)

Four hooks, each a `call` over one instruction that the entry does itself (built first on
#452's Rust tooling, checked in Unicorn). They move onto the Rust framework (#475) once it
lands: one arena crate `patches` with a module per patch and generated hook entries, the
solver staying its own library crate `ragdoll` that a `patches::ragdoll` module uses; the
hand-written entries in `ragdoll_026g.asm` and the crate `ragdoll_arena` then go.

```
  game_world::tick                       player::mt_draw (one a place, maybe on threads)
  ----------------                       ----------------------------------------------
  move (0x1F7C37)                        0x24757C cull: a corpse of ours is drawn
  0x1F7C3C step: every place's corpse    0x2476AB the game's bones (model space)
     - read: alive / dead(+328h)         0x247716 apply: the clip captured;
     - the ragdolls stepped, ray_test             the ragdoll's bones written over
     - the draws' table, the eye                  the array (world <-> model by +40Ch)
  place loop: mt_draw ...                0x24780A update_skeleton (copies the array)
                                         0x247847 update_camera:
                                            0x1F3643 eye: the local player's ragdoll's
                                                     eye instead of &player+44Ch
```

- **The step** reads the places through the network client's own vtable (0x9FFB14:
  match_options +34h, the count at +577B5h; get_active_player +50h, its reference given
  back), each player's +33Fh, +33Ch and +328h, its model's skeleton (matched once by name
  and parent: another skeleton gets no ragdoll), +40Ch, +350h and the held item +128h; and
  the frame's real time (QueryPerformanceCounter, IAT 0x870158). It writes only its own
  state: the corpses, the draws' table (player, key, slot, bone indices) and the eye.
- **The apply** checks the player is still dead in the same life, reads the 69 matrices (a
  matrix that is not rigid leaves the skeleton to the game), and writes the ragdoll's
  bones back as model-space matrices. **The eye** answers only for the local player's own
  corpse with a ragdoll, and only when it was given &player+44Ch.
- Nothing serialized, hashed or kept by the simulation is written: not +40Ch, +44Ch,
  +750h, the Bullet world or the hit model.

## 9. Tests and costs

- `cargo test -p ragdoll` (19): the rig draws the death pose back; a body shot holds its
  pelvis up for half a second, its knees give way from 0.6 s and it is down by about 1 s,
  a headshot is down by 0.5 s; shot, blast and headshot bodies settle 0.8 to 2.0 s after
  landing (1.8 to 3.2 s after the death); limits held (8 degrees of give) through a
  3 m fall against a wall; the drive follows the clip and fades; no NaN on degenerate
  input (every bone at one point, NaN or infinite bones, NaN dt, a huge dt); queries
  bounded; the corpse machine: a death, a respawn, a rollback that undoes the death, the
  same death again, another death, the pool's oldest settled slot given up, the camera
  following and stopping.
- `check_ragdoll_026g.py` (Unicorn, the built exe from the four sites, the game's own
  network client code): the clip twice then the ragdoll; the same death simulated again
  keeps the moving ragdoll; a rollback that undoes the death gives the game's bones, eye
  and cull at once; a skeleton not ours gets nothing; the eye level and the local
  player's only; the body falls onto the floor, through ray_test with the level's filter.
- **Cost** (`cargo test --release -p ragdoll cost -- --ignored --nocapture`, x86-64 host,
  20 places shot in the same frame, each its own direction, the clip falling with the
  body): the step 0.63 ms a frame on average and 1.3 ms at worst while they move (about
  50 microseconds a moving ragdoll), the 20 draws 0.03 ms; up to 430 rays a frame
  unbudgeted (the game caps 160; at 2 to 10 microseconds a ray, at most 0.3 to 1.6 ms);
  all 20 still moving at 1.5 s (standing on their legs for the first second), all
  settled by 3.4 s. One ragdoll: about 50
  microseconds and 12 to 24 rays a frame while it moves; a settled one costs nothing.
  The game's code is i686 SSE2 (32-bit): expect it somewhat slower than this host's.

## What the owner should see (the test round)

- **Shot in the body:** the player does not drop like a sack. For a moment (about a
  quarter of a second) the legs keep standing while the head, chest and arms sag; then the
  knees buckle under the weight and fold, and the body goes down within about a second,
  falling the way it was leaning (and the way the canned clip fell). The gun falls out of
  the hands on its own and lands beside the body.
- **Shot in the head:** limp at once: the body crumples straight down, no standing.
- **On a slope, stairs, a ledge or against a wall:** the body follows the ground: it rolls
  or slides a little and comes to rest on it, never floating above it or sinking in; a
  body half over a ledge stays on it, one fully past it falls off.
- **Settled:** within about 2 s of hitting the ground the body stops moving and stays as
  it lies; it is drawn even just off screen, so turning toward it shows it where it lies.
- **Your own death:** the camera rides your own body's head as it falls, smoothly, never
  rolling, never looking straight up or down, never below the ground; dragging still
  turns the view (patch 46).
- **A respawn, or a rollback that takes the death back:** the ragdoll is gone at once.
- **What may look off (tell me):** a corpse that crawls or twitches on the ground (the
  drive's floor), legs that stand too long or too short (`LEG_HOLD`, `LEG_FADE`), an upper
  body that slumps too early or too late (`TRUNK_FADE`), feet that slide oddly while the
  body sags (`BALANCE_ASSIST`), a pistol that drops as long as a rifle (the gun's size is a
  rifle's for now).

## 10. Left, and the tuning to watch

- **Rewire** the game side through `feat/rust-framework` (its hook template, bindings and
  test harness), then a local test build for the owner. Not run in a game yet.
- **The last hit:** a capture hook in hit_impl (0x2D9EFC) for the impulse and the
  headshot's fade (section 6, item 6). Until then every death fades as a body shot.
- **The gun's bounds:** read the held item's model box for the dropped capsule.
- **The drive's floor** (0.03): a clip whose end pose disagrees with the ground (a slope,
  the body landed the other way) keeps pulling; the 5 s cap ends it. Lower it to 0 if
  corpses crawl.

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
