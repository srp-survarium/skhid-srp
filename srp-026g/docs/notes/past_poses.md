# Others drawn in the past, hits rewound to what the shooter saw (exe patch 105)

Piece 2 of `pixel_correct_shooting_069.md` section 8 (PR #1274), built on patch 103
(`subtick_bullet_hits.md`). The owner approved building it (2026-11-05); merged only on his word,
after a feel test. The code: `client-patches/rust/patches/src/past_poses.rs`, the rewind in
`subtick_hits.rs`, the shared state in `crates/rewind`; the evidence `client_patch.md` 3.68.

## What players notice

- Other players are drawn a little in the past: by about how late their moves reach you (one age
  for everyone, at most 150 ms). What you see of them is what the world has confirmed, so a
  player who stops or turns no longer slides on and snaps back.
- A shot is tested against the target where you saw him: the hit you see is the hit every game
  computes, and a late move of his no longer takes it away.
- The cost: a target can still be hit up to that age after he reached cover on his own screen, on
  top of today's route between the two (about 90 -> 230 ms in the 30/150 ms example). Peeking
  players are seen later.
- Your own player, and the one the camera follows when you spectate, are drawn at the present.

## The rules

- **The ring** (world state). At the end of every 50 ms grid step each place's living player's
  feet (the simulated transform's place) go into his place's ring of the last 4 grid points.
- **The declared age** (world state). A game declares the age it draws the others at in the
  byte of a kit choice (0xDE, relayed as 0xF3, recorded in replays): `3 + target x 11 + level`,
  level 0..10 = 0..150 ms in 15 ms steps, target 0 every place (version 1) and 1 + p one place
  (a later version's: direct routes). 0..2 stay the kits; the place's tick ignores the rest.
- At the end of every grid step the place's newest kit choice at or before it is taken: a
  declaration is in force for the place's shots fired after its tick; a bullet fired at or before
  it (still in flight) keeps the age before. "Fired" is the bullet's segment origin (+5Ch): the
  shot's ms, a pierce's or ricochet's after one.
- Ring and age are the place's record, 73 bytes after his player state (after patch 78's carry):
  in every history item, the hash and an uploaded world, so a late joiner or a resynced game has
  them from its first tick.
- **The rewind** (patch 103's test). A target near the ray is tested `crossing + age` ms before the
  step's end: the crossing as patch 103 has it (never before the step's start), the age the
  shooter's record has for that shot, capped by the match (srp's block of the static info, tag 3,
  one byte ms; 150 without it, 150 at most). Within the step: patch 103's share of the step's
  move. Before it: from the step's start back along the ring, lerped, held at its oldest root and
  at the target's spawn (roots of his last life are never used). Translation only.
- An age of 0 (no declaration, a bot, a replay before one) is patch 103, bit for bit.
- Nobody dead, or spawned in the step, is put back (as patch 103). A shooter no longer in the
  world: age 0.
- **What is drawn** (display only). Every other living player at `now - age`, the age the
  player the camera looks from has in force (a shot of his at that moment is tested at the same
  age), from the same ring: his bones in mt_draw (so his weapon, muzzle flash and shadow follow)
  and his name tag. Not moved: tracers' starts and 3D sounds (the simulation's), corpses (drawn
  at the present: a death shows up to `age x speed` ahead, about 0.7 m at a run with 150 ms).
- **The age a game declares** (client only; "confirmed" is not knowable in 0.26g: inputs come on
  change, no complete-up-to time). Per other place, how late its inputs reach this game (the
  world's newest time less the input's, when it arrives, newest inputs only); its estimate the
  second latest of its last 16 (about its 90th percentile); the age the latest place's estimate
  and 10 ms, capped, rounded up to 15 ms. Declared when more than in force, or two levels less,
  at most once a second, only while the local player is alive, and never within 100 ms of a kit
  choice of his own (a place has one node a time: a pick on the death screen or before the start
  must not share a tick with a declaration; if one does, the pick wins and the age goes again a
  second later).
- The server relays the byte as any kit choice and counts only bytes 0..2 as kit changes.

## When a move comes later than the age

- The world re-simulates that past, as today: the tested pose changes and no longer equals what
  was drawn; every game still computes the same hit from the same inputs (the vote is
  unchanged). What was drawn was predicted for the part beyond the age.
- Measured on the recordings (`private/prediction-data`, the fitting worker's, read only): 60
  matches of 4 or more humans, 40 M (receiver, input) pairs; an input's lateness at a receiver =
  its server receipt after its game time + the receiver's median round trip / 2 (median 73 ms,
  p90 124, p99 187). This patch's estimator and sender rule replayed on them:

  | Age from | Mean age | At the cap | Inputs after the age | Excess then (median, p90) | Unconfirmed draw time vs today |
  |---|---|---|---|---|---|
  | v1, the latest place (built) | 135 ms | 50 % | 4.0 % | 16, 61 ms | 1.5 % |
  | v1, all places' samples' p90 | 125 ms | 33 % | 4.4 % | | 1.6 % |
  | v1, the places' median | 106 ms | 8 % | 10.0 % | | 3.5 % |
  | per place (a later version's pairs) | 101 ms | 12 % | 4.6 % | 15, 60 ms | 1.6 % |

  So about 1 input in 25 of another player still reaches you after the age: for 16 ms (median)
  he is drawn from a prediction, and a shot in that time is tested against where he really was.
  Today every input is: the time drawn from predictions falls to about 1.5 % of today's.
- The cost of v1's one value: it is the latest place's, so one far player ages everyone (half
  the time at 150 ms). A target can be hit about that much longer after reaching cover. The pair
  form (per place) gives the same confirmation at a mean of 101 ms.

## Composing with patch 104 (remote smoothing, PR #1279)

- Both draw an offset on the same sites: `player_draw.bones` (104 order 1, 105 order 2; the
  offsets add) and `name_tag.eye` (each a copy of the eye and a Skip: they must become one
  handler adding both offsets when the two meet).
- 104 measures a correction at the newest re-saved grid point; 105 draws at `now - age`. A late
  input newer than `now - age` changes the present, not what 105 draws: 104 would blend an offset
  in for nothing. With 105, 104 must take the correction at the drawn time (the re-saved point
  just before `now - age`), so it smooths only what 105 still draws from unconfirmed states.

## Checked

- `cargo test -p rewind` (the ring, the lerp, the clamps at the ring's end and the life's start,
  the clock's wrap, the cap, ages and levels, every declaration byte and the pair form, the
  declared age's transition, the record's bytes); the patch crate's tests (the rewind along the
  ring, an age of 0 bit for bit patch 103's, the estimate, the sender, the cap reaching the test).
- `check_subtick_hits_026g.py` (Unicorn, the game's own ray test): see its header.
- `check_world_roundtrip_026g.py`: the record written as read, in fresh games and a resync.
- Not played.
