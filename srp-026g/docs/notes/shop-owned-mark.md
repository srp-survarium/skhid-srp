# The shop's owned mark and its "not owned only" toggle

The owner (2026-11-01): "I should see in shop all items that I bought somehow differently"; "same
for shop marks, they should be for everything"; "I would also want a button to show only non owned
items".

- **The mark:** a shop entry of an item the account owns has a green check mark on a small dark
  square at its tile's top-right corner (where an owned tile shows its equipped mark). The level
  (top-left), the price and the name stay as they are; the entry is not dimmed: it is still bought
  as before.
- **Owned:** the inventory has an item of that dictionary id, in any condition; a stack (ammo,
  consumables) while it holds any. Every kind: weapons, armour, scopes, camouflages, decals. Bundles
  are never marked (a bundle's id is not an item's).
  - A scope, camouflage or decal worn on an item stays one item of the stash (`items.md` §6, the
    stash lists it once, attached to its host): it is owned whatever wears it.
- **The toggle:** a square button at the right end of the filters' row (under the category tabs),
  the mark's check crossed out; its tooltip says what it does (`st_srp_shop_not_owned`). On (a green
  frame), the shop leaves out the entries of items owned, by the mark's own check, so the two never
  disagree; it works with every tab and filter. Shop only; off when the lobby opens, kept as set
  while it is open.
  - The filters are centred in the row but its right end, the toggle's room: with the most filters
    (equipment, seven) they still do not reach it.
- **When it changes:** the shop's list is drawn again whenever the account comes (after every buy
  and sale): a bought item gets the mark at once, or leaves the list with the toggle on; a sold one
  loses it.
- **From the inventory the movie has:** no server change.
- Code: `InventoryList.hasItem` (the one ownership check, also the buy's "already purchased"),
  `InventoryList.owned` (the same for the shop's tiles), `ItemRendererInv.updateOwnedMark`;
  `ShopOwnedToggle`, `InventoryList.makeOwnedToggle`, `hiddenAsOwned` (`ApplyInventoryFilter`'s
  base options), `OptionFilterControl.TOGGLE_ROOM`.
- Checked by `scripts/shop-owned-mark/check.sh` (Ruffle: owned and not owned entries, an empty
  stack, a scope loose and one worn on a weapon, a camouflage, a bundle, a buy and a sale; the toggle
  on, with the upgrades tab, a buy while it is on, off again; screenshots `before.png`,
  `bought.png`, `not-owned.png`, `upgrades.png`).
