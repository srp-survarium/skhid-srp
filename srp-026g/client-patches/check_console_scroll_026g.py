#!/usr/bin/env python3
"""Patch 159 (the Rust patch console_scroll, rust/patches/src/console_scroll.rs):
the console's output scrolls (docs/notes/console-scroll.md, client_patch.md
section 3.85).

    python check_console_scroll_026g.py <original survarium.exe>

Requires Python `unicorn`. The original exe and the build, each with its
sections and arena as they are. The game's own code makes the console's
scroll view (ui_world::create_scroll_view 0x3466A0: the constructor, its pad,
bar and v scroll) and runs the console's three functions the patch hooks on a
made-up console around it: console_impl::on_keyboard_action (0x1EAA0),
game_console::on_mouse_move (0x1C810) and game_console::on_activate
(0x1C760). The view is left as console_impl::tick leaves it (its step 20, the
pad a log's lines long, at its end while following); the tick itself is not
run (it reads the log file). Stubbed: the allocator (a bump allocator), the
console's dialog and its input handler (they take nothing), its text edit
(an empty line) and the input world.

Checked:
- in both exes every key and mouse move the console is given is taken (AL 1)
  and EBX, ESI, EDI, EBP kept; the original never moves the view;
- the build: the wheel three lines a notch (a small turn one), Page Up and
  Page Down a page (the shown lines less one), the follow flag cleared while
  up; down to the end, the flag set and the pad at the end; End, Enter and
  opening the console back to the newest; a held or released key, other
  keys, a wheel-less move: nothing; the top stops at the first line; new
  lines while scrolled up (a longer pad) move the end the wheel reaches;
- the view's state read back through the game's own getters
  (get_follow_last_line 0x349520, the v scroll's get_position 0x349C90 and
  get_length 0x349C80): the fields the patch reads and the engine agree.
"""
import struct
import sys

from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI, UC_X86_REG_EIP,
                               UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_EBP)

import patch_026g as base

CREATE_SCROLL_VIEW = 0x3466A0
ON_KEY, ON_MOUSE_MOVE, ON_ACTIVATE = 0x1EAA0, 0x1C810, 0x1C760
GET_FOLLOW, GET_POSITION, GET_LENGTH, SET_STEP = 0x349520, 0x349C90, 0x349C80, 0x349510
HANDLER_VTABLE = 0x9DC030                       # the console's input handler, at its +268h

STACK, HEAP, OBJECTS, STUBS, RETURN = 0x20000000, 0x21000000, 0x22000000, 0x23000000, 0x25000000
CONSOLE = OBJECTS + 0x1000                      # 270h bytes
(WORLD, ALLOCATOR, ALLOCATOR_VT, DIALOG, DIALOG_VT, DIALOG_INPUT, DIALOG_INPUT_VT, EDIT, EDIT_VT, EDIT_TEXT,
 EDIT_TEXT_VT, EDIT_WINDOW, EDIT_WINDOW_VT, INPUT_WORLD, INPUT_WORLD_VT, EMPTY, READ) = (
    OBJECTS + 0x100 * i for i in range(17))
KEPT = {UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_ESI: 0x05050505, UC_X86_REG_EDI: 0x0E0E0E0E,
        UC_X86_REG_EBP: 0x0019FF00}

