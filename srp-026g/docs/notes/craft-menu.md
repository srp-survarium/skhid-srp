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
  shop's price, after the "already purchased" question for an item owned (the owner, 2026-11-01:
  only the inventory's right-click Buy skips it; `items.md` §6).
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
- **The loadouts' badge** (presets.md) moves with the item: the main and donor slots show it at
  their tile's bottom-left corner, as the list did (owner, rev 31: the numerals disappeared once
  an item was put in a slot).
- **Closed:** the inventory lists every item again, as before.
- Code: `MessageCraftItem.hideSlotted`, `InventoryList.hideCraftSlotted`, `LobbyMenu.openCraftWindow`.

## A failed upgrade

The owner's rule: 2026-10-23 ("remove chance for bonus removal during combination"; players called
the silver upgrade's 67.5 % chance to lose a modification too harsh).

- **An upgrade never takes a modification away**, whatever the item holds (reliability, a golden
  weapon's silver bonus, premium armour's bonuses too).
- **Success:** as before: one more modification, at the chances of `craft.options`.
- **Otherwise** (the data's "-1 mod." chance as well as its "nothing"): the price and the donor are
  spent, the item stays as it was; the window says "you failed to upgrade the item".
- **Prices, donors, success chances, silver and gold:** unchanged.
- **The window:** no failure ("-1 mod.") row; the server sends every failure chance as 0 (0x9B).
  The chances' panel, the prices under it and the window close up by the row's height (23 px), the
  window's top where it was; the result view is as before.
- Code: `items_rules::craft`, `wire::craft_tables`, `MessageCraftItem.removeFailureRow`.

## Buy from the slots

The owner's rule: 2026-10-23 ("in upgrade window it should be possible to click on donor /
upgradable item and press buy").

- **Before:** a click on either slot did nothing (the donor slot only drags), the footer is locked
  while the window is open (and shows Sell, not Buy, for an owned item), and the slots' items are
  left out of the list, so with one copy owned there was nothing to right-click: buying another
  meant closing the window.
- **Now:** a click (either button) on the item in the main slot or the donor slot gives **Buy**, as
  the list's Buy: a stock item of the kind at the shop's price, the game's buy dialog, nothing new on
  the server. The new copy shows in the window's list; the window and its slots stay as they were.
- **Only Buy** (no Sell: the slot's item is the craft's). An item the shop does not sell, or not to
  this account (golden, premium, drops, not unlocked): no menu, as in the list.
- **A drag** of the donor out of its slot (the press moved) gives no menu. Once the result shows,
  the slots are hidden: nothing.
- Code: `MessageCraftItem.onSlotDown`, `onSlotClick`; `InventoryList.showCraftSlotMenu`.
