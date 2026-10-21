# Item search in the item lists (Ctrl+F)

The rules the code follows. The owner, 2026-10-22: "we need search. pressing ctrl+F in any item menu
(shop/inventory) adds a small fill in box above buy (below) into which you can type the name of what
you search for and it will automatically update the shop/inventory for that, fuzzy search is also
nice to have".

Code, in `flash/lobby_menu/src` ([lobby-source.md](lobby-source.md)):

- `survarium/controls/InventoryList`: the box (`makeSearch`, `onSearchKey`, `closeSearch`,
  `applySearch`) and the matching (`searchItems`, `searchForm`, `wordScore`, `spreadSpan`,
  `typoCount`);
- `LobbyMenu`: Esc (`close_active_window`), the window keys (`windowKeysOff`), the Ctrl keys
  (`onKeyDownStage`, `onKeyUpStage`);
- `KeyCode` (`F`, `LEFT_CONTROL`, `RIGHT_CONTROL`) and `MainScene.isCtrl`.

## The box

- **Where it works:** every item list. The Shop, the Inventory, and the item lists of the craft and
  upgrade windows are all the same list (`InventoryList`).
- **Ctrl+F opens it.** The box takes the keys, ready to type.
- **When Ctrl+F does nothing:**
  - over a message;
  - over a popup, except the upgrade window;
  - while another text box (the chat, a rename) has the keys. Ctrl+F then belongs to that box.
- **Ctrl+F again** clears and hides the box. If the box is showing but you clicked away, Ctrl+F
  gives it the keys again, with its text selected.
- **Where it sits:**
  - below the item grid, just above the item panel's buttons (Buy, Sell);
  - centred on the list.
  - The item grid gets 34 px shorter while the box shows, and the fade at the grid's foot moves up
    with it. The box covers nothing.
- **What it looks like:**
  - the friends window's name box (266 × 28), with the player search's × in it (× clears and
    hides it);
  - when empty and without the keys, it reads Search / Пошук / Поиск / Suche / Szukaj.
- **Every resolution:** the box is part of the list, and the lobby scales the list (4K: #279).

## What it finds

- **Live:** the list changes with each key typed.
- **With the list's own filters:** the category tab, the filter icons, the seller, the sort, and the
  craft window's donors still apply. The search narrows what they list.
- **Order:** best matches first. Among equal matches, the list's own order (price, category,
  rank and so on).
- **By the name shown, in the player's language.** There is no internal name to search: the
  exe's item dictionary gives the movie only the translated `name` and `descr`
  (`fill_items_dictionary_impl`: the keys it sets, 0x2866B1..0x28761D).
- **How letters compare:**
  - case does not count;
  - accents do not count (ä = a, ł = l, ó = o, ß = ss, ё = е, ї = і, є = е, ґ = г, й = и);
  - Cyrillic counts as Latin letters, so "ak" finds АК-74М, "мосин" finds Mosin-Nagant, and a
    Ukrainian name finds its Russian spelling;
  - spaces and punctuation do not count, so "ak74" finds AK-74M.
- **Every word typed must match.** "ak 74" lists only names with both.
- **How a word matches, best first:**
  1. at the name's start;
  2. at the start of a word of the name;
  3. inside the name;
  4. a word of 3 or more letters: its letters in order, with others between (at most 3 × its
     length in all; closer is better). "vntrz" finds Vintorez;
  5. a word of 4 or more letters: with one typo (a letter wrong, missing, extra, or two swapped);
     a word of 8 or more: with two. "mosni" finds Mosin. Each typo makes the match worse.
- **The item you chose** stays chosen while it is still listed. The list scrolls back to the top
  each time the search changes.

## Closing, Esc, Enter, the other keys

- **The search goes away, emptied, and the list shows everything again** after:
  - Esc while the box has the keys;
  - Ctrl+F while the box has the keys;
  - its ×;
  - the list closing.
- **Switching Shop ↔ Inventory clears the search.** The game closes the one list and opens it
  again. A clear box when the window changes is the less surprising of the two choices.
- **Category tabs, filters and the sort keep the search.**
- **Esc order** (the order of 2026-10-21: menus and messages close first):
  1. a right-click menu closes first;
  2. then Esc clears and hides the box;
  3. over a message or a popup, Esc only takes the keys from the box, and the message closes on
     the next Esc, as with the game's other text boxes.
- **Esc while the box is showing but does not have the keys:** as before (it closes the window,
  and that clears the search).
- **Enter in the box:** gives the keys back to the lobby. The results stay. Enter is only the box's
  own key: the box takes no Enter or Esc meant for other windows.
- **The game's keys while typing:** none fire. The exe asks `root.is_typing` before its lobby
  window keys, and the box is an input text field.
- **Ctrl+F does not open Friends.** The exe sends a key's window action before the movie sees the
  key, and F is Friends by default (`user.cfg`: `bind friends kF`). So with Ctrl held over an open
  item list, the window keys (character, friends, shop, inventory) do nothing.

## How Ctrl+F reaches the movie (0.26g exe)

- **The lobby's key handler, 0x26CB80:**
  - Esc: `root.has_active_window`, then `root.close_active_window`;
  - otherwise `root.is_typing`;
  - if not typing, the window keys by bind action (0x40 character, 0x41 inventory, 0x42 shop, 0x43
    settings, 0x44 friends);
  - then it passes the key to the movie, 0x1FDD70.
- **The key the movie gets:**
  - The DIK key becomes a Windows VK code by the table built at 0x1FDF20: F → 70 (0x1FE443), left
    Ctrl → 162 (0x1FE8A2), right Ctrl → 163 (0x1FE87E).
  - 0x2AF680 sends GFx a key event with the exe's Shift, Ctrl and Alt bits, so `ctrlKey` is set.
    LIKELY: the bits come from the exe's keyboard state.
  - Then comes the character, as a separate event.
- **What the lobby checks for Ctrl:** `KeyboardEvent.ctrlKey`, or its own record of the Ctrl keys
  (`MainScene.isCtrl`, set by key 162/163 down and up).
- **The box takes the keys on the next render.** The F's character has gone by then (the chat's
  box does the same: `LobbyChatTab.onRenderFocus`), so no "f" is typed into it.

## Release

- **Release:** the lobby movie only (a content patch).
- **Server:** nothing changes.
- **Not yet checked in the game:**
  - the exe's `ctrlKey`;
  - the box's look and place;
  - that no "f" is typed;
  - the speed of typing over the full shop.
