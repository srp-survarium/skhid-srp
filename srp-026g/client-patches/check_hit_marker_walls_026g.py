#!/usr/bin/env python3
"""Patch 167 (hit_marker_walls): no white hit marker for a hit through a wall
(docs/notes/hit-marker-walls.md), the default build beside the original.

    python check_hit_marker_walls_026g.py <original survarium.exe>

Requires Python `unicorn`. The game's own code is run at the three sites, on
made-up bullets, players and a hud, the state of the build's patch kept from
run to run as a game keeps it:

- a pierce: check_collision from its test of try_pierce's answer (0x32D886,
  AL the answer) to its next step (0x32D8C4, or 0x32D891 when the bullet
  stopped), the bullet in EDI, the hit receiver in ESI;
- a hit: player::generate_hit_event's store of the shooter's collector from
  the fraction's sum (0x1F3D50) to its `ret 1Ch`, with the shooter's place
  and the time in its arguments;
- a draw: player::st_draw's call of draw_damage_collector (0x24745B, its read
  of the ui, to 0x24746C), with the game's draw (its hook inside, after it
  took its copy) and root.crosshair_enemy_hit (Scaleform's invoke, 0x511600)
  a stand-in that keeps what it is given; one hud for both shooters.

What a game shows, for a shooter at place 2 on the victim at place 7:
- a bullet that hit clear, through a surface of armour 0 (glass, foliage,
  cloth, paper; -0 too), through a body (a receiver), a gun-stock (blunt)
  through a wall: the marker, as the original;
- a round through a wall (a level's surface of armour above 0, glass_hard's
  0.10 too): none, and none on the next frames; the original shows it;
- the next clear hit: the marker again, with the collected fraction; one on
  the first victim again after a hidden one on another, the same damage as
  its first (the draw took the hidden one's copy);
- a clear hit not drawn yet, then a walled one in a later step: the marker;
- pellets of one step: one clear among walled ones shows it, in either order;
  only walled ones: none;
- another shooter's (place 5) collector: its own hits only;
- a bullet try_pierce stopped is not noted;
- a lethal round through a wall: no white marker, then its kill event, run
  through the game world's walk of its events (0x1FB6C0, 0x233770, 0x23CF20
  to the invoke): the kill marker (1.0, killed) in both, once.
In every run the registers the game keeps are the original's. The kill's
marker (0x23CF20) and draw_damage_collector but its hook's five bytes are the
original's: a kill through a wall reaches the hud by the kill's call alone,
as "Kill marker only" shows it.
"""
import os
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI,
                               UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP, UC_X86_REG_XMM0)

if len(sys.argv) > 1:
    os.environ.setdefault("SRP_026G_EXE", sys.argv[1])    # a span hook's bytes (patch_026g.RUST_HOOKS)
import patch_026g as base

PIERCE_FROM, PIERCE_ON, PIERCE_STOPPED = 0x32D886, 0x32D8C4, 0x32D891
HIT_FROM = 0x1F3D50
DRAW_FROM, DRAW_DONE = 0x24745B, 0x24746C
INVOKE = 0x511600
KILL_MARKER, DRAW_SITE = (0x23CF20, 0x23D100), 0x238272
DRAW_BEFORE, DRAW_AFTER = (0x238230, DRAW_SITE), (DRAW_SITE + 5, 0x2383AA)
SITES = {0x32D88A: "bullet.pierced", 0x1F3D76: "damage_collector.hit", DRAW_SITE: "damage_collector.draw"}
THREAD_SLOT = 0x870118
HEAP, STUBS, STACK = 0x30000000, 0x38000000, 0x39000000
PLAYERS = {2: HEAP, 5: HEAP + 0x10000}
UI, HOLDER, MOVIE_HOLDER = HEAP + 0x20000, HEAP + 0x21000, HEAP + 0x22000
BULLETS, AMMO, MATERIALS = HEAP + 0x30000, HEAP + 0x40000, HEAP + 0x50000
RETURN = STUBS + 0xF00
COLLECTOR, PLACE, UI_OF = 0xF534, 0x114, 0xF480
RANGED, BLUNT = 0, 1
CONCRETE, GLASS_HARD = 0.23, 0.10    # game.materials' armor
VICTIM = 7
KILL_WALK, SHOW_KILL_MARKER = 0x1FB6C0, 0xA70EC6    # process_kill_cross_events, show_enemy_killed_marker
KILL_EVENT, HIT_EVENT = 0, 2    # a world event's type (+18h)
KILL_LIST, KILL_EVENTS, KILL_UI, GAME_WORLD = HEAP + 0x23000, HEAP + 0x24000, HEAP + 0x26000, HEAP + 0x27000
KEPT = (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP)

