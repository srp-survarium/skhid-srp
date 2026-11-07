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
- Every 0.69d0 slot locator is in the model beside 0.26g's contract (`aim`, `barrel_point`,
  `scope_point`, `handguard_point`, the hands): its rails, muzzle and grip points at 0.69d0's names, for
  the attachments made later.
- A 0.69d0 material or texture at a path the players have with other bytes gets a `_069` name; the same
  bytes (an earlier port's: the AKMSU's magazine) are not shipped again.
- 0.69d0's models as they are (`donor-models-are-reference`): where 0.69d0's default gun differs from
  ours, ours changes. Seen in the renders: the 9A-91 without its suppressor (0.69d0's has none by
  default), the SR-3M without its vertical foregrip and with 0.69d0's magazine, the SOC Vepr's dark
  stock, the premiums' 0.69d0 camouflages.

- Camouflage: pack remodel-069-camo, an `_aoac` mask for each of the remodels' 208 diffuse textures
  (`remodel-069/camo.tsv`, `../weapons-069/camo_rows.py` from the bundle): 0.69d0's guns have no
  premium twin to compare, so every large part is painted (as most of our premiums were repainted as a
  whole); `parts.tsv` leaves the cartridges and the MP7's glowing sight dots plain.

## Not yet

- The scope variants: a variant that sets its own weapon model (a copy of the old model with its eye)
  still draws the old gun, and one that does not places its optic at the old scope point. They are made
  again for the remodels with scope-fits' approach (0.69d0's mounts, rails and sight swaps).
- The kit's muzzle devices (`weapon-attachments-1`) at the remodels' `barrel_point`.
- The pack is 1.4 GB (0.69d0's textures, 2048 pixels for most guns).
