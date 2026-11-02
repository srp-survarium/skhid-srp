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
- **The "Hide owned" box:** the lobby's own check box, its tooltip and label sizing as the
  character panel's Autobuy box's (the owner, 2026-11-01: "there was already toggle logic before").
  Label `st_srp_shop_not_owned`, tooltip `st_srp_shop_not_owned_hint`. Checked, the shop leaves out
  the entries of items owned, by the mark's own check, so the two never disagree; it works with
  every tab and filter. Shop only; off when the lobby opens, kept as set while it is open.
- **The shop's foot** (the owner, 2026-11-02: "Hide owned" comes after "Comparing items", directly
  below it, in the same column; it replaces the box's place left of Buy):
  - "Comparing items" at the left of the Buy button's row, on Buy's middle; "Hide owned" directly
    below it, at the same left edge (the window's margin), in the window's last row, where the
    compare box was. The two rows keep their pitch; the window has no room below its last row.
  - The two boxes look exactly the same (the owner, 2026-11-02): one symbol, the compare box's
    (`CheckBoxShared`), at one scale, read from the compare box when the box is made: the same box
    and tick, the label in the same font, size, weight and colour in every state. A check box is
    not sized by CLIK (`preventAutosizing`): its scale is its placement's, its label
    counter-scaled; the symbol's scale-9 grid keeps the square as it is. The compare box is
    narrowed to end where Buy begins, so neither box takes a press over Buy, nor its label reaches
    it. Autobuy's own symbol (`CheckBoxSharedPD`) is not used: it fills its bitmaps
    unsmoothed, so its tick was pixelated wherever the lobby is scaled to the screen (live rev 41).
  - The sort ("Sort", by level, by price) stays where it is, on the last row: "Hide owned"'s
    labels are shorter than the compare box's that sat there, so it has more room than before
    (the owner: "then sort can be moved a bit too"; not needed).
  - In every language the labels are whole and nothing overlaps: the boxes, their labels, Buy, the
    sort's label and choices, the search box, the list. The Polish compare label is "Porównanie
    przedmiotów" (was "Porównywanie przedmiotów", which reached Buy).
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
  on, with the upgrades tab, a buy while it is on, off again; in every language (the dictionaries'
  texts), the box directly below the compare box at its left edge, both labels whole, the foot's
  parts clear of each other and inside the window, no press over Buy or the sort's choices; the
  box as the compare box: the same symbol, bounds and scale, its label in the same font, size,
  weight and colour, at the same scale, as shown and in every state;
  screenshots `before.png`, `bought.png`, `not-owned.png`, `upgrades.png`, `long-label.png`).
