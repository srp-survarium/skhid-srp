# The aimed view is aligned to the gun's idle pose (patch 165)

The owner, 2026-11-18 and 2026-11-20: guns should behave on a shot in sights and scopes as build
0.45a6 shows it. Both exes were read; the proof is `client_patch.md` section 3.87. The clips' half
is `shot-kick-045.md`.

## What the two builds do

- **Both** lock the gun to the camera at full aim: the hands' `Weapon` bone is overwritten with
  inverse(alignment) x the `Item` bone (the camera), so the hand and recoil clips cannot move the
  gun. What still moves is the gun's own fire clip: its `offset` bone, the body's kick, which the
  mesh and the scope's picture (`scope_point`) hang on. An optical scope hides the gun and the
  hands in both builds; the scoped view is the scope's aimed model alone, on that bone.
- **0.26g** aligns to the `aim` locator on its bone as the clips pose it this frame
  (`calculate_aim_alignment` 0x244D40, the `calculate_locator` call at 0x244D66). A gun whose
  `aim` hangs on `offset` therefore shows no kick at all when aimed: the camera follows the kick.
  That is the ten ported guns Beretta 93R, Colt M1911, CZ 75, Fort-401, Glock 17, Jackhammer,
  L85A2, MP-443, MP5 and SVU-A; the stock guns keep `aim` on the root and show the kick.
- **0.45a6** aligns to that bone's pose in the idle clip at time 0, kept from the weapon's
  creation (0x6E4EF0 fills weapon+50E4h, 0x6E5260 reads it). Its aim locators all end on the
  mount's bone (`offset`), and every gun's kick is seen. It has no other kick mechanism for the
  aimed view: no blend, no kick from the recoil state, no field-of-view punch.

## The rule

- **The camera is the `aim` locator at the gun's idle pose.** On an aimed frame on which the
  gun's idle clip alone poses its bones (the weapon in its idle state, one animation of the gun
  in its user's mixing tree), the weapon's aim locator is moved from its bone into the gun's
  model at that pose (locator x bone, on no bone). The game's own alignment reads it from then
  on; a new weapon starts again.
- **The pose is taken again on every such frame** of the gun posed last (the patch keeps what
  its locator was). The same pose writes nothing. A world loaded from another game carries the
  weapon's state and the tree as that game sent them; should one say idle with the gun off rest,
  the pose it gives lasts until the next frame at rest, not for the weapon's life.
- **Until then** the alignment is the original's (a player who fires before ever aiming at rest
  sees the old behaviour until the shot's clip has faded).
- **At rest nothing moves:** the idle pose is the pose the sights and scopes were fitted at
  (`sight-alignment.md`), so every dot and reticle stays where it is.
- **Iron sights too,** as in 0.45a6: on those ten guns the body now kicks under the eye, as every
  stock gun's does.

## What it does not change

- **Drawing only.** The locator is read by `calculate_aim_alignment` alone and is not in a world;
  the recoil, the spread, the shot and the view's climb are the game's. No
  `simulation_changes.tsv` row; games with and without it play together.
- **Guns whose `aim` is on a bone their clips do not move** (the stock guns): the same picture as
  before. Their kick changes with the clips (`shot-kick-045.md`), not with this patch.
- **Not ported, each a change of what a match computes** (asked of the owner first): 0.45a6's
  camera shake on every shot (`min/max_shake_recoil` 1/1 on the SVD, 0/0 here), its first-person
  recoil pose clips, `recoil_time`, the second recoil calculator used at full aim (`aim_recoil`),
  and 0.46c2's per-shot pattern (the owner, 2026-11-18: not for now).

## Limits

- The idle pose is taken from the bones, not evaluated from the clip as 0.45a6 does, so it is the
  idle clip's pose at the frame it is taken: the same thing while the idle item clips hold the
  aim's bones still, which they do in today's content (103 live weapons read: 0.000 mm). A
  pistol's and a double-barrelled gun's other idle clips (by the rounds loaded) give those bones
  the same pose (within 0.001 mm).
- Only the gun posed last is posed again: after a switch to another gun and back, the first
  keeps the pose it has.
- `check_aim_pose_026g.py` (`tools/check aim-pose`) runs the site through the game's own
  alignment in both exes.
