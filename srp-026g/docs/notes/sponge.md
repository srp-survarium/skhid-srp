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
- Its first contact explodes it at that tick when it comes 1.5 s (`safe_time` 1500) or more
  after the pull; an earlier contact is spent and the fuse decides. A contact is its body's
  (the physics world's) or the sensor's: another player in its way (below).
- While it is in the air another press does not throw it again.
- Its body is of the game material "grenade": it bounces and sounds as a grenade.
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

## 0.69d0's own, read (2026-11-05)

Read from `/mnt/hdd/survarium-offload/0.69d0/Survarium.zip`:
- the config: `gameplay/game.options` of resources.75.db (resources.43.db's the same),
  `ammo_grenades_etc/artefact_sponge`;
- the materials: `game_materials/game.materials` (resources.72.db) and `material.pairs`
  (resources.54.db);
- the texts: resources.70.db;
- the exe: `binaries/x64/survarium.exe`, image base 0x200000000.

- **The contact rule.**
  - The config's `data/`: `ttl_ms` 3000, `safe_time` 1500, `explode_on_contact` true,
    `start_timer_on_throw` false, `collide_with_player` true.
  - `grenade_core_cook` 0x200322450 builds a `grenade_core_state_transition_by_contact` (vtable
    0x20098D750) on the thrown state, with `[t+58h]` = ttl_ms - safe_time (0x2003231EF..0x2003232F8).
  - `pull_pin` 0x20039D920 sets the fuse's end to the pull time + ttl_ms, on the tick grid, and
    clears the first contact.
  - The contact handler 0x2003ECC00 explodes it at the first contact since the pull when the
    fuse's end - now <= 1500; that first contact is consumed either way.
  - The timeout (0x2003EFF60) is the fuse's end.
  - Ours is the same rule (0.46c2's 0x7AD5E0 was too). An early landing waits for the 3 s fuse
    in retail as well.
- **The sensor.**
  - With `collide_with_player` the thrown state is `grenade_core_thrown_with_sensor_state`. Its
    tick 0x2003EBF10 sweeps a ray from the last position and fires the same handler for a hit
    on a player other than the thrower.
  - 0.26g's grenade_core has no states and no sensor: its RTTI names only `grenade_core`, its
    cook and its contact callback.
  - Ours: the sweep is in the Sponge's own tick (`in_the_way`). Each tick in the air, the way
    from the last position (m_transform: the hand's at the launch, serialized in the air) to this
    one is swept with spheres of 0.1 m (its body's), at most that far apart, every living
    player's bones made fresh first (0x2DA5B0, as the splash does); a corpse is no contact. Each sphere is the collision query the splash makes (the hit manager's
    collision world, vt+60h, flags 10h and 40h). A found object whose owner is a player not of
    the thrower's place is a contact, under the same rule.
  - Not retail's exactly: spheres along the path, where retail casts one ray.
- **The sound.**
  - `data/active/damage_zone/activated_sound` is "weapons/f1" (resource 257), retail's own: the
    damage-zone cook 0x20021EEF0 reads it (0x20021EF98..0x20021EFC1).
  - There is no other explosion sound: the visual cook 0x2002410F0 sends subtype 2 to
    0x200323C10, which reads none.
  - The throw's grenade sounds (the pin's "grenade_pin_out_01", "grenade_wear",
    "grenade_second_pin_out") are its throw_sponge state's, the same as throw_grenade's. Kept.
- **The material.**
  - `data/mtl_idx` 45: game.materials' id 45 "grenade".
  - Its pairs sound "weapons/f1_surface_hit" on ground, concrete, stone, wood and the like;
    "weapons/f1_metal_hit" on metals; nothing on bodies, water or sand.
  - 0.26g's "grenade" is id 45 too (its `game_materials/game.materials`, materials[35]). Its
    material.pairs give it "weapons/f1/f1_hit_ground" and the others, and the RGD-5 and the smoke
    grenade have `data/mtl_idx` 45.
  - Ours: the edit list `sponge-material-1` gives the Sponge's config `data/mtl_idx` 45, and the
    patch passes it to its body's shape (0x7A390's material, as grenade_core passes its own at
    0x300353). Before this, it had none (0).
- **The numbers, all as ours:**
  - duration 15 s (retail's key `data/duration_s`);
  - `data/explosive` min = max 0.3, `player_pierce` 0.1, fire, `affected_bodies` [18],
    `max_range` 10;
  - the zone: fire, `hit_parts_filter` ["health"], `max_hit` 0.2, `interval_in_msec` 1000;
  - its mesh: type 2, scale [5.5, 1.5, 5.5], position [0, 1, 0];
  - `idle_particle` mochalka_explosion, no `activated_particle`;
  - `object/particle_on_spawn` pre_mochalka1;
  - `throw_power` 15.
- **Deviations, with their reasons:**
  - The passive is the owner's rule.
  - The body's numbers stay 0.26g's grenade's: retail's `rigid_body` has mass 0.5, friction 10,
    restitution 0, no damping, and its shape scale is [0.04, 0.07, 0.07]. 0.26g's body cook
    takes a description of its own layout (make_body), not those keys. Not ported.
- **The texts.**
  - Retail's: "Protection from anomalies and explosions +%VALUE%%.\nExplodes when activated."
    (its pickup hint, of its own passive) and "Throw / tox.damage".
  - It has no description and no activation hint (`st_sponge_activation_hint` is named but
    missing).
  - Ours are written for what it does: fire, a burst on impact from 1.5 s after the pull,
    otherwise at 3 s.

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
- Exploded, it stays in its slot until the throw state holding it (from the pull) lets it go,
  however long it takes (a dead or still holder's state is not ticked): each tick it reads the
  world, its holder's weapon's current state and that state's grenade, so a finalize, a respawn
  (which finalizes nothing), a return and the next grenade all count. A world
  saved in between names it by its slot, and its load finds it there. Its cloud over first, it is
  spent (still 7) until then. With no throw state holding it, it leaves its slot at the explosion.
  The throw state's own save and load are safe either way (no grenade held: slot 17h, loaded as
  none).
- Its key does nothing while another grenade (a stock one being cooked) holds the grenade slot.
- Patch 78 (H + an artefact's key raises it in the hand) does not take the Sponge: the carry
  has no model for it, and its key throws it.

## Every game the same (lockstep, late join, rollback)

- The press is an input. The pin, throw, explosion and zone ticks are on the 50 ms grid. A
  contact is ruled at the next tick; the sensor's sweep is part of the tick.
- Its state is one tagged union (sponge_a_026g.c `struct sp_state`, AGENTS.md types and states):
  - held, as artefact_base has it;
  - in the hand (the fuse's end, the thrower, the first contact, whether a throw state holds it);
  - in the air (the same, and the physics world);
  - the cloud (its start, the thrower, the hold);
  - spent (its cloud over while a throw state still holds it).

  One function, `sp_transition`, takes (state, event) to the new state and its actions. The
  events are the pull, the throw, a drop, a contact, a tick and the throw state letting it go. The stream's bytes are parsed
  into the state at the load and written from it.
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
- Its texts are in the ten catalogs (client-patches/localization), one line each: the pack's
  six languages' strings are the build's no more.
- `data/mtl_idx` 45 is the edit list `sponge-material-1`'s.
- The lobby icon: atlas cell [22, 1] (in 0.26g already). The combat-log icon 17: 0.26g's empty
  frame (no weapon uses it), drawn by the hud movie's `SpongeLogIcon`.
- The server needs nothing: it reads the item from the game data; artefacts are never sold.

## Release

- Changes what a match computes: the `next` row of `crates/session/src/v026g/simulation_changes.tsv`.
- The exe needs the pack's config: the release's exe requires its resources.db patch (patch 49).
- The site's /maps shows it at its spots only (scripts/maps-tab/build.py reads the mode configs'
  `mode/artefact_spots`).
