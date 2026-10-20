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
  for, only into slot 3 (the camouflage's; weapons take no camouflage), one scope a weapon (a
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

## 5. Weapon right-click menu

The released revision 19/20 movie's `ItemCard` and `ItemControlPanel` already allow
the scope editor for a weapon with `UpgradeWindow.hasScopes(dictId)`. The separate
`InventoryList.isUpgradeable` predicate still only accepts armor. Both inventory
right-clicks and `LocalPaperDoll.onMouseItemClick` use `InventoryList.showContextMenu`,
so both omit the action despite the working item-details button.

`client-patches/scope_context_menu_026g.py` fixes that predicate and gives weapons
the existing localized `scope_window_title` menu label. The existing
`upgradeElelemnt` callback opens the normal editor; `UpgradeWindow.setData` selects
category 22 and its normal slot handles attach, replace and detach. Compatibility
comes from the movie's existing `scopePairs`, populated by the game data. Weapons
with no compatible scopes get no action. Armor keeps its cosmetic predicate and
`btn_upgrade_label`; ownership, stack, disabled-item and locked-inventory checks
remain in the original menu method.

Build an extracted movie for the next resource patch (JDK and FFDec 25.1.3):

```sh
# development only: this one step; the movie that ships is build_lobby_026g.py's
python3 srp-026g/client-patches/scope_context_menu_026g.py \
  <the craft window's movie> /tmp/step.swf --ffdec /path/to/ffdec.jar --step-only
python3 -m unittest discover -s srp-026g/client-patches -p test_scope_context_menu.py
```

The builder accepts the newest movie of the lobby patches' chain (since
2026-10-20: revision 24's with Get Gold, the exchange update, Buy on owned items
and the craft window, `1091a19d…`; the output with FFDec 25.1.3 is deterministic,
its hash in the README) and refuses unknown or already patched inputs and existing
outputs. The work of 2026-10-18 (built on revisions 19/20, never merged) was
taken up again then. It changes one ABC block, verifies every
unrelated existing method is byte-identical, and transplants that block into the
original movie without round-tripping its Scaleform graphics. It is step 6 of
`build_lobby_026g.py` (README "Building the lobby movie"): the movie that ships is
that driver's, never this step's output alone (it would lack the steps not run);
do not replace an already published patch. No executable, protocol or simulation change is needed.

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
