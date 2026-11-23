#!/usr/bin/env python3
"""Runs the "Allow torrent" box of the patched launcher (patch 15,
launcher_values_026g.asm "Allow torrent") under Unicorn, through the sites
the patch changed, at its own base and rebased, with Windows' functions and
the launcher's drawing helpers stubbed, user.cfg (check_language_026g's) and
the registry in memory.

Usage: python check_torrent_box_026g.py ORIGINAL_survarium_launcher.exe

Checks:
- the start (launcher::launcher 0x141D9..0x1420C: settings::reload, then
  settings::reload_language): "Allow torrent" (settings+0, +1) is user.cfg's
  `enable_torrent_seeding on`, off without the line or with `off`, whatever
  the registry's `download_type` says;
- the bottom panel (0x1AD64..0x1AD7D): "Allow torrent" (string 0D3h) drawn
  into the panel's picture in Calibri bold 18, white, 25 pixels right of the
  box and 1 below its top, the font deleted; each of the box's four pictures
  replaced by itself over the panel's picture where the box is (28, 20, 20
  by 20), the PNG's deleted; Twitter's three pictures replaced by the panel's
  picture (text included) where the text is (53, 20 to 187, 40), the PNGs
  deleted; the relay button's three (patch 16, check_relay_box_026g.py) where
  it is (28, 46); then image_window made as before;
- the box (0x1ADE2..0x1AE24): YouTube's control with the unticked pictures,
  or the ticked one as its picture and the one under the pointer, as
  settings+0 says; its click is 0x1BD03 with the form;
- the text's control (0x1AE66..0x1AEA8): Twitter's, a visible window in the
  bottom panel at its static rect (its initializer returns at once), the
  hand cursor, its three pictures the panel's, opaque: a WM_PAINT copies
  its picture with BitBlt (SRCCOPY), no AlphaBlend; its click is 0x1BD29
  with the form;
- a click (the control's window procedure, WM_LBUTTONDOWN): settings+0 and
  +1 toggled, user.cfg's line written at once (`on`, then `off`; the other
  lines kept), the "download type changed" callback fired once (1A to the
  updater), the pictures changed, no registry call; then a fresh start reads
  the box as it was left: it stays ticked until it is unticked; clicks on
  the text (its control's window procedure, through 0x1BD29) do the same,
  alternating with the box's, and leave the text's pictures as they are;
- set_status_texts (0x1B4D9..0x1B4DE): the panel's picture copied anew as
  before, then the text drawn again;
- the one-time reset at the start: without `launcher_skhid.torrent_reset`
  beside user.cfg, that file is made and the line written `off` (the
  original game saved `on` for every player); a user.cfg that cannot be
  written (locked, read-only) keeps no file and its `on` is not taken (the
  next start tries again); with the file there, a tick stays across restarts.
"""
from pathlib import Path
import struct
import sys
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX,
                               UC_X86_REG_EDI, UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_EIP)
import check_language_026g as C
import patch_launcher_026g as L

HERE = Path(__file__).resolve().parent
FORM, LAUNCHER, FRAME = 0x33000000, 0x34000000, 0x30010000
EXTRA = C.STUBS + 0x800
PRODUCT_KEY = 'Software\\Vostok Games\\Survarium'
# Windows functions the box's code reaches that check_language does not stub: (IAT slot, name, stdcall arguments)
EXTRA_IAT = [(0x3B2EC, 'LoadCursorW', 2), (0x3B07C, 'DeleteObject', 1), (0x3B2F0, 'InvalidateRect', 3),
             (0x3B028, 'RegOpenKeyExW', 5), (0x3B020, 'RegQueryValueExW', 6), (0x3B024, 'RegCloseKey', 1),
             (0x3B02C, 'RegCreateKeyExW', 9), (0x3B000, 'RegSetValueExW', 6),
             (0x3B2F4, 'BeginPaint', 2), (0x3B2F8, 'EndPaint', 2), (0x3B070, 'CreateCompatibleDC', 1),
             (0x3B06C, 'SelectObject', 2), (0x3B060, 'BitBlt', 9), (0x3B29C, 'AlphaBlend', 11),
             (0x3B05C, 'DeleteDC', 1)]
