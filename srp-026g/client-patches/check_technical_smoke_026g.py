#!/usr/bin/env python3
"""Patch 180's Dense Smoke (402) and Toxic Smoke (441) in Unicorn on the built exe
(docs/notes/technical-skills.md, "Dense Smoke and Toxic Smoke").

Each hook runs at its site of the patched exe over a constructed grenade, stack, thrower and
victim (made-up memory, the game's own code around the sites):

- the grenade's tail: the cook asks 270h bytes (0x1F6642); the ctor's last store (0x2483BE) clears
  the tail; the cook's query (0x25E154) of a smoke grenade asks four requests, the fourth
  fx/explosions/smoke_grenade_toxic of class 3Dh, and goes past the native call; another
  grenade's is the native three; the callback (0x25E308) keeps the fourth result with a reference;
  the destructor (0x2483F0) releases it (0x0E0210 at the last reference);
- the blast (0x248720): a smoke of a thrower with Toxic Smoke keeps the cloud (its explode time,
  its centre); a thrower without the skill, or a grenade that damages, keeps none;
- the saved state: grenade_core::serialize (0x303DA0) writes bit 3 and the cloud's 16 bytes after
  the thrown fields; the load (0x30406C to 0x30419D) reads them back to the same cloud and the
  reader's end; without bit 3 a load clears a cloud the grenade held (a rollback's);
- the hits (battery.carrier_tick, 0x2DAAD3): an enemy whose eyes are in the cloud gets
  base_player::hit (stubbed) with the face, kind 4, the tick's dose, the thrower's place and the
  stack; a teammate, a player outside, a dead one or a tick before the cloud is full: none;
- the presentation: the play (0x248908) gives the green instance in EAX for Toxic Smoke and the
  stock one otherwise; the particle tick (0x35DAF5) pauses a Dense Smoke thrower's instance from 2 s
  after the play for rank x 2.9 s, then resumes it.

A constructed match, not a played one. Usage:
    python check_technical_smoke_026g.py ORIGINAL_EXE
"""
import struct
import sys

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_XMM0)

import patch_026g as base
import check_hide_ui_026g as hide

DATA = 0x30000000
STACK = hide.STACK + 0x8000
GRENADE_VTABLE, GRENADE_SET_VTABLE = 0xA046A0, 0xA0B56C
QUERY, RELEASE, HIT, W_U8, W_U32 = 0xE5B60, 0xE0210, 0x2D9AE0, 0x1F3DE0, 0x1F48A0
TOXIC, DENSE = 995328, 4  # a rank of Toxic Smoke (441), of Dense Smoke (402) in modifier 47


def f32(value):
    return struct.unpack('<I', struct.pack('<f', value))[0]


