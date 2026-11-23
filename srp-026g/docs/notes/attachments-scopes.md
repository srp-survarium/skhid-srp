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
- **A collimator's reticle.** 0.69d0 draws it as a model of its own in the optic's `reticle` slot,
  which is the optic's `aim` locator (`slots_to_add { id: reticle, locator: aim }`): a quad whose
  texture's alpha (the atlas `collimator_reticles`) draws the shape, an ordinary forward material, not
  at infinity (`possible_reticles`, the first until a player picks another). 0.26g draws a scope's
  aimed model alone, so the pack puts the players' own emitter (patch 19's `collimator_glowing_dot_s`:
  the dot, the EOTech's ring for the two EOTechs, whose first reticle is `eotech_552_o_reticle`) into
  each collimator's aimed model at that same place, scaled to 0.69d0's lit size (the dot's half-lit
  width 0.63 mm, `ncstar_4_dot_04`; the EOTech ring 6.4 mm along its line), so it subtends 0.69d0's
  angle from any eye; and leaves the aimed model's first-person lens out, as every released
  collimator has it. A magnified scope's reticle is its aimed model's own (`scope_aimgrid_s`,
  `scope_reddot_s`).
- **The glass.** An idle optic's outside lens (`scope_lense_3p_s`: the front and the rear glass in
  one surface, two discs at the tube's ends) is see-through (pack version 5):
  `weapons/modules_069/scope_lense_3p_glass`, the stock PSO-1's lens material
  (`weapons/pso_1_lens`, `forward_lighting_no_distort`) on patch 19's lens normal and gloss maps,
  and a texture of its own made from patch 19's lens map: opacity 40/255 and a light cool tint
  (90, 110, 120) in the glass, the map's dark opaque rim kept. 0.26g's lens materials draw so: the
  forward stage's alpha is the transparency map's alpha times `constant_transparency`
  (`forward_base.ps`), and the lit stage scales its diffuse by the diffuse map's alpha and adds its
  specular whole (`forward_lighting.ps`), so the highlights stay where the glass lets the world
  through. The stock scopes' own lens maps are the same (alpha 0 in the glass, 255 on the rim).
  An aimed optic's first-person lens (`scope_lense_1p_s`) stays on `weapons/loc2/scope_lense_1p`.
  Behind the glass (a ray test of the models): the tube's inside is modelled in ten optics; the
  Leupold 3-7, the PSO-1 old and the PU 3.5x have none, and through their glass at an angle one
  sees the back of the body's outer shell (their materials are two-sided); the 1P69 Hyperon is
  closed inside (a part 9 to 25 cm along its tube), so its glass shows that, not the world.
  Version 3 had remade the outside lens's material (0.69d0's is an opaque stage with all its maps)
  on the AK-12's, whose Fresnel map (`weapons/ak_12/ak_12_fmap`) drew the AK-12's parts in the
  glass of the 13 idle optics that have one (the owner, 2026-11-23); version 4 put it on patch 19's
  opaque `weapons/loc2/scope_lense_3p`, as the variants had it (seen in a game: the picture gone;
  the owner wished it were see-through).

## No released pair is lost

`scripts/scope-fits/released-pairs.tsv` is the record of release 68: 1,205 (weapon, scope) pairs of
20 scopes on 102 weapons, each with the variant that showed it, made once by
`scripts/scope-fits/released.py` from release 68's files (the same pairs and variants as the
server's own parser gave, compared row by row). Everything below is checked against it: the
server's `srp-check-scope-fits <released-pairs.tsv> <candidate game data>` asks the candidate's
`Modules::check` for every pair (`tools/checks/scope_preservation.sh`), so the check needs no game
data of release 68 and outlives the variants.

- **In 0.69d0's own data: 1,159 pairs** (computed from the pack's record of 2026-11-20).
- **The other 46 are legacy rows** of the table, made by the porter from the same list
  (`port-modules --released`, resource-porter `donor_069d/modules/legacy.rs`):
  1. *as its base gun*: a premium item with links of its own but no optic among them takes the optic
     as its base gun does, where 0.69d0's rules place it on the item's own parts (the SR-3M
     premium's eighteen);
  1b. *on a slot of the gun's own*: an optic that names a slot the gun has free takes it, by
     0.69d0's rules for that slot (the PSO-1 on the Saiga-12's and its premium's dovetail, which
     0.69d0 links only to collimators through a rail);
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
- **`wpn_mp7_silencer_special`** (three released pairs) is a gun of the table since the pack's
  version 2 (its row in `scripts/remodel-069/items.tsv`): the MP7 with a silencer, whose model it has.
