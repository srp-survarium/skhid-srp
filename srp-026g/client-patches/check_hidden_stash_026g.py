#!/usr/bin/env python3
"""Patch 109's Hidden Stash (docs/notes/artefact-skills.md "Hidden Stash") in Unicorn on the built exe.

    python check_hidden_stash_026g.py <original survarium.exe>

Requires Python `unicorn`. The game's own code runs on made-up objects (check_artefact_keeper_026g's
world, anomalies and stubs): artefact_container_core::can_use (0x31C6B0, with the hook at its
container test 0x31C6FB), use_execute from its transfer (0x31C810: patch 80's score, then the
stash, then transfer_artefact and the game's take_artefact, set_item, on_taken), a player's tick
(Keeper's gift through take_artefact too), and the death entry base_player::on_player_death
(0x2D77C0, the stash first, then patch 78's) up to its first own instruction. The run is the
inventory's own slots (+10Ch + 4 x 13..18), its count +16Ch, the kit's modifier 14 its float.

Checked: a kit with the bit (14), two drugs (13, 14) and two containers (15, 16): its own empty
containers fill first; both full, a take opens the stash (slot 17, the run 3 long) and the
artefact is in it with its passive (state 3, owned); every slot full, the game's own eviction (the
run stays 3); no bit, or a kit whose run ends at 18: the game's eviction, the run unchanged; with
no container at all (three drugs, modifier 14 at 0): can_use refuses without the bit and lets the
user in with it, and the take opens the run at the first empty quick slot (16); the death takes
the stash's artefact out (state 0, no inventory, the slot empty, the run the kit's own) and leaves
the own containers' artefacts; a death with the stash shut changes nothing; Keeper's gift with
every container full goes into the stash, and back to the pool at the death (dormant, its count:
never again). Says nothing of the HUD, the save or a real match.
"""
import struct
import sys

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_ESP

import patch_026g as base
from check_artefact_keeper_026g import Keeper, KEEPER, VTABLES, draw

CAN_USE = 0x31C6B0                    # thiscall (&user), `ret 4`, al
TRANSFER, TRANSFER_END = 0x31C810, 0x31C81C   # use_execute: the transfer and its call
DEATH, DEATH_END = 0x2D77C0, 0x2D77C5          # base_player::on_player_death's entry span
HIDDEN = 1 << 14
M14 = 14 * 4                          # the kit's modifier 14 (artefact_slots) float


