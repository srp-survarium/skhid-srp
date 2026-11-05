#!/usr/bin/env python3
"""Patch 109's Keeper of the Artifacts (docs/notes/artefact-skills.md) in Unicorn on the built exe.

    python check_artefact_keeper_026g.py <original survarium.exe>

Requires Python `unicorn`. The game's own code runs on made-up objects:
generic_anomaly_core::register_artefacts (0x31B8E0, with patch 109's hook at its entry) registers
a map anomaly's pool and the hidden Keeper anomaly's (no container) into a core world's artefact
vector; the respawn rule's spawn from 0x316EAB (the hook, then its five pops and `ret 0Ch`) gives
made-up players' kits their gift through the game's own take_artefact (0x2FC850), set_item and
on_taken. Stubs: each kind's client register (vfn +70h: it writes the world and links a client
list) and passive link (vfn +7Ch).

Checked: the pool is the container-less anomaly's ids alone (the map's own dormant artefacts,
lower ids, are never taken); a Keeper's first spawn takes the lowest dormant of the draw's kind
into its first container (state 3, the spawn's count, cooldown 0, its owner and slot), the next
Keeper the next one, none when the kind is spent; nothing without the Keeper bit, after a death or
a suicide counted (any kit), with the first container held or no container, or a draw of no kind;
a draw without the skill is accepted and gives nothing; a new load (the vector empty again)
forgets the last map's pool. Says nothing of the cook, the save or a real match.
"""
import struct
import sys

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_EIP

import patch_026g as base
from check_artefacts_026g import Game, STOP

REGISTER = 0x31B8E0           # (anomaly, world), `ret 8`
SPAWN_TAIL = 0x316EAB         # the hook; then pop edi, esi, ebp, ebx, ecx; ret 0Ch
VTABLES = {'Larkspur': 0xA05FE4, 'Onyx': 0xA06084, 'Spring': 0xA05F44}
CLIENT_REGISTER = {0x25F0B0, 0x25F5F0, 0x25EB60}   # vfn +70h of each
PASSIVE_LINK = {0x3121A0, 0x312810, 0x3118C0}       # vfn +7Ch of each
RANKS = 0xA0                  # kit modifier 40, artefact_power
CREATE_FILE, WRITE_FILE, CLOSE_HANDLE, THREAD = 0x870048, 0x870108, 0x870040, 0x870118  # the log's imports
IMPORTS = STOP + 0x800        # their stubs
KEEPER = 8                    # its bit 3


def draw(n):
    return n << 9


