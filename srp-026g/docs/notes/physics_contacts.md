# The contact manifolds go with the world (patch 121)

## The split it fixes

Live match 0x54417103 (rev52, 2026-11-07, 16 players), 22:54:32 UTC: the hash reports of
135450 ms split seven ways among fifteen games (`no majority`), thirteen games were resynced
from 139400 ms, two agreed again by themselves. The worlds the server kept (`snapshots/`) show
what differed:

- at 138400 ms, player 5's world (CRC 9eb0f7be) and player 10's (101cf201) have every player
  block, the bullets, the statistics, the objects and the effects byte for byte the same; only
  the rules section differs, in one free battery's record (`victory_item_core::serialize`
  0x301470, snapshot_rules.md section 2.4): its `m_transform` and both velocities, about 5 mm
  apart, the battery still active (activation 1, deactivation time 0.617);
- at 134550 ms (player 5's world, checked) that battery is falling at 7 m/s from its
  carrier's death point; it lands at about 135.2 s, where the games part;
- the earlier split of the same match (85150 ms, six CRCs) is the same: two batteries falling
  at the same spot, and at 88100 ms the worlds differ in their two records alone.

Over the 5.9 hours of server logs kept that day (17:03-22:57 UTC, 139 matches, 39 with kept
pairs of disagreeing worlds), 15 of the 227 disagreeing ticks with two kept worlds differ in
battery records alone (6 matches) and caused 36 resyncs (about 6 an hour); 118 differ in
player blocks alone (other causes, not looked at here); no tick differs in a grenade's record
alone (two objects-only ticks are a different count of objects).

## The cause

A world restores a free battery's body completely: victory_item_core's reader (0x301660) sets
its transform through set_transform (0x3009B0 -> the body wrapper's 0x3578B0: the world and the
interpolation transform, updateInertiaTensor 0x8C2E0), both velocities (0x3576C0, 0x357750),
the deactivation time (+0E8h) and the activation state (+0E4h). A grenade's record holds the
same (srp-validate `objects.rs`).

What no world held is Bullet's (2.79, as in the 0.10b sources) persistent contact manifolds:
for each overlapping pair of bodies the dispatcher (`btCollisionDispatcher`, made by
`bullet_physics_world::initialize` 0x74B20 at +2Ch) keeps a btPersistentManifold (0x4B0 bytes:
four btManifoldPoint of 120h at +10h, the bodies at +490h/+494h, the count at +498h; the
dispatcher's array at +0Ch/+14h) whose points live from step to step: each step refreshes them,
replaces the nearest with a new one, keeps the four that span the most area. The physics world's
own save and load of a world are empty (`bullet_physics_world`'s vtable +40h `ret 4` 0x75AF0,
+44h `ret 8` 0x15A4B0; `game_world_core::serialize` 0x2D26D9, world apply 0x2D2C18), so a game
that loaded a world (a rollback, which every game does many times a second; a resync; a joiner;
a replay's seek) stepped a battery from the points its dispatcher had then, those of a later
moment or none, where a game that stepped straight on had those of the world's moment. A battery
in flight has none; one landing, bouncing or settling has them, and every game parted. Warm
starting does not come into it: the game's solver mode is SOLVER_SIMD alone with two
iterations (initialize 0x74E9D, 0x74EA7), so the points' impulses are never read; the points
themselves are (`contacts_sim.cpp` with every impulse zeroed at each tick settles as stock).

A second, smaller part: solveGroup (0x9A8B0) solves the manifolds in the order the dispatcher
made them (the island manager sorts them by island with a quick sort, not stable), and a load
does not keep that order either: two batteries touching each other, or one touching two
fixed bodies, are solved in a different order.

The near callback is vostok's own (0x8F860): it passes the body nearer the world's origin
first, keeps that order at the pair's +0Ch (m_internalInfo1), and makes the pair's algorithm
again (its manifolds released) when the order changes (0x8F906). The collision configuration
(0x8EEE0) is vostok's too: its own algorithms for its custom shape types, Bullet's for the rest
(0xAB430).

## The fix

Patch 121 (`physics_contacts`, Rust, seven lohooks) writes the manifolds into the world's
physics section, which the history item, the hash, the upload, the resync and the wire reader
already carry (physics_world_history_item 0x324410, the reader at 0x324FB6; patch 98's heap
buffer at item +0BBF08h), and puts them back at every load:

- **The save** (`physics_contacts.saved`, 0x2D26DB): every manifold with points whose pair has a
  moving body (filter group bit 800h: a battery 0A18h, a grenade 818h) and only that manifold (a
  compound's children share their pair: those stay the game's), as a record of
  `srp_validate::world::Contacts`: its bodies' keys, whether the near callback passes its body
  0 first (the pair's +0Ch), its points (145 bytes each: every field but the user pointer, the
  vectors' w and the constraint rows). A body's key is what the world sets the same in every
  game: a moving body's origin (the world puts it there), a fixed body's broadphase box and
  filter; -0 as 0. The records are in key order, at most 32.
- **The load** (`keeper.world_loaded`, order 2, 0x2D2D5D, which every load passes): those
  manifolds of the game emptied, the world's records waiting.
- **The step** puts each record into its pair's algorithm as the game that saved the world had
  it:
  - before a pair's processCollision (`physics_contacts.near_process`, 0x8F9C2, the algorithm
    made or kept): into the pair's manifold when the manifold's body order and the order the
    near callback passes the bodies in are the record's; else it is spent (that game's near
    callback made its algorithm again at this step, empty);
  - a manifold made in a processCollision (`physics_contacts.new_manifold`, 0x8F5B6; a
    btConvexConvexAlgorithm makes its own in its first; `near_entry`, 0x8F860, ends a pair's
    call): its record;
  - after the first collision detection since the load (`physics_contacts.detected`, 0x9B3E3,
    before calculateSimulationIslands): a record whose pair the near callback did not take (both
    asleep: needsCollision 0x8F770) gets the pair's algorithm in the record's order, the bodies
    awake while it is made (an algorithm that makes its manifold in its constructor makes none
    for two asleep) and asleep again after, and its points; one of the other order is made again
    (cleanOverlappingPair 0xB40E0, findAlgorithm 0x8F6E0); a record whose bodies are gone or
    moved is spent.
  - A record no manifold took yet is saved with the next worlds while its bodies stay where the
    world has them.
- **The solver** (`physics_contacts.solve`, 0x9A8B0): every solveGroup takes its manifolds in
  their bodies' key order (a stable insertion sort of the batch).

What a match computes changes (the order the solver takes the manifolds in, and every load):
a `next` row in `simulation_changes.tsv`. The world check (srp-validate `world::physics`, the
server's and the game's, patch 87) reads the section as the patch writes it, where it refused
any byte there. A world grows by 28 + 145 x points bytes a touching moving body: about 0.6 KB
for each battery lying on the ground, nothing for one carried.

Not covered: a body's CCD sweep (integrateTransforms) meets the other bodies in the order of the
world's body array, which a load does not keep either; two moving bodies within a sweep of each
other in the same step could still part. Not seen in the kept worlds; not fixed.

## Checks

- `client-patches/check_physics_contacts_026g.py` (Unicorn, the game's own configuration,
  dispatcher, pair cache, near callback and convex-concave algorithm on made-up bodies): a
  rollback, a joiner and two asleep bodies get the world's contacts in their processCollision,
  the world's bytes are the record, solveGroup gets one order; on the original exe, and on the
  build of main 164c3118a, the contacts of another moment (the check fails there).
- `client-patches/contacts_sim.cpp` (Bullet 2.79 of the 0.10b source with the game's world,
  step, solver settings, near callback and battery; the fix as above): a game stepped straight,
  its twin with rollbacks and a joiner from its world, 600 drops on rough ground (also a battery
  dropped on another asleep): stock, the twin apart in 599 and the joiner in 533; fixed, every
  one bit for bit, bodies and contacts; the fix without the solver order, 32 and 24.
- srp-validate's `world::physics` tests (the format, its refusals).
- Not run in a game; not replayed against the live recording (a replay needs the game).
