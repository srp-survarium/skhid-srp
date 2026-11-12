# The exe in Unicorn on the game's own memory: is it the game? (0.26g)

The walk tick's port (`crates/mapdata/src/controller/tick.rs`) is checked against the exe's own `player_step`
run in Unicorn. That check is only as good as two things: whether the emulated exe computes what the running
game computes, and whether the memory it is given is the game's. This note answers both with the game's own
steps (`body_trace.md`) and says where the port stands on them. Tags: PROVEN, LIKELY, GUESS. Addresses are image
VAs (build 2777, base 0x10000). Written 2026-11-12, revision-56 exe. The rig's scripts sit beside the Unicorn
harness they extend, outside the repository; the tools they build on are in `scripts/collision-world`.

**Result in one line:** the revision-56 exe's `updateAction`, run in Unicorn at control word 0x007F on memory
fetched from the frozen game, leaves the running game's own bytes in 6,784 of 6,784 traced steps; the port, on
the same triangles as the exe, is within 1e-4 m of it in 881 of 1,000 and bit-identical in 137.

## 1. The rig

A traced whole step gives the controller's bytes before `player_step` and after it. The rig writes the state
before into a controller the exe's own constructor built (walk and air-control vectors, pos, `m_last_pos`, the
ghost's whole transform, the flags, the capsule through the exe's `setup_shape_dim` 0x7F100, gravity,
`m_jumping`, the two normals, the ceiling slide, the velocity), calls `updateAction` 0x7B730 with the traced dt,
and at 0x7B7B7 lays 12 fields against the game's bytes there: the walk vector as the step left it, pos,
`m_last_pos`, the basis, the origin, the dims, six flags, `m_jumping`, the two normals, the ceiling slide, the
velocity.

**The memory is the game's.** The emulator starts with the exe's image and that controller. Any other page
the emulated code touches is fetched when first touched: from the frozen game's process, or from the file an
earlier run saved. So `btCollisionWorld::convexSweepTest` 0xA36E0 and `rayTestSingle` 0xA02D0 run as they are:
the broadphase's tree, the compounds' trees, the meshes' BVH, the shapes, the face data, the user objects.
Ours are only Bullet's two allocator functions (`[0xA7AAF0]` alloc(size, alignment) and `[0xA7AAF4]` free: a
bump allocator; the exe's own, 0x8A130 and 0x8A170, lead through malloc into the engine's start-up and its
imports) and the landing callback (none). A sweep's world argument is set to the game's dynamics world (the
controller's testers hold it at +5060h, the cast shape at +5058h: 0x87C44..0x87C76).

A run: one hidden game with a replay, 30 s traced (`step_trace.py`), the game frozen, 6,784 moving whole
steps in 274 s, 501 MB; 1,561 pages fetched (6.4 MB), saved. From that file 1,000 of 1,000 steps come out
the same with no game. The trace and the pages file together are a test set with the game's own answers.

## 2. The exe in Unicorn against the running game (PROVEN)

| the world the emulated exe is given | steps | the same bits as the game |
|---|---|---|
| the game's own memory, fetched on demand | 6,784 | **6,784** |
| the captured shape objects, with our stand-ins for the compounds' trees, the meshes' BVH order and rays | 400 | 383 |
| the capture cut into triangles (meshes as they are, primitives 12-sided and sharp, placed meshes moved in double precision) | 400 | 278 |
| the same, steps with only level meshes at the origin near the body | 69 | 69 |

- **Unicorn is a true oracle for the walk tick**: add, multiply, divide, square root and compare at 24-bit
  precision give the CPU's bits through 18,663 sweeps and 3,449 rays (jumps, falls, crouches, slopes, marked
  faces included). The trace's state is the whole state a step needs: nothing in the controller's 181 KB
  between +210h and +2C700h carries over.
- **What cutting the world into triangles costs** (third row): 122 of 400 steps differ, up to 3.2 cm, every one
  near a primitive or a placed object. On this project (Vector Laboratory, mode 4) 429,672 of 663,122 walker
  triangles come from primitives. The game's box is rounded by its margin where ours is sharp, a round shape
  is 12-sided, and a tie between two shapes is won by the tree's order.
- **Order decides ties** (second row): with the real shapes the last 17 differing steps were ties (the
  same origin, another surface normal) and rays; with the game's own trees they are gone.

**The x87 trigonometry** (hazard for later stages, not for the walk tick): FSIN, FCOS, FPTAN, FSINCOS and
FPATAN in Unicorn against this machine's CPU on 2,000 inputs each at control word 0x007F: stored as f32, the
same bits in 10,000 of 10,000; as 80-bit results the same in 10 of 10,000 (Unicorn's are a double's precision:
off by up to 626,000 units of the 64-bit last place). FSQRT is the same either way. So code that stores the
result to a float, or goes on at 24-bit precision, sees a difference only when the result sits on a rounding
boundary: about once in a million operations (LIKELY, from the two precisions; none was met).

## 3. The port against the exe on equal input (X1b, the first count)

1,000 traced steps; the port (`port-runner`: every public field of `Controller` set from the state before,
one `step`) and the exe in Unicorn on the same cut triangles.

