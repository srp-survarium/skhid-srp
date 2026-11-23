#!/usr/bin/env python3
"""Unicorn: patch 196, mt_draw's tick of a not alive player with no animation tree (draw_no_tree.rs),
with the original 0.26g executable. The game's own player::mt_draw from its alive test (0x24751C) to
the tick's join (0x247548), on a made-up player:
- never spawned (the state of the 2026-11-22 crash, as base_player::base_player leaves a player:
  +33Ch 0, +324h and +328h -1, its animation player's tree 0 and its callbacks empty): the original
  calls animation_player::tick 0x3917D0, whose n_ary_tree::tick reads the null tree at 0x3A19F9
  (a read of 4, the crash); the build skips the tick, reaches 0x247548 and logs one line;
- the same player drawn again: the tick skipped again, no second line; another player at that place:
  one line more;
- dead with a tree behind the player's time: both exes call animation_player::tick (stubbed: its
  animation player and time recorded), nothing logged;
- alive: both exes go to 0x247548 without the tick.
"""
import struct
import sys

from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_READ_UNMAPPED, UC_PROT_ALL, UcError
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP

import patch_026g as base
import check_hide_ui_026g as hide
import check_console_where_026g as where

ALIVE_TEST, JOIN = 0x24751C, 0x247548        # mt_draw: cmp byte [ebx+33Ch],0 .. the tick's join
TICK, TREE_TICK_READ = 0x3917D0, 0x3A19F9    # animation_player::tick; n_ary_tree::tick's read of its tree
PLAYERS = 0x46000000                         # made-up players, 20000h each
TREES = 0x46100000
ANIMATION, TREE = 0x750, 0xE850              # the player's animation player, its tree (+E100h of it)
TIME, SPAWNED, ALIVE, INSERTED, PLACE, KIT = 0x324, 0x328, 0x33C, 0x33F, 0x114, 0x115


class Run(where.Run):
    def __init__(self, data, stub_tick):
        super().__init__(data)
        self.emu.mem_map(PLAYERS, 0x100000, UC_PROT_ALL)
        self.emu.mem_map(TREES, 0x10000, UC_PROT_ALL)
        self.ticks, self.fault = [], None
        if stub_tick:
            self.emu.hook_add(UC_HOOK_CODE, self.tick_stub, begin=TICK, end=TICK)
        self.emu.hook_add(UC_HOOK_MEM_READ_UNMAPPED, self.unmapped)

    def tick_stub(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.ticks.append((emu.reg_read(UC_X86_REG_ESI), self.read(sp + 4)))
        emu.reg_write(UC_X86_REG_EAX, 0)
        emu.reg_write(UC_X86_REG_ESP, sp + 8)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def unmapped(self, emu, access, address, size, value, _):
        self.fault = (emu.reg_read(UC_X86_REG_EIP), address)
        return False

    def player(self, n, place, *, alive, time, tree_time=None):
        """Player n as base_player::base_player leaves it (time and spawn -1, not alive, not in the
        world, its animation player with no tree and empty callbacks), then `alive`, `time` and, with
        `tree_time`, a tree (its references 5, never released here; its time +2Ch)."""
        p = PLAYERS + 0x20000 * n
        self.emu.mem_write(p, bytes(0x20000))
        self.emu.mem_write(p + PLACE, bytes([place, 0]))
        self.word(p + TIME, time)
        self.word(p + SPAWNED, 0xFFFFFFFF)
        self.emu.mem_write(p + ALIVE, bytes([1 if alive else 0]))
        anim = p + ANIMATION
        self.word(anim + 0xE104, anim + 0x7000)
        self.word(anim + 0xE10C, anim + 0xE118)
        self.word(anim + 0xE110, 0xA00)
        if tree_time is not None:
            tree = TREES + 0x100 * n
            self.word(tree, 5)
            self.word(tree + 0x2C, tree_time)
            self.word(p + TREE, tree)
        return p

    def draw(self, player):
        """mt_draw from its alive test to the tick's join; where it stopped (JOIN or None on a fault)."""
        self.ticks, self.fault = [], None
        sp = hide.STACK + 0x8000
        try:
            self.run(ALIVE_TEST, (JOIN,), sp, {**hide.GAME, UC_X86_REG_EBX: player})
        except UcError:
            return None
        return self.stopped


def check(ok, text):
    print(("ok  " if ok else "FAIL ") + text)
    if not ok:
        sys.exit(1)


def main():
    original = open(sys.argv[1], "rb").read()
    built, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None,
                          open(base.late_join_bin(), "rb").read())
    site = base.file_offset(0x24752B)
    check(original[site:site + 6] == bytes.fromhex("8bbb24030000") and built[site] == 0xE9,
          "0x24752B: mov edi,[ebx+324h] in the original, a jmp to the hook's entry in the build")

    # Never spawned: the crash's state.
    run = Run(original, stub_tick=False)
    p = run.player(0, 12, alive=False, time=0xFFFFFFFF)
    check(run.draw(p) is None and run.fault == (TREE_TICK_READ, 4),
          f"original: a never spawned player at place 12: animation_player::tick, n_ary_tree::tick reads 4 at "
          f"0x3A19F9 (the crash) {run.fault}")
    run = Run(bytes(built), stub_tick=False)
    p = run.player(0, 12, alive=False, time=0xFFFFFFFF)
    check(run.draw(p) == JOIN and run.fault is None
          and run.logged.count(b"draw_no_tree: place 12 kit 0: a not alive player with no animation tree") == 1
          and b"time 0xffffffff" in run.logged and b"not in the world" in run.logged,
          "build: the tick skipped, at 0x247548, one line: " + run.logged.decode(errors="replace").strip())
    check(run.draw(p) == JOIN and run.logged.count(b"draw_no_tree:") == 1,
          "build: drawn again: the tick skipped again, no second line")
    q = run.player(1, 12, alive=False, time=0xFFFFFFFF)
    check(run.draw(q) == JOIN and run.logged.count(b"draw_no_tree: place 12") == 2,
          "build: another player at place 12: one line more")

    for name, data in (("original", original), ("build", bytes(built))):
        run = Run(data, stub_tick=True)
        dead = run.player(2, 3, alive=False, time=2000, tree_time=1000)
        check(run.draw(dead) == JOIN and run.ticks == [(dead + ANIMATION, 2000)] and run.logged == b"",
              f"{name}: dead with a tree behind its time: animation_player::tick(its animation player, 2000), "
              f"nothing logged")
        live = run.player(3, 4, alive=True, time=2000)
        check(run.draw(live) == JOIN and run.ticks == [] and run.logged == b"",
              f"{name}: alive: no tick, nothing logged")
    print("all checks passed")


if __name__ == "__main__":
    main()