# The launcher's own helpers, stubbed by address: (VA, name, stdcall arguments or None for cdecl)
HELPERS = {0x12140: ('image_window', 2), 0x13885: ('window', 8), 0x12051: ('crop', None),
           0x12BE0: ('copy', None), 0x137A8: ('load_string', None), 0x11CC7: ('make_font', 3),
           0x137C3: ('draw_text', None), 0x17522: ('function0', None), 0x1D496: ('settings_key', None)}
PANEL, PNGS, FONT, CONTROL_WINDOW = 0xB0, (0xA1, 0xA2, 0xA3, 0xA4), 0x6F0, 0x888
SLOTS = (0x140, 0x14C, 0x158, 0x164)    # the box's pictures: unticked, under the pointer, pressed, ticked
BOX = 0x648
LABEL, LABEL_SLOTS, LABEL_PNGS, LABEL_WINDOW = 0x698, (0x188, 0x194, 0x1A0), (0xB1, 0xB2, 0xB3), 0x889
LABEL_RECT = (53, 20, 187, 40)          # the text's: x 53..186 in the widest language (Russian)
RELAY_SLOTS, RELAY_PNGS = (0x1AC, 0x1B8, 0x1C4), (0xC1, 0xC2, 0xC3)   # patch 16's button
BODY = b'account_name x\r\nlocalization polish\r\nr_fullscreen on\r\n'


