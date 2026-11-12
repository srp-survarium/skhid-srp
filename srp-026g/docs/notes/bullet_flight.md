# A bullet in the 0.26g client: its tick, its flight, what it hits, what it leaves a body

Read in the executable (virtual addresses, image base 0x10000; names in `tools/re-kit/v026g`) and the game data.
PROVEN where an address is given, unless said otherwise. Counted on the saved worlds of recorded matches read with
the repository's own parser: 59 recordings of bot matches (27,151 worlds, 151,739 bullet records of 106,722
bullets) and 37 of matches between players; the guns rebuilt for each match's content revision (40 to 50) with the
kit items' modifications; a bot is a place whose account id is 0, and their bots ran the code released then. Every
game computes a bullet in the lockstep. See also `player_bots_weapons.md`, `damage_model.md`, `pierce_line.md`,
`bullet_bounds.md`, `bullet_cast.md`, `weapon_fire.md`.

## What a bullet holds (`bullet::bullet` 0x32BA60; saved in every world)

Its id, shooter and weapon slot; **the current segment's** start position, start velocity and born time (a change
of course starts a new segment, `change_trajectory` 0x32EE20: born + floor(1000 t), life 0); the life time in the
segment, the flown distance (the steps' chords, all segments), the damage and pierce as they stand, the two damage
speeds, the largest damage dealt, the pellet number, the count of changes of course and of ricochets, and **the
ammunition's ordinal**: the index in the weapon's list of the kit's slots 8, 9, 11, 12, 19, 20, 21, 22 (in that
order, the table 0x928780) that hold an ammunition the weapon takes (`inventory_cook::on_subresources_loaded`
0x2C78AC..0x2C799A, `weapon_core::set_ammo_slots` 0x2E41D0, read back by `ammo_by_ordinal` 0x2E2750): which
stack the round came from, so the round is that slot's item (in range in 96,474 of 96,474 bullets; no kit of
the samples holds two different rounds for one weapon). The current
position and velocity are not saved (rebuilt from the segment at a load). The manager holds the generator of the
turns (x 0x08088405 + 1, saved), gravity (0, -9.81, 0) and its time.

**The bots read no bullet**: the only readers of the worlds' bullets are the validator and its bench.

## When a bullet moves (`bullet_manager::tick` 0x321D10)

