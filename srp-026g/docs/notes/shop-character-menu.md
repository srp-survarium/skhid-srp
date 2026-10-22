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
- **Nothing else changes:** the shop's list and its item card are as before. Server: nothing.
- Code: `InventoryList.set visible`.
