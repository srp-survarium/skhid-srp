# The character menu in the shop

The owner (2026-10-22): "when you are in shop, your character profiles are shown on the left like
with inventory ... not character, but character menu ... with 3 profiles currently equipped".

- **Where:** the Shop tab. The character menu (the kits and what each has on) shows on the left, as
  on the Inventory tab. Before, the shop hid it.
- **What it does there:** what it does on the Inventory tab. A shop item dropped on one of its slots
  is bought into that slot, as the game already did (`DragRegion`, `MainScene.buyFunction`).
- **Its right-click menu acts on the kit's item:** choosing a kit item makes it the card's item
  even though the shop does not list it (`InventoryList.selectItem`). Before that fix, Attachment
  there opened the shop card's item, which the game does not have, and the game crashed in
  `lobby_character::place_decal_markers` (0x265F80, a null read at 0x265FE4). The modification
  window now opens only for an owned item (`LobbyMenu.openModifierWindow`).
- **The modification window opened from the shop** lists the inventory (what can go on the item)
  while it is open, and the shop again when it closes. It listed the shop's decals and camouflages:
  one dropped in a circle was not owned, the drop failed with an error and the drag stayed on the
  cursor. A drop of an item not owned is now ignored (`UpgradeWindow.onItemDropped`).
- **Every entry of a kit item's menu acts on that item** (live, rev 30, 2026-10-23: Modification
  on an equipped item from the shop was refused, "no such item"): Repair, Modification, Upgrade
  and Attachments act on the owned item the menu was opened on, that very copy, put on the card
  first (`InventoryList.menuItem`). They read the card or the list's selection, and in the shop
  the list holds only shop entries (item id 0): the card could be a shop entry again by the time
  an entry was chosen, and Repair and Upgrade used the list's selected shop entry (Upgrade did
  nothing). These entries are listed only for an owned item. The craft window (Upgrade) opened
  from the shop lists the inventory while it is open, as the modification window does.
- **The shop's own entries** (its list, its card) offer none of them, as before: Buy only.
- **Server:** a refused reroll window, reroll or repair logs the item id the client sent.
- Code: `InventoryList.set visible`, `InventoryList.showContextMenu`, `LobbyMenu.openCraftWindow`.
