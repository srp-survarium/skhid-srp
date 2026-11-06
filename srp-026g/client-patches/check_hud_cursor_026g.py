#!/usr/bin/env python3
"""Patch 113 (hud_cursor): the hud's kits and cursor start over at a new match,
and the main menu covers the hud's cursor (README "The hud's kits and cursor").

    python check_hud_cursor_026g.py <original survarium.exe>

The original and the patched exe run on a made-up game (its game world inside
it at +18h, the hud at the world's +1FCh, the main menu at +57F4h), the
scene's movie calls (base_game_scene::show_movie 0x23DC00, hide_movie
0x23DC70) stubbed to record what they were given:
1. show_cursor (0x23A930) whole: with the menu closed both show the hud's
   cursor movie; with it shown only the original does; both keep the flag;
2. game_options::activate's start (0x200F50, to the end of the span its hook
   replays): over a running match with the hud's cursor shown the patched exe
   hides the cursor movie; not with the cursor hidden, nor when the game world
   is not active (the lobby);
3. game_options::deactivate's start (0x201040, to its je): a shown menu over
   a running match with the cursor shown puts the movie back; nothing when the
   menu is not shown, the world not active (a match's end), the cursor hidden
   or its movie not loaded; the flags of its cmp as the original's;
4. initialize_match's start (0x234880): the patched exe hides the cursor
   (the movie, +2DCh 0) and the kits (+2DFh 0) the last match left;
5. every span returns where the original's does, the registers as its.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EFLAGS, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

SHOW_CURSOR, SHOW_MOVIE, HIDE_MOVIE = 0x23A930, 0x23DC00, 0x23DC70
ACTIVATE, ACTIVATE_AFTER = 0x200F50, 0x200F56
DEACTIVATE, DEACTIVATE_JE = 0x201040, 0x201045
INITIALIZE, INITIALIZE_AFTER = 0x234880, 0x234886

DATA = 0x30000000
GAME = DATA                                      # survarium::game, 0x6000 bytes and more
WORLD = GAME + 0x18                              # its game world (a base_game_scene)
HUD = WORLD + 0x1FC                              # the world's hud
OPTIONS = GAME + 0x57F4                          # the main menu
CURSOR_MOVIE = HUD + 0xC                         # the hud's cursor movie (a pointer to its holder)
SCREEN = DATA + 0x10000                          # game+5348h: vtable+1Ch the screen size
WORLD_VTABLE, SCREEN_VTABLE = DATA + 0x11000, DATA + 0x12000
HOLDER, SIZE = DATA + 0x13000, DATA + 0x14000
STUBS = DATA + 0x20000
SIZE_STUB, POSITION_STUB, RETURN = STUBS, STUBS + 0x10, STUBS + 0x20
STACK = 0x21000000
REGS = {UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_EBP: 0x0019FF00, UC_X86_REG_ECX: 0x0C0C0C0C,
        UC_X86_REG_EDX: 0x0D0D0D0D, UC_X86_REG_EDI: 0x0E0E0E0E}
ZF = 1 << 6


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(DATA, 0x30000, UC_PROT_ALL)
        emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
        emu.mem_write(SIZE_STUB, b"\xb8" + struct.pack("<I", SIZE) + b"\xc2\x04\x00")   # mov eax,SIZE / ret 4
        emu.mem_write(POSITION_STUB, b"\xc2\x08\x00")                                 # ret 8
        emu.mem_write(SIZE, struct.pack("<2f", 1920.0, 1080.0))
        self.word(WORLD, WORLD_VTABLE)
        self.word(WORLD_VTABLE + 0x3C, POSITION_STUB)  # game world vtable+3Ch: the cursor's position
        self.word(WORLD + 0x9C, GAME)
        self.word(GAME + 0x5348, SCREEN)
        self.word(SCREEN, SCREEN_VTABLE)
        self.word(SCREEN_VTABLE + 0x1C, SIZE_STUB)
        self.word(HUD + 0x18, WORLD)
        self.word(OPTIONS + 0x34, GAME)
        emu.hook_add(UC_HOOK_CODE, self.movie, begin=SHOW_MOVIE, end=SHOW_MOVIE)
        emu.hook_add(UC_HOOK_CODE, self.movie, begin=HIDE_MOVIE, end=HIDE_MOVIE)
        emu.hook_add(UC_HOOK_CODE, self.done, begin=RETURN, end=RETURN)
        self.calls = []

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def byte(self, at, value):
        self.emu.mem_write(at, bytes([value]))

    def read_byte(self, at):
        return self.emu.mem_read(at, 1)[0]

    def movie(self, emu, address, size, _):
        """show_movie (EAX the scene, ESI the movie) or hide_movie (EAX the movie, EDX the scene): recorded,
        then its `ret`."""
        eax, esi, edx = (emu.reg_read(r) for r in (UC_X86_REG_EAX, UC_X86_REG_ESI, UC_X86_REG_EDX))
        self.calls.append(("show", eax, esi) if address == SHOW_MOVIE else ("hide", eax, edx))
        sp = emu.reg_read(UC_X86_REG_ESP)
        emu.reg_write(UC_X86_REG_EIP, struct.unpack("<I", emu.mem_read(sp, 4))[0])
        emu.reg_write(UC_X86_REG_ESP, sp + 4)

    def done(self, emu, address, size, _):
        emu.emu_stop()

    def state(self, *, menu, active, cursor, kits=0, holder=True):
        self.byte(OPTIONS + 0x44, int(menu))
        self.byte(WORLD + 0xD4, int(active))
        self.byte(HUD + 0x2DC, int(cursor))
        self.byte(HUD + 0x2DF, kits)
        self.word(CURSOR_MOVIE, HOLDER if holder else 0)

    def start(self, at, stack, regs, until, flags=0x202):
        """Run from `at` to `until` with `stack` above ESP: the movie calls and the registers there."""
        sp = STACK + 0x8000
        self.emu.mem_write(sp, struct.pack(f"<{len(stack)}I", *stack))
        for reg, value in {**REGS, **regs}.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.reg_write(UC_X86_REG_EFLAGS, flags)
        self.calls = []
        self.emu.emu_start(at, until, count=2000000)
        assert self.emu.reg_read(UC_X86_REG_EIP) == until, hex(self.emu.reg_read(UC_X86_REG_EIP))
        regs = {r: self.emu.reg_read(r) for r in (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX,
                                                  UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP)}
        return self.calls, regs, self.emu.reg_read(UC_X86_REG_EFLAGS)

    def show_cursor(self, shown):
        calls, regs, _ = self.start(SHOW_CURSOR, [RETURN, shown], {UC_X86_REG_EDI: HUD}, RETURN)
        assert regs[UC_X86_REG_ESP] == STACK + 0x8000 + 8, "ret 4"
        return calls


SHOWN = [("show", WORLD, CURSOR_MOVIE)]
HIDDEN = [("hide", CURSOR_MOVIE, WORLD)]


def same_registers(old, new, label):
    assert old[1] == new[1], f"{label}: {old[1]} != {new[1]}"


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    old, new = Run(original), Run(bytes(built))

    for run in (old, new):
        run.state(menu=False, active=True, cursor=False)
        assert run.show_cursor(1) == SHOWN
        assert run.read_byte(HUD + 0x2DC) == 1
    old.state(menu=True, active=True, cursor=False)
    new.state(menu=True, active=True, cursor=False)
    assert old.show_cursor(1) == SHOWN
    assert new.show_cursor(1) == []
    assert old.read_byte(HUD + 0x2DC) == 1 and new.read_byte(HUD + 0x2DC) == 1
    for run in (old, new):
        assert run.show_cursor(0) == HIDDEN and run.read_byte(HUD + 0x2DC) == 0
    print("ok show_cursor: the movie shown with the menu closed by both, kept off under the menu by the "
          "patched exe; the flag and the hide as the original's")

    def activate(run, **state):
        run.state(menu=False, **state)
        return run.start(ACTIVATE, [RETURN, 0x5C5C5C5C], {UC_X86_REG_EAX: OPTIONS, UC_X86_REG_ESI: 0x05050505},
                         ACTIVATE_AFTER)

    for state, expected in (({"active": True, "cursor": True}, HIDDEN),
                            ({"active": True, "cursor": False}, []),
                            ({"active": False, "cursor": True}, [])):
        o, n = activate(old, **state), activate(new, **state)
        assert o[0] == [] and n[0] == expected, (state, n[0])
        same_registers(o, n, f"activate {state}")
    print("ok the menu shown: the cursor movie off over a running match with the cursor shown; nothing with "
          "it hidden or over the lobby")

    def deactivate(run, **state):
        run.state(**state)
        return run.start(DEACTIVATE, [RETURN], {UC_X86_REG_ESI: OPTIONS, UC_X86_REG_EAX: 0xA0A0A0A0},
                         DEACTIVATE_JE, flags=0x202 | ZF)

    for state, expected in (({"menu": True, "active": True, "cursor": True}, SHOWN),
                            ({"menu": False, "active": True, "cursor": True}, []),
                            ({"menu": True, "active": False, "cursor": True}, []),
                            ({"menu": True, "active": True, "cursor": False}, []),
                            ({"menu": True, "active": True, "cursor": True, "holder": False}, [])):
        o, n = deactivate(old, **state), deactivate(new, **state)
        assert o[0] == [] and n[0] == expected, (state, n[0])
        same_registers(o, n, f"deactivate {state}")
        assert o[2] & ZF == n[2] & ZF == (0 if state["menu"] else ZF), "the cmp's flags for the je"
    print("ok the menu closed: the cursor movie back over a running match with the cursor shown and loaded; "
          "nothing else; the je's flags as the original's")

    def initialize(run, **state):
        run.state(menu=False, active=True, **state)
        return run.start(INITIALIZE, [RETURN, HUD, 0x0F0F0F0F], {UC_X86_REG_ESI: 0x05050505}, INITIALIZE_AFTER)

    for state, expected in (({"cursor": True, "kits": 1}, HIDDEN), ({"cursor": False, "kits": 0}, [])):
        o = initialize(old, **state)
        assert o[0] == [] and old.read_byte(HUD + 0x2DC) == int(state["cursor"])
        assert old.read_byte(HUD + 0x2DF) == state["kits"]
        n = initialize(new, **state)
        assert n[0] == expected, (state, n[0])
        assert new.read_byte(HUD + 0x2DC) == 0 and new.read_byte(HUD + 0x2DF) == 0
        same_registers(o, n, f"initialize {state}")
    print("ok a new match: the last match's kits and cursor put away by the patched exe, kept by the original")
    print("ok every span ended where the original's does, the registers as its")


if __name__ == "__main__":
    main()
