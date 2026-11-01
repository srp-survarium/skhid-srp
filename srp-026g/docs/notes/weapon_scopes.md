# 0.26g weapon scopes: bought once, put on any number of weapons

**Version scope:** client 0.26g (build 2777, no PDB; VAs are image VAs, base 0x10000), srp's
lobby. Tags: PROVEN (read at the address or in the file named, or seen in a local game),
LIKELY, OPEN. The server side is `crates/survarium/src/v026g/scopes.rs` and the scope rules
in `crates/session/src/v026g/items_rules.rs`.

## 1. What 0.26g has

- Three scope items, category 22: `scope_leupold` 69, `scope_pso1` 304, `scope_giperon` 362
  (`gameplay/items/scopes/*.options`, block `data`: `fov_factor`, `hide_weapon_on_aim`,
  `aimed_model`, `idle_model`, `type`, `near_plane_factor`, `change_scope_factor`). No profile
  slot, no drop weight; the dictionary gives 69 and 304 a `base_cost` (25,000 and 20,000) but no
  shop ever listed them. PROVEN (data).
- A weapon names its scope in its own config, `addons.rifle_scope_dict_id`: rem_700 69;
  svd, vsk_94, vss, vss_premium 304; sv_98, vssk 362. No other weapon has `addons`. PROVEN.
- A scope is **view-only**: the zoom and the models only. No shooting, dispersion, recoil, aim
  time or hit code reads it; bullets land at the screen centre whatever the scope shows (seen in
  a local game). So a scope never changes what a match computes. PROVEN.

## 2. How the client builds a weapon's scope (PROVEN)

- `weapon_cook::on_weapon_config_loaded` 0x24A920 reads `addons.rifle_scope_dict_id`
  (0x24AA0C..0x24AA33), looks the id up in the item dictionary and loads that item's config as a
  sub-resource of the weapon; `rifle_scope_cook` (0x258580, 0x258740) builds the scope from its
  `data` block.
- An item with upgrades is built from a **modified config** (query builder 0x2F14C0, class 0x43
  `item_config_cook`): `item_config_cook::translate_query` 0x2F1750 applies, for each upgrade
  record, the `parameters.modification` of the item **whose dict id is in the record** (the
  record's second u32, read at 0x2F15A2), looked up in the table the dictionary cook collects
  from every item config's own `modifications` block (0x2C9E31.., dictionary +0x150).
- A modifier of `type` 0 sets a config node; `type` 3 adds a child to a named table, given as a
  `[name, value]` list, and only to a table that already has a child (an empty `addons` gets
  nothing). Seen in local games: a weapon without a stock scope needs `addons { mount: 1 }` as a
  placeholder, which the weapon cook ignores.
- So a **variant** (a category-22 entry made for one weapon: the optic's models placed for that
  weapon's mount and eye line) whose modification sets (`type` 0, weapons with a stock scope) or
  adds (`type` 3, the others) `addons.rifle_scope_dict_id` = itself puts its scope on the weapon
  that wears its record. Without the record the weapon is built from its plain config: a stock
  scope comes back.
- The record on the wire: `n2 × {u32 upgrade item id, u32 dict id, u64 value, u8 slot}`, at most
  three an item, in the lobby's inventory and in a match's static info alike.

## 3. The feature (srp)

- **Shop items.** One scope item per optic, category 22, sold by seller 0 at no reputation
  level (a scope is view-only: nothing to unlock), at the item's own `base_cost` from the
  dictionary, the same number the client shows and sells back for a quarter of. Seller 0 is the
  shop's own shared seller: the movie puts a tab for it before the factions' tabs, named by
  `non_faction_items` (`st_non_faction_items`, "Independent traders"; `InventoryList.fillSellers`),
  and every record of seller 0 is buyable at any reputation (`GangShop.setupShopItems`:
  `_unlockedLevel` starts at 0 for every seller, and a seller-0 record goes straight into the
  buyable list). PROVEN in the ActionScript; the tab with scopes in it not yet seen in a game.
  Category 22 falls in the movie's fourth item group (`ItemFactory.getCategory`: everything but
  armour, weapons, ammo and cosmetics), with drugs, traps and grenades. Not sold: 0.26g's
  three scope entries, the variants, any category-22 entry no variant names. Scopes never drop
  and never go into a kit slot (category 22 has no profile slot; data giving it one is refused at
  start).
- **Pairs from the game data.** The server reads, from every category-22 item's config, a block
  the client does not read:

  ```text
  srp_scope_fit {
      scope: <dict id or name of the scope item sold in the shop>
      weapons: [<dict id or name of a weapon>, ...]
  }
  ```

  A variant is kept when its own `modifications` holds its `parameters.modification`; when that
  modification puts the variant itself into each weapon's scope as the weapon can take it (a
  `type` 0 set of `[addons, rifle_scope_dict_id]` on a weapon with a stock scope, a `type` 3 add
  of `[rifle_scope_dict_id, <variant>]` to `[addons]` on one without a scope but with an `addons`
  child: an add where a stock scope is would hit the client's duplicate-name assert, a set where
  none is would show nothing); when its scope is a non-premium category-22 item that is no
  variant and no weapon's stock scope (0.26g's 69, 304, 362 are never sold or put on); and when
  each weapon is of a weapon category (8, 13..17). On a weapon with a stock scope the bought one
  replaces it while the record is on; taken off, the plain config brings the stock one back. A `(weapon, scope)` pair named twice keeps the lower variant id. Whatever is left
  out is written to the log at start with the reason. The pairs live only in the game data, so
  the server and the players' game cannot disagree about them; **game data with no variant has
  no pair: no scope is sold and none goes on a weapon** (the players' data until the release
  patch).
