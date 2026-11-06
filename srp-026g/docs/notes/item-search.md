# Item search in the item lists (Ctrl+F)

The rules the code follows. The owner, 2026-10-22: "we need search. pressing ctrl+F in any item menu
(shop/inventory) adds a small fill in box above buy (below) into which you can type the name of what
you search for and it will automatically update the shop/inventory for that, fuzzy search is also
nice to have".

Code, in `flash/lobby_menu/src` ([lobby-source.md](lobby-source.md)):

- `survarium/controls/InventoryList`: the box (`makeSearch`, `onSearchKey`, `closeSearch`,
  `applySearch`), its tip (`showSearchTip`, `SEARCH_TIP`) and the matching (`searchItems`,
  `searchQuery` into a `SearchQuery`, `searchAdmits`, `searchFactions`, `searchBonuses` with
  `SEARCH_BONUSES`, `searchForm`, `wordScore`, `spreadSpan`, `typoCount`);
- `survarium/data/PropertiesDescriptor.descriptions`: every bonus's card name;
- `LobbyMenu`: Esc (`close_active_window`), the window keys (`windowKeysOff`), the Ctrl keys
  (`onKeyDownStage`, `onKeyUpStage`);
- `KeyCode` (`F`, `LEFT_CONTROL`, `RIGHT_CONTROL`) and `MainScene.isCtrl`.

## The box

- **Where it works:** every item list. The Shop, the Inventory, and the item lists of the craft and
  upgrade windows are all the same list (`InventoryList`).
- **Always there** (owner 2026-10-22): the box shows whenever an item list does; there is no
  opening or closing it.
- **Typing goes to it:** a letter or a digit typed over an open item list gives the box the keys,
  and that character goes on at the end of its text. The keys of other layouts' letters
  (`; , . [ ] '` …) count too, and "+" (the row's and the pad's: a "+" word).
- **Ctrl+F** gives the box the keys with its text selected.
- **A click** on the box or its magnifier gives it the keys.
- **When typing and Ctrl+F do nothing:**
  - over a message;
  - over a popup, except the upgrade window;
  - while another text box (the chat, a rename) has the keys. The keys then belong to that box.
- **Where it sits:**
  - in its own row under the item grid, just above the item panel's buttons (Buy, Sell);
  - centred on the list;
  - the grid is 40 px shorter for that row, and the fade at its foot moves up with it.
- **What it looks like:**
  - the friends window's name box, 1.25 times as wide and 4 px taller with a 4 px bigger font
    (332 × 32), with a magnifier at its right (drawn: the friends search's is part of its
    window's art). No × (owner 2026-10-22): Esc empties it;
  - when empty and without the keys, it reads "Search" and three "+" examples, two bonuses and
    a level (owner 2026-11-06: the box telegraphs the syntax, bonuses first): "Search: +ammo,
    +pocket, +10" / "Поиск: +патрон, +карман, +10" / "Пошук: +набої, +кишень, +10" …, each
    language's own words for bonuses its cards name, on one line inside the box (the box is not
    wider for it); upright:
    every CLIK text box's hint is upright now (`TextInput.initialize`), as the lobby's Calibri
    has no italic and the game takes "Calibri Bold Italic" from the system's fonts, so under Wine
    without Calibri the hints (the friends search's too) showed as squares.
- **The tip** (owner 2026-11-06): while the box is empty, hovering it or its magnifier shows the
  game's tooltip (the one of the shop's "Hide owned", a line each, `ReasonsTip`) of all the
  search takes, one short line each with an example, bonuses first:
  - `+ammo +pocket`: has each bonus named;
  - `+10 +11`: of any of these levels;
  - `+army`: sold by that faction (several: any);
  - `ak 74`: in the name, typos forgiven;
  - `helmet, boots`: that kind of item;
  - `ak +10 +ammo`: all of these at once.

  Once something is typed there is no tip; a click on the empty box is a click on the box. Each
  line is a dictionary key (`st_srp_inventory_search_tip_*`), its examples the language's own
  words that find what they say.
- **Every resolution:** the box is part of the list, and the lobby scales the list (4K: #279).

## What it finds

- **Live:** the list changes with each key typed.
- **With the list's own filters:** the category tab, the filter icons, the seller and the craft
  window's donors still apply. The search narrows what they list.
- **Order:** best matches first. Among equal matches, the list's own order (level, category, rank
  and so on: `InventoryList.LIST_ORDER`; the list has no sort to choose, shop-owned-mark.md).
- **By the name shown, in the player's language.** There is no internal name to search: the
  exe's item dictionary gives the movie only the translated `name` and `descr`
  (`fill_items_dictionary_impl`: the keys it sets, 0x2866B1..0x28761D).
- **How letters compare:**
  - case does not count;
  - accents do not count (ä = a, ł = l, ó = o, ß = ss, ё = е, ї = і, є = е, ґ = г, й = и);
  - Cyrillic counts as Latin letters, so "ak" finds АК-74М, "мосин" finds Mosin-Nagant, and a
    Ukrainian name finds its Russian spelling;
  - spaces and punctuation do not count, so "ak74" finds AK-74M.
- **A number first is a level** (owner 2026-10-22): "15" or "15 ak" lists the items of
  equipment level 15 first, then the names with 15 in them ("74" still finds the AK-74). Only the
  first word; a number later is a part of the name.
