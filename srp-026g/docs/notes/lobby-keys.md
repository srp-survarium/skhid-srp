# Lobby keys (0.26g lobby movie)

Owner 2026-10-22. Keys the lobby movie takes itself (`LobbyMenu.lobbyKey`, `RoomsList.onKey`,
`InventoryList.onSearchKey`); the item search has its own spec (`item-search.md`).

- **R** opens the rooms window, or closes it (as a click on the Rooms tab; not while the tab is
  off; once per press). Over an item list R types into its search instead.
- **Up / Down** in the rooms list choose the room above or below; the list scrolls to it.
- **Enter** in the rooms list joins the room chosen.
- **Enter** on the lobby's main screen presses Play:
  - only with no window open (no shop, equipment, rooms, craft, friends list, squad search,
    tournament, …) and not in a room or a room's queue;
  - only while Play reads PLAY (not Leave or Ready), and it is on;
  - once per press: a held Enter does not press it again (that would leave the queue).
- **Alt** (left or right) held over a weapon's card shows its stats in a match, with the kit and
  the skills (`alt-weapon-stats.md`); let go, the weapon's own stats.
- **Esc** closes the topmost modal only (the friends list (F) is one): `esc.md`.
- **None of them** while a text box has the keys, a message, popup, right-click menu or the
  results are up, or with Ctrl or Alt held.
- **Typing over an item list** (shop, equipment, craft, upgrade) starts the item search; the
  exe's window keys (character, inventory, shop, friends) do nothing there, as their letters are
  the search's. Esc still closes the windows.
