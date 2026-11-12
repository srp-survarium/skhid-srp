# Bullet's convex cast as the game calls it (0.26g)

Every sweep of the walking body (`bullet_character_controller`, `player_bots_mapdata.md`, the walk tick's port
`crates/mapdata/src/controller/tick.rs`) ends in Bullet's `btCollisionWorld::convexSweepTest`. This note is that
path read in the exe: which caster runs, with what margins, and what it answers at a contact that is already
there at the start. Tags: PROVEN, LIKELY, GUESS. Addresses are image VAs (build 2777, base 0x10000). Written
2026-11-12. The port's `Caster::Bullet` (`crates/mapdata/src/controller.rs`, A325) already follows the rules of
section 2; this note confirms them, proves two it had as LIKELY or did not have, and names what is still not
Bullet's own.

**The players' exe runs this code as shipped**: between the stock exe and the revision-56 exe no byte differs
in `btCollisionWorld` (0x9FC00..0xA4000), the triangle mesh shapes and their BVH (0xA7900..0xA9700),
`btRaycastCallback` and `btContinuousConvexCollision` (0xB8400..0xB9A00), the simplex solver, the GJK pair
detector and EPA (0xD1000..0xD9000). In the controller two places differ: the step's epilogue 0x7C7D7 (a NaN
position is put back; finite steps are the original's) and step_down's free fall 0x7E6E6..0x7E75C (patch 72:
gravity for the step's own dt; whole 50 ms ticks are the original's). PROVEN.

## 1. The path

| code | what it does |
|---|---|
| `btCollisionWorld::convexSweepTest` 0xA36E0 (stack: world, cast shape, &from, &to, &callback) | builds a `btSingleSweepCallback` (ctor 0xA3420) and walks the broadphase. **The allowed penetration is 0**: the exe has no such argument, 0xA34E8 stores 0 for all 13 callers |
| `btSingleSweepCallback::process` (ends 0xA36D8) | stops once the callback's closest fraction is 0; asks the callback's **`needsCollision`** for the object's proxy; then `objectQuerySingle` |
| `btCollisionWorld::objectQuerySingle` 0xA14E0 (cdecl, 8 args) | a shape of type under 20 (a box, sphere, cylinder, capsule of the map): **`btContinuousConvexCollision`** (vtable 0x9E4E80) with a Voronoi simplex solver and the GJK/EPA penetration solver; a hit with `normal.length2 > 1e-4` and a fraction under the callback's closest goes to `addSingleResult` (normal in world space). Type 21 (a triangle mesh): a `BridgeTriangleConvexcastCallback` (ctor 0xA29F0, vtable 0x9E4820) through `btBvhTriangleMeshShape::performConvexcast` 0xA8380. A compound's children and the other concave types: not read |
| `BridgeTriangleConvexcastCallback` ctor 0xA29F0 | the triangle's margin is **the mesh shape's `getMargin()`** (vcall +28h): 0 for every map mesh (`collision_world.md` §3, read from the game's memory) |
| `btTriangleConvexcastCallback::processTriangle` 0xB8670 | a `btTriangleShape` with that margin; **`btContinuousConvexCollision`** again; a hit with `normal.length2 > 1e-4` and a fraction **under** the fraction the callback began this object with goes to `reportHit`. That fraction is not updated by the answers (the return value is dropped at 0xB8842) |
| `BridgeTriangleConvexcastCallback::reportHit` 0xA2A40 | a hit **at or under** the result callback's closest now goes to its `addSingleResult` with the triangle's part and index: two triangles with the same fraction both arrive, in the BVH's order |
| `btContinuousConvexCollision::computeClosestPoints` 0xB8900 | a `btGjkPairDetector` of the two shapes with each shape's own margin (`getClosestPoints` 0xD6CD0) |

## 2. `btContinuousConvexCollision::calcTimeOfImpact` 0xB9320 (PROVEN, read whole)

Constants: FLT_EPSILON 1.1920929e-7 (0xA2DA0C), the contact band **0.001** (0x91A8F0), 64 rounds.

1. Nothing moves: no hit. No closest points: no hit.
2. `dist` = the GJK distance (+ the allowed penetration, 0); `n` = the normal on the world's shape toward the
   cast shape. **The move's part along `-n` at most FLT_EPSILON: no hit.**
3. While `dist > 0.001`: that test again; `lambda += dist / (the move's part along -n)`; no hit when `lambda`
   is over 1, under 0 or not past the last one; the shape moved to `lambda`, closest points again (none: no
   hit); over 64 rounds: no hit.
4. A hit: fraction `lambda`, the last `n`, the last closest point on the world's shape.

So:

- **A contact already there at the start** (a gap of 0.001 m or less, or an overlap) is fraction 0 when the move
  goes into it by more than 1.19e-7 m over the cast, and no hit at all otherwise.
- **The cast stops as soon as the gap is 0.001 m or less.** Against a flat face the first step lands on it;
  against an edge or a round feature the normal turns on the way and the cast ends up to a millimetre short.
- **A cast that gives up is no hit**, not a hit.
- The normal handed on is normalised in x87 at the thread's precision (24 bits in the running game:
  `collision_world.md` §6); everything else is scalar single precision.

