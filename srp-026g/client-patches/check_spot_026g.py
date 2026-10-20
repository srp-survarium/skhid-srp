#!/usr/bin/env python3
"""Run the original's and patch 38's spot (base_player::apply_new_input,
0x2D86E0) on made-up players, batteries and ray hits.

    python check_spot_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are. The
input apply runs as machine code from its entry to the end of the spot and
use blocks (0x2D8991), with the game's own spottable code: base_player's
set_spotted and can_be_spotted_by (0x2DC0B0, 0x2DC0C0, vtable 0xA10B64),
victory_item_core's (0x301C40, 0x301CA0, vtable 0xA13EE0) and its rigid
body's owner (vtable 0xA13EEC, slots 1 and 2 = 0x300040), and between
inputs the player's tick's countdown of the spot field (0x2DAB44..). The
ray query, the hits' collision groups, other owners, the geometry's
gather (0x2DC390), the allocator, process_quick_slots and the match
statistics' event sink are stubs. Says nothing about which hits the real
collision world returns, or in what order.

Each case is a few ticks of one spotter's inputs. The original must give
the original's answers (a 3 s wait after any spot; a battery's body
spots nothing), the build: no wait (the field +34Ch 0 after every tick),
the body spots the battery as its use box does, and everything else as
the original.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EIP, UC_X86_REG_ESP, \
    UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_FPSW

import patch_026g as base

APPLY = 0x2D86E0                         # base_player::apply_new_input(tick, &input), thiscall, `ret 8`
APPLIED = 0x2D8991                       # after the spot and the use blocks
COUNTDOWN = 0x2DAB44                     # the player's tick: the spot field counted down, `ret 4`
QUICK_SLOTS = 0x2D8BB0                   # process_quick_slots, `ret 0Ch`: a stub here
GATHER = 0x2DC390                        # query_objects_by_type: the geometry's spottables
ALLOCATOR = 0xA8001C
PLAYER_SPOTTABLE, ITEM_SPOTTABLE, BODY_OWNER = 0xA10B64, 0xA13EE0, 0xA13EEC
FIELD, TIMER, MASK = 0x34C, 0x348, 0x1A4  # the spot wait (spotter); spotted for 10 s (player); teams (battery)
SPOT_BIT = 1 << 28
TICK_MS = 50

MEMORY, STACK, STOP, STUB = 0x20000000, 0x21008000, 0x22000000, 0x22000100
PHYS, VT, HITS, INPUT, TEAM_A, TEAM_B, WORLD = (MEMORY + 0x40000 + i * 0x1000 for i in range(7))
RAY, GROUP, OWNER_PLAYER, OWNER_GEOMETRY, FREE, EVENT, NOTHING = (STUB + i * 0x10 for i in range(7))
REGISTERS = (UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI)


class Match:
    """One game's world: the spotter, the others, a battery, the stubs."""

    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(MEMORY, 0x80000, UC_PROT_ALL)
        emu.mem_map(0x21000000, 0x10000, UC_PROT_ALL)
        emu.mem_map(STOP, 0x10000, UC_PROT_ALL)
        self.next = MEMORY
        for at, value in ((PHYS, VT), (VT + 0x4C, RAY), (VT + 0x100, GROUP),
                          (VT + 0x204, OWNER_GEOMETRY), (VT + 0x208, OWNER_PLAYER),
                          (ALLOCATOR, VT + 0x300), (VT + 0x318, FREE),
                          (VT + 0x400, VT + 0x400), (VT + 0x40C, EVENT), (WORLD + 0x18FA8, VT + 0x400), (TEAM_B + 0x45EC, 1)):
            self.word(at, value)
        self.spotter = self.player(TEAM_A, index=3, kit=1)
        self.hits, self.rays, self.events, self.freed = [], 0, [], []
        emu.hook_add(UC_HOOK_CODE, self.stub)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def block(self, size=0x1000):
        at, self.next = self.next, self.next + size
        return at

    def player(self, team, index=0, kit=0):
        p = self.block(0x2000)
        self.emu.mem_write(p + 0x33C, b"\1")              # in play
        self.emu.mem_write(p + 0x114, bytes([index, kit]))
        self.word(p + 0x134, 1)                           # no use this tick (the use block is skipped)
        self.word(p + 0x350, PHYS)
        self.word(p + 0x6B8, team)
        self.word(p + 0x11C, PLAYER_SPOTTABLE)
        self.emu.mem_write(p + 0x46C, struct.pack("<3f", 0.0, 0.0, 1.0))
        self.emu.mem_write(p + 0x47C, struct.pack("<3f", 1.0, 2.0, 3.0))
        return p

    def battery(self):
        item = self.block(0x1000)
        self.word(item + 0x54, ITEM_SPOTTABLE)
        self.word(item + 0x58, BODY_OWNER)
        self.word(item + 0x2AC, item + 0x800)
        self.word(item + 0x800 + 0x158, WORLD)
        return item

    def thing(self, group, owner):
        obj = self.block(0x100)
        self.word(obj, VT + 0x100)
        self.word(obj + 4, group)
        self.word(obj + 0xC, owner)
        return obj

    def owner(self, player=0, geometry=0):
        o = self.block(0x100)
        for at, value in ((o, VT + 0x200), (o + 4, player), (o + 8, geometry)):
            self.word(at, value)
        return o

    # The hits a ray returns, nearest first.
    def body(self, item):                              # the battery's rigid body, group 0A18h (0x301CF5)
        return self.thing(0xA18, item + 0x58)

    def use_box(self, item):                           # !victory_item_collision!, filter 2176 = 880h
        geometry = self.block(0x100)
        self.word(geometry, item + 0x54)               # its first spottable: the battery
        return self.thing(0x880, self.owner(geometry=geometry))

    def hitbox(self, target):                          # a player's
        return self.thing(0x808, self.owner(player=target))

    def stub(self, emu, address, size, _):
        sp, this = emu.reg_read(UC_X86_REG_ESP), emu.reg_read(UC_X86_REG_ECX)
        if address in (STOP, APPLIED):
            emu.emu_stop()
        elif address == RAY:                           # (from, dir, length, &out, group, filter), `ret 18h`
            assert (self.read(sp + 4), self.read(sp + 8)) == (self.spotter + 0x47C, self.spotter + 0x46C)
            assert struct.unpack("<f", emu.mem_read(sp + 12, 4))[0] == 100.0
            assert (self.read(sp + 20), self.read(sp + 24)) == (0xFFFF, 0x80A)
            assert emu.reg_read(UC_X86_REG_FPSW) & 0x3800 == 0, "x87 stack not empty"
            self.rays += 1
            out = self.read(sp + 16)
            for i, obj in enumerate(self.hits):
                self.word(HITS + 40 * i + 0x1C, obj)
            for k, value in enumerate((HITS, HITS + 40 * len(self.hits), ALLOCATOR, HITS + 40 * len(self.hits))):
                self.word(out + 4 * k, value)
            self.returns(0, 24)
        elif address == GROUP:
            self.returns(self.read(this + 4))
        elif address == OWNER_PLAYER:
            self.returns(self.read(this + 4))
        elif address == OWNER_GEOMETRY:
            self.returns(self.read(this + 8))
        elif address == GATHER:                        # (geometry, 0x2DD760, 0), esi = &vector, 12 popped
            assert self.read(sp + 8) == 0x2DD760
            cell = self.block(0x10)
            self.word(cell, self.read(self.read(sp + 4)))
            vector = emu.reg_read(UC_X86_REG_ESI)
            self.word(vector, cell)
            self.word(vector + 4, cell + 4)
            self.returns(0, 12)
        elif address == FREE:
            self.freed.append(self.read(sp + 4))
            self.returns(0, 16)
        elif address == EVENT:                         # statistics: (place, kit, event, item), `ret 10h`
            self.events.append(tuple(self.read(sp + 4 + 4 * k) for k in range(4)))
            self.returns(0, 16)
        elif address == QUICK_SLOTS:
            self.returns(0, 12)

    def returns(self, value, pop=0):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        self.emu.reg_write(UC_X86_REG_EAX, value)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pop)

    def apply(self, actions, hits=(), mouse=(0, 0)):
        """One input of the spotter; the ray, if it is cast, returns `hits`."""
        emu = self.emu
        self.hits = list(hits)
        emu.mem_write(INPUT, struct.pack("<HHI", *mouse, actions))
        for at, value in ((STACK, STOP), (STACK + 4, 0x777), (STACK + 8, INPUT)):
            self.word(at, value)
        emu.reg_write(UC_X86_REG_ESP, STACK)
        emu.reg_write(UC_X86_REG_ECX, self.spotter)
        for register in REGISTERS:
            emu.reg_write(register, 0x12345678)
        rays, freed = self.rays, len(self.freed)
        emu.emu_start(APPLY, 0, count=40000)
        eip = emu.reg_read(UC_X86_REG_EIP)
        assert eip in (STOP, APPLIED), hex(eip)
        if eip == STOP:                                # the same input as before: nothing applied
            assert emu.reg_read(UC_X86_REG_ESP) == STACK + 12
            assert all(emu.reg_read(r) == 0x12345678 for r in (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EBP))
        else:                                          # the frame as the original leaves it there
            assert emu.reg_read(UC_X86_REG_EDI) == self.spotter
            assert emu.reg_read(UC_X86_REG_ESP) == ((STACK - 4) & ~7) - 0x3C - 12
            assert emu.reg_read(UC_X86_REG_EBP) == STACK - 4
        assert emu.reg_read(UC_X86_REG_FPSW) & 0x3800 == 0, "x87 stack left non-empty"
        if self.rays > rays:                           # the hits are freed whatever was found
            assert HITS in self.freed[freed:]
        return eip == APPLIED

    def tick(self):
        """The spotter's tick's countdown of the spot field (dt 50 ms)."""
        emu = self.emu
        for k, value in enumerate((0x11111111, 0x22222222, 0x33333333, 0x44444444, STOP, 0)):
            self.word(STACK + 4 * k, value)
        emu.reg_write(UC_X86_REG_ESP, STACK)
        emu.reg_write(UC_X86_REG_EBX, self.spotter)
        emu.reg_write(UC_X86_REG_EBP, TICK_MS)
        emu.emu_start(COUNTDOWN, 0, count=100)
        assert emu.reg_read(UC_X86_REG_EIP) == STOP and emu.reg_read(UC_X86_REG_ESP) == STACK + 24

    def wait(self):
        return self.read(self.spotter + FIELD)

    def mask(self, item):
        return self.read(item + MASK)

    def timer(self, p):
        return self.read(p + TIMER)


