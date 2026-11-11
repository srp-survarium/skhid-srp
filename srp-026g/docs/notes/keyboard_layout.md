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
