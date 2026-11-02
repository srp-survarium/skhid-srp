#!/usr/bin/env python3
"""Patch 92 (battery_progress): a battery's carriers earn 2 points (40
hundredths of event 6) for each 10 m the battery comes nearer their team's
container than ever in its life, held on the battery and paid when it is
delivered there.

Runs the installed Rust hooks in the patched exe on made-up objects; the
batteries' lives are the patch's own, seen through the game's serialize:
- base_player::tick after the held object's tick (0x2DAAD3, to the push after
  the replayed mov ecx,[esp+10h] / push edi): the first carry of a life keeps
  both containers' bands and earns nothing; coming nearer holds a step a 10 m
  for the carrier's place and kit and changes no score and tells no listener;
  back and forth and a second carrier over the same ground earn nothing;
  nothing for a player holding no battery or another item, without a battery
  rule or statistics handler, or for a place or team that is none of the
  match's; the registers, the stack and every byte of the game's as the
  replayed instructions leave them;
- on_event's put-in (0x334CEA): every carrier of the container's team (EAX,
  the putter's) is paid what it holds in its own place and kit cell, the
  listener told; the other team's steps are dropped; the battery's life
  begins again; the count wraps as the game's; the owner team written as
  before;
- start_match (0x334999): every life begins again, the rule read as before;
- the game's own serialize (0x334DC0) and deserialize (0x334FB0) of the
  records, the hooks inside: each record's life (two progress bytes, the
  count of cells holding steps, each one's cell and steps) follows its
  first_finder, a load gives every life back, and a stream without them
  loads the record's own fields and a new life.
"""
import struct
import sys
from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import *
import patch_026g as base
import check_hide_ui_026g as hide

TICK, TICK_AFTER = 0x2DAAD3, 0x2DAAD8         # mov ecx,[esp+10h] (4) / push edi (1), then push ecx
START, START_AFTER = 0x334999, 0x33499F       # mov eax,[esi+0A38h] (6)
DELIVERED, DELIVERED_AFTER = 0x334CEA, 0x334CF0   # mov [ebx+0F8h],eax (6)
SERIALIZE, DESERIALIZE = 0x334DC0, 0x334FB0   # stdcall (manager), EDI the writer / EDX the reader; ret 4
NOTIFY = 0x2F5D40                             # notify_listener: eax the listener; place, event, amount; ret 0Ch
WRITERS = {0x1F3DE0: 1, 0x804E0: 4, 0x1F48A0: 4}   # w<u8>, w<f32>, w<u32>: eax the value; writer + 4 names; ret 14h

DATA = 0x30000000
PLAYER, ITEMS, RULE, OTHER = DATA, DATA + 0x1000, DATA + 0x2000, DATA + 0x3000
RETURN, WRITER, READER, STREAM = DATA + 0x4000, DATA + 0x4100, DATA + 0x4200, DATA + 0x5000
CORE = DATA + 0x10000                         # the statistics handler at +18FA8h
HANDLER = DATA + 0x30000                      # its shared statistics at +8
SHARED = HANDLER + 8
MANAGER = SHARED + 0x2760                     # the batteries' records at +20h, 100h each
LISTENER = SHARED + 0x31A0
OPTIONS = DATA + 0x40000                      # a place's team at +45ECh + place x 45FCh
SIZE = 0xA0000
RECORD, KITS, PLACE_SIZE = 0x1F8, 3, 0x45FC
BATTERIES = [DATA + 0x8000 + 0x400 * i for i in range(10)]
EVENT, STEP = 6, 40
NEW = (0, 0, ())                              # a life that begins: untouched for both teams, nothing held


def cell(place, kit, event=EVENT):
    """A count's address: the place's record, then (3 x event + kit) u16."""
    return SHARED + RECORD * place + 2 * (KITS * event + kit)


def record(index):
    return MANAGER + 0x20 + 0x100 * index