class Smoke(hide.Run):
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_map(DATA, 0x100000)
        self.next = DATA
        self.queries, self.released, self.hits, self.written = [], [], [], bytearray()
        for at in (QUERY, RELEASE, HIT, W_U8, W_U32):
            self.emu.hook_add(UC_HOOK_CODE, self.stubbed, begin=at, end=at)

    def block(self, size):
        at = self.next
        self.next += (size + 0xFF) & ~0xFF
        self.emu.mem_write(at, bytes(size))
        return at

    def stubbed(self, emu, address, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        ret, pops = self.read(sp), 0
        if address == QUERY:      # EAX the requests, ECX the count; cdecl, the caller cleans
            requests, count = emu.reg_read(UC_X86_REG_EAX), emu.reg_read(UC_X86_REG_ECX)
            self.queries.append([(self.text(self.read(requests + 8 * i)).decode(), self.read(requests + 8 * i + 4))
                                 for i in range(count)])
        elif address == RELEASE:  # ECX the object, EAX its count
            self.released.append(emu.reg_read(UC_X86_REG_ECX))
        elif address == HIT:      # thiscall, eight stack words, ret 20h
            args = [self.read(sp + 4 + 4 * i) for i in range(8)]
            self.hits.append((emu.reg_read(UC_X86_REG_ECX), args))
            pops = 0x20
        else:                     # w<u8> / w<u32>: EAX the value; writer + 4 names, cdecl? ret 14h
            self.written += bytes(emu.mem_read(emu.reg_read(UC_X86_REG_EAX), 1 if address == W_U8 else 4))
            pops = 0x14
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, ret)

    def at(self, start, stop, regs=None, stack=(), xmm=None):
        sp = STACK - 0x400
        for i, value in enumerate(stack):
            self.word(sp + 4 * i, value)
        if xmm is not None:
            self.emu.reg_write(UC_X86_REG_XMM0, f32(xmm))
        for reg, value in (regs or {}).items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.emu_start(start, stop, count=2_000_000)
        eip = self.emu.reg_read(UC_X86_REG_EIP)
        assert eip == stop, (hex(start), hex(eip))
        return sp

    def call(self, start, args, regs=None):
        sp = STACK - 0x400
        self.word(sp, hide.SENTINEL)
        for i, value in enumerate(args):
            self.word(sp + 4 + 4 * i, value)
        for reg, value in (regs or {}).items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.emu_start(start, hide.SENTINEL, count=2_000_000)
        assert self.emu.reg_read(UC_X86_REG_EIP) == hide.SENTINEL, hex(self.emu.reg_read(UC_X86_REG_EIP))

    # -- the fixture -------------------------------------------------------------------------------

    def player(self, place, team, ranks=0, feet=(0.0, 0.0, 0.0), alive=1):
        """A kit-0 player at `place` of `team`, `ranks` in its kit's modifier 47, an empty inventory."""
        player, profile, inventory = self.block(0x800), self.block(0x4800), self.block(0x200)
        self.word(player + 0x6B8, profile)
        self.emu.mem_write(player + 0x114, bytes([place, 0]))
        self.emu.mem_write(player + 0x33C, bytes([alive]))
        self.word(profile + 0x45EC, team)
        self.word(profile + 0xF4 + 0xBC, f32(float(ranks)))
        self.word(player + 0x10C, inventory)
        self.word(inventory + 0x174, player + 0x108)
        self.emu.mem_write(player + 0x43C, struct.pack('<3f', *feet))
        return player

    def stack_of(self, player, damage=0.0, slot=3):
        """A grenade stack in `player`'s slot with one grenade object of the game's class."""
        inventory = self.read(player + 0x10C)
        item, objects, grenade = self.block(0x300), self.block(0x10), self.block(0x270)
        self.word(item, GRENADE_SET_VTABLE)
        self.word(item + 0x250, inventory)
        self.word(item + 0x268, objects)
        self.word(item + 0x26C, objects + 4)
        self.word(item + 0x2A8, f32(damage))
        self.word(objects, grenade)
        self.word(inventory + 0x10C + 4 * slot, item)
        self.word(grenade, GRENADE_VTABLE)
        self.word(grenade + 0xC, item)
        return item, grenade

    def cloud(self, grenade):
        word, blast = self.read(grenade + 0x258), self.read(grenade + 0x25C)
        centre = struct.unpack('<3f', self.emu.mem_read(grenade + 0x260, 12))
        return (blast, centre) if word == 1 else None if word == 0 else ('bad', word)


