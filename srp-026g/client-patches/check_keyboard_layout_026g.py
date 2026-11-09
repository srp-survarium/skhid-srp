#!/usr/bin/env python3
"""Patch 107 (keyboard_layout): chat text follows the keyboard layout the
window was last told of (docs/notes/keyboard_layout.md).

    python check_keyboard_layout_026g.py <original survarium.exe>

message_processor (0x181E0) and keyboard::translate_text (0x3B92D0) run
whole, in the original and in the patched exe, on a made-up engine and
game (the game's own text-focus check, 0x1EE7C0, reads its +586Ch), the
user32 functions they call stubbed to record their arguments:
1. no layout told yet: both type with GetKeyboardLayout(0)'s HKL;
2. a WM_INPUTLANGCHANGEREQUEST with no text input focused: both drop it
   (return 0, no DefWindowProc); the patched exe types with its HKL from then
   on, the original with the thread's;
3. a WM_INPUTLANGCHANGE with a text input focused: both call
   ActivateKeyboardLayout(hkl, KLF_SETFORPROCESS) and DefWindowProc; the
   patched exe types with it; a request with the focus on goes to
   DefWindowProc in both, and is typed with by the patched exe;
4. an lParam of 0 is no layout: the last one stays;
5. both functions return as the original (EAX, the pops, EBX ESI EDI EBP
   kept), and the typed character is the stub's for the HKL it was given.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

WINDOW_PROC, TRANSLATE_TEXT, FOCUS_CHECK = 0x181E0, 0x3B92D0, 0x1EE7C0
ENGINE_SLOT = 0x1F1D310                          # engine_world*; its +2D0h the game (survarium::game)
THREAD_SLOT = 0x870118                           # kernel32 GetCurrentThreadId
# user32 (delay-loaded): slot -> (name, stack arguments)
USER32 = {0xAEFF1C: ("DefWindowProcA", 4), 0xAEFF20: ("ActivateKeyboardLayout", 2),
          0xAEFF4C: ("ToUnicodeEx", 7), 0xAEFF50: ("MapVirtualKeyExA", 3),
          0xAEFF54: ("GetKeyboardState", 1), 0xAEFF58: ("GetKeyboardLayout", 1),
          0xAEFF5C: ("GetKeyState", 1)}
WM_INPUTLANGCHANGEREQUEST, WM_INPUTLANGCHANGE = 0x50, 0x51
KLF_SETFORPROCESS = 0x100

THREAD_HKL, RUSSIAN, POLISH = 0x04090409, 0x04190409, 0x04150409
CHARACTER = {THREAD_HKL: ord("w"), RUSSIAN: 0x0446, POLISH: ord("W")}   # the stub's ToUnicodeEx: w, ts, W
DIK_W, VK_W = 0x11, 0x57
DEF_RESULT = 0xDEF0

DATA = 0x30000000
ENGINE, GAME, VTABLE, KEYBOARD, OUT = (DATA + 0x10000 * i for i in range(5))
STUBS = DATA + 0x60000
THREAD, RETURN = STUBS, STUBS + 0x20
USER32_STUBS = STUBS + 0x100                     # one address each, 0x10 apart
STACK = 0x21000000
KEPT = {UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_ESI: 0x05050505, UC_X86_REG_EDI: 0x0E0E0E0E,
        UC_X86_REG_EBP: 0x0019FF00}


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(DATA, 0x70000, UC_PROT_ALL)
        emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
        emu.mem_write(THREAD, bytes.fromhex("b801000000c3"))     # GetCurrentThreadId: mov eax,1 / ret
        self.word(THREAD_SLOT, THREAD)
        self.stubs = {}
        for i, (slot, (name, count)) in enumerate(sorted(USER32.items())):
            stub = USER32_STUBS + 0x10 * i
            self.word(slot, stub)
            self.stubs[stub] = (name, count)
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=stub, end=stub)
        emu.hook_add(UC_HOOK_CODE, self.done, begin=RETURN, end=RETURN)
        self.word(ENGINE_SLOT, ENGINE)
        self.word(ENGINE + 0x2D0, GAME)
        self.word(GAME, VTABLE)
        self.word(VTABLE + 0x44, FOCUS_CHECK)
        self.calls = []

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def stub(self, emu, address, size, _):
        name, count = self.stubs[address]
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 + 4 * i) for i in range(count)]
        result = 0
        if name == "GetKeyboardLayout":
            result = THREAD_HKL
        elif name == "GetKeyboardState":
            result = 1
        elif name == "MapVirtualKeyExA":
            assert args[:2] == [DIK_W, 3], args
            result = VK_W
        elif name == "ToUnicodeEx":
            vk, scan, _, buf, room, flags, hkl = args
            assert (vk, scan, room, flags) == (VK_W, DIK_W, 3, 0), args
            emu.mem_write(buf, struct.pack("<H", CHARACTER[hkl]))
            result = 1
        elif name == "DefWindowProcA":
            result = DEF_RESULT
        elif name == "ActivateKeyboardLayout":
            result = args[0]
        self.calls.append((name, *args))
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + 4 * count)
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

    def message(self, message, hkl, focused):
        """The window procedure given a layout message: its result and calls."""
        self.word(GAME + 0x586C, 1 if focused else 0)
        return self.call(WINDOW_PROC, [0x00010042, message, 0xCC, hkl])

    def type_w(self):
        """W typed: the HKL the text was made with, and the character."""
        self.emu.mem_write(KEYBOARD + 0x90C, bytes(2))
        self.word(OUT, 0)
        result, calls = self.call(TRANSLATE_TEXT, [DIK_W, OUT], ecx=KEYBOARD)
        assert result & 0xFF == 1, "a character"
        layouts = {c[-1] for c in calls if c[0] in ("MapVirtualKeyExA", "ToUnicodeEx")}
        assert len(layouts) == 1, calls
        (hkl,) = layouts
        assert self.read(OUT) & 0xFFFF == CHARACTER[hkl]
        return hkl


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, True)
    old, new = Run(original), Run(bytes(built))

    assert old.type_w() == THREAD_HKL and new.type_w() == THREAD_HKL
    print("ok no layout told: both type with GetKeyboardLayout(0)'s")

    for run in (old, new):
        assert run.message(WM_INPUTLANGCHANGEREQUEST, RUSSIAN, focused=False) == (0, [])
    assert old.type_w() == THREAD_HKL
    assert new.type_w() == RUSSIAN
    print("ok a request with no text input focused: dropped by both; the patched exe types with it")

    for run in (old, new):
        assert run.message(WM_INPUTLANGCHANGE, POLISH, focused=True) == (
            DEF_RESULT, [("ActivateKeyboardLayout", POLISH, KLF_SETFORPROCESS),
                         ("DefWindowProcA", 0x00010042, WM_INPUTLANGCHANGE, 0xCC, POLISH)])
    assert old.type_w() == THREAD_HKL and new.type_w() == POLISH
    for run in (old, new):
        assert run.message(WM_INPUTLANGCHANGE, RUSSIAN, focused=False) == (
            DEF_RESULT, [("DefWindowProcA", 0x00010042, WM_INPUTLANGCHANGE, 0xCC, RUSSIAN)])
        assert run.message(WM_INPUTLANGCHANGEREQUEST, POLISH, focused=True) == (
            DEF_RESULT, [("DefWindowProcA", 0x00010042, WM_INPUTLANGCHANGEREQUEST, 0xCC, POLISH)])
    assert old.type_w() == THREAD_HKL and new.type_w() == POLISH
    print("ok a change (focus on: ActivateKeyboardLayout and DefWindowProc in both; off: DefWindowProc) and a "
          "focused request: the same in both; the patched exe types with the last")

    for run in (old, new):
        assert run.message(WM_INPUTLANGCHANGEREQUEST, 0, focused=False) == (0, [])
    assert new.type_w() == POLISH and old.type_w() == THREAD_HKL
    print("ok an lParam of 0 is no layout: the last stays")
    print("ok every run returned as the original, popped its arguments, kept EBX ESI EDI EBP")


if __name__ == "__main__":
    main()
