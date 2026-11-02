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

**The owner's asks of 2026-10-26** (section 11): bullets that cross a corpse push it ("bodies
continue on being physics objects so that bullets even in ragdoll state affected them"), and
show the game's body-hit effect there ("bullet hits should also be seen on ragdoll bodies");
bodies and batteries interact ("batteries and bodies should interact too": one way, a body
rests on a battery, the battery is never moved).

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
| Does it stop bullets or grenades? | No: the hit model's body (`[+498h]+12Ch`, group 40h, mask 11h when alive) leaves the world; a bullet's ray (group 10h, mask 48h) goes through a corpse as through air, and patch 76 keeps that (section 11) | same call, world vtable+38h; `activate_physics` 0x2D9930 is the add. PROVEN, the field's meaning LIKELY |
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
   (the foot IK's filter: walkers' geometry, no players, hit models, grenades or items;
   section 17 adds the collision bullets hit, group 10h, mask 8).
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
- **The muscles (464c, section 12; 464b's timed stance is gone):** every joint has a muscle
  that pulls toward the death clip's relative pose (the pose at the death while no clip is
  drawn) as a compliant constraint: stiffness x strength, the strength s0 exp(-t / decay),
  nothing below 1 %. Legs 500 N m/rad, decay 0.5 s; waist 250, 0.25 s; neck 60, 0.25 s;
  arms 40, 0.2 s. The hit side's decay is 0.55 times shorter, a strong hit (60 N s) takes
  40 % of every muscle at once. A weakening knee's target bends toward flexion by up to
  1.4 rad as its strength goes, so the legs give way by themselves, more and more. A
  headshot: strength 0.2, decay 0.07 s; a blast: 0.4, 0.15 s. The relative spin is damped by
  s x 0.5 (at least 2.5 %, the joints' friction). A corpse a shot wakes is limp (no muscle).
- **The impulse:** the killing bullet (and those within 0.15 s before it, at half) on the part
  nearest its point: 3 times its momentum push (section 12), and 0.6 of it on the whole body,
  level, at most 1.2 m/s; each part's speed change at most 6 m/s, its spin's 15 rad/s. The
  killing hit's part names the cause (the head: a headshot). With no bullet known (a fall, a
  blast): the motion the player had, no push.
- **A frame:** the frame's time (capped at 1/30 s) is stepped in fixed substeps of 1/360 s,
  the rest carried to the next frame (464c: the answer does not depend on the frame rate),
  one iteration each: integrate (g 9.81, damping 0.05/s linear, 0.3/s angular), joints,
  muscles, limits, collision between bodies, the level and friction (static 0.9, kinetic
  0.7, no bounce), the velocities. A depth a body or two bodies already had (a plane found
  late, two limbs wedged) goes out by at most 6 mm a substep and makes no speed: the
  substep's start moves with it.
- **The level:** each capsule end has its own plane (464c). One query per moving end a
  frame (over 0.08 m/s; an end at rest keeps its plane), along its motion with the ground
  below it added (v dt plus the radius and 5 cm). An end that finds nothing while the other
  end lies on a plane, at that plane's height or above, takes that plane (a capsule across a
  battery's or a ledge's edge stays on it); below it, it hangs past a step and is free. The
  game's budget is 160 rays a frame for all ragdolls; an end not asked keeps its plane.
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

## 8. The game side (patch 76, the Rust patch `ragdoll`)

The Rust exe-patch framework's (`rust_patches.md`): the module `patches::ragdoll`
(`client-patches/rust/patches/src/ragdoll.rs`, `#![forbid(unsafe_code)]`, vostok only) over the
solver crate `ragdoll` (section 7, its own library crate, a path dependency of `patches`). Its
six hooks (five since the port, the sixth 464f's) are names only (`ragdoll.toml`: a site and a handler each); the sites are rows of
`sites_026g.tsv`, all kind `lohook` (each span 5 bytes or more and branch-free, so the
generated entry saves the machine, calls the handler and replays the span, or skips it):

| site | VA, span | handler | registers (vostok) | flow |
|---|---|---|---|---|
| `game_world.after_move` | 0x1F7C3C, 5 | `on_step` | EBX the game world | replay (the compare) |
| `player_draw.cull` | 0x24757C, 7 | `on_cull` | EBX the player | a corpse of ours: ZF set, skip (the `je` draws); else replay |
| `player_draw.bones` | 0x247716, 6 | `on_bones` | EBX the player, EBP mt_draw's frame | replay |
| `camera.eye` | 0x1F3643, 6 | `on_eye` | EBX the player, EAX the eye given | EAX our eye for our corpse, replay |
| `bullet.ray_traced` | 0x32D627, 10 | `on_ray` | EDI the bullet, EAX its result, ESP its frame | replay (the two moves) |
| `player_draw.held_item` | 0x24754F, 6 | `on_held_item` | EBX the player | a corpse of ours holding nothing: its bones to update_skeleton, exit `no_bones` 0x24781E (464f, section 14); else replay |

What the game is, is vostok's: `vostok::world` (the game world, the network client and its
places, a player's fields, its skeleton's bones, its inventory's weights, the physics world's
ray), `vostok::draw` (mt_draw's cull and bones), `vostok::camera` (the eye, kept in a matrix of
vostok's), `vostok::bullet` (a bullet's ray, its round, the body-hit effects), `vostok::clock`
(QueryPerformanceCounter), `vostok::trace` (the opt-in file). Every field is a row of
`fields_026g.tsv` (an `::image` row for bt_animated_rigid_body's vtable; the frame locals by
EBP and ESP as `mt_draw_frame` and `check_collision_frame`), every function one of
`engine_026g.tsv` (ray_test, the object release, the material pair, play_particle as thunks;
play_sound stdcall, match_options and get_active_player thiscall, called directly: the
network client's vtable 0x9FFB14 is the only one that names them, so the step calls them as
the vtable would; QueryPerformanceCounter/-Frequency and GetCurrentThreadId as imports).
Which register a site holds is said once, in vostok's view of that site (the `what` of its
row says the same).

