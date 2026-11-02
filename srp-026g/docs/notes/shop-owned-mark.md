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
- **The "Hide owned" box:** the lobby's own check box, the character panel's Autobuy box: its look,
  label and tooltip as Autobuy's (the owner, 2026-11-01: "there was already toggle logic before").
  Its symbol is "Comparing items"' (`CheckBoxShared`): the same box, tick and label as Autobuy's
  (`CheckBoxSharedPD`), whose bitmaps fill shapes unsmoothed, so its tick was pixelated wherever
  the lobby is scaled to the screen (live rev 41). Its look is otherwise as the compare box's in
  the game (the owner, 2026-11-02, after the game test: the box sizes and labels were fine there;
  Ruffle draws the compare box's square stretched and some labels in another weight, the game
  does not). Label `st_srp_shop_not_owned`, tooltip `st_srp_shop_not_owned_hint`. Checked, the
  shop leaves out the entries of items owned, by the mark's own check, so the two never disagree;
  it works with every tab and filter. Shop only; off when the lobby opens, kept as set while it is
  open.
  - A box whose label fits its text (`UnfocusedCheckBox.fitLabel`, Autobuy's and this one's) sizes
    the label again at every state change: each state's frame may hold a text field of its own, and
    a checked box's label was cut to the symbol's width otherwise (seen in Ruffle).
- **The window's last row** (the owner, 2026-11-02: "put both on the same row"): [Comparing items]
  [Hide owned], from the window's margin; "Hide owned" after the compare box's label as far as that
  label is from its square (`InventoryList.placeOwnedToggle`, when the shop opens and when the
  texts come). Every language's own texts fit whole. The Buy row keeps only Buy. "Hide owned" is
  above the compare box: it takes the presses on itself (a box's invisible area reaches past its
  label).
- **No sort** (the owner, 2026-11-02: "noone cares about sorting by price, so we can just remove
  that sort completely", "it should always be per level"): the list (shop and inventory) has no
  "Sort / By level / By price"; the timeline's sort label and choices are taken off it. Its one
  order is `InventoryList.LIST_ORDER`: what can be bought first, then by level (the price last).
- **When it changes:** the shop's list is drawn again whenever the account comes (after every buy
  and sale): a bought item gets the mark at once, or leaves the list with the box on; a sold one
  loses it.
- **From the inventory the movie has:** no server change.
- Code: `InventoryList.hasItem` (the one ownership check, also the buy's "already purchased"),
  `InventoryList.owned` (the same for the shop's tiles), `ItemRendererInv.updateOwnedMark`;
  `InventoryList.makeOwnedToggle`, `placeOwnedToggle`, `hiddenAsOwned` (`ApplyInventoryFilter`'s base
  options), `UnfocusedCheckBox.fitLabel`.
- Checked by `scripts/shop-owned-mark/check.sh` (Ruffle: owned and not owned entries, an empty
  stack, a scope loose and one worn on a weapon, a camouflage, a bundle, a buy and a sale; the box
  on, with the upgrades tab, a buy while it is on, off again; the box the compare box's symbol;
  the list in level order when it opens, after a buy, with the box on, on a tab and in the
  inventory, and no sort on it; the last row in every language (the dictionaries' texts): the box
  on the compare box's row after its label, both labels whole, inside the window, above the
  compare box; screenshots `before.png`, `bought.png`, `not-owned.png`, `upgrades.png`).