- **A word naming a kind** (owner 2026-10-22), or its start from 3 letters on, lists the items of
  that kind first, whatever their names: gear / equipment (all equipment), items, helmet, mask,
  body / armour / vest, rig, gloves, pants / trousers, boots; in Ukrainian and Russian too
  (шолом, шлем, маска, бронежилет, рукавички, перчатки, штани, черевики, …).
- **Every word typed must match.** "ak 74" lists only names with both.
- **How a word matches, best first:**
  1. at the name's start;
  2. at the start of a word of the name;
  3. inside the name;
  4. a word of 3 or more letters: its letters in order, with others between (at most 3 × its
     length in all; closer is better). "vntrz" finds Vintorez;
  5. a word of 4 or more letters: with one typo (a letter wrong, missing, extra, or two swapped);
     a word of 8 or more: with two. "mosni" finds Mosin. Each typo makes the match worse.
- **The loose matches (4 and 5) only when nothing else matches** (owner 2026-10-22: "mask"
  listed Master's gloves, a typo of "mast"). An item matching every word otherwise (a kind, a
  level, the name) hides every item that needs a loose match.
- **The item you chose** stays chosen while it is still listed. The list scrolls back to the top
  each time the search changes.

## "+" words: levels, factions, bonuses

Players, 2026-11-06: "filter by tier level, faction, and perk", "+ammo, +pocket, +slot, +artifact";
the owner: "levels should also become +1 +10". A word starting with "+" (to the next space or "+":
`+10+11` is two) is a filter. The text is read once into levels, factions, bonuses and the name's
words (`SearchQuery`).

- **`+<number>` is an item level** (the card's equipment level), anywhere in the text.
- **`+<word>` names a faction** when a seller tab's name (as the lobby shows it) has a word starting
  with it: `+army`, `+black`, `+scav`, `+independent`. A word naming a faction is a faction, never a
  bonus.
- **Any other `+<word>` names a bonus** (owner 2026-11-06: what players search for most), when a
  word of the bonus starts with it, letters compared as in names (case, accents, Cyrillic as Latin):
  - by its name as the card shows it, in the game's language (`+патрон`: Подсумки для патронов);
  - and in every language by English words: the bonus's English name and the words players use
    (`SEARCH_BONUSES`): `+ammo`, `+pocket`, `+slot`, `+artifact` and `+artefact`, `+weight`,
    `+stamina`, `+aim`, `+recoil`, `+armor` and `+armour`, `+reload`, `+regen`, `+xp`, …;
  - every bonus the card lists: rolled, built-in, the default modifications, the shop's items'
    (theirs are built-in only). A defect (the card's red line) is no bonus.
- **Several of a kind:**
  - levels: any of them (`+10 +11` lists both);
  - factions: any of them (an item has one seller);
  - bonuses: all of them (`+ammo +pocket`: both on the item);
  - name words: all of them, as before.
- **The kinds combine with "and":** `ak +10 +11 +ammo` is a name matching ak, of level 10 or 11, with
  an ammo bonus.
- **A "+" word is a filter, never fuzzy:** no loose or typo match (`+amo` is not ammo). A `+word`
  naming nothing lists nothing. A "+" alone is nothing typed.
- **The rest stays:** the name words rank as before (a plain number first is a level first), the
  list's own filters, tabs and seller still apply, and with filters only the list keeps its own
  order.
- **Checked** by `scripts/item-search/check.sh <lobby_menu.swf>` (Ruffle, the movie's own list):
  the rules above, and in every language the hint on one line in the box and each of its and the
  tip's examples finding what they name.

## Closing, Esc, Enter, the other keys

- **The search is emptied, and the list shows everything again,** after:
  - Esc while the box has the keys (the box then also gives the keys back);
  - the list closing.
- **Switching Shop ↔ Inventory clears the search.** The game closes the one list and opens it
  again. A clear box when the window changes is the less surprising of the two choices.
- **Category tabs and filters keep the search.**
- **Esc order** (`esc.md`: Esc closes the topmost modal; a text box gives the keys back first):
  1. a right-click menu closes first;
  2. then Esc empties the box and leaves it;
  3. over a message or a popup, Esc only takes the keys from the box, and the message closes on
     the next Esc, as with the game's other text boxes.
- **Esc while the box does not have the keys:** as before (it closes the window, and that clears
  the search).
- **Esc and the friends list:** see `esc.md`.
- **Ctrl+W in the box** (owner 2026-10-22) deletes the word before the caret; **Ctrl+U**
  everything before the caret.
- **Enter in the box:** gives the keys back to the lobby. The results stay. Enter is only the box's
  own key: the box takes no Enter or Esc meant for other windows.
- **The game's keys while typing:** none fire. The exe asks `root.is_typing` before its lobby
  window keys, and the box is an input text field.
- **The window keys do nothing over an item list.** The exe sends a key's window action before the
  movie sees the key (F is Friends by default: `user.cfg` `bind friends kF`), so while an item
  list takes typing, the window keys (character, friends, shop, inventory) do nothing and their
  letters go to the search (`LobbyMenu.windowKeysOff`). Esc still closes the windows.

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
  - the "+" words over the live shop and inventory (the real bonus names and sellers);
  - the hint's fit with the game's font, and the tip over the box.