class Stash(Keeper):
    def __init__(self, data):
        super().__init__(data)
        self.drug = self.block(0x300)          # a pocket's item: never read by these paths
        # Each kind's passive unlink (vfn +84h, thiscall, `ret`), as Keeper's check stubs the link.
        self.unlinked = []
        for vtable in VTABLES.values():
            at = self.read(vtable + 0x84)
            self.emu.hook_add(UC_HOOK_CODE, self.on_unlink, begin=at, end=at)

    def artefact(self, kind):
        at = super().artefact(kind)
        self.word(at + 0xD0, 10)               # the world registry's references (never released here)
        return at

    def on_unlink(self, emu, address, size, _):
        self.unlinked.append(self.ecx())
        esp = emu.reg_read(UC_X86_REG_ESP)
        emu.reg_write(UC_X86_REG_ESP, esp + 4)
        emu.reg_write(UC_X86_REG_EIP, self.read(esp))

    def user(self, place, ranks, drugs, containers, first=None, m14=None):
        """A player whose inventory has `drugs` pockets from slot 13, then `containers` from
        `first` (13 + drugs), modifier 14 the float `m14` (the containers)."""
        player, inventory, profile = self.block(0x800), self.block(0x200), self.block(0x2000)
        self.word(player + 0x10C, inventory)
        self.emu.mem_write(player + 0x114, bytes([place, 0]))
        self.word(player + 0x120, self.world)
        self.word(player + 0xD0, 10)
        self.emu.mem_write(player + 0x33C, b'\x01')
        self.word(self.places + place * 0x30, player)
        self.emu.mem_write(self.places + place * 0x30 + 0xC, b'\x00')
        self.word(self.world + 0x18FD8, self.read(self.world + 0x18FD8) | 1 << place)
        self.word(player + 0x6B8, profile)
        kit = profile + 0xF4
        self.word(kit + 0xA0, struct.unpack('<I', struct.pack('<f', float(ranks)))[0])
        self.word(kit + M14, struct.unpack('<I', struct.pack('<f', float(containers if m14 is None else m14)))[0])
        for n in range(drugs):
            self.word(inventory + 0x10C + 4 * (13 + n), self.drug)
        first = 13 + drugs if first is None else first
        if containers:
            self.word(inventory + 0x168, inventory + 0x10C + 4 * first)
        self.emu.mem_write(inventory + 0x16C, bytes([containers]))
        return player, inventory

    def slot(self, inventory, n):
        return self.read(inventory + 0x10C + 4 * n)

    def run(self, inventory):
        count = self.read(inventory + 0x16C) & 0xFF
        first = (self.read(inventory + 0x168) - inventory - 0x10C) // 4 if count else None
        return first, count

    def can_use(self, player, artefact):
        container = self.block(0x100)
        self.word(container + 0x4C, artefact)
        self.word(artefact + 0x280, 2)
        user = self.block(8)
        self.word(user, player)
        self.emu.reg_write(UC_X86_REG_EAX, 0)
        self.call(CAN_USE, (user,), ecx=container)
        return self.emu.reg_read(UC_X86_REG_EAX) & 0xFF

    def take(self, player, artefact):
        """use_execute's transfer of `artefact` (in its anomaly, state 2) to `player`."""
        container = self.block(0x100)
        self.word(container + 0x4C, artefact)
        self.word(artefact + 0x280, 2)
        self.emu.reg_write(UC_X86_REG_ESP, 0x21007000)
        self.emu.reg_write(UC_X86_REG_EBX, player)
        self.emu.reg_write(UC_X86_REG_EDI, container)
        self.run_to(TRANSFER, TRANSFER_END)

    def die(self, player):
        self.emu.reg_write(UC_X86_REG_ESP, 0x21007000)
        self.emu.reg_write(UC_X86_REG_ECX, player)
        self.run_to(DEATH, DEATH_END)

    def run_to(self, start, end):
        from unicorn import UcError
        try:
            self.emu.emu_start(start, end, count=2_000_000)
        except UcError as error:
            raise SystemExit(f'{error} at {self.emu.reg_read(UC_X86_REG_EIP):#x}') from error


def held(g, item, inventory):
    return g.state(item) == 3 and g.read(item + 0x250) == inventory


def dormant(g, item):
    return g.state(item) == 0 and g.read(item + 0x250) == 0


