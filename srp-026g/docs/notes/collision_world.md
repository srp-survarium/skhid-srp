# The game's own collision world (0.26g), read out and laid against the map export

The bots' maps (`player_bots_mapdata.md`) are rebuilt from the game's `resources.db` by our rules. This note is
the check of those rules against the thing itself: the Bullet world a running game built for a map, read from
the game's memory, compared with the export object by object. Tags as elsewhere: PROVEN, LIKELY, GUESS.
Addresses are image VAs of the 0.26g exe (build 2777, base 0x10000). Written 2026-11-12, for the game at
patch revision 56 (the exe and the game data the players have).

**Result in one line:** on the 16 Player vs Bots projects (levels 01-08, day, modes 2 and 4) every static body
of the game is one export placement in the same place with the same vertices and the same walker behaviour;
the export differs only where the archive reader does not follow a folder hard link (71 walker placements
missing or in another shape, 94 bullet-only ones missing) and in how primitives are scaled and rounded
(section 5).

## 1. How the game builds the world (the code)

| step | code | what it does | tag |
|---|---|---|---|
| the world | `bullet_physics_world` (vtable 0x9E2EBC, ctor 0x74950), `initialize` 0x74B20 | +24h allocator, +28h collision configuration, +2Ch dispatcher, +30h broadphase, +34h solver, +38h soft body info, **+3Ch the `btSoftRigidDynamicsWorld`** (ctor 0x9AC50), +48h..+5Ch the world box | PROVEN |
| a library | `collision_shape_cook::translate_query` 0x82590, `on_collision_sources_loaded` 0x829E0 | one shape per `lib_name` (`<model>.model/{collision,walker_collision}#[sx][sy][sz]`): its `exported_primitives`, the model's `settings`, `vertices`, `indices`, `face_data` | the cook's material loops PROVEN (below); the request list as in the 0.10b source, LIKELY |
| a primitive | `create_bt_primitive` 0x7A1F0 (type EDX, dims ECX; jump table 0x7A374) | 0 `btSphereShape(dim.x)`, 1 `btBoxShape(dim)`, 2 `btCylinderShape(dim.x, dim.y, dim.x)` (0.10b took `dim.z`), 3 `btCapsuleShape(radius dim.x, height dim.y)`; `dim` is the primitive's `scale` | PROVEN |
| one primitive at the origin | `create_primitive_shape` 0x7A390 | the primitive itself (its rotation is not used), `setLocalScaling(library scale)` | PROVEN (the call); the "one, at the origin" test as in 0.10b, LIKELY |
| several | `create_compound_shape` 0x7A460 | a `btCompoundShape` (60h bytes, ctor 0x8A230); a child per row with `create_rotation(rotation) * create_translation(position)` through `from_vostok` 0x74470, `addChildShape` 0x8A3E0; then `setLocalScaling(library scale)` **on the compound** | PROVEN |
| the mesh | `create_btBvhTriangleMeshShape` 0x7A6E0 | a `btTriangleIndexVertexArray` (vtable 0x9E3248) over the file's own vertex and index bytes (stride 12 and 12), **scaling (sx, sy, -sz)**: the vertices stay in the map's frame, the scaling turns them into Bullet's; the shape `btBvhTriangleMeshShapeResource` (70h bytes, vtable 0x9E3284, ctor 0x79EE0), quantized BVH built at once (0xA8A40) | PROVEN |
| the materials | the cook at 0x82BF0..0x82C9A (primitives) and 0x83251..0x832A5 (triangles) | a face data word = **(the game material's byte +68h, its `walker_behaviour`) << 14 \| the material id**; 0xFFFF for a shading group the model's settings do not bind | PROVEN |
| a face's walker behaviour | `bt_static_rigid_body::get_walker_behaviour` 0x356D00 (vtable 0xA1F258 +14h), the vcall of the walk tick's down sweep | the word of the triangle (shape's +10Ch) or of the child shape (+110h), **>> 14**: 0 by slope, 1 walk, 2 dont_walk; 0 for 0xFFFF or no face data. `get_triangle_material` 0x3570A0 (+10h) is the word & 3FFFh | PROVEN |
| the body | `create_static_rigid_body` 0x356D60, `static_collision::insert` 0x2F8850, `bullet_physics_world::add` 0x758B0 | a `btRigidBody` of mass 0 at the placement's transform, added with the placement's `cgroup` and `cmask` (the world's vcall +4Ch); only groups with bit 2 or 8 widen the world box | PROVEN (`add`); the rest as in 0.10b, LIKELY, and the dump agrees |

The 0.10b engine source reconstruction has the same functions; each row above marked PROVEN was read in the
0.26g exe. Names and calling conventions are in `tools/re-kit/v026g` (`functions.tsv`, `abi.tsv`,
`abi_inputs.tsv`, `vtables.tsv`).

## 2. The layouts the reader walks (PROVEN: read in the exe, and every field checked by the values it gives)

| object | fields |
|---|---|
| `btCollisionWorld` | +08h object count, +10h `btCollisionObject**` |
| `btCollisionObject` | +10h world transform (3 rows of 4 floats, then the origin), +C8h broadphase proxy, +CCh shape, +D4h root shape, +D8h flags (1 static, 10h character), +F4h internal type (2 rigid body, 4 ghost), +F8h the vostok object (`bt_static_rigid_body`, `bt_dynamic_rigid_body`, `bt_animated_rigid_body`, the character controller, a ghost object) |
| `btBroadphaseProxy` | +04h group (u16), +06h mask (u16), +10h and +20h the box |
| `btCollisionShape` | +04h shape type (0 box, 8 sphere, 10 capsule, 13 cylinder, 21 triangle mesh, 31 compound), +08h the vostok `bt_collision_shape` (118h bytes: +108h the Bullet shape, +10Ch triangle face data, +110h shapes' face data) |
| a convex primitive | +10h local scaling, +20h implicit dimensions (the half extents less the margin, scaled), +30h margin, +40h up axis (capsule, cylinder) |
| `btCompoundShape` | +10h child count, +18h children (50h each: transform, +40h shape, +44h type), +50h local scaling |
| `btBvhTriangleMeshShape` | +0Ch margin, +30h mesh interface (+10h scaling, +24h part count, +2Ch parts of 20h: triangles, index base, index stride, vertices, vertex base, vertex stride, index type, vertex type), +60h face data |

## 3. What is in a match's world

| objects | Bullet group / mask | met by the walking body (group 4, mask 2)? |
|---|---|---|
| the project's `collision_objects`: one static body each | as placed: 2 / 4 walker, 8 / 16 bullets, 10 / 20 both | group & 2 |
| a player: the character controller's ghost, a capsule (0.38, 0.52), margin 0.04 | 0x824 / 3 | no: `4 & 3` is 0, so neither the broadphase nor a cast pairs two players' capsules (LIKELY that nothing else does: the tick's own handling of bodies is not read here) |
| a player's hit body: a compound of 20 shapes (`bt_animated_rigid_body`) | 64 / 17 | no |
| a battery (`bt_dynamic_rigid_body`, a box) and its use ghost | 0xA18 / 0x418, 0x880 / 256 | no |
| sensor ghosts (zones, anomalies, artefacts) | 1 / 32, 128 / 256 | no |

In all 16 projects the static bodies in the world 30-120 s after the start are exactly the project's
placements: nothing static is added, removed or moved by then. Later in a match was not sampled.

Margins (from memory): **every triangle mesh 0**; a box and a cylinder min(0.04, a tenth of the smallest half
extent of the unscaled primitive), kept when the library scale shrinks the shape; a capsule 0.04; a sphere is
its radius. Bullet casts against the shape with its margin: a box's and a cylinder's edges are rounded by it.

## 4. The reader and the comparison

`scripts/collision-world/` (its README has the commands): `bw_run_replay.py` starts the game on a hidden
desktop with a replay of the project, waits until the world's object count has settled, walks the world from
outside (`ReadProcessMemory`: nothing is written to the game and none of its code is changed) and saves every
byte it touched as a capture; `bw_diff.py` lays a capture against the export (`mapdata-flat`).

For each pair (a game object and the export placement at the same place with the same group and mask; several
at one place are told apart by their shape): the rotation matrices, every mesh vertex placed in the map frame,
the triangle indices, every face's walker behaviour (the game's word >> 14 against the export's byte), and each
primitive as the exact convex shape Bullet casts against (margin included) against the export's triangles for
it, by their support functions over 642 directions.

## 5. Results: 16 projects, revision 56

| | |
|---|---|
| game objects | 22,199: 21,247 static bodies and 952 run-time objects (section 3) |
| export placements | 21,247: every one paired, none left over on either side |
| rotation | largest element difference 1.8e-7 |
| meshes | 10,696 pairs, 6,152,908 vertices: **0 meshes with a vertex over 1 mm off, 0 with other indices** |
| walker behaviour | 9,487,606 triangles and 218,940 primitives: **0 differ** |
| empty export libraries the game has a shape for | 417 placements: 252 walker ones the stand-in gets right, 9 it gets wrong, 62 walker-and-bullet ones with nothing, 94 bullet-only ones with nothing (5.1) |
| primitives | 278,040; 498 differ by 0.05 m or more (below) |

So PROVEN, for these projects: the placement rule (`player_bots_mapdata.md` §2), the mesh scale rule
(the name's `#[sx][sy][sz]`, nothing else), the collision groups, and **the walker behaviour values** (LIKELY
until now).

### 5.1 Folder hard links (the "missing `walker_collision`")

A model's `collision` or `walker_collision` can be a **hard link to another folder** in the archive's index
(58 such links in the stock `resources.db`: 12 `walker_collision`, 11 `collision`, 7 `hq_collision`, the rest
render folders). The engine reads the target's files under the link's path. Our reader (`resources-db`,
`fat.rs` `visit_node`) resolves a hard link as a file, so a linked folder lists as one entry of kind folder
and the library comes out empty. The map builder's stand-in (an empty `walker_collision` is the model's own
`collision`) was a guess at this:

| model | placements (16 projects) | the stand-in |
|---|---|---|
| `flora/fruit/fruit_tree_01`, `_02`, `_03`, `_05`; `level/03/anomal_tree_01_` .. `_04_`; `props/street/bench_21b` (`walker_collision`) | 252 | right: the game's walker shape is the model's own `collision` (the same vertex buffer in memory) |
| `vehicle/azlk_damaged` (`walker_collision`) | 2 (Rudnya) | **wrong**: the game walks on 3 plain boxes and a 12-triangle mesh; the own `collision` is 36 primitives and a 284-triangle mesh |
| `props/tool/weel_sport_02` (`walker_collision`) | 4 (School) | **wrong**: the game has 4 boxes, the own `collision` one primitive |
| `props/electric/tv_working_02` (`walker_collision`) | 3 (Vostok Radar Station, Cologne) | **nothing**: its `collision` is a link too; the game has 4 boxes |
| group 10 (walker and bullets) `collision`: `level/05/brick_debry_burned_01`, `_03`, `brick_debry_part_02` (rubble 0.5-1.2 m across, 0.5-1 m high), `level/05/bridge_big_burned_01` (1.6 x 6.4 x 7.0 m); `flora/poplar_04_sux`, `poplar_06_sux` (trunks 0.5 m across, 8.6-12 m high) | 24 a mode on Tarakanovsky Fort, 7 a mode on School | **nothing**: no walker surface and no bullet surface in the export |
| group 8 (bullets) `collision`: `props/electric/cable_hank_black`, `props/light/alarm_red`, `bonefire_01_on`, `barrel_stove_01_fire`, `tv_working_02` | 94 | nothing: sight and pierce do not see them |

**The fix, tried:** in `visit_node`, a hard link whose target is a folder lists the target's children under the
link's own path. With it (a private build of the exporter) no library of the 16 projects is empty, and the
comparison finds every one of those shapes equal to the game's (0 meshes off, no shape mismatch). Then the
stand-in has nothing left to stand in for.

### 5.2 Primitives

Bullet's shapes against the export's triangles, 278,040 primitives:

| cause | how far | where it shows |
|---|---|---|
| a compound's children under a **non-uniform** library scale: `btCompoundShape::setLocalScaling` multiplies each child's origin by the scale and each child's own local scaling by it **in the child's own axes**; the export scales the points in the library's axes. Equal when the scale is uniform or the child is not turned | up to 4.3 m (the game's shape past the export's) and 5.7 m (the other way) | 370 primitives: `props/garbage/concrete_plita2` (Mamayev Kurgan, 85 a mode in each of the walker and the bullet library, the export 0.05-0.11 m past the game's); `level/02/fence/fence_small_opaq_01#[4.55][7.11][4.55]` and `#[3.88][2.16][3.26]` (School: the game's wall is 24 x 9.8 m, the export's 15.3 x 15.4 m); `light_conductor_01#[1.00][1.95][1.00]` (Cologne); `plywood_01#[1.24][1][1]` (Rudnya, 0.12 m) |
| a cylinder under a non-uniform scale: Bullet's radius is the x half extent for both x and z (LIKELY: Bullet's own support function, not read in this exe) | up to 0.11 m | bushes (`flora/vine/bush_small`, `bush_wide`) |
| round shapes as 12 sides and 6 rings, inscribed | up to 5 % of a sphere's radius and 3.4 % of a cylinder's: 0.25 m on the ball tanks of Chemical Plant (`level/04/balls`, r 5 m), 0.06-0.11 m on Vostok Radar Station's hangar domes, 0.03-0.05 m on cisterns and boilers, 0.01-0.05 m on capsules (tree trunks) | 114 primitives over 0.05 m |
| the margin's rounding of a box's and a cylinder's edges | under 0.03 m (a corner), 0.017 m (a rim) | everywhere; the export is the sharp shape |

### 5.3 Against the recordings

- The export audit of the owner's 14 matches (bodies standing on level ground against the export under them)
  had 1,158 spots "0.06 m+ above every exported surface". Probed in the game's own world of each match's
  project: **0 are a map difference**. At 671 the game's world has the same gap under the body (it did not
  stand on the static map there), at 487 both have a surface at the feet's height beside the centre (a thin
  thing under the capsule's rim).
- Of the 1,025 stuck episodes of those matches, **0 began within 1 m of a walker object the export lacks
  (5.1) and 0 within 1.5 m of a primitive 0.2 m or more off (5.2)**.

So the map differences of this section are real and explain none of the navigation faults counted in those
matches: those are in the movement and in the map builder's rules, not in the geometry.

## 6. Float control words at the walk tick (PROVEN)

`vostok::setup_floating_point_control` 0xFE5E0, on unless its switch (0xA86BD0, made from the literal "1") says
otherwise: `_controlfp_s(_PC_24, _MCW_PC)`, `_controlfp_s(_RC_NEAR, _MCW_RC)`, MXCSR's rounding bits cleared,
`_controlfp_s(_DN_FLUSH, _MCW_DN)`, MXCSR `or 0x8000`. Its two callers are the engine's thread procedure (0xE36ED:
every thread the engine spawns, at its start) and the core's start on the main thread (0xE4298). So the engine's
threads run with **x87 control word 0x007F (24-bit precision, round to nearest) and MXCSR 0x9FC0 (flush to zero,
denormals are zero)**.

