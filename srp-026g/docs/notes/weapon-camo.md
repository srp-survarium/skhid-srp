# Camouflage on weapons

The owner's request (2026-10-20): the armour camouflages can be put on the regular weapons, painted
where the premium (camouflaged) weapons have their paint; the tooling to do that in game.

## Rules

- **Which weapons.** Every weapon (all 83 weapon configs, pistols and 0.29a3 ports included):
  91 textures, 111 `_aoac` files with the premium copies (`scripts/weapon-camo/guns.tsv`,
  `scripts/content-edit/weapon-camo-1.json`). Not their barrels, sights, suppressors and
  attachments when those have textures of their own, nor the 12-gauge shell, the SV-98's net or
  the premium-only `_camo` textures.
- **Where the paint goes.** On some parts of a weapon, never the whole: the parts (UV islands)
  `scripts/weapon-camo/parts.tsv` names, chosen by hand in the picker page (all 91 textures,
  2026-10-21: 22 by the owner, the rest after them): the body (receiver, covers, handguard,
  stock, grip, magazine); not barrels, muzzles, sights, rails, bolts, triggers, pins, nor a
  magazine inside a pistol's grip. A weapon without a line there gets the suggestion: the large parts its premium painted (each at least 2 % of the
  texture's parts: receiver, covers, handguard, stock, magazine); with no premium (65 textures),
  the large parts, which can take in a barrel (the TOZ-66's): pick those before a release. Small parts (triggers, bolts,
  pins, sights, screws) stay bare. Most premiums were repainted as a whole, so "where the premium
  painted" alone would cover nearly the whole weapon (97 % of the AK-12's parts).
- **Which camouflage.** Any camouflage item (category 25) that goes on armour; one at a time.
- **Slot.** [srp] A weapon's camouflage is in upgrade slot 2: slot 3 holds its scope, 0 its decal
  (`WEAPON_CAMO_SLOT`, `modifications::camo_slot`). A scope and a camouflage are worn together.
  The client keeps a record in any slot.
- **Three records, six upgrades.** The client keeps at most three upgrade records on an item (an
  inline array of the item record, `limits::UPGRADES`; a fourth is a fatal assert). Since the
  weapon kit (client patch 68, [weapon-kit.md](weapon-kit.md)) a weapon's upgrades are logical
  slots on the server (decals 0 and 1, camouflage 2, scope 3, muzzle 4, grip 5) sent as the decals
  and one kit record: a scoped weapon's camouflage travels named in the kit's value.
- **Shared cosmetics.** As for armour (PR 149): one camouflage item on any number of items,
  weapons and armour mixed, each with its own record; sold, it comes off all.