def check(data):
    g = Stash(data)
    mine, own = g.anomaly(['Spring', 'Onyx', 'Larkspur', 'Spring', 'Onyx', 'Larkspur', 'Spring', 'Onyx'], containers=2)
    g.register(mine)
    a = iter(own)

    # Two drugs, two containers, the bit: own containers first, then the stash at 17.
    p, inv = g.user(0, HIDDEN, drugs=2, containers=2)
    first, second, third, fourth = next(a), next(a), next(a), next(a)
    g.take(p, first)
    g.take(p, second)
    assert (g.slot(inv, 15), g.slot(inv, 16), g.run(inv)) == (first, second, (15, 2))
    g.take(p, third)
    assert g.run(inv) == (15, 3) and g.slot(inv, 17) == third and held(g, third, inv)
    assert held(g, first, inv) and held(g, second, inv)
    # Every slot full: the game's eviction (the first container's out, the newest last).
    g.take(p, fourth)
    assert g.run(inv) == (15, 3), g.run(inv)
    assert (g.slot(inv, 15), g.slot(inv, 16), g.slot(inv, 17)) == (second, third, fourth)
    assert dormant(g, first)
    print('Hidden Stash: own containers first; both full, the take opens the slot after them (17) with '
          'its passive; all full, the game\'s eviction: PASS')

    # The death: the stash's artefact out, dormant; the own containers' stay; the run the kit's own.
    g.die(p)
    assert g.run(inv) == (15, 2) and g.slot(inv, 17) == 0 and dormant(g, fourth), g.run(inv)
    assert (g.slot(inv, 15), g.slot(inv, 16)) == (second, third) and held(g, second, inv)
    # Again: the stash is shut, nothing changes.
    g.die(p)
    assert g.run(inv) == (15, 2) and (g.slot(inv, 15), g.slot(inv, 16)) == (second, third)
    assert any('hidden stash: place 0 loses its artefact' in line for line in g.lines), g.lines
    print('Hidden Stash: the death takes the stash\'s artefact out (state 0, no inventory) and closes '
          'it, the containers\' artefacts kept; a death with it shut changes nothing: PASS')

    # No bit, or a kit whose run ends at 18 (four drugs, two containers): the game's eviction.
    for ranks, drugs in ((0, 2), (HIDDEN, 4)):
        g2 = Stash(data)
        m2, o2 = g2.anomaly(['Spring'] * 3, containers=2)
        g2.register(m2)
        q, qi = g2.user(1, ranks, drugs=drugs, containers=2)
        for item in o2:
            g2.take(q, item)
        start = 13 + drugs
        assert g2.run(qi) == (start, 2) and (g2.slot(qi, start), g2.slot(qi, start + 1)) == (o2[1], o2[2])
        assert start + 2 > 18 or g2.slot(qi, start + 2) == 0
        assert dormant(g2, o2[0])
    print('Hidden Stash: no bit, or no quick slot past the containers: the game\'s eviction, the run '
          'unchanged: PASS')

    # No container: can_use refuses without the bit, lets the user in with it; the take opens at 16.
    g3 = Stash(data)
    m3, o3 = g3.anomaly(['Onyx', 'Spring'], containers=2)
    g3.register(m3)
    r, ri = g3.user(2, 0, drugs=3, containers=0)
    s, si = g3.user(3, HIDDEN, drugs=3, containers=0)
    assert g3.can_use(r, o3[0]) == 0 and g3.can_use(s, o3[0]) == 1
    g3.take(s, o3[0])
    assert g3.run(si) == (16, 1) and g3.slot(si, 16) == o3[0] and held(g3, o3[0], si)
    # Open and full: can_use as the game (a container), the take its eviction.
    assert g3.can_use(s, o3[1]) == 1
    g3.die(s)
    assert g3.run(si)[1] == 0 and g3.slot(si, 16) == 0 and dormant(g3, o3[0])
    print('Hidden Stash: with no container, can_use only with the bit; the take opens it at the first '
          'empty quick slot; the death empties it: PASS')

    # Keeper: every container full, the gift goes into the stash; at the death back to the pool.
    g4 = Stash(data)
    m4, o4 = g4.anomaly(['Spring', 'Spring'], containers=2)
    hidden, pool = g4.anomaly(['Onyx'] * 4, containers=0)
    g4.register(m4)
    g4.register(hidden)
    k, ki = g4.user(0, HIDDEN + KEEPER + draw(2), drugs=2, containers=2)
    g4.take(k, o4[0])
    g4.take(k, o4[1])
    g4.started()
    g4.tick(k)
    assert g4.run(ki) == (15, 3) and g4.slot(ki, 17) == pool[0] and held(g4, pool[0], ki)
    g4.die(k)
    assert dormant(g4, pool[0]) and struct.unpack('<H', g4.emu.mem_read(pool[0] + 0x260, 2))[0] == 1
    g4.statistics(0, 2, 0)
    g4.tick(k)
    assert g4.run(ki) == (15, 2) and g4.slot(ki, 17) == 0 and dormant(g4, pool[0])
    print('Hidden Stash: Keeper\'s gift with every container full goes into the stash; at the death '
          'back to the pool, dormant with its count, never given again: PASS')


def main():
    original = open(sys.argv[1], 'rb').read()
    patched, _ = base.build(original, 'section', open(base.default_bin('section'), 'rb').read(), None,
                            open(base.late_join_bin(), 'rb').read())
    check(patched)


if __name__ == '__main__':
    main()