- **43 pairs are new** (the table has them and no release did; pack version 3): eleven on the MP7
  with a silencer and eleven on the event MP7, eight on the two Neosteads, thirteen on the VSSK's
  camouflaged copy (it takes the VSSK's fits; two of its fifteen were released). Each has its
  `items_compatibility` row in `scope-modules-1.json`; `modules_edit.py --check` (`tools/check
  scope-modules`) refuses a list that is not the listed pack's record's: the pairs change with a
  pack version (the list of 19 made before version 2 left 24 pairs without a row, which
  `scope-preservation` refused).

## The account's load

A released record is `{scope item id, variant dict id, 0, 3}`: it names the owned scope item, so
the move needs no table. At an account's load every slot-3 record whose item is a scope item
(category 22) gets that item's own dict id; then the usual repair runs (`Modules::check`), and a
record whose pair the table does not have is taken off and logged with its reason. The player keeps
the scope item either way. A kept results screen loses a record that still names a variant (it is a
picture of a past match).

What that does to the owners of each released pair. Run, 2026-11-21: a database made by release
68's own server code (its session crate at tag `026g-rev68` on release 68's game data: 102 accounts
registered through the sign-in, each pair's weapon and scope bought through the lobby's buy or
added as the admin's give does where the shop refuses the account, the scope attached by release
68's `attach_upgrade`, each record checked to hold the variant `released-pairs.tsv` names; 9,448
items, 102 weapons worn in a kit), then loaded by this branch's `srp` on the full content's game
data, saved, and compared row by row; and the same on the game data of the content with the
variants retired:

| Released pairs (the record of pack 3) | At the load, both runs |
|---|---|
| 906 that are 0.69d0's own fits of the gun | record rewritten to the scope's own id, kept on its weapon |
| 273 on the premium and camouflaged copies that take their base gun's fits (22 guns; the premium SR-3M's 18 and `vssk_camo`'s 2 among them) | rewritten, kept |
| 2 on the event MP7, as the MP7 with a silencer; the Mosin's PU 3.5x | rewritten, kept |
| 2 on a slot of the gun's own (PSO-1 on `saiga_12` and `saiga_12_premium`) | rewritten, kept |
| 21 where their variant put them (19 Docter pairs, the XPS2 on the Kedr and the AEK-919K) | rewritten, kept |

1,205 records in 102 accounts rewritten, none taken off, no warning in the server's log; no other
item, kit slot or column changed. Not in that database: a kept results screen that names a
variant, clans, whatever only real accounts have (the release's dry run loads the live database).

## The variants retired

Prepared on `feat/attachments-scopes-retire` (`scripts/scope-fits/retire.py`): the three packs that
were the variants, and their edit lists. A patch only adds files, so a pack is retired by a version
that makes less (`release/content.tsv`'s header); nothing is taken from a player's disk.

- **Out of the dictionary: 1,036 variants** (`retired-variants.tsv`, with their rows of
  `content-edit/items.tsv`): the 184 of `rifle-scopes`, the 212 of `scope-fits`, the 640 of
  `scope-coverage`. Their dict ids are never used again.
- **The packs' last versions** (`rifle-scopes@19`, `scope-fits@14`, `scope-coverage@3`) are the
  earlier ones less those entries and less every model (30,464 files of 7,639 models: the
  per-pair optics and the gun bodies with a moved eye). They keep the eight shop scope items 2113-2120 (configs,
  dictionary entries, texts, icons) and the textures and materials. 732 variant configs are left in
  players' archives as they are; **304 stay in the packs**, with `scope-centering-1.json`, which
  changes them after their pack: `release/build-content` takes a file as left by an earlier pack
  version only when players have that version's bytes, and it cannot take an edit off a file (a
  file no row makes is never packed again: the first build without the list stopped on the 151
  variant configs of the base patches that the list changes). They are in no dictionary and
  nothing names them.
- **The edit lists**: `scope-fits-1.json` and `scope-coverage-1.json` are gone.
  `scope-centering-1.json` stays as it is (455 retired variants' configs, as players have them)
  and `rifle-scopes-1.json` keeps the Aimpoint CompM4's x2, on its shop item and on 25 retired
  variants, for the same reason: an edit of a file is never taken back. The lobby's
  `items_compatibility` rows they made (1,036) are `scope-modules-1.json`'s, with the 43 new
  pairs': every pair of the table that no base patch's dictionary has. The `addons { mount: 1 }`
  placeholders of 32 guns are gone: a variant's type-3 modifier needed a table with a child; a
  scope item's adds to `data`, and patch 161's two sites stand before the game's own look at
  `addons` (`attachments.scope_dict`, `attachments.scope_result`).
- **Not retired: the base patches' own variants**, 166 (99 `fit_`, 67 `fitn_`) and five `loc_`
  entries. A base row is a published patch taken as it is; taking an entry out of the dictionary
  needs an edit operation `content_edit.py` does not have (an entry of an indexed table). They are
  entries nothing uses: not sold (no gun takes them as a module), never dropped (category 22), no
  record names them after an account's first load.
- **The server** needs nothing more: a variant's dict id is only in the scopes' records of accounts
  saved up to release 68, which the load rewrites before anything reads them. An account that
  *owns* a retired variant as an item (no path gives one but an admin's give by dict id) stops
  `srp`'s start, which names it (`known::Lacking`) and changes nothing: the release's dry run on
  the live database shows it before players do.
- The variants' generators (`scripts/rifle-scopes`, `scripts/scope-coverage`, `scope-fits`'s
  `make_pack.py`) are left as they are: other packs' scripts read from them (`weapon-grips`,
  `weapon-modules/ids.py`). Their three asset checks are gone with the packs they checked.
- **Built** (`release/build-content 69`, a quick build of this tree with the pack
  `weapon-modules@3`, 2026-11-21): it passes. Left from the retired versions, live and no longer
  made: 21,444 files of `scope-coverage@2`, 5,555 of `scope-fits@13`, 4,197 of `rifle-scopes@18`
  (30,464 model files and 732 variant configs). The dictionary has 919 entries where it had
  1,955: exactly the 1,036 of `retired-variants.tsv` are gone. The lobby's rows of the scopes'
  pairs are 1,248 (they were 1,224 with the 24 missing). `srp-check-scope-fits` on its game data:
  1,205 released pairs preserved, 43 added, none refused. The load of the release-68 database on
  it: as above.
- **Seen in a game on it** (headless, exe and server 98843a3ef; `private/renders/
  attachments-scopes/retire-2-window-aimed.png`): the L85A2 with the ACOG TA01 (a released pair on
  a gun that lost its `addons` placeholder), the event MP7 with the Cobra (a new pair), the AK-74M
  with the Aimpoint ("CompM4 collimator, x2"), the APS with the DeltaPoint: each in the weapon's
  Attachments window (the scope in its slot and on the gun) and aimed, the reticle within 0.2 px of
  the screen's centre; no record taken off, no complaint in the patch log. The L85A2's window on
  the full content before the retirement is the same picture (mean difference 1.1 of 255).

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
(uro_fov 65) and 0.39 mrad at x4. The point taken is a collimator's dot (its box's middle) and a
magnified scope's first-person lens (`scope_lense_1p_s`, whose middle is the optical axis; the
reticle's own box is not on the axis: the PSO-1's chevron and range scale gave 71 mrad).

