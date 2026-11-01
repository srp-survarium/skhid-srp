# The shop's owned mark

The owner (2026-11-01): "I should see in shop all items that I bought somehow differently".

- **What:** a shop entry of an item the account owns has a green check mark on a small dark square
  at its tile's top-right corner (where an owned tile shows its equipped mark). The level
  (top-left), the price and the name stay as they are; the entry is not dimmed: it is still bought
  as before.
- **Owned:** the inventory has an item of that dictionary id, in any condition; a stack (ammo,
  consumables) while it holds any. Bundles are never marked (a bundle's id is not an item's).
- **When it changes:** the shop's list is drawn again whenever the account comes (after every buy
  and sale), so a bought item gets the mark and a sold one loses it at once.
- **From the inventory the movie has:** no server change, no new text.
- Code: `InventoryList.hasItem` (the one ownership check, also the buy's "already purchased"),
  `InventoryList.owned` (the same for the shop's tiles), `ItemRendererInv.updateOwnedMark`.
- Checked by `scripts/shop-owned-mark/check.sh` (Ruffle: owned and not owned entries, an empty
  stack, a bundle, a buy and a sale; screenshots `before.png`, `bought.png`).
