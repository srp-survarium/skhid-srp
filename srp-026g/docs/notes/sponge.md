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

- The thrower is credited with the zone's hits (0.26g's zones name no one); the thrower is who
  armed it (the release), by his roster place, which no one else gets in that match.
- No credit (and no modifiers on the splash) while the thrower's place is out of the match.
- A tap (the key let go before the pin is pulled) arms it at the pull; 0.28d0 threw it unarmed,
  and it never exploded.
- Taken out of its inventory while armed in the hand, it drops from the hand and explodes where
  it lands, its thrower's; 0.28d0 exploded it at the identity transform.
- The passive is off when it leaves an inventory or explodes (0.28d0 left it in the holder's
  pool while armed).
- A spent Sponge cannot be picked up while its zone runs, and the zone's end only resets a
  Sponge still spent.

## Every game the same

- The press is an input; the pin, throw, explosion and zone ticks are on the world's 50 ms grid.
- Serialized: the fuse, the pin, whether thrown, the body (transform, velocities), a tap
  waiting for the pull, the thrower's place, and while the zone is on its state and the
  explosion point.
- A rollback's load sets the passive by the state loaded, so every game has the same pools.
