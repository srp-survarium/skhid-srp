#!/usr/bin/env python3
"""Patch 109's Courier (docs/notes/artefact-skills.md "Courier") in Unicorn on the built exe.

    python check_artefact_courier_026g.py <original survarium.exe>

Requires Python `unicorn`. The game's own code runs on made-up objects (check_artefact_keeper_026g's
places, kits and artefacts): artefacts go into a kit by the game's take_artefact (0x2FC850:
set_item, on_taken), a death is the game's on_holder_death (0x2FCA50) and artefact_base::tick
(0x313200, its holder dead: the passive unlinked, state 5), and a respawn is the respawn rule's
spawn (0x316DA0) at its two sites with patch 109's hooks: from 0x316DDA through the write that
makes the chosen kit current (0x316DE3), and from 0x316EAB (its end) to its `ret`. Before them, as
the respawn rule's tick does (0x31699B, base_player::remove with 1), the dead kit leaves the world:
the game's own unload_to_profile (0x2FD2D0), which clears its container range (0x2FDBE8); between
the two sites the new kit is set up as setup_from_profile leaves it (its container range, as
0x2FD29F sets it; on_taken for what it holds) and alive. Every artefact taken is in the world's
registered artefacts (+18940h), as every map artefact is. Stubs: each kind's passive link and
unlink (vfn +7Ch, +84h), counted.

Checked, the owner's runs (2026-11-21): kit 0 with two artefacts dies into kit 1 with no
container: nothing moves (both stay in kit 0, state 5); kit 1 dies into kit 2: nothing (kit 0's
are not kit 1's); kit 2 takes one and dies into kit 0 with three containers: kit 0 holds three
(its two, kit 2's one, which kit 2 no longer holds). Kit 0 with two dies into kit 1 with one
container: one moves (the first container's), the other stays in kit 0. Each run fails on revision
69's build, which read the dead kit's artefacts through the container range its unload had
cleared. Each moved artefact: in the new kit's inventory and container, state 3, its passive
unlinked once (the death's) and linked once (the take's); the old kit's slot empty and its weight
to recalculate. Nothing without the
skill, nothing at a respawn in the same kit, and only the spawning place's kits. Says nothing of a
real match, the HUD or the save (the moved artefact is a pickup's: its owner and slot, which every
world carries).
"""
import sys

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_ESP

import patch_026g as base
from check_artefact_keeper_026g import ARTEFACT_TICK, ON_HOLDER_DEATH, ON_TAKEN, PASSIVE_LINK, PASSIVE_UNLINK, TAKE, Keeper

COURIER = 1 << 14             # modifier 40 bit 14
KITS, KITS_END = 0x316DDA, 0x316DE6     # the spawn's kit read, through mov [eax+0Ch],dl
END, END_RET = 0x316EAB, 0x316EB0       # the spawn's end: its pops, up to ret 0Ch
FRAME = 0x21006000
UNLOAD = 0x2FD2D0             # inventory::unload_to_profile (al the kit; the inventory, the profile)