- **Attach** (`46 01 <kit> <weapon> <scope> 3 <u64 0>`): only to a weapon the scope has a variant
  for, only into slot 3 (the armour camouflage's; a weapon's camouflage goes into 2), one scope a weapon (a
  second replaces the first). The record is `{scope item id, the variant's dict id, 0, 3}`. The
  scope stays one item of the inventory; any number of weapons wear it, each its own record (the
  rule of shared cosmetics, PR 149). After `83 46 01` the client writes its own copy of the
  record with the scope item's dict id (0x275671); the inventory and account that follow every
  answer replace it with the server's (variant) record (LIKELY: srp's client model reads `89` so,
  as for every cosmetic; to be seen in the lobby's 3D view right after a drop).
- **Detach** (`46 02`, client patch 30 names the weapon): off that weapon only. A scope that is
  sold comes off every weapon (`drop_gone_upgrades`).
- **Repair of what is kept.** On load (`make_safe`) a scope record must hold a variant made for
  its weapon, in slot 3; after any change of the inventory (`drop_gone_upgrades`) a record must be
  the one its scope item makes on that weapon. Anything else is taken off.
- **A release that withdraws a pair or a variant** never stops srp. A record's dict id is not part
  of the start's check of the kept state (`known.rs`, which still refuses an *owned item* the data
  lacks). An account's record that no longer fits, or whose variant is gone, is taken off at the
  load, logged with its reason, and the account is written again. A kept results screen loses such
  records when it is restored, logged, and is written again. The wire never sends a record whose
  dict id the data lacks. Seen on a local test server on 2026-10-17: a kept report named a
  withdrawn variant and the start was refused; test
  `a_withdrawn_scope_variant_in_the_kept_state_is_taken_off_at_start`. Not covered: match
  recordings and replays of matches played before the withdrawal keep the old records (raw
  datagrams); a game that watches such a replay loads them.
- **Matches.** Records reach the match as they are (`play.rs` `match_profile`, the static info's
  item upgrades); the scope item itself is in no kit slot, so it is not in the match, adds no
  weight, wears nothing and does not count toward the gear level (matchmaking, team balance).

## 4. What a release needs besides this server change

1. A release patch (`resources.db.patch_<n>`, `release/patches.tsv`) with: the 84 variants
   (dict ids 2001-2084) with their models, materials and textures, each config carrying its
   `srp_scope_fit` block; the weapon configs that get an `addons` placeholder; the twelve scope
   items (dict ids 2101-2112) in `items_dict` with their `base_cost`; and 99
   `items_compatibility` rows, weapon and scope item (premium copies counted), for the lobby
   movie to grey out a scope that does not fit, plus 7 rows weapon and its stock scope (69, 304,
   362) for a greyed "standard scope" in an empty slot. A stock scope stays no shop scope: not
   sold, not attachable, no kit slot. In the game's file `items_compatibility` and
   `slots_restrictions` are named tables (`compatibility_<n>`, `restriction_<n>`): new rows need
   names of their own. srp reads compatibility only for items in a kit's ammo slots
   (`check_play`), where no scope can be: such rows change nothing on the server. A
   `slots_restrictions` row for category 22 would let a scope into a kit; srp refuses such data at
   start.
