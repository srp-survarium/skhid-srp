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

The 19 steps left: the origin in 15 (12 under 0.1 mm, the worst 1.8 mm), the surface normal alone in 4. Told
apart by marking, in a run of the same 1,000 steps, each answer that is not the exe's own arithmetic:

| in the step | the 15 with the origin off | the 4 with only the normal off | the 981 equal |
|---|---|---|---|
| a wall-normal ray met something (`ray_pick` 0x898C0) | 15 | 0 | 26 |
| a hit on a primitive | 15 | 0 | 509 |
| two hits with the same fraction | 4 | 4 | 161 |
| the penetration fallback | 0 | 0 | 3 |

So (LIKELY, by these counts): the 15 are the wall normal's ray, which here meets a primitive's drawn triangles
where the exe's ray meets the convex shape (Bullet's ray against a convex shape is not ported); the 4 are which
of two faces met at the same fraction the down sweep keeps (the exe's order through its broadphase and mesh
trees is not ported). The penetration fallback is in no step that is off.

### 3e. The second looks' ray, bit for bit (PROVEN)

The walk tick looks twice at some hits with a ray: the wall normal (`get_wall_normal_via_ray` 0x898C0) and steep
ground (0x849BB in 0x84810). Read in the exe: each makes a closest-ray callback (0x83FD0) and calls
`btCollisionWorld::rayTestSingle` 0xA02D0 **on the object the sweep hit** (the result's own object, its root shape
+D4h under its transform +10h; 0x899C7..0x899FF, 0x84B5D..0x84B8E): one object, never the world. Afterwards only the
callback's object (+8, a hit or not) and its normal (+40h) are read. The port's ray went over every placed
triangle, so it could meet a nearer face of another object, and met a primitive's drawn triangles.

`rayTestSingle` is ported whole (`crates/mapdata/src/controller/exe_ray.rs`), in the exe's order of operations:

- a convex shape: `btSubsimplexConvexCast::calcTimeOfImpact` 0xB72A0 of a point (`btSphereShape(0)`) against the
  shape, 32 rounds at most (Bullet's source has 64), with the shapes' own virtual supports, margin in (not
  0x8D680's; the virtual capsule support groups its sums differently). A hit's normal is rotated by the ray's own
  basis and made unit; the callback's fraction must be strictly over the hit's. **This build adds a second cast:**
  when the forward cast misses and the ray starts inside the shape's box, it casts back from the ray's end to its
  start and reports `1 - fraction` with that cast's normal;
- a mesh: the ray's ends in the mesh's frame through `btTransform::inverse` 0x72D20, each triangle by
  `btTriangleRaycastCallback::processTriangle` 0xB8010 (the normal turned over when the start is not in front), the
  bridge's `reportHit` 0xA0F60 rotating the normal by the object's basis;
- a compound: each child by the same function under `object * child`, composed inline as 0xA2C30 composes it but
  for the origin's x (`RayTester::Process` 0xA10B0).

The callback's `addSingleResult` 0x84040 stores whatever it is handed, with no test of its own.

Proof (the private rig; `tests/exe_ray_corpus.rs` with `SRP_EXE_RAY_CORPUS`): every 0xA02D0 call of all 6,784
traced steps (1,285 calls of the two looks, 2,164 calls for compounds' children under them) and 11,206 more calls
made in the rig on the game's objects with made rays (the capsule only so: the traced world has none).

| | recorded | the exe's bits |
|---|---|---|
| a triangle (0xB8010) | 7,668 | 7,668 |
| a cast against a box / a cylinder / a sphere / a capsule | 7,687 / 679 / 72 / 546 | all |
| a compound child's transform | 4,200 | 4,200 |
| the whole call: the callback's bytes after | 12,491 | 12,491 |

Put back into the source's order one sum at a time, 23 of 29 changes break records (the others are sums the
recorded data does not tell apart: LIKELY).

With it (`Collision::look_ray`, the contact carrying its object) the port's step on the shapes world against the
game, the same 1,000 steps: **every field the game's in 993** (981 before), the origin's bits in 997. The 7 left
(3 with the origin off, the worst 1.8 mm; 4 with the surface normal alone) each have two hits at exactly one
fraction in the step: which comes first follows the exe's broadphase and trees, whose walks are not ported
(LIKELY the cause, by that count).

### 3f. A body kept across ticks (PROVEN as counts)

The counts above start every step from the game's own state. A reckoning keeps one controller for a body and
steps it tick after tick; `Controller::tick` is that step: the crouch key, the walk vector and the air control as
the game's logic writes them, the sprint flag (+204h: `step_forward_impl` then takes the simple move test,
0x7D33D), then the step.

Every whole step of the traced match in order (`export_chain_cases.py`, `port-runner ... chain`,
`chain_report.py`): 8,993 steps of 16 bodies in 50 chains of steps 50 ms apart (the longest 348 steps, 17 s). The
port carries its own state from step to step; from the trace it takes only what the game wrote between two steps
(a field whose bytes before a step are not its bytes after the step before: the crouch wish 71 times, the ceiling
normal 38, a jump's flag 12, the velocity 9, the place 3) and the step's inputs.

| | steps | share |
|---|---|---|
| each step from the game's own state: every field the game's | 8,954 of 8,993 | 99.6 % |
| the chain: the origin the game's bits | 8,012 | 89.1 % |
| the chain: within 1 mm / 1 cm / 10 cm of the game | 8,115 / 8,712 / 8,993 | 90.2 / 96.9 / 100 % |

42 of the 50 chains are the game's bits from end to end; where a chain leaves the game (after a median of 162
steps, at one of the 39 steps above) it stays within 5.6 cm and ends within 2.7 cm: the difference is carried,
it does not grow.

### 3g. What a step costs, and what was taken out of it (PROVEN as counts)

Where a step's time goes on the shapes world (one thread; clocks and counts patched in for the measurement only):
the casts are nearly all of it, and a cast is the exe's closest-point loop (`getClosestPointsNonVirtual` 0xD6CD0),
7 to 8 rounds a call from its fixed first axis because the capsule's core is round. That arithmetic is the exe's
and stays as it is. What is not the exe's is which triangles are cast at all: a leaf of a mesh's tree holds up to
four, and every one was cast when the leaf's box met the swept box.

- **A triangle wholly outside the swept shape's box grown by 0.02 m is not cast** (`out_of_reach`: the corners'
  box in the mesh's frame, then Akenine-Moller's triangle-box test on the placed corners). Its cast is a miss:
  the cast reports a hit only where the detector's distance is at most 0.001 m and goes to the penetration solver
  only where the shapes' cores are under 0.01 m apart, and the detector's distance is never under the true one but
  by rounding (its closest vector is a point of its simplex, so of the difference of the two shapes). Counted with
  both run side by side: 2.06 million casts the test leaves out, none other than a miss.