def check_tail(data):
    s = Smoke(data)
    s.at(0x1F6642, 0x1F6647)
    assert s.emu.reg_read(UC_X86_REG_EAX) == 0x270
    grenade = s.block(0x270)
    s.emu.mem_write(grenade + 0x258, b'\xAB' * 0x18)
    s.at(0x2483BE, 0x2483C5, {UC_X86_REG_EDI: grenade, UC_X86_REG_EAX: 0xFFFF})
    assert bytes(s.emu.mem_read(grenade + 0x258, 0x18)) == bytes(0x18)
    print('the grenade\'s tail: 270h bytes asked, the ctor clears it: PASS')

    # the cook's query: a smoke's four requests, another grenade's three (the native call)
    for path, extended in ((b'fx/explosions/smoke_grenade', True), (b'fx/explosions/rgd_5', False)):
        s.queries = []
        names = s.block(0x100)
        s.emu.mem_write(names, b'item/rdg\0'.ljust(0x40, b'\0') + path + b'\0')
        stack = [0x11, 0x22, 0, 0x44, 1] + [0] * 12 + [names, 0xF, names + 0x40, 0x3D, names, 0x100]
        stop = 0x25E162 if extended else 0x25E15D
        s.at(0x25E154, stop, stack=stack)
        if extended:
            assert s.queries == [[('item/rdg', 0xF), (path.decode(), 0x3D), ('item/rdg', 0x100),
                                  ('fx/explosions/smoke_grenade_toxic', 0x3D)]], s.queries
        else:
            assert s.queries == [] and s.emu.reg_read(UC_X86_REG_ECX) == 3
    print('the cook\'s query: a smoke grenade asks its green copy fourth, another grenade its own three: PASS')

    # the callback keeps the fourth with a reference; the dtor releases it
    results, green = s.block(0xA00), s.block(0x400)
    s.word(results + 0x38, 4)
    s.word(results + 0x9CC, green)
    s.word(green + 0xD0, 1)
    s.word(grenade, GRENADE_VTABLE)
    s.at(0x25E308, 0x25E30E, {UC_X86_REG_EDI: grenade, UC_X86_REG_EBX: results})
    assert s.read(grenade + 0x26C) == green and s.read(green + 0xD0) == 2
    s.word(green + 0xD0, 1)        # the result's own reference let go
    s.at(0x2483F0, 0x2483F9, {UC_X86_REG_ECX: grenade})
    assert s.read(grenade + 0x26C) == 0 and s.released == [green], s.released
    print('the cook\'s callback keeps the green instance (a reference), the destructor releases it: PASS')


def check_blast_and_save(data):
    for ranks, damage, keeps in ((2 * TOXIC, 0.0, True), (0, 0.0, False), (3 * TOXIC, 80.0, False),
                                 (3 * DENSE, 0.0, False)):
        s = Smoke(data)
        thrower = s.player(1, 0, ranks)
        _, grenade = s.stack_of(thrower, damage)
        s.word(grenade + 0x14C, 12_050)
        s.emu.mem_write(grenade + 0x1C0, struct.pack('<3f', 4.0, 1.25, -9.5))
        s.at(0x248720, 0x248726, {UC_X86_REG_ECX: grenade})
        want = (12_050, (4.0, 1.25, -9.5)) if keeps else None
        assert s.cloud(grenade) == want, (ranks, damage, s.cloud(grenade))
    print('the blast: a smoke of a Toxic Smoke thrower keeps its cloud (time, centre); no skill, a damaging '
          'grenade, Dense Smoke alone: none: PASS')

    for thrown in (False, True):
        s = Smoke(data)
        thrower = s.player(1, 0, 2 * TOXIC)
        _, grenade = s.stack_of(thrower)
        s.word(grenade + 0x258, 1)
        s.word(grenade + 0x25C, 12_050)
        s.emu.mem_write(grenade + 0x260, struct.pack('<3f', 4.0, 1.25, -9.5))
        s.word(grenade + 0x14C, 20_000 if thrown else 0xFFFFFFFF)
        s.emu.mem_write(grenade + 0x154, b'\x00' if thrown else b'\x01')
        s.word(grenade + 4, 77)
        writer = s.block(0x40)
        s.call(0x303DA0, [writer, 0], {UC_X86_REG_ECX: grenade + 0x10})
        cloud = struct.pack('<I3f', 12_050, 4.0, 1.25, -9.5)
        head = bytes([2 | 8]) + struct.pack('<II', 20_000, 77) if thrown else bytes([4 | 8])
        assert bytes(s.written) == head + cloud, bytes(s.written).hex()
        # the load, after the thrown fields: the same cloud, the reader at its end
        loaded = s.player(1, 0)
        _, other = s.stack_of(loaded)
        stream = s.block(0x40)
        s.emu.mem_write(stream, bytes(s.written))
        reader = s.block(0x10)
        s.word(reader, stream)
        s.word(reader + 4, stream + len(head))
        s.word(reader + 8, len(s.written))
        stack = [0] * 4 + [s.written[0] << 24] + [0] * 0x20
        s.at(0x30406C, 0x30419D, {UC_X86_REG_EBX: other + 0x10, UC_X86_REG_ESI: reader}, stack=stack)
        assert s.cloud(other) == (12_050, (4.0, 1.25, -9.5)) and s.read(reader + 4) == stream + len(s.written)
        # a world without the bit clears the grenade's cloud
        s.word(reader + 4, stream + len(head))
        s.at(0x30406C, 0x30419D, {UC_X86_REG_EBX: other + 0x10, UC_X86_REG_ESI: reader},
             stack=[0] * 4 + [(s.written[0] & 7) << 24] + [0] * 0x20)
        assert s.cloud(other) is None and s.read(reader + 4) == stream + len(head)
    print('the saved cloud: bit 3 and 16 bytes after the thrown fields, read back to the same cloud; '
          'a world without it clears it: PASS')