Computed, 2026-11-21, the full record of the lane's porter (all 102 guns, resource-porter 1e5587e,
`private/local-clients-attachments-scopes/modules-069d-full-3.json`, `check_record.py`): 1,221 scope
fits on 101 guns, every one within 0.0003 mrad of the eye's line at rest and through the aimed idle
clip, but the Elcan Specter DR (60 guns): 1.89 mrad, 3.6 px at x4, the same on every gun, so a
property of its model (its lens off the line of its `aim`), not of a mount. Seen in a game: see
"State".

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


## State, 2026-11-21 (06:00)

| Part | Where | State |
|---|---|---|
| The released pairs' record | `scripts/scope-fits/released.py`, `released-pairs.tsv` | made from release 68's files; the same 1,205 pairs and variants as the server's parser gave |
| The scope items' edit list | `scripts/scope-fits/modules_edit.py`, `scripts/content-edit/scope-modules-1.json` | made from the full record of the lane's porter (1e5587e); it names that porter's models, so it goes with the first pack built with it; row at the end of `release/content.tsv` |
| A build's scopes checked from its record | `scripts/scope-fits/check_record.py` | run on the full record: every released pair a fit but `wpn_mp7_silencer_special`'s three; no row in `tools/checks.tsv` yet (the record must be kept beside the pack first) |
| The collimators' reticle, the legacy rows, the centre numbers | resource-porter `feat/attachments-069d-scopes` | run over all 102 guns: 6,999 checks against 0.69d0, none over 0.5 mm; 42 legacy rows, none lost |
| The servers on the modules table, the load's rewrite, the preservation checker | `crates/survarium/src/v026g/scopes.rs`, `modifications.rs`, `crates/session/src/v026g/items_rules.rs`, `bin/srp-check-scope-fits.rs` (reads `released-pairs.tsv`) | crate tests pass but three that fail on the branch's base too (two of them on main: the badges' price, the comments' migration); clippy clean; run in the local games below |
| `plane_slot` | above; `tools/re-kit/v069d/functions.tsv` | read and run in Unicorn; table version 3 is the core's |

Seen in local games (exe e17ace688, table version 2, quick packs of the lane's porter; pictures in
`private/renders/attachments-scopes/`):