The state (the corpses' pool, the draws' table, the shots; about 150 KB, the block
`patches.data`) is one `vostok::Lock` (a spin lock: the draws run one a place on mt_draw's
threads and the bullets 256 a task, so `GameCell`'s one thread does not hold; a lock taken
again on its own thread panics). The step, a draw, the eye and a bullet's trace each hold it
for their own work; the bullets' check of "any body" is an atomic, so a frame with no ragdoll
takes no lock.

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
  inside move: bullet::check_collision (each bullet's ray, maybe on worker threads)
     0x32D627 bullet: the segment against the ragdolls (read only); a hit's push
              queued for the next step (once a shot), the body-hit particle and sound
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

- `cargo test -p ragdoll` (19 of these; the shots' 7 more in section 11): the rig draws the death pose back; a body shot holds its
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

- **Shot in the body (464c):** the body is knocked along the bullet: shot from the front it
  goes back, from the side sideways, harder for a sniper's round than a pistol's, less in
  heavy armour. The legs keep working as they weaken: the knees give way from the first
  moment and more and more, the body down within about a second, falling the way the shot
  pushed it. The gun falls out of the hands and lands beside the body.
- **Shot in the head:** the head snaps back, the body crumples at once.
- **On a slope, stairs, a ledge or against a wall:** the body follows the ground: it rolls
  or slides a little and comes to rest on it, never floating above it or sinking in; a
  body half over a ledge stays on it, one fully past it falls off.
- **Settled:** within about 2 s of hitting the ground the body stops moving and stays as
  it lies; it is drawn even just off screen, so turning toward it shows it where it lies.
- **Your own death:** the camera rides your own body's head as it falls, smoothly, never
  rolling, never looking straight up or down, never below the ground; dragging still
  turns the view (patch 46).
- **A respawn, or a rollback that takes the death back:** the ragdoll is gone at once.
- **What may look off (tell me):** a body that shakes on the ground (464c's fixes, section
  12), a knock-back too strong or too weak (`tuning::DEATH_KNOCK`, `BODY_SHARE`), legs that
  give way too early or too late (`LEG_DECAY`, `KNEE_GIVE`), knees left raised on a body on
  its back, a pistol that drops as long as a rifle (the gun's size is a rifle's for now).
- **Shooting a corpse, a body on a battery:** section 11's list.

## 10. Left, and the tuning to watch

- **Rewire** the game side through `feat/rust-framework` (its hook template, bindings and
  test harness), then a local test build for the owner. Not run in a game yet.
- **The last hit:** done in 464c by the bullet hook (section 12), not hit_impl.
- **The gun's bounds:** read the held item's model box for the dropped capsule.
- **The muscles' floor:** gone in 464c (they decay to nothing).
- **Explosions** push no corpse yet (section 11).

## 11. Bullets through a corpse, the hit effect, batteries (2026-10-26)

**What stock 0.26g does at a corpse (PROVEN).** A bullet is a simulation object
(`bullet_manager`, serialized and hashed); each tick `bullet::tick` (0x32C180) calls
`check_collision` (0x32D4E0) with the segment from its last place to its place at the tick's
end. That traces one ray through the physics world (vtable+48h = `ray_test` 0x75E20, group
10h, mask 48h, the bullet as the hit models' delegate argument) and, on a hit, pierces,
reflects or stops (try_pierce 0x32D960, try_reflect 0x32DE40, the process_* effects). A
corpse's hit model has left the world at the death (section 3), so the ray goes through the
corpse as through air: no hit, no effect, the bullet flies on. Patch 76 keeps exactly that:
it only reads the ray and its result.

**The hook (patch 76's fifth).** `check_collision` 0x32D627, right after the ray_test, over
the two moves that copy its result (`movq xmm0,[eax]` / `movq [esp+38h],xmm0`, 10 bytes):
a jmp to the generated lohook entry + 5 nops, the entry replaying both moves. There EAX is the ray's result (+0 the
point, +1Ch the object), EDI the bullet, and check_collision's frame holds the ray: +90h the
origin, +20h the unit direction, +14h the length. It is the one place where each bullet's
segment of each tick is known, on every client (the same simulation), and it runs again for
every segment after a pierce or reflection (the loop at 0x32D7D1 too). With 256 bullets or
more, `bullet_manager::tick` (0x321D10) ticks them as tasks of 256 on several threads and
waits for them; the hook is thread safe (below).

- **The segment** is cut where the game's own ray stopped (a wall, a living player): a
  corpse behind a wall is not hit. `ragdoll_bullet` (the arena crate, its own function so it
  ports cleanly to #475) tests it against the ragdolls, moving or settled: each place's box
  (made by the last step) first, then the eleven capsules of those the segment crosses
  (ray against capsule, the nearest hit). The corpses are only read here: the step writes
  them after the move, never during it.
- **The push** is queued (32 a frame at most) and applied at the next step: an impulse at
  the hit point along the bullet, its momentum push (section 12: 0.8 x the ammo's `mass` x
  the bullet's speed; 464b read `[[bullet+40h]+0Ch]` as the weapon's stopping power, but
  `[bullet+40h]` is the ammo's data and +0Ch its `k_power`, 1 for every round), lifted by 0.35 of its size (a shot
  from a standing shooter goes down into a body on the ground, which the ground would stop
  dead), each body's speed change capped at 6 m/s and its spin's at 15 rad/s. The part hit
  takes it: a head or a forearm swings on its joint, the torso hardly moves. For 0.3 s after
  a push the near-rest slowing is off (it would take a small push away at once).
- **A settled body wakes:** a settled slot now keeps its ragdoll frozen (`Slot::Settled {
  ragdoll, since }`), so a shot makes it a moving ragdoll again, at rest where it lay, limp
  (no drive: the clip's end pose would pull a body lying elsewhere along the ground). It
  settles again by the same rules; the 5 s cap counts from the last push.
- **Once a shot (rollbacks).** A rollback restores the bullets and simulates their ticks
  again, so the same segment is traced again. The game's own hit effects are kept once by
  `player_shootmarks_storage::add_shootmark` (0x32E460): per shooter's place
  (`[[bullet+8]+4]`), the bullet's id (+4) and its segment (+84h, counted by its pierces and
  reflections), all three in the bullet's serialized state, so a re-simulated tick gives the
  same three. The push uses the same key with the corpse's place and death
  (`shot::ShotKey`), in a log of the last 256 (`shot::ShotLog`; living hits share it): the first trace pushes and
  plays, the next ones of the same shot do nothing. A rollback that takes the death back
  drops the ragdoll anyway (section 5). A push of a shot the rollback took back (a mispredicted
  shot of another player) stays: visual only.
- **The hit effect (the owner's second ask).** A living player's hit model is game material
  10, `actor` (`bt_animated_rigid_body` 0x70E70 sets every hit body's +38h to 10;
  `game_materials/game.materials`); a bullet is material 7. Their pair
  (`game_materials/material.pairs`, pair 7 x 10) plays `fx/material_hits/blod`, the sound
  `material/body/bullet/body_hits` and the decal `decals/bullet_body_in`. A pierced player gets
  them from `process_front_face_piercing` (0x32E4F0): the pair's first particle slot (+24h)
  by `bullet_manager::play_particle` (0x3223E0: the point, the bullet's direction, the
  surface's normal, the pair's +74h), its sound (+18h) by `play_sound` (0x3222F0) when the
  bullet's +80h is clear and its segments at most one. The hook plays the same two, at the
  point on the ragdoll's capsule, the normal of the capsule there, through the game's own
  queues (they run after the bullets' tick, threads allowed), once a shot, and only when the
  bullet has a shooter (the game's own condition, 0x32E54C). The material pair comes from the
  game's own lookup (0x306D50). **No wound decal:** the game's own decal of a body hit
  goes nowhere on the body (a player's hittable gives `add_decal` no model: vtable+2Ch and
  +30h are both 0x1F1780, `*out = 0`; the decal falls to the world's), so a corpse gets
  none either.
- **Thread safety.** The trace only reads; the log and the queue are taken under a spin
  lock (an atomic in the ragdoll's state); the effects go through the game's lock-free
  queues. A flag set by the step (`any_bodies`) makes the hook return at once when no
  ragdoll is drawn.
- **Batteries (the owner's third ask).** The ragdoll's rays (group 24h, mask 2, the foot
  IK's) did not hit batteries: a battery's dynamic body is put into the world by
  `victory_item_core` 0x301CB0 (its vtable+0ACh) with group 0A18h, mask 418h, and Bullet's
  filter (needsCollision 0x75BD0: the object's group has a bit of the ray's mask and its mask
  a bit of the ray's group) fails both ways. The rays now use group 424h, mask 202h (434h and
  20Ah since section 17): the level as before, and the batteries (bits 400h and 200h; no
  other object put in with an immediate group has them: character controllers 824h/3, hit models 40h/11h, grenades
  818h/8, the lobby's 800h/800h and 20h/20h). The rays only read: a body lies on a battery
  or against it, and the battery is never moved or woken. A dropped battery (a dying
  carrier's, at the simulation's "Weapon" bone, 0x300E50) is a dynamic body that the
  simulation's physics drops (activated, then its velocity set, 0x300DC0); while carried it
  is out of the world (vtable+0B0h removes it).
- **A body over a battery:** the pickup is the simulation's own ray from the eye
  (`detect_usable_object` 0x2D84C0, against the battery's usable shape, 2 m) and the
  ragdoll is in no world, so a corpse never blocks a pickup or the prompt. A body can lie
  on a battery and hide it from view (a torso is about as big), as a corpse's clip could
  before; the pickup still works by looking at it. A battery picked up from under a settled
  body leaves the body as it lay (a settled body asks the level nothing) until a shot wakes
  it.
- **Explosions (not built).** A grenade's blast is applied by grenade code that is not read
  yet (`grenade_core::tick` 0x303A60 and what it calls); the push itself would be a few
  lines (every body near the centre pushed away, `Ragdoll::push` per part, keyed by the
  grenade's id), but the site, its frame and its Unicorn check are another hook's work, not
  a few lines. Left for a next step.

**Costs** (`cargo test --release -p ragdoll cost -- --ignored --nocapture`, x86-64 host):
a segment's trace with 20 ragdolls is 0.14 microseconds when it misses every box, 0.3 to 0.5
when it crosses some (4,000 segments in 0.5 to 2 ms); with no ragdoll the hook returns after
one flag. Heavy fire, every one of 20 bodies hit every frame for 2 s (20 pushes a frame, all
kept moving): the step and the draws 0.77 ms a frame on average, 2.0 ms at worst, up to 328
rays (the game's cap 160); all settled again 2.3 s after the last shot.

**Checked:** `cargo test -p ragdoll` (26, and 3 opt-in): the ray against a capsule's side,
caps and edge, a miss beside or short of it, from inside, along its axis; the box test; the
log takes a shot once and forgets the oldest after 128; the push's scale; a shot through a
settled body hits its head, the head swings with the shot more than the pelvis, the body
wakes and settles again within 6 s, above the ground; a shot pushes a moving ragdoll's
forearm ahead of an unshot twin's; a miss (beside, above, short) and a push for another
death do nothing; the same shot traced twice moves the body exactly as once.
`check_ragdoll_026g.py` (Unicorn): the hook against the original exe's two moves (the same
registers, frame and XMM0), the bullet and the result untouched; a ray above, short of or
stopped before the corpse plays nothing; one through it plays the pair's particle (on the
body, along the ray, the body's normal, the pair's parameter) and its sound, the next steps
wake the corpse, the same ray again plays nothing, another bullet does, and the body settles
again; a body that falls onto batteries by its feet rests on them (the stub's filter is
Bullet's). The self-test's check 76 has the new site and entry.

**What the owner should see:** shooting a corpse (lying or still falling) jolts the part hit:
a head or an arm swings, legs kick a little, the torso barely moves; a burst keeps it moving;
blood puffs and the body-hit sound at each hit; it lies still again about a second after the
last shot. A bullet through a wall into a corpse does nothing. A body falling onto a battery
lies over it, not through it, and the battery stays where it was. Tell me if a corpse jumps
too much or too little (`tuning::SHOT_IMPULSE`, `SHOT_LIFT`), or blood shows where no body is.

## 12. 464c: the dance, the killing shot's push, a powered ragdoll, kit and ammo (2026-10-26)

The owner's reports on 464b: "on the ground the bodies often start to dance"; "bullets don't
seem to be giving momentum to where the body should fall"; "the thing we did with legs trying
to stand doesn't look correct ... since legs dont move"; "characters have equipment which
weighs differently and each bullet has a different force when hitting".

**The dance: the causes (reproduced in `rest_report`, then fixed).** With the frame time the
game has (it is measured each frame and varies), 464b's bodies on a floor, a slope, stairs,
against a wall and on a battery never came to rest before the 5 s cap: their ends still moved
at 1 to 19 m/s between 3 and 5 s after the death (at a steady 1/60 s most settled by 2.5 to
4.9 s). Three causes, each fixed where it is:
- **The substep followed the frame:** 6 substeps a frame of whatever length. A depth pushed
  out at a fixed 6 mm a substep counted as speed, 6 mm / h: 3.6 m/s at 1/60 s, 5 m/s and more
  at a short frame, more energy than the step had. Now the solver steps fixed 1/360 s
  substeps (the frame's remainder carried over), and a depth already there goes out without a
  speed.
- **Two limbs wedged between each other and the ground** were pushed apart in full each
  substep and pushed back by the ground: a limb shook in place (1.2 m/s of jitter on 2 cm of
  creep). Now a self-collision's old depth goes out like the level's, slowly and without a
  speed.
- **One plane for a whole capsule:** a body across two surfaces (the floor and a battery, a
  step's edge) flipped its one plane between them as it rocked. Now each capsule end has its
  own plane, an end over an edge takes the other end's only at its height or above, and an
  end below it falls free (464b-like inheritance from below lifted an end onto the step above
  each frame and it fell back: a 5 cm sawtooth).
- **Not a cause:** a settled body keeping its ragdoll (frozen, it steps nothing: the 60 s test
  below); the 0.3 s window and the lift (only a push starts them, and only shots push, once);
  the battery filter (the 108 maps' collision objects are 80h/100h and 1/20h, no 200h or 400h;
  only batteries carry those bits among the objects put in with a group).
- **The muscles' floor** (464b's 0.03 toward the clip for ever) pulled a body lying elsewhere
  than the clip's last pose along the ground; the muscles now decay to nothing.
- **Test** `a_lying_body_comes_to_rest_and_stays_still_on_every_surface` (corpse machine, the
  game's frame times 1/30, 1/144, 1/60, 1/45, 1/75 s in turn, a rifle shot in the back): on the
  floor, the slope, against the wall and on the battery the body is at rest 2.9 to 3.6 s after
  the death with no bone shaking (no bone moves back and forth 4 times in 0.4 s by over 5 mm);
  down the stairs it slides to the bottom and is at rest by 5.1 s; then not one bone moves for
  60 s; after a burst of five rounds it is at rest again 1.5 to 3.9 s later and stays so.

**The killing shot's push.** The same bullet hook now also keeps a bullet whose ray hit a
living player's hit model (the result's object a `bt_animated_rigid_body`, vtable 0x9E2AF4,
material +38h = 10, actor): the point, the bullet's push, once a shot (the same `ShotKey`, the
place 0FEh and the body). The step gives it to the place whose `[[player+498h]+12Ch]` is that
body (`activate_physics` 0x2D9930 adds it to the world), timed by the step's clock; a death
takes the latest hit (kept 1 s) and those within 0.15 s before it (`shot::Recent`). The
ragdoll starts with them (section 7, the impulse): a front shot falls back, a side shot
sideways, a headshot snaps the head back, a leg hit kicks the leg. A death with no bullet (a
fall, a blast, an anomaly): no push. (A hit of the same life that did not kill, within 1 s of
another death, pushes too: rare, visual.)

**A powered ragdoll** (the Bodycam look; section 7, the muscles). 464b held the legs at full
strength toward a fixed stance for 0.25 s and then faded them: legs pinned like stilts while
the trunk sagged, then the knees went at once. Now nothing is timed: every muscle weakens from
the death on, the legs strongest and longest, the knee giving way more as it weakens, the hit
side faster, the fall's direction from the push and the body's weight. The body shot's knee
bends on every 0.1 s step (0, 0.17, 0.61, 0.80, 1.01, 1.19, 1.32 rad over 0.6 s), the pelvis
is down by 1 s.

**The kit's weight** (the dead player's, read every frame of its death): the inventory
(`[player+10Ch]`), its 23 slots (+10Ch) as `inventory::update_weight` 0x2FC6E0 sums them into
m_carried_weight: an item's +218h (kg a piece) times its count for a stack. Each slot on the
parts it sits on: the helmet and the mask on the head, the torso's armour 70 % on the chest
and 10 % each on the upper arms and the pelvis, the back's on the chest, the legs' 30 % on the
pelvis and 25 % / 10 % on the thighs / shins, the gloves on the forearms, the boots on the
shins, the weapons and the rest half on the chest, half on the pelvis; at most 40 kg a part.
A heavier body takes the same push with less speed and loads its knees more.

| kit (0.26g configs, kg) | helmet | mask | torso | back | legs | gloves | boots |
|---|---|---|---|---|---|---|---|
| lightest | 0.15 | 0.2 | 3 | 3 | 0.8 | 0.2 | 0.7 |
| heaviest | 6 | 2 | 26.4 (Duty exo) | 35 (Duty exo) | 5 | 0.8 | 3 |

A Duty exo-suit adds about 70 kg (the chest capped at 40); a scavenger's kit about 9.

**The bullet's force.** The game has no hit impulse anywhere (no `impulse` key; bullets do not
push rigid bodies). Its ammo data (`[bullet+40h]`, read by 0x332440 from the config's `data`)
has `mass` (+14h, a gameplay scale) and `muzzle_speed` (+3Ch); the bullet's velocity now is
+1Ch (LIKELY). The push is 0.8 x mass x speed at the hit (N s); a corpse's shot takes it as
is, a killing shot 3 times on the part plus 0.6 of that on the whole body:

| round (config) | mass, kg | speed, m/s | corpse push, N s | killing push, N s |
|---|---|---|---|---|
| 9x18 (pistols) | 0.02 | 315 | 5.0 | 15 |
| 9x19, .357 | 0.02 | 430-450 | 7 | 21 |
| 5.45x39, 5.56x45 | 0.02 | 800 | 12.8 | 38 |
| 7.62x39 | 0.02 | 700 | 11.2 | 34 |
| 7.62x51, 7.62x54 | 0.1 | 800-850 | 64-68 | 190-200 (the part's cap: 6 m/s) |
| 12.7x55 (ASh-12), 12.7x54 | 0.1-0.2 | 320-350 | 28-51 | 84-154 |
| 12 mm slug | 0.1 | 450 | 36 | 108 |
| 12 mm buckshot | 0.01 x 10 pellets | 250 | 2 each | 6 each |

**Costs** (`cargo test --release -p ragdoll cost -- --ignored --nocapture`): 20 ragdolls
dying at once, the step 0.77 ms a frame on average, 1.2 ms at worst (464b 0.63 / 1.3), all
settled by 3.8 s; heavy fire on all 20 (every body hit every frame for 2 s) 0.99 ms on
average, 1.4 ms at worst, settled again 3.1 s after; up to 440 level rays a frame before the
game's 160 budget (two ends a body now). A trace with 20 ragdolls 0.14 to 0.5 microseconds.

**Renders** (the solver offline, a standing body on flat ground, side and front views, 464b
left, 464c right; 464b's renders use its own body-shot impulse, which the game never had):
`ragdoll464c-vs-464b.mp4` (real time), `.slow4x.mp4`, `.gif`, made by `render_dump`
(`cargo test -p ragdoll render_dump -- --ignored` with `RAGDOLL_DUMP`) and a small Python
renderer. The front shot falls back, the knees giving way as it goes; the side shot buckles
and topples sideways; the headshot snaps back and crumples.

**Tests:** `cargo test -p ragdoll` (30, 6 opt-in): besides the above, the killing shot decides
the fall (front: back, side: sideways, the head snapped back, the leg kicked, no shot no
push); the legs give way progressively within a second; a heavy kit is knocked less, a sniper's
round more than a pistol's; the push is the round's momentum (pistol < rifle < sniper, a pellet
less than a pistol's); a death takes its killing hit and those 0.15 s before. The Unicorn check
adds: a bullet in a living player's hit model (the hook as the original exe's code), the same
ray again, the death knocked along it (the hip 0.8 m along the shot, an unshot death's none);
its players have an inventory read for the kit's weight.

## 13. 464d: the push proven, the opt-in trace, a little more cinematic (2026-10-26)

The owner on 464c (PvB, bots): "it is almost perfect, maybe tiny changes to make it a bit more
cinematic". 464c's model stays; this is the proof of its push and small tuning.

**The killing shot's push applies (PROVEN, static and Unicorn).**
- **The speed it reads:** `[bullet+1Ch]` is the bullet's velocity, its segment's start. The
  bullet's ctor 0x32BA60 writes the fire velocity (EAX) to +1Ch and +34h (0x32BA8E, 0x32BAAC;
  the self-test's check 76). change_trajectory 0x32EE20 writes a pierce's or a ricochet's new
  velocity to both and counts the segment; `check_ragdoll_026g.py` runs it from the original
  exe and reads them back. compute_parabolic_velocity 0x32C950 reads +34h and takes the air's
  drag off it (+6Ch, the ammo's `air_resistance`): over a tick the drag is small, so the push is
  the segment's start speed (464c's "LIKELY" is settled).
- **The mass:** `[bullet+40h]` is the ammo's data. The ctor checks it against the weapon's
  +53Ch at 0x32BB53, and it has the ammo's `combat_log_icon` at +50h, the kill feed's weapon
  icon (patch 67, client_patch.md). `[[bullet+40h]+14h]` is the ammo's `mass`.
- **Bots:** a bot is a place of the match like a player, driven by the server's inputs
  (`player_bots.md` section 1). Its player object, hit model and `[[player+498h]+12Ch]` are a
  player's, so its hits are matched the same way.
- **The trace says it in game:** the opt-in build below writes each living hit's mass, speed
  and push, and each death's.

**The opt-in diagnostics** (`patch_026g.py ... --ragdoll-trace`; a local build only, never a
release's: release/build-zip does not pass it, and the self-test checks the default build has
it off). The crate's `SRP_DIAGNOSTICS` (`vostok::trace`) is "OFF" in every build and 1 in this one; then the
step writes lines to `srp_ragdoll.csv` in the user data folder (Documents\survarium, the
game's own `get_user_data_directory` 0x18BD0) through the game's CreateFileA and WriteFile:
- `hit,time,place,mass kg,speed m/s,push N s`: each bullet in a living player, when the step
  gives it to its place;
- `death,time,place,cause (0 none, 1 body, 2 head, 3 blast),hits,push N s,knock m/s,mass kg,
  speed m/s,legs,waist,neck,arms`: each ragdoll's start, its muscles' strength then.
- The Unicorn check runs the build with it on: the hit (place 0, 0.02 kg, 800 m/s, 12.8 N s)
  and its death (a body shot, one hit, 12.8 N s, 0.34 m/s, 79.6 kg with the test kit, the
  muscles at 0.91); with it off nothing is opened.

**The tuning** (`crates/ragdoll/src/tuning.rs`):

| constant | 464c | 464d | what |
|---|---|---|---|
| `NECK_DECAY` | 0.25 s | 0.30 s | the head follows the clip a little longer |
| `DEATH_KNOCK` | 3.0 | 3.5 | the killing hit's push on the part hit |
| `BODY_KNOCK_MAX` | 1.2 m/s | 1.6 m/s | the whole body's knock-back from it |
| `DEATH_VELOCITY_SHARE` | (none) | 0.7 | the body's speed at the death carried into the ragdoll |
| `DEATH_VELOCITY_MAX` | (none) | 7 m/s | at most |

- **The speed at the death:** the step now keeps each place's motion while alive (its
  transform's place, the velocity smoothed over two frames, a jump of 2 m a frame is a
  teleport); a death carries 0.7 of it. In 464c a running body lost its speed at the death (the
  simulation moves a dead player only by its clip's root motion), so it dropped where it died.
- **Not changed:** `WAIST_DECAY` (0.25). Already at 0.28 a body shook on the battery in the
  rest test, and a violent fall pulled joints 7 cm apart.

**Renders:** 464c left, 464d right: the front, side and head shots, a sniper's round in the
chest, and a rifle round from the front into a body running forward at 5 m/s, the body
carrying its speed. Files `ragdoll464d-vs-464c.mp4`, `.slow4x.mp4`, `.gif` (`render_dump5`).

**Tests:** `cargo test -p ragdoll` 32 (7 opt-in): 464c's (the 60 s rest on five surfaces, no
stilts, the fall's direction, the kit, the rounds), and a running body carrying its momentum,
the start report. The Unicorn check adds +1Ch from change_trajectory, the trace, a running
player's death (the hip 1.8 m on, a standing one's 0.15 m).

## 14. 464f: too much impulse, a battery carrier's corpse (2026-10-26)

The owner on 464d: "sometimes ragdoll gets too much impulse". The 464d trace (4562 hits, 216
deaths) shows two causes.

**1. The ammo's `mass` is not the round's (PROVEN).** The configs hold four values: 0.01 (a
buckshot pellet), 0.02 (every pistol and intermediate round, 5.45 to 9x39), 0.1 (7.62x51,
7.62x54R, .30-30, the 12.7x55 and the 12 mm slug), 0.2 (the 12.7x54). It is the game's impulse
on a prop a bullet hits: `bullet::apply_hit_impulse` 0x32E3E0 (new re-kit rows) applies the
velocity times `[[bullet+40h]+14h]` to the object's physics body, from process_collision
0x32EBF6 with the bullet's velocity and from try_pierce / try_reflect with the speed lost. A
0.1 round pushed 5 to 10 times a 5.45; real rounds' momenta differ by about 2. The fix is the
smallest: `SHOT_MAX_MASS` 0.04 kg in tuning.rs, the mass counted at most that, so the heavy
classes push 2.1 times a 5.45. A table by calibre was the other way, but the ammo's data has no
calibre and no name (ammo_data::load 0x332440 reads the distance, dispersion, damage, mass,
muzzle speed, air resistance, the hit type and the kill feed's icon), so it would need the item's identity read
and a hand-kept list of 34 configs that drifts with every content pack.

**2. One bullet counted several times on one victim.** A bullet that pierces a body is traced
again, a segment a part, from inside it (850 -> 504.6 -> 504.6 -> 504.5 m/s at one place and
time, up to 17 lines a bullet; a death summed up to 4). The segment is out of `ShotKey` now:
one hit a bullet a victim (living or a corpse), its first segment's (the fastest).

| ammo (mass) | 464d push p50 / p90 / max, N s | 464f p50 / p90 / max | hits counted |
|---|---|---|---|
| 0.01 pellets | 1.25 / 2.0 / 8.0 | 2.0 / 2.0 / 8.0 | 1888 -> 473 |
| 0.02 pistol, intermediate | 6.8 / 12.1 / 12.8 | 7.2 / 12.8 / 12.8 | 2567 -> 626 |
| 0.1 full-power, 12.7x55, slug | 40.1 / 64.0 / 68.0 | 25.6 / 27.2 / 27.2 | 107 -> 28 |

(464f from the same trace: the mass clamped, the first line of each bullet; the p50 of the
first two rises because the repeats were the slowed segments.) The deaths' whole-body knock,
modelled from the trace's hits (the model gives 464d's p50 and p90 exactly): p50 0.29 -> 0.20,
p90 0.71 -> 0.54, max 3.05 -> 0.68 m/s.

At the muzzle, a config's push is 0.8 x min(mass, 0.04) x its speed: a 5.45 12.8, a 7.62x39
11.2, a 9x19 7.2, a 7.62x51 27.2, a 7.62x54 25.6, a slug 14.4, a 12.7x55 11.2, a pellet 2.

**Detection, for the next time.**
- The trace build's lines grow (old readers still read the first columns): `hit` adds the
  muzzle speed (with the mass, the ammo's identity: one or two configs a pair), the segment,
  the bullet's id and the shooter; a new `spike` line is written for a death whose knock is
  past 1 m/s or capped, a push past 30 N s, or a part past 7 m/s after it (one hit gives at
  most its cap, 6, and the knock), with its inputs: the round (mass, speed, muzzle, segment),
  the hits summed, the part, the kit (a push: the part's mass), the push, the part's speed,
  capped.
- `srp-026g/scripts/ragdoll-trace/summary.py <srp_ragdoll.csv> [--calc calc.js]`: the pushes
  per ammo (named from the damage calculator's table), the repeat groups, the top deaths by
  knock, the spikes.
- `no_single_hit_reaches_a_cap` (`cargo test -p ragdoll`): every distinct (mass, muzzle) of
  the 34 configs, on seven parts: the whole body's knock never capped, no part seen faster than
  `SHOT_MAX_SPEED` in the first frames. Without the clamp it fails (a slug on a shin).

**A battery carrier's corpse (PROVEN in code; a design study of the carried battery,
2026-10-26).** When the simulation drops a dead carrier's battery, +128h (the
held item) becomes 0, and mt_draw's `je 0x24781E` at 0x247557 skips every bone update: no
update_skeleton, neither of our hooks after it. The corpse froze at the drop.
- A sixth lohook, `player_draw.held_item` (0x24754F, `mov esi,[ebx+128h]`, the
  named exit `no_bones` at 0x24781E): a corpse of ours with a ragdoll and nothing held is
  posed from the ragdoll alone (`Slot::draw_alone`: every one of the 69 bones from the
  bodies, the drive's clip the last seen) and given to update_skeleton (0x22490, a new thunk
  row: EDI the scene renderer `[[[player+F474h]+9Ch]+5360h]+200064h`, the model's +108h, the
  matrices twice, 69) as mt_draw gives its own at 0x24780A; then the draw goes on at
  0x24781E, ESI 0 as the je leaves it. A dying body (its clip not captured), a living player
  or a skeleton with bones not ours take the original's je.
- No gun capsule when the held item at the death is a battery (`[[player+128h]]` the
  victory_item vtable 0xA042AC: an `::image` row; its +30h is is_ready_to_be_deactivated
  0x301270, which the item switch 0x2D76A0 calls on +128h). The battery's own drop (the study's
  option B, a simulation patch) is the owner's to decide and is not here.
- Tests: `a_corpse_holding_no_item_is_still_drawn_and_settles` (cargo); the Unicorn check runs
  the site for a corpse with +128h 0 (the 69 bones, the ragdoll's, to the stubbed
  update_skeleton, the exit, ESI 0; it falls and settles), a living player holding nothing
  (the original's je), and a carrier's ragdoll with no gun.

## 15. Cosmetic helmet fly-off (2026-10-28)

The owner's candidate list, with Pilgrim explicitly excluded: the currently
available Biker's helmet, Explorer's hat, Keeper, Paratrooper's helmet,
Vityaz-S, Scout's helmet, OZK-18KB and Mounting helmet. The level-2 ported
beanie (`novice_026_helmet`) keeps its binding (the owner, 2026-10-29: the
Novice's knit cap shouldn't fly off). Other names in the list are not present
in the mounted helmet configs and are not ported here.
Hoods, closed helmets with replacement heads, and the exoskeleton helmets
retain their existing bindings.

The 0.29 helmets of `equipment-029` (the owner, 2026-10-29: "yes they
should"): Viper (`black_market_fighter_05_helmet`, 883 vertices), Mercenary
(`black_market_raider_09_helmet`, 676), Nomad (`scavenger_sniper_10_helmet`,
789), Pathfinder (`scavenger_scout_07_helmet`, 1066), Combat
(`scavenger_master_08_helmet`, 506) and Metal (`scavenger_anomaly_06_helmet`,
448): each a 69-bone render bind with Head at 44 and one surface skinned to
Head alone. The Omicron helmet (`black_market_anom_07_helmet`, 670: a hood
joined to its collar, also on RightClavicle, LeftClavicle, LeftEye, Jaw and
RightEye for 36 vertices) and the Warrior helmet (`edge_defender_helmet`,
1122: a neck and shoulder guard, 129 vertices led by Neck, also on Chest, the
clavicles, the shoulders, the eyes and Jaw) fly off whole, the collar or guard
with them, as the Biker's does (the owner, 2026-10-29). `headwear.py` refuses
any bone beyond Neck and Head unless the surface's edit names it
(`allow_bones`, with a `why`): theirs do. While worn, their collar and guard
then follow the head rigidly instead of the shoulders. The Omicron mask, the
Barrier gas mask and the Scavenger hood are not helmets. Their eight materials become two-sided
like the stock seven (no underside audit: a two-sided inner shell stays hidden
under the outer one). The edit list runs after the pack, so it edits the
pack's files.

`headwear-1.json`, listed in `release/content.tsv`, changes source assets
through `content_edit.py` / `headwear.py`. It appends `SrpHeadwear` at render
bind index 69, an exact copy of Head (44), to the combined character's bind
and the eight stock and eight 0.29 standalone headwear binds. The combined bind is selected by
`gameplay/bodyparts/default` → `body_parts/bind_pose`: it is
`scavengers_01_torso_s1.skinned_model/render/bind_pose`. The build checks
that this exact referenced asset is extended; extending only
`base_character_full` leaves the assembled model at 69 bones and makes
the rebound helmets invisible. The sixteen surfaces bind rigidly to 69. Positions, normals, UVs, material references and indices stay exact.
The Biker's three vertices with neck influence deliberately become rigid.
The animation skeleton, animation clips and hit model are unchanged.

The underside audit found no inner crown faces on Biker, Explorer,
Paratrooper, Vityaz-S, Scout, OZK-18KB and Mounting. The same edit list sets
those seven materials' `material/g_stage/effect/two_sided/value`
from false to true so their backfaces render.
Their original texture maps also cover the reverse side. Keeper already
has a textured inner surface and keeps its one-sided material. No new
geometry, UVs, texture maps or material instances are added.

**PROVEN in the executable:** `skeleton_render_model::load_bind_pose`
(0x15A000, called at 0x128D46) takes its count and order from the render
bind file, skips names, and builds inverse matrices without consulting the
animation skeleton. `update_render_matrices` uses the command's count;
its current, alternate and previous matrix buffers each hold 128 matrices.
The scene command copies at most 80 matrices. The extended count is 70.

Three additional Rust lohooks:

- `player.killed_event` (0x2DA3DF), after `generate_killed_event` enqueues
  the native death event: ESI is the victim and BL is its cause icon (1 is
  a headshot). The enqueue preserves both. The hook records this result
  against the victim's place and spawn time, outside patch 67's span.
  Only a matching fatal headshot creates an independent headwear body;
  body hits, suicide/environmental kills and unknown deaths keep Head's
  pose. A recent head impact alone cannot enable detachment. Alive,
  absent and uninserted players clear the record; a new life cannot use
  an old result. A corrected non-headshot event also restores Head's pose.

- `player_draw.post_bones` (0x2477DF), before the original render command's
  arguments are pushed: for an extended bind, copy the 69 drawn matrices
  and append the cosmetic bone. Alive or canned bodies use Head's pose;
  ragdolls use their own headwear pose. The command copies the temporary
  arrays, then the hook exits to 0x24781E. The unheld-corpse path uses the
  same posting API. No pointer to temporary data survives the call.
- `render.skeleton_updated` (0x15A950), on the rendering thread after the
  matrix loop: a 69-matrix command with the extended bind copies Head's
  skinned matrices into 69, including previous/shadow data and first-update
  initialization. This covers ordinary commands and baking. A 70-matrix
  command already supplies the detached pose and is left alone.

The extra channel is recognized by the immutable render bind: exactly 70
matrices and inverse bind 69 equal to finite inverse bind 44. Stock data
therefore keeps the original draw path. Only edited meshes use the extra
channel; it is visually unused for other equipment, including Pilgrim.
The cosmetic body exists with an extended character rig even when no mesh
uses that channel, avoiding a second item table in client code.

The channel starts exactly at the captured Head pose, inherits its motion,
and gets a small release velocity (0.8 m/s up, 0.5 m/s sideways) and tumble.
It uses the solver's ground/battery queries, fixed substeps, friction,
finite fallback and settling. It shares the corpse's ownership and death
key: rollback/respawn discards it, the same death keeps it, slot eviction
falls back to the worn pose. It is cosmetic: no inventory removal, armour
change, pickup, simulation-world body or replay compatibility change.
Like the dropped gun, it has no collision response against the corpse and
does not receive subsequent bullet hits. Shape and release motion need
in-game visual tuning; the initial capsule is 0.04 m half-length and 0.10 m
radius with a 0.10 m offset above the captured Head bone.

Validation: solver tests cover independent fall, ground contact and death
ownership; Unicorn executes the eight linked hooks, extended commands,
stock fallback, first-update/shadow/previous matrices, unheld corpses and
rollback; content edits are checked against the actual stock meshes.
The executable and content must ship together through the normal release.
The isolated PR's full manifest build passed on 2026-10-28: all 2,643
manifest files match when its generated patch overlays the listed player
patches. The 19 bind/skin transformations are checked against the source
assets, including the beanie from the published content patches. The
packed material audit confirms exactly one boolean byte changed in each
of the eight edited materials; Keeper’s material and all geometry, UVs
and texture maps are preserved. Offline underside renders cover the
eight original candidates. A local capture client and an isolated bot
server now support a visual pass: bots cycle through all nine headwear
items in both kits. Native lighting, shape and release motion still need
the owner’s visual assessment before release.

## 16. The headshot launch (2026-10-30)

A headshot corpse still creeping after about 3.6 s was flung 2-6 m up and
frozen there by the 5 s age cap (the owner: "it is often broken"). The
muscles' strength is `s0 * exp_neg(age / tau)`, and `exp_neg` raised a
5th-order series on x / 16 to the 16th power: past x = 50 the series turns
and grows, so the strength became 2.66 s0 at x = 52 and 1e17 s0 at x = 79
(the headshot's x passes 50 at 3.61 s, a blast's at 7.73 s, an arm's at
10.3 s). `exp_neg` is now 0 from x = 20 (e^-20 is 2e-9); below it the values
are unchanged. Offline, 26 of 80 headshot scenes launched before and none
after. Tests: `exp_neg` against the real exponential over 0..80, never
growing; a headshot body on flat ground keeps every part below 0.6 m after
1.5 s (it was 0.83 m and rising at 3.77 s).

## 17. Guns in Rudnya's ground (2026-11-01)

The owner: "on Rudnya, weapons quite often fall under the textures"; the dropped gun lies
partly or wholly under the ground, hard to see.

**Why (PROVEN, on the data).** The rays met the walker collision alone (cgroup 2 / cmask 4,
the foot IK's filter). On other maps that mesh is about the drawn ground (level 04's terrain: a
median 0 cm, 4 cm at the 90th percentile). Rudnya's (native level_01's `terrain.model` and the v0.69 port's
`terrain_game_*`, the same geometry) is coarse and lies under the drawn ground: over the
play area the drawn ground (its base surfaces and the blend overlays `*_vc`, the dirt,
earth and grass drawn over 57 % of it) is a median 4.1 cm above it, 9.7 cm at the 90th
percentile, 17 cm at the 99th; more than the gun's radius (4 cm) on 51 % of the ground. The
gun's axis rested 4 cm over the walker mesh: at the drawn ground or under it, the gun half
or wholly hidden; the bodies' thinner parts (forearms, r 5 cm) partly. The level's other
collision, the one bullets hit (cgroup 8 / cmask 16), follows the drawn ground: within
2.2 cm at the 90th percentile, 3.4 cm at the 99th.

**The fix.** The rays meet that collision too: group 434h, mask 20Ah (added group 10h, mask
8, as the lobby's camera ray 0x2844D0). The nearer surface is met, so nothing lies lower
than before. Nothing else in the world has those bits both ways: grenades (818h/8) have
group 8 but none of 434h in their mask; players 824h/3, hit models 40h/11h, the lobby's,
the client objects' geometries (80h/100h, 1/20h) neither. Visual only: no data, no
simulation change.

**The mud field.** East of the play area (x 95 to 144, z -24 to 31, about 950 m2) the
dirt overlay is drawn 30 to 48 cm above both collisions, in 0.26g's native Rudnya, in
0.69d's own `terrain_game_07` and so in the port (0.69d had no collision at that surface
either; 0.26g's `hq_collision`, in no project, follows it). A body or gun there would sink out of
sight, and players' feet sink there as they always did; it lies outside the play area
(the site's `play_area_px` for level_01: x -86 to 86), so nothing was changed for it.

**Checked.** `cargo test -p ragdoll`: on 6 x 6 m of Rudnya's ground by the decent house
(`ragdoll/rudnya_floor.rs`, the three meshes' triangles), five deaths: on the walker mesh
alone the gun's axis settles 5.8 to 6.3 cm under the drawn ground (deeper than its radius),
the bodies' 2 to 4 cm; with the collision bullets hit, 3 cm above it, the bodies' 4 cm.
The patches' test: the filter meets the walker collision, the hittable one, a collision of
both and the batteries, and none of the others above. `check_ragdoll_026g.py`: a walker
floor at 0 and the hittable one 4 cm over it, a grenade's and a player's slab where the
body falls; the filter is 434h/20Ah and the body lands on the hittable floor, through both
slabs.

## 18. The gun a body leaves (2026-11-02, patch 85's drop drawn alone)

A respawn reuses the player: its body goes, and with it the gun the ragdoll
drew. Patch 85's drop lingers past it (weapon_pickup.md, "The gun drawn alone").
`corpse::Lone`: when a place's body goes because its player lives again, its
gun body (`Ragdoll::lone_gun`) is stepped alone by the ragdoll's own rules
(`LoneGun::step`: `query_body`, the contacts, friction, the near-rest slowing,
the same rest rules for one body) until it rests, then kept until the place's
next death (or a rollback that brings the body back). `Corpses::lone_gun`
gives its `Weapon` bone's world pose; the patch hands it to
`weapon_pickup::draw_alone` after each step. Visual only.

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
| 0x32D4E0 | `bullet::check_collision`: the bullet's fields (EAX) | abi_inputs |
| 0x306D50 | `game_material_manager::material_pair` (ECX, DX, DI; the pair record) | abi, abi_inputs, inferred |
| 0x3223E0, 0x3222F0 | `bullet_manager::play_particle` / `play_sound` | abi, abi_inputs |
| 0x32E460 | `player_shootmarks_storage::add_shootmark` (an effect once a shot) | abi |
| 0x70E70 | `bt_animated_rigid_body` ctor: material 10 (`actor`) | abi |
| 0x301CB0 | `victory_item_core::activate_physics`: group 0A18h, mask 418h | abi, inferred |
| 0x332440 | `ammo_data::load`: the ammo config's `data` (+14h mass, +3Ch muzzle_speed) | abi, abi_inputs, inferred |
| 0x2FC6E0 | `inventory::update_weight`: the slots, an item's weight and count | abi, abi_inputs |
| 0x18BD0 | `engine_world::get_user_data_directory` (the trace's folder) | abi, inferred |
| 0x32E3E0 | `bullet::apply_hit_impulse` (464f: the ammo's mass x the velocity on a prop hit) | inferred, abi, abi_inputs |

Already in the kit and used above: 0x338330 dead state activate, 0x3383E0 / 0x338430 /
0x3384E0 its serialize / deserialize / end, 0x2474A0 mt_draw, 0x1F77E0 game_world::tick,
0x22490 update_skeleton, 0x75E20 ray_test, 0x76920 object_query, 0x70F40, 0x71080,
0x714F0, 0x71600 (the hit model's makers), 0x324410 physics history item.
