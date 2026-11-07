#!/usr/bin/env python3
"""Patch 113 (hud_cursor): the hud's kits and cursor start over at a new match,
and the main menu covers the hud's cursor (README "The hud's kits and cursor").

    python check_hud_cursor_026g.py <original survarium.exe>

The original and the patched exe run on a made-up game (its game world inside
it at +18h, the hud at the world's +1FCh, the main menu at +57F4h), the
scene's movie calls (base_game_scene::show_movie 0x23DC00, hide_movie
0x23DC70) stubbed by a model of the scene's list: a show adds the movie, a
hide takes one copy out, and a hide of a movie the list has not fails, as the
render thread's erase of find's end does (engine::world::hide_movie 0x118FE0:
find 0x3C5510 gives the list's end, erase 0x15EE30 walks past it; the live
crash of 2026-11-07). Each case is a sequence of the game's steps on a new
game: show_cursor (0x23A930) whole, game_options::activate's start (0x200F50,
to the end of its hook's span) with the menu then shown, deactivate's start
(0x201040, to its je) with the menu then closed, initialize_match's start
(0x234880), a match's end (the world's +D4h cleared). Checked:
1. no case hides a movie the scene has not (the original none either);
2. the movie is in the scene where the case says: off under the menu over a
   match, back after Resume, never twice; the hud's cursor flag the
   original's at every show_cursor;
3. initialize_match hides the cursor (+2DCh 0) and the kits (+2DFh 0);
4. every span returns where the original's does, the registers as its
   (show_cursor: the ones it keeps), deactivate's je flags as its cmp's.
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
ALL = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI,
       UC_X86_REG_EBP, UC_X86_REG_ESP)
KEPT = (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP)
ZF = 1 << 6


class Crash(Exception):
    """A hide of a movie the scene's list has not: the render thread's crash."""


class Run:
    """One game, new: the menu closed, the world active, the cursor hidden, the kits shown (the last match's),
    the movie loaded and not in the scene."""

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
        self.word(CURSOR_MOVIE, HOLDER)
        self.byte(WORLD + 0xD4, 1)
        self.byte(HUD + 0x2DF, 1)
        emu.hook_add(UC_HOOK_CODE, self.movie, begin=SHOW_MOVIE, end=SHOW_MOVIE)
        emu.hook_add(UC_HOOK_CODE, self.movie, begin=HIDE_MOVIE, end=HIDE_MOVIE)
        emu.hook_add(UC_HOOK_CODE, self.done, begin=RETURN, end=RETURN)
        self.scene = []
        self.fault = None

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def byte(self, at, value):
        self.emu.mem_write(at, bytes([value]))

    def read_byte(self, at):
        return self.emu.mem_read(at, 1)[0]

    def read_word(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def movie(self, emu, address, size, _):
        """show_movie (EAX the scene, ESI the movie) or hide_movie (EAX the movie, EDX the scene) on the
        scene's list, then its `ret`. hide_movie of an unloaded movie returns at once (0x23DC74); show_movie
        reads the holder (0x23DC26)."""
        eax, esi, edx = (emu.reg_read(r) for r in (UC_X86_REG_EAX, UC_X86_REG_ESI, UC_X86_REG_EDX))
        if address == SHOW_MOVIE:
            assert (eax, esi) == (WORLD, CURSOR_MOVIE), (hex(eax), hex(esi))
            holder = self.read_word(esi)
            assert holder, "show_movie of an unloaded movie"
            self.scene.append(holder)
        else:
            assert (eax, edx) == (CURSOR_MOVIE, WORLD), (hex(eax), hex(edx))
            holder = self.read_word(eax)
            if holder and holder not in self.scene:
                self.fault = "hide_movie of a movie not in the scene (erase of find's end, 0x15EE30)"
                emu.emu_stop()
                return
            if holder:
                self.scene.remove(holder)
        sp = emu.reg_read(UC_X86_REG_ESP)
        emu.reg_write(UC_X86_REG_EIP, self.read_word(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 4)

    def done(self, emu, address, size, _):
        emu.emu_stop()

    def start(self, at, stack, regs, until, flags=0x202):
        """Run from `at` to `until` with `stack` above ESP: the registers and the flags there."""
        sp = STACK + 0x8000
        self.emu.mem_write(sp, struct.pack(f"<{len(stack)}I", *stack))
        for reg, value in {**REGS, **regs}.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.reg_write(UC_X86_REG_EFLAGS, flags)
        self.emu.emu_start(at, until, count=2000000)
        if self.fault:
            raise Crash(self.fault)
        assert self.emu.reg_read(UC_X86_REG_EIP) == until, hex(self.emu.reg_read(UC_X86_REG_EIP))
        return {r: self.emu.reg_read(r) for r in ALL}, self.emu.reg_read(UC_X86_REG_EFLAGS)

    # The game's steps; each returns what the original's must equal.

    def cursor(self, shown):
        regs, _ = self.start(SHOW_CURSOR, [RETURN, int(shown)], {UC_X86_REG_EDI: HUD}, RETURN)
        assert regs[UC_X86_REG_ESP] == STACK + 0x8000 + 8, "ret 4"
        assert self.read_byte(HUD + 0x2DC) == int(shown), "the flag"
        return {r: regs[r] for r in KEPT}

    def menu(self, shown):
        if shown:
            regs, _ = self.start(ACTIVATE, [RETURN, 0x5C5C5C5C],
                                 {UC_X86_REG_EAX: OPTIONS, UC_X86_REG_ESI: 0x05050505}, ACTIVATE_AFTER)
            out = regs
        else:
            regs, flags = self.start(DEACTIVATE, [RETURN], {UC_X86_REG_ESI: OPTIONS, UC_X86_REG_EAX: 0xA0A0A0A0},
                                     DEACTIVATE_JE, flags=0x202 | ZF)
            assert flags & ZF == (0 if self.read_byte(OPTIONS + 0x44) else ZF), "the cmp's flags for the je"
            out = regs, flags & ZF
        self.byte(OPTIONS + 0x44, int(shown))
        return out

    def end(self):
        self.byte(WORLD + 0xD4, 0)

    def match_start(self):
        regs, _ = self.start(INITIALIZE, [RETURN, HUD, 0x0F0F0F0F], {UC_X86_REG_ESI: 0x05050505},
                             INITIALIZE_AFTER)
        self.byte(WORLD + 0xD4, 1)
        return regs

    def unload(self):
        self.word(CURSOR_MOVIE, 0)


# A case: its name, then its steps: a step of Run's and its argument, or ("scene", n), the movie's copies in
# the patched exe's scene there.
CASES = [
    ("Esc then Resume with the hud's cursor shown",
     [("cursor", 1), ("scene", 1), ("menu", 1), ("scene", 0), ("menu", 0), ("scene", 1), ("cursor", 0),
      ("scene", 0)]),
    ("the cursor shown under the menu (a death), then Resume",
     [("menu", 1), ("cursor", 1), ("scene", 0), ("menu", 0), ("scene", 1), ("cursor", 0), ("scene", 0)]),
    ("the menu over the lobby: nothing taken off",
     [("end",), ("cursor", 1), ("menu", 1), ("scene", 1), ("menu", 0), ("scene", 1), ("cursor", 0)]),
    ("the menu shown with the cursor hidden, then closed: nothing put up",
     [("menu", 1), ("scene", 0), ("menu", 0), ("scene", 0), ("cursor", 1), ("scene", 1)]),
    ("the movie unloaded under the menu: nothing put back",
     [("cursor", 1), ("menu", 1), ("unload",), ("menu", 0), ("scene", 0), ("cursor", 0)]),
    ("a new match with the last match's kits and cursor shown",
     [("cursor", 1), ("match_start",), ("scene", 0), ("cursor", 1), ("scene", 1)]),
    ("Esc then Leave with the hud's cursor shown, then the next match's initialize_match",
     [("cursor", 1), ("menu", 1), ("scene", 0), ("end",), ("menu", 0), ("scene", 0), ("match_start",),
      ("scene", 0), ("cursor", 1), ("scene", 1)]),
    ("a match's end with the menu open and the cursor shown under it, then the next match",
     [("menu", 1), ("cursor", 1), ("end",), ("menu", 0), ("match_start",), ("scene", 0), ("cursor", 1),
      ("scene", 1)]),
    ("a respawn under the menu: show_cursor(0) after the menu took the movie off",
     [("cursor", 1), ("menu", 1), ("cursor", 0), ("scene", 0), ("menu", 0), ("scene", 0), ("cursor", 1),
      ("scene", 1)]),
    ("the Kept exit, then a hide under the menu",
     [("menu", 1), ("cursor", 1), ("cursor", 0), ("menu", 0), ("scene", 0), ("cursor", 1), ("scene", 1)]),
]


def play(original, built, steps):
    old, new = Run(original), Run(built)
    for step in steps:
        name, *args = step
        if name == "scene":
            assert len(new.scene) == args[0], (step, new.scene)
            continue
        o, n = getattr(old, name)(*args), getattr(new, name)(*args)
        assert o == n, (step, o, n)
        assert len(new.scene) <= 1, (step, new.scene)
        if name == "match_start":
            assert (new.read_byte(HUD + 0x2DC), new.read_byte(HUD + 0x2DF)) == (0, 0), "the cursor and the kits"


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    built = bytes(built)
    failed = 0
    for name, steps in CASES:
        try:
            play(original, built, steps)
            print(f"ok {name}")
        except (Crash, AssertionError) as error:
            print(f"FAIL {name}: {error!r}")
            failed += 1
    if failed:
        sys.exit(f"{failed} of {len(CASES)} cases failed")
    print("ok every span ended where the original's does, the registers as its")


if __name__ == "__main__":
    main()