class Courier(Keeper):
    def __init__(self, data):
        super().__init__(data)
        self.links, self.unlinks = [], []
        for address in PASSIVE_LINK:
            self.emu.hook_add(UC_HOOK_CODE, lambda emu, a, s, _: self.links.append(self.ecx()), begin=address, end=address)
        for address in PASSIVE_UNLINK:
            self.emu.hook_add(UC_HOOK_CODE, lambda emu, a, s, _: self.unlinks.append(self.ecx()), begin=address, end=address)
        self.kits = {}

    def kit(self, place, kit, containers, ranks=COURIER):
        player, inventory, slots = self.player(place, ranks, containers, kit)
        self.holder(player, inventory)
        # The profile's kit slot words (+98h + kit x 704h): no lobby item in any slot.
        profile = self.read(player + 0x6B8)
        self.emu.mem_write(profile + 0x98 + kit * 0x704, b'\xff' * 4 * 23)
        self.kits[(place, kit)] = (player, inventory, slots, containers)
        return player, inventory, slots

    def entry(self, place):
        return self.places + place * 0x30

    def take(self, place, kit, kind):
        _, inventory, _, _ = self.kits[(place, kit)]
        item = self.artefact(kind)
        self.word(item + 0x280, 0)
        held = self.block(4)
        self.word(held, item)
        self.call(TAKE, (held,), eax=inventory)
        assert self.read(item + 0x250) == inventory and self.state(item) == 3
        end = self.read(self.world + 0x18944)
        self.word(end, item)
        self.word(self.world + 0x18944, end + 4)
        return item

    def held(self, place, kit):
        _, _, slots, containers = self.kits[(place, kit)]
        return [self.read(slots + 4 * n) for n in range(containers)]

    def die(self, place, kit):
        player, inventory, _, _ = self.kits[(place, kit)]
        self.emu.mem_write(player + 0x33C, b'\x00')
        self.call(ON_HOLDER_DEATH, eax=inventory)
        for item in self.held(place, kit):
            if item:
                self.call(ARTEFACT_TICK, (16, 1000), ecx=item)
                assert self.state(item) == 5, 'its holder dead: passive off'

    def leave(self, place, kit):
        """The respawn rule's tick takes the dead kit's player out of the world before the spawn
        (0x31699B: base_player::remove with 1, whose unload_to_profile clears its container range)."""
        player, inventory, _, _ = self.kits[(place, kit)]
        self.call(UNLOAD, (inventory, self.read(player + 0x6B8)), eax=kit)
        assert self.read(inventory + 0x168) == 0 and self.read(inventory + 0x16C) & 0xFF == 0

    def respawn(self, place, into):
        """The dead kit out of the world, then the spawn's two sites around the new kit's setup
        (setup_from_profile: its container range, on_taken for what it holds; the kit alive)."""
        entry = self.entry(place)
        current = self.read(entry + 0xC) & 0xFF
        self.leave(place, current)
        self.emu.mem_write(entry + 0xD, bytes([into]))
        old = self.read(entry + 4 * current)
        self.emu.reg_write(UC_X86_REG_ESP, FRAME)
        for reg, value in ((UC_X86_REG_EAX, entry), (UC_X86_REG_EBX, place), (UC_X86_REG_ECX, old), (UC_X86_REG_EDX, current)):
            self.emu.reg_write(reg, value)
        self.emu.emu_start(KITS, KITS_END, count=200_000)
        assert self.read(entry + 0xC) & 0xFF == into, 'the spawn made the chosen kit current'
        player, inventory, slots, containers = self.kits[(place, into)]
        self.word(inventory + 0x168, slots)
        self.emu.mem_write(inventory + 0x16C, bytes([containers]))
        if into != current:
            for item in self.held(place, into):
                if item:
                    self.call(ON_TAKEN, ecx=item)
        self.emu.mem_write(player + 0x33C, b'\x01')
        self.emu.reg_write(UC_X86_REG_ESP, FRAME)
        self.emu.reg_write(UC_X86_REG_EBX, place)
        self.emu.reg_write(UC_X86_REG_ESI, player)
        self.emu.emu_start(END, END_RET, count=2_000_000)


def owners_runs(data):
    g = Courier(data)
    _, inv0, _ = g.kit(0, 0, containers=3)
    _, inv1, _ = g.kit(0, 1, containers=0)
    _, inv2, _ = g.kit(0, 2, containers=2)
    g.emu.mem_write(g.entry(0) + 0xC, b'\x00')
    a, b = g.take(0, 0, 'Spring'), g.take(0, 0, 'Larkspur')
    g.die(0, 0)
    # Kit 0 with two into kit 1 with none: nothing moves.
    g.respawn(0, 1)
    assert g.held(0, 0) == [a, b, 0] and g.state(a) == g.state(b) == 5
    assert g.read(a + 0x250) == g.read(b + 0x250) == inv0
    # Kit 1 into kit 2: nothing (kit 0's are not the dying kit's).
    g.die(0, 1)
    g.respawn(0, 2)
    assert g.held(0, 2) == [0, 0] and g.held(0, 0) == [a, b, 0]
    # Kit 2 takes one and dies into kit 0 (three containers): kit 0 holds three.
    c = g.take(0, 2, 'Onyx')
    g.die(0, 2)
    g.respawn(0, 0)
    assert g.held(0, 0) == [a, b, c] and g.held(0, 2) == [0, 0]
    assert g.read(c + 0x250) == inv0 and g.read(c + 0x254) == 16 + 2 and g.state(c) == 3
    assert g.read(inv2 + 0x184) & 0xFF == 1, "the old kit's weight to recalculate"
    assert (g.unlinks.count(c), g.links.count(c)) == (1, 2), 'unlinked once (the death), linked by each take'
    assert all(g.state(item) == 3 for item in (a, b, c))
    want = ['srp: courier: place 0 kit 0 -> kit 1: 0 artefacts moved, 2 left',
            'srp: courier: place 0 kit 2 -> kit 0: 1 artefacts moved, 0 left']
    assert [line for line in g.lines if 'courier' in line] == want, g.lines
    assert inv1  # kit 1 never held one
    print('Courier: kit 0 (two) into kit 1 (no container): nothing; kit 1 into kit 2: nothing of kit 0\'s; '
          'kit 2 (one) into kit 0 (three): three, the moved one in its third container, state 3, unlinked once '
          'and linked by the take: PASS')


