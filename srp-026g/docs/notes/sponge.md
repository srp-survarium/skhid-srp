# The Sponge (the owner, 2026-10-22)

Client patch 59 with the Rattle (README section 59, client_patch.md section 3.52;
`rattle.md`): 0.28d0's `artefact_sponge_core` written again on 0.26g's interfaces. The
content pack `sponge@2` (release/content.tsv; srp-026g/scripts/sponge/make_pack.py) and the
owner's spots (edit list `artefact-spots-1`, below). 0.28a2 brought the Sponge; the reference is
0.28d0's `artefact_sponge.options` and its code.

## As in 0.28d0

- An artefact (items_dict 758, `artefact_sponge`, category 12, data type 12, base_cost 0)
  picked up from an anomaly into an artefact slot.
- Its key: the press starts the stock throw-grenade state and animation (the Sponge as the
  grenade), the pin is pulled in it; the release starts the fuse `activation_timeout` (3 s) on
  the 50 ms grid and the throw goes on with `throw_power` (15) x the thrower's throw modifier.
- While it is in the air another press does not throw it again.
- At the fuse's end: the splash (toxic, radius 10 m x the splash radius modifier, 1.5 down to
  0.05 x the grenade damage modifier); its `splash_pierce` 0 makes it deal no damage, in
  0.28d0 as here: only the zone hurts.
- Then the config's `damage_zone` on at the explosion point for `duration` (10 s): 1.5 a
  second on the face within a cylinder 3.5 m across, its `hit_curve` and `speed_curve`.
- Consumed at the explosion (out of its slot); the anomaly spawns it again later, as any
  artefact.
- A press throws one Sponge: patch 54's "one press fires every ready artefact of its kind"
  does not reach it (its key is its own).

## The owner's rules

- Static toxin rule: while you own a Sponge, `infection` (the toxin pool) is kept full. Its
  passive is 0.28d0's regeneration modifier on that pool with regen_add 1000 (0.28d0: 0) and
  timeout_mul 0 (0.28d0: 1): a toxic hit lands (its damage, the pool's thresholds and their HUD
  happen at the hit) and the pool is full again at the next regeneration tick. A single hit
  that takes infection from full to 0 kills (the zone's 1.5 can).
- Owned: in an inventory with its holder alive, from the pickup until it explodes or leaves
  the inventory.
- Everything else as 0.28d0's file: the pack ships it as it is but those two numbers and the
  description's key (0.28d0's names Spring's text; the Sponge's is ours).

## Ours (0.26g has no such code; 0.28d0's holes closed)

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
- Destroyed (when the world goes at a match's end: the anomaly holds it until then), it frees
  only what is its own (its body, the geometry it made, its hold on the zone) and touches
  nothing the world owns (the physics world, the holder's pool, the scene's list), as 0.26g's
  grenades, Springs and Rattles do: whichever of those goes first, nothing is called or walked
  that may be gone (2026-10-23, #344's open "crash at a match's end").

## Every game the same (lockstep, late join, rollback)

- The press is an input; the pin, throw, explosion and zone ticks are on the world's 50 ms grid.
- Serialized: the fuse, the pin, whether thrown, the body (transform, velocities), a tap
  waiting for the pull, the thrower's place, and while the zone is on its state and the
  explosion point.
- A rollback's load sets the passive by the state loaded, so every game has the same pools.

## Spawns (the owner's spots, 2026-10-24)

- The owner chose where the Sponge and the Rattle spawn, in a spawn editor: 60 containers of
  map anomalies on 11 maps, 36 Sponge and 24 Rattle (`scripts/sponge/spots.tsv`; level_01..06,
  London, the v0.11e and 0.21d maps). "I click a Larkspur spot and make it a Rattle: now the
  Rattle spawns there."
- A picked container spawns only its kind. The anomaly's other containers spawn its own kinds,
  as before. An anomaly with no pick is as 0.26g has it: no Sponge or Rattle there.
- Per map: level_03 and v011e_level_03, level_04 and v011e_level_04 have the same anomalies but
  their own picks. A container two anomalies share (level_03's antenna) is picked for one of
  them only; none of the picks is one.
- The content: each picked map's mode configs hold `mode/artefact_spots`, one row a spot (the
  core's `full_name`, the container's index in its `artefact_containers`, its name, the kind):
  edit list `artefact-spots-1`, made by `scripts/sponge/spots.py` from spots.tsv, which checks
  every row against every live project of its map and stops on a core or container it does
  not find. No map project ships.
- The exe (client_patch.md 3.52, "The owner's spots"): the match's map (its options' map id)
  picks the mode config (the items dictionary keeps them loaded); the anomaly cook gives a core
  one artefact of the kind for each of its spots (the world grows by those units only: 60
  units in all, at most 10 on a map); the core's links keep which containers are picked; the
  spawn takes the picked kind there, the core's own kinds elsewhere.
- The draws are 0.26g's (the chance, the container, the artefact, the delay; the server's
  bots model them so): the artefact's draw is made whenever any of the pool is free, as
  0.26g's, and chooses among those the container may take. When the picked kind is not free
  (a player has it, or its cloud is still on), nothing spawns there that time and the anomaly
  stays due: it tries again after its delay (0.26g marks an anomaly that is about to fill as
  not due before it picks the artefact; the patch marks it due again, or an anomaly of one
  artefact, as London's, would never spawn again in that match).
- Each game keeps the picked anomalies of its current world only (a new world's cooks start
  afresh), so a long session, a fresh game and a late joiner all have the same.
- No timed anomalies: main has none since #345; #346 (their redesign) decides for its own
  cores.

## Content (pack sponge@2)

- items_dict 758, base_cost 0; `gameplay/items/artefacts/artefact_sponge.options` (0.28d0's, as
  above); 0.28d0's model (also the hand's: the throw draws the item's own model), particles
  (without `MarginMultiplier`, which 0.26g does not know), the materials and textures 0.26g
  lacks.
- Texts in the six languages: the name and pickup hint 0.28d0's, the description and the
  activation hint ours, one line each.
- The lobby icon: atlas cell [22,1] (already in 0.26g). The combat-log icon 17: 0.28d0's
  picture drawn by the hud movie's code (srp-026g/flash/hud, `SpongeLogIcon`).
- The server needs nothing: it reads the item from the game data; artefacts are never sold.

## Release

- Changes what a match computes: the `next` row of
  `crates/session/src/v026g/simulation_changes.tsv`.
- The exe needs the pack's config (item 758): the release's exe requires its resources.db
  patch (patch 49).
- The site's /maps shows it at its spots only: its builder (scripts/maps-tab/build.py) reads
  the mode configs' `mode/artefact_spots` as the patch does; the lobby icon
  `artefact_sponge.webp` comes from the calculator's icon export (cell [22,1]).