- The world's step (0x2D1E20) hands every tick list the flag time % 50 == 0; the manager's tick is in list 5,
  after the players' update (the shot), the physics step and the bodies' transforms, before the fuses, and
  **returns at once unless the flag is set** (0x321D29): bullets move only at the 50 ms grid. A shot made inside a
  tick first moves at the first grid tick at or after it, flown (that tick less the shot's event time), against
  the bodies as that tick left them. Nothing is instant. Counted: the manager's time is on the grid in 26,479 of
  26,479 worlds; a segment-0 bullet's life is (now - born) / 1000 in 75,334 of 75,334 records.
- `bullet::tick` 0x32C180 flies each bullet to the tick's time in steps: the longest step whose chord's mid-point
  is within 0.05 m of the path (`pick_next_permissible_time` 0x32CDD0), one collision check a step. **The number
  of steps is not bounded**; the bullet ends when its speed is under 1 m/s (0x32C204) or after **30 changes of
  course** (0x32C20D). Counted: the flown distance's growth between two worlds equals the simulated steps' chords
  in 14,479 of 14,479 pairs on segment 0 and 25,236 of 25,296 on later ones.
- `update_bullet_position` 0x32EE90 ends a bullet outside the physics world's box, at the ammunition's
  `distance` flown (1,000 m for every ammunition) or under 0.0032 m/s.

## The flight (`compute_trajectory_position` 0x32CCC0)

- With k the ammunition's `air_resistance` (1 for every ammunition) and T = max(0, 1 / (1.1 k)): for t < T,
  **p = p0 + v0 (t - k t^2 / 2) + g t^2 / 2** and v = v0 (1 - k t) + g t; **past T the bullet flies on** as a free
  fall at the velocity it had (v0 / 11 + g T at k = 1). The flight does not end at T = 0.909 s: about 17,000
  records are of bullets past it (to 11.8 s of life and 999 m flown). Counted: the flown distance equals the
  path's length in 73,279 of 73,282 records.
- **The fall-off** (0x32EFE9..0x32F174), at every step and at a hit: s = clamp((|v| - v_min) / (v_max - v_min),
  0, 1), |v| WITH gravity; damage = min(damage, lo + (hi - lo) s), pierce = min(pierce, plo + (phi - plo) s).
  hi = the weapon's `bullet_damage` x the ammunition's `k_damage`; lo = hi x `min_damage_amount` x
  `k_min_damage_amount`; phi = `bullet_pierce` x `k_arp`; plo = phi x the same two; **all after the kit item's
  modifications**. v_max and v_min (`compute_damage_speed` 0x32C520, at the shot) are the speeds at
  `max_damage_distance` x `k_max_damage_distance` and `min_damage_distance` x `k_min_damage_distance` WITHOUT
  gravity: v0 sqrt(1 - 2 k d / v0). Counted: damage and pierce equal the law within 2e-4 in 73,250 of 73,282
  records of bullets that had hit nothing (34,451 of 34,455 between players); with today's guns and no
  modifications 6,731 are off (whole weapons changed between revisions, and modified guns).

## What a step hits (`check_collision` 0x32D4E0)

- One closest-hit ray along the chord (`bullet_physics_world::ray_test` 0x75E20): the level's bullet mesh,
  grenades, batteries and the bodies' hit boxes. **The shooter's own boxes are never met** (0x2DA5E3). A body's
  boxes are posed at most once a tick, at the first ray that asks: that tick's animation at the body's transform
  after the physics step. No lag compensation: no code poses a body at another time.
- **A back face of the map costs nothing**: the ray goes on from 1 mm past it (0x32D72F). A face is a back face
  when dot(normal, direction) >= 0 (0x32D69F), the normal being e1 x e2 of the placed triangle
  (`btTriangleRaycastCallback::processTriangle` 0xB8010 reports -(v10 x v20) in the library's mirrored frame):
  `mapdata/src/sight.rs`'s `FRONT_NORMAL_SIGN = +1` is PROVEN for meshes (a placement whose transform mirrors is
  not checked).
- A front face of the map, or any face of an object: **pierce** (`try_pierce` 0x32D960), else for a front face
  **ricochet** (`try_reflect` 0x32DE40), else the bullet stops.

## At a surface

- **Pierce**: A the material's `physic/armor`, P the bullet's pierce after the fall-off at the hit;
  f = clamp((P - A) / A, 0, 1), 1 for A = 0; k = 1 - (1 - f)^2; k <= 0 stops the bullet; else damage, pierce and
  speed x k, the direction turned by a random share of the ammunition's `pierce_dispersion_angle` x the
  material's `k_pierce_dispersion` (two draws of the manager's generator), and a change of course from 1 mm past
  the point. A face of armour 0 costs nothing but turns the bullet and counts as a change.
- **A body is always pierced** (0x32DA3C..0x32DA68): after the body's hit, k = the weapon's least damage lo / the
  bullet's damage. The bullet goes on through every body with damage lo and pierce plo (k has no cap: a bullet a
  wall left under lo leaves a body with lo again). No team test anywhere on the path.
