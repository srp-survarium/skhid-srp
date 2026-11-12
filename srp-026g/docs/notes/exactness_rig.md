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

### 3b. The cast, bit for bit (PROVEN)

The 119 steps over tolerance sat on edges and corners: the exe's surface normal after the step is some face's
normal in 822 of the 1,000 steps (47 of them over), an edge's or a corner's in 174 (72 over). The port's caster
had the time-of-impact rules of `calcTimeOfImpact` on an exact capsule-triangle distance; the exe's distance is
GJK's, an iteration in `f32`.

**The corpus.** The exe in Unicorn on 1,000 traced steps, with hooks around
`btGjkPairDetector::getClosestPointsNonVirtual` 0xD6CD0: 57,641 calls, each a record of the two shapes' bytes,
the input, the detector's settings and its state after, and the point collector after (`gjk_corpus.py`). Cast
shapes: the capsule (51,643), the cylinder of the move test (5,884), the sphere at the capsule's top (114); the
other shape is always a triangle; every transform has the identity's bits; the largest distance squared is 1e18;
24 calls go on to the penetration solver.

**The port.** `controller/exe_gjk.rs`: the detector's loop, the Voronoi simplex solver (0xD1170 .. 0xD2700) and
the support function 0x8D680 for the four shapes, in the exe's order of operations (scalar SSE `f32`; the x87
square roots at 24-bit precision are `f32`'s). Each sum of three terms is grouped as the asm groups it, which is
often not the source's order: put back one at a time, the source's order breaks between 146 and 9,988 of the
records. This build differs from stock Bullet in one place that matters: the exit on a stalled distance (12)
makes no `backup_closest`. **57,617 of 57,617 calls without a penetration call give the exe's bits** (normal,
point, distance, the detector's method, round count, degeneracy, cached axis and distance), and the port stops
for the penetration solver on exactly the 24 calls the exe makes it on
(`tests/exe_gjk_corpus.rs`, run with `SRP_EXE_GJK_CORPUS`: the records hold the game's triangles and stay
private). Not reached by the corpus, ported from the asm only: exits 6, 10, 13, the round limit, ignored margins,
bases other than the identity.

`controller/exe_cast.rs` is `processTriangle` 0xB8670 over `calcTimeOfImpact` 0xB9320 for a shape that only
translates: the way is the end's origin less the start's (`calculateVelocity` 0x72EB0), the origin at lambda is
`from + way * lambda` (`integrateTransform` 0x8B2D0; the basis stays the identity's bits), the reported normal is
made unit again. `Caster::Exe` puts it under the sweeps; `Shape::CapsuleAt` carries the controller's own centre
and dims to it; the tick's sweeps hand the way as end less start (a displacement added and subtracted again is
not the displacement: that alone was one unit in the last place of y on 358 landings).

