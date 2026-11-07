#!/usr/bin/env python3
"""Patch 109's Keeper of the Artifacts (docs/notes/artefact-skills.md) in Unicorn on the built exe.

    python check_artefact_keeper_026g.py <original survarium.exe>

Requires Python `unicorn`. The game's own code runs on made-up objects:
generic_anomaly_core::register_artefacts (0x31B8E0, with patch 109's hook at its entry) registers
a map anomaly's pool and the hidden Keeper anomaly's (no container) into a core world's artefact
vector; each player's tick from 0x2DAAD3 (base_player::tick before tick_animations: the hooks of
the site in their order, patch 109's Keeper last) gives a due Keeper its place's artefact through
the game's own take_artefact (0x2FC850), set_item and on_taken. Stubs: each kind's client register
(vfn +70h: it writes the world and links a client list), passive link (vfn +7Ch), and the resource
query the glow asks at its first tick (answered nothing).

Checked: the pool is the container-less anomaly's ids alone (the map's own dormant artefacts,
lower ids, are never taken); nothing before the start (waiting, the countdown); from the start a
Keeper there takes its place's artefact of the draw's kind (place p the p-th by id; state 3,
count 1, cooldown 0) into its first empty container (the second when the first is held); with
every container held it waits, and takes it when one frees in the same life, or at a respawn;
dead, it waits; never a second (in play, or back in the pool with its count); a late joiner
(bit 13) nothing before its first death, then the same; nothing without the Keeper bit, a draw of
no kind, or a place past the pool; a new load (the vector empty again) forgets the last map's
pool. Says nothing of the cook, the save or a real match.
"""
import struct
import sys

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_EIP

import patch_026g as base
from check_artefacts_026g import Game, STOP

REGISTER = 0x31B8E0           # (anomaly, world), `ret 8`
VTABLES = {'Larkspur': 0xA05FE4, 'Onyx': 0xA06084, 'Spring': 0xA05F44}
CLIENT_REGISTER = {0x25F0B0, 0x25F5F0, 0x25EB60}   # vfn +70h of each
PASSIVE_LINK = {0x3121A0, 0x312810, 0x3118C0}       # vfn +7Ch of each
RANKS = 0xA0                  # kit modifier 40, artefact_power
CREATE_FILE, WRITE_FILE, CLOSE_HANDLE, THREAD = 0x870048, 0x870108, 0x870040, 0x870118  # the log's imports
IMPORTS = STOP + 0x800        # their stubs
KEEPER = 8                    # its bit 3
LATE = 1 << 13                # a late joiner's kits (srp's mark)
TICK, TICK_END = 0x2DAAD3, 0x2DAADB   # base_player::tick: the carrier-tick hooks, up to tick_animations
QUERY = 0xE5B60               # query_resources (cdecl), which the glow calls at its first tick


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
        self.places = self.block(20 * 0x30)                     # the roster: core_active_player's table
        self.word(self.world + 0x18458, self.places)
        self.word(self.world + 0x1845C, self.places + 20 * 0x30)
        self.handler = None
        self.lines = []
        for n, slot in enumerate((CREATE_FILE, WRITE_FILE, CLOSE_HANDLE)):
            self.word(slot, IMPORTS + 0x10 * n)
            self.emu.hook_add(UC_HOOK_CODE, self.on_import, begin=IMPORTS + 0x10 * n, end=IMPORTS + 0x10 * n)
        self.emu.mem_write(IMPORTS + 0x30, bytes.fromhex('b801000000c3'))   # GetCurrentThreadId: 1
        self.word(THREAD, IMPORTS + 0x30)
        self.emu.hook_add(UC_HOOK_CODE, self.no_query, begin=QUERY, end=QUERY)

    def no_query(self, emu, address, size, _):
        esp = emu.reg_read(UC_X86_REG_ESP)
        emu.reg_write(UC_X86_REG_EIP, self.read(esp))
        emu.reg_write(UC_X86_REG_ESP, esp + 4)

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
        self.word(player + 0xD0, 10)                            # references (the roster lookup adds one)
        self.emu.mem_write(player + 0x33C, b'\x01')
        self.word(self.places + place * 0x30 + kit * 4, player)
        self.emu.mem_write(self.places + place * 0x30 + 0xC, bytes([kit]))
        self.word(self.world + 0x18FD8, self.read(self.world + 0x18FD8) | 1 << place)
        self.word(player + 0x6B8, profile)
        self.word(profile + 0xF4 + kit * 0x704 + RANKS, struct.unpack('<I', struct.pack('<f', float(ranks)))[0])
        self.word(inventory + 0x168, slots)
        self.emu.mem_write(inventory + 0x16C, bytes([containers]))
        return player, inventory, slots

    def statistics(self, place, event, kit, count=1):
        if self.handler is None:
            self.handler = self.block(0x40 + 20 * 0x1F8)
            self.word(self.world + 0x18FA8, self.handler)
        shared = self.handler + 8
        self.emu.mem_write(shared + place * 0x1F8 + 2 * (3 * event + kit), struct.pack('<H', count))

    def tick(self, player):
        """The player's tick at the carrier-tick site, up to tick_animations."""
        from unicorn.x86_const import UC_X86_REG_EBX, UC_X86_REG_EDI
        self.emu.reg_write(UC_X86_REG_ESP, 0x21007000)
        self.emu.reg_write(UC_X86_REG_EBX, player)
        self.emu.reg_write(UC_X86_REG_EDI, 0)
        self.emu.emu_start(TICK, TICK_END, count=2_000_000)

    def started(self, on=True):
        self.word(self.world + 0x18FE4, 3 if on else 2)

    def state(self, item):
        return self.read(item + 0x280)


