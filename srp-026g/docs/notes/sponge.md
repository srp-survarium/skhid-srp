# The Sponge (the owner, 2026-10-22)

Client patch 59 (README section 59, client_patch.md section 3.52): 0.28d0's
`artefact_sponge_core` written again on 0.26g's interfaces, thrown and exploding as retail's
(the owner, 2026-10-25: "exactly the same logic as in retail", 0.69d0's in everything but our
toxin passive). Content: pack `sponge@5` (release/content.tsv, scripts/sponge/make_pack.py) and
the spots (edit list `artefact-spots-1`, below).

## As retail (0.69d0's numbers)

- An artefact: items_dict 758, `artefact_sponge`, data type 12, base_cost 0, picked up from an
  anomaly into an artefact slot.
- Its key starts the stock throw-grenade state and animation, the Sponge as the grenade.
- The pin's pull starts the fuse: 3 s (`ttl_ms` 3000), on the 50 ms grid. The release throws it
  with `throw_power` (15) x the thrower's throw modifier.
- Held past the fuse it explodes in the hand: the cloud on its holder, the Sponge consumed, the
  grenade slot given back, the throw state left.
- Its first contact explodes it at the next tick when it comes 1.5 s (`safe_time` 1500) or more
  after the pull (0.46c2's rule, 0x7AD5E0); an earlier contact is spent and the fuse decides.
- While it is in the air another press does not throw it again.
- The explosion: a splash of fire, 0.3 at any distance within 10 m (x the splash radius
  modifier; x the grenade damage modifier), pierce 0.1, on hit target 18 (the face). It hurts and
  kills.
- Then the config's `damage_zone` for 15 s at the explosion point: fire, 0.2 a second on the
  `body`, a cylinder 5.5 m across, particle `mochalka_explosion`; the F1's sound once at its
  start (`weapons/f1`, the only sound retail's chain names).
- At its spawn in the anomaly: particle `pre_mochalka1`.
- Consumed at the explosion; the anomaly spawns it again later, as any artefact.
- A press throws one Sponge: patch 54's "one press fires every ready artefact of its kind" does
  not reach it.
- Not taken from 0.69d0 (the owner's to decide): its passive (-20 % from radiation and
  explosions for 180 s), the skill Sponge II's second charge.

## The owner's rules

- While you own a Sponge, `infection` (the toxin pool) is kept full: its passive is a
  regeneration modifier on that pool (regen_add 1000, timeout_mul 0). A toxic hit lands, then the
  pool is full again at the next regeneration tick. A single hit from full to 0 kills.
- Owned: in an inventory with its holder alive, from the pickup until it explodes or leaves the
  inventory.
- It spawns only at the owner's spots (below).

## Ours (holes of 0.28d0 closed)

- The thrower is credited with the zone's hits (who pulled the pin, by his roster place); no
  credit and no modifiers on the splash while that place is out of the match.
- A tap (the key let go before the pull) is armed at the pull like any throw.
- Taken out of its inventory while armed in the hand, it drops and explodes where it lands.
- The passive is off when it leaves an inventory or explodes.
- A spent Sponge cannot be picked up while its zone runs.
- Destroyed at a match's end, it frees only its own (its body, its geometry, its hold on the
  zone) and touches nothing the world owns.
- Patch 78 (H + an artefact's key raises it in the hand) does not take the Sponge: the carry
  has no model for it, and its key throws it.

## Every game the same (lockstep, late join, rollback)

- The press is an input; the pin, throw, explosion and zone ticks are on the 50 ms grid; a
  contact is ruled at the next tick.
- Serialized: the fuse, the pin, whether thrown, the body (transform, velocities), the first
  contact's state, the thrower's place, and while the zone is on its state and the explosion
  point.
- A rollback's load sets the passive by the state loaded.
- Its state numbers are 6 (armed: in the hand or in the air) and 7 (the zone). Patch 78 uses 6
  for a dropped artefact: its serialize adds a 20-byte record to every artefact in state 6, the
  Sponge's included (written and read the same in every game), and it finds no drop of a Sponge.

## Spawns: the owner's picks (2026-11-05, final)

- Where it spawns is one file, `scripts/sponge/spots.tsv`: one row a container of an anomaly core
  of a map. The owner's export from the spawn editor replaces it whole (`spots.py import`).
- 19 containers on 5 maps: level_01 and v069d_level_01 (3 each, the swamp's Lifebone anomaly),
  level_04 and v011e_level_04 (6 each, the toxic moss's Onyx anomaly), level_05 (1, the Onyx
  anomaly). No picks elsewhere: every other anomaly is stock. Cologne and London have no
  anomaly cores on main (the dynamic anomalies were taken out in #345), so no Sponge there.
- Dynamic anomalies (#346, if they return): a core cooked mid-world starts the spots' next
  generation (sp_artefacts) and the current world's picked cores lose their spots; that must be
  fixed before they come back. Not done here.
- `scripts/sponge/spots.py edit-list` makes the edit list `artefact-spots-1` of it: each map's
  mode configs get `mode/artefact_spots`; every row is checked against every live project of its
  map. `spots.py check` says whether the edit list is current.
- A picked container spawns only the Sponge. The anomaly's other containers spawn its own kinds,
  as before. An anomaly with no pick is 0.26g's: no Sponge there.
- The exe (client_patch.md 3.52, "The owner's spots"): the anomaly cook gives a picked core one
  Sponge for each spot; the core's links keep which containers are picked; the spawn takes the
  Sponge there and the core's own kinds elsewhere.
- The draws are 0.26g's (the chance, the container, the artefact, the delay). When the Sponge is
  not free (a player has it, or its cloud is on), nothing spawns at its spot that time and the
  anomaly stays due.
- Each game keeps the picked anomalies of its current world only, so a long session, a fresh
  game and a late joiner have the same.

## Content (pack sponge@5)

- items_dict 758; `gameplay/items/artefacts/artefact_sponge.options` (0.28d0's with retail's
  numbers: `ttl_ms`, `safe_time`, `explode_on_contact`); 0.28d0's model; 0.69d0's
  `pre_mochalka1` and `mochalka_explosion`; 0.28d0's other particles; the materials and textures
  0.26g lacks (`release/texture_closure.py --pack` checks every texture of the pack's particles;
  run by hand, not by build-content: main's live weapon packs fail it today).
- Texts in the six languages, one line each.
- The lobby icon: atlas cell [22, 1] (in 0.26g already). The combat-log icon 17: 0.26g's empty
  frame (no weapon uses it), drawn by the hud movie's `SpongeLogIcon`.
- The server needs nothing: it reads the item from the game data; artefacts are never sold.

## Release

- Changes what a match computes: the `next` row of `crates/session/src/v026g/simulation_changes.tsv`.
- The exe needs the pack's config: the release's exe requires its resources.db patch (patch 49).
- The site's /maps shows it at its spots only (scripts/maps-tab/build.py reads the mode configs'
  `mode/artefact_spots`).