| the port against the exe on equal triangles, 1,000 steps | the same bits | under 1e-4 m | under 1 mm | under 1 cm | under 10 cm | surface normal off |
|---|---|---|---|---|---|---|
| `Caster::Bullet` (exact distance, the exe's rules) | 137 | 744 | 84 | 33 | 2 | 151 |
| `Caster::Exe` | **991** | 0 | 3 | 3 | 3 | 7 |

The 9 steps left all met a wall (the move test: the wall normal by rays, the penetration solver's contact where
the cylinder's core starts in a face). The whole run of the 1,000 cases takes 0.87 s with `Caster::Bullet` and
1.10 s with `Caster::Exe` (one thread, the cases' parsing and tree building included).

Not ported: the penetration-depth solver (Bullet's EPA). Where the exe calls it, `Caster::Exe` gives
`Caster::Bullet`'s contact.

### 3c. The bots' map export against the game's world (PROVEN as counts)

The export of `level_07/day_4` (`MapData::walker_triangles`, 414,210 triangles) against the capture of the
running game (663,122 walker triangles as this rig cuts them):

- every level-mesh triangle of the game (95,554) and every placed-mesh triangle (137,896) is in the export with
  the same corners in the same order; no walker behaviour differs; the boxes are equal to a millimetre;
- the primitives (boxes, spheres, cylinders, capsules: real Bullet shapes in the game) are triangles in both, cut
  differently (35,188 of the rig's 429,672 primitive triangles have a twin).

The exe's own step on each world against the game, 1,000 steps:

| | the same bits | under 1e-5 m | under 1 mm | under 1 cm | 1 cm or more |
|---|---|---|---|---|---|
| the exe on the rig's triangles | 636 | 141 | 136 | 55 | 30 |
| the exe on the export's triangles | 637 | 138 | 137 | 57 | 30 |

So the export is as good as triangles can be, and what is left against the game is the primitives being
triangles at all: about 36 % of the steps are not the game's bits, 3 % are a centimetre or more off (the worst
0.23 m), every one of the worst 15 beside a primitive. With the game's real shapes the same exe is the game in
6,784 of 6,784 steps (§2).

### 3d. The game's shapes as the port's world (PROVEN as counts)

`Collision::build_exe` (`crates/mapdata/src/controller/solids.rs`) makes the walking body's world from the map
file's own Bullet shapes (`crate::bullet`, format 6) and casts against them as `objectQuerySingle` 0xA14E0 does:

- a mesh: each triangle in the mesh's own frame with the object's transform as the cast's transform B
  (`cast_triangle`), also for a mesh that is only moved;
- a box, a sphere, a cylinder, a capsule: the convex shape with its own margin (`cast_convex`: the convex
  branch's report, the normal's length summed `(x*x + y*y) + z*z`, the callback's fraction strictly over the
  hit's);
- a compound: each child under `object * child` in the exe's order of operations (`compose`, 0xA2C30).

Everything is held in Bullet's frame; a sweep goes in and its contacts come out in the map frame (z negated).

What the walk tick's callbacks read of the hit (read in the exe):

- the walker behaviour (the down sweep's callback 0x84420, 0x84561..0x84584): with a shape info the object's
  `get_walker_behaviour(index, is_shape_index)` 0x356D00: a mesh's triangle its own word, a compound's child the
  child's (`LocalInfoAdder` 0xA2BE0 names the child). **A convex shape that is an object's whole shape has no
  shape info and counts as 0, by slope, whatever its material.** No such primitive of the eight offered maps
  has another behaviour (613 of them, all 0), so nothing changes there today;
- steep ground looked at again (0x84810): the hit object's shape (+CCh, the child while a compound is queried) of
  type 8, 10 or 13 with a radius under 0.8 m (0xA7AB04: a sphere's +20h times +10h, a capsule's implicit
  dimension on `(up + 2) % 3`, a cylinder's `getRadius` 0xA9BC0: the half extent with its margin) does not get
  the ray's look: a sphere's normal is tilted to just walkable (0x84E20); a capsule's or a cylinder's too when
  it lies, `1 - |up . (M up)|` over the callback's +94h (`M` the shape's world rotation through a quaternion,
  0x84190; the constructor 0x842E0 gets the slope limit's cosine there, 0x85176); a standing capsule's normal
  stays as it is, a standing cylinder gets the ray's look. Ported as `SmallRound`, `Controller::steep_ground`.

The port's step on that world against the game's own bytes, the same 1,000 steps (`port-runner ... world
level_07_day_4.mapdata`, `port_vs_game.py`):

| the port's world | every field the game's | the origin the game's bits | off by 1 cm or more |
|---|---|---|---|
| the capture's triangles, the exe's cast (§3b) | | 637 | 31 |
| the map file's shapes (`build_exe`) | 981 | 985 | 0 |

The 19 steps left: the origin in 15 (12 under 0.1 mm, the worst 1.8 mm), the surface normal alone in 4. 15 of
them meet something on the move, all beside a primitive. LIKELY causes, none ported: hits at equal fractions are
taken in another order than the exe's broadphase and mesh trees hand them; Bullet's penetration solver (the
contact is read from the drawn triangles there); the rays of the second looks meet a primitive's drawn
triangles, not the shape.

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
