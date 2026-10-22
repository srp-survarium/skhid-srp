# Buy and Sell in the craft window

The rules the code follows. Code: `InventoryList` (`showCraftMenu`, `findCraftWindow`,
`buyFromCraftMenu`) and `MessageCraftItem` (`offerContinue`, `onContinue`, `holds`) in
`flash/lobby_menu/src/survarium` ([lobby-source.md](lobby-source.md)); README "The craft window: Buy
and Sell, Continue and Exit". The owner's rule:
2026-10-20 ("rmb on bought item -> upgrade -NEW-> rmb on any item in the list -> buy/sell (NOT
upgrade)").

- **Where:** the craft window (right-click an owned item → **Upgrade**: two items of a kind make a
  better one). Its list shows the items that can go in (copies of the same item).
- **What:** right-click on an item of that list gives **Buy** and **Sell**, nothing else (no
  Upgrade, Wear, Fix, Modification, Appearance). Before, it gave no menu: the window locks the list.
- **Buy:** as in the inventory: listed when the shop sells the item and the account has unlocked it;
  for an owned item whatever its modifications (`buy-modified.md`). It buys a stock item at the
  shop's price, without the "already purchased" question.
- **Sell:** an owned item, not in the shop's list, and not the item being upgraded or the donor
  the window holds (the craft would name an item no longer owned); the game's sell dialog asks
  first.
- **Elsewhere:** the inventory's right-click menu is as before; a list locked for another reason
  (another window) still gives no menu.
- **Server:** nothing changes.

## After an upgrade: Continue and Exit

The owner's rule: 2026-10-20 ("instead it should be 'Exit' and 'Continue', Exit does what Ready does
while Continue keeps you in the same 'Upgrade' window").

- **When:** the craft window shows its result (better, worse, or unchanged). It had one button,
  **Ready**, which closed it.
- **Exit:** closes the window, as Ready did (the item stays selected in the inventory).
- **Continue:** the Upgrade window again, for the same item as it is now (upgraded or not), with
  the donor slot empty: drop another copy and upgrade again.
- **Continue on an item that cannot be upgraded any more** (three modifications, or not craftable):
  it closes, as Exit.
- **Order:** Continue left, Exit right, as the game's own dialogs put the accepting button left
  and Cancel right (owner 2026-10-20: do as the other menus).
- **A refused craft** (`cannot_craft_item`): the window closes by itself, as before; no buttons.
- **Words:** the game's own "Exit" (`btn_exit_label`) and "Continue" (`btn_go_to_online_store`, the
  store dialog's Continue), in every game language.

## Comparing items

The owner's rule: 2026-10-22 ("comparing items in upgrade window shouldn't compare equipped to what
is there, but instead upgradable item to a what will be the donor; if no upgradable item is set,
then pop up doesn't show up").

- **When:** the "Comparing items" checkbox is on and the craft (Upgrade) window is open.
- **Hovering an item of the list** compares it with the item being upgraded (the main slot), not
  the equipped one: its card beside the hovered one, and the hovered card's bars against it.
- **Both cards are coloured**, each against the other: a better value green, a worse one red, on
  the item being upgraded's card too (owner 2026-10-22: "when hovering the relative changes should
  be green <-> red"). So is the equipped item's card when an inventory item is compared with it
  (owner 2026-10-22: "we should do the same thing for comparisons in character <-> inventory").
- **No item being upgraded** (the main slot empty): no compare card, the hovered card alone.
- **The item being upgraded itself** in the list: no compare card (nothing to compare).
- **After an upgrade** (the result shows): compared with the upgraded item.
- **Hovering the window's donor slot** shows the donor's card compared with the item being upgraded;
  **hovering the main slot**, the item being upgraded compared with the donor (owner 2026-10-22:
  "when hovering in upgrade window itself, it should still compare donor to receiver"). The other
  slot empty: the card alone. Once the result shows: no card on the slots, as before.
- **The item being upgraded is always the left card** of the two (owner 2026-10-22: "the upgradable
  one should always be on left"), whichever slot is hovered; the list's compare already had it there.
- **Elsewhere** (the window closed): compared with the equipped item, as before.
- Code: `MessageCraftItem.compareWith`, `LobbyMenu.showItemHint` (and `updateHintItem`, which leaves
  the compare card alone while the window is open).

## Items in the slots

The owner's rule: 2026-10-22 ("once you move an item to donor/receiver, it should disappear from
the inventory ... it still is in inventory, it is just a visual thing ... to know what is moved and
what isn't").

- **While the craft window is open:** its list leaves out the item in the main slot and the one in
  the donor slot. Taken out of a slot, an item is listed again.
- **Items with three modifications** (rank 3) are not listed either: neither slot takes them (owner
  2026-10-22: "items with 3 mods shouldn't show up in craft too").
- **Only the list:** the items stay owned; nothing is sent to the server.
- **Closed:** the inventory lists every item again, as before.
- Code: `MessageCraftItem.hideSlotted`, `InventoryList.hideCraftSlotted`, `LobbyMenu.openCraftWindow`.
