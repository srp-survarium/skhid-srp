# The character menu in the shop

The owner (2026-10-22): "when you are in shop, your character profiles are shown on the left like
with inventory ... not character, but character menu ... with 3 profiles currently equipped".

- **Where:** the Shop tab. The character menu (the kits and what each has on) shows on the left, as
  on the Inventory tab. Before, the shop hid it.
- **What it does there:** what it does on the Inventory tab, and a shop item can be bought by a drop
  (the owner, 2026-10-29: "either normal buy, or drag into weapon slot buy"):
  - a shop item dropped on a slot it fits is bought into that slot after the usual buy dialog
    ("already purchased" first for an item owned), weapons, equipment and pocket items alike;
  - an item that slot held goes to the inventory (a helmet dropped on the helmet replaces it);
  - a pocket item already in another pocket is refused, one pocket an item type, and nothing is
    bought;
  - dropped anywhere else on the character, it does nothing (it was bought into a slot picked for it);
  - Buy and a double click buy into the inventory, as before.
- **How the drop buys:** a weapon or equipment goes as `relocate_item` (0x288D60) with item id 0
  and source slot 200, which sends the buy `47 00` with that target slot; the server buys and moves
  it into the slot (`rules::buy`). It never reached the game: `LobbyMenu.relocate_to_paperdoll`
  called the profile's relocate getter in place and Scaleform threw "is not a function"; it reads
  it first now. A pocket item goes as the ammo dialog's `46 00`, its row from slot 200: the server
  buys what is missing and moves it from the inventory (`rules::move_items`); it refused slot 200
  before ("Impossible to relocate the item").
- **A refused buy into a slot** (no money) gets the account sent again: the game locks the paper
  doll for the buy (`set_ui_element_lock_state` 0x229400), and only the account unlocks it.
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
- **Buy in a right-click menu buys the item the menu was opened on** (live, rev 39, 2026-10-30: "the
  Buy button in the inventory's context menu does not work"), put on the card first, as the other
  entries do (`InventoryList.buyMenuItem`). It bought what the list had selected (`buyElement`, now
  a double click's only), so it did nothing for a kit item in the shop unless a row the shop sells
  happened to be selected (the shop's list has no row of a kit item), and for a shop entry locked at
  its seller while another seller sells the item unlocked (Buy is listed by what the shop sells,
  `buyable`; the greyed row takes no selection). Checked by `scripts/inventory-menu/check.sh`.
- **The shop's own entries** (its list, its card) offer none of them, as before: Buy only.
- **Server:** a refused reroll window, reroll or repair logs the item id the client sent.
- Code: `InventoryList.set visible`, `InventoryList.showContextMenu`, `InventoryList.buyMenuItem`,
  `LobbyMenu.openCraftWindow`.