def check_hits(data):
    def match(victim_team=1, feet=(4.0, -0.2, -6.0), alive=1, now=15_000, elapsed=50):
        s = Smoke(data)
        thrower = s.player(1, 0, 2 * TOXIC)
        item, grenade = s.stack_of(thrower)
        s.word(grenade + 0x258, 1)
        s.word(grenade + 0x25C, 12_050)
        s.emu.mem_write(grenade + 0x260, struct.pack('<3f', 4.0, 0.0, -9.5))
        victim = s.player(2, victim_team, feet=feet, alive=alive)
        core, places = s.block(0x19000), s.block(0x100)
        for p in (thrower, victim):
            s.word(p + 0x120, core)
        s.word(core + 0x18458, places)
        s.word(core + 0x1845C, places + 3 * 0x30)
        s.word(core + 0x18FD8, 0b110)
        s.word(places + 0x30, thrower)
        s.word(places + 0x60, victim)
        s.at(0x2DAAD3, 0x2DAAD8, {UC_X86_REG_EBX: victim, UC_X86_REG_EDI: now, UC_X86_REG_EBP: elapsed},
             stack=[0] * 16)
        return s, thrower, victim, item

    s, thrower, victim, item = match()
    assert len(s.hits) == 1, s.hits
    receiver, (time, attacker, part, kind, damage, piercing, initiator, source) = s.hits[0]
    dose = struct.unpack('<f', struct.pack('<I', damage))[0]
    assert (receiver, time, attacker, kind, piercing, initiator, source) == \
        (victim + 0x118, 15_000, thrower + 0x110, 4, 0, 0, item), s.hits
    assert s.text(s.read(part + 0x4C)) == b'face' and abs(dose - 0.06 * 0.05) < 1e-7, dose
    for case in (dict(victim_team=0), dict(feet=(13.0, 0.0, -9.5)), dict(alive=0), dict(now=13_900)):
        s, *_ = match(**case)
        assert s.hits == [], (case, s.hits)
    print('the hits: an enemy whose eyes are in the cloud takes the face, toxic, 0.06 a second x 50 ms, '
          'the thrower\'s place and stack; a teammate, outside, dead, before the cloud is full: none: PASS')


