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
- **The reticle's material** (pack version 8): the players' emitter (`weapons/loc2/collimator_glowing_dot_red`,
  patch 21's: red, 20 times, `constant_transparency` 0.3) without its fade by the view's angle, as
  `weapons/modules_069/collimator_glowing_dot_red` (the one setting that differs). The fade
  (`use_angle_depended_attenuation`, `attenuation_power` 7000; `forward_base.ps` multiplies the alpha
  by |cos|^7000 of the angle between the pixel's ray and the view's axis) keeps a pixel lit only within
  about a degree of the screen's centre (half at 0.8 degrees). It was made for the 0.51c5 optics' dot,
  0.6 m ahead of the optic, which the gun's moves hardly move; at 0.69d0's place, 0.08 to 0.8 m ahead
  of the eye (the AK-74M's EOTech 0.13 m), a few millimetres of the gun's move against the eye take the
  reticle out of that degree, and the EOTech's ring (2.8 degrees across there) was lit at an eighth
  even at rest. Versions 3 to 7 had it (live since revision 69): the owner saw the dot flicker and go
  out (2026-11-22, again 2026-11-26). 0.69d0's reticle material and 0.26g's own EOTech 552 and
  Walther emitters (`eotech_552_emissive`, `walther_comp2_emissive`, at the same place in their
  housings) have no fade; loc2's material, which the 0.26 and 0.30-era sights use, is unchanged.
- **The reticle and the hi-Z occlusion**: the dot is a surface of its own, a disk 0.66 mm across, and
  until patch 213 it took the world's hi-Z test like any surface in view, the housing around it a
  separate answer (`renderer-026g.md` 5.7): a wrong answer dropped the dot alone for a frame or more.
  Patch 213 keeps every first-person surface out of that test.
- **The glass.** An idle optic's outside lens (`scope_lense_3p_s`: the front and the rear glass in
  one surface, two discs at the tube's ends) is dark coated glass one sees through (pack version
  9: version 8 with it; versions 4 to 6 were its steps, never released):
  `weapons/modules_069/scope_lense_3p_glass`, the stock PSO-1's lens material
  (`weapons/pso_1_lens`, `forward_lighting_no_distort`) on patch 19's lens normal and gloss maps,
  and a texture of its own made from patch 19's lens map (alpha 40/255 and colour (90, 110, 120)
  in the glass, the map's dark opaque rim kept). How 0.26g draws it (the shipped bytecode, and
  captures with one stage switched off): the forward stage (`forward_base.ps`) blends black by the
  transparency map's alpha times `constant_transparency`; the lit stage (`forward_lighting.ps`)
  blends its lit colour (diffuse times the diffuse map's alpha, plus the light's specular) by its
  own alpha, which is `constant_transparency` alone unless the stage reads the transparency map
  too (`use_ttransparency`). The PSO-1's lit stage does not, and its constant is 1: version 5's
  glass drew its lit colour over everything behind it, a flat light-grey disc at the hip in sun and
  in shade (the owner, 2026-11-23: "the scope is opaque"; the capture rig: grey 37 in shade, and
  black once that constant is 0). The stock scopes' own glass has the same lit stage (opaque, lit,
  its map's coating colour, by the same rule; not captured). Version 6 (and 9): the lit stage reads the map (stock `weapons/pasted_glass`'s
  switches), both stages' constant 2.2 (88/255 in the glass), the lit diffuse times (0.25, 0.22,
  0.45) (a dark blue-violet coating, about (22, 24, 54)), the specular times 0.5 (the PSO-1's 0.1):
  at the hip the tube's dark inside shows through, with the light's highlight on the glass.
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


## Why the EOTech looks small on the A-545 (2026-11-25)

Two differences add up. The first is the distance from the camera to the optic.
The 0.69d0 A-545 and the shipped remodel both put the long rail at local z = −0.25013 m
on the offset bone. Both EOTechs use that rail directly. The porter's version 3 gun record
takes its `body_aim` from the **live** model's `aim` locator. Its z on the root bone is
+0.3140 m. Retail takes it from its own model's `aim` at +0.1771 m. The plane intersection
therefore puts our eye 13.7 cm farther behind either sight. The porter's `eye line` check
projects the reference onto the closest point to our eye on that line. Moving the eye along
the line can pass both this check and the reticle centring check.

| A-545, camera to reticle | Retail 0.69d0 | Shipped 0.26g remodel |
|---|---:|---:|
| EOTech 552 | 39.46 cm | 53.16 cm |
| EOTech XPS2 | 42.85 cm | 56.55 cm |

Measured in bind pose from each model's `render/export_properties` and `render/bind_pose`.
Use the optic's `aim` on the rail as the reticle position and intersect its sight line with
the corresponding gun's eye plane. On the shipped side these distances also agree with
`weapon-modules@3`'s aimed-idle `centres[].ahead` within 0.1 mm. Of 33 gun records with
a directly mounted 552 and both slots on the gun, the A-545 alone puts the bind-pose eye
more than 5 cm farther back than retail. Seventeen put it more than 5 cm closer, including
the M4A1 (20.78 cm in retail, 7.99 cm here). A single per-optic offset cannot restore both.

The second difference is the first-person projection. The 0.69d0 first-person camera controller
builds two perspective matrices. The world follows the view setting and the current sight zoom
(both EOTechs have `aim_zoom_factor = 1.2`). The weapon uses a fixed 65° vertical field of view
multiplied by the interpolation from `weapon_fov_factor` to `aimed_weapon_fov_factor` over the aim.
The A-545 goes from 1 to 0.5, giving **32.5°** at full aim regardless of the world FOV setting.
The same aimed factor occurs on 84 of the 103 gun item rows in `scripts/remodel-069/items.tsv`.
Examples of other factors are 0.35 on the SKS, 0.4 on the Remington 870 and 0.65 on the Jackhammer.

In revision 73 both shipped shop EOTechs have `data/fov_factor = 0.8` (their files in
`resources.db.patch_69`). At the default `uro_fov = 65`, the aimed world FOV is **52°**.
The 0.26g foreground pass (0x13E010, identical in the stock and revision 73 executables)
copies that projection and changes its depth terms. The weapon and reticle therefore also use
52°. At equal viewport height and reticle size, projected width varies as
`1 / (distance × tan(vertical FOV / 2))`. The two measured differences predict a retail
ring about **2.25× as wide for the 552** and **2.21× for the XPS2** on the A-545. These are
geometry estimates at default settings, without a matched retail gameplay capture.

The shipped v3 table already has both locators. Its gun header holds the composed body's
own `aim` (what the game reads as the eye with no part moving it) and its ordinary slots hold
the donor's resolved `body_aim` on the shipped rig. Patch 161 takes that slot as the plane of
every `InGunPlane` eye (`Gun::eye_plane`); the header is never a plane. A table with such a fit
or stock part and no slot is refused (`GunFault::NoEyePlane`), and a sight whose line never
meets the plane is a refusal (`Refusal::OffPlane`, where 0.69d0 would leave the eye inside the
sight); the check `weapon-modules-table` holds the shipped table to both, and to every scope fit
standing. The corrected aimed-idle distances for the A-545 are 39.459 cm and 42.852 cm, and for
the M4A1 20.781 cm and 24.185 cm (552, then XPS2).

**Which fits move** (both resolvers over `weapon-modules@3`, 2026-11-26). 1,248 scope fits on 102
guns: 413 take the optic's own eye (`Own`: every magnified scope, the PSO-1, ACOG, Elcan, Leupold
and 1P69 among them) and are unchanged. 835 keep the gun's eye plane (collimators and the rail
iron sight): 725 move, 110 do not. Of the 110, none has a missing slot and none meets the
plane-less case; the header lies in the plane already: AKMSU, FAMAS, G3SG1, KS-23M and PPSh
drum (header equal to the slot), Beretta 93R, CZ 75, MP-443, SVU-A (within 0.01 mm), Glock 17,
Rem 700 and its premium, SV-98, VSSK and its camo (the header off the slot by 0.4 to 4.3 cm, but
in its plane). The moving ones, along the line, negative toward the shooter: the pistols with a
collimator 43 to 49 cm (Fort-17, APS, PM, TT-33, Magnum), Kriss 26.6, Uzi 23.0, PPSh 21.4, HK416
17.7, SKS 16.7, Winchester 15.0, VHS-2 13.6, OTs-14 13.4, M4A1 12.8, AEK-919K 11.9, the
shotguns 9 to 11.5, most rifles and SMGs 2.5 to 10, and forward the VSS (+18.1), A-545 (+13.7),
MP5 and Val (+6.7, +6.6), VSK-94 (+3.9), Rem 870 (+3.6), Vepr (+1.5), MP-153 (+1.1), SVD (+0.7).

