# The Sponge, path A (the owner, 2026-10-22)

Client patch 55 path A (README section 55, client_patch.md section 3.52): 0.28d0's
`artefact_sponge_core` written again on 0.26g's interfaces. The item, its configs, model,
particles, texts and spawns are the content's (path B's branch); the code runs on 0.28d0's
`artefact_sponge.options` as it is.

## As in 0.28d0

- Artefact type 12 (`artefact_sponge`, items_dict 758), in an artefact slot.
- Its key: the press starts the stock throw-grenade state and animation (the Sponge as the
  grenade), the pin is pulled in it; the release starts the fuse `activation_timeout` (3 s) on
  the 50 ms grid and the throw goes on with `throw_power` (15) x the thrower's throw modifier.
- While it is in the air another press does not throw it again.
- At the fuse's end: the splash (`splash_*`, toxic; radius x the splash radius modifier, the
  rest x the grenade damage modifier, as a grenade), then the inline `damage_zone` on at the
  explosion point for `duration` (10 s), then off.
- Consumed at the explosion: out of its slot.
- Carried: the passive, a regeneration modifier on the `infection` pool (Lifebone's code).
- 0.28d0's `splash_pierce` 0 makes the splash do no damage, in 0.28d0 as here.

## Ours

- The thrower is credited with the zone's hits (0.26g's zones name no one).
- The passive is off once the Sponge has exploded (0.28d0 left it linked).
- The thrower's place is part of the Sponge's state (late join, rollback).

## Every game the same

- The press is an input; the pin, throw, explosion and zone ticks are on the world's 50 ms grid.
- Serialized: the fuse, the pin, whether thrown, the body (transform, velocities), and while
  the zone is on its state, its geometry's transform and the thrower's place.
