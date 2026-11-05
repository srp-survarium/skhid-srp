#!/usr/bin/env python3
"""Patch 110 (MOUSE_PITCH): the mouse look's pitch is a 4:3 window's at every
window (client_patch.md section 3.69, ../docs/notes/mouse-pitch.md).

    python check_mouse_pitch_026g.py <original survarium.exe>

player_input_handler::on_mouse_move (0x240200) runs, in the original and in
the built exe, from its start to the wheel's store (0x2403E7) on a made-up
handler (its +334h the game world, whose +9Ch is the game, whose +5348h is
the render window: vtable+1Ch stubbed to give the window's width and height
as floats, thiscall, `ret 4`), the half-float pack (0xF65A0) and unpack
(0xF66D0) the game's own. The pack's argument is the new yaw (the first
call) and the new pitch (the second), floats; the handler's turn starts at 0,
so they are the move's turn. For 4:3, 5:4, 16:10, 16:9, 21:9 and 32:9
windows, moves (x, y), the warmup's mode (3) and the first person's (0), an
inverted mouse, another sensitivity and an aimed weapon's factor (+94h):
- the yaw is the same float in both exes at every window;
- the original's pitch is the 4:3 window's times (h/w) / 0.75;
- the build's pitch at every window is the original's at 4:3, bit for bit,
  so the pitch's radians a count over the yaw's are the same at every
  window: 0.75 up and 0.84375 down (a look-pitch unit is 60 degrees of view
  up, 67.5 down: the look clip, abi.tsv 0x2FBF90);
- the spectator's mode (4) turns nothing in either.
Says nothing about the render window's real size or the look clip.
"""
import math
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_ESP, UC_X86_REG_EIP

import patch_026g as base

START, END = 0x240200, 0x2403E7               # on_mouse_move; `mov eax,esi` after the wheel's store
PACK = 0xF65A0                                # float -> half, cdecl (the float on the stack)
SENSITIVITY, INVERT, JOURNAL = 0xA74AEC, 0x1F1D9D3, 0xA8394C
OBJECTS = 0x40000000
HANDLER, WORLD, GAME, RENDER, RVT = (OBJECTS + 0x1000 * i for i in range(5))
SIZE_STUB = OBJECTS + 0x8000
RETURN = OBJECTS + 0x8010                     # the caller: a move that turns nothing returns here
STACK = 0x41000000
SP = STACK + 0x8000
# (name, width, height)
WINDOWS = (("4:3", 1024, 768), ("4:3", 1600, 1200), ("5:4", 1280, 1024), ("16:10", 1680, 1050),
           ("16:9", 1920, 1080), ("16:9", 1366, 768), ("21:9", 2560, 1080), ("21:9", 3440, 1440),
           ("32:9", 5120, 1440))
# (mode, x, y, invert, sensitivity, the handler's +94h)
MOVES = ((0, 1, 1, 0, 1.0, 1.0), (0, 7, -3, 0, 1.0, 1.0), (0, -40, 25, 1, 1.0, 1.0), (0, 13, 9, 0, 0.37, 1.0),
         (0, 5, -11, 0, 1.0, 0.25), (3, 12, 6, 0, 1.0, 1.0))