class Run(hide.Run):
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_map(DATA, SIZE, UC_PROT_ALL)
        self.notified, self.written = [], bytearray()
        self.emu.hook_add(UC_HOOK_CODE, self.notify, begin=NOTIFY, end=NOTIFY)
        for at in WRITERS:
            self.emu.hook_add(UC_HOOK_CODE, self.write, begin=at, end=at)

    def notify(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.notified.append((emu.reg_read(UC_X86_REG_EAX), self.read(sp + 4), self.read(sp + 8), self.read(sp + 12)))
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 16)

    def write(self, emu, address, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        assert self.read(sp + 4) == WRITER, hex(self.read(sp + 4))
        self.written.extend(emu.mem_read(emu.reg_read(UC_X86_REG_EAX), WRITERS[address]))
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 24)

    def setup(self, place=3, kit=1, team=1, batteries=10, statistics=True, rule=True, held=2, places=4):
        """A match of `places` places whose rule has `batteries`; the player
        at `place` with `kit`, of `team`, holds battery `held` (an index, None
        for nothing, 'other' for an item of no rule's). Team 0's container at
        the origin, team 1's 100 m along x. Places other than `place` are of
        the other team. Every life begins (start_match)."""
        self.emu.mem_write(DATA, bytes(SIZE))
        self.places = places
        self.emu.mem_write(PLAYER + 0x114, bytes([place, kit]))
        self.word(PLAYER + 0x120, CORE)
        self.word(PLAYER + 0x128, {None: 0, 'other': OTHER}.get(held, BATTERIES[held] if isinstance(held, int) else 0))
        self.word(CORE + 0x18FA8, HANDLER if statistics else 0)
        self.word(LISTENER, 0x12345)
        self.word(MANAGER + 0xA38, RULE if rule else 0)
        self.word(MANAGER + 0xA3C, OPTIONS)
        self.emu.mem_write(MANAGER + 0xA20, struct.pack('<3f', 0, 0, 0))
        self.emu.mem_write(MANAGER + 0xA2C, struct.pack('<3f', 100, 0, 0))
        self.word(RULE + 0x110, ITEMS)
        self.word(RULE + 0x114, ITEMS + 4 * batteries)
        for i in range(batteries):
            self.word(ITEMS + 4 * i, BATTERIES[i])
        self.emu.mem_write(OPTIONS + 0x577B5, bytes([places]))
        for p in range(20):
            self.team(p, team if p == place else 1 - team)
        self.site(START, START_AFTER, {UC_X86_REG_ESI: MANAGER})
        self.at(50)

    def team(self, place, team):
        self.word(OPTIONS + PLACE_SIZE * place + 0x45EC, team)

    def player(self, place, kit, team):
        """The ticked player is at `place` with `kit`, of `team`."""
        self.emu.mem_write(PLAYER + 0x114, bytes([place, kit]))
        self.team(place, team)

    def at(self, x, z=0.0):
        """The player stands at (x, 0, z), its transform otherwise the identity."""
        self.emu.mem_write(PLAYER + 0x40C, struct.pack('<16f', 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, x, 0, z, 1))

    def tick(self, x=None, z=0.0):
        """The tick's site to the push: what changed of DATA ({address: (before, after)} by u16)."""
        if x is not None:
            self.at(x, z)
        before = self.read(DATA, SIZE)
        sp = hide.STACK + 0x9000
        self.word(sp + 0x10, 0x5EC0DE)
        regs = {**hide.GAME, UC_X86_REG_EBX: PLAYER, UC_X86_REG_EDI: 123450}
        self.notified = []
        after, sp_after = self.run(TICK, (TICK_AFTER,), sp, regs)
        assert self.stopped == TICK_AFTER, hex(self.stopped or 0)
        assert sp_after == sp - 4 and self.read(sp - 4) == 123450, 'push edi replayed'
        assert after == {**regs, UC_X86_REG_ECX: 0x5EC0DE}, after
        return diff(before, self.read(DATA, SIZE))

    def site(self, start, end, regs):
        before = self.read(DATA, SIZE)
        sp = hide.STACK + 0x9000
        self.notified = []
        after, sp_after = self.run(start, (end,), sp, {**hide.GAME, **regs})
        assert self.stopped == end and sp_after == sp
        return after, diff(before, self.read(DATA, SIZE))

    def deliver(self, index, team):
        """On_event's put-in of battery `index` by a player of `team`."""
        after, changed = self.site(DELIVERED, DELIVERED_AFTER,
                                   {UC_X86_REG_EBX: record(index), UC_X86_REG_ESI: MANAGER, UC_X86_REG_EAX: team})
        assert after[UC_X86_REG_EAX] == team
        return changed

    def serialize(self):
        sp = hide.STACK + 0x9000 - 8
        self.emu.mem_write(sp, struct.pack('<II', RETURN, MANAGER))
        self.written = bytearray()
        _, sp_after = self.run(SERIALIZE, (RETURN,), sp, {**hide.GAME, UC_X86_REG_EDI: WRITER})
        assert self.stopped == RETURN and sp_after == sp + 8
        return bytes(self.written)

    def deserialize(self, stream):
        self.emu.mem_write(STREAM, stream + bytes(16))
        for offset, value in ((0, STREAM), (4, STREAM), (8, len(stream))):
            self.word(READER + offset, value)
        sp = hide.STACK + 0x9000 - 8
        self.emu.mem_write(sp, struct.pack('<II', RETURN, MANAGER))
        _, sp_after = self.run(DESERIALIZE, (RETURN,), sp, {**hide.GAME, UC_X86_REG_EDX: READER})
        assert self.stopped == RETURN and sp_after == sp + 8
        return self.read(READER + 4) - STREAM

    def lives(self):
        """Every battery's life as serialize writes it: (team 0's byte, team 1's, ((cell, steps), ...))."""
        stream, at, lives = self.serialize(), 0, []
        for _ in range(self.read(RULE + 0x114) - self.read(RULE + 0x110) >> 2):
            for _ in range(KITS):
                mask, = struct.unpack_from('<I', stream, at)
                at += 4 + 4 * bin(mask).count('1')
            at += 8 + 2
            zero, one, count = stream[at:at + 3]
            pairs = tuple((stream[at + 3 + 2 * i], stream[at + 4 + 2 * i]) for i in range(count))
            lives.append((zero, one, pairs))
            at += 3 + 2 * count
        assert at == len(stream), (at, len(stream))
        return lives

    def life(self, index=2):
        return self.lives()[index]


