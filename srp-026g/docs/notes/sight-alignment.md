# Sight alignment: collimator dots at the screen's centre (2026-11-05)

The owner's report: some live collimators are off when aimed, the 9A-91 for one ("the problem is
from retail"). Tool `scripts/sight-alignment/` (README there), edit list
`scripts/content-edit/sight-alignment-1.json`. Context: [weapon_scopes.md](weapon_scopes.md).

- **Where the bullets go:** the screen's centre (weapon_scopes.md). A collimator is aligned when
  its dot is there.
- **The aimed view** (retail `weapon::calculate_aim_alignment` 0x244D40 -> `calculate_locator`
  0x243BC0, `model.py`; `check_engine.py` compares it with the exe under Unicorn): the camera is the
  weapon model's `aim` locator on its bone, `translate(0, 0, aim_offset) * rotate_y(pi) * locator *
  bone`. The gun's mesh and `scope_point` hang on the `offset` bone, which the weapon's item idle
  clip poses against the aim's root bone: the identity on most guns, turned and moved on the 9A-91
  (8.8 mm aside, 21.4 mm up, -0.0148 rad about x), the Val, Vepr, SR-2M, UMP45 and others.
- **The fault, in retail's data:** 0.29a3's fitted collimator models (base patch 19 shipped them as
  they were) kept the iron sights' eye turn (the Ash-12's 0.017 rad, the Kiparis's 0.010, the
  Vityaz's 0.008) or took none where the idle pose turns the gun (the 9A-91's Cobra: eye 0.009,
  gun -0.0148), or have the eye a few mm off the dot's line (the Benelli). The 9A-91's Cobra dot shows
  14.5 px above the centre at 720 lines.
- **The fix:** each such model's `aim` looks along the optic's own axis (its posed frame at
  `scope_point`), moved across that axis onto the dot's line, as the reference variants' unturned
  eyes do; nothing else changes, the model edited in place (a model of collimator pairs only).
  `align.py plan` makes it for every collimator off by more than 1.5 px at 720 lines (about half the
  dot); `align.py export` checks it into the edit list. 24 models, 28 pairs (content-edit README).
- **Left:** the AK family's collimators (1 px, a 0.004 rad eye 1.3 mm off), the IZh-27's, SR-3M's
  and VZ.58's (1.1 to 1.2 px); the EOTech 552 on the HK416 and MSBS (an emissive ring, no dot mesh:
  the tool wants an explicit pick). Optical scopes are #958's.
- **Not seen in a game:** the numbers are the engine's formula on the files, at the idle clip's
  first key; sway, recoil and the aim-in blend are not modelled.