def check_forest_protection(data):
    """`smoke.anomaly_zones` (0x332833, where a hit with an owner reads the victim's damage zones): in
    a toxic cloud the hook leaves by its exit, where modifier 41 scales the damage."""
    SCALED, PLAIN = 0x33283C, 0x332874

    def reached(victim_team=1, feet=(4.0, -0.2, -6.0), now=15_000, zones=0):
        s = Smoke(data)
        thrower = s.player(1, 0, 2 * TOXIC)
        _, grenade = s.stack_of(thrower)
        s.word(grenade + 0x258, 1)
        s.word(grenade + 0x25C, 12_050)
        s.emu.mem_write(grenade + 0x260, struct.pack('<3f', 4.0, 0.0, -9.5))
        victim = s.player(2, victim_team, feet=feet)
        s.emu.mem_write(victim + 0x2E4, bytes([zones]))
        core, places = s.block(0x19000), s.block(0x100)
        for p in (thrower, victim):
            s.word(p + 0x120, core)
        s.word(core + 0x18458, places)
        s.word(core + 0x1845C, places + 3 * 0x30)
        s.word(core + 0x18FD8, 0b110)
        s.word(places + 0x30, thrower)
        s.word(places + 0x60, victim)
        hit = s.block(0x80)
        s.word(hit + 0x3C, now)
        ends = []

        def stop(emu, address, *_):
            ends.append(address)
            emu.emu_stop()
        for at in (SCALED, PLAIN):
            s.emu.hook_add(UC_HOOK_CODE, stop, begin=at, end=at)
        s.emu.reg_write(UC_X86_REG_EAX, victim)
        s.emu.reg_write(UC_X86_REG_EBP, hit)
        s.emu.reg_write(UC_X86_REG_ESP, STACK - 0x400)
        s.emu.emu_start(0x332833, 0, count=2_000_000)
        assert s.emu.reg_read(UC_X86_REG_EAX) == victim
        return ends[0]

    assert reached() == SCALED and reached(victim_team=0) == SCALED
    for case in (dict(feet=(13.0, 0.0, -9.5)), dict(now=13_900), dict(now=12_050 + 26_000)):
        assert reached(**case) == PLAIN, case
    assert reached(feet=(13.0, 0.0, -9.5), zones=1) == SCALED
    print('Forest Protection: a hit with an owner on a player in a toxic cloud (an enemy\'s or its team\'s) goes to '
          'modifier 41 as in a damage zone; outside, before the cloud is full or after it is gone: the game\'s path: PASS')


def check_presentation(data):
    for ranks, green_wanted in ((2 * TOXIC + 2 * DENSE, True), (2 * DENSE, False)):
        s = Smoke(data)
        thrower = s.player(1, 0, ranks)
        _, grenade = s.stack_of(thrower)
        stock, green = s.block(0x400), s.block(0x400)
        s.word(grenade + 0x21C, stock)
        s.word(grenade + 0x26C, green)
        s.at(0x248908, 0x24890E, {UC_X86_REG_ESI: grenade})
        played = s.emu.reg_read(UC_X86_REG_EAX)
        assert played == (green if green_wanted else stock), (hex(played), hex(green), hex(stock))
        # the particle world's ticks of the played instance: still for 2 s, then paused 5.8 s
        paused, waited, frames = 0.0, 0.0, 0
        dt = 1 / 30
        while frames < 600:
            s.at(0x35DAF5, 0x35DAFC, {UC_X86_REG_EDI: played}, xmm=dt)
            if s.read(played + 0x302, 1)[0]:
                paused += dt
            elif paused == 0:
                waited += dt
            frames += 1
        assert abs(waited - 2.0) <= 2 * dt and abs(paused - 5.8) <= 2 * dt, (waited, paused)
        assert s.read(played + 0x302, 1)[0] == 0
    print('the presentation: the green instance for Toxic Smoke, the stock one otherwise; Dense Smoke 2 holds '
          'the playing instance still from 2 s for 5.8 s, then resumes it: PASS')


def main():
    original = open(sys.argv[1], 'rb').read()
    patched, _ = base.build(original, 'section', open(base.default_bin('section'), 'rb').read(), None,
                            open(base.late_join_bin(), 'rb').read())
    patched = bytes(patched)
    check_tail(patched)
    check_blast_and_save(patched)
    check_hits(patched)
    check_forest_protection(patched)
    check_presentation(patched)


if __name__ == '__main__':
    main()