class Keeper(Game):
    def __init__(self, data):
        super().__init__(data)
        self.emu.hook_add(UC_HOOK_CODE, self.on_stub, begin=0x25E000, end=0x313000)
        self.world = self.block(0x19100)
        self.storage = self.world + 0x1894C
        self.word(self.world + 0x18940, self.storage)
        self.word(self.world + 0x18944, self.storage)
        self.word(self.world + 0x18948, self.storage + 0x200)
        self.lines = []
        for n, slot in enumerate((CREATE_FILE, WRITE_FILE, CLOSE_HANDLE)):
            self.word(slot, IMPORTS + 0x10 * n)
            self.emu.hook_add(UC_HOOK_CODE, self.on_import, begin=IMPORTS + 0x10 * n, end=IMPORTS + 0x10 * n)
        self.emu.mem_write(IMPORTS + 0x30, bytes.fromhex('b801000000c3'))   # GetCurrentThreadId: 1
        self.word(THREAD, IMPORTS + 0x30)

    def on_import(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 * i) for i in range(1, 6)]
        if address == IMPORTS:                  # CreateFileA: 7 arguments, a handle
            pops, result = 28, 0x1234
        elif address == IMPORTS + 0x10:         # WriteFile(file, bytes, size, &written, 0)
            self.lines.append(bytes(emu.mem_read(args[1], args[2])).decode().strip())
            self.word(args[3], args[2])
            pops, result = 20, 1
        else:                                   # CloseHandle
            pops, result = 4, 1
        from unicorn.x86_const import UC_X86_REG_EAX
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def on_stub(self, emu, address, size, _):
        if address in CLIENT_REGISTER:          # thiscall (world), `ret 4`
            esp = emu.reg_read(UC_X86_REG_ESP)
            self.word(self.ecx() + 0x248, self.read(esp + 4))
            emu.reg_write(UC_X86_REG_ESP, esp + 8)
            emu.reg_write(UC_X86_REG_EIP, self.read(esp))
        elif address in PASSIVE_LINK:           # thiscall (), `ret`
            esp = emu.reg_read(UC_X86_REG_ESP)
            emu.reg_write(UC_X86_REG_ESP, esp + 4)
            emu.reg_write(UC_X86_REG_EIP, self.read(esp))

    def ecx(self):
        from unicorn.x86_const import UC_X86_REG_ECX
        return self.emu.reg_read(UC_X86_REG_ECX)

    def artefact(self, kind):
        at = self.block(0x400)
        self.word(at, VTABLES[kind])
        self.emu.mem_write(at + 0x27C, struct.pack('<H', 0xFFFF))
        self.emu.mem_write(at + 0x27E, b'\xff')
        self.word(at + 0x254, 23)
        self.word(at + 0x28C, 1234)             # a cooldown left over: the gift clears it
        return at

    def anomaly(self, kinds, containers):
        at = self.block(0x200)
        pool = [self.artefact(kind) for kind in kinds]
        array = self.block(4 * len(pool))
        for n, item in enumerate(pool):
            self.word(array + 4 * n, item)
        self.word(at + 0x1B0, array)
        self.word(at + 0x1B4, len(pool))
        listed = self.block(8)
        self.word(at + 0x140, listed)
        self.word(at + 0x144, listed + 4 * containers)
        return at, pool

    def register(self, anomaly):
        self.call(REGISTER, (anomaly, self.world))

    def ids(self):
        begin, end = self.read(self.world + 0x18940), self.read(self.world + 0x18944)
        return [self.read(at) for at in range(begin, end, 4)]

    def player(self, place, ranks, containers=2, kit=0):
        player, inventory, profile, slots = self.block(0x800), self.block(0x200), self.block(0x2000), self.block(16)
        self.word(player + 0x10C, inventory)
        self.emu.mem_write(player + 0x114, bytes([place, kit]))
        self.word(player + 0x120, self.world)
        self.emu.mem_write(player + 0x33C, b'\x01')
        self.word(player + 0x6B8, profile)
        self.word(profile + 0xF4 + kit * 0x704 + RANKS, struct.unpack('<I', struct.pack('<f', float(ranks)))[0])
        self.word(inventory + 0x168, slots)
        self.emu.mem_write(inventory + 0x16C, bytes([containers]))
        return player, inventory, slots

    def spawn(self, player):
        esp = 0x21007000
        for n, value in enumerate([0, player, 0, 0, 0, STOP, 0, 0, 0]):
            self.word(esp + 4 * n, value)
        self.emu.reg_write(UC_X86_REG_ESP, esp)
        self.emu.reg_write(UC_X86_REG_ESI, player)
        self.emu.emu_start(SPAWN_TAIL, STOP, count=2_000_000)

    def statistics(self, place, event, kit):
        handler = self.block(0x40 + 20 * 0x1F8)
        self.word(self.world + 0x18FA8, handler)
        shared = handler + 8
        self.emu.mem_write(shared + place * 0x1F8 + 2 * (3 * event + kit), struct.pack('<H', 1))

    def state(self, item):
        return self.read(item + 0x280)


def check(data):
    g = Keeper(data)
    # The map's own anomaly first (lower ids), then the hidden one.
    mine, own = g.anomaly(['Spring', 'Onyx', 'Larkspur'], containers=2)
    hidden, pool = g.anomaly(['Larkspur', 'Larkspur', 'Onyx', 'Onyx', 'Spring', 'Spring'], containers=0)
    g.register(mine)
    g.register(hidden)
    assert g.ids() == own + pool, 'both pools registered in order'
    assert [g.read(a + 0x27E) & 0xFF for a in own + pool] == list(range(9))
    assert all(g.read(a + 0x248) == g.world for a in own + pool)

    def gift(player, slots):
        g.spawn(player)
        return g.read(slots)

    # Draw 1 (Larkspur): the hidden pool's first, never the map's own (id 2).
    p0, inv0, slots0 = g.player(0, KEEPER + draw(1))
    assert gift(p0, slots0) == pool[0]
    assert g.state(pool[0]) == 3 and g.read(pool[0] + 0x250) == inv0
    assert struct.unpack('<H', g.emu.mem_read(pool[0] + 0x260, 2))[0] == 1
    assert g.read(pool[0] + 0x28C) == 0
    assert g.state(own[2]) == 0
    # The next Keeper the next one, then the kind is spent.
    p1, _, slots1 = g.player(1, KEEPER + draw(1) + 5)
    assert gift(p1, slots1) == pool[1]
    p2, _, slots2 = g.player(2, KEEPER + draw(1))
    assert gift(p2, slots2) == 0
    # Onyx, Spring.
    p3, _, slots3 = g.player(3, KEEPER + draw(2))
    assert gift(p3, slots3) == pool[2]
    p4, _, slots4 = g.player(4, KEEPER + draw(3), kit=2)
    assert gift(p4, slots4) == pool[4]
    # Nothing: no skill (a draw alone), a draw of no kind, a held first container, no container.
    p5, _, slots5 = g.player(5, draw(3))
    assert gift(p5, slots5) == 0
    p6, _, slots6 = g.player(6, KEEPER + draw(5))
    assert gift(p6, slots6) == 0
    p7, _, slots7 = g.player(7, KEEPER + draw(3))
    g.word(slots7, own[0])
    g.spawn(p7)
    assert g.read(slots7) == own[0] and g.state(pool[5]) == 0
    p8, _, slots8 = g.player(8, KEEPER + draw(3), containers=0)
    g.spawn(p8)
    assert g.state(pool[5]) == 0
    # A death or a suicide of the place, in another kit than the spawn's: a later life.
    for place, event, kit in [(9, 2, 1), (10, 3, 0)]:
        g.statistics(place, event, kit)
        p, _, slots = g.player(place, KEEPER + draw(3))
        assert gift(p, slots) == 0, (place, event)
    # The statistics without a death for the place: still its first life.
    p11, _, slots11 = g.player(11, KEEPER + draw(3))
    assert gift(p11, slots11) == pool[5]
    for want in ['keeper: place 0 takes Larkspur id 3', 'keeper: place 1 takes Larkspur id 4',
                 'keeper: place 2: no Larkspur free in the pool', 'keeper: place 6: no draw of a kind in its kit',
                 'keeper: place 11 takes Spring id 8']:
        assert 'srp: ' + want in g.lines, (want, g.lines)
    assert not any('place 5' in line or 'place 9' in line or 'place 10' in line for line in g.lines), g.lines
    print('Keeper: the hidden pool alone, the lowest dormant of the draw\'s kind into the first container '
          '(state 3, count 1, cooldown 0), none spent, no skill, a later life, a held or no container: PASS')

    # A new load: the vector rebuilt from empty, a map without the hidden anomaly.
    g.word(g.world + 0x18944, g.storage)
    g.word(g.world + 0x18FA8, 0)
    again, own2 = g.anomaly(['Spring'], containers=1)
    g.register(again)
    p, _, slots = g.player(12, KEEPER + draw(3))
    assert gift(p, slots) == 0 and g.state(own2[0]) == 0
    print('Keeper: a new load forgets the last map\'s pool; a map without one gives none: PASS')


