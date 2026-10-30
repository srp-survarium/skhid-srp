# Esc in the lobby (0.26g lobby movie)

Owner 2026-10-23, after Esc over the Upgrade window's "more modifiers" message closed the whole
window: "we fixed that a few times already, but ... not from a single place".

## The rule

- **Esc closes the topmost modal only.** One press, one modal; the one under it stays as it was.
- **Every modal registers on open and leaves on close** (`survarium.controls.Modals`): `open(owner,
  layer, close, shown)` as it opens, `leave(owner)` as it closes. Nothing else decides what Esc
  closes.
- **Topmost:** a layer is over the ones below it whatever the order; in a layer, the modal opened
  last is on top. Layers, bottom to top:
  - `LOBBY`: the lobby's windows and what opens over them;
  - `MESSAGE`: message boxes (drawn over every window);
  - `MENU`: the right-click menu (drawn over messages).
- **`shown`** says whether the modal is up now. One that is not (the rooms window under the
  results, a message held back while the results are up, a window closed by a path that forgot to
  leave) is passed over, not dropped. A box forced over the results (a refused card's "ok") is
  shown: Esc closes it before the results.
- **A message's Esc is what its own close does** (`MessageBase.cancelByEsc`): its Cancel or X
  (`onNo` runs: the Upgrade window's donor with more modifiers leaves its slot), an OK box's OK
  (it tells the game). The tutorial-complete box only closes (its Cancel would repeat the
  tutorial). An invite (squad, clan, clan team) does what its hide button does: the box closes,
  nothing is sent (no decline) and the invite stays in the list.
- **Modals that keep Esc out stay:** a message not closable now (`closable` false: a reroll being
  paid), the reroll's choice of modifier (its X is off).
- **Text box first:** with no right-click menu open, Esc first takes the keys from a text box
  (the item search also empties, `item-search.md`, unless a message box is on top; the Upgrade
  window is not one); the next Esc closes the modal.
- **Nothing open** (no modal, no text box): Esc is the game's (its menu).
- **Transient windows first** (`transient-windows.md`: History, the match settings under Play,
  the character panel's presets list, a dropdown's open list): Esc closes the top one before anything else; they are not modals, and
  a modal opening closes them.
- **A match found or loading** (`set_mm_sync_state`, `set_loading_level`) closes the message up
  with `Message.close`, not as Esc: nothing is answered.

## The modals

| Modal | Layer | Registers | Esc |
|---|---|---|---|
| Top-menu window (Character, Inventory, Shop, Quests, Rankings, Clan), the results | LOBBY | `MainScene.activeWindow` setter | its `closeWindow` |
| Attachments window, ammo box | LOBBY | `MainScene.activePop` setter | its `closePopup` |
| Upgrade (craft) window | LOBBY | `LobbyMenu.openCraftWindow` | its close (`onCloseCraft`) |
| Tournament window, unfolded | LOBBY | `LobbyMenu.openTournamentWindow` | folds it |
| Friends list (F) | LOBBY | `LobbyMenu.friendsToggled` | hides it |
| Squad's player search | LOBBY | `SquadList.onItemClick` | closes it |
| Rooms window | LOBBY | `RoomsWindow.show` | hides it |
| Rooms join popup (loadout offer) | LOBBY | `RoomsWindow.takeOffer` | its Exit (no join) |
| Tutorial slides | LOBBY | `LobbyMenu.show_tutorial` | ends them |
| Message box (any type) | MESSAGE | `Message.addMessageToStage` | `cancelByEsc` |
| Right-click menu | MENU | `SrvContextMenu.show` | hides it |

## Where Esc arrives

- The exe, on every key, calls `root.has_active_window` (true: a transient window, a modal or a
  text box) and on Esc then `root.close_active_window` (`tools/re-kit/v026g/abi.tsv`, 0x26CB80).
- `LobbyMenu.handleInput` (CLIK) calls the same two; `_escapeHandled` makes one press act once.
- No other Esc handler: the messages', the friends list's, the squad search's, the player
  search's and CLIK's `DropdownMenu`'s own Esc cases are gone.
