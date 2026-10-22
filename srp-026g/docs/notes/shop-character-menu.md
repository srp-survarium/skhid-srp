# The character menu in the shop

The owner (2026-10-22): "when you are in shop, your character profiles are shown on the left like
with inventory ... not character, but character menu ... with 3 profiles currently equipped".

- **Where:** the Shop tab. The character menu (the kits and what each has on) shows on the left, as
  on the Inventory tab. Before, the shop hid it.
- **What it does there:** what it does on the Inventory tab. A shop item dropped on one of its slots
  is bought into that slot, as the game already did (`DragRegion`, `MainScene.buyFunction`).
- **Nothing else changes:** the shop's list and its item card are as before. Server: nothing.
- Code: `InventoryList.set visible`.
