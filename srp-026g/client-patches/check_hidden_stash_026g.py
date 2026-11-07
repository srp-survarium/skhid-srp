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
every container full goes into the stash, and back to the pool at the death (dormant, still marked:
never again). With patch 78's carry (check_artefact_carry_026g's fixture, a donor whose only
container is its open stash): a hand-over closes it at once and the HUD's loop (0x23B792..0x23B7B9,
its two position functions stubbed) shows the position it left empty once, its cache zeroed; held
in the hand at death, the artefact goes back dormant (state 0), not dropped. Says nothing of the
save or a real match.
"""
import struct
import sys

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_ESP

import patch_026g as base
from check_artefact_keeper_026g import Keeper, KEEPER, draw

CAN_USE = 0x31C6B0                    # thiscall (&user), `ret 4`, al
TRANSFER, TRANSFER_END = 0x31C810, 0x31C81C   # use_execute: the transfer and its call
DEATH, DEATH_END = 0x2D77C0, 0x2D77C5          # base_player::on_player_death's entry span
HIDDEN = 1 << 14
M14 = 14 * 4                          # the kit's modifier 14 (artefact_slots) float


class Stash(Keeper):
    def __init__(self, data):
        super().__init__(data)
        self.drug = self.block(0x300)          # a pocket's item: never read by these paths
        # Keeper's fixture stubs each kind's passive link and unlink (vfn +7Ch, +84h) and gives
        # each artefact its references and config.

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
            raise SystemExit(f'{error} at {self.emu.reg_read(UC_X86_REG_EIP):#x} (run from {start:#x})') from error


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
    assert dormant(g4, pool[0]) and g4.read(pool[0] + 0x2A0) != 0xFFFFFFFF, 'the gift lost its mark'
    g4.statistics(0, 2, 0)
    g4.tick(k)
    assert g4.run(ki) == (15, 2) and g4.slot(ki, 17) == 0 and dormant(g4, pool[0])
    print('Hidden Stash: Keeper\'s gift with every container full goes into the stash; at the death '
          'back to the pool, dormant and still marked given (its activation time), never given again: PASS')


HUD_LOOP, HUD_END = 0x23B792, 0x23B7B9          # update_local_player_state's quick loop, past the hook
SHOW_ITEM, SHOW_EMPTY = 0x23A200, 0x23A380       # its position fill (stubbed), update_quick_slot (stubbed)


def carried(data):
    """Patch 78's carry over a donor whose only container is its open stash (slot 15: two pockets'
    items before it, no container of its own): a hand-over closes it at once and the HUD's loop
    clears the position it left; held in the hand at death, it goes back dormant, never dropped."""
    from check_artefact_carry_026g import Run, COMMAND, USE

    def donor_with_stash(g, donor):
        # The artefact's own cast_artefact (vfn +58h, 0x246380: this), which the fixture replaced:
        # the HUD's fixed-slot loop skips an artefact by it.
        g.word(g.read(g.item_ptr) + 0x58, 0x246380)
        inventory = g.block(0x190)
        drug = g.block(0x340)                  # kind 0 (+24Ch): no quick item of the fixed slots
        g.word(inventory + 0x10C + 4 * 13, drug)
        g.word(inventory + 0x10C + 4 * 14, drug)
        g.word(inventory + 0x168, inventory + 0x10C + 4 * 15)
        g.emu.mem_write(inventory + 0x16C, b'\x01')
        reference = g.block(4)
        g.word(reference, g.item_ptr)
        g.call(0x2FC620, (reference,), esi=inventory, edi=15)
        g.word(donor + 0x10C, inventory)
        kit = g.read(donor + 0x6B8) + 0xF4
        g.word(kit + 0xA0, struct.unpack('<I', struct.pack('<f', float(HIDDEN)))[0])
        g.word(kit + M14, 0)
        return inventory

    # The hand-over, and the HUD's positions before and after it.
    g = Run(data)
    donor, recipient = g.players
    inventory = donor_with_stash(g, donor)
    ui = g.block(0x2000)
    shown, emptied = [], []

    def stub(emu, address, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        (shown if address == SHOW_ITEM else emptied).append(
            g.read(sp + 4) if address == SHOW_EMPTY else emu.reg_read(UC_X86_REG_EDI))
        emu.reg_write(UC_X86_REG_EIP, g.read(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 8)
    for at in (SHOW_ITEM, SHOW_EMPTY):
        g.emu.hook_add(UC_HOOK_CODE, stub, begin=at, end=at)

    def hud():
        shown.clear(), emptied.clear()
        g.site(HUD_LOOP, [HUD_END], ebx=inventory, esi=ui)
        return list(shown), list(emptied)

    first = hud()
    assert first == ([0], []), f'the stash is the HUD\'s first position: {first}'
    g.emu.mem_write(ui + 0x1B5C, bytes(range(1, 9)))       # its cached description, as 0x23A200 leaves one
    g.input(donor, 0, COMMAND)
    g.tick(donor, 800)
    g.input(recipient, 930, 0)
    g.word(recipient + 0x128, 0)
    assert g.input(recipient, 940, USE) & USE == 0, 'the hand-over did not take Use'
    assert g.contents(g.inventories[1], 1) == [g.item_ptr]
    assert g.read(inventory + 0x10C + 4 * 15) == 0 and g.read(inventory + 0x16C) & 0xFF == 0
    assert hud() == ([], [0]), 'the position the stash left was not cleared'
    assert bytes(g.emu.mem_read(ui + 0x1B5C, 8)) == bytes(8), 'its cached description was kept'
    assert hud() == ([], []), 'a cleared position cleared again'
    print('Hidden Stash: a hand-over from the stash closes it at once; the HUD\'s position it left is '
          'shown empty once (update_quick_slot) with its cache zeroed: PASS')

    # Held in the hand at death: back dormant (state 0), not dropped (6), the run closed.
    g = Run(data)
    donor, _ = g.players
    inventory = donor_with_stash(g, donor)
    g.input(donor, 0, COMMAND)
    g.tick(donor, 800)
    g.renderer()
    g.draw()
    g.floor = 0.
    g.site(0x1F28E0, [0x1F28E5], (0,), ecx=donor)
    assert g.read(g.item_ptr + 0x280) == 0 and g.read(g.item_ptr + 0x250) == 0, g.read(g.item_ptr + 0x280)
    assert g.read(inventory + 0x10C + 4 * 15) == 0 and g.read(inventory + 0x16C) & 0xFF == 0
    print('Hidden Stash: held in the hand at death, the stash\'s artefact goes back dormant (state 0), '
          'not dropped (patch 78 finds nothing): PASS')


def main():
    original = open(sys.argv[1], 'rb').read()
    patched, _ = base.build(original, 'section', open(base.default_bin('section'), 'rb').read(), None,
                            open(base.late_join_bin(), 'rb').read())
    check(patched)
    carried(patched)


if __name__ == '__main__':
    main()
