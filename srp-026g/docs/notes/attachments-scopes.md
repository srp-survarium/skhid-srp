# Scopes on the modules table, and the move of the scopes players own

The scopes lane of the weapon attachments ([attachments.md](attachments.md); its contract section is
what this note builds on). Client 0.26g, 0.69d0 the reference for content. Tags: PROVEN (read in the
data or the code named, or seen in a local game, which is said), LIKELY, OPEN. Before this, a scope
went on a weapon by a variant: [weapon_scopes.md](weapon_scopes.md), kept as the record of releases
19 to 68.

## What a scope is now

- **One item an optic.** A shop scope item (dict ids 2101-2120, twenty of them) is the optic: its
  config's `data/idle_model` and `data/aimed_model` are the optic's own pair of models of the pack
  `weapon-modules` (0.69d0's, their origin the frame of the slot the optic goes in), and its
  modification adds `data/srp_scope` = its own dict id to the weapon that wears its record
  (`scripts/content-edit/scope-modules-1.json`, made by `scripts/scope-fits/modules_edit.py` from
  the pack's record; the items' zoom, prices, texts and icons stay; the modification's id is
  994000 + the item's dict id, the fitted items' rule).
- **The record** on a weapon is `{scope item id, the scope item's own dict id, 0, 3}`. No variant
  ids. The game's cook takes `data/srp_scope` as the scope item in place of the gun's own
  `addons/rifle_scope_dict_id` (patch 161, the core's), so a gun's default optic stays in its
  config and comes back when the scope is taken off.
  - Why not `addons/rifle_scope_dict_id` by the modification, as the variants did: one
    modification cannot do it on every gun. A type 0 set is skipped where the gun has no such node
    (every gun without a default optic); a type 3 add where it has one is the client's
    duplicate-name assert (0x3581C0). The variants were made per pair, set-form or add-form.
- **Where it stands and what the eye is** is the table's: the optic's slot on the gun (0.69d0's, its
  rail carriers with it), the eye the optic's own `aim` locator (`attachments::EYE`, anchored on the
  gun's aimed idle pose). The baked per-pair copies (a moved optic, a gun body with a moved `aim`,
  1,462 gun bodies) are not used any more.
- **`parameters/aim_offset`.** The game adds the gun's `aim_offset` (a translation along the eye's
  own line: the eye relief of its iron sights, or of its default optic on the fourteen guns that
  have one; -0.11 to +0.08 m) to whatever the eye is (`calculate_aim_alignment` 0x244D5B). A scope
  item's modification sets it to 0, so a fitted optic's eye is the table's alone; a gun without the
  key has none, and the modifier is skipped there. It stays the knob, now per optic: a value in a
  scope item's modification moves that optic's eye along its line on every gun.
- **A collimator's reticle.** 0.69d0 draws it as a model of its own at the optic's eye
  (`possible_reticles`, the first until a player picks another); 0.26g draws a scope's aimed model
  alone. The pack puts the players' own emitter (patch 19's `collimator_glowing_dot_s`: the dot, the
  EOTech's ring for the two EOTechs, whose first reticle is `eotech_552_o_reticle`) into each
  collimator's aimed model, its middle on the line of the optic's eye, 0.6 m before it, and leaves
  the aimed model's first-person lens out, as every released collimator has it. A magnified scope's
  reticle is its aimed model's own (`scope_aimgrid_s`, `scope_reddot_s`).

## No released pair is lost

`scripts/scope-fits/released-pairs.tsv` is the record of release 68: 1,205 (weapon, scope) pairs of
20 scopes on 102 weapons, each with the variant that showed it, made once by
`scripts/scope-fits/released.py` from release 68's files (the same pairs and variants as the
server's own parser gave, compared row by row). Everything below is checked against it.