Not read: the GJK pair detector 0xD6CD0, the simplex solver, EPA (their rounding decides the last 1e-5 of a
normal, and the normal of an overlap), the BVH's order of triangles.

## 3. Run: the walk tick on the exe's own cast

The Unicorn harness of the walk tick (the exe's `player_step` on made worlds) answered `convexSweepTest` with
exact geometry of its own. Run again with every sweep answered by the exe's own `objectQuerySingle`,
`processTriangle`, `calcTimeOfImpact` and GJK (only `performConvexcast` stood in: the triangles of the sweep's
box, in file order), at the game's control word 0x007F and at 0x037F:

| suite | outcome against the stand-in cast | largest difference of the feet |
|---|---|---|
| walk (flat, a 0.5 m step, a 1.0 m step, a wall) | the same | 0.03-0.09 mm on flats (the body rests that far inside the floor), 4.5 mm on the step's edge |
| crouch and ceilings (5) | the same | 3-5 mm at an edge or the stand test's tick, else under 0.04 mm |
| slopes (13) | the same branch in each | 0.1-0.2 mm on ramps, 4 mm on a pitch's rim |
| the air branch (jump on a floor, at a wall, onto a box) | **differs: see below** | 0.03-0.10 m |

Between the two control words, both on the exe's own cast: 1e-7 m on flats, 0.5-5 mm on edges and rims, the
same outcomes. A millimetre is what "the same as the game" can mean at an edge until GJK itself is ported.

**A level tick of the air branch on the ground is slid aside or lost.** The air branch's forward sweep
(`step_jump_forward`, the from-to sweep 0x87B60, whose callback 0x87B40 filters nothing) casts the full capsule,
which rests on the floor. GJK's normal of the floor is off straight up by about 1e-5, so a level move of 0.17 m
goes "into" the floor by a few 1e-6 m, more than FLT_EPSILON: a hit at fraction 0, and the tick's move goes
through the wall slide. On level floors of 24, 4 and 1 m triangles, five headings, two starts: 4 to 20 of 20
such ticks were deflected or lost in each of 30 runs. PROVEN for the exe's code under Unicorn; LIKELY in the
game (the deciding quantity is GJK's rounding; Unicorn's SSE arithmetic is exact).

Whether a real jump has such ticks is not known yet. The controller's jump flag (+2C70Ch) is set by
`base_player::jump` 0x2D7920, whose only caller is `jump_logic_state_start::on_jump_event` 0x3434A0 (an event of
the jump clip), so the prepare runs the walk branch. The candidates are the event's own tick and the ticks after
a landing that comes before `end_jump` 0x7F050 (a jump up onto a ledge lands while the clip is still falling).
When the client calls `end_jump` is not read.

## 4. Against our code

| the game | `Caster::Bullet` (`controller.rs` `advance_bullet`) |
|---|---|
| the contact band 0.001, the FLT_EPSILON test at the start and on the way, 64 rounds | the same rules (A325) |
| the triangle margin 0 | 0, tagged LIKELY in its doc: PROVEN here |
| the distance and the normal are GJK's | an exact distance to the face: equal on faces, up to a millimetre apart on edges; **a start contact on a flat floor never fires in the port and does in the game** (section 3) |
| an overlap at the start: the penetration solver's normal and witness | `StartOverlap::FaceWitness` (A329): a rule fitted to one case |
| the map's boxes, spheres, cylinders, capsules are cast against as Bullet's own shapes | faces of the export's triangles (`collision_world.md` §5.2) |
| equal fractions arrive in the BVH's order | every face's contact, in the tree's own order |

Exactness of the last millimetre needs Bullet's GJK pair detector, simplex solver and the shapes' supports in
Rust, each checked bit for bit against the exe's own function in Unicorn. Not done.