**Iron sights.** No stock part of the shipped table sets the eye, so with no optic (nothing
fitted, or only a muzzle device or a grip) the eye is the body's own `aim`, moved along its line
by the gun's `parameters/aim_offset` (`calculate_aim_alignment` 0x244D40; the cook makes the
matrix at 0x24A8E7). Every 0.69d0 `ironsight` module sets the eye with `plane_slot: body_aim`, so
0.69d0's iron sights stand at the eye plane. `weapon_modules::iron_sights` gives each gun how far
its own eye goes along its line to that plane (`scripts/weapon-modules/iron_sights.tsv`), and the
edit list `iron-sights-1.json` makes it the gun's `aim_offset` (`iron_sights.py`) on the 89 guns
without an optic of their own; a fitted optic's modification still sets it to 0. Against the
revision 73 eye: the pistols 38 to 43 cm farther back, Uzi 23, PPSh 22, RPD 20, PKP 16, Winchester
15, SKS 15, VHS-2 14, OTs-14 13, M4A1 13, Mosin 12, most others 1 to 12; forward MP5 6.7, Val 3.8;
the A-545 (its `aim_offset` +15 cm already) 1.3; twelve within 0.5 cm. The fourteen guns with an
optic of their own (SVD, VSS, VSK-94, Rem 700, SV-98, VSSK and their premiums, HK416, MSBS, Kriss
Vector, P90) keep theirs: it is that optic's eye relief, and their iron sights are never seen.