2. The scope items' configs: a `data` block for the lobby's 3D view and a
   `parameters.modification` of their own that changes nothing a weapon has (a modifier at a path
   no weapon config has is skipped, 0x2F1430), so the client's own copy of a record between
   `83 46 01` and the inventory that follows never names a modification the table lacks. OPEN:
   whether the client tolerates an upgrade item with no modification at all; not relied on.
3. Names and icons: the stock scope configs all name `st_leupold_name` and icon `[0, 0]`; each
   new item needs its own `ui_desc` keys, localisation lines and icon.
4. The lobby movie (`lobby_menu.swf`): the upgrade action and a slot that takes category 22 for
   weapons, the icon container and a label (the stock movie offers upgrades only for armour with
   `camo_available` / `decal_available`), and a shop tab that lists the scope items.
5. A look on a second screen (another resolution and scaling) before the release.

Not needed: a client patch beyond patch 30, a simulation change (a scope is view-only), a server
table in code.

## 4a. A sight's card

- A sight's card lists the weapons it goes on ("Can be installed on: …"). The weapons in the
  chosen kit's slot 1 or slot 2 are gold in that list (owner 2026-10-22: "it should in gold
  highlight your weapon, if it is in the list ... it should work for weapon being either in slot 1
  or slot 2 in the currently selected profile"). The rest of the list stays as it was.
- **Hover:** hovering a weapon (in the list or the kit) makes the listed sights that fit it look
  hovered; hovering a sight in the list makes the kit's slot 1 and slot 2 weapons it fits look
  hovered, as the kit's copy of a hovered item already did (owner 2026-10-22: "when I hover mouse
  over my weapon the optical sights that fit into it are highlighted too with the same hover, same
  the other way around").
- Code: `ItemInfoRenderer.scopeHintText`, `UpgradeWindow.kitWeapons`, `InventoryList.kitWeaponDicts`. Hover: `LobbyMenu.linkScopeHovers`, `InventoryList.hoverScopesFor`, `LocalPaperDoll.hoverWeaponsFor`.

## 5. Weapon right-click menu

The released revision 19/20 movie's `ItemCard` and `ItemControlPanel` already allow
the scope editor for a weapon with `UpgradeWindow.hasScopes(dictId)`. The separate
`InventoryList.isUpgradeable` predicate still only accepts armor. Both inventory
right-clicks and `LocalPaperDoll.onMouseItemClick` use `InventoryList.showContextMenu`,
so both omit the action despite the working item-details button.

The lobby's `InventoryList` (`flash/lobby_menu/src/survarium/controls/InventoryList.as`) fixes
that predicate (`isUpgradeable`) and gives weapons the existing localized `scope_window_title`
menu label (`upgradeLabelKey`). The existing
`upgradeElelemnt` callback opens the normal editor; `UpgradeWindow.setData` selects
category 22 and its normal slot handles attach, replace and detach.

