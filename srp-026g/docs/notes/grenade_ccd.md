# Grenades under the ground (patch 105)

- **Reports:** the owner (2026-11-05): "many reports that grenades often fall under
  collision". Discord #general-ru, 2026-11-04: egredr (09:38, "sometimes it goes into
  the textures"), sanguis_extremis (21:06, "grenades often fall under the textures when
  they hit the ground; thrown at a wall it works"). No map, spot or video given; the
  #bugreports forum has none.
- **Not ours:** stock 0.26g. No Skhid patch touches the grenade's body, the physics
  step or Bullet's collision (the ragdoll is visual only; patch 72 changes the player
  controller's fall, the bullet bounds hook the bounds cache alone); the constants
  below are the original exe's.
- **The grenade's body** (`grenade_core::load` 0x303890; the RGD-5 and the RDG-M smoke
  grenade are both grenade configs, LIKELY both this class): a
  capsule, radius 0.05 m, height 0.1 m (create_primitive_shape 0x7A390, type 3, at
  0x3038DB..0x303914), mass 0.3, friction 0.5, restitution 0 (Bullet's defaults);
  thrown (`grenade_core::throw_grenade` 0x303C10) into the physics world as group 818h,
  mask 8 (it meets the level's collision bullets hit, group 8 mask 10h; not the walker
  mesh), with its velocity and no spin. Continuous collision: m_ccdMotionThreshold 0.03
  (+104h, 0x303A0B), m_ccdSweptSphereRadius 0.01 (+100h, 0x303A1C). PROVEN (code).
- **The step** (`bullet_physics_world::tick` 0x75550): stepSimulation(0, 0, 1/60),
  which zeroes Bullet's time left over, then stepSimulation(dt, 3, 1/60), dt the tick's
  (or a split tick's part) ms / 1000. PROVEN (code).
- **Why it sinks:** vostok's `integrateTransforms` (0.10b source,
  `sources/bullet/BulletDynamics/Dynamics/btDiscreteDynamicsWorld.cpp`) sweeps the swept
  sphere from the body's centre to where it would be; on a hit it moves the body to the
  sphere's time of impact and caps its speed at threshold / step (1.8 m/s), still into
  the surface. With a 0.01 m sphere in a 0.05 m capsule the capsule then sits at least
  4 cm into the surface. The level's collision is a triangle mesh with nothing behind
  it, so the contact that should push the capsule back often pushes it out the other
  side, or the next step carries the centre past the triangle. A wall that is a closed
  box catches it on its other face, the ground has nothing under it (LIKELY why the
  report says walls work: the simulation's one-sided wall loses grenades too).
- **Simulated** (`client-patches/grenade_ccd_sim.cpp`, Bullet 2.79 of the 0.10b source,
  the game's world, step and body; 1000 throws a case at 17 m/s, the RGD-5's
  throw_power):

  | case | stock (0.01 m) | patch 105 (0.04 m) |
  |---|---|---|
  | flat ground, 4 m triangles | 15.0 % under | 0 |
  | rough ground (+-0.3 m), 1 m triangles | 17.3 % | 0 |
  | the same, thrown down (-70..-20 degrees) | 22.5 % | 0 |
  | a short lob (6 m/s) | 11.4 % | 0 |
  | a one-sided wall 4 m ahead | 10.1 % under the floor, 10.9 % through the wall | 0, 0 |

  0.03 m: 0 to 0.7 % a case; 0.05 m (the capsule's radius): 0.
- **The fix:** the swept sphere 0.04 m (the 0.04 of 0xA2D7DC, .rdata, for the 0.01 of
  0xA2D774 at 0x303A13; its base relocation stays): Bullet's rule is a sphere inside the
  shape, and 1 cm short of the capsule's surface it never touches the ground the grenade
  lies or rolls on. The capsule then stops at the surface it hits.
- **What else changes:** stock, the sunk capsule was pushed back out with speed of its
  own; without it a grenade rolls on less after it lands (simulated: 2.2 m on rough
  ground, was 4.6 m; 2.7 m on flat, was 5.0 m; its pops after landing 0.17 m high, were
  0.35 m). The flight before the first hit is the same.
- **Not changed:** the threshold 0.03 and the speed cap; the trap; the other body with
  0.01/0.01 (made at 0x300468, a box: not a grenade).
- **Simulation:** what a match computes changes (every game runs each grenade's flight):
  a `next` row in `crates/session/src/v026g/simulation_changes.tsv`.
- **Checked:** selftest check 105 (`test_grenade_ccd`: the original's capsule and CCD
  stores, the 8 bytes, the constant, the relocation, no branch inside) and the late-join
  section's site and relocation checks; the simulation above. **Not run in a game.**
- **In game:** any map, open ground (Rudnya's or Vostok's fields, a concrete yard):
  throw an RGD-5 or a smoke grenade far at a flat spot, then straight down at your
  feet, ten times each; stock, one or two in ten drop through and explode or smoke
  under the ground. With patch 105 none should.
