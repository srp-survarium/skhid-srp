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
