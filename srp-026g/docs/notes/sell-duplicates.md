# Sell duplicates

The owner's rules of 2026-11-14. Code: the lobby movie, `InventoryList.duplicates` /
`sellDuplicates`, `ItemControlPanel` (the button).

- **Where:** the inventory's item row, in the place of the stock Fix button (repair is gone); shown
  where Sell is (an owned item that is no stack), not in the shop. Greyed when there is nothing to sell.
- **What it does:** one question naming the count of items it sells and the silver they bring, then one ordinary sale
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
