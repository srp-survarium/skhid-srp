#!/usr/bin/env python3
"""Runs the "Relay" row of the patched launcher (patch 16,
launcher_values_026g.asm "Relay") under Unicorn, through the sites the patch
changed, at its own base and rebased, with Windows' functions and the
launcher's helpers stubbed (check_torrent_box_026g's).

Usage: python check_relay_box_026g.py ORIGINAL_survarium_launcher.exe

Checks:
- message 0A (on_updater_message's dispatch, 0x15BAC, to the handlers' end
  0x16093, the stack and the launcher's registers as they were): a view of
  1..6 is kept with the launcher; the button (VK's control) is shown
  (ShowWindow(SW_SHOWNA)), its pictures left as they are; refresh_status
  runs once (the panel and its text anew); 0, 7 and 255 change nothing and
  show nothing;
- the tooltip: made once for the button's window (comctl32 loaded,
  InitCommonControlsEx with ICC_BAR_CLASSES, a topmost tooltips_class32
  popup owned by the button's parent, 360 pixels wide, 30 s), the button its
  tool (TTF_IDISHWND | TTF_SUBCLASS), its text what the relay is and the
  view's line in the launcher's language; another view updates the text in
  place; a button made again (the window made again in another language)
  destroys the old tooltip and makes one for the new button;
- the button's making (0x1AEA8..0x1AEEA): VK's control at 28, 46, 20 by 20
  in the bottom panel, with its three pictures (the gear's), its click
  0x1BD3C; hidden (SW_HIDE) until a view came, else shown;
- the bottom panel's text: none while hidden; "Relay: auto" (1 and 4),
  "Relay: auto (off)" (2), "Relay: auto (on)" (3), "Relay: on" (5), "Relay:
  off" (6), 25 pixels right of the button and 1 below its top, in the
  launcher's language (Russian, Ukrainian, German, Polish; English for the
  game's other languages);
- a click (the button's window procedure, WM_LBUTTONDOWN): a menu of the
  three in the launcher's language, the chosen one checked (auto for 1..4),
  under the button; a pick is 22h, 23h or 24h to the updater
  (send_simple_message), no pick nothing; no menu while no updater is
  connected or no view came;
- the status line of the launcher's states 6 and 7 (refresh_status's
  load_string call, 0x1687C to 0x16881): "Server maintenance..." (0B2h)
  while the updater counts the seconds to its next try (message 13h's byte,
  launcher+2BA0h) is "The server is restarting. Next try in <n> s." in the
  launcher's language, 1, 5, 60 and 255 seconds; with no count (0), and for
  state 6's text (87h) whatever the count, load_string's own; the stack and
  the registers the caller keeps as they were.
"""
from pathlib import Path
import struct
import sys
from unicorn.x86_const import UC_X86_REG_ESI
import check_language_026g as C
import check_torrent_box_026g as T
import patch_launcher_026g as L

HERE = Path(__file__).resolve().parent
FORM, LAUNCHER, FRAME = T.FORM, T.LAUNCHER, T.FRAME
RELAY = 0x6C0
SLOTS, PNGS = T.RELAY_SLOTS, T.RELAY_PNGS     # the gear: its picture, under the pointer, pressed
STATE, VIEW, TIP, TIP_TOOL = 0x4A800, 0x4A804, 0x4A808, 0x4A80C
LANGUAGE_GLOBAL = 0x4819C
BOX_WINDOW, PARENT, TOOLTIP = 0x991, 0x445, 0x5551
RECT = (28, 46, 48, 66)
BUTTON_RECT = (300, 600, 320, 620)      # the button's window on the screen
# The row's text by the view: auto (1 probing, 4 no answer), auto (off), auto (on), on, off.
LABELS = {'en': ('Relay: auto', 'Relay: auto (off)', 'Relay: auto (on)', 'Relay: on', 'Relay: off'),
          'ru': ('Релей: авто', 'Релей: авто (выкл)', 'Релей: авто (вкл)', 'Релей: вкл', 'Релей: выкл'),
          'uk': ('Релей: авто', 'Релей: авто (вимк)', 'Релей: авто (увімк)', 'Релей: увімк', 'Релей: вимк'),
          'de': ('Relay: auto', 'Relay: auto (aus)', 'Relay: auto (an)', 'Relay: an', 'Relay: aus'),
          'pl': ('Relay: auto', 'Relay: auto (wył.)', 'Relay: auto (wł.)', 'Relay: wł.', 'Relay: wył.')}
