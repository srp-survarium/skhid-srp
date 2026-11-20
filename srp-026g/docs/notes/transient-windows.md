# Transient windows in the lobby (0.26g lobby movie)

The owner, 2026-10-30, on History staying open under a newly opened window and the PvP / bots
list under Play opening behind the rooms window: "those are transient windows ... just like in
UI on Windows, or in a browser".

## The rule

A **transient window** is a pop-up that lives only until the player turns to something else:

- **History** (the bottom bar's button: match history and replays);
- **the match settings under Play** (the arrow beside "PvP": PvP, clan, bot matches);
- **the character panel's presets list** (its title: the five presets);
- **a dropdown's open list** (every CLIK `DropdownMenu` of the lobby: the bot-match size, mode and
  map, the premium boxes').

They behave like an OS's or a browser's menus:

- **One at a time.** Opening one closes the others, except the one it is opened from: a dropdown
  inside the match settings keeps them open, History closes them.
- **A press outside closes it**, and the press still goes on to what it is on: a press on Shop
  closes History and opens the shop. A press inside it, or on its own button, is its own (the
  button's click closes it). A press inside the match settings but outside their open dropdown
  closes only the dropdown.
- **Esc closes it**, the top one first (the dropdown, then the match settings), before anything
  else Esc does (`esc.md`).
- **Any regular window opening closes them all**: a top-menu window, the rooms window, a message,
  a popup, the right-click menu, anything that enters `Modals`. Settings (the game's own window)
  is opened by a press on its button, which is outside.
- **It is drawn over every regular window while open.** Closed, everything is where it was.
- **A resize closes them**, and so do the screens over the lobby that are not windows: the match
  loading (`set_loading_level`) and the sync clip (`show_sync`). A transient's owner hidden closes
  it too (the character panel's: its `HIDE`).

Regular windows (shop, inventory, character, rooms, messages, Settings...) are not transient: they
stay until closed, and Esc closes them one at a time (`esc.md`).

## How it is made

- `survarium.controls.Transient` is the one place. A transient enters with
  `Transient.open(view, opener, close)` as it opens and leaves with `Transient.closed(view)` as it
  closes by its own path; `close` is what its own button does. Nothing else closes a transient on
  a press or a key.
- The open ones are a chain, each opened from the one before (its `opener` inside it). A press
  (the stage's `MOUSE_DOWN`, capture phase, never stopped) closes the top ones it is not in;
  opening one does the same from its opener.
- `Modals.open` (every regular window: `esc.md`) calls `Transient.closeAll`.
- `LobbyMenu.has_active_window` is true while one is open; `close_active_window` calls
  `Transient.escape` (the top one) before the text box and the modals.
- Over the windows: an open transient is carried onto the **transient layer**
  (`LobbyMenu._transients`, in the stage layer over `_layers`, under the tooltips and item cards),
  in a holder drawn as its parent is, so it stays where it was and as big; it goes back to its
  place among its parent's children as it closes. Only the transient moves: the top bar stays
  under the rooms window while the match settings are over it. CLIK's popup canvas (a dropdown's
  list) is on the same layer, so a dropdown opened from the match settings is over them.
- A press Scaleform gives the holder itself (a control straight in a sprite gives the sprite the
  press: `scaleform-movie-pitfalls`) goes on to the transient as a press at the same point, so its
  own press handling finds what is there (the presets list, its row).
- A transient's code does not look at its parent while open: the match settings are told the top
  bar's Play box, History the lobby.
- Gone with it: History's own raise on open, the top bar's stage click that closed the match
  settings, their bot-match dropdowns' press guard (`pvbPopupDown`), the CLIK dropdown's own stage
  press and Esc handlers, `has_active_window`'s dropdown case, and the presets list's stage press,
  per-frame watcher and `Modals` entry.

## Checks

`srp-026g/scripts/transient-windows/check.sh <lobby_menu.swf>` (Ruffle, CPU): History closed by the
shop, the rooms window and a press on Settings, opened again, kept with nothing else opened; one at
a time; a press outside closes and still arrives; Esc, the top one first; the match settings over
the rooms window and taking the press there, no part of the top bar over the rooms window, the
settings back in the bar when they close; the presets list (press outside, Esc, a window, the
panel hidden); a resize.
