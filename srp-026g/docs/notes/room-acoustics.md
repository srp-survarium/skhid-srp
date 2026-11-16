# Room acoustics (patch 155, pack room-acoustics, edit list room-acoustics-1)

The owner, 2026-11-16, after reading the sound survey (`sound-gaps.md` 5.1): "i want that system:
… No room acoustics. 0.69 changes how shots and steps sound per room, with 19 room types, and
plays a background sound per room. 0.26 has no such system, so this is big exe work."

Built, checked offline (Unicorn, crate tests, build-content); not played, not released.

## What players get

- A shot or a step in a room sounds like that room: the room's recorded tail (0.69d's) is added to
  the gun's and the step's own sound. Outdoors a shot sounds as today.
- Each room has its background sound (a bed): it fades in as the player walks in, the last room's
  fades out at the same time.
- Maps without zones sound exactly as today.

## Which zone decides (0.69d's rule)

- **A placed (3D) sound: the room at its source.** Another player's shot in a classroom rings like a
  classroom wherever the listener is. 0.69d: the event player 0x200232AE0 gives a third-person
  instance the room at the event's position (0x200232EAD).
- **A hud (2D) sound: the listener's room** (own first-person shots, 0.69d 0x200232D03). In 0.26g the
  proxy's sound type 3 (hud) says which.
- **The listener's room:** the local player's feet while he is in the match and alive; otherwise
  (dead, the death camera, spectating, a replay without a living local player) the listener's
  position, the camera. 0.69d: the controlled player's cached room (player+23AAAh) if alive
  (+74Ch), else the ray from game_world+98h, the listener (0x20023737B..0x2002374B2). Updated every
  frame (0.69d: every tick). 0.69d refreshes a player's cached room only when he moves by input;
  ours is computed each frame (the same where he stands).
- **The bed: the listener's room**, always.

## How a point's zone is found

- **A ray straight up, 100 m** (0x20023AF50, direction (0, 100, 0)). The nearest mesh surface it
  meets names the zone. Faces are two-sided (the triangle test takes |det|, 0x20059FC32): from
  inside a box it meets the ceiling, from below it meets the floor, so a point under a zone (the
  ground floor below an upper room) is in that zone too. A point above or beside every mesh is
  outdoors.
- **Nested or overlapping zones:** no priority, no smallest-zone rule; the surface closest above
  wins; a tie goes to the later mesh (0.69d keeps `t <= best`).
- **Outdoors:** 0.69d's built-in first environment, named `default`, no bed (0x2001F84FD). Most
  maps have one large outdoor zone of their own (`cp_outdoor`, `rudnya_outdoor`, ...), which is
  a room like the others.
- **Meshes:** 0.69d's `sound_environment_geometry` primitives: boxes, cylinders (along their own
  y) and spheres, placed and turned (0.69d triangulates them; we test the exact shapes).
- **Scripted environments** (`is_scripted`) are skipped: 0.69d ignores them until a script
  activates them, which no tdm map does.

## Which sounds take the room

- **The composite filter** (0.69d's `composite_sound::play` 0x200635F70): an item with a
  `sound_environment <name>` tag plays only when the sound's room has that name, byte for byte
  (tags cut at 31 bytes, as 0.69d keeps them); an untagged item plays everywhere. No fallback: a
  room no item names plays the untagged items alone.
- **In 0.26g:** the composite cook (0x60860) keeps each item's tag as a room number in the item's
  padding byte (+15h, which the game copies and never reads); emit (0x63D60) skips an item of
  another room as it skips a disabled one.
- **Content (the pack):** the 0.69d room roots, flattened into our composites (0.26g's nest none):
  - guns, first and third person: each gun's composite keeps its items and gains the 0.69d root's
    items for the indoor rooms, tagged. Today's tails are the outdoor sound (0.26g's stock tails
    are baked into the gun's own samples), so no `*_outdoor` items are added;
  - steps: the four 0.69d step room layers (`environment_run`, `_sprint`, `_run_3rd`,
    `_sprint_3rd`, 14 to 15 rooms each) added to the step sounds of running and sprinting, own and
    others'. A third-person step was a collection in `material.pairs`; it becomes a composite of
    that collection and the room layers (the edit list). Crouching has none (0.69d neither).

## The bed

- **Enter:** the room's `ambient_sound` loops, 2D, fading in over its `fade_in` (0.69d enter
  0x20029FF80).
- **Leave:** the old room's bed fades out over the old room's `fade_out` (0.69d leave 0x2002A0140),
  in the same frame: a crossfade.
- **The same sound in both rooms:** it plays on, no restart, no fade.
- **Back in before it is silent:** it fades back in from where it is.
- **Fades:** linear, stepped on the sound thread (0.69d's curve NOT VERIFIED). Volume: the sample's
  `volume_k`; the Game slider scales it as every game sound.
- **Mono:** the bed plays as recorded. 0.69d's `force_mono` touches only third-person sounds from a
  room other than the listener's (its mixer's channel matrix, 0x20062B92B; a downmix LIKELY): not
  ported, a follow-up if a room's tails heard from outside it sound wrong.
- **Slots:** four beds at most at once (one fading in, three fading out); a fifth takes a fading one.
- **Map unload:** every bed stops at once (0.69d's destructor releases the instance, no fade).

## Data

- **Zones:** a top-level table `srp_sound_environments` of each map's client project (the edit
  list, every weather and mode of the map), read once at `project.insert`:
  `version` 1; `environments.<name>`: `room`, optional `ambient_sound` (a single sound) with
  `fade_in`, `fade_out` (ms); `meshes.<name>`: `kind` (1 box, 2 cylinder, 0 sphere, 0.69d's),
  `x y z` (centre), `r00`..`r22` (rows: local = R (world - centre)), `hx hy hz` (half sizes;
  cylinder radius, half height; sphere radius). At most 32 environments and 256 meshes a map.
  Data that does not parse is logged and the map is unzoned.
- **Maps with zones:** the nine present in both builds (Rudnya, School, Radar, Chemical Plant, Fort,
  Mamayev, Vector, Cologne, London) and the 0.69d Rudnya port, from 0.69d's `day_tdm` projects
  (0.69d's other modes and weathers share them). All nine are at 0.69d's coordinates (the most
  common offset of matched models is 0 on each).
- **Follow-up, no zones yet:** the STALKER and Dirty Bomb ports, the 0.21d and 0.11e maps; a zone
  editor could come later.

## Replays, spectating, the death camera

- Presentation only: no simulation change, no lockstep state, no `simulation_changes.tsv` row.
  Replays play the same; their sounds take the rooms by the same rules (the listener's room: the
  camera's when no living local player).

## Release impact

- The exe (patch 155) and the content (pack `room-acoustics@1`, about 63 MB of 0.69d audio, and the
  edit list) ship together; an old exe with the new content would play every room layer of a
  composite at once.

## 0.69d addresses (retail x64, base 0x200000000)

- Controller: level+3A8h (vtable 0x2009752D0, constructor 0x2001F8470), its built-in `default`.
- Tree build after level load 0x20023B2A0; lookup 0x20023AF50 (callback 0x20023AED0).
- Listener room: world tick 0x200236FF0's tail, stored at level+6C0h; enter 0x20029FF80, leave
  0x2002A0140; a player's room 0x200230400 (+23AAAh).
- Composite reader (tag at item+1Ah) 0x2006329EF; play 0x200635F70.
- Environment config 0x20029FBE0 (`environment_name`, `force_mono`, fades, `ambient_sound`).
