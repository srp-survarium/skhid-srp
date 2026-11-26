# A scope's ways onto its gun, chosen in the Attachments window

The owner, 2026-11-26: "we added second scopes with select button, right? let's also add select
button for scopes which allow for different locations for the scope from 0.69". Client 0.26g; 0.69d0
the reference. Tags: PROVEN (read in the data or the code named), [srp] our choice, OPEN.

## What 0.69d0 has (PROVEN, its `gameplay/game.options`)

No sliding along a rail: a place is a slot. An optic's module names the slots it goes in
(`slots_to_attach_to`); a gun has some of them, and its linked rails, covers and forends bring
others (`slots_to_add`). Where an optic names two slots a gun offers, it may stand in either; in
0.69d0 the player picks by which rail module he installs.

The modules table already has every slot an optic names (its attaches) and, by the porter's rule,
the carriers of the one slot each fit took (`docs/notes/attachments.md`, rule 2: the first slot the
gun has, else the first carriers bring). The porter now also gives every other slot an optic names
that the gun has or carriers 0.69d0 links bring (`others` of a fit in the record, the carriers in
the table); 966 checks of those ways against 0.69d0 itself, none over 0.5 mm (resource-porter
feat/scope-mounts, weapon-modules@11's record).

## The ways (weapon-modules@11)

`weapon_modules::mounts(gun, scope)`: the first way rule 2's (what every scope had before), then
each other slot the optic names, in its order, where the scope alone stands. 21 weapon items (14
guns and their premium copies), 234 (weapon, optic) pairs, two ways each; every other pair has one
way and no buttons.

| guns | first way (today's) | the other | optics |
|---|---|---|---|
| AK-74M, AK-74N (+premium), AKM (+2 premiums) | receiver: dovetail rail adapter | handguard: the Midwest tactical forend's top rail | 13 |
| RPK | receiver: dovetail rail adapter | handguard: the RedHeat forend's rail | 13 |
| Vepr (SOC) | receiver: dovetail rail adapter | handguard: the Krebs forend's rail | 13 |
| AKS-74U (+premium) | handguard: the RedHeat custom forend | gas tube: the Zenit B-18 rail | 12 |
| AKMSU | handguard: Zenit B-11 forend and B-19 handguard | gas tube: the Zenit B-18 rail | 11 |
| Vityaz (+premium) | receiver: its own top rail | handguard: the RedHeat forend's rail | 13 |
| SOCOM 16 | handguard: its own rail | receiver: the SOCOM 16 scope rail on its dovetail | 13 |
| vz.58 | handguard: its own rail | receiver: the tactical receiver cover | 13 |
| SVU-A | handguard: its own rail | receiver: dovetail rail adapter | 13 |
| VSS, Val, VSK-94 (+premiums) | barrel: the barrel bracket (five collimators); receiver: dovetail adapter (the aperture sight) | the other of the two | 6 |

A way's carriers come with it and take the gun's own part's place as any fit's do (the tactical
forend for the handguard rail, the vz.58's railed cover for its own); a grip that needs the same
forend shares it.

## The choice [srp]

- **Kept** with the scope on that weapon, as its upgrade record's value: a `Mount`, 0 the first way.
  On the wire it rides in the weapon kit's bits 52..53 (both layouts; the scope is always the kit's
  own when held), 0 for any other own (`crates/weapon-kit`, `weapon-kit.md`). A kit of before is
  the first way: nothing changes for anyone who does not choose.
- **Changed** in the Attachments window: under the scope's label, a button a way (ScopeMount,
  ScopeMounts; SideButton's look, as the second sight's Left/Right), named by the place
  (Receiver, Handguard, Barrel, Gas tube: `st_srp_mount_*`, the twelve dictionaries). A press sends
  `srp_set_upgrade_value(host, scope, way)`: patch 207 makes it `46 05` for the host's scope as for
  its second sight (client_patch.md 3.111), the client's copy of the kit packed again.
- **The server** (session `items_rules`): `46 05` on a weapon's scope takes a way (0..3) the gun has
  for it with what it holds (`Modules::check` with the way), else it is refused and the account sent
  as it is. Another scope put on starts on its first way, the same one kept on keeps its own; a
  value moving between kinds (a second sight's side to a scope) is not carried. At an account's load
  a way the table does not offer (another pack version's) is the first again, before anything comes
  off.
- **Free**: nothing to buy.

## Drawing it (patch 161)

The cook's scope record carries the way as its value (`weapon_kit::expand`); each scope item's
modification adds `data/srp_scope_mount` = `<low_param>` (scope-modules-1.json,
`scope-fits/modules_edit.py`). Patch 161 reads it beside `data/srp_scope` (`vostok/attachments.rs`
`named`; a config without the key is the first way) into `Wanted`, and `resolve` puts the optic,
its carriers and its eye at the chosen slot: the eye is the module's own on that slot (an `Own`
eye for a magnified scope, its line at the gun's eye plane for a collimator, `InGunPlane`,
attachments-scopes.md), so it moves with the optic.

## Simulation and the world: neither

The way changes which models are drawn and where the first-person eye is. Nothing a match computes
reads them: the shot leaves from the camera, the scope item's modification changes no stat (the
`data` node is read by patches 161 and 206 alone, and patch 206 reads only the second sight), and
the scope view byte (+181Eh) keeps its rule. No world byte is written for it and none read: the
kit is static info, as every client's. So no `simulation_changes.tsv` row and no change of the
world parser. The carriers of the ways set no fire point and no shot effect.

## Content

- `weapon-modules@11` over @9 (`pack_over.py --made`: the table and the parts a way brings; the
  vz.58's own receiver cover and the Vityaz's own forends become parts, so their bare bodies'
  settings list one surface less). @10 is PR #2314's (its masks); whichever lands second is made
  again over the other the same way.
- `ScopeMounts.tsv` (the lobby's ways and their names) from the record by
  `scripts/weapon-modules/scope_mounts.py` (its check `scope-mounts-table`).
- scope-modules-1.json retuned: the twenty scope items' modifications add `srp_scope_mount`.

## Not played

Built, not run in a game: the buttons in the Attachments window, the press reaching the server and
back, each way drawn and aimed (the reticle centred), and the other clients' view of the gun.
