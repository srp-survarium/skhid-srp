#!/usr/bin/env python3
"""Unicorn: patch 192, a damage zone's two unchecked finds (zone_guards.rs), with the original 0.26g
executable. The game's own damage_zone_core::on_objects_entered (0x3198D0) and on_objects_left
(0x319780) on a made-up zone sensor and players (each a game_object part with the player's own
vtable 0x9FB3DC: subscribe_to_actions 0x2DBE50 and unsubscribe_from_actions 0x2DBE90 run as they are):
- entered then left: subscribed and unsubscribed, the inside list filled and emptied, the zone count
  up and down, as the original (nothing logged);
- left while not subscribed (the inside list holds him, his subscribers none: the state of the
  2026-11-22 crash): the original calls memmove(it, it + 4, -4) (0x2DBEBA); the build does not, the
  player's list stays as it is, the inside list loses its entry, one line in srp_patches.log;
- left while not in the inside list (another player's entry in it): the original pops that entry;
  the build leaves it, one line in srp_patches.log.
"""
import struct
import sys

from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_ECX, UC_X86_REG_ESP

import patch_026g as base
import check_hide_ui_026g as hide
import check_console_where_026g as where

ENTERED, LEFT = 0x3198D0, 0x319780           # thiscall (listener; objects vector*, time), ret 8
MEMMOVE = 0x4226A0
PLAYER_PART_VTABLE = 0x9FB3DC
SENSOR_VTABLE = 0xA165CC                      # damage_zone_core's sensor (the listener)
ZONE = 0x45000000                             # the zone; its sensor (the listener) at +108h
LISTENER = ZONE + 0x108
WORLD = 0x45100000                            # players (10000h each), holders, objects, vectors
STUB = 0x45F00000                             # an owner's getter: mov eax, <part> / ret