LABEL_OF_VIEW = {1: 0, 2: 1, 3: 2, 4: 0, 5: 3, 6: 4}
# The menu's rows: auto, on, off.
ROWS = {'en': ('Auto: only when blocked', 'On: always through the relay', 'Off: never'),
        'ru': ('Авто: только при блокировке', 'Вкл: всегда через релей', 'Выкл: никогда'),
        'uk': ('Авто: лише при блокуванні', 'Увімк: завжди через релей', 'Вимк: ніколи'),
        'de': ('Auto: nur bei Sperre', 'An: immer über das Relay', 'Aus: nie'),
        'pl': ('Auto: tylko przy blokadzie', 'Wł.: zawsze przez relay', 'Wył.: nigdy')}
# The game's languages (launcher_languages_026g.inc order) -> the row's.
BY_INDEX = {0: 'en', 1: 'uk', 2: 'ru', 3: 'de', 4: 'pl', 5: 'en', 6: 'en'}
NOW_EN = ['auto, checking...', 'auto: direct.', 'auto: through the relay.',
          'auto: the server does not answer.', 'on: through the relay.', 'off: direct.']
RESTART = {'en': 'The server is restarting. Next try in {} s.',
           'ru': 'Сервер перезапускается. Повтор через {} с.',
           'uk': 'Сервер перезапускається. Повтор через {} с.',
           'de': 'Der Server startet neu. Nächster Versuch in {} s.',
           'pl': 'Serwer uruchamia się ponownie. Kolejna próba za {} s.'}
TIP_EN = ('The relay is a second server that forwards your game when your provider blocks ours.\r\n'
          'Auto: through the relay only when the direct way is blocked.\r\n'
          'On: always through the relay (a bit more ping).\r\nOff: never.\r\nClick the gear to choose.\r\n\r\nNow: ')

NEW = {'ShowWindow': 2, 'GetParent': 1, 'SendMessageW': 4, 'CreateWindowExW': 12, 'DestroyWindow': 1,
       'LoadLibraryW': 1, 'InitCommonControlsEx': 1}
for name, count in NEW.items():
    if name not in C.ARGS:
        C.ARGS[name] = count
        C.NAMES.append(name)
T.HELPERS.update({0x16811: ('refresh', 0), 0x1749F: ('send_simple', 2)})


