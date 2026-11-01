# The shop's owned mark and its "Hide owned" box

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
- **The "Hide owned" box:** the lobby's own check box, the character panel's Autobuy box
  (`CheckBoxSharedPD`): its look, label and tooltip as Autobuy's (the owner, 2026-11-01: "there was
  already toggle logic before"). It sits in the Buy button's row, at its left, at the window's margin
  as "Compare items" below it (the owner: "can it be on BUY level to the left?"); the longest label
  (Russian, "Скрыть имеющиеся") ends well before Buy. Label `st_srp_shop_not_owned`, tooltip
  `st_srp_shop_not_owned_hint`. Checked, the shop leaves out the entries of items owned, by the mark's
  own check, so the two never disagree; it works with every tab and filter. Shop only; off when the
  lobby opens, kept as set while it is open.
  - A box whose label fits its text (`UnfocusedCheckBox.fitLabel`, Autobuy's and this one's) sizes
    the label again at every state change: each state's frame may hold a text field of its own, and
    a checked box's label was cut to the symbol's width otherwise (seen in Ruffle).
- **When it changes:** the shop's list is drawn again whenever the account comes (after every buy
  and sale): a bought item gets the mark at once, or leaves the list with the box on; a sold one
  loses it.
- **From the inventory the movie has:** no server change.
- Code: `InventoryList.hasItem` (the one ownership check, also the buy's "already purchased"),
  `InventoryList.owned` (the same for the shop's tiles), `ItemRendererInv.updateOwnedMark`;
  `InventoryList.makeOwnedToggle`, `hiddenAsOwned` (`ApplyInventoryFilter`'s base
  options), `UnfocusedCheckBox.fitLabel`.
- Checked by `scripts/shop-owned-mark/check.sh` (Ruffle: owned and not owned entries, an empty
  stack, a scope loose and one worn on a weapon, a camouflage, a bundle, a buy and a sale; the box
  on, with the upgrades tab, a buy while it is on, off again; the box left of Buy on its row, with the Russian label too;
  screenshots `before.png`, `bought.png`, `not-owned.png`, `upgrades.png`, `long-label.png`).