class Run(where.Run):
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_map(ZONE, 0x10000, UC_PROT_ALL)
        self.emu.mem_map(WORLD, 0x100000, UC_PROT_ALL)
        self.emu.mem_map(STUB, 0x1000, UC_PROT_ALL)
        self.moves = []
        self.emu.hook_add(UC_HOOK_CODE, self.memmove, begin=MEMMOVE, end=MEMMOVE)
        self.reset()

    def memmove(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        count = self.read(sp + 12)
        self.moves.append((self.read(sp + 4), self.read(sp + 8), count))
        if count >= 0x80000000:
            emu.emu_stop()                    # the wild move: no further
            self.stopped = MEMMOVE

    def reset(self):
        self.emu.mem_write(ZONE, bytes(0x10000))
        self.emu.mem_write(WORLD, bytes(0x100000))
        self.word(LISTENER, SENSOR_VTABLE)
        self.word(LISTENER + 0xA4, 1)          # active
        self.word(LISTENER + 0xCC, 0)          # no client effects
        inside = WORLD + 0xF0000
        self.word(LISTENER + 0x50, inside)
        self.word(LISTENER + 0x54, inside)
        self.word(LISTENER + 0x58, inside + 0x40)
        self.moves, self.logged = [], b""

    def player(self, n, place):
        """Player n's game_object part (player+118h), its subscriber vector (room for four), its sensed
        object and that object's holder; place and kit as the player's +114h, +115h."""
        base_ = WORLD + 0x10000 * n
        part = base_ + 0x118
        self.word(part, PLAYER_PART_VTABLE)
        self.emu.mem_write(base_ + 0x114, bytes([place, 0]))
        self.word(base_ + 0x324, 438000)
        subs = base_ + 0x8000
        self.word(part + 0x3B0, subs)
        self.word(part + 0x3B4, subs)
        self.word(part + 0x3B8, subs + 0x10)
        # the object (its +4 what a holder names), its +0Ch a thing whose vtable +0 returns the part
        obj, thing, vt = base_ + 0x9000, base_ + 0x9100, base_ + 0x9200
        self.word(obj + 0xC, thing)
        self.word(thing, vt)
        getter = STUB + 0x10 * n
        self.emu.mem_write(getter, b"\xB8" + struct.pack("<I", part) + b"\xC3")
        self.word(vt, getter)
        holder = base_ + 0x9300
        self.word(holder, obj + 4)
        self.word(holder + 4, 100)             # references: never released here
        return part, holder

    def subscribers(self, part):
        b, e = self.read(part + 0x3B0), self.read(part + 0x3B4)
        return [self.read(a) for a in range(b, e, 4)] if e >= b else ["end before begin"]

    def inside(self):
        b, e = self.read(LISTENER + 0x50), self.read(LISTENER + 0x54)
        return [self.read(a) for a in range(b, e, 4)]

    def notify(self, call, holders):
        vec = WORLD + 0xE0000
        for i, h in enumerate(holders):
            self.word(vec + 0x10 + 4 * i, h)
        self.word(vec, vec + 0x10)
        self.word(vec + 4, vec + 0x10 + 4 * len(holders))
        sp = hide.STACK + 0x8000
        self.emu.mem_write(sp, struct.pack("<3I", hide.SENTINEL, vec, 438000))
        self.run(call, (hide.SENTINEL,), sp, {**hide.GAME, UC_X86_REG_ECX: LISTENER})
        return self.stopped


def check(ok, text):
    print(("ok  " if ok else "FAIL ") + text)
    if not ok:
        sys.exit(1)


def main():
    original = open(sys.argv[1], "rb").read()
    built, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None,
                          open(base.late_join_bin(), "rb").read())
    for name, run in (("original", Run(original)), ("build", Run(bytes(built)))):
        a, ha = run.player(0, 1)
        subscriber = LISTENER + 0x24
        check(run.notify(ENTERED, [ha]) == hide.SENTINEL and run.subscribers(a) == [subscriber]
              and run.inside() == [ha] and run.read(WORLD + 0x2E4, 1)[0] == 1,
              f"{name}: entered: subscribed, in the inside list, his zone count 1")
        check(run.notify(LEFT, [ha]) == hide.SENTINEL and run.subscribers(a) == [] and run.inside() == []
              and run.read(WORLD + 0x2E4, 1)[0] == 0 and run.logged == b"",
              f"{name}: left: unsubscribed, out of the inside list, count 0, nothing logged")

    # The crash's state: the inside list holds him, his subscribers none.
    for name, run in (("original", Run(original)), ("build", Run(bytes(built)))):
        a, ha = run.player(0, 1)
        run.notify(ENTERED, [ha])
        b = run.read(a + 0x3B0)
        run.word(a + 0x3B4, b)                 # his subscription gone, the zone's entry kept
        stopped = run.notify(LEFT, [ha])
        if name == "original":
            check(stopped == MEMMOVE and run.moves[-1][2] == 0xFFFFFFFC,
                  "original: left without a subscription: memmove(it, it + 4, -4) at 0x2DBEBA (the crash)")
        else:
            check(stopped == hide.SENTINEL and not any(m[2] >= 0x80000000 for m in run.moves)
                  and run.subscribers(a) == [] and run.inside() == []
                  and b"zones: unsubscribe of a zone the player is not subscribed to, skipped: place 1 kit 0 at 438000 ms"
                  in run.logged and b"subscribers 0" in run.logged and b"inside 1" in run.logged,
                  "build: the erase skipped (his list as it was), the inside list emptied, the line logged: "
                  + run.logged.decode(errors="replace").strip())

    # Left while not in the inside list, another player's entry in it.
    for name, run in (("original", Run(original)), ("build", Run(bytes(built)))):
        a, ha = run.player(0, 1)
        c, hc = run.player(1, 4)
        run.notify(ENTERED, [ha, hc])
        run.word(LISTENER + 0x54, run.read(LISTENER + 0x50) + 4)   # only A's... then make it C's alone
        run.word(run.read(LISTENER + 0x50), hc)
        check(run.inside() == [hc], f"{name}: the inside list holds C alone, A subscribed")
        stopped = run.notify(LEFT, [ha])
        if name == "original":
            check(stopped == hide.SENTINEL and run.inside() == [] and run.subscribers(a) == [],
                  "original: A left: unsubscribed, and C's entry popped from the inside list")
        else:
            check(stopped == hide.SENTINEL and run.inside() == [hc] and run.subscribers(a) == []
                  and b"zones: a left player not in the zone's inside list, its erase skipped: place 1 kit 0"
                  in run.logged,
                  "build: A unsubscribed, C's entry kept, the line logged: " + run.logged.decode(errors="replace").strip())
    print("all checks passed")


if __name__ == "__main__":
    main()
