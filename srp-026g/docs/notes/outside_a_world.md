# What decides a match but is not in a world (0.26g client)

Read in the executable (virtual addresses, image base 0x10000; names in `tools/re-kit/v026g`). PROVEN where an
address is given, unless said otherwise. Counted on the uploads and hash reports of recorded matches read with the
repository's own parsers: 949 recordings of one live set (revisions 40 to 47; 476,315 uploads) and 446 of a later
one (47 to 50; 222,556 uploads). See also `match.md`, `snapshot_rules.md`, `client_patch.md`, `battery_taker.md`,
`bullet_bounds.md`, `exactness_rig.md`.

## The save, the wire and the load

- **Save** (`game_world_core::serialize` 0x2D2570): the time; for each place in the mask its current kit's player
  (`game_world_core_client::serialize` 0x3218E0); the bullets; the statistics; the rules and the blackboard; the
  game objects; the physics world; the effects.
- **On the wire** (`game_state_history_item::serialize` 0x324BE0): each player's state and animation tree, then six
  sections (bullets, statistics, objects, physics, rules, effects). The history item keeps two more kinds of buffer
  that are never sent: the objects' and each player's second writer, where effects of serialization mode 2 go
  (`game_effect_player::serialize` 0x2DDB83..0x2DDB9D). **The hash** (0x325150) is over the wire's bytes exactly.
  Counted: an upload's CRC-32 is a hash some game reported for 431,589 of 476,315 uploads (the others: joiners'
  initial worlds, which nobody hashes, and ticks nobody reported).
- **A world of time T is the state after the tick ending at T**: the step loop applies the step's events, runs all
  seven tick lists, then saves (0x2D22E2 -> 0x2D230A). Every section is "after tick T". World times are on the 50 ms
  grid (0 off of 66,143 times two or more games uploaded).
- **Load** (`game_world_core::deserialize` 0x2D2770), the same for a world from the network and a local rollback:
  the mask, the time, each place's next kit (0x2D2A37) and current player, then the sections. Only a game's FIRST
  world also adds list entries for each place (the current input and the kit in use, 0x2D0FDD -> 0x2D4AB0,
  0x2D4B50).

## What a world does not carry

| state | in a world? | what it decides | what a loading game has |
|---|---|---|---|
| a place's two other kit players (weapons, items, damage model) | no: only the current one is written (0x321924..0x32193C) | the next spawn into that kit | the constructor's values, or its own past; patch 10 resets 12 weapon fields at a change of kit |
| a departed place's profile (stocks, conditions) | no | its kit when it returns | the static info's full values |
| **the kit-choice lists** | no | **the next kit, every tick** (below) | kept through a resync; a first world adds one entry |
| the input lists | no | the replay after a rollback | kept through a resync |
| the physics world (broadphase, pairs, manifolds, the bounds cache) | no, **not even in the local history**: its save and load slots are empty (0x75AF0, 0x15A4B0) | ties at one fraction; a bullet's out-of-bounds cut | built from the map's load and each body's own load |
| effects of mode 2 | no (rollback only; the core's own are dropped, 0x2D26F1) | not read | none after a world |
| the battery taker, the Team Deathmatch completion time, the gather rule's completion, the active-objects registry | no (`match.md`, `battery_taker.md`, `client_patch.md`) | as read there | the game's own |
| broken limbs' counts, the scoreboard's score, the alive counts | derived | the leg and hand gates | **recounted at every load** from the saved affects (`damage_model::deserialize` 0x2E5518..0x2E5538) |
| the particle generator, the camera, sounds | no | drawing only | each client's own |
| **the animation's mixing tree** | **yes**: every world's animation stream is the whole tree, each node with its tracks, u32 times and raw f32 (0x397250, 0x396410, 0x396530) | the clips' times, blends, events | rebuilt from it, for a world and a rollback alike (`deserialize_state` 0x395B50, one caller) |

- **The next kit is worked out of the kit-choice list every tick**: `apply_profile_input` 0x321790 walks the place's
  list to the newest entry at or before the tick and makes its kit the next one (when the place has that kit). A
  world's next kit therefore lasts one tick in a game whose own list says otherwise, and a resync's world adds no
  entry. Counted: of the bodies that differ between two games' uploads of one world time only in the next kit, **58
  of 58 came from a game that had loaded a world after its first**.
- **The clips' clocks are in every world.** "The clip's own clock is not in a world" (`weapon_fire.md`, Not read) is
  wrong: the tree's nodes carry their times. Two games' uploads of one world time are byte-equal, animation
  included, in 35,661 of 36,871 times (28,535 of 29,272 in the second set): a load does not shift a clip against
  the other games, so the fire clip's passes that are 1 to 3 ms off are the shared computation's.
- **The order of ties is the map's, the same in every game**: among 1,733 bodies that differ between two games in
  play, the ground normal never differs without the position.
- **`user.cfg` cannot set a simulation variable**: it runs with mask 2, and `cc_step_height` and
  `jump_prepare_interval_length` are of kind 1, which only `startup.cfg` (in resources.db) sets
  (`console_commands::execute` 0xEBA93..0xEBAAA).

## Counted: two games' copies of one world time

| | first set | second set |
|---|---|---|
| world times uploaded by two or more games | 36,871 | 29,272 |
| all copies equal | 35,661 | 28,535 |
| differing in play | 1,045 | 596 |
| hash ticks (3+ reporters, a strict majority) with a minority | 183,881 of 2,644,985 | 96,033 of 2,163,394 |

Of the differing bodies: 362 differ only in the pickup record, all at revision 42 (gone after it); 58 only in the
next kit (above); the rest in position, view and speed first. Long splits in which every member of the minority had
loaded a world, against games that never had: 302 and 6, nearly all in 7 recordings of revisions 43 and 47; a load
also FOLLOWS a split, so that share alone does not tell cause from effect.

## Against the bots

- `bots/stamina.rs:1` and `bots/motor.rs:538` say the world has no stamina: it has (29 bytes, every state).
- `bots/body.rs:136` passes `legs_broken: false` "not in the worlds": the count is derived at every load from the
  saved leg affects, which the bots can read (the legs' `leg_damage_ends` after now).
- After a world moves a body more than 5 cm the kept controller restarts as a standing one
  (`mapdata/src/grid/controlled.rs:499`): the client restores the controller's 67 bytes (flags, fall velocity,
  ground and ceiling normals, slide) from the world.
- Never exact from worlds alone: an enemy's other kits until he spawns into them; the kit-choice entries (the
  server relays every one and can keep them); the tie order (from the map).
- Exact if read, in every world today: the mixing tree (a weapon's clip clock), the stamina, the controller's block.

## Not read

The cause of the loaded games' splits (the candidates are the rows of the table). Settled since: the core's
physics object is `bullet_physics_world`; the effects of mode 2 are the artefacts', the zones', the hits' and the
affects' (the match HUD's are mode 1), and the simulation reads none; the in-game console cannot set a
simulation variable mid-match (it runs with mask 2, and its config loader is itself of kind 1).
