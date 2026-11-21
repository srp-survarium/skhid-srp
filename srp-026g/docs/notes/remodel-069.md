# Every gun on 0.69d0's model, in two bodies (pack weapon-modules)

The owner, 2026-11-07: "let's start with all 0.69 guns and let's properly port them. We will need this
any way for other attachments later on." Every live gun item whose gun 0.69d0 has (102 of them; all but
the event MP7) is drawn with 0.69d0's own model of it. The item keeps everything else: its stats,
states, clips, sounds and icons. Since 2026-11-20 this is the foundation of the weapon attachments
(`attachments.md`): each gun has two bodies, and 0.69d0's modules go on them by the modules table.
Built 2026-11-20 (pack weapon-modules@1); not played.

## How

- `scripts/remodel-069/sidecar.py` writes the list from 0.69d0's `gameplay/game.options` and the
  players' files: `remodel-069d.json` (resource-porter's sidecar) and `items.tsv` (each live item, its
  config and model, its 0.69d0 item and its remodel). A live item takes the 0.69d0 item of its name:
  0.69d0 has the premium items too. The six guns of pack weapons-069, 0.69d0's models already, are
  remodelled all the same, for their bare bodies.
- `scripts/weapon-modules/make_pack.sh` builds the pack with resource-porter's `port-modules` (its
  `docs/modules-069d-026.md`): the bodies at `weapons/remodel_069/<item>/<item>` (whole) and
  `<item>_bare`, the modules at `weapons/modules_069/...`, the table at `gameplay/weapon_modules.srpm`.
- `scripts/weapon-modules/edits.py` writes the edit lists: `remodel-069-1.json` (each config's
  `object/model` is its whole body) and `weapon-modules-1.json` (`data/srp_gun`, its record of the table).
- `scripts/weapon-modules/ids.tsv` (ids.py, append-only): the dict id of every module a player fits.
- `scripts/weapon-modules/review.py` makes the review pages of a build (`private/renders/attachments`).

## Rules

- **Two bodies.** `whole`: the gun with every own part it shows, its default muzzle device, bipod or
  foregrip too; drawn when nothing is fitted. `bare`: the same without the own parts a fit can displace
  (0.69d0's own rules decide which: a fit or its carrier goes in the part's slot, closes it or folds
  it); the same skeleton, bind pose and locators. 95 own parts of the 102 guns are such.
- **On the live rig**: the live model's skeleton, bind pose and hand points, so the live clips play. A
  bone named otherwise is mapped (the VSS's spare magazine); a bone only 0.69d0 has rides on the body.
- **0.26g's contract locators** (`aim`, `scope_point`, `barrel_point`, `handguard_point`, the hands) are
  the live model's in both bodies: the sights, the flash and a default optic are where they are today.
  Every 0.69d0 slot locator is in the model beside them.
- **The shift is measured.** 0.69d0 placed most guns on their rig as 0.26g did and a few otherwise. The
  build lays each remodel on its live mesh by a trimmed nearest-point fit and applies the shift when it
  is 1 mm or more; nothing is typed. Measured 2026-11-20 (mm; the median gap before and after):
  9A-91 (8.8, -20.7, -0.4; 11.0 to 1.9), Benelli M4 (0, -24.3, -21.3; 17.5 to 0.1), MP-153 (0, -22.2,
  -1.2; 15.2 to 0.1), SPAS-12 (0, -20.3, -4.4; 15.0 to 8.4: another shape), TOZ-34 (0, -2.1, -0.8; 2.3
  to 0.1), Uzi and its two premiums (0, -17.6, -18.4; 11.3 to 4.1: its sights). Every other gun: none.
- **A bone the live rig binds otherwise** than 0.69d0: its locators take 0.69d0's place with the shift,
  as the mesh around them, unless the gun's own parts on that bone lie closer on the live mesh at the
  live bind frame (the AK family's magazine bone, bound 15 cm from the well in 0.69d0; the L85A2's).
- **Under a default optic** (the live config's rifle scope: the HK416, MSBS, Kriss Vector, P90, the
  sniper rifles) the whole body has its sights folded and no part in a slot an optic takes, as 0.69d0
  draws a gun under one; the game draws that optic itself. The second `_optic` bodies and the scope
  variants' copies of 2026-11-07 are gone: a fitted scope is the table's.
- **A suppressor the live gun has built in** is the gun's own part when 0.69d0's linked suppressor lies
  on the live mesh (median gap 1.5 mm or less): the VSK-94 and its premium. Never an item.
- The Uzi's 0.69d0 rear sight stands taller than ours: its aim is on 0.69d0's own sight line
  (`AIM_069`, still a typed set in sidecar.py).
- A 0.69d0 material or texture at a path the players have with other bytes gets a `_069` name; the same
  bytes (an earlier port's) are not shipped again.

## What changes to the eye

`private/renders/attachments/look.html` shows every gun as it is today beside its remodel, the most
changed first, with what each has that the other lacks. 0.69d0's models as they are
(`donor-models-are-reference`): where 0.69d0's default gun differs from ours, ours changes. Seen:
the 9A-91 without its suppressor (0.69d0's has none by default, and its VSK-94 suppressor is not the
live one's shape); the SR-3M without its vertical foregrip (now a grip a player fits) and with 0.69d0's
magazine; the SPAS-12 and the Uzi in another shape; the AK-74M's bakelite pistol grip; the SOC Vepr's
dark stock; the premiums in 0.69d0's camouflages.

## Not yet

- Camouflage masks (`_aoac`) of the remodels' textures: `remodel-069/camo.tsv` and `parts.tsv` are those
  of 2026-11-07 (96 guns) and no mask pack is built for this pack. To make again by the rule of
  `fix/weapon-camo-coverage` once it is merged (`references.py --write`, `--v069`). Parts drawn
  separately stay plain under a camouflage (the owner's decision for now).
- Device items, their texts, icons, prices and shot effects; the scope items' models on the new origin.
- `release/build-content` was not run with the pack and its edit lists.
- 1.7 GB.