- **Tiling.** `data/camo_scale` of the weapon config (4.0 for all, the stock bakes' value): the
  camouflage repeats that many times across the weapon's texture. Per weapon, tunable in data.
- **What a match sees.** Every game builds a weapon from its item's upgrades (0x2F14C0); the
  camouflage is baked on each game's own GPU, as armour's. No simulation change: games with and
  without patch 44 play together (without it the weapon is plain).
- **Where it shows.** Wherever a weapon model is built (weapon_cook): the lobby's character, the
  match (first and third person, one model), replays. The weapon shows plain for a moment and
  takes the camouflage when its bake lands (the lobby's decal editor does the same on armour).
- **Cost.** A bake per weapon surface the first time a camouflage is seen on that weapon (cached in
  `Documents/Survarium/streamed_textures/srpw1_*`, the 1 GiB baked-texture cache armour uses):
  three 2048² textures (diffuse, normal, smoothness), about 11 MB on disk and in video memory.

## The weapon's upgrade window (patches 32 and 45)

The owner (2026-10-21): "the camera would flow to the working table and the 3d model of the weapon
would have been there", "that would allow you to rotate it", "with weapon in the air".

- **What is shown.** The window's weapon with its scope and camouflage, as its item has them: the
  kit that holds it, or the chosen kit holding it from a scratch profile (patch 32, never written
  to the account). Armour windows are the stock ones.
- **Where.** In the air above the workbench by the lobby's right wall (0.4 m over its top, between
  the drill press and the vice); the camera flies there from the character in about a second and
  back at the close, to the turn it had.
- **Turning.** A drag turns the weapon (sideways: about the vertical; up and down: tilted, within
  about 70 degrees), not the camera; the wheel brings the camera nearer or further (0.6 to 4.6 m).
  Each window starts with the weapon level and side on. It turns about its middle along its length
  (and across), which sits at the bench point: the middle of the gun itself, not of parts the model
  keeps beside it (the VSS's spare magazine), within 1 cm for every weapon.
- **Changing it.** Putting a scope or a camouflage on keeps the weapon at the bench until the new
  one replaces it (no empty bench); the new one may show plain for a moment while its camouflage
  bakes.
- **Shown at once** (patch 48, owner 2026-10-21: "the server can be in the background"). A
  scope, camouflage or decal dropped on the window's weapon, or taken off it, is shown without
  waiting for the server: the lobby builds the weapon from its own copy with the change. The
  request is sent as before; the server's answer is the truth: confirmed, nothing changes on
  screen; refused, the weapon goes back to what the account says. A scope shows the variant made
  for that weapon (`srp_scope_fit`); one with none waits for the server. Weapons only.
- **Buttons.** The right (and other) buttons as the lobby's; the left button too at the bench,
  when its press is on the 3D view: not when it is on the window's controls or moves an item
  (a camouflage, a scope dragged from the inventory) — until it is let go.
- **Order.** The weapon appears at the bench first, then the camera flies to it (it waits for the
  weapon at most 2 s).
- **Light.** The lamp under the workbench (the scene's fill light there) is brighter, 2.5 times
  (a content patch).
- **Lobby only.** Nothing a match or a replay sees; a game without patch 45 shows the weapon in the
  character's hands (patch 32).

## The weapon's window: two pages

The owner (2026-10-21): "it should also be not called optics and be called attachments", "we need
to somehow differentiate skins/decals from real attachments like optics", "the menu should be only
a single one still", "maybe it can be < appearance > and when you press on those < > you go to
attachments", "scrolling form one preselects correct selection in upgrades in inventory", "the order
of decals/camo should be exactly the same as in appearance", "with camo at the bottom separated by
separator".

- **One window, two pages.** Attachments: the scope (slot 3). Appearance: the decal (slot 0) and
  the camouflage (slot 2). One page at a time, in armour's window as drawn (three slots at
  most, at scale 1).
- **The title bar.** The page's name in the middle, an arrow button on each side. Either arrow
  turns to the other page. A weapon with only one page (no scopes, or no camouflage or decal) has
  no arrows.
- **The arrows** (`PageArrow`; the owner, 2026-10-21: "buttons are good, though can we change their
  color and make them slightly wider"): the window's Confirm button art (`button_normal_bitmap`,
  `_hover_`, `_click_`, `_disabled_`: its slate blue-grey, lighter on hover, darker pressed) at
  21 x 24, its ends kept and its middle stretched, with a drawn triangle (6 x 10, light grey, a
  shadow): x 7 and 162, y 6.5 in the 180 x 27 bar. The title fits the 124 between them.
- **Names.** Attachments: the movie's `scope_window_title` (`st_srp_attachments`, a new text in the
  six languages: Attachments, Anbauteile, Dodatki, Обвес (russian and original), Обвіс;
  `scripts/content-edit/weapon-attachments-1.json`). Appearance: armour's title,
  `btn_upgrade_label` (Appearance, Verbesserungen, Wygląd, Внешний вид, Зовнішній вигляд). The
  weapon's button in the item panel and its inventory menu entry say Attachments.
- **Where the slots are.** Armour's places: the Appearance page has the decal at the top (where
  armour's second decal is), the line, the camouflage at the bottom (where armour's camouflage
  is); the Attachments page has the scope at the top, without the line.
- **Which page opens.** Attachments when the weapon takes scopes, else Appearance; the account's
  update after a change keeps the page.
- **The inventory follows.** Opening and each turn select the inventory's filter of the page, as
  armour's window opens on its own: scopes (22); camouflages and decals (25, 26).
- **Changes.** Each slot sends its change when it is made, on either page; Confirm closes the
  window. A turn changes nothing on the account.