| | the same bits | under 1e-4 m | under 1 mm | under 1 cm | under 10 cm | 10 cm or more |
|---|---|---|---|---|---|---|
| port, `exe_collision` rules, against the exe (origin) | 137 | 744 | 84 | 33 | 2 | |
| port, default rules, against the exe | 121 | 721 | 122 | 20 | 15 | 1 |
| the exe on those triangles against the game | 638 | 253 | 24 | 55 | 28 | 2 |
| port, `exe_collision` rules, against the game | 129 | 719 | 50 | 71 | 29 | 2 |

- 119 of 1,000 steps are over the port's own tolerance (1e-4 m) on equal input, 35 of them over a millimetre.
  By the port's notes 82 of the 119 are a plain landing and 29 a forward hit with a landing. The surface
  normal is another vector than the exe's in 151 steps (83 by 0.01 or more). These are for Stage A to take one
  class at a time; the cast (GJK's own distance, `bullet_cast.md`) is the first suspect.
- **`m_can_stand`** (PROVEN on the traces, 30,618 steps): the game holds +202h at 0 while the crouch INPUT
  (+200h `m_input_is_in_crouch`) is held (7,604 steps), tests standing on the step after the input is let go
  while +201h `m_logic_is_in_crouch` is still set (118 times it becomes 1, 16 times it stays 0), and the capsule
  stands on the next step, when the game's logic has cleared +201h (118). The port had no +200h and left
  `can_stand` true on a held crouch (149 of 149 crouched steps of the sample): §3a.

### 3a. Crouch, read in the exe and ported (PROVEN)

Four bytes of the controller, by the exe's names: +200h `m_input_is_in_crouch` (i), +201h
`m_logic_is_in_crouch` (l), +202h `m_can_stand` (s), +203h `m_capsule_is_in_crouch` (c).

| who | where | what |
|---|---|---|
| the game's input | `base_player::apply_new_input` 0x2D86E0 (0x2D89EE) | i = the crouch key (bit 8 of the actions) |
| the game's logic | `player_logic_crouch_state` 0x337810 / 0x337830; `base_player::jump` 0x2D793E | l = 1 on a crouch; l = 0 only when s is set; a jump clears l |
| `player_step`'s head | 0x7BDAE..0x7BDDD | l and not c: crouch (`setup_crouch_state(1)`, which writes s = 0); not l, c and s: stand (`setup_crouch_state(0)`, s = 1) |
| `can_skeep_player_step` | 0x7C7E0 (0x7C89A) | a step is skipped only when c equals i (and nothing else moves the body) |
| `step_down`'s tail | 0x7E871..0x7E90D | crouched and the key RELEASED: s = the stand test 0x7F7B0 (the stand tester's sweep 0x873C0 from the collision object's origin); then, still with the key released and s = 0, the push from under a slanted ceiling 0x7BAD0 |

So with the key held the game makes no stand test and no ceiling push, and a crouched body at rest skips its
steps. After a release the next step makes the stand test, the logic then lets go of the crouch, and the step
after that stands: the capsule is up 50 ms after the port's old rule had it up.

Counted on 27,790 traced whole steps of the game's own bodies (three traces, `crouch_census.py`): held and
crouched 6,954 steps, s = 0 after every one; released with s = 0 before, 128 steps, s set by 110 and kept 0 by
18 (a ceiling); 113 presses (i and l set together between two steps), 112 releases (i alone cleared), 108 times
the logic then cleared l. Of 114 releases followed for four steps, 99 are exactly held, test, stand; the rest
are ceilings and keys pressed again. Not read (the logic's side, B6): 36 steps with the key held and no crouch
asked (i set, l clear), and 39 steps where the logic kept l with s set.

The port (`tick.rs`): `Controller::input_crouch` is +200h, `Controller::crouch_key(held)` is what the input
and the logic do between two steps, the tail and the skip test read the key as above. On the 1,000 traced
steps of §3 the port's crouch flags, ceiling normal and ceiling slide are now the exe's in every step (before:
149, 5 and 1 steps off).

## 4. What was read on the way (PROVEN)

- `objectQuerySingle` 0xA14E0, a compound (type 31, 0xA22EE): the cast shape's box in the compound's frame,
  then `btDbvt::collideTV` 0xA58F0 over the compound's own tree (+40h, `m_dynamicAabbTree`; its root at the
  tree's +0); with no tree no child is tested. The walk's node stack is 256 bytes from `[0xA7AAF0]`.
  Compounds hold triangle meshes too (1,283 of 2,232 objects are compounds).
- `btBvhTriangleMeshShape::performConvexcast` 0xA8380 as 0xA1BB5 calls it: [esp+4] the mesh shape, [esp+8]
  the triangle callback, [esp+0Ch] the cast's start in the mesh's frame, eax its end, esi and edx the cast
  shape's box.

## 5. Not covered

- One map, one replay, 30 s; the live game's own player (`body_trace.md`).
- A random-input mode (the exe as the judge of states nobody played) is not built; the rig can run any state
  on the saved pages.
- The rig calls `updateAction`; calling another function on the game's memory needs its arguments described,
  nothing else.