class Relay(T.Box):
    def __init__(self, data, delta):
        super().__init__(data, delta)
        self.loaded = False
        self.windows = 0

    def GetModuleHandleA(self, name):
        return {'kernel32.dll': 0x7100, 'user32.dll': 0x7200, 'shell32.dll': 0x7300,
                'comctl32.dll': 0x7400 if self.loaded else 0}[self.ansi(name).lower()]

    def GetProcAddress(self, module, name):
        text = self.ansi(name)
        if (module, text) in {(0x7200, n) for n in ('ShowWindow', 'GetParent', 'SendMessageW', 'CreateWindowExW',
                                                     'DestroyWindow')} | {(0x7100, 'LoadLibraryW'),
                                                                          (0x7400, 'InitCommonControlsEx')}:
            return C.STUBS + 4 * C.NAMES.index(text)
        return super().GetProcAddress(module, name)

    def ShowWindow(self, window, how):
        self.log.append(('show', window, how))
        return 1

    def GetParent(self, window):
        assert window == self.word(FORM + RELAY + 4), window
        return PARENT

    def LoadLibraryW(self, name):
        assert self.wide(name) == 'comctl32.dll'
        self.loaded = True
        return 0x7400

    def InitCommonControlsEx(self, init):
        self.log.append(('init', *struct.unpack('<2I', self.u.mem_read(init, 8))))
        return 1

    def CreateWindowExW(self, extended, kind, name, style, x, y, width, height, parent, menu, instance, param):
        self.windows += 1
        self.log.append(('create', extended, self.wide(kind), name, style, (x, y, width, height), parent))
        return TOOLTIP + self.windows

    def DestroyWindow(self, window):
        self.log.append(('destroy', window))
        return 1

    def SendMessageW(self, window, message, wparam, lparam):
        if message in (0x432, 0x439):
            info = struct.unpack('<4I4i3I', self.u.mem_read(lparam, 44))
            self.log.append(('tool', window, message, info[:4], self.wide(info[9])))
        else:
            self.log.append(('send', window, message, wparam, lparam))
        return 1

    def load_string(self, number, buffer, size, *_):
        if number == 0xD3:
            return super().load_string(number, buffer, size)
        self.log.append(('load', number, buffer, size))
        self.u.mem_write(buffer, f'string {number:#x}'.encode('utf-16-le') + b'\0\0')
        return 1

    def GetWindowRect(self, window, rect):
        assert window == BOX_WINDOW, window
        self.u.mem_write(rect, struct.pack('<4i', *BUTTON_RECT))
        return 1

    def TrackPopupMenu(self, menu, flags, x, y, reserved, window, rect):
        # Under the button, at its left.
        assert (menu, flags, x, y, window) == (0x900, 0x180, BUTTON_RECT[0], BUTTON_RECT[3], BOX_WINDOW)
        self.log.append(('menu', list(self.menu)))
        return self.pick

    def refresh(self, *_):
        self.log.append(('refresh', self.reg(UC_X86_REG_ESI)))
        return 0

    def send_simple(self, launcher, message):
        self.log.append(('pipe', launcher, message))
        return 1

    def byte(self, address):
        return self.u.mem_read(address + self.delta, 1)[0]


def form(one, delta):
    one.u.mem_write(FORM, bytes(0x2000))
    one.word(FORM + 4, 0x444)
    one.word(FORM + 0xEC, T.PANEL)
    for slot, png in zip(SLOTS, PNGS):
        one.word(FORM + slot, png)
    one.word(FORM + 0x4E0, PARENT)                  # the bottom panel's window
    one.word(FORM + RELAY + 4, BOX_WINDOW)
    one.word(LAUNCHER + 0x2DC4, FORM)
    one.run(0x3A761)                                # VK's rect: its right and bottom
    assert struct.unpack('<4i', one.u.mem_read(0x458D4 + delta, 16)) == RECT


def message(one, view):
    """Message 0A with `view`, from on_updater_message's dispatch to its end."""
    payload = T.FRAME + 0x200
    one.u.mem_write(payload, bytes([view]))
    sp = C.STACK - 0x100
    one.u.mem_write(sp, bytes(0x40))
    one.u.mem_write(sp + 0xF, bytes([0x0A]))
    one.word(sp + 0x18, payload)
    one.reg(T.UC_X86_REG_ESP, sp)
    for register, value in ((T.UC_X86_REG_EBX, LAUNCHER), (T.UC_X86_REG_ESI, 0x1234), (T.UC_X86_REG_EDI, 0x5678),
                            (T.UC_X86_REG_EBP, 0x9ABC)):
        one.reg(register, value)
    one.u.emu_start(0x15BAC + one.delta, 0x16093 + one.delta, count=500000)
    assert one.reg(T.UC_X86_REG_EIP) == 0x16093 + one.delta
    assert one.reg(T.UC_X86_REG_ESP) == sp
    assert [one.reg(r) for r in (T.UC_X86_REG_EBX, T.UC_X86_REG_ESI, T.UC_X86_REG_EDI, T.UC_X86_REG_EBP)] \
        == [LAUNCHER, 0x1234, 0x5678, 0x9ABC]


def tip_text(view):
    return TIP_EN + NOW_EN[view - 1]