Since 2026-10-21 `scope_window_title` says Attachments, not Optics (`st_srp_attachments` in the six
languages, `scripts/content-edit/weapon-attachments-1.json`; it was patch 19's `st_srp_optics`): the
weapon's window has an Attachments page (the scope) and an Appearance page (camouflage and decals),
[weapon-camo.md](weapon-camo.md#the-weapons-window-two-pages). Compatibility
comes from the movie's existing `scopePairs`, populated by the game data. Weapons
with no compatible scopes get no action. Armor keeps its cosmetic predicate and
`btn_upgrade_label`; ownership, stack, disabled-item and locked-inventory checks
remain in the original menu method.

It is built with every other lobby change by `client-patches/build_lobby_026g.py`,
from source ([lobby-source.md](lobby-source.md)): `InventoryList.isUpgradeable` also takes a
weapon with scopes (`UpgradeWindow.isWeaponCategory`, `UpgradeWindow.hasScopes`), and the upgrade
entry's label is `LocalizationData.getName(this.upgradeLabelKey(itemData))`; every other method
stays revision 24's byte for byte. (The work of 2026-10-18, built on revisions 19/20, was taken up again on
2026-10-20 as a bytecode step, and became this source feature the same day.)
No executable, protocol or simulation change is needed.

Validation of the first version (2026-10-18; the build of 2026-10-20 on the newer
movie is checked in the game separately): built against both released input movies with FFDec
26.0.0; the saved revision 20 movie decompiles to the expected predicate call
and localized label, retaining the original `upgradeElelemnt` callback. Four
patch-tool tests and the five `cargo test -p session scope` tests pass. In the
native revision 20 client under Wine at 1280x720, right-clicking the equipped
AK-74M and an inventory SVD showed “Optics” and opened the existing editor.
Compatible PSO-1 variants attached and persisted on the server; dragging the SVD
scope back to inventory removed its saved upgrade and restored “Stock scope”.
Incompatible scopes were greyed out, and the TT-33 menu had no optics action.

## 6. The rifle scopes from 0.69d0 (2026-10-26)

The owner (2026-10-26): "let's do those 8", the scopes the later builds added for rifles that 0.26g's
scope code can show (one zoom, the reticle in the aimed model). The picks the owner left open are
the coordinator's, to be changed: the CompM4's item itself goes to x2; the switchable scopes at one
zoom (the aperture sight x3, the TA01 x4, the Leupold 3-7 x6, as 0.46c2); prices in line with the
scopes sold.

| item | dict id | zoom | price | donor | fits |
|---|---|---|---|---|---|
| CompM4 collimator (the sold one) | 2110 | x1.25 -> x2 | 9,000 | 0.46c2 and 0.69d0 have it x2 | as before |
| NcSTAR 4 collimator | 2113 | x1.25 | 7,000 | 0.69d0 `ncstar_4_collimator` | 37 weapons |
| EOTech XPS2 collimator | 2114 | x1.25 | 9,000 | `eotech_xps2_collimator` | 34 |
| PK-01VI collimator | 2115 | x2 | 10,000 | `belomo_pk01_collimator` | 35 |
| RSA-S collimator | 2116 | x2 | 10,000 | `hensoldt_rsas_collimator` | 37 |
| Aperture optical sight | 2117 | x3 (0.69d0: x3/x6) | 11,000 | `aperture_scope` | 17 |
| TA01 optical sight | 2118 | x4 (x4/x8) | 14,000 | `acog_ta01_4x32_scope` | 17 |
| Leupold 3-7 optical sight | 2119 | x6 (x4/x8; 0.46c2 x6) | 17,000 | `leupold_3_7x4x8_scope` | 7 |

Made by `scripts/rifle-scopes` (prepare_donor.sh, prepare_live.sh, survey.py, make_pack.py;
pack `rifle-scopes`, edit list `rifle-scopes-1`):

- **Placement, as patch 19's** (PROVEN in our data): our variants (models/weapons/attach/fit*,
  "0.51c5 modules, placed by locfit.py") are 0.69d0's own module meshes (0.51c5's, vertex for
  vertex) moved by one translation per weapon, with the rails the weapon needs (picatini_lastochka
  on a Kalashnikov, a scope rail where there is none). A new optic takes, on each weapon, the
  translation and rails of our variant of its kind there (`mounts.tsv`): a collimator a
  collimator's, a scope a scope's. The weapon draws a scope model at scope_point turned 180 degrees
  about Y (x and z mirrored: the MP7's and the Vepr-12's variants centre their eye so).
- **The eye**: a collimator's variant sets the weapon's model to one whose `aim` locator is on the
  optic's sight line (x and y; as our variants do: `hk416_a5_n_aimpoint_compm4` and the like); one
  of ours with that eye already is taken, else the mount variant's weapon model is copied with only
  that locator changed (`variants.tsv`). A scope's aimed view (the weapon hidden) is the eyepiece of
  our scope variant on that weapon (its background and lens) with the new scope's reticle grid and
  dot centred where that variant's are, and its weapon model: the same eye.
