# Every live gun on 0.69d0's model (pack remodel-069)

The owner, 2026-11-07: "let's start with all 0.69 guns and let's properly port them. We will need this
any way for other attachments later on." Every live gun item whose gun 0.69d0 has (96 of them; all but
the event MP7 and the six of pack weapons-069, which are 0.69d0's already) is drawn with 0.69d0's own
model of it. The item keeps everything else: its stats, states, clips, sounds and icons.

## How

- `scripts/remodel-069/sidecar.py` writes the list from 0.69d0's `gameplay/game.options` and the
  players' files: `remodel-069d.json` (resource-porter's remodel sidecar) and `items.tsv` (each live
  item, its config and model, its 0.69d0 item and its remodel). A live item takes the 0.69d0 item of
  its name: 0.69d0 has the premium items too, so a premium takes 0.69d0's premium model (the AKMN
  Premium's `akmn_camo`).
- `scripts/remodel-069/make_pack.sh` builds the pack with resource-porter's `remodel-weapons-069d`
  (`docs/weapons-069d-026.md` there): each gun's 0.69d0 model at `weapons/remodel_069/<item>/<item>`,
  its structural modules baked (receiver parts, stocks, grips, magazines, iron sights, handguards), on
  the live gun's rig (its skeleton, bind pose and hand points: the live clips play on it; a bone named
  otherwise is mapped, the VSS's spare magazine; a bone only 0.69d0 has rides on the body).
- `content-edit/remodel-069-1.json` sets each config's `object/model` to its remodel.

## Rules

- The gun's own default muzzle device (or bipod) is baked, as a surface of its own named after its
  module (`<module>_<surface>`): the gun looks as it does today, and a later attachment leaves that
  surface out by name. Default optics and every extra 0.69d0's premium items bundle (optics, lasers,
  other brakes) are not baked: the scopes and the kit attach them.
- 0.26g's contract locators (`aim`, `scope_point`, `barrel_point`, `handguard_point`, the hands) are
  the live model's: every scope item and variant, the muzzle flash and the kit's devices sit where they
  sit today, the aim with its config's `aim_offset` too (0.69d0's aim is the eye for 0.69d0's pose and
  has no `aim_offset`). Every 0.69d0 slot locator is in the model beside them: its rails, muzzle and
  grip points at 0.69d0's names, for the attachments made later.
- Most of 0.69d0's guns lie on the live model's mesh to the millimetre (`remodel-069/fit.py`); six it
  placed otherwise on the same rig are shifted onto it (sidecar.py's `OFFSET`: the MP-153 2.2 cm
  higher, the Benelli M4, SPAS-12, TOZ-34, 9A-91 and Uzi), so the live hands, sights and points fit.
  The Uzi's 0.69d0 rear sight stands taller than ours: its aim is on 0.69d0's own sight line
  (`AIM_069`). The iron sights were compared in aim renders, live and remodel, for all 96.
- Under an optic a gun is drawn as 0.69d0 draws it under one (0.69d0's `_scope` and `_collimator`
  modules all do the same): each sight in a slot they swap at its `model_param1`, folded (the M4A1's
  front sight becomes its gas block, the Vepr's leaf lies down), and no module in a slot they take (the
  M4A1's, L85A2's and Fort-401's rear sights make way). A gun that carries an optic by default (its
  config's rifle scope: the HK416, MSBS, Kriss Vector, P90 and the sniper rifles) is so itself
  (`optic`); a gun with sights that change has a second model so, `<item>_optic` (`optic_model_path`:
  the M4A1, L85A2, Fort-401, SCAR-H, SVU-A, Vepr, Vepr Premium and VHS-2), which its scope variants draw.
- A 0.69d0 material or texture at a path the players have with other bytes gets a `_069` name; the same
  bytes (an earlier port's: the AKMSU's magazine) are not shipped again.
- 0.69d0's models as they are (`donor-models-are-reference`): where 0.69d0's default gun differs from
  ours, ours changes. Seen in the renders: the 9A-91 without its suppressor (0.69d0's has none by
  default), the SR-3M without its vertical foregrip and with 0.69d0's magazine, the SOC Vepr's dark
  stock, the premiums' 0.69d0 camouflages.

- Camouflage: pack remodel-069-camo, an `_aoac` mask for each of the remodels' diffuse textures but
  those an earlier port has masked (203; `remodel-069/camo.tsv`, `../weapons-069/camo_rows.py` from the bundle): 0.69d0's guns have no
  premium twin to compare, so every large part is painted (as most of our premiums were repainted as a
  whole); `parts.tsv` leaves the cartridges and the MP7's glowing sight dots plain.

- Scope variants: a live variant that sets its weapon's model to a copy of the gun's (its eye on the
  optic) draws a copy of the remodel instead, with the locators the copy set otherwise (100 copies,
  pack remodel-069-variants, `remodel-069/variants.py`, `variants.tsv`; edit list
  `remodel-069-variants-1.json`). One for a gun and its premium draws the first gun's remodel on both,
  as it draws the first gun's copy today. A variant that keeps its gun's model draws the remodel, or
  the folded one where the gun has it (a modifier added). The variants of the M4A1 and Vepr were
  compared in aim renders: the dots and reticles sit where they do today, and no sight stands in the
  optic.

## Not yet

- A scope variant's optic still sits where it does today, on the live optic's own mount, not on a
  0.69d0 mount and rail.
- The packs of the unmerged scope PRs (rifle-scopes, scope-fits) are made on the live models: once
  they are in, their variants are made again on the remodels.
- The kit's muzzle devices (`weapon-attachments-1`) sit at the live `barrel_point`, as today; leaving
  the baked default device out under a kit device (its surface is named for that) is not done.
- The pack is 1.4 GB (0.69d0's textures, 2048 pixels for most guns).