def check(data, delta):
    count = 0
    # Message 0A: each view, then the next; the tooltip made once.
    one = Relay(data, delta)
    form(one, delta)
    for number, view in enumerate([1, 2, 3, 4, 5, 6, 1]):
        one.log.clear()
        message(one, view)
        assert one.word(STATE + delta) == LAUNCHER and one.byte(VIEW) == view
        # The button's pictures are the form's: no view changes them.
        assert [one.word(FORM + RELAY + 8), one.word(FORM + RELAY + 0xC)] == [0, 0], view
        assert ('show', BOX_WINDOW, 8) in one.log and ('refresh', LAUNCHER) in one.log
        assert [e for e in one.log if e[0] == 'refresh'] == [('refresh', LAUNCHER)]
        tools = [e for e in one.log if e[0] == 'tool']
        if number == 0:
            assert one.log.index(('init', 8, 4)) < one.log.index(next(e for e in one.log if e[0] == 'create'))
            creates = [e for e in one.log if e[0] == 'create']
            assert creates == [('create', 8, 'tooltips_class32', 0, 0x80000003, (0x80000000,) * 4, PARENT)], creates
            assert ('send', TOOLTIP + 1, 0x418, 0, 360) in one.log
            assert ('send', TOOLTIP + 1, 0x403, 2, 30000) in one.log
            assert tools == [('tool', TOOLTIP + 1, 0x432, (44, 0x11, PARENT, BOX_WINDOW), tip_text(view))], tools
        else:
            assert not [e for e in one.log if e[0] in ('create', 'destroy', 'init')], one.log
            assert tools == [('tool', TOOLTIP + 1, 0x439, (44, 0x11, PARENT, BOX_WINDOW), tip_text(view))], tools
        assert one.word(TIP + delta) == TOOLTIP + 1 and one.word(TIP_TOOL + delta) == BOX_WINDOW
        count += 1
    # A view the box does not know: nothing at all.
    for view in (0, 7, 255):
        one.log.clear()
        message(one, view)
        assert one.byte(VIEW) == 1 and not one.log, (view, one.log)
        count += 1

    # The button's making: hidden before any view; shown after one, the
    # tooltip made again for the new window.
    for view in (0, 3, 6):
        one = Relay(data, delta)
        form(one, delta)
        if view:
            message(one, view)
            one.log.clear()
            one.word(FORM + RELAY + 4, 0)
        one.made_window = BOX_WINDOW + 1

        def window(this, parent, kind, x, y, width, height, style, one=one):
            rect = struct.unpack('<4i', one.u.mem_read(one.reg(UC_X86_REG_ESI), 16))
            one.log.append(('window', this, parent, style, rect))
            one.word(this + 4, one.made_window)
            return this
        one.window = window
        assert one.go(0x1AEA8, 0x1AEEA, ebx=FORM)
        control = FORM + RELAY
        assert ('window', control, PARENT, 0x50000000, RECT) in one.log, one.log
        assert one.word(control + 0x18) == 0x1BD3C + delta and one.word(control + 0x1C) == FORM
        assert [one.word(control + at) for at in (8, 0xC, 0x10)] == list(PNGS)
        if not view:
            assert [e for e in one.log if e[0] == 'show'] == [('show', BOX_WINDOW + 1, 0)]
            assert not [e for e in one.log if e[0] in ('create', 'tool')]
        else:
            assert [e for e in one.log if e[0] == 'show'] == [('show', BOX_WINDOW + 1, 8)]
            assert one.log.index(('destroy', TOOLTIP + 1)) < one.log.index(next(e for e in one.log if e[0] == 'create'))
            assert [e for e in one.log if e[0] == 'tool'] == [
                ('tool', TOOLTIP + 2, 0x432, (44, 0x11, PARENT, BOX_WINDOW + 1), tip_text(view))]
            assert one.word(TIP_TOOL + delta) == BOX_WINDOW + 1
        count += 1

    # The panel's text: none while hidden; the view's, in each language.
    for view in (0, 1, 2, 3, 4, 5, 6):
        for index, language in BY_INDEX.items():
            one = Relay(data, delta)
            form(one, delta)
            one.word(FORM + 0xB40, C.SETTINGS)
            one.word(LANGUAGE_GLOBAL + delta, index)
            one.u.mem_write(VIEW + delta, bytes([view]))
            assert one.go(0x1AD64, 0x1AD7D, ebx=FORM)
            texts = [e for e in one.log if e[0] == 'text']
            relay = [e for e in texts if e[1] != 'Allow torrent']
            if not view:
                assert not relay, relay
            else:
                label = LABELS[language][LABEL_OF_VIEW[view]]
                assert relay == [('text', label, T.PANEL, T.FONT, 0xFFFFFF, 0, 28 + 25, 0, 46 + 1)], relay
            count += 1

    # A click: the menu of the three while connected and shown, the chosen
    # row checked; its pick to the updater. No menu otherwise.
    cases = [(3, view, pick, index) for view in (1, 2, 3, 4, 5, 6) for pick in (0, 1, 2, 3) for index in (0, 2)]
    cases += [(3, 5, 2, index) for index in BY_INDEX] + [(2, 5, 1, 0), (3, 0, 1, 0)]
    for connected, view, pick, index in cases:
        one = Relay(data, delta)
        form(one, delta)
        one.word(LANGUAGE_GLOBAL + delta, index)
        one.word(LAUNCHER + 0x2B70, connected)
        if view:
            message(one, view)
        one.log.clear()
        assert one.go(0x1AEA8, 0x1AEEA, ebx=FORM)
        control = FORM + RELAY
        one.word(control + 4, BOX_WINDOW)
        one.log.clear()
        one.pick, one.menu_destroyed = pick, False
        one.u.mem_write(control + 0x14, b'\0\0')
        one.go(0x126F4, C.STOP, args=(BOX_WINDOW, 0x201, 1, 0), ecx=control)
        menus = [e for e in one.log if e[0] == 'menu']
        sent = [e for e in one.log if e[0] == 'pipe']
        if connected == 3 and view:
            chosen = {5: 1, 6: 2}.get(view, 0)
            rows = [(8 if row == chosen else 0, row + 1, text) for row, text in enumerate(ROWS[BY_INDEX[index]])]
            assert menus == [('menu', rows)], menus
            assert one.menu_destroyed
            assert sent == ([('pipe', LAUNCHER, 0x21 + pick)] if pick else []), one.log
        else:
            assert not menus and not sent, one.log
        count += 1
    # The status line: the restart's count in the place of "Server
    # maintenance...", only while the updater counts.
    buffer = T.FRAME + 0x400
    for index, language in BY_INDEX.items():
        for number, seconds in ((0xB2, 0), (0xB2, 1), (0xB2, 5), (0xB2, 60), (0xB2, 255), (0x87, 0), (0x87, 5)):
            one = Relay(data, delta)
            one.word(LANGUAGE_GLOBAL + delta, index)
            one.u.mem_write(LAUNCHER + 0x2BA0, bytes([seconds]))
            one.u.mem_write(buffer, b'\xEE' * 0x100)
            kept = {'ebx': 0x1111, 'edi': 0x2222, 'ebp': 0x3333, 'esi': LAUNCHER}
            assert one.go(0x1687C, 0x16881, args=(number, buffer, 0x80), **kept)
            assert [one.reg(r) for r in (T.UC_X86_REG_EBX, T.UC_X86_REG_EDI, T.UC_X86_REG_EBP, UC_X86_REG_ESI)] \
                == [0x1111, 0x2222, 0x3333, LAUNCHER]
            if number == 0xB2 and seconds:
                assert not one.log, one.log
                assert one.wide(buffer) == RESTART[language].format(seconds), one.wide(buffer)
            else:
                assert one.log == [('load', number, buffer, 0x80)], one.log
                assert one.wide(buffer) == f'string {number:#x}'
            count += 1
    return count


def main():
    original = Path(sys.argv[1]).read_bytes()
    code = (HERE / 'launcher_values_026g.bin').read_bytes()
    patched, _ = L.build(original, code)
    C.use_documents(C.DOCUMENTS_FOLDERS[0])
    for delta in (0, 0x100000):
        print(f'base+{delta:#x}: {check(bytes(patched), delta)} Relay cases pass')
    print('ALL RELAY CASES PASSED')


if __name__ == '__main__':
    main()
