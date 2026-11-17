# Paste into the console (0.26g)

The owner, 2026-11-17: "let's add ctrl+v to console".

## The rule

- **Ctrl+V** in the console's line (`~`) pastes the clipboard's text at the caret, as if typed.
  **Shift+Insert** does the same: the game binds both keys to the one paste.
- **The first line only.** A line's end (CR, LF), a tab or another control character, or a line or
  paragraph separator (U+2028, U+2029) ends the paste. A paste never runs a command: Enter does.
- **The console's characters only:** printable ASCII (what its keys type). Any other character
  (Cyrillic, accented letters, emoji, a no-break space) is left out; the rest goes in.
- **The line's limit:** 255 characters (the line's max length). What does not fit is cut; what is
  already on the line stays.
- **A selection** is replaced: deleted first, as a typed character deletes it.
- **Nothing to paste** (no text on the clipboard, another program holding it, an empty first line, a
  full line): nothing changes, the selection stays.
- **One undo step** (Ctrl+Z) takes the paste back. Held down, Ctrl+V repeats like a typed key.
- **Every other key as before:** V types v, Ctrl+C, Ctrl+X, Ctrl+A, Ctrl+Z, Ctrl+Shift+V unchanged
  (copy and cut stay the engine's "not implemented").

## How

- Exe patch 157 (`client-patches/rust/patches/src/console_paste.rs`, `client_patch.md` section
  3.83, `check_console_paste_026g.py`): the engine's own `ui_text_edit::paste_from_clipboard`, a
  stub that only logged "not implemented", now pastes. Nothing a match computes changes.
- Not tried in a game: Wine's clipboard (the X11 and Wayland drivers) is the desktop's.