- **Double click** (the owner, 2026-11-01: "I would expect the camo to equip, just like it works
  with double clicking weapons in inventory"). With the window open, a double click on an owned
  camouflage, decal or scope in the inventory puts it where a drop would: into the slot of its
  kind the page shows, a decal into the first empty decal slot, over the first when both are full.
  Nothing when the page shows no slot for it or the window already holds it; armour's window the
  same. With the window closed, as before: nothing (a cosmetic has no slot on the character).
- **The hint** under the weapon: the scope's on Attachments, armour's (with the decal editor's
  line) on Appearance.
- **Armour** keeps its window: no arrows, its title, its slots where the movie has them.

## Decals (patch 47)

The owner (2026-10-21): "implement and allow put two decals on weapons"; then, the client holding
three upgrades an item: "let's remove one decal instead for now". Two again since the weapon kit
(2026-10-25, [weapon-kit.md](weapon-kit.md)).

- **Which weapons.** The same 83 configs (`scripts/content-edit/weapon-decals-1.json`, after
  weapon-camo-1): `parameters/decal_available` and an empty `data/decals`, as armour has.
- **Which decals.** Any decal item (category 26); two, in the decal slots.
- **Slots.** [srp] 0 and 1 (`WEAPON_DECAL_SLOTS`, `modifications::cosmetic_slots`, as armour's):
  a weapon holds its scope, its camouflage and two decals at once, the decals as records of their
  own beside the kit (patch 68). Patch 47 bakes both entries of `data/decals`.
- **Shared cosmetics.** As for armour: one decal item on any number of items, each with its own
  place.
- **Placing one.** In the weapon's upgrade window (the weapon above the workbench, patch 45):
  "change" on a decal's slot, then a click on the weapon where it goes; the decal looks along the
  mouse's ray and lands on the first surface it meets. A click places it; a drag (either button)
  turns the weapon and places nothing, so its other side can be reached during the edit. The
  wheel with a modifier key: size or turn, as on armour. Cancel keeps the old place.
- **Never placed** (just put in the slot): on the left side of the receiver.
- **Size.** Half an armour decal's: 2.5 to 25 cm, 6 cm to start.
- **Where it is kept.** The record's u64, as armour's, in the weapon's own space (its model's,
  grip at the origin).
- **What a match sees.** Every game builds the weapon from its item's upgrades and bakes its
  decals, as the camouflage (patch 44); a game without patch 47 shows the weapon without them.
  No simulation change.
- **Moving a decal** to another decal slot (armour's two; a weapon has one) keeps it on the item,
  where it was placed (2026-10-21, the owner: "move the same decal into another slot ... it doesn't
  appear any longer": the window took it off after the move).
- **After a place.** The server sends the account back (only for a weapon), the lobby builds the
  weapon again and bakes the new place: the weapon shows its old decals for a moment.
- **Cost.** One bake per weapon surface for each new combination of camouflage and decals (the
  same cache as the camouflage's, `srpw1_*` names with the decals' textures and places).
- **On a camouflaged weapon** a surface without an `_aoac` texture (barrels, own sights) takes
  neither; without a camouflage, decals go on any surface.

## The engine (client 0.26g, PROVEN unless tagged)

- `camouflage_decal_texture_type_baker_cook` bakes a camouflage into a surface's textures only
  when `<diffuse>_aoac.dds` loads beside its diffuse (0x12E8E9): c = lerp(diffuse, camo(uv ×
  scale), 1 − aoac.G); overlay(c, aoac.B); c × aoac.R (`bake_decal_composition.ps`). Without it
  the surface stays plain, silently.
- A camouflage item's modification adds `camouflage` to the host config's `data` table (modifier
  type 3, path `data`); a host config without `data` drops it (0x2F1430). Weapon configs have no
  `data`: the data patch adds one (holding `camo_scale`).
- Only the character's parts (0x204560 → 0x129830) and the lobby's decal editor (0x2641A0) ask for
  a bake; weapon_cook never did. Patch 44 (`client-patches/weapon_camo_026g.asm`, client_patch.md
  §3.41) asks for one per weapon surface, as the editor does.
- The lobby (`flash/lobby_menu/src/survarium/upgrade/UpgradeWindow.as`): a weapon's upgrade
  window shows the scope slot (index 3, category 22) when the weapon takes scopes, and the
  camouflage slot (decalSlot2, index 2, category 25) when it has `camo_available`, and the decal
  slots (decalSlot, decalSlot1) for `decal_available`, on two pages (`showPage`; the arrows
  `addPageArrow`, the places `placeWeaponSlots`, armour's from `_slotPlaces`); the Attachments
  button shows on a weapon with any (`isUpgradeableWeapon`). The slots are filled while all are
  shown, then the page hides the other's: a slot keeps its item while hidden. The empty scope
  slot says `st_srp_scope_slot` (Optics, Прицел...), short enough for its field in every language;
  the stock `st_picatinny_scope_medium_name` (Optics / collimators) stays the inventory filter's
  tip and the item hint's prefix. A weapon with a stock scope says `st_default_scope_name`.

## The masks (`scripts/weapon-camo/`)

- Donors: the camouflaged textures of the premium weapons in our data (ported from 0.29a3 in
  patch 19) and in 0.28a2's resources.db, on the same UVs as ours (same base texture, or same
  mesh UVs; checked for the Lancaster's ported model).
- Two donors (Uzi, SKS, AKMN, Lancaster, AK-74N wooden stock): texels both leave alike are bare;
  a part is painted when they differ on most of it and each differs from the plain texture
  (uzi_camo_st only tints the Uzi's grips blue: not paint).
- One donor (the other 21): a part is painted when the premium differs from the plain texture on
  a fifth of it or more (bare parts differ on under 6 %, a dark camouflage over a dark part on
  about 44 %).
- From the donors' parts, the suggestion keeps the large ones (`MIN_PART`); parts.tsv overrides
  it per texture (a UV point on each painted part; a point on no part stops the build).
- The picker (`weapon_camo.py --picker page.html --camos <items>`): each texture with its parts
  outlined and the weapon in 3D beside it, the painted parts under the camouflage as the bake
  makes it, any of the game's 50 camouflage items; a click on either paints a part or leaves it
  bare; the premium texture can be shown beneath; the page gives the parts.tsv lines (kept in the
  browser meanwhile).
- The aoac: G the mask, B the plain texture's fine shading on painted parts (×4, 0.5 elsewhere),
  R its broad shading (to 0.3, 1 elsewhere), so the camouflage keeps the part's wear and edges.
  DXT1 with mips, the stock files' header.

## Checked

- `client-patches/check_weapon_camo_026g.py` (Unicorn): the hook site and the code on made-up
  configs and models, every case (selftest check 44 for the bytes).
- 2026-10-20, a local game (patch revision 25 build, a test patch of this data, a server of this
  branch, headless Xorg dummy at 60 Hz): an account with the City camouflage in slot 2 of an
  AK-12, then an Uzi: the lobby baked `srpw1_character_human_camo_premium_city_camo_weapons_ak_12_ak_12_0{d,n,s}`
  and the Uzi's, and the character's weapon showed the camouflage. The server kept the slot-2
  records at load.
- 2026-10-21, on main for revision 27 (the integration of #244): patches 44, 45, 47 and 48 in
  `.srpw` at 0xA42A000, after the death camera's `.srpc`; selftest and every
  `check_*_026g.py` pass. The content comes from the manifest (`release/content.tsv`): the pack
  `weapon-camo@1` (the masks), the edit lists weapon-camo-1, weapon-decals-1,
  lobby-bench-light-1 and weapon-attachments-1; `release/build-content 27` makes it with every
  edit list at 0 unexpected differences. **Lobby-tested only** (the camouflage, revision 25's
  build); this build is not run in a game.

## Not done

- Decals on weapons: written (patch 47, above), not run in a game: the preview on the weapon, the
  place a click gives and a weapon's decal bake are emulated only.
- A match (first and third person) not yet seen.
- The two-page window (arrows, places, the inventory's filter on a turn, the new texts): built,
  not seen in a game.
- The other 69 textures' picks were made after the owner's 22, by looking (the MP7 black copies the owner's MP7): worth a pass in the picker.
- One-donor masks look coarse: the AK-12, the Vepr-12 and the VSS / SR-3M textures; to look at
  every `guns.tsv` preview with the owner. If the 4.0 tiling looks too big on a weapon, give it
  its own `data/camo_scale`.
- A second decal on scoped guns: done by the weapon kit (patch 68, [weapon-kit.md](weapon-kit.md)).