Measured on the running game from outside (`fpu_sample.py`, `fpu_eip.py`; a sample suspends one thread for one
`GetThreadContext`): the 71 engine threads show 0x007F / 0x9FC0, the driver's, the audio's and the system's
threads 0x027F / 0x1F80. Of 355,000 samples of the engine threads, 84 had the thread's own instruction pointer
inside the walk tick or the cast (the character controller 12, `btCollisionWorld` 3, the continuous convex cast
6, the simplex solver, GJK and EPA 63): **all 84 at 0x007F / 0x9FC0**, on one thread. 26 samples were at another
word (0x027F, or 0x0C7F as a float-to-int helper sets for one instruction), none inside those ranges.

For a port:

- An x87 add, subtract, multiply, divide or square root at 24 bits is rounded to single precision as it is made:
  for results in single precision's exponent range it is `f32` arithmetic, step by step. The walk tick's port in
  plain `f32` is the right model of the exe's x87 chains; arithmetic in double rounded at each store would not be.
- Under flush to zero and denormals are zero a result under 1.18e-38 is 0 in the game and a denormal in Rust.
- A Unicorn run of the game's code must set the control word to 0x007F (Unicorn's default is 0x037F). On the
  exe's own cast the two words give the same outcomes and differ by 1e-7 m on flats and 0.5-5 mm at edges
  (`bullet_cast.md` §3).

## 7. Not covered

- Weather variants, mode 3 and 5 projects and the ported maps: not captured (recordings exist for them).
- A world later in a match (after the first two minutes); doors and breakable things, if any act on the
  walking body.
- Bullet's own cast against these shapes: `bullet_cast.md`.
- `hq_collision` (7 folder links): nothing here uses it.
- A capsule under a library scale: none met in these projects.
