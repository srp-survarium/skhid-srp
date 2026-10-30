# Transient windows in the lobby (0.26g lobby movie)

The owner, 2026-10-30, on History staying open under a newly opened window and the PvP / bots
list under Play opening behind the rooms window: "those are transient windows ... just like in
UI on Windows, or in a browser".

## The rule

A **transient window** is a pop-up that lives only until the player turns to something else:

- **History** (the bottom bar's button: match history and replays);
- **the match settings under Play** (the arrow beside "PvP": PvP, clan, bot matches);
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
- **A resize closes them.**

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
- Over the windows: the lobby root's child holding the transient goes to the top of the root while
  it is open, and back under the window it was under when it closes (History is the root's child
  itself, the match settings the top bar's). A dropdown's list is on CLIK's popup canvas, already
  over every window.
- Gone with it: History's own raise on open, the top bar's stage click that closed the match
  settings, their bot-match dropdowns' press guard (`pvbPopupDown`), the CLIK dropdown's own stage
  press and Esc handlers, and `has_active_window`'s dropdown case.

## Checks

`srp-026g/scripts/transient-windows/check.sh <lobby_menu.swf>` (Ruffle, CPU): History closed by the
shop, the rooms window and a press on Settings, opened again, kept with nothing else opened; one at
a time; a press outside closes and still arrives; Esc, the top one first; the match settings over
the rooms window and taking the press there, the top bar back under it when they close; a resize.