CONTAINERS_READ, CONTAINERS_READ_END = 0x2FE5DE, 0x2FE77B   # inventory::deserialize's containers


def late_join(data):
    """A late joiner (or a resynced game) loads the map as the others did: the same pools in the
    same order give the same ids; inventory::deserialize's own container read then names, by the
    id a world carries, the joiner's own copy of the gift (its state, owner and cooldown come with
    the hidden anomaly's save, native as any anomaly's: check_artefact_carry_026g.py runs
    artefact_base's save and load)."""
    worlds = []
    for _ in range(2):
        g = Keeper(data)
        mine, own = g.anomaly(['Spring', 'Onyx', 'Larkspur'], containers=2)
        hidden, pool = g.anomaly(['Larkspur', 'Larkspur', 'Onyx', 'Onyx', 'Spring', 'Spring'], containers=0)
        g.register(mine)
        g.register(hidden)
        worlds.append((g, pool))
    (played, pool), (joiner, copies) = worlds
    p, inventory, slots = played.player(0, KEEPER + draw(2))
    played.spawn(p)
    given = played.read(slots)
    assert given == pool[2]
    gift_id = played.read(given + 0x27E) & 0xFF
    # The id the world carries (inventory::serialize writes +27Eh), read by the joiner's own code.
    inventory = joiner.block(0x400)
    joiner.word(inventory + 0x108, joiner.world)
    stream = joiner.block(16)
    joiner.emu.mem_write(stream, bytes([2, 0x40, gift_id, 0xFF]))
    reader = joiner.block(12)
    joiner.word(reader + 4, stream)
    from unicorn.x86_const import UC_X86_REG_EDI
    frame = 0x21006000
    joiner.emu.reg_write(UC_X86_REG_ESP, frame)
    joiner.emu.reg_write(UC_X86_REG_ESI, reader)
    joiner.emu.reg_write(UC_X86_REG_EDI, inventory)
    joiner.word(frame + 0x24, 0)
    joiner.emu.emu_start(CONTAINERS_READ, CONTAINERS_READ_END, count=100_000)
    containers = joiner.read(inventory + 0x168)
    assert containers == inventory + 0x10C + 0x40 * 4 and joiner.read(inventory + 0x16C) & 0xFF == 2
    assert joiner.read(containers) == copies[2] and joiner.read(containers + 4) == 0
    print('Keeper: a late joiner registers the same pool at the same ids and its inventory load names its '
          'own copy of the gift: PASS')


def main():
    original = open(sys.argv[1], 'rb').read()
    patched, _ = base.build(original, 'section', open(base.default_bin('section'), 'rb').read(), None,
                            open(base.late_join_bin(), 'rb').read())
    check(patched)
    late_join(patched)


if __name__ == '__main__':
    main()