- **The reticle**: 0.69d0 draws a collimator's reticle as a separate model; 0.26g draws the aimed
  model alone. The red dot of ours (patch 21's `collimator_glowing_dot`, an emissive disk with the
  view-angle falloff of a collimator) is baked into each new collimator's aimed model as a surface,
  at the optic's eye point 0.6 m on along the sight line, where every collimator of ours has it
  (resource-porter's `vostok-port compose-model`, which also places the optics: a general merge of
  static models' surfaces, each moved). The XPS2 gets the dot, not 0.69d0's ring.
- **Fit**: each weapon of ours 0.69d0 gives the module to (`mods.<name>.used_by` of the weapon
  survey; a premium copy as its weapon; `a545` is our `a_545`) that has a mount of the scope's kind
  (`fit.tsv`). Weapons with no variant of a picatinny optic of ours (the SVD, VSS, VSK-94, SV-98,
  Rem 700, Mosin and the like) take none. A weapon with a stock scope (the HK416's and MSBS's
  EOTech/Walther) gets the set form (type 0), the others the add (type 3), as section 3 says.
- **Materials**: the lens, reticle and grid materials are ours (weapons/loc2, materials.tsv); each
  optic's own is 0.69d0's made again on a loc2 template, given 0.69d0's textures slot for slot
  (albedo, normal, smoothness as roughness, metalness as fresnel; retexture.py).
- **Shop**: items 2113..2119 by seller 0 at no rank at their base_cost (as every scope item with a
  pair, `prices::sellers_of_in`); icons rendered from the models (render_icon.py) in column 4 of the
  consumables atlas (rows 13 to 19); names and short descriptions in the six languages of the pack
  (texts.tsv) and in all ten of `client-patches/localization/*.tsv`, which `localization-1.json`
  compiles last. The CompM4's name says x2 there.
- **Size**: 191 items (7 shop items, 184 variants), 139 weapon models with a moved eye (a copy of a
  weapon's model, about 0.3 MB each), 24 textures; the content patch grows by about 170 MB.
- **Checked**: `release/build-content` changes only the dictionary, the localization tables,
  the lobby movie (the icons) and the 26 CompM4 configs, and adds the pack's files; srp starts on the
  data with 353 scope pairs (169 before) and no variant left out. A review sheet renders each scope
  on three weapons, outside and aimed (`review.py`). **Not run in a game.** The in-game check: each
  scope's aimed view on a few weapons (the reticle centred, the dot in the window, the zoom), the
  outside look on the AKs' side rail and the M4's carry handle, the Leupold 3-7's eyepiece far back
  on the AKs, the icons, the CompM4 at x2.
- **In the game (2026-11-01, @1, headless, a local server on the release-41 build):** the shop lists the seven
  with their icons and prices, the CompM4 reads x2 and zooms x2; NcSTAR 4, XPS2, CompM4 on the M4A1, TA01 and
  aperture on the HK416: dot and reticle at the screen's centre; zooms x6 : x4 : x3 : x2 as set. Three faults,
  fixed in @2:
  - the AK-74M's TA01 and Leupold 3-7 reticles 10 and 16 px above the centre: their weapon model's aim locator
    is turned 0.004 rad about x, which tilts the view off the bore the eyepiece is set along. A scope's
    variant now sets the model again with that locator unturned (`<model>_n_scope`); a collimator's eye copy
    always was unturned. Patch 19's own scope variants on such weapons (the AK-74M's ACOG TA31 among them)
    keep the tilt: not this pack's;
  - the NcSTAR 4 on the M4A1 under its carry handle: its eye matched the stock model's, which was taken;
    the mount variants set a flat-top model. A model the mount variant does not set, or with a turned aim
    locator, is no longer a candidate;
  - the aperture sight's body garbled: its material is made on the Leupold Mk4's template (the Leupold 3-7's,
    which drew right), not the ELCAN's.
  - @3, the aperture sight's body still garbled with @2's material: its mesh runs its texture coordinates to u = 1.91
    (0.69d0 repeats the texture; 0.26g clamps a weapon's, smearing the edge's texels over those faces). The made
    models' triangles wholly past an edge are moved back by whole textures (`wrap_uv`; a surface with a triangle
    across an edge, our eyepiece's tiled background, is left as it is).
  - @4, the owner's game (2026-11-01): the new optics' bodies drew nearly black with light streaks, metal as a
    mirror of its surroundings. 0.69d0's metalness-workflow maps had gone into 0.26g's slots as they are:
    metalness 1.0 as a Fresnel of 1.0, smoothness as gloss, bare metal bright in the albedo. They are converted
    as resource-porter converts a 0.69d0 weapon's (`pbr_026.py`): the diffuse `0.514 * (albedo * (1 - 0.8 *
    metalness)) ^ 0.75`, the gloss 0.36 x smoothness and the Fresnel 0.162 x metalness^0.388 in the DXT
    endpoints, the normal map as it is. The camouflage bake does not reach a scope: it bakes only the weapon
    model's own surfaces (the game's log: `srpw1_..._<model>_<0..n-1>`, n the weapon's surface count).

## 7. Scope fits for the release-41 guns, and the Walther Competition II (2026-11-01)

The owner (2026-11-01): "Properly port all weapons. And their attachments (all we can)." Every scope
we sell goes on each release-41 gun 0.69d0 gives it to; the one optic of 0.69d0 and 0.46c2 that we
did not sell, the Walther Competition II collimator, becomes a shop scope. Pack `scope-fits`, edit
list `scope-fits-1`, made by `scripts/scope-fits` (prepare_donor.sh, prepare_live.sh, survey.py,
make_pack.py, review.py).

- **What else exists:** 0.69d0's optics are 0.46c2's and more. Of them we did not sell the Walther
  Competition II (the HK416's and MSBS's stock collimator), the EOTech G33 x3 magnifier (it sits behind a
  collimator: no scope alone, 0.26g has one zoom a scope) and the Leupold 3-7 "summer17" (a skin). Laser
  sights and suppressors are not scopes (and no suppressors, ever).
- **The Walther Competition II:** item 2120, seller 0, 8,000 silver, x1.25 (fov 0.8, as 0.69d0's
  aim_zoom 1.2), its icon in LobbyMenu_AmmoIcons [20, 4], the texts our stock Walther has
  (`st_walther_competition2_collimator_*`). Made as the rifle scopes' collimators: 0.69d0's module and
  aimed model, our red dot at its eye 0.6 m on; drawn with the stock Walther's material (the same
  mesh and UVs, turned half about Y: 0.26g's own textures, nothing imported). It goes on each
  weapon 0.69d0 gives it to that has a collimator mount of ours (section 6's mounts.tsv) and on the
  release-41 guns, but not the HK416 and MSBS (it is their stock scope).
- **Where an optic goes** (`survey.py`, `mounts.tsv`): where 0.69d0 mounts it: the gun's slot, or the
  rail a module adds (the MP5's claw mount, the FAMAS carry-handle rail, the G3's claw mount, the
  KS-23M's and PPSh's medium rail, the AKMSU's Zenit B-18 on its cover, the SVU's dovetail mount,
  the pistols' bridge mount), the rail drawn with the optic. The new guns' meshes are 0.69d0's vertex
  for vertex (checked), so 0.69d0's locators hold, each in its bone's frame (the Minimi's rail hangs
  on its feed cover's bone). A chain that needs a module our gun does not have in its mesh (a
  replaced handguard) is not taken. The variant's translation is the scope frame's: the weapon
  draws a scope at scope_point turned half about Y, as 0.69d0 draws a module at its locator
  (PROVEN: patch 19's AK-12 and M4A1 variants sit exactly where 0.69d0's locators put the module).
- **How a variant is made** (`make_pack.py`): from a variant of ours of that scope (its reference: the
  same models, materials, red dot and eyepiece players know), the module's mesh found in it (exactly,
  or by its bounding box: our Kobra, DeltaPoint and old PSO-1 are the same shapes written in another
  order; our Leupold Mk4's turret is bigger, its lens the same), moved to the gun's mount. A
  collimator's eye: the gun's model copied with only its `aim` locator moved onto the sight line
  (as section 6). A scope's aimed model (its eyepiece) sits before the gun's own eye as it sits
  before the reference's; the gun's model is its own (hidden when aiming). The L85A2's and Fort-401's
  rear sights, which their port baked in on the very rail the optic takes (0.69d0 takes the
  module off for it), are left out of the model every scope variant of theirs sets.
- **Fits** (fit.tsv): 212 variants, dict ids 2901 on (claimed to 3299). No scope: the OTs-14 Groza
  (0.69d0 mounts its optics only on the tactical handguard that replaces its own; our 0.31a2 Groza has
  its carry handle and handguard in its mesh) and the Mauser (0.69d0 gives it none). The Docter fits
  none (0.69d0 links it but gives it no slot on them).
- **Checked:** `release/build-content` with the three weapon packs; srp starts on the data; a review
  sheet (`review.py`) of each gun with a collimator and a scope, outside and aimed: optics on their
  rails, reticles centred, red dots in the windows. The pack is made from the weapon packs' files
  (`guns.sha256`): a new version of a weapon pack means survey.py and make_pack.py again.
- **@2 (2026-11-01, after the owner's game):** made on the final weapon packs (weapons-031@7, weapons-046@6,
  weapons-069@9): @1's model copies were of older gun models. The rails' 0.69d0 textures are converted as the
  rifle scopes' @4 converts the optics' (`../rifle-scopes/pbr_026.py`): taken as they were, metal drew as a black
  mirror. Every variant's reticle (red dot, else grid) is checked against its weapon model's eye: before it as
  before its reference's, within 0.2 mm, and the eye unturned (`eyes.tsv`; the SCAR-H's Walther: 0.00 mm). The
  camouflage bake reaches only the weapon model's own surfaces (the game's log names surfaces 0..n-1 of the
  variant's gun model), not the scope's.