- The tree walk keeps its stack in the frame (one heap allocation a sweep before).

| per step | bodies walking into what is there (36,000 steps, two maps) | traced play (8,993 steps) |
|---|---|---|
| triangle casts, before | 118.3 / 71.5 | 25.1 |
| triangle casts, after | 45.1 / 30.5 | 10.2 |
| closest-point rounds, before -> after | 997 -> 569 / 706 -> 435 | 145 after |
| time (pairs run in turn, the least of each) | 86 -> 66 us / 75 -> 62 us | 21.1 -> 15.5 us |

The first column is the walks of a cost test (300 bodies from floor triangles of Chemical Plant mode 2 / Vector
Laboratory mode 4, straight on for three seconds, turning every second): 20 hits and 14 starts inside a face a
step, four times the casts of play. The second is the chain of section 3f, the bodies of a real match.

The answers are the same to the bit: the chain's and the 1,000 single steps' outputs byte for byte (993 of 1,000
the game's in every field, as before), and every body's whole state after each of 4.32 million steps on all 72
map files (1,000 bodies of 60 steps a file; there about 100 -> 71 us a step). The walk grid of Chemical Plant
mode 2 built with its tick passes on the shapes world has the same 3.07 million links to the byte and takes 96.6 s
on 4 CPUs (132.2 s before; 88.0 s on the placed triangles). After the change the closest-point loop is about 60 %
of a step in play (19 calls, 10 of 16 us): what is left is the exe's own work.

### 3h. A long replay, and two things it found in the port (PROVEN)

A second, longer trace of a replayed match (16 bodies, 220,994 whole steps in 2,421 chains, 116 jumps: 1,612 steps in
the air), every step through the port on the shapes world, each from the game's own state and in chains
(`run_each_jumps2.sh`, `offsteps_jumps2.py`):

| | every field the game's | first air steps of a jump off | chains the game's bits end to end |
|---|---|---|---|
| before | 219,482 (99.32 %) | 27 of 155 | 2,213 of 2,421 |
| after the two fixes | 219,868 (99.49 %) | 1 of 155 | 2,257 of 2,421 |

- **The jump's sideways step makes the hit's horizontal normal unit by its largest component, with no least
  length** (`step_jump_forward` 0x7CB90, inlined at 0x7CC93..0x7CDCB: each component over the largest's size, then
  over the length; (1, 0, 0) only for a vector with no component over 0). The port used the ground steps'
  `normalized_safe` 0x7AA50, which gives its fallback under a squared length of 1e-5. A jump's first air step does
  not rise: the capsule sweeps while it still stands on its floor, and on a box's top the contact's normal is up
  but for rounding (about 1e-5 across). The game slides by that rounding's direction, or passes when it points
  away; the port slid along x. With the game's own walk vectors fed in, the kept body is the game's feet within
  1 cm from the press to the landing in 104 of 115 recorded jumps (78 before; short jumps 91 of 99, 65 before).
- **A body that met an enemy's never skips its step again.** `can_skip_player_step` 0x7C7E0 also asks that +205h
  `m_colliding_with_player` is clear (0x7C892); the world sets it on both bodies of an overlapping enemy pair
  (`bullet_physics_world::tick` 0x75811, 0x75848) and only the controller's `initialize` clears it. The port had no
  such flag and skipped 355 steps the game took (their feet under a millimetre apart in 340).
  `Controller::colliding_with_player` is the world's write; the bodies' pushing each other apart is not ported.

What is left (1,126 steps, 0.51 %): 504 landings whose only difference is the surface normal kept of two contacts
at one fraction (the exe's order of objects and triangles, not ported), and steps at walls and edges of the same
kind; 22 steps in the air.

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
