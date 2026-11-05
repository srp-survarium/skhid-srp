# Bullets hit players where they stood when crossed (exe patch 103)

Piece 1 of `pixel_correct_shooting_069.md` section 8 (PR #1274), the owner 2026-11-05:
"then let's do 1". The code is `client-patches/rust/patches/src/subtick_hits.rs` and
`src/vostok/ray_test.rs`; the evidence `client_patch.md` 3.67.

## What players notice

- A shot at a player running across the crosshair lands where he was when the bullet got
  there. Before, the game tested him up to 50 ms later (where the 50 ms step left him): up to
  25 cm off at a run, the trailing edge of a strafer missed, the leading edge hit.
- Nothing changes on screen, in the protocol or on the server.

## The rules

- Bullets are still tested on the 50 ms grid; at a grid step each player near the bullet's
  ray is tested where he stood at the ms the bullet reached him.
- That ms is the game's own hit time of the ray's point nearest his middle (0.9 m above his
  feet). One pass: in a step a bullet flies ~35 m, a player moves ~25 cm.
- Where he stood: back along the move his character controller made in the step (from
  `m_last_pos` to its position: world state every game holds, a late joiner's and a resynced
  one's too), by the share of the step between that ms and the step's end.
- A step the world took in parts (an off-grid input, a fire press, splits it): the last part's
  move, never further back than its start.
- Translation only: his bones keep the step's pose about his feet; crouching, turning and the
  animation are the step's.
- Nobody is put back who is dead, or was inserted (spawned) during the step.
- The level (walls, batteries, everything but hit models) is tested at the step's end, as
  before. The closest hit wins; a tie goes to the level, then to the lower place.
- The shooter's own body is never hit (the game's own filter), as before.
- No player moving near the ray: the game's own result, byte for byte.
- Nothing of the physics world is moved: a player is tested at a copy of his transform.

## What it does not do

- No rewind for the shooter's view here: that is patch 105 (`past_poses.md`), which tests a target
  further back by the age his shooter declared, along a ring of his grid roots; with no age
  declared a target is never put back before the step the bullet is ticked in.
- No change to explosions, grenades, melee or traps.
- No change to what is drawn.

## A simulation change

Every game computes the same hits from the same world, so worlds stay in agreement; replays
recorded before it play out otherwise and are refused (`simulation_changes.tsv` `next`).

## Checked

- `cargo test` (the crossing time, the share, the clamp at the step's start and end, a split
  step, a bullet spawned in the step, a ray of no length, the wrap of the clock).
- `check_subtick_hits_026g.py`: the game's own ray test end to end in both exes on made-up
  bodies; a still target byte for byte the original's; a moving target the original misses is
  hit exactly as the original hits him standing where he stood; walls, the shooter, a spawned
  or dead target, a split step; nothing of the world written.
- Not played.