**The projection.** 0.69d0's first-person camera (`update_camera` 0x2002C2D70) makes two
projections every frame: the world's, the view setting times the sight's zoom
(`current_camera_fov_factor`), and the first person's, 65 degrees times the held gun's factor
interpolated from `weapon_fov_factor` to `aimed_weapon_fov_factor` by the aim, 1 for anything that
is no gun (the 1.0 at 0x2002C2DC6). `send_camera` 0x2001F7A40 hands both to the render view
(camera +80h and +C0h). The gun, its modules and its optic go to the foreground together
(`weapon::set_foreground` 0x2002B7630 through each module's visual), and a rifle scope's visual
puts its idle and aimed models and its current reticle there alike (`rifle_scope_visual` vtable
0x200984240, slot 6): **a magnified scope's lens and reticle are drawn with the first person's
projection**, not the world's. Only the item's two factors feed it (their one reader is the
weapon cook, 0x2002025C5; the G33 magnifier's `effects_to_override` of `weapon_fov_factor` is the
one module that would change them, and it is not shipped). The geometry agrees: at 32.5 degrees
the PSO-1's first-person lens (radius 13.1 mm, 67.7 mm from its eye) spans 0.66 of the half
height, the 1P69's 0.64, the ACOG's 0.61, inside the 0.7 of the PSO-1's `scope_radius` (its
exposure mask); at the world's x4 (16.25 degrees) the PSO-1's would span 1.35, past the screen
(929 px of 720, as seen at revision 73). The world's zoom is the same in both (the shop scopes'
`fov_factor` 0.25 is 1 / `aim_zoom_factor` 4), so the reticle against the world is 0.69d0's. The
patch draws every held thing so: a gun at its profile (none: 65 degrees), anything else or nothing
in hand at 65 degrees (`weapon_view.no_gun` 0x1F3579, the camera's branch with no weapon), so a
knife, a grenade or a medkit is drawn at 0.69d0's 65 whatever `uro_fov` is, as 0.69d0 draws them.
**One exception**: a gun drawing an optic of its own (0.26g's PSO-1, Leupold and Giperon items 304,
69, 362, the EOTech and Walther 2206, 2207) keeps the world's projection, as at revision 73:
0.26g made those models and their eye relief for it, and 0.69d0 has none of them. The optic is
the table's exactly when the weapon's config names one by `data/srp_scope` and its parts are held
(`attachments::scope_fitted`).

The shared `srp_weapon_views` dictionary holds both factors for every gun id
(`weapon-view-069.json`, made by `scripts/weapon-modules/view_profiles.py` from
`view_factors_069.tsv`, which `--retail <game.options leaves>` takes from 0.69d0's items). This
keeps the weapon config entry count within its existing bound, including the premium Remington 700
already at 565 entries. The client copies the profiles when the items dictionary loads; a row that
is no `{gun, fov}`, has a factor outside (0, 1] or repeats a gun is left out with a line, the
others kept. In first person the camera samples the weapon's aim progress. The matching scene draw
command carries its own lens to the render thread in base_command's padding at +14h, which the
constructor (0x20310) leaves: every draw_scene command is made at 0x1F8EE, its one caller, and the
submit site writes the field (0 for no lens) before the push publishes it (0x1F8FF); the render
channel links the command itself (0x1F2C0), never a copy, and on_defer_execution (0x28620) is
handed the command (this one is made with an empty defer). Each foreground pass replaces the two
scale terms of its private projection matrix, retaining aspect, jitter and depth. Other scene
views use their original projection. The attachments Unicorn check runs queued frames through the
patched executable: aimed, half aimed, another view, a gun's own optic, a fitted optic, no profile,
no gun, the bad rows, and a command made by the game's own constructor over garbage, deferred and
run through its own slots.

The player can scale these angles, draw everything held at the world's projection again, and move
an iron sight's or collimator's eye along its line (patch 219, `weapon-view-settings.md`); the
defaults are what this section says.

Measured in local matches with the EOTech 552, at 1280×720 and `uro_fov 65`.
Ring diameter is measured across its red band, excluding the centre dot and the four
cardinal marks. The retail column is the geometric prediction from the 0.69d0 assets
and camera code, relative to revision 73.

| Gun | Revision 73 | Corrected client | Measured change | Retail prediction |
|---|---:|---:|---:|---:|
| A-545 | 8.6 px | 19.7 px | 2.28× | 2.25× |
| M4A1 | 56.9 px | 37.1 px | 0.65× | 0.64× |

The A-545 with the XPS2 was also aimed in a local match on the final executable.
Its ring measured 18.0 px across at the same resolution.

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


