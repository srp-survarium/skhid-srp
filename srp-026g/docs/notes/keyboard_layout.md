# Chat text follows the keyboard layout (patch 107)

- **Players' report (2026-11-05, Linux, niri):** the chat stays in one
  layout; Alt+Shift does not switch it.
- **The engine.** `keyboard::translate_text` (0x3B92D0) makes a key's text
  on the logic thread: `GetKeyboardLayout(0)`, then `MapVirtualKeyExA(dik,
  MAPVK_VSC_TO_VK_EX, hkl)` and `ToUnicodeEx(.., hkl)`. The window procedure
  (`message_processor`, 0x181E0, the main thread) passes a layout request
  (WM_INPUTLANGCHANGEREQUEST) on only while a text input has the keyboard
  (`survarium::game` slot 44h, 0x1EE7C0: its +586Ch, 1 in the login and the
  lobby, +1 while the chat is open in a match). It answers a layout change
  (WM_INPUTLANGCHANGE) with `ActivateKeyboardLayout(hkl, KLF_SETFORPROCESS)`,
  which gives the logic thread that layout too.
- **Wine** ignores KLF_SETFORPROCESS (win32u `NtUserActivateKeyboardLayout`
  sets only the calling thread's layout). The logic thread keeps the layout
  it started with, `MAKELONG(user locale, user locale)`.
  - X11 driver: no effect. Its `ToUnicodeEx` and `MapVirtualKeyEx` ignore
    the HKL; the text follows the X event's group.
  - Wayland driver (the launcher's choice under niri since #1240): the text
    comes from the tables of the HKL it is given. The logic thread's HKL
    matches none of the session's layouts, so the text is always the US
    tables'. Wine's driver asks for a new layout when the group changes
    (WM_INPUTLANGCHANGEREQUEST). It does not ask again for one the game
    dropped.
- **The patch** (Rust `keyboard_layout`, three lohooks):
  - `window.lang_request` (0x1835F) and `window.lang_change` (0x183E3)
    record the lParam HKL of every request and change, before the game's
    text-input check;
  - `keyboard.text_layout` (0x3B92E2) types with the recorded HKL, or with
    the thread's own until one is recorded.
  - One atomic: written on the main thread, read on the logic thread.
- **What changes:**
  - X11: nothing (the HKL is ignored).
  - Wayland: Alt+Shift switches the chat, in a match too.
  - Windows: a switch made outside a text input carries over to the next
    chat (it was dropped before).
  - Text only: no row in `simulation_changes.tsv`.
- **Not fixed here:** the X11 driver's start in a non-Latin layout. Its scan
  codes are made up from the layout active at the start (W/A/S/D 61h, 6Ch,
  6Dh, 6Eh), a winex11 bug (`tools/wine`).
- **Checked:** `check_keyboard_layout_026g.py` (both exes, Unicorn).
  **Not run in a game.**

# The console types the keyboard layout's text (patch 107)

- **The owner's report (2026-11-17):** in the `~` console the layout is not
  followed; players switch it in the chat first, and even then the console
  is inconsistent; on the owner's setup (Wine 11.8, Wayland driver) every
  key types `c`.
- **The engine.** The console's input is a `ui_text_edit`. Each key it types
  is an `insert_char_action` (`create_char_action` 0x34A4C0, made in
  `init_internals` 0x34A5E0 with a fixed character and its Shift one).
  `insert_char_action::execute` (0x34B4D0) never asks for the layout's text:
  - digits, the numeric keypad, Space: the fixed character;
  - letters and the punctuation keys (made "named"): the first letter of the
    key's DirectInput name (`get_dik_name` 0x3B9C00, `wcstombs_s` in the
    user's locale), lowercased, when that locale calls it a letter, else the
    fixed character.
  - Windows, Russian locale: Cyrillic letters, lowercase only, whatever was
    switched; another locale: Latin.
  - Wine's Wayland driver: the name is the key's keysym in its layout
    (`Cyrillic_tse`, `Cyrillic_a`, ...): `c` for every letter key of a
    Cyrillic layout.
  - The chat instead asks the keyboard's `translate_text` (0x3B92D0) for the
    key's text (`swf_input_translator::process_keyboard`, 0x1FDDE1), in the
    layout patch 107 follows.
  - The window procedure drops a layout request (Alt+Shift, Win+Space) unless
    a text input has the keyboard (`game+586Ch`: the login, the lobby, the
    chat); the console is not one. On Windows a switch in the console never
    happens.
- **The patch** (in `keyboard_layout`, four more lohooks):
  - `console.char_typed` (0x34B4DA, the start of execute): the action's key
    through its input world's keyboard's `translate_text`, the same call as
    the chat's, with patch 107's layout. A character the console font has is
    typed (its 256 glyphs are Windows-1251: ASCII, Russian, Ukrainian and
    Belarusian letters, №; not Ґ ґ). The text edit keeps any byte: its
    insert (0x348000) stores it as given, the font's glyph lookup (0x346200)
    reads it unsigned; nothing limits it to ASCII. A paste should take the
    same characters (`vostok::console::ConsoleChar::of`). Anything else (no character, a dead
    key, a control character, another script, a keyboard that is not the
    platform one): the game's own path, as before.
  - `console.opened` (0x1C77E) and `console.closed` (0x1C7D3), in
    `game_console::on_activate` and `on_deactivate`: one atomic, the console
    shown.
  - `window.lang_focus` (0x18365, the window procedure's call of the
    text-input check for a request): while the console is shown the request
    goes to DefWindowProc, as in the chat.
- **What changes:**
  - Windows: Alt+Shift switches the layout in the console; the console types
    the layout's text, Shift and Caps Lock included.
  - Wine, Wayland: the console types the layout's text; no more `c`.
  - Wine, X11: the text follows the X layout's group, as the chat's.
  - Text only: no row in `simulation_changes.tsv`.
- **Checked:** `check_keyboard_layout_026g.py` (both exes, Unicorn): execute
  for W, the name stubbed as Wine's Wayland driver gives it (the original
  types `c` in every layout, the patched exe W, then `ц`); a request passed
  while the console is shown, dropped once it is hidden. **Not run in a
  game.**

# The desktop's moved keys are moved in the game (patch 110)

- **The owner's report (2026-11-05, niri, `caps:swapescape`):** since
  revision 49 (#1240, Wine's Wayland driver) the Caps Lock key is no longer
  Esc in the game.
- **The engine.** The game's keys are DirectInput's buffered events, read in
  `keyboard::execute` (0x3B8D20): each event's DirectInput code is the key
  for the keys' states, the Shift/Ctrl/Alt flags and every handler.
- **Wine.**
  - X11 driver: a key's scan code and virtual key come from its keysym, so a
    key the desktop moved arrives moved.
  - Wayland driver: the scan code is the physical key's and the virtual key
    comes from a fixed QWERTY table of it, in DirectInput and WM_KEYDOWN
    alike. Only the layout's key names (each key's keysym: the Caps Lock key
    is `Escape`) and its text say what the desktop moved.
- **The patch** (Rust `key_remap`, one lohook `keyboard.events`, 0x3B914B,
  after GetDeviceData): each event's key among the 14 function keys (Esc,
  Backspace, Tab, Enter, both Ctrl, Shift, Alt and Windows keys, Caps Lock,
  Menu) becomes the function key its name says.
  - The names: DirectInput's (the game's get_dik_name, 0x3B9C00), in the
    layout patch 107 recorded, which the polling thread is lent while it
    reads them; read once a layout.
  - Letters and digits stay the physical keys, as DirectInput gives them on
    Windows: WASD stay WASD in every layout.
  - A key named nothing of the 14 (AltGr's `ISO_Level3_Shift`,
    `caps:hyper`'s `Hyper_L`) stays itself.
- **What changes:**
  - Wayland: `caps:swapescape`, `caps:escape`, `ctrl:nocaps`,
    `ctrl:swapcaps`, `altwin:swap_alt_win` and the like work in the game.
  - X11: nothing (its names are the moved keys' own, or `Control`).
  - Windows: nothing (no `wine_get_version` in ntdll: no name is read).
  - Before a layout is told (none before the window's first focus): nothing.
  - Each client's own keys: no row in `simulation_changes.tsv`.
- **Not fixed here:** Caps Lock's light for the text. Wine toggles its
  VK_CAPITAL with the physical Caps Lock key, which the desktop made Esc, so
  the chat's capitals may follow that key, not the desktop's Caps Lock.
- **Checked:** `check_key_remap_026g.py` (both exes, Unicorn).
  **Not run in a game.**
