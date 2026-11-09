#!/usr/bin/env python3
"""Patch 113 (hud_cursor): the hud's kits and cursor start over at a new match,
and the main menu covers the hud's cursor (README "The hud's kits and cursor").

    python check_hud_cursor_026g.py <original survarium.exe>

The original and the patched exe run on a made-up game (its game world inside
it at +18h, the hud at the world's +1FCh, the main menu at +57F4h), the
scene's movie calls (base_game_scene::show_movie 0x23DC00, hide_movie
0x23DC70) stubbed to record what they were given:
1. show_cursor (0x23A930) whole: with the menu closed both show and hide the
   hud's cursor movie; with it shown only the original does; both keep the flag;
2. game_options::activate's start (0x200F50, to the end of the span its hook
   replays): a closed menu with the hud's cursor shown, in a match or not,
   hides the cursor movie in the patched exe; nothing with the cursor hidden
   or the menu already shown (activate returns at once then);
3. game_options::deactivate's start (0x201040, to its je): a shown menu with
   the cursor shown puts the movie back, in a running match and at a match's
   end (the world no longer active); nothing when the menu is not shown, the
   cursor hidden or its movie not loaded; the flags of its cmp as the original's;
4. initialize_match's start (0x234880): the patched exe hides the cursor
   (the movie, +2DCh 0) and the kits (+2DFh 0) the last match left;
5. every span returns where the original's does, the registers as its;
6. the sequences, a scene's movie list kept from the calls: the menu over the
   kits, then a match's end, the next match's start (the 2026-11-07 and
   2026-11-09 crashes at exe+0x14EE8D: the movie taken off twice), a spawn or
   a death under the menu, Esc again: the patched exe never takes off a movie
   the scene does not hold, nor puts one in twice, and the movie is in the
   scene exactly when the flag is set and the menu closed.
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
    built, _ = base.build(original, "section", code, None, True)
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
    old.state(menu=True, active=True, cursor=True)
    new.state(menu=True, active=True, cursor=True)
    assert old.show_cursor(0) == HIDDEN and new.show_cursor(0) == []
    assert old.read_byte(HUD + 0x2DC) == 0 and new.read_byte(HUD + 0x2DC) == 0
    for run in (old, new):
        run.state(menu=False, active=True, cursor=True)
        assert run.show_cursor(0) == HIDDEN and run.read_byte(HUD + 0x2DC) == 0
    print("ok show_cursor: the movie shown and hidden with the menu closed by both, left alone under the menu "
          "by the patched exe; the flag as the original's")

    def activate(run, menu=False, **state):
        run.state(menu=menu, **state)
        return run.start(ACTIVATE, [RETURN, 0x5C5C5C5C], {UC_X86_REG_EAX: OPTIONS, UC_X86_REG_ESI: 0x05050505},
                         ACTIVATE_AFTER)

    for state, expected in (({"active": True, "cursor": True}, HIDDEN),
                            ({"active": True, "cursor": False}, []),
                            ({"active": False, "cursor": True}, HIDDEN),
                            ({"menu": True, "active": True, "cursor": True}, [])):
        o, n = activate(old, **state), activate(new, **state)
        assert o[0] == [] and n[0] == expected, (state, n[0])
        same_registers(o, n, f"activate {state}")
    print("ok the menu shown: the cursor movie off with the cursor shown, in a match or not; nothing with it "
          "hidden or the menu shown already")

    def deactivate(run, **state):
        run.state(**state)
        return run.start(DEACTIVATE, [RETURN], {UC_X86_REG_ESI: OPTIONS, UC_X86_REG_EAX: 0xA0A0A0A0},
                         DEACTIVATE_JE, flags=0x202 | ZF)

    for state, expected in (({"menu": True, "active": True, "cursor": True}, SHOWN),
                            ({"menu": False, "active": True, "cursor": True}, []),
                            ({"menu": True, "active": False, "cursor": True}, SHOWN),
                            ({"menu": True, "active": True, "cursor": False}, []),
                            ({"menu": True, "active": True, "cursor": True, "holder": False}, [])):
        o, n = deactivate(old, **state), deactivate(new, **state)
        assert o[0] == [] and n[0] == expected, (state, n[0])
        same_registers(o, n, f"deactivate {state}")
        assert o[2] & ZF == n[2] & ZF == (0 if state["menu"] else ZF), "the cmp's flags for the je"
    print("ok the menu closed: the cursor movie back with the cursor shown and loaded, over a running match "
          "and at its end; nothing else; the je's flags as the original's")

    def initialize(run, menu=False, **state):
        run.state(menu=menu, active=True, **state)
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

    sequences(new, activate, deactivate, initialize)


def sequences(run, activate, deactivate, initialize):
    """The game's own order of calls on one patched exe, its game state carried over, the scene's movie list
    kept as world::hide_movie and show_movie would (a hide of a movie not there is the crash at exe+0x14EE8D:
    remove_movie erases the end std::find returns)."""

    def play(steps):
        run.state(menu=False, active=True, cursor=False)
        scene = []
        for step in steps:
            menu, active = run.read_byte(OPTIONS + 0x44), run.read_byte(WORLD + 0xD4)
            cursor, holder = run.read_byte(HUD + 0x2DC), struct.unpack("<I", run.emu.mem_read(CURSOR_MOVIE, 4))[0]
            if step == "kits":            # show_profiles_selector's show_cursor(1)
                calls = run.show_cursor(1)
            elif step == "spawn":         # the kits hidden, show_cursor(0)
                calls = run.show_cursor(0)
            elif step == "esc":           # game_options::activate, then its shown byte (0x200F86)
                calls = activate(run, menu=bool(menu), active=bool(active), cursor=bool(cursor),
                                 holder=bool(holder))[0]
                run.byte(OPTIONS + 0x44, 1)
            elif step == "resume":        # game_options::deactivate to its je, then its clear (0x201067)
                calls = deactivate(run, menu=bool(menu), active=bool(active), cursor=bool(cursor),
                                   holder=bool(holder))[0]
                run.byte(OPTIONS + 0x44, 0)
            elif step == "end":           # game_world::on_deactivate: active cleared (0x1F952B), the menu closed
                run.byte(WORLD + 0xD4, 0)
                calls = deactivate(run, menu=bool(menu), active=False, cursor=bool(cursor), holder=bool(holder))[0]
                run.byte(OPTIONS + 0x44, 0)
            elif step == "match":         # the next match: active again, initialize_match (the lobby's menu open or not)
                run.byte(WORLD + 0xD4, 1)
                calls = initialize(run, menu=bool(menu), cursor=bool(cursor), kits=run.read_byte(HUD + 0x2DF))[0]
            else:
                raise ValueError(step)
            for call in calls:
                if call[0] == "show":
                    assert CURSOR_MOVIE not in scene, (steps, step, "shown twice")
                    scene.append(CURSOR_MOVIE)
                else:
                    assert CURSOR_MOVIE in scene, (steps, step, "taken off a scene without it: exe+0x14EE8D")
                    scene.remove(CURSOR_MOVIE)
            shown, under = run.read_byte(HUD + 0x2DC), run.read_byte(OPTIONS + 0x44)
            assert (CURSOR_MOVIE in scene) == bool(shown and not under), (steps, step, scene, shown, under)
        return scene

    for steps in (["kits", "esc", "end", "match"],                       # 2026-11-09: left before the start
                  ["kits", "esc", "end", "kits", "match"],
                  ["kits", "esc", "spawn", "resume"],                    # spawned under the menu
                  ["kits", "esc", "spawn", "end", "match", "kits"],
                  ["esc", "kits", "resume", "spawn"],                    # died under the menu
                  ["esc", "kits", "end", "match", "kits", "spawn"],
                  ["kits", "esc", "esc", "resume", "spawn"],             # activate twice
                  ["kits", "esc", "resume", "esc", "end", "match", "esc", "resume"],
                  ["kits", "end", "esc", "resume", "match", "kits", "esc", "spawn", "resume"],
                  ["kits", "end", "esc", "match", "kits", "resume", "spawn"]):   # a match starts under the lobby's menu
        play(steps)
    print("ok the sequences: no movie taken off a scene without it, none put in twice; the movie in the scene "
          "exactly when the cursor is shown and the menu closed")


if __name__ == "__main__":
    main()