failed = 0


def check(ok, what):
    global failed
    failed += not ok
    print(f"  {'ok' if ok else 'FAIL'}  {what}")


class Game:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(HEAP, 0x60000, UC_PROT_ALL)
        for at in (STUBS, STACK):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        self.u32(THREAD_SLOT, STUBS + 0x80)
        emu.mem_write(STUBS + 0x80, b"\xB8\x01\x00\x00\x00\xC3")    # GetCurrentThreadId: mov eax,1 / ret
        for place, player in PLAYERS.items():
            emu.mem_write(player + PLACE, bytes([place]))
            self.u32(player + UI_OF, UI)
            emu.mem_write(player + COLLECTOR, struct.pack("<BxxxIf", 0xFF, 0, 0.0))
        self.u32(UI + 8, HOLDER)
        self.u32(HOLDER + 0x108, MOVIE_HOLDER)
        self.u32(MOVIE_HOLDER + 4, STUBS + 0x400)
        emu.hook_add(UC_HOOK_CODE, self.invoke, begin=INVOKE, end=INVOKE)
        self.bullets = 0
        self.markers = []

    def u32(self, at, value=None):
        if value is None:
            return struct.unpack("<I", self.emu.mem_read(at, 4))[0]
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def invoke(self, emu, *_):
        """GFx movie invoke (thiscall, `ret 10h`): the name, the result, the
        arguments and their count; keeps root.crosshair_enemy_hit's."""
        esp = emu.reg_read(UC_X86_REG_ESP)
        name, args, count = self.u32(esp + 4), self.u32(esp + 0xC), self.u32(esp + 0x10)
        text = bytes(emu.mem_read(name, 32)).split(b"\0")[0]
        number = struct.unpack("<d", emu.mem_read(args + 8, 8))[0]
        killed = emu.mem_read(args + 0x30 + 8, 1)[0]
        self.markers.append((text, count, round(number, 4), killed))
        emu.reg_write(UC_X86_REG_ESP, esp + 4 + 0x10)
        emu.reg_write(UC_X86_REG_EIP, self.u32(esp))

    def run(self, start, stops, regs, count=2_000_000):
        emu = self.emu
        hits = []
        hook = emu.hook_add(UC_HOOK_CODE, lambda e, a, *_: (hits.append(a), e.emu_stop()) if a in stops else None)
        for reg, value in regs.items():
            emu.reg_write(reg, value)
        emu.emu_start(start, 0, count=count)
        emu.hook_del(hook)
        assert hits, f"stopped at {emu.reg_read(UC_X86_REG_EIP):#x}"
        return hits[0], {reg: emu.reg_read(reg) for reg in KEPT}

    def bullet(self, kind=RANGED):
        """A made-up bullet: its id, born time, ammo (its damage type)."""
        self.bullets += 1
        bullet, ammo = BULLETS + 0x100 * self.bullets, AMMO + 0x100 * self.bullets
        self.u32(bullet + 4, 1000 + self.bullets)
        self.u32(bullet + 0x5C, 40_000)
        self.u32(bullet + 0x40, ammo)
        self.u32(ammo + 0x4C, kind)
        return bullet

    def pierce(self, bullet, armor, receiver=0, pierced=True):
        """check_collision after try_pierce: the bullet struck a material of
        this armour (its +4Ch, try_pierce's A)."""
        at = MATERIALS + 0x100 * self.bullets
        self.emu.mem_write(at + 0x4C, struct.pack("<f", armor))
        self.u32(bullet + 0x4C, at)
        sp = STACK + 0x8000
        stop, kept = self.run(PIERCE_FROM, {PIERCE_ON, PIERCE_STOPPED},
                              {UC_X86_REG_EAX: int(pierced), UC_X86_REG_EDI: bullet, UC_X86_REG_ESI: receiver,
                               UC_X86_REG_EBX: 0, UC_X86_REG_EBP: 0x0BADF00D, UC_X86_REG_ESP: sp})
        return stop, self.emu.reg_read(UC_X86_REG_EBP), kept

    def hit(self, shooter, bullet, time, fraction, victim=VICTIM):
        """generate_hit_event's store of `shooter`'s collector: the victim, the
        time, the fraction (its sum, clamped there)."""
        sp = STACK + 0x8000
        self.u32(sp + 0x10, RETURN)
        self.u32(sp + 0x14, time)
        self.u32(sp + 0x18, shooter)
        self.emu.reg_write(UC_X86_REG_XMM0, struct.unpack("<I", struct.pack("<f", fraction))[0])
        stop, kept = self.run(HIT_FROM, {RETURN},
                              {UC_X86_REG_ESI: PLAYERS[shooter] + COLLECTOR, UC_X86_REG_EBP: bullet,
                               UC_X86_REG_EBX: victim, UC_X86_REG_EDI: 0x0D0D0D0D, UC_X86_REG_ESP: sp})
        assert struct.unpack("<BxxxIf", self.emu.mem_read(PLAYERS[shooter] + COLLECTOR, 12))[:2] == (victim, time)
        return kept

    def draw(self, player):
        """st_draw's draw of `player`'s collector: the markers the hud got."""
        self.markers = []
        _, kept = self.run(DRAW_FROM, {DRAW_DONE},
                           {UC_X86_REG_EBP: PLAYERS[player], UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_ESI: 0x05050505,
                            UC_X86_REG_ESP: STACK + 0x8000})
        return [m[2] for m in self.markers if m[0] == b"root.crosshair_enemy_hit" and m[1] == 4 and not m[3]], kept

    def kill_walk(self, events, shooter=2):
        """The game world's tick handing its world events (each (type, victim,
        killer, time)) to the kill marker (0x1FB6C0, as 0x1F7FF6 calls it: the
        bound the newest ticked time, stock's delay 0): the kill markers the hud
        got."""
        node = KILL_EVENTS
        self.u32(KILL_LIST, node if events else 0)
        for n, (kind, victim, killer, time) in enumerate(events):
            self.emu.mem_write(node, bytes([victim, killer]))
            self.u32(node + 0x18, kind)
            self.u32(node + 0x1C, node + 0x40 if n + 1 < len(events) else 0)
            self.u32(node + 0x24, time)
            self.emu.mem_write(node + 0x28, b"\x01" if kind == HIT_EVENT else b"\x00")
            node += 0x40
        self.u32(KILL_UI + 0x9C, KILL_UI + 0x100)
        self.u32(KILL_UI + 0x100 + 0x53AC, KILL_UI + 0x400)
        self.u32(KILL_UI + 0x400 + 4, PLAYERS[shooter])
        self.u32(PLAYERS[shooter] + 0xD0, 100)    # its reference count, never 0 here
        self.u32(KILL_UI + 0x1FC + 8, HOLDER)
        self.u32(GAME_WORLD + 0x568, KILL_UI)
        self.emu.mem_write(SHOW_KILL_MARKER, b"\x01")
        sp = STACK + 0x8000
        self.u32(sp, RETURN)
        self.u32(sp + 4, self.u32(GAME_WORLD + 0x564))
        self.u32(sp + 8, GAME_WORLD + 0x564)
        self.markers = []
        self.run(KILL_WALK, {RETURN}, {UC_X86_REG_ECX: KILL_LIST - 0x12744, UC_X86_REG_EDX: max(e[3] for e in events),
                                      UC_X86_REG_ESP: sp})
        return [m[2] for m in self.markers if m[0] == b"root.crosshair_enemy_hit" and m[1] == 4 and m[3]]


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    built = bytes(built)
    installs = {h[0]: h for h in base.rust_hooks(original)}
    for va, site in SITES.items():
        at = base.file_offset(va)
        check(va in installs and built[at] == 0xE9 and original[at] != 0xE9, f"{site} ({va:#x}): a jump to the build's hook")
    for start, end in (KILL_MARKER, DRAW_BEFORE, DRAW_AFTER):
        a, b = base.file_offset(start), base.file_offset(end)
        check(built[a:b] == original[a:b], f"{start:#x}..{end:#x}: the original's bytes")

    stock, ours = Game(original), Game(built)

    def both(what, steps, stock_shows, ours_shows, shooter=2):
        """Runs `steps(game)` in each, then a draw; the markers shown."""
        seen = []
        for game in (stock, ours):
            kept = steps(game)
            markers, drawn = game.draw(shooter)
            seen.append(markers)
            if game is stock:
                regs = kept, drawn
            else:
                check((kept, drawn) == regs, f"{what}: the registers the game keeps are the original's")
        check(seen == [[round(x, 4) for x in shows] for shows in (stock_shows, ours_shows)], f"{what}: the original shows {stock_shows}, the build {ours_shows}")

    def clear(time, fraction, shooter=2, victim=VICTIM):
        return lambda game: game.hit(shooter, game.bullet(), time, fraction, victim)

    def through(material, time, fraction, kind=RANGED, receiver=0, shooter=2, victim=VICTIM):
        def steps(game):
            bullet = game.bullet(kind)
            stop, ebp, _ = game.pierce(bullet, material, receiver)
            assert stop == PIERCE_ON and ebp == 2, "check_collision goes on, pierced"
            return game.hit(shooter, bullet, time, fraction, victim)
        return steps

    def then(*steps):
        def run(game):
            kept = None
            for step in steps:
                kept = step(game)
            return kept
        return run

    for game in (stock, ours):
        game_kept = [game.pierce(game.bullet(), CONCRETE, pierced=pierced) for pierced in (True, False)]
        if game is stock:
            pierce_regs = game_kept
    check([(s, e) for s, e, _ in pierce_regs] == [(PIERCE_ON, 2), (PIERCE_STOPPED, 0x0BADF00D)]
          and pierce_regs == game_kept, "check_collision after try_pierce: on when pierced, stopped when not, "
          "the registers the original's")

    both("a clear hit", clear(100, 0.2), [0.2], [0.2])
    both("a round through a wall (concrete, armour 0.23)", through(CONCRETE, 150, 0.4), [0.4], [])
    both("the next frame", lambda game: None, [], [])
    both("the next clear hit", clear(200, 0.6), [0.6], [0.6])
    both("a clear hit on another victim (8)", clear(205, 0.2, victim=8), [0.2], [0.2])
    both("through a wall on the first (7)", through(CONCRETE, 210, 0.5), [0.5], [])
    both("a clear hit on 8 again, the damage of its first", clear(215, 0.2, victim=8), [0.2], [0.2])
    both("a clear hit not drawn, then a walled one a step later",
         then(clear(217, 0.3), through(CONCRETE, 218, 0.35)), [0.35], [0.35])
    both("a round through glass_hard (armour 0.10)", through(GLASS_HARD, 220, 0.65), [0.65], [])
    for n, open_ in enumerate((0.0, -0.0)):
        both(f"a round through glass, foliage, cloth, paper (armour {open_})", through(open_, 250 + n, 0.7 + n / 100),
             [0.7 + n / 100], [0.7 + n / 100])
    both("a round through a body (a hit receiver)", through(CONCRETE, 300, 0.75, receiver=0x5A5A0000), [0.75], [0.75])
    both("a gun-stock (blunt) through a wall", through(CONCRETE, 320, 0.8, kind=BLUNT), [0.8], [0.8])
    both("a walled pellet, then a clear one, one step", then(through(CONCRETE, 350, 0.82), clear(350, 0.84)), [0.84], [0.84])
    both("a clear pellet, then a walled one, one step", then(clear(400, 0.86), through(CONCRETE, 400, 0.88)), [0.88], [0.88])
    both("two walled pellets, one step", then(through(CONCRETE, 450, 0.9), through(CONCRETE, 450, 0.92)), [0.92], [])
    both("a clear hit after them", clear(500, 1.0), [1.0], [1.0])

    def stopped(game):
        bullet = game.bullet()
        stop, _, _ = game.pierce(bullet, CONCRETE, pierced=False)
        assert stop == PIERCE_STOPPED
        return game.hit(2, bullet, 550, 0.3)
    both("a bullet try_pierce stopped, then its hit", stopped, [0.3], [0.3])

    both("place 5's clear hit", clear(600, 0.4, shooter=5), [0.4], [0.4], shooter=5)
    both("place 2 through a wall, the same victim and step as place 5's next clear hit",
         then(through(CONCRETE, 650, 0.5), clear(650, 0.45, shooter=5)), [0.5], [])
    both("place 5's collector drawn", lambda game: None, [0.45], [0.45], shooter=5)
    both("place 5 through a wall", through(CONCRETE, 700, 0.55, shooter=5), [0.55], [], shooter=5)
    both("place 2's clear hit", clear(700, 0.6), [0.6], [0.6])

    both("a lethal round through a wall", through(CONCRETE, 800, 1.0), [1.0], [])
    kill = [(HIT_EVENT, VICTIM, 2, 800), (KILL_EVENT, VICTIM, 2, 800)]
    check([game.kill_walk(kill) for game in (stock, ours)] == [[1.0], [1.0]],
          "its kill event: the kill marker (1.0, killed) in the original and the build")
    check([game.kill_walk(kill) for game in (stock, ours)] == [[], []], "the next frame: no second kill marker")

    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
