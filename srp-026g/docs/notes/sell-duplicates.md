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
- **A duplicate:** a copy of an item (same dictionary id) past the ones kept. The whole inventory, not
  only the selected item.
- **Kept copies:** one a loadout uses, a worn cosmetic, an item wearing a cosmetic or a scope. An item
  none of whose copies is kept keeps the copy of the highest rank.
- **Never sold:**
  - ammo, of any kind;
  - stacks (consumables);
  - golden items (golden weapons, premium armour): golden copies of different rolls suit different
    play;
  - the plain copies of a weapon that has a golden version, while no golden one of it is owned (they
    are kept for crafting). Owning a golden one, the spare plain copies are sold as any other.
- **Release:** the lobby movie and two dictionary keys (`st_srp_inventory_sell_duplicates*`): a
  resources.db patch. No server change.
