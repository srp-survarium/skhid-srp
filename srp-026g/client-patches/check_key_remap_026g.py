#!/usr/bin/env python3
"""Patch 110 (key_remap): the keys a desktop moves are moved in the game
(docs/notes/keyboard_layout.md).

    python check_key_remap_026g.py <original survarium.exe>

keyboard::execute (0x3B8D20) runs whole, in the original and in the patched
exe, on a made-up platform keyboard whose DirectInput device (+914h) is
stubbed: GetDeviceData gives the poll's events, GetProperty(DIPROP_KEYNAME)
names a key by the layout the calling thread has (ActivateKeyboardLayout,
stubbed, sets it), as Wine's dinput does (GetKeyNameTextW). The layout the
window was told of is given to the patched exe through the real
message_processor (0x181E0, patch 107's site). kernel32's GetModuleHandleA and
GetProcAddress say Wine (ntdll's wine_get_version) or Windows. The CRT's
wcstombs_s (0x420C5F, which get_dik_name calls) is stubbed for ASCII.
The keyboard after a poll (the keys' states, the Shift/Ctrl/Alt flags, the
events, their count) is compared, with the patched exe given the physical
keys, to the original's given the keys their names say:
1. no layout told: nothing moves, no layout lent, no name read;
2. Wine, caps:swapescape: the Caps Lock key is Escape and Escape Caps Lock,
   a letter and Left Ctrl stay; the names read while the told layout is the
   thread's (ActivateKeyboardLayout(hkl, 0), then its own back);
3. the next poll in that layout reads no name again;
4. a new layout, ctrl:nocaps: the Caps Lock key is Left Ctrl (its flag set);
5. a layout the system refuses: nothing moves;
6. Windows: nothing moves, no layout lent, no name read, whatever is told;
7. execute returns as the original (ESP, EBX ESI EDI EBP kept).
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

EXECUTE, WINDOW_PROC, FOCUS_CHECK, WCSTOMBS_S = 0x3B8D20, 0x181E0, 0x1EE7C0, 0x420C5F
KEYBOARD_VTABLE = 0xA25478
ENGINE_SLOT = 0x1F1D310                          # engine_world*; its +2D0h the game (survarium::game)
# kernel32 and user32 (delay-loaded) slots -> (name, stack arguments)
IMPORTS = {0x870118: ("GetCurrentThreadId", 0), 0x87003C: ("GetModuleHandleA", 1),
           0x870084: ("GetProcAddress", 2), 0xAEFF1C: ("DefWindowProcA", 4),
           0xAEFF20: ("ActivateKeyboardLayout", 2), 0xAEFF5C: ("GetKeyState", 1)}
DEVICE_SLOTS = {0x14: ("GetProperty", 3), 0x1C: ("Acquire", 1), 0x28: ("GetDeviceData", 5)}
WM_INPUTLANGCHANGEREQUEST = 0x50
DIPROP_KEYNAME, DIERR_NOTFOUND = 0x14, 0x80070002

THREAD_HKL, SWAP, NOCAPS, REFUSED = 0x04090409, 0x04150409, 0x04190409, 0x04220409
ESC, CTRL, ALT, CAPS, W, RCTRL, RALT = 0x01, 0x1D, 0x38, 0x3A, 0x11, 0x9D, 0xB8
# The names DirectInput gives a key in a thread's layout: Wine's Wayland driver's keysyms; the
# thread's own layout (none of the session's) Wine's built-in US names.
NAMES = {
    THREAD_HKL: {ESC: "Esc", CAPS: "Caps Lock", CTRL: "Ctrl", ALT: "Alt", W: "W"},
    SWAP: {ESC: "Caps_Lock", CAPS: "Escape", CTRL: "Control_L", RCTRL: "Control_R", ALT: "Alt_L",
           RALT: "ISO_Level3_Shift", 0x0E: "BackSpace", 0x0F: "Tab", 0x1C: "Return", W: "w"},
    NOCAPS: {ESC: "Escape", CAPS: "Control_L", CTRL: "Control_L", ALT: "Alt_L", W: "Cyrillic_tse"},
}

DATA = 0x30000000
ENGINE, GAME, VTABLE, KEYBOARD, DEVICE, DEVICE_VTABLE = (DATA + 0x10000 * i for i in range(6))
STUBS = DATA + 0x60000
RETURN, NTDLL, WINE_GET_VERSION = STUBS, 0x7BC00000, 0x7BC10000
IMPORT_STUBS, DEVICE_STUBS = STUBS + 0x100, STUBS + 0x400   # one address each, 0x10 apart
STACK = 0x21000000
KEPT = {UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_ESI: 0x05050505, UC_X86_REG_EDI: 0x0E0E0E0E,
        UC_X86_REG_EBP: 0x0019FF00}


class Run:
    def __init__(self, data, wine):
        self.wine = wine
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(DATA, 0x70000, UC_PROT_ALL)
        emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
        self.stubs = {}
        for i, (slot, (name, count)) in enumerate(sorted(IMPORTS.items())):
            self.stub_at(IMPORT_STUBS + 0x10 * i, name, count)
            self.word(slot, IMPORT_STUBS + 0x10 * i)
        for i, (slot, (name, count)) in enumerate(sorted(DEVICE_SLOTS.items())):
            self.stub_at(DEVICE_STUBS + 0x10 * i, name, count)
            self.word(DEVICE_VTABLE + slot, DEVICE_STUBS + 0x10 * i)
        emu.hook_add(UC_HOOK_CODE, self.wcstombs_s, begin=WCSTOMBS_S, end=WCSTOMBS_S)
        emu.hook_add(UC_HOOK_CODE, self.done, begin=RETURN, end=RETURN)
        self.word(ENGINE_SLOT, ENGINE)
        self.word(ENGINE + 0x2D0, GAME)
        self.word(GAME, VTABLE)
        self.word(VTABLE + 0x44, FOCUS_CHECK)
        self.word(KEYBOARD, KEYBOARD_VTABLE)
        self.word(KEYBOARD + 0x914, DEVICE)
        self.word(DEVICE, DEVICE_VTABLE)
        self.layout = THREAD_HKL                 # the polling thread's
        self.events = []
        self.calls = []

    def stub_at(self, address, name, count):
        self.stubs[address] = (name, count)
        self.emu.hook_add(UC_HOOK_CODE, self.stub, begin=address, end=address)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def string(self, at):
        out = bytearray()
        while (b := self.emu.mem_read(at + len(out), 1)[0]) != 0:
            out.append(b)
        return out.decode()

    def stub(self, emu, address, size, _):
        name, count = self.stubs[address]
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 + 4 * i) for i in range(count)]
        result = 0
        if name == "GetCurrentThreadId":
            result = 1
        elif name == "GetModuleHandleA":
            assert self.string(args[0]).lower() == "ntdll.dll", args
            result = NTDLL
            args = [self.string(args[0])]
        elif name == "GetProcAddress":
            assert args[0] == NTDLL and self.string(args[1]) == "wine_get_version", args
            result = WINE_GET_VERSION if self.wine else 0
            args = [args[0], self.string(args[1])]
        elif name == "ActivateKeyboardLayout":
            assert args[1] == 0, args
            if args[0] != REFUSED:
                result, self.layout = self.layout, args[0]
        elif name == "GetDeviceData":
            device, size_, buffer, count_at, flags = args
            assert (device, size_, buffer, flags) == (DEVICE, 0x14, KEYBOARD + 0x408, 0), args
            assert self.read(count_at) == 0x40
            for i, (key, down) in enumerate(self.events):
                emu.mem_write(buffer + 0x14 * i, struct.pack("<5I", key, 0x80 if down else 0, 1000 + i, i, 0))
            self.word(count_at, len(self.events))
        elif name == "GetProperty":
            device, guid, header = args
            size_, header_size, key, how = struct.unpack("<4I", emu.mem_read(header, 16))
            assert (device, guid, size_, header_size, how) == (DEVICE, DIPROP_KEYNAME, 0x218, 0x10, 1), args
            named = NAMES.get(self.layout, {}).get(key)
            if named is None:
                result = DIERR_NOTFOUND
            else:
                emu.mem_write(header + 0x10, (named + "\0").encode("utf-16-le"))
            args = [key, self.layout]
        self.calls.append((name, *args))
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + 4 * count)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def wcstombs_s(self, emu, address, size, _):
        """errno_t wcstombs_s(size_t *count, char *out, size_t room, const wchar_t *text, size_t most), cdecl."""
        sp = emu.reg_read(UC_X86_REG_ESP)
        count_at, out, room, text, most = (self.read(sp + 4 + 4 * i) for i in range(5))
        chars = []
        while len(chars) < min(room - 1, most) and (c := struct.unpack("<H", emu.mem_read(text + 2 * len(chars), 2))[0]):
            assert c < 0x80
            chars.append(c)
        emu.mem_write(out, bytes(chars) + b"\0")
        if count_at:
            self.word(count_at, len(chars) + 1)
        emu.reg_write(UC_X86_REG_EAX, 0)
        emu.reg_write(UC_X86_REG_ESP, sp + 4)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def done(self, emu, address, size, _):
        emu.emu_stop()

    def call(self, function, args, ecx=0):
        """Run one function to its return: EAX and what it called."""
        sp = STACK + 0x8000
        self.emu.mem_write(sp, struct.pack(f"<{1 + len(args)}I", RETURN, *args))
        for reg, value in KEPT.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ECX, ecx)
        self.emu.reg_write(UC_X86_REG_EAX, 0xA0A0A0A0)
        self.emu.reg_write(UC_X86_REG_EDX, 0xD0D0D0D0)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.calls = []
        self.emu.emu_start(function, 0, count=2000000)
        assert self.emu.reg_read(UC_X86_REG_EIP) == RETURN
        assert self.emu.reg_read(UC_X86_REG_ESP) == sp + 4 + 4 * len(args), "pops its arguments"
        for reg, value in KEPT.items():
            assert self.emu.reg_read(reg) == value, hex(reg)
        return self.emu.reg_read(UC_X86_REG_EAX), self.calls

    def tell(self, hkl):
        """The window told of a layout, no text input focused (dropped by the game, kept by patch 107)."""
        self.word(GAME + 0x586C, 0)
        assert self.call(WINDOW_PROC, [0x00010042, WM_INPUTLANGCHANGEREQUEST, 0, hkl]) == (0, [])

    def poll(self, events):
        """One keyboard::execute with these events: the keyboard after it, and the calls but
        DirectInput's poll."""
        self.events = events
        _, calls = self.call(EXECUTE, [], ecx=KEYBOARD)
        assert self.layout == THREAD_HKL, "the polling thread has its own layout back"
        count = self.read(KEYBOARD + 0x404)
        assert count == len(events)
        keyboard = (bytes(self.emu.mem_read(KEYBOARD + 4, 0x400)),                 # the keys' states
                    bytes(self.emu.mem_read(KEYBOARD + 0x408, 0x14 * count)),     # this poll's events
                    self.read(KEYBOARD + 0x908))                                  # the Shift/Ctrl/Alt/Caps flags
        return keyboard, [c for c in calls if c[0] not in ("GetDeviceData", "GetKeyState")]


