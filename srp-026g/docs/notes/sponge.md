# The Sponge (the owner, 2026-10-22)

Client patch 59 with the Rattle (README section 59, client_patch.md section 3.52;
`rattle.md`): 0.28d0's `artefact_sponge_core` written again on 0.26g's interfaces, thrown and
exploding as retail's (2026-10-24; the owner, 2026-10-25: "exactly the same logic as in retail",
0.69d0's in everything but our toxin passive). The content pack `sponge@5` (release/content.tsv;
srp-026g/scripts/sponge/make_pack.py) and the owner's spots (edit list `artefact-spots-1`,
below). 0.28a2 brought the Sponge; the object is 0.28d0's `artefact_sponge.options` and code,
the throw, contact, numbers and particles retail's (0.69d0's: the owner's pick, 2026-10-24).

## As retail (0.31a2 on; 0.69d0's numbers)

- An artefact (items_dict 758, `artefact_sponge`, category 12, data type 12, base_cost 0)
  picked up from an anomaly into an artefact slot.
- Its key: the press starts the stock throw-grenade state and animation (the Sponge as the
  grenade; retail's own throw state has the same sounds and animations); the pin's pull starts
  the fuse, 3 s (`ttl_ms` 3000) on the 50 ms grid; the release throws it with `throw_power`
  (15) x the thrower's throw modifier.
- Held past the fuse (cooked, never thrown) it explodes in the hand, where the hand is: the cloud
  on its holder, the Sponge consumed, the grenade slot given back and the throw state left (its
  is_ready: exploded), as retail's grenade_core at its ttl.
- Its first contact (thrown, or dropped from the hand) explodes it at the next tick when it comes
  1.5 s (`safe_time` 1500) or more after the pull, that is with at most `ttl_ms - safe_time` of
  the fuse left (0.46c2's code, 0x7AD5E0 and its cook 0x731027..0x731065); a first contact
  earlier is spent and the fuse decides. A quick throw landing 1.6 s after the pull bursts on
  landing; one landing at 0.8 s rolls and bursts at 3 s; a throw cooked 1 s bursts on landing.
- While it is in the air another press does not throw it again.
- The explosion: the splash (fire, radius 10 m x the splash radius modifier, 0.3 x the grenade
  damage modifier at any distance: min = max, no falloff; pierce 0.1) on retail's body list
  [18]: a hit target of the character, the same 19 in 0.26g, 0.46c2 and 0.69d0
  (base_character_full's hit_targets: [18] the face on the Head bone), so each player within
  10 m takes 0.3 fire on the face (0.26g's face: head and pain). It hurts and kills.
- Then the config's `damage_zone` on at the explosion point for 15 s: fire, 0.2 a second on
  the `body` (0.26g's life pool; retail's `health`, which every part of 0.69d0 feeds) within a
  cylinder 5.5 m across, its `hit_curve` and `speed_curve`; its particle `mochalka_explosion`
  (0.69d0's) for its whole time.
- Its sound at the cloud's start: `weapons/f1`, the only sound retail's chain names: 0.69d0's
  `ammo_grenades_etc/artefact_sponge/data/active/damage_zone/activated_sound` is "weapons/f1"
  (resource 257, sounds/composite/weapons/f1.composite_sound: the F1's explosion, the same five
  collections in 0.69d0 as in 0.26g); 0.46c2's grenade cook skips the model, particle_explosion
  and sound_explosion of a `subtype` 2 grenade, the Sponge's (0x68941D -> 0x7313C0), its damage
  zone cook reads activated_sound (0x67025D), its particles hold no sound event, and neither
  0.46c2 nor 0.69d0 holds a sound file of the Sponge.
- How retail plays it (0.46c2's code, traced 2026-10-25): the zone cook keeps the activated sounds
  at the client zone's +590h (0x670262), one a sound point. The zone's update delegate 0x6F3E00
  (its constructor stores it at +5C8h, 0x6F3378) reads the state each frame: off, on, or on and
  hitting (+1A0h, +2A9h). Leaving off it calls 0x6F37E0, which plays each activated sound once
  through 0x6F35D0 at its sound point's position (+588h + 40h x i, +30h; the sound world's
  0x8C88E0, no volume of its own), then the activated particles (+5B4h, 0x8B47A0), then
  starts the idle sounds and particles (0x6F3A10; the Sponge names none). Nothing on that path
  reads the zone's kind or who switched it on, and no other sound is chosen there. So the F1
  plays once at the cloud's start, at the explosion point, in retail as in ours (0.26g's
  damage_zone::draw 0x2993F0 -> play_activated 0x298D80 -> survarium::play_sound 0x657C0).
  Inferred, not traced: that 0.46c2's Sponge zone is that class (its config's
  game_object_type and lib_name are "damage_zone", made by the damage zone cook that reads
  activated_sound).
- At its spawn in the anomaly: 0.69d0's `pre_mochalka1`.
- Consumed at the explosion (out of its slot); the anomaly spawns it again later, as any
  artefact.
- A press throws one Sponge: patch 54's "one press fires every ready artefact of its kind"
  does not reach it (its key is its own).
- Not taken from 0.69d0 (the owner's to decide): its passive (-20 % from radiation and
  explosions, +1.5 threshold, for 180 s) instead of ours, the skill Sponge II's second charge.

## The owner's rules

- Static toxin rule: while you own a Sponge, `infection` (the toxin pool) is kept full. Its
  passive is 0.28d0's regeneration modifier on that pool with regen_add 1000 (0.28d0: 0) and
  timeout_mul 0 (0.28d0: 1): a toxic hit lands (its damage, the pool's thresholds and their HUD
  happen at the hit) and the pool is full again at the next regeneration tick. A single hit
  that takes infection from full to 0 kills.
- Owned: in an inventory with its holder alive, from the pickup until it explodes or leaves
  the inventory.
- Everything else as 0.28d0's file with retail's fuse, contact and numbers: those two numbers
  and the description's key are ours (0.28d0's names Spring's text; the Sponge's is ours).

## Ours (0.26g has no such code; 0.28d0's holes closed)

- The thrower is credited with the zone's hits (0.26g's zones name no one); the thrower is who
  armed it (the pull), by his roster place, which no one else gets in that match.
- No credit (and no modifiers on the splash) while the thrower's place is out of the match.
- A tap (the key let go before the pin is pulled) is armed at the pull as any throw; 0.28d0,
  armed at the release, threw it unarmed, and it never exploded.
- Taken out of its inventory while armed in the hand, it drops from the hand and explodes where
  it lands (its first contact or its fuse), its thrower's; 0.28d0 exploded it at the identity
  transform.
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

- The press is an input; the pin, throw, explosion and zone ticks are on the world's 50 ms grid;
  a contact (the physics world's step, as every game steps it) is ruled at the next tick.
- Serialized: the fuse, the pin, whether thrown, the body (transform, velocities), the first
  contact's state (armed; a contact noted, not yet ruled), the thrower's place, and while the
  zone is on its state and the explosion point.
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

## Content (pack sponge@5)

- items_dict 758, base_cost 0; `gameplay/items/artefacts/artefact_sponge.options` (0.28d0's with
  retail's numbers, as above: `ttl_ms`, `safe_time` and `explode_on_contact` in data, 0.26g's
  grenade keys); 0.28d0's model (also the hand's: the throw draws the item's own model);
  0.69d0's `pre_mochalka1` and `mochalka_explosion` (written in 0.26g's config layout), 0.28d0's
  other particles (all without `MarginMultiplier`, which 0.26g does not know); the materials and
  textures 0.26g lacks, 0.69d0's `fx/artifacts/magic_smoke` and `magic_smoke1` among them (0.26g's
  materials magic_smoke and magic_smoke2 name them, no archive of 0.26g holds them: the spawn's
  fluid emitters were drawn as plain squares, 2026-10-25); no `fx/test/idle_particle`.
- Every texture every enabled emitter of the pack's particles draws is in players' files or the
  pack: `release/texture_closure.py`, which `release/build-content` runs on every pack of the
  manifest (allowed: `skin/skin_shadow`, which 0.26g's own light and distortion materials name
  and its own emergence, lamp and grenade particles draw).
- Texts in the six languages: the name and pickup hint 0.28d0's, the description and the
  activation hint ours, one line each ("bursts on impact (or after 3 s)", the cloud's 15 s).
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
