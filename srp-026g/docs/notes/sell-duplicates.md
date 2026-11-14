# Sell duplicates

The owner's rules of 2026-11-14. Code: the lobby movie, `InventoryList.duplicates` / `makeSellDuplicates` /
`sellDuplicates`, `MessageApply` (its `silver` row).

- **Where:** the inventory window's bottom row (the Comparing items band), at its right: the lobby's own
  button (`SrvButtonSkinned`), not in the item row, which is for the item selected (the owner,
  2026-11-14). In the inventory only, not the shop. Greyed when there is nothing to sell or the
  inventory is locked.
- **What it does:** one question: the count of items it sells, and the silver they bring under it as the price windows show it (the game's price label, its silver icon); then one ordinary sale
  (`47 01`) of each duplicate, at the usual price (a quarter of the cost). The server knows no new
  request.
- **A duplicate:** a copy of an item (same dictionary id) whose modifications are not all there,
  once another copy of it has them all (rank 3, its name gold; the owner: "gold means three
  modifiers"). Until then the copies are kept to be combined; after it they are spares ("people who
  got golden gun no longer care about duplicates"). Any item with modifications: weapons, armour,
  gear. The whole inventory, not only the selected item.
- **Never sold:** a copy with all its modifications (different rolls suit different play); a copy a
  loadout uses; a worn cosmetic, an item wearing a cosmetic or a scope; stacks; ammo.
- **Release:** the lobby movie and two dictionary keys (`st_srp_inventory_sell_duplicates*`): a
  resources.db patch. No server change.