PHYSICAL = [(CAPS, True), (W, True), (ESC, True), (CTRL, True)]
SWAPPED = [(ESC, True), (W, True), (CAPS, True), (CTRL, True)]
RELEASED = [(CAPS, False), (W, False), (ESC, False), (CTRL, False)]
SWAPPED_RELEASED = [(ESC, False), (W, False), (CAPS, False), (CTRL, False)]
ASKED = [("GetModuleHandleA", "ntdll.dll"), ("GetProcAddress", NTDLL, "wine_get_version")]   # Wine or Windows
NAME_READS = [ESC, 0x0E, 0x0F, 0x1C, CTRL, 0x2A, 0x36, ALT, CAPS, RCTRL, RALT, 0xDB, 0xDC, 0xDD]


def lent(hkl):
    """What reading the names in `hkl` calls: the layout lent, each function key's name, its own back."""
    return ([("ActivateKeyboardLayout", hkl, 0)] + [("GetProperty", key, hkl) for key in NAME_READS]
            + [("ActivateKeyboardLayout", THREAD_HKL, 0)])


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    old, new = Run(original, wine=True), Run(bytes(built), wine=True)

    for events in (PHYSICAL, RELEASED):
        keyboard, calls = old.poll(events)
        assert calls == [] and new.poll(events) == (keyboard, [])
    print("ok no layout told: the keys are DirectInput's, no layout lent, no name read")

    new.tell(SWAP)
    state, calls = new.poll(PHYSICAL)
    assert calls == ASKED + lent(SWAP), calls
    assert state == old.poll(SWAPPED)[0]
    assert [new.read(KEYBOARD + 0x408 + 0x14 * i) for i in range(4)] == [ESC, W, CAPS, CTRL], \
        "the events themselves are moved, for process too"
    print("ok Wine, caps:swapescape: Caps Lock is Escape, Escape Caps Lock, W and Left Ctrl stay; names read in "
          "the told layout, the thread's own back")

    assert new.poll(RELEASED) == (old.poll(SWAPPED_RELEASED)[0], [])
    print("ok the next poll in that layout reads no name again")

    new.tell(NOCAPS)
    state, calls = new.poll([(CAPS, True)])
    assert calls == ASKED + lent(NOCAPS), calls
    assert state == old.poll([(CTRL, True)])[0]
    assert new.read(KEYBOARD + 0x908) & 2, "Ctrl's flag"
    assert new.poll([(CAPS, False)])[0] == old.poll([(CTRL, False)])[0]
    print("ok a new layout, ctrl:nocaps: Caps Lock is Left Ctrl, its flag set")

    new.tell(REFUSED)
    state, calls = new.poll(PHYSICAL)
    assert calls == ASKED + [("ActivateKeyboardLayout", REFUSED, 0)], calls
    assert state == old.poll(PHYSICAL)[0]
    assert new.poll(RELEASED) == (old.poll(RELEASED)[0], [])
    print("ok a layout the system refuses: nothing moves, no name read")

    old, new = Run(original, wine=False), Run(bytes(built), wine=False)
    for hkl in (SWAP, NOCAPS):
        new.tell(hkl)
        assert new.poll(PHYSICAL) == (old.poll(PHYSICAL)[0], ASKED)
        assert new.poll(RELEASED) == (old.poll(RELEASED)[0], [])
    print("ok Windows: nothing moves, no layout lent, no name read")
    print("ok every poll returned as the original, kept EBX ESI EDI EBP")


if __name__ == "__main__":
    main()
