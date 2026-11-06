# Bullet cutoff after a late join

The original physics world grows its cached AABB when a collision body is
inserted, but does not shrink it when that body leaves. The cache is not
saved. Grenades and victory items can therefore extend the bounds in an
existing client while a client loading the same snapshot has smaller bounds.
Native bullet motion uses those bounds to remove out-of-bounds projectiles,
so the two clients can disagree about a bullet's lifetime.

`bullet_physics_world::add` (0x758B0) unions the body's AABB into world+0x48
when the collision group matches bits 0xA. That includes level groups 2 and
8, but also grenades (0x818) and victory items (0xA18). `get_world_aabb`
(0x728F0) returns this cache to `bullet::update_bullet_position` (0x32EE90).

The `bullet_bounds_static` hook at 0x758E1 excludes groups carrying the
transient-body bit 0x800 from bounds growth. All bodies still enter Bullet
and receive the original broadphase update; collision behavior is preserved.
The hook is NASM in the linked `.srpx` arena, with no additional state.

`check_bullet_bounds_026g.py` executes the native constructor, add/remove,
AABB union/getter, trajectory calculation and bullet cutoff. The original
code keeps a bullet below the level after a transient body has extended the
bounds, but removes it in a fresh world. The repaired code removes it in
both histories and preserves bullets inside the level. Controls cover
both transient groups, static level groups, repeated insertions, unrelated
groups, arguments, stack cleanup and callee-saved registers. Body insertion,
shape queries and broadphase update are fixture endpoints.

A private recorded-match experiment also reproduces the observed missing
projectile by changing only its bounds decision; eight complete native
serialized projectile records match the captured majority. The historical
object insertion that enlarged the old cache was not captured. This does
not establish the cause of every later resync or every missing-bullet case.
No recording or player snapshot is included in the repository.

This changes simulation results. The feature adds a `next` entry to
`simulation_changes.tsv`; the release must assign its revision and exclude
older recordings. The feature itself does not change `REVISION`.