class Box(C.Launcher):
    def __init__(self, data, delta, registry=None):
        super().__init__(data, delta)
        for address, size in [(FORM, 0x2000), (LAUNCHER, 0x6000)]:
            self.u.mem_map(address, size)
        for i, (slot, _, _) in enumerate(EXTRA_IAT):
            self.word(slot + delta, EXTRA + 4 * i)
        self.registry = dict(registry or {})
        self.log, self.made = [], 0x7000
        self.folders.add((C.DOCUMENTS + '\\survarium').lower())

    def hook(self, u, address, size, user):
        sp = u.reg_read(UC_X86_REG_ESP)
        if EXTRA <= address < EXTRA + 4 * len(EXTRA_IAT):
            _, name, count = EXTRA_IAT[(address - EXTRA) // 4]
            args = [self.word(sp + 4 + 4 * i) for i in range(count)]
            self.ret(getattr(self, name)(*args), count)
            return
        helper = HELPERS.get(address - self.delta)
        if helper:
            name, count = helper
            args = [self.word(sp + 4 + 4 * i) for i in range(count or 8)]
            self.ret(getattr(self, name)(*args), count or 0)
            return
        super().hook(u, address, size, user)

    def reg(self, register, value=None):
        if value is None:
            return self.u.reg_read(register)
        self.u.reg_write(register, value)

    # --- Windows ---
    def LoadCursorW(self, instance, name):
        return 0x55 if (instance, name) == (0, 0x7F89) else 0x56      # IDC_HAND

    def BeginPaint(self, window, paint):
        self.u.mem_write(paint, struct.pack('<2I4i', 0xDC1, 0, 0, 0, 134, 20) + bytes(48))
        return 0xDC1

    def EndPaint(self, window, paint):
        return 1

    def CreateCompatibleDC(self, dc):
        return 0xDC2

    def SelectObject(self, dc, picture):
        self.log.append(('select', dc, picture))
        return 0x77

    def BitBlt(self, dc, x, y, width, height, source, sx, sy, rop):
        self.log.append(('bitblt', dc, x, y, width, height, source, sx, sy, rop))
        return 1

    def AlphaBlend(self, *args):
        self.log.append(('alphablend',))
        return 1

    def DeleteDC(self, dc):
        return 1

    def DeleteObject(self, handle):
        self.log.append(('delete', handle))
        return 1

    def InvalidateRect(self, window, rect, erase):
        self.log.append(('invalidate', window))
        return 1

    def RegOpenKeyExW(self, hive, key, options, sam, out):
        self.log.append(('registry', 'open'))
        self.word(out, 0x1004)
        return 0

    def RegQueryValueExW(self, key, name, reserved, kind, data, size):
        name = self.wide(name)
        self.log.append(('registry', name))
        if name not in self.registry:
            return 2
        self.u.mem_write(data, struct.pack('<I', self.registry[name]))
        self.word(kind, 4)
        self.word(size, 4)
        return 0

    def RegCloseKey(self, key):
        return 0

    def RegCreateKeyExW(self, *args):
        self.log.append(('registry', 'create'))
        return 2

    def RegSetValueExW(self, *args):
        self.log.append(('registry', 'set'))
        return 2

    # --- the launcher's helpers ---
    def image_window(self, parent, picture):
        self.log.append(('image_window', self.reg(UC_X86_REG_EDI), self.reg(UC_X86_REG_ESI) - self.delta,
                         parent, picture))
        return self.reg(UC_X86_REG_EDI)

    def window(self, this, parent, kind, x, y, width, height, style):
        rect = struct.unpack('<4i', self.u.mem_read(self.reg(UC_X86_REG_ESI), 16))
        self.log.append(('window', this, parent, style, rect))
        return this

    def crop(self, bitmap, *_):
        rect = struct.unpack('<4i', self.u.mem_read(self.reg(UC_X86_REG_EAX), 16))
        self.made += 1
        self.log.append(('crop', bitmap, rect, self.made))
        return self.made

    def copy(self, destination, source, point, *_):
        rect = struct.unpack('<4i', self.u.mem_read(self.reg(UC_X86_REG_EAX), 16))
        self.log.append(('copy', destination, source, struct.unpack('<2i', self.u.mem_read(point, 8)), rect))
        return 1

    def load_string(self, number, buffer, size, *_):
        assert number == 0xD3 and size == 64
        self.u.mem_write(buffer, 'Allow torrent'.encode('utf-16-le') + b'\0\0')
        return 1

    def make_font(self, height, weight, face):
        self.log.append(('font', height, weight, self.wide(face)))
        self.word(self.reg(UC_X86_REG_ESI), FONT)
        return self.reg(UC_X86_REG_ESI)

    def draw_text(self, text, bitmap, font, colour, form, left, margin, top):
        self.log.append(('text', self.wide(text), bitmap, self.word(font), colour, form, left, margin, top))
        return 0

    def function0(self, *_):
        self.log.append(('download type changed', self.reg(UC_X86_REG_EAX)))
        return 0

    def settings_key(self, *_):
        at, self.heap = self.heap, self.heap + 0x100
        self.u.mem_write(at, PRODUCT_KEY.encode('utf-16-le') + b'\0\0')
        return at

    # --- runs ---
    def go(self, start, stop, args=(), **registers):
        """From start to stop (C.STOP: a whole function, which returns there,
        args its arguments; else a VA past the site, args what the code
        before the site pushed); whether the stack is as it was."""
        whole = stop == C.STOP
        sp = C.STACK - 4 * (len(args) + whole)
        if whole:
            self.word(sp, C.STOP)
        for i, value in enumerate(args):
            self.word(sp + 4 * whole + 4 * i, value)
        self.reg(UC_X86_REG_ESP, sp)
        names = {'eax': UC_X86_REG_EAX, 'ebx': UC_X86_REG_EBX, 'ecx': UC_X86_REG_ECX, 'edi': UC_X86_REG_EDI,
                 'esi': UC_X86_REG_ESI, 'ebp': UC_X86_REG_EBP}
        for name, value in registers.items():
            self.reg(names[name], value)
        self.u.emu_start(start + self.delta, stop if stop == C.STOP else stop + self.delta, count=500000)
        assert self.reg(UC_X86_REG_EIP) == (stop if stop == C.STOP else stop + self.delta), hex(self.reg(UC_X86_REG_EIP))
        return self.reg(UC_X86_REG_ESP) == (sp + 4 if stop == C.STOP else sp)

    def start(self):
        """launcher::launcher's settings: settings::reload, then reload_language -> (+0, +1)."""
        settings = LAUNCHER + 0x1A48
        self.u.mem_write(settings, b'\x5a' * 0x60)
        self.word(FRAME + 8, 0)
        assert self.go(0x141D9, 0x1420C, eax=0, ebx=LAUNCHER, ebp=FRAME)
        assert not self.live, 'a buffer left unfreed'
        return tuple(self.u.mem_read(settings, 2))


def check(data, delta):
    count = 0
    text_at = ('text', 'Allow torrent', PANEL, FONT, 0xFFFFFF, 0, 28 + 25, 0, 20 + 1)

    def form(one, settings):
        one.u.mem_write(FORM, bytes(0x2000))
        one.word(FORM + 4, 0x444)
        one.word(FORM + 0xEC, PANEL)
        for slot, png in zip(SLOTS + LABEL_SLOTS + RELAY_SLOTS, PNGS + LABEL_PNGS + RELAY_PNGS):
            one.word(FORM + slot, png)
        one.word(FORM + 0x4E0, 0x445)                   # the bottom panel's window
        one.word(FORM + 0xB40, settings)                # settings_window (+6E8h) + 458h
        one.run(0x3A710)                                # YouTube's rect: its right and bottom
        one.run(0x3A746)                                # Twitter's: returns, the rect is static
        one.run(0x3A761)                                # VK's, the relay button's
        assert struct.unpack('<4i', one.u.mem_read(0x458C4 + delta, 16)) == LABEL_RECT

    # The start: user.cfg's line, never the registry's download_type.
    for cfg, registry, expected in [
            (None, {'download_type': 1}, (0, 0)),
            (BODY, {'download_type': 1}, (0, 0)),
            (BODY + b'enable_torrent_seeding off\r\n', {'download_type': 1}, (0, 0)),
            (BODY + b'enable_torrent_seeding on\r\n', {'download_type': 0}, (1, 1)),
            (b'enable_torrent_seeding\ton', {}, (1, 1)),
            (b'enable_torrent_seeding on2\r\n', {}, (0, 0)),
            (b'enable_torrent_seedings on\r\n', {}, (0, 0))]:
        one = Box(data, delta, registry)
        if cfg is not None:
            one.files[C.CFG] = cfg
        assert one.start() == expected, (cfg, registry, one.start())
        assert one.files == ({} if cfg is None else {C.CFG: cfg}), 'the start writes nothing'
        count += 1

    # The bottom panel: the text, the box's pictures made opaque, image_window.
    one = Box(data, delta)
    form(one, C.SETTINGS)
    assert one.go(0x1AD64, 0x1AD7D, ebx=FORM)
    made = [entry for entry in one.log if entry[0] == 'crop']
    assert one.log[:2] == [('font', 18, 700, 'Calibri'), text_at], one.log
    assert made == ([('crop', PANEL, (28, 20, 48, 40), 0x7001 + i) for i in range(4)]
                    + [('crop', PANEL, LABEL_RECT, 0x7005 + i) for i in range(3)]
                    + [('crop', PANEL, (28, 46, 48, 66), 0x7008 + i) for i in range(3)]), made
    copies = [entry for entry in one.log if entry[0] == 'copy']
    assert copies == ([('copy', 0x7001 + i, png, (0, 0), (0, 0, 20, 20)) for i, png in enumerate(PNGS)]
                      + [('copy', 0x7008 + i, png, (0, 0), (0, 0, 20, 20)) for i, png in enumerate(RELAY_PNGS)]), copies
    assert [entry for entry in one.log if entry[0] == 'delete'] == [('delete', FONT)] + [('delete', p) for p in
                                                                                          PNGS + LABEL_PNGS + RELAY_PNGS]
    assert [one.word(FORM + slot) for slot in SLOTS + LABEL_SLOTS + RELAY_SLOTS] == list(range(0x7001, 0x700B))
    assert one.log[-1] == ('image_window', FORM + 0x4DC, 0x45884, 0x444, PANEL), one.log[-1]
    count += 1

    # The box, unticked and ticked; then clicks, and the start after each.
    for ticked in (0, 1):
        one = Box(data, delta)
        one.u.mem_write(C.SETTINGS, bytes([ticked, ticked]) + bytes(0x60))
        form(one, C.SETTINGS)
        one.word(FRAME + 0xC, C.SETTINGS)               # launcher_form's [ebp+0Ch]
        assert one.go(0x1ADE2, 0x1AE24, ebx=FORM, ebp=FRAME)
        control = FORM + BOX
        pictures = [one.word(control + at) for at in (8, 0xC, 0x10)]
        expected = [PNGS[3], PNGS[3], PNGS[2]] if ticked else list(PNGS[:3])
        assert pictures == expected, (ticked, pictures)
        assert one.word(control + 0x18) == 0x1BD03 + delta and one.word(control + 0x1C) == FORM
        count += 1

    # The text's control: Twitter's, over the text, opaque, its click 0x1BD29.
    one = Box(data, delta)
    form(one, C.SETTINGS)
    assert one.go(0x1AD64, 0x1AD7D, ebx=FORM)
    one.log.clear()
    assert one.go(0x1AE66, 0x1AEA8, ebx=FORM)
    label = FORM + LABEL
    assert one.log == [('window', label, 0x445, 0x50000000, LABEL_RECT)], one.log
    assert [one.word(label + at) for at in (8, 0xC, 0x10)] == [0x7005, 0x7006, 0x7007]
    assert one.word(label + 0x18) == 0x1BD29 + delta and one.word(label + 0x1C) == FORM
    assert one.word(label + 0x20) == 0x55 and one.u.mem_read(label + 0x24, 1)[0] == 1
    one.word(label + 4, LABEL_WINDOW)
    for hover, pressed in ((0, 0), (1, 0), (1, 1)):
        one.log.clear()
        one.u.mem_write(label + 0x14, bytes([hover, pressed]))
        one.go(0x126F4, C.STOP, args=(LABEL_WINDOW, 0xF, 0, 0), ecx=label)
        picture = (0x7007 if pressed else 0x7006 if hover else 0x7005)
        assert ('select', 0xDC2, picture) in one.log
        assert [e for e in one.log if e[0] in ('bitblt', 'alphablend')] == [
            ('bitblt', 0xDC1, 0, 0, 134, 20, 0xDC2, 0, 0, 0xCC0020)], one.log
    count += 1

    one = Box(data, delta, {'download_type': 1})
    one.files[C.CFG] = BODY
    form(one, C.SETTINGS)
    one.u.mem_write(C.SETTINGS, bytes(0x60))
    one.word(C.SETTINGS + 0x28, 1)                      # the "download type changed" callback is set
    one.word(FRAME + 0xC, C.SETTINGS)
    assert one.go(0x1ADE2, 0x1AE24, ebx=FORM, ebp=FRAME)
    assert one.go(0x1AE66, 0x1AEA8, ebx=FORM)
    control, label = FORM + BOX, FORM + LABEL
    one.word(control + 4, CONTROL_WINDOW)
    one.word(label + 4, LABEL_WINDOW)
    label_pictures = [one.word(label + at) for at in (8, 0xC, 0x10)]
    on, off = b'enable_torrent_seeding on\r\n', b'enable_torrent_seeding off\r\n'
    box, text = (control, CONTROL_WINDOW), (label, LABEL_WINDOW)
    for state, line, (clicked, window) in ((1, on, box), (0, off, text), (1, on, box), (0, off, text),
                                           (1, on, text)):
        one.log.clear()
        one.u.mem_write(clicked + 0x14, b'\0\0')        # not under the pointer, not pressed
        one.go(0x126F4, C.STOP, args=(window, 0x201, 1, 0), ecx=clicked)  # stdcall: ret 10h
        assert tuple(one.u.mem_read(C.SETTINGS, 2)) == (state, state)
        assert one.files == {C.CFG: BODY + line}, one.files
        assert not one.live and not one.handles
        assert [e for e in one.log if e[0] == 'download type changed'] == [('download type changed', C.SETTINGS + 0x28)]
        assert not [e for e in one.log if e[0] == 'registry'], 'no registry call'
        expected = [PNGS[3], PNGS[3]] if state else [PNGS[0], PNGS[1]]
        assert [one.word(control + 8), one.word(control + 0xC)] == expected
        assert ('invalidate', CONTROL_WINDOW) in one.log
        assert [one.word(label + at) for at in (8, 0xC, 0x10)] == label_pictures
        # A fresh start (the launcher started again, the PC restarted, a
        # new launcher): the box as it was left.
        again = Box(data, delta, {'download_type': 1 - state})
        again.files = dict(one.files)
        assert again.start() == (state, state)
        count += 1
    # A click (the box's or the text's) whose line cannot be written: the
    # box, the updater and the file stay as they were.
    one.locked.add(C.CFG.lower())
    for clicked, window in (box, text):
        one.log.clear()
        one.u.mem_write(clicked + 0x14, b'\0\0')
        one.go(0x126F4, C.STOP, args=(window, 0x201, 1, 0), ecx=clicked)
        assert tuple(one.u.mem_read(C.SETTINGS, 2)) == (1, 1)
        assert one.files == {C.CFG: BODY + on}, one.files
        assert not [e for e in one.log if e[0] == 'download type changed']
        assert [one.word(control + 8), one.word(control + 0xC)] == [PNGS[3], PNGS[3]]
        assert not one.live and not one.handles
        count += 1

    # The one-time reset: the original game's `on` becomes `off` once.
    legacy = BODY + b'enable_torrent_seeding on\r\n'
    for files, after in [({C.CFG: legacy}, {C.CFG: BODY + b'enable_torrent_seeding off\r\n'}),
                         ({}, {C.CFG: b'enable_torrent_seeding off\r\n'})]:
        one = Box(data, delta)
        one.files, one.marker = dict(files), False
        assert one.start() == (0, 0)
        assert one.files == after and one.marker and one.markers_made == 1, (one.files, one.markers_made)
        count += 1
    # A user.cfg that cannot be written (locked, read-only): the marker made
    # goes again, the old `on` is not taken, the next start tries again.
    for blocked in ('locked', 'readonly'):
        one = Box(data, delta)
        one.files, one.marker = {C.CFG: legacy}, False
        getattr(one, blocked).add(C.CFG.lower())
        assert one.start() == (0, 0), blocked
        assert one.files == {C.CFG: legacy} and not one.marker, (blocked, one.files)
        count += 1
    getattr(one, blocked).clear()
    assert one.start() == (0, 0) and one.marker and one.markers_made == 2
    # Then ticked: it stays ticked, the reset done.
    form(one, C.SETTINGS)
    one.u.mem_write(C.SETTINGS, bytes(0x60))
    one.word(FRAME + 0xC, C.SETTINGS)
    assert one.go(0x1ADE2, 0x1AE24, ebx=FORM, ebp=FRAME)
    one.u.mem_write(FORM + BOX + 0x14, b'\0\0')
    one.go(0x126F4, C.STOP, args=(CONTROL_WINDOW, 0x201, 1, 0), ecx=FORM + BOX)
    assert one.files == {C.CFG: legacy}
    for _ in range(2):
        assert one.start() == (1, 1) and one.files == {C.CFG: legacy} and one.markers_made == 2
    count += 2

    # set_status_texts: the copy as before, then the text again.
    one = Box(data, delta)
    form(one, C.SETTINGS)
    rect, point = FRAME + 0x100, FRAME + 0x120
    one.u.mem_write(rect, struct.pack('<4i', 0, 0, 899, 86))
    one.u.mem_write(point, bytes(8))
    assert one.go(0x1B4D9, 0x1B4DE, args=(PANEL, 0x99, point), eax=rect, edi=FORM)
    assert one.log[0] == ('copy', PANEL, 0x99, (0, 0), (0, 0, 899, 86)), one.log
    assert one.log[1:3] == [('font', 18, 700, 'Calibri'), text_at] and one.log[3] == ('delete', FONT)
    count += 1
    return count


def main():
    original = Path(sys.argv[1]).read_bytes()
    code = L.code()
    patched, _ = L.build(original, code)
    for folder in C.DOCUMENTS_FOLDERS:
        C.use_documents(folder)
        for delta in (0, 0x100000):
            print(f'{folder}, base+{delta:#x}: {check(bytes(patched), delta)} Allow torrent cases pass')
    print('ALL ALLOW TORRENT CASES PASSED')


if __name__ == '__main__':
    main()