def run(data, patched):
    """Every case on one build; returns how many were checked."""
    checks = 0
    stock = not patched

    def expect(ok, what):
        nonlocal checks
        assert ok, f"{'build' if patched else 'original'}: {what}"
        checks += 1

    def spot_event(m, item):
        return (3, 1, 4, item)                         # the spotter's place 3, kit 1, event 4

    # A miss, then the next press on a battery (its use box), two ticks later.
    m = Match(data)
    item = m.battery()
    m.apply(SPOT_BIT, [])
    m.tick()
    expect(m.rays == 1 and m.wait() == (3000 - TICK_MS if stock else 0), "a miss: the wait")
    m.apply(0)
    m.tick()
    m.apply(SPOT_BIT, [m.use_box(item)])
    m.tick()
    expect(m.rays == (1 if stock else 2), "the press after a miss casts a ray in the build alone")
    expect(m.mask(item) == (0 if stock else 1) and m.events == ([] if stock else [spot_event(m, item)]),
           "the press after a miss spots the battery in the build alone")
    expect(m.wait() == (3000 - 3 * TICK_MS if stock else 0), "the wait after the second press")
    # ... and in the original, once the 3 s are over.
    if stock:
        for _ in range(57):
            m.apply(0)
            m.tick()
        expect(m.wait() == 0, "the original's wait is over after 60 ticks")
        m.apply(SPOT_BIT, [m.use_box(item)])
        expect(m.rays == 2 and m.mask(item) == 1, "the original spots after the 3 s")

    # One press: the same input on the next ticks (the key held, no mouse
    # move) is no new input: nothing is applied again, in both.
    m = Match(data)
    enemy = m.player(TEAM_B)
    for t in range(5):
        expect(m.apply(SPOT_BIT, [m.hitbox(enemy)]) == (t == 0), f"the same input again, tick {t}")
        m.tick()
    expect(m.rays == 1 and m.timer(enemy) == 10000, "one press, one ray; the enemy spotted for 10 s")

    # A spot every tick (a press every tick, the mouse moving), on a battery
    # (even ticks) and an enemy (odd): the original casts a ray at ticks 0
    # and 60 (both on the battery), the build one a tick; the battery's
    # event is raised at each of its spots, the enemy's 10 s start again.
    m = Match(data)
    enemy, item = m.player(TEAM_B), m.battery()
    for t in range(80):
        m.apply(SPOT_BIT, [m.hitbox(enemy)] if t % 2 else [m.use_box(item)], mouse=(t + 1, 0))
        m.tick()
        expect(stock or m.wait() == 0, "the build's wait stays 0")
    expect(m.rays == (2 if stock else 80), f"a spot every tick: {m.rays} rays")
    expect(m.events == [spot_event(m, item)] * (2 if stock else 40), f"a spot every tick: {len(m.events)} events")
    expect(m.mask(item) == 1 and m.timer(enemy) == (0 if stock else 10000), "a spot every tick: who is spotted")
    m.word(enemy + TIMER, 5)
    m.apply(SPOT_BIT, [m.hitbox(enemy)], mouse=(99, 0))
    expect(m.timer(enemy) == (5 if stock else 10000), "the enemy's 10 s again at the next tick's spot")

    # A battery's body alone.
    m = Match(data)
    item = m.battery()
    m.apply(SPOT_BIT, [m.body(item)])
    expect(m.mask(item) == (0 if stock else 1) and m.events == ([] if stock else [spot_event(m, item)]),
           "the battery's body spots it in the build alone")
    # The body, the floor behind it, the use box: the original stops at the
    # floor (the body's 8h and the floor's 2h); the build spots at the body.
    m = Match(data)
    item = m.battery()
    m.apply(SPOT_BIT, [m.body(item), m.thing(0x2, 0), m.use_box(item)])
    expect(m.mask(item) == (0 if stock else 1) and len(m.events) == (0 if stock else 1),
           "body, floor, use box: the original stops at the floor")
    # The use box first: both spot it once.
    for hits in (lambda m, item: [m.use_box(item)], lambda m, item: [m.use_box(item), m.body(item)]):
        m = Match(data)
        item = m.battery()
        m.apply(SPOT_BIT, hits(m, item))
        expect(m.mask(item) == 1 and m.events == [spot_event(m, item)], "the use box spots it in both")
    # Spotted by the other team: its bit (2).
    m = Match(data)
    item = m.battery()
    m.word(m.spotter + 0x6B8, TEAM_B)
    m.apply(SPOT_BIT, [m.body(item) if patched else m.use_box(item)])
    expect(m.mask(item) == 2, "team 1's bit is 2")
    # Already spotted by one's team: the bit stays, the event is raised again
    # (as a use box's spot after the original's 3 s).
    m = Match(data)
    item = m.battery()
    m.word(item + MASK, 1)
    m.apply(SPOT_BIT, [m.body(item) if patched else m.use_box(item)])
    expect(m.mask(item) == 1 and m.events == [spot_event(m, item)], "a spot of a battery one's team sees")
    # A wall before the battery: nothing.
    m = Match(data)
    item = m.battery()
    m.apply(SPOT_BIT, [m.thing(0xA, 0), m.body(item), m.use_box(item)])
    expect(m.mask(item) == 0 and not m.events and m.rays == 1, "a wall before the battery")

    # Players: an enemy; a teammate (not spottable, the walk goes on); a
    # wall before the enemy; another spottable thing with neither a player
    # nor a geometry behind it.
    m = Match(data)
    enemy, mate = m.player(TEAM_B), m.player(TEAM_A)
    m.apply(SPOT_BIT, [m.hitbox(enemy)])
    expect(m.timer(enemy) == 10000 and m.wait() == (3000 if stock else 0), "an enemy: spotted for 10 s")
    m = Match(data)
    enemy, mate = m.player(TEAM_B), m.player(TEAM_A)
    m.apply(SPOT_BIT, [m.hitbox(mate)])
    expect(m.timer(mate) == 0, "a teammate: not spotted")
    m = Match(data)
    enemy, mate = m.player(TEAM_B), m.player(TEAM_A)
    m.apply(SPOT_BIT, [m.hitbox(mate), m.hitbox(enemy)])
    expect(m.timer(mate) == 0 and m.timer(enemy) == 10000, "a teammate before an enemy: the enemy")
    m = Match(data)
    enemy = m.player(TEAM_B)
    m.apply(SPOT_BIT, [m.thing(0xA, 0), m.hitbox(enemy)])
    expect(m.timer(enemy) == 0, "a wall before an enemy")
    m = Match(data)
    enemy, item = m.player(TEAM_B), m.battery()
    m.apply(SPOT_BIT, [m.thing(0x800, m.owner()), m.hitbox(enemy)])
    expect(m.timer(enemy) == 10000, "a spottable hit with no owner behind it, then an enemy")
    # No spot bit: no ray; the field as it was.
    m = Match(data)
    m.apply(0x1)
    expect(m.rays == 0 and m.wait() == 0, "no spot bit: no ray")
    return checks


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    patched, _ = base.build(original, "section", code, None, late_join)
    for data, name in ((original, "the original"), (patched, "the build")):
        print(f"patch 38, {name}: {run(data, data is patched)} checks as expected")


if __name__ == "__main__":
    main()