- **In 0.69d0's own data: 1,159 pairs** (computed from the pack's record of 2026-11-20).
- **The other 46 are legacy rows** of the table, made by the porter from the same list
  (`port-modules --released`, resource-porter `donor_069d/modules/legacy.rs`):
  1. *as its base gun*: a premium item with links of its own but no optic among them takes the optic
     as its base gun does, where 0.69d0's rules place it on the item's own parts (the SR-3M
     premium's eighteen);
  2. *where its variant put it*: the optic's 0.69d0 mesh is found in the variant's idle model
     (vertex for vertex, else by its box), which gives one translation in the frame of the gun's
     `scope_point`; the gun's record gets a slot of the optic's own there (the first slot its
     module names that the gun lacks: the Docter's is 0.69d0's slot `none`, which no 0.69d0 gun
     has) and the optic a row of its own in it, with 0.69d0's rules for that slot. The variant's
     rail surfaces become a carrier of that gun alone, a model made of them as they are.
  Nothing is typed per pair. A pair that can be neither given nor placed is listed as lost by the
  build, with its reason.
- **The Mosin's PU 3.5x** (2103) is a 0.69d0 module (`pu_35x22_scope`, slot `mosin_scope`): a plain
  fit once it has its row in `scripts/weapon-modules/ids.tsv`.
- **`wpn_mp7_silencer_special`** (three released pairs) is no gun of the table yet: it needs its row
  in `scripts/remodel-069/items.tsv` (the core's), as the MP7 with a silencer whose model it has.
- **19 pairs are new** (0.69d0 gives them and no variant did): eleven on the MP7 with a silencer,
  eight on the two Neosteads.

## The account's load

A released record is `{scope item id, variant dict id, 0, 3}`: it names the owned scope item, so
the move needs no table. At an account's load every slot-3 record whose item is a scope item
(category 22) gets that item's own dict id; then the usual repair runs (`Modules::check`), and a
record whose pair the table does not have is taken off and logged with its reason. The player keeps
the scope item either way. A kept results screen loses a record that still names a variant (it is a
picture of a past match).

## The reticle's centre

*The rule.* Aimed and at rest, the reticle's middle is on the eye's line: the game moves the gun
until the eye's frame sits on the camera, the eye's record is the optic's own eye in the gun's aimed
idle pose, and the reticle lies on that eye's line in the optic's own model. So the centre does not
depend on the gun, the slot or the carriers; what can move it is the gun's aimed idle clip moving
the optic's bone against the bone the gun's own `aim` hangs on.

*The numbers.* The porter's record has, for every scope fit of every gun (`centres`), the angle
between the eye's line and the reticle's middle through the game's own two formulas
(`calculate_locator`, `calculate_aim_alignment`) on the records the client writes, at the start,
the middle and the end of the gun's aimed idle clip. One pixel at 1280x720 is 1.57 mrad at x1
(uro_fov 65) and 0.39 mrad at x4.

## The eye's plane (`plane_slot`)

Read in 0.69d0's exe (`private/music-research/retail-069d.exe`, the re-kit's identity exe; static,
and the two functions run in Unicorn, 400 plane cases; names in `tools/re-kit/v069d/functions.tsv`).
Not in the table yet: the crate's change (a flag on an attach's eye, table version 3) is the core's.

- **The data** (PROVEN): every collimator, every rear iron sight and the dovetail, pistol and G3
  rail mounts move the eye as `slots_to_override { id: aim, locator: aim, plane_slot: body_aim }`
  (39 keys, all `body_aim`; 34 on `aim`, 5 on the piggyback sights' `aim_3..5`). No magnified scope
  has it. Every gun has a `body_aim` slot (its model's `aim` locator on most).
- **The reader** (PROVEN): `parse_slot` 0x200385FF0 keeps `find_slot(weapon, name)` in the slot
  record at +0x88 (0x2003861EF), null without the key. Its one reader is
  `get_slot_transform` 0x2006CF320 (0x2006CF46E), which evaluates every slot.
- **The rule** (PROVEN; row vectors): a slot's frame M is its locator's, whole. With a plane slot,
  whose frame P the same call evaluates on the same bones, only M's origin moves, along M's own k
  (the sight line), to the plane through P's origin normal to P's k:
  `s = dot(P.c - M.c, P.k) / dot(M.k, P.k)`, `M.c += s * M.k`, when `s >= -2.0`; a plane more than
  2 m behind the locator, or a line parallel to it, leaves the locator whole. The axes are never
  changed. So a collimator gives the line of sight and the gun keeps its own eye plane; a
  magnified scope brings its eye relief with it.
- `parameters/aim_offset` is added after, along the same k, either way
  (`calculate_aim_alignment` 0x2002B9070), as in 0.26g. The eye is evaluated in first person while
  aiming and nowhere else.
- **Here, until version 3:** a collimator's eye is the optic's own `aim` point, nearer the optic
  than 0.69d0 has it. The reticle's centre is the same either way (the eye moves along its line).
- Not established: what 0.69d0's `Item` bone holds (LIKELY the camera's frame, as in 0.26g); which
  module wins when two override one slot.

## State, 2026-11-21

Written and not yet run, unless said: nothing of this lane has been seen in a game, and no pack has
been built with the porter's scope changes.

| Part | Where | State |
|---|---|---|
| The released pairs' record | `scripts/scope-fits/released.py`, `released-pairs.tsv` | made from release 68's files; the same 1,205 pairs and variants as the server's parser gave |
| The scope items' edit list | `scripts/scope-fits/modules_edit.py` | written; `scripts/content-edit/scope-modules-1.json` is not made yet (it needs a full pack's record with the porter's `optics`), so it has no row in `release/content.tsv` |
| A build's scopes checked from its record | `scripts/scope-fits/check_record.py` | written, not run; no row in `tools/checks.tsv` yet (the record must be kept beside the pack first) |
| The collimators' reticle, the legacy rows, the centre numbers | resource-porter `feat/attachments-069d-scopes` (`donor_069d/modules.rs`, `modules/legacy.rs`) | type-checked, not run |
| The servers on the modules table, the load's rewrite, the preservation checker | `crates/survarium/src/v026g/scopes.rs`, `modifications.rs`, `crates/session/src/v026g/items_rules.rs`, `bin/srp-check-scope-fits.rs` | see the commits' own messages for what compiles and which tests pass |
| `plane_slot` | above; `tools/re-kit/v069d/functions.tsv` | read and run in Unicorn; the crate's version 3 is the core's |

Not done: the AK-74M with its eighteen scopes in a game; a default-optic gun with a fitted scope
(the first proof of the cook's `data/srp_scope` site); the legacy rows built and looked at; the
centre numbers for every fit and pictures across mounts; the shot's kick under a scope with #2294;
the `wpn_mp7_silencer_special` gun's row (the core's `items.tsv`); retiring the variant packs and
their edit lists (last, after the branch is merged, on the coordinator's word).