UP, DOWN = math.pi / 3, 3 * math.pi / 8        # radians of view a look-pitch unit, up and down


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(OBJECTS, 0x10000, UC_PROT_ALL)
        emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
        emu.hook_add(UC_HOOK_CODE, self.size, begin=SIZE_STUB, end=SIZE_STUB)
        emu.hook_add(UC_HOOK_CODE, self.pack, begin=PACK, end=PACK)
        emu.hook_add(UC_HOOK_CODE, self.end, begin=END, end=END)
        emu.hook_add(UC_HOOK_CODE, self.end, begin=RETURN, end=RETURN)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def size(self, emu, address, _size, _user):
        sp = emu.reg_read(UC_X86_REG_ESP)
        assert emu.reg_read(UC_X86_REG_ECX) == RENDER
        out = self.read(sp + 4)
        emu.mem_write(out, struct.pack("<ff", self.window[0], self.window[1]))
        self.sizes += 1
        emu.reg_write(UC_X86_REG_EAX, out)
        emu.reg_write(UC_X86_REG_ESP, sp + 8)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def pack(self, emu, address, _size, _user):
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.packed.append(struct.unpack("<f", emu.mem_read(sp + 4, 4))[0])

    def end(self, emu, address, _size, _user):
        emu.emu_stop()

    def move(self, window, mode, x, y, invert, sensitivity, factor):
        """The pack's floats (the new yaw, the new pitch) and the window
        size's calls of one move."""
        emu = self.emu
        emu.mem_write(OBJECTS, bytes(0x8000))
        emu.mem_write(STACK, bytes(0x10000))
        emu.mem_write(SIZE_STUB, b"\xcc")
        emu.mem_write(RETURN, b"\xcc")
        self.word(HANDLER + 0x358, mode)
        emu.mem_write(HANDLER + 0x94, struct.pack("<f", factor))
        self.word(HANDLER + 0x334, WORLD)
        self.word(WORLD + 0x9C, GAME)
        self.word(GAME + 0x5348, RENDER)
        self.word(RENDER, RVT)
        self.word(RVT + 0x1C, SIZE_STUB)
        emu.mem_write(INVERT, bytes([invert]))
        emu.mem_write(SENSITIVITY, struct.pack("<f", sensitivity))
        self.word(JOURNAL, 0)                      # no input journal: the mouse's own step
        self.word(SP, RETURN)                      # the return address, then the input world, x, y, z
        for i, value in enumerate((0, x, y, 0)):
            self.word(SP + 4 + 4 * i, value & 0xFFFFFFFF)
        emu.reg_write(UC_X86_REG_ESP, SP)
        emu.reg_write(UC_X86_REG_ECX, HANDLER)
        self.window, self.packed, self.sizes = window, [], 0
        emu.emu_start(START, 0, count=20000)
        eip = emu.reg_read(UC_X86_REG_EIP)
        return self.packed, self.sizes, eip


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    old, new = Run(original), Run(bytes(built))
    for mode, x, y, invert, sensitivity, factor in MOVES:
        reference, _, _ = old.move((1024.0, 768.0), mode, x, y, invert, sensitivity, factor)
        for name, w, h in WINDOWS:
            o_packed, o_sizes, o_eip = old.move((float(w), float(h)), mode, x, y, invert, sensitivity, factor)
            n_packed, n_sizes, n_eip = new.move((float(w), float(h)), mode, x, y, invert, sensitivity, factor)
            case = (name, w, h, mode, x, y, invert, sensitivity, factor)
            assert o_eip == n_eip == END and o_sizes == n_sizes == 2 and len(o_packed) == len(n_packed) == 2, \
                (case, hex(o_eip), hex(n_eip), o_sizes, n_sizes, o_packed, n_packed)
            (o_yaw, o_pitch), (n_yaw, n_pitch) = o_packed, n_packed
            assert struct.pack("<f", o_yaw) == struct.pack("<f", n_yaw), (case, o_yaw, n_yaw)
            assert struct.pack("<f", n_pitch) == struct.pack("<f", reference[1]), (case, n_pitch, reference)
            ratio = f32(f32(h) / f32(w)) / 0.75
            assert abs(o_pitch - reference[1] * ratio) <= 1e-6 * abs(reference[1]), (case, o_pitch, reference)
            yaw_rad = abs(n_yaw / x)
            n_up, o_up = abs(n_pitch / y) * UP / yaw_rad, abs(o_pitch / y) * UP / yaw_rad
            assert abs(n_up - 0.75) < 1e-5 and abs(abs(n_pitch / y) * DOWN / yaw_rad - 0.84375) < 1e-5, (case, n_up)
            if mode == 0 and (x, y) == (1, 1):
                print(f"ok {name:>5} {w}x{h}: yaw {yaw_rad:.9f} rad/count; pitch / yaw, up: "
                      f"original {o_up:.4f}, build {n_up:.4f} (down x 1.125)")
        print(f"ok mode {mode}, move ({x}, {y}), invert {invert}, sensitivity {sensitivity}, factor {factor}: "
              f"the yaw as before; the pitch the 4:3 window's at all {len(WINDOWS)} windows")
    for run in (old, new):
        packed, sizes, eip = run.move((1920.0, 1080.0), 4, 9, 9, 0, 1.0, 1.0)
        assert packed == [] and sizes == 0 and eip == RETURN, (packed, sizes, hex(eip))
    print("ok mode 4 (a spectator's): no turn in either")


if __name__ == "__main__":
    main()
