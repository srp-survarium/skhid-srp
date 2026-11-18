# Sight alignment: the 9A-91's Cobra dot at the screen's centre (2026-11-05)

The owner's report: the 9A-91's collimator is off when aimed ("the problem is from retail"). Tool
`scripts/sight-alignment/` (README there), edit list `scripts/content-edit/sight-alignment-1.json`.
Context: [weapon_scopes.md](weapon_scopes.md).

- **Where the bullets go:** the screen's centre (weapon_scopes.md). A collimator is aligned when
  its dot is there.
- **The aimed view** (retail `weapon::calculate_aim_alignment` 0x244D40 -> `calculate_locator`
  0x243BC0, `model.py`; `check_engine.py` compares it with the exe under Unicorn): the camera is the
  weapon model's `aim` locator on its bone, `translate(0, 0, aim_offset) * rotate_y(pi) * locator *
  bone`. The gun's mesh and `scope_point` hang on the `offset` bone, which the weapon's item idle
  clip poses against the aim's root bone: the identity on most guns, turned and moved on the 9A-91
  (8.8 mm aside, 21.4 mm up, -0.0148 rad about x), the Val, Vepr, SR-2M, UMP45 and others.
- **The fault, in retail's data:** 0.29a3's Cobra model for the 9A-91 (base patch 19 shipped it as
  it was) keeps the iron sights' eye turn (+0.009 rad) where the idle pose turns the gun -0.0148: the
  dot shows 14.5 px above the centre at 720 lines.
- **The fix:** that model's `aim` looks along the optic's own axis (its posed frame at
  `scope_point`: rotation (-0.01476, 0, -0.00607)), moved 3.8 mm down and 0.5 mm aside onto the dot's
  line, as the reference variants' unturned eyes do; nothing else changes, the model edited in place
  (only the Cobra variant names it). Made by `align.py plan` and `export`, cut to that one entry.
  Checked in the game by the owner (2026-11-05): centred.
- **Known, left alone by the owner's choice** (2026-11-05: "others I don't care about, they seemed
  to be fine either way"): `align.py plan` finds 23 more collimator models further than 1.5 px
  (about half the dot) off, px at 1280x720 (right, up):

  | pair | offset |
  |---|---|
  | Ash-12 + CompM4 / EOTech | -0.1, +9.7 / +9.8 |
  | UMP45 + EOTech | +0.9, -9.3 |
  | Vepr (+ premium) + Cobra | +0.5, +6.7 |
  | Val + Cobra | -0.3, +5.8 |
  | Kiparis (+ premium) + Docter | 0.0, +5.5 |
  | Fort-17 + DeltaPoint | -0.5, +5.2 |
  | SR-2M + CompM4 / EOTech | +0.5, +5.0 |
  | Vityaz (+ premium) + CompM4 / Cobra / EOTech | 0.0, +3.0 to +3.2 |
  | Remington 870 + CompM4 / Docter | -2.2 / -1.8, -1.9 |
  | Benelli M4 + CompM4 / Docter / EOTech | 0.0, +2.2 to +2.3 |
  | Saiga-12 (+ premium) + Cobra | +0.2, +2.3 |
  | Valmet + CompM4 / EOTech | -0.5, +2.2 |
  | Kriss Vector, its own Walther | -0.6, -2.2 |

  Within 1.5 px: the AK family (1 px), the IZh-27, SR-3M, VZ.58, UMP45 + CompM4, Magnum. Not
  measured: the EOTech 552 on the HK416 and MSBS (an emissive ring, no dot mesh). Optical scopes
  are #958's.