def diff(before, after):
    return {DATA + i: (struct.unpack_from("<H", before, i)[0], struct.unpack_from("<H", after, i)[0])
            for i in range(0, len(before), 2) if before[i:i + 2] != after[i:i + 2]}


def owner(index, team, before=0):
    """The put-in's own write: the record's owner team (a dword at +F8h)."""
    return {record(index) + 0xF8: (before, team)} if before != team else {}


def main():
    original = open(sys.argv[1], 'rb').read()
    patched, _ = base.build(original, 'section', open(base.default_bin('section'), 'rb').read(), None,
                            open(base.late_join_bin(), 'rb').read())
    r = Run(patched)

    # A team-1 carrier (place 3, kit 1: cell 10) at x: 0 m to team 0's
    # container is x, to team 1's 100 - x.
    r.setup()
    assert r.tick(40) == {} and r.notified == []
    assert r.life() == (5, 7, ())
    print('ok the first carry of a life keeps both bands (4, 6) and earns nothing')
    assert r.tick(75) == {} and r.notified == []
    assert r.life() == (5, 3, ((10, 4),))
    print('ok 60 m -> 25 m from its container: four steps held for the carrier, no score, no listener')
    assert r.tick(50) == {} and r.tick(76) == {} and r.life() == (5, 3, ((10, 4),))
    print('ok back and forth over the same ground: nothing more')
    assert r.tick(85) == {} and r.life() == (5, 2, ((10, 5),))
    print('ok new ground: one more step held')
    r.word(PLAYER + 0x128, 0)
    assert r.tick(99) == {} and r.life() == (5, 2, ((10, 5),))
    r.word(PLAYER + 0x128, BATTERIES[2])
    r.player(1, 0, 1)
    assert r.tick(86) == {} and r.life() == (5, 2, ((10, 5),))
    assert r.tick(95) == {} and r.life() == (5, 1, ((3, 1), (10, 5)))
    print('ok dropped and taken by a teammate (cell 3): nothing for the ground covered, a step held for new ground')
    assert r.notified == [] and all(r.read(cell(p, k), 2) == b'\0\0' for p in range(4) for k in range(3))
    print('ok no score anywhere during the carry')

    # The relay delivered into team 1's container: both carriers paid at once.
    changed = r.deliver(2, 1)
    assert changed == {**owner(2, 1), cell(3, 1): (0, 5 * STEP), cell(1, 0): (0, STEP)}, changed
    assert r.notified == [(LISTENER, 1, EVENT, STEP), (LISTENER, 3, EVENT, 5 * STEP)], r.notified
    assert r.life() == NEW
    print('ok its own team\'s delivery pays both carriers 2 points a step (event 6), the listener told; '
          'the life begins again')

    # An enemy carries it nearer its own container, then the team-1 side
    # delivers it: team 0's steps are dropped.
    r.setup()
    r.player(0, 2, 0)
    r.tick(60), r.tick(10)
    r.player(3, 1, 1)
    r.tick(10), r.tick(75)
    assert r.life() == (2, 3, ((2, 5), (10, 2)))
    changed = r.deliver(2, 1)
    assert changed == {**owner(2, 1), cell(3, 1): (0, 2 * STEP)}, changed
    assert r.notified == [(LISTENER, 3, EVENT, 2 * STEP)] and r.life() == NEW
    print('ok the other team\'s delivery: its carrier paid, team 0\'s five steps dropped unpaid')
    r.setup()
    r.tick(40), r.tick(75)
    changed = r.deliver(2, 0)
    assert changed == owner(2, 0) and r.notified == [] and r.life() == NEW
    print('ok delivered by the enemy: the carrier\'s steps dropped, nothing paid')
    r.setup()
    r.tick(40), r.tick(75)
    changed = r.deliver(4, 1)
    assert changed == owner(4, 1) and r.notified == [] and r.life() == (5, 3, ((10, 4),))
    print('ok another battery\'s delivery: this one\'s steps stay held')

    for label, kwargs in [('nothing held', dict(held=None)), ('another item', dict(held='other')),
                          ('a battery of no rule', dict(held=7, batteries=5)), ('no battery rule', dict(rule=False)),
                          ('no statistics handler', dict(statistics=False)), ('team 2', dict(team=2)),
                          ('place 20', dict(place=20)), ('place 255', dict(place=255)), ('kit 3', dict(kit=3))]:
        r.setup(**kwargs)
        assert r.tick(40) == {} and r.tick(90) == {} and r.notified == [], label
        r.word(MANAGER + 0xA38, RULE)
        assert all(life == NEW for life in r.lives()), label
    print('ok nothing held, another item, a battery of no rule, no rule, no handler, a team, place or kit '
          'that is none of the match\'s: nothing')

    r.setup()
    r.tick(40), r.tick(51)
    r.emu.mem_write(cell(3, 1), struct.pack('<H', 0xFFFA))
    changed = r.deliver(2, 1)
    assert changed == {**owner(2, 1), cell(3, 1): (0xFFFA, (0xFFFA + 2 * STEP) & 0xFFFF)}, changed
    print('ok the count wraps as the game\'s u16')

    # start_match: every life begins again.
    r.setup()
    r.tick(40), r.tick(75)
    after, changed = r.site(START, START_AFTER, {UC_X86_REG_ESI: MANAGER})
    assert changed == {} and after[UC_X86_REG_EAX] == RULE
    assert all(life == NEW for life in r.lives())
    print('ok start_match: every life begins again, nothing of the game\'s touched; the rule read as before')

    # The game's serialize and deserialize, two batteries, the hooks inside.
    r.setup(batteries=2, places=2, place=1, kit=0)
    for i, (distance, nearest, owner_team, finder) in enumerate([(2.0, (1.0, 3.0), 1, 0), (0.5, (0.5, 0.25), 3, 0xFF)]):
        r.emu.mem_write(record(i) + 4 * 3, struct.pack('<f', distance))   # carried_distances[1][0]
        r.emu.mem_write(record(i) + 0xF0, struct.pack('<ffI', *nearest, owner_team) + bytes([finder, 0xEE, 0, 0]))
    r.word(PLAYER + 0x128, BATTERIES[1])
    r.tick(60), r.tick(81)
    saved = {i: r.read(record(i), 0x100) for i in range(2)}
    stream = r.serialize()
    one = lambda distance, nearest, owner_team, finder, life: (
        struct.pack('<If', 2, distance) + struct.pack('<II', 0, 0) + struct.pack('<ff', *nearest)
        + bytes([owner_team, finder, *life]))
    expected = one(2.0, (1.0, 3.0), 1, 0, (0, 0, 0)) + one(0.5, (0.5, 0.25), 3, 0xFF, (7, 2, 1, 3, 3))
    assert stream == expected, (stream.hex(), expected.hex())
    print('ok serialize: each record\'s life follows its first_finder (progress, count, cell and steps)')
    for i in range(2):
        r.emu.mem_write(record(i), bytes(0x100))
    r.site(START, START_AFTER, {UC_X86_REG_ESI: MANAGER})
    assert r.deserialize(stream) == len(stream)
    for i in range(2):
        loaded = r.read(record(i), 0x100)
        assert loaded[:0xFD] == saved[i][:0xFD] and loaded[0xFE:] == saved[i][0xFE:], (i, loaded.hex())
    assert r.serialize() == stream
    print('ok deserialize: every record back, its life too, the reader at the stream\'s end')
    assert r.deserialize(stream[:-5]) == len(stream) - 5
    assert r.lives()[1] == NEW and r.read(record(1) + 0xFC, 1) == b'\xFF'
    print('ok a stream without the life: the record\'s own fields read, a new life')


if __name__ == '__main__':
    main()