def as_many_as_fit(data):
    g = Courier(data)
    _, inv0, _ = g.kit(3, 0, containers=2)
    _, inv1, _ = g.kit(3, 1, containers=1)
    g.emu.mem_write(g.entry(3) + 0xC, b'\x00')
    a, b = g.take(3, 0, 'Spring'), g.take(3, 0, 'Spring')
    links = len(g.links)
    g.die(3, 0)
    g.respawn(3, 1)
    assert g.held(3, 1) == [a] and g.held(3, 0) == [0, b]
    assert g.read(a + 0x250) == inv1 and g.state(a) == 3 and g.read(a + 0x254) == 16
    assert g.read(b + 0x250) == inv0 and g.state(b) == 5
    assert g.unlinks.count(a) == 1 and len(g.links) == links + 1
    assert 'srp: courier: place 3 kit 0 -> kit 1: 1 artefacts moved, 1 left' in g.lines, g.lines
    print('Courier: kit 0 (two) into kit 1 (one container): the first moves, the other stays in kit 0: PASS')


def left_alive(data):
    """A kit left alive (patch 31's switch in a countdown, a round's start): base_player::remove's
    remove_alive (0x2D99C0) writes alive 0 and its unload_to_profile leaves the artefact linked
    (state 3); it moves as remove_item takes it (on_removed unlinks it once), then the take."""
    g = Courier(data)
    _, inv0, _ = g.kit(8, 0, containers=1)
    _, inv1, _ = g.kit(8, 1, containers=2)
    g.emu.mem_write(g.entry(8) + 0xC, b'\x00')
    a = g.take(8, 0, 'Spring')
    g.emu.mem_write(g.kits[(8, 0)][0] + 0x33C, b'\x00')
    g.respawn(8, 1)
    assert g.held(8, 1) == [a, 0] and g.held(8, 0) == [0], (g.held(8, 1), g.held(8, 0))
    assert g.read(a + 0x250) == inv1 and g.state(a) == 3
    assert g.read(inv0 + 0x184) & 0xFF == 1, "the old kit's weight to recalculate"
    assert (g.unlinks.count(a), g.links.count(a)) == (1, 2), 'unlinked by on_removed, linked by each take'
    assert 'srp: courier: place 8 kit 0 -> kit 1: 1 artefacts moved, 0 left' in g.lines, g.lines
    print("Courier: a kit left alive (a countdown's switch): its artefact moves, unlinked once by on_removed "
          'and linked by the take: PASS')


def nothing(data):
    g = Courier(data)
    # No skill.
    g.kit(5, 0, containers=2, ranks=0)
    g.kit(5, 1, containers=2, ranks=0)
    g.emu.mem_write(g.entry(5) + 0xC, b'\x00')
    a = g.take(5, 0, 'Onyx')
    g.die(5, 0)
    g.respawn(5, 1)
    assert g.held(5, 1) == [0, 0] and g.held(5, 0) == [a, 0]
    # The same kit again: the game keeps them (on_taken), nothing moves.
    g.kit(6, 0, containers=2)
    g.kit(6, 1, containers=2)
    g.emu.mem_write(g.entry(6) + 0xC, b'\x00')
    b = g.take(6, 0, 'Larkspur')
    g.die(6, 0)
    g.call(ON_TAKEN, ecx=b)
    g.respawn(6, 0)
    assert g.held(6, 0) == [b, 0] and g.held(6, 1) == [0, 0]
    # Another place's kit is never the giver: place 7 into its kit 1 while place 6's kit 0 holds one.
    g.die(6, 0)
    g.kit(7, 0, containers=2)
    g.kit(7, 1, containers=2)
    g.emu.mem_write(g.entry(7) + 0xC, b'\x00')
    g.die(7, 0)
    g.respawn(7, 1)
    assert g.held(7, 1) == [0, 0] and g.held(6, 0) == [b, 0]
    assert not [line for line in g.lines if 'courier' in line], g.lines
    print('Courier: nothing without the skill, at a respawn in the same kit, or from another place: PASS')


def main():
    original = open(sys.argv[1], 'rb').read()
    patched, _ = base.build(original, 'section', open(base.default_bin('section'), 'rb').read(), None,
                            open(base.late_join_bin(), 'rb').read())
    owners_runs(patched)
    as_many_as_fit(patched)
    left_alive(patched)
    nothing(patched)


if __name__ == '__main__':
    main()