def check(data):
    g = Keeper(data)
    # The map's own anomaly first (lower ids), then the hidden one: four a kind, a place each.
    mine, own = g.anomaly(['Spring', 'Onyx', 'Larkspur'], containers=2)
    hidden, pool = g.anomaly(['Larkspur'] * 4 + ['Onyx'] * 4 + ['Spring'] * 4, containers=0)
    larkspur, onyx, spring = pool[0:4], pool[4:8], pool[8:12]
    g.register(mine)
    g.register(hidden)
    assert g.ids() == own + pool, 'both pools registered in order'
    assert [g.read(a + 0x27E) & 0xFF for a in own + pool] == list(range(15))
    assert all(g.read(a + 0x248) == g.world for a in own + pool)

    def given(player, slots, at=0):
        g.tick(player)
        return g.read(slots + 4 * at)

    g.statistics(0, 2, 1, 0)
    p0, inv0, slots0 = g.player(0, KEEPER + draw(1))
    # Before the start (the countdown): nothing.
    g.started(False)
    assert given(p0, slots0) == 0 and g.state(larkspur[0]) == 0
    # From the start: its place's artefact (place 0, the first Larkspur) into its first empty container.
    g.started()
    assert given(p0, slots0) == larkspur[0]
    assert g.state(larkspur[0]) == 3 and g.read(larkspur[0] + 0x250) == inv0
    assert struct.unpack('<H', g.emu.mem_read(larkspur[0] + 0x260, 2))[0] == 1
    assert g.read(larkspur[0] + 0x28C) == 0 and g.state(own[2]) == 0
    # Never a second: its next ticks, then the gift back in the pool after a death (dormant, its count).
    assert given(p0, slots0) == larkspur[0]
    g.word(slots0, 0)
    g.word(larkspur[0] + 0x280, 0)
    g.word(larkspur[0] + 0x250, 0)
    g.statistics(0, 2, 0)
    assert given(p0, slots0) == 0 and g.state(larkspur[0]) == 0
    # The first container held, the second empty: into the second.
    p1, _, slots1 = g.player(1, KEEPER + draw(1))
    g.word(slots1, own[0])
    assert given(p1, slots1, 1) == larkspur[1] and g.read(slots1) == own[0]
    # Every container held: it waits; a container freed in the same life: then.
    p2, _, slots2 = g.player(2, KEEPER + draw(2))
    g.word(slots2, own[0])
    g.word(slots2 + 4, own[1])
    assert given(p2, slots2) == own[0] and g.state(onyx[2]) == 0
    g.word(slots2 + 4, 0)
    assert given(p2, slots2, 1) == onyx[2]
    # Every container held, then dead: it waits; the respawn with an empty kit container: then.
    p3, _, slots3 = g.player(3, KEEPER + draw(2))
    g.word(slots3, own[0])
    g.word(slots3 + 4, own[1])
    assert given(p3, slots3) == own[0]
    g.emu.mem_write(p3 + 0x33C, b'\x00')
    g.word(slots3, 0)
    g.word(slots3 + 4, 0)
    g.statistics(3, 2, 0)
    assert given(p3, slots3) == 0 and g.state(onyx[3]) == 0
    g.emu.mem_write(p3 + 0x33C, b'\x01')
    assert given(p3, slots3) == onyx[3]
    # A late joiner: nothing before its first death, its place's artefact after it.
    p4, _, slots4 = g.player(2, KEEPER + draw(3) + LATE, kit=1)
    assert given(p4, slots4) == 0
    g.statistics(2, 3, 1)
    assert given(p4, slots4) == spring[2]
    # Nothing: no skill (a draw alone), a draw of no kind, a place past the pool.
    p5, _, slots5 = g.player(5, draw(3))
    p6, _, slots6 = g.player(6, KEEPER + draw(5))
    p7, _, slots7 = g.player(7, KEEPER + draw(3))
    assert (given(p5, slots5), given(p6, slots6), given(p7, slots7)) == (0, 0, 0)
    for want in ['keeper: place 0 takes Larkspur id 3', 'keeper: place 1 takes Larkspur id 4',
                 'keeper: place 2 takes Onyx id 9', 'keeper: place 3 takes Onyx id 10',
                 'keeper: place 2 takes Spring id 13']:
        assert 'srp: ' + want in g.lines, (want, g.lines)
    assert sum('takes' in line for line in g.lines) == 5, g.lines
    print('Keeper: nothing in the countdown; from the start its place\'s artefact of the draw\'s kind (state 3, '
          'count 1, cooldown 0) into the first empty container (the second behind a held first); all held: '
          'when one frees, or at the respawn; dead: waits; never a second (in play or back with its count); '
          'a late joiner after its first death; no skill, no kind, past the pool: nothing: PASS')

    # A new load: the vector rebuilt from empty, a map without the hidden anomaly.
    g.word(g.world + 0x18944, g.storage)
    again, own2 = g.anomaly(['Spring'], containers=1)
    g.register(again)
    p, _, slots = g.player(8, KEEPER + draw(3))
    assert given(p, slots) == 0 and g.state(own2[0]) == 0
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
    played.started()
    played.tick(p)
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