- **The AK-74M with its eighteen scopes**: each drawn in the lobby and aimed; at rest every dot,
  ring and crosshair within 0.5 px of the screen's centre (the Elcan within 1.1 px), the chevrons'
  tips on it. With version 2 a collimator is aimed from inside its housing (its eye is its own
  `aim` point): the housing is not seen, the EOTechs' hood floor fills the lower third. Version 3
  (`plane_slot`) is the fix.
- **A gun with a default optic** (the cook's `data/srp_scope` site): the SVD shows its own PSO-1 with
  nothing fitted, 0.69d0's PSO-1 or the Aimpoint (no PSO-1) with one fitted; the HK416 the Aimpoint
  in place of its EOTech, and its EOTech again when the load took off a record of a pair the table
  does not have.
- **`aim_offset` = 0 takes**: the PSO-1's eyepiece is the same size on the SVD (aim_offset -0.11) as
  on the AK-74M (none), 929 against 926 px wide; the SVD's own PSO-1 at its -0.11 is 558 px.
- **Legacy rows**: Kiparis and TOZ-122 with the Docter, Saiga-12 with the PSO-1, Kedr with the XPS2,
  SR-3M premium with the PSO-1, and the Mosin's PU 3.5x: kept at the load, reticles centred.

Seen on table version 3 (server and exe of feat/attachments c8b28fb43; pictures in
`private/renders/attachments-scopes/ak74m-v3/`, `c23/`, `full-v2/`):

- **The eighteen again**: every collimator's housing is seen from behind at the gun's own eye plane
  (`plane_slot`), the EOTechs' hood floor gone; every reticle within 0.5 px of the centre.
- **The dot at 0.69d0's size**: the Cobra's dot as small as before (about 2 px, 0.69d0's 2.4 mrad), the
  EOTech's ring 1.8 times larger than before (0.69d0's).
- **Every class of default optic** (the lead's full build): with nothing fitted the gun's own (Kriss and
  P90 the Walther, Rem 700 its Leupold, MSBS its EOTech, VSS its PSO-1, SV-98 its 1P69); a fitted scope
  in its place, nothing of the default left. With only a muzzle device or only a grip (the HK416, 2613 or
  2660) the default EOTech stays; the part itself was not in these pictures.
- **`wpn_mp7_silencer_special`** with the EOTech 552: a fit now, drawn, centred.
- **The scoped shot with #2294** (its patch, 172 since it was merged, and its 0.45a6 fire clips, against
  patch 161 alone; 60 frames a second, `rig/out/kick165-c23`, `no165-c23`): the SVU-A (aim on the
  kicked bone) shows the kick in the scope only with the patch (the eyepiece moves 18-20 px, 3-5
  without); the L85A2 likewise (the ACOG's eyepiece is larger than the screen, so its reticle's dot
  was tracked: it lifts 22-25 px for 4-8 frames and is back within 0.3 px; without the patch it
  stays within 0.5 px); the SVD (aim on the root) shows it either way (30-34 px), as the crate's
  table says.

Seen on the mount kinds beyond the AK-74M (2026-11-21; exe and server 98843a3ef, the lead's full
content `content-full-69-v3`; a session a pair, the scope's record seeded as the lobby's attach
writes it; in the lobby and aimed, never from the hip; a sheet a gun,
`private/renders/attachments-scopes/mounts-v3-<gun>.png`, its numbers beside it in `.tsv`). The
reticle's place is measured on the aimed picture: a dot's or a ring's centre, a chevron's or a
post's tip.

| Gun | Mount | Scopes seen | The reticle against the screen's centre |
|---|---|---|---|
| Mosin | the PU's own mount | the PU 3.5x | the post's tip 1 px low |
| Saiga-12 | the dovetail (the PSO-1, a legacy row on it), collimators on the dovetail-to-rail bracket | all 10 | the PSO-1's chevron tip on it; dots within 0.2 px; the two EOTech rings centred (their box 0.0 px, the centre dot within 0.15 px) |
| Kiparis | its own short rail carrier (the Docter a legacy row) | all 7 | within 0.05 px |
| APS | the pistols' rail | all 4 | within 0.05 px |
| Remington 870 | the medium rail carrier of the thirteen shotguns and PPShs (the Docter a legacy row) | all 10 aimed (two of its lobby pictures were taken before the character was drawn and are taken again) | dots within 0.1 px, rings within 0.3 px |
| HK416 (picatinny on a rail gun; its default EOTech replaced), SVD (the dovetail and the bracket; its default PSO-1 replaced), UMP45 (an SMG's own rail) | | 15, 18 and 13 | not captured yet when this was written (the run was waiting for the machine); `mounts-v3-summary.txt` beside the sheets has their numbers once it ends |

No session's load took a record off; no patch log has a complaint.


