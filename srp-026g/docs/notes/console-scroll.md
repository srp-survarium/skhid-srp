# Scrolling the console (0.26g)

The owner, 2026-11-17: "I also wish scrolling in console was supported".

## The rule

- **The mouse wheel** moves the console's output three lines a notch (a smaller turn, one line).
- **Page Up / Page Down** move it a page: the lines the console shows, less one, so one line stays
  in sight.
- **End** (Ctrl+End too) and **Enter** go back to the newest line; so does **opening the console**.
  End still moves the line's caret to its end, as before.
- **Scrolled up, the view stays where it is** while new lines come in. Scrolled (or paged) down to
  the newest line, it follows the new lines again.
- **The history is the whole log of this run**: every line the console has shown since the game
  started (the game's own log, `Survarium_<user>.log`, which the console has always read). The top
  stops at its first line.
- **Nothing reaches the game** while the console is open: it takes every key and every mouse move
  (wheel included) before the game's handlers, as it always did. No weapon switch, no camera move.
- A key held down does not repeat the scroll (press again).

## How

- Exe patch 159 (`client-patches/rust/patches/src/console_scroll.rs`, `client_patch.md` section
  3.85, `check_console_scroll_026g.py`, `tools/check console-scroll`). The engine's console already
  draws its lines through a scroll view, whose "follow the last line" flag the stock console never
  clears; the patch moves that view and sets or clears the flag, on the console's own keys
  (`console_impl::on_keyboard_action`), its mouse moves (`game_console::on_mouse_move`) and its
  opening (`game_console::on_activate`). Nothing a match computes changes.
- Not tried in a game.