- **Ricochet** (a front face not pierced; at most 2 a bullet, 0x32DE4D): a the angle above the surface, most =
  min(90 degrees, the ammunition's `ricochet_angle` x the material's `k_ricochet`); none above most; else with a
  draw u, a ricochet when u > a (1 - `ricochet_chance` x `k_ricochet_chance`) / most (always when grazing). The
  mirrored direction turned by a random share of `ricochet_dispersion_angle` x `k_ricochet_dispersion`, the speed
  x (1 - the material's `reflection_speed_down` x a / most), and **damage and pierce at most lo and plo**
  (0x32E33A..0x32E3C6).
- What stops a bullet: a front face whose armour is at least its pierce and no ricochet; the world's box;
  1,000 m; 30 changes of course; under 1 m/s.

## What a body takes

The first box a bullet meets takes D x (1 - clamp(A - P, 0, 1)), D and P the bullet's at that step; **every later
box, of that body or of the next, takes lo x (1 - clamp(A - plo, 0, 1))**; each pool keeps the largest routed share
(`damage_model.md`). A box's armour is the worn items' sum with their modifications' adds, only on the boxes the
item's own config names: a modification whose key the item lacks is skipped (`find_modifier_target` 0x2F1462, the
cook 0x2F1ABF; it explains 1,876 hits against 1,853 with every box). Counted: a hit foretold from a bullet an
earlier world held in flight agrees in every pool in **1,876 of 1,955** (683 of 717 between players; the rule
without the fall-off and the later boxes: 610); of bullets with two or more box records on one body, every box
but the largest is lo's in 75 of 76 (37 of 37); **1,000 of 33,014 bullets that hit left records on two or more
bodies** (291 of 17,226).

## Against the bots

| our rule | the client | counted |
|---|---|---|
| `aimpoint.rs` `clear_shot`: a shot is clear when no face of the bullet mesh is on the line | back faces and faces of armour 0 cost nothing; a front face is pierced by armour; the path drops | of 9,615 bot bullets fired at a body, 1,237 on a line the any-face test blocks went through by the law (128 hit; 1,022 met no armoured face); 388 on a clear line ended in a wall (mostly the ground under the drop) |
| fire through cover (`reflex.rs`, `tuning/cover.rs`, `tactics.rs`): a fixed pierce 0.3125 | the bullet's own: `bullet_pierce` x `k_arp` with the item's modifications, after the fall-off at the wall's distance | |
| `friends.rs`: "a bullet hits whatever body it meets first" | and goes on through it with lo and plo | 1,000 of 33,014 |
| `bots/flight.rs`, `survarium/.../damage.rs`: "the flight ends at t = 1/(1.1 k)" | the resistance ends there; the bullet flies on | about 17,000 records past T |
| `damage.rs` "gravity left out: LIKELY" | the two damage speeds leave it out; the step's speed has it | 73,250 of 73,282 with it; 7 of the 570 beyond 2e-5 fit without |
| `weapons.rs` `with_mods` leaves the damage keys | the item's modifications change `bullet_damage`, `bullet_pierce` and the damage distances | 6,731 records off without the revision's guns and the modifications |
| `mapdata/src/sight.rs`: 64 crossings, armoured faces only; a 2 mm gap; a straight line | 30 changes, every pierced face and ricochet; 1 mm; a pierce turns the bullet, a front face may ricochet | |
| a hit = the nominal `bullet_damage` (`mod.rs`, `worth.rs`) | the first box D(s), later boxes lo | 1,876 of 1,955 |
| no bullet is read from a world | every bullet in flight: shooter, segment start and velocity, life, damage | |

The same as the client: `flight.rs`'s `time_to` and `drop_m` before T; `damage.rs`'s `bullet_at`; `pierce.rs`'s
front-face rule for the map.

## Not read

A mirrored placement's faces; whether a ray starting inside a hit box reports an exit hit (it could change no
record); the clock of a bullet that changes course several times in one tick (2,805 of 76,405 records off the
floored millisecond); 60 pairs off the simulated steps by more than 2 mm;
the materials' ricochet and dispersion keys' values (in the players' resources.db only); the pose of the hit boxes
at a hit (not ported); the parallel tick at 256 bullets or more (never reached: at most 105 in a world).