DOWN, UP, HELD = 1, 2, 3
PAGE_UP, PAGE_DOWN, END, HOME, RETURN_KEY, LEFT, A = 0xC9, 0xD1, 0xCF, 0xC7, 0x1C, 0xCB, 0x1E
HEIGHT, LINE = 354.0, 20.0                       # the view at 768: 17 lines of 20
SHOWN = 17


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((STACK, 0x20000), (HEAP, 0x100000), (OBJECTS, 0x2000), (STUBS, 0x1000),
                         (RETURN, 0x1000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        self.heap, self.sizes, self.calls, self.stubs = HEAP, {}, [], {}

        self.word(WORLD + 0x10, ALLOCATOR)
        self.word(ALLOCATOR, ALLOCATOR_VT)
        for slot in range(0, 0x40, 4):
            self.word(ALLOCATOR_VT + slot, self.stub(f"allocator+{slot:#x}", 0, None))
        self.word(ALLOCATOR_VT + 0x10, self.stub("malloc", 0x14, self.malloc))
        self.word(ALLOCATOR_VT + 0x14, self.stub("realloc", 0x18, self.realloc))
        self.word(ALLOCATOR_VT + 0x18, self.stub("free", 0x10, lambda args: 0))
        # the console's dialog: its input handler takes nothing (ui_dialog's answer 0)
        self.vtable(DIALOG, DIALOG_VT, {4: ("dialog.input_handler", 0, lambda args: DIALOG_INPUT)})
        self.vtable(DIALOG_INPUT, DIALOG_INPUT_VT, {0: ("dialog.key", 0xC, lambda args: 0),
                                                    0xC: ("dialog.mouse_move", 0x10, lambda args: 0)})
        # its line, empty; its window's focus
        self.vtable(EDIT, EDIT_VT, {0x10: ("edit.text", 0, lambda args: EDIT_TEXT),
                                    0x14: ("edit.w", 0, lambda args: EDIT_WINDOW)})
        self.vtable(EDIT_TEXT, EDIT_TEXT_VT, {0xC: ("text.get_text", 0, lambda args: EMPTY)})
        self.vtable(EDIT_WINDOW, EDIT_WINDOW_VT, {0x34: ("window.set_focused", 4, lambda args: 0),
                                                  0x38: ("window.get_focused", 0, lambda args: 0)})
        self.vtable(INPUT_WORLD, INPUT_WORLD_VT, {0x10: ("input.add_handler", 4, lambda args: 0)})
        emu.hook_add(UC_HOOK_CODE, self.on_stub, begin=STUBS, end=STUBS + 0xFFF)

        self.view = self.call(CREATE_SCROLL_VIEW, [], ecx=WORLD)
        assert self.view, "create_scroll_view made a view"
        self.word(CONSOLE + 0x4, 1)                     # m_self_deactivate
        for offset, value in ((0x14, DIALOG), (0x18, self.view), (0x1C, EDIT), (0x268, HANDLER_VTABLE),
                              (0x26C, INPUT_WORLD)):
            self.word(CONSOLE + offset, value)

    def stub(self, name, pops, action):
        at = STUBS + 0x10 * len(self.stubs)
        self.stubs[at] = (name, pops, action)
        return at

    def vtable(self, obj, table, slots):
        self.word(obj, table)
        for slot in range(0, 0x40, 4):
            name, pops, action = slots.get(slot, (f"{obj:#x}+{slot:#x}", 0, None))
            self.word(table + slot, self.stub(name, pops, action))

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def float(self, at):
        return struct.unpack("<f", self.emu.mem_read(at, 4))[0]

    def set_float(self, at, value):
        self.emu.mem_write(at, struct.pack("<f", value))

    def on_stub(self, emu, address, size, _):
        name, pops, action = self.stubs[address]
        sp = emu.reg_read(UC_X86_REG_ESP)
        if action is None:
            raise AssertionError(f"unexpected call of {name} from {self.read(sp):#x}")
        args = [self.read(sp + 4 + 4 * i) for i in range(pops // 4)]
        self.calls.append(name)
        emu.reg_write(UC_X86_REG_EAX, action(args))
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def malloc(self, args):
        at, self.heap = self.heap, (self.heap + args[0] + 0x1F) & ~0xF
        assert self.heap < HEAP + 0x100000, "the bump allocator's room"
        self.emu.mem_write(at, bytes(args[0]))
        self.sizes[at] = args[0]
        return at

    def realloc(self, args):
        old, size = args[0], args[1]
        at = self.malloc([size])
        if old:
            self.emu.mem_write(at, bytes(self.emu.mem_read(old, min(size, self.sizes[old]))))
        return at

    def call(self, function, args, ecx=0, eax=None):
        sp = STACK + 0x10000
        self.emu.mem_write(sp, struct.pack(f"<{1 + len(args)}I", RETURN, *args))
        for reg, value in KEPT.items():
            self.emu.reg_write(reg, value)
        if eax is not None:
            self.emu.reg_write(UC_X86_REG_EAX, eax)
        self.emu.reg_write(UC_X86_REG_ECX, ecx)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        try:
            self.emu.emu_start(function, RETURN, count=2_000_000)
        except UcError as error:
            raise AssertionError(f"{function:#x}: {error} at {self.emu.reg_read(UC_X86_REG_EIP):#x}")
        assert self.emu.reg_read(UC_X86_REG_EIP) == RETURN, f"{function:#x} did not return"
        assert self.emu.reg_read(UC_X86_REG_ESP) == sp + 4 + 4 * len(args), f"{function:#x}: its pops"
        for reg, value in KEPT.items():
            assert self.emu.reg_read(reg) == value, f"{function:#x} kept {reg}"
        return self.emu.reg_read(UC_X86_REG_EAX)

    def st0(self, function, ecx):
        """A getter's x87 result: call function / fstp dword [READ], run to its end (each getter's
        code at its own place, written once: Unicorn keeps a block it has translated)."""
        code = RETURN + 0x800 + 0x20 * (GET_POSITION, GET_LENGTH).index(function)
        call = function - (code + 5)
        self.emu.mem_write(code, b"\xE8" + struct.pack("<i", call) + b"\xD9\x1D" + struct.pack("<I", READ))
        self.emu.reg_write(UC_X86_REG_ECX, ecx)
        self.emu.reg_write(UC_X86_REG_ESP, STACK + 0x10000)
        self.emu.emu_start(code, code + 11, count=10_000)
        return self.float(READ)

    # the view as console_impl::tick leaves it

    def filled(self, lines, top=None):
        """`lines` of the log in the pad as the tick fills it following (its length lines x 20 + 4 at
        768), the step 20 (set_step_size, the game's), the pad at its end (recalc's move_end) or at
        `top` pixels down, not following."""
        self.set_float(self.view + 0x18, HEIGHT)
        self.set_float(self.view + 0x5C, lines * LINE + 4.0)
        self.call(SET_STEP, [struct.unpack("<I", struct.pack("<f", LINE))[0]], ecx=self.view + 0x1C8)
        end = max(0.0, lines * LINE + 4.0 - HEIGHT)
        flags = self.read(self.view + 0x44) & 0xFFFF
        if top is None:
            self.set_float(self.view + 0x54, -end)
            flags |= 2
        else:
            self.set_float(self.view + 0x54, -top)
            flags &= ~2
        self.emu.mem_write(self.view + 0x44, struct.pack("<H", flags))
        return end

    def state(self):
        """(follows, top in pixels), read through the game's own getters."""
        follows = self.call(GET_FOLLOW, [], ecx=self.view) & 0xFF
        top = -self.st0(GET_POSITION, self.view + 0x1C8)
        assert top == -self.float(self.view + 0x54), "get_position is the pad's y the patch reads"
        assert self.st0(GET_LENGTH, self.view + 0x1C8) == self.float(self.view + 0x5C), "get_length"
        assert bool(follows) == bool(self.read(self.view + 0x44) & 2), "the flag the patch reads"
        return bool(follows), top + 0.0

    def key(self, code, kind=DOWN):
        taken = self.call(ON_KEY, [0x7000, code], ecx=CONSOLE, eax=kind) & 0xFF
        assert taken == 1, f"key {code:#x} kind {kind}: taken"
        return self.state()

    def wheel(self, z, x=0, y=0):
        move = [v & 0xFFFFFFFF for v in (x, y, z)]
        taken = self.call(ON_MOUSE_MOVE, [0x7000, *move], ecx=CONSOLE + 0x268) & 0xFF
        assert taken == 1, f"wheel {z}: taken"
        return self.state()

    def activate(self):
        self.calls = []
        self.call(ON_ACTIVATE, [], ecx=CONSOLE)
        assert "input.add_handler" in self.calls, "the console's handler added"
        return self.state()


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    built = bytes(built)

    lines = 1000
    old = Run(original)
    end = old.filled(lines)
    assert end == 19_650.0 and old.state() == (True, end), "following, at the end"
    for step in (lambda: old.wheel(120), lambda: old.key(PAGE_UP), lambda: old.wheel(-120),
                 lambda: old.key(END), lambda: old.key(RETURN_KEY), old.activate):
        assert step() == (True, end)
    old.filled(lines, top=1000.0)
    assert old.key(PAGE_DOWN) == (False, 1000.0) and old.key(END) == (False, 1000.0)
    assert old.activate() == (False, 1000.0), "the original keeps a moved view where it was"
    print("ok the original: every key and wheel move taken (AL 1), the view never moved")

    r = Run(built)
    r.filled(lines)
    assert r.wheel(120) == (False, end - 3 * LINE), "a notch up: three lines, no longer following"
    assert r.wheel(240) == (False, end - 9 * LINE)
    assert r.wheel(15) == (False, end - 10 * LINE), "a small turn: one line"
    assert r.wheel(0, x=5, y=-3) == (False, end - 10 * LINE), "a move without the wheel: nothing"
    assert r.wheel(-120) == (False, end - 7 * LINE)
    assert r.wheel(-360) == (True, end), "down to the end: following again, the pad at the end"
    assert r.wheel(-120) == (True, end), "past the end: nothing"
    print("ok the wheel: three lines a notch, one for a small turn, following again at the end")

    page = (SHOWN - 1) * LINE
    assert r.key(PAGE_UP) == (False, end - page), "Page Up: the 17 lines shown less one"
    assert r.key(PAGE_UP) == (False, end - 2 * page)
    assert r.key(PAGE_UP, HELD) == (False, end - 2 * page), "held: no move"
    assert r.key(PAGE_UP, UP) == (False, end - 2 * page), "let go: no move"
    for other in (LEFT, A, HOME):
        assert r.key(other) == (False, end - 2 * page), f"key {other:#x}: no move"
    assert r.key(PAGE_DOWN) == (False, end - page)
    assert r.key(PAGE_DOWN) == (True, end), "Page Down to the end: following"
    print("ok Page Up and Page Down a page; held and released keys, Left, A, Home: nothing")

    r.key(PAGE_UP)
    assert r.key(END) == (True, end), "End: the newest"
    r.key(PAGE_UP)
    assert r.key(RETURN_KEY) == (True, end), "Enter: the newest"
    r.key(PAGE_UP)
    assert r.activate() == (True, end), "opening the console: the newest"
    print("ok End, Enter and opening the console go back to the newest line")

    r.filled(lines, top=2 * LINE)
    assert r.key(PAGE_UP) == (False, 0.0), "the top stops at the first line"
    assert r.wheel(120) == (False, 0.0)
    r.filled(lines, top=end - 5 * LINE)
    assert r.wheel(-240) == (True, end), "six lines down from five above the end: the newest"
    r.filled(lines + 50, top=end - 5 * LINE)    # 50 new lines while up: the tick lengthens the tail
    assert r.wheel(-240) == (False, end + LINE), "the end is the longer pad's: still held"
    print("ok the top stops at the first line; the end is where the log's new lines put it")

    r.filled(10)
    assert r.state() == (True, 0.0), "a short log: the pad shorter than the view"
    assert r.wheel(120) == (True, 0.0) and r.key(PAGE_UP) == (True, 0.0), "nothing to scroll"
    fresh = Run(built)
    fresh.set_float(fresh.view + 0x18, HEIGHT)
    assert fresh.wheel(120) == (True, 0.0), "a view the tick never filled (no step): nothing"
    assert fresh.activate() == (True, 0.0)
    print("ok a log the view holds, and a view never filled: nothing to scroll")
    print("patch 159: all checks passed")


if __name__ == "__main__":
    main()
