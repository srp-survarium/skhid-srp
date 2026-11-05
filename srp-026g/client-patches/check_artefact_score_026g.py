#!/usr/bin/env python3
"""Patch 80 (artefact_score): an anomaly's artefact taken into an empty
artefact slot counts event 16 besides the game's event 5.

Runs the installed Rust hook at use_execute's transfer site (0x31C810) to the
push after it, in the patched exe, on made-up objects: event 16 counted in the
user's place and kit cell, and the listener told, with an empty slot; nothing
with every slot full, no slots, no statistics handler, or a place or kit that
is no record's; the count wraps as the game's; the registers (EAX the replaced
lea's), the stack and every other byte unchanged. Then the game's own count of
the taken artefact (the handler's slot +10h, 0x26DAC0 -> 0x2F55F0) on the same
statistics: event 5's cell is the one the hook's indexing gives for event 5.
And take_artefact's own slot search (0x2FC850 to its two branches) on the
same slot layouts: it takes an empty slot exactly when the hook counted.
"""
import struct
import sys
from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import *
import patch_026g as base
import check_hide_ui_026g as hide

SITE, AFTER = 0x31C810, 0x31C816          # lea eax,[ebx+108h] (6 bytes), then push eax
NOTIFY = 0x2F5D40                         # notify_listener: eax the listener; place, event, amount; ret 0Ch
COUNT_ARTEFACT = 0x26DAC0                 # the statistics handler's slot +10h: ecx the handler; place, kit; ret 8
TAKE, TAKE_FREE, TAKE_FULL = 0x2FC850, 0x2FCA28, 0x2FC897

DATA = 0x30000000
USER, INVENTORY, SLOTS, CONTAINER = DATA, DATA + 0x1000, DATA + 0x2000, DATA + 0x3000
RETURN = DATA + 0x4000
CORE = DATA + 0x10000                     # the statistics handler at +18FA8h
HANDLER = DATA + 0x30000                  # its shared statistics at +8, their listener at +31A0h
SHARED = HANDLER + 8
LISTENER = SHARED + 0x31A0
ARTEFACT = DATA + 0x5000                  # what a full slot holds (never read)
RECORD, KITS = 0x1F8, 3


def cell(place, event, kit):
    """A count's address, as the hook indexes it: record, then (3 x event + kit) u16."""
    return SHARED + RECORD * place + 2 * (KITS * event + kit)


class Run(hide.Run):
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_map(DATA, 0x40000, UC_PROT_ALL)
        self.notified = []
        self.emu.hook_add(UC_HOOK_CODE, self.notify, begin=NOTIFY, end=NOTIFY)

    def notify(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.notified.append((emu.reg_read(UC_X86_REG_EAX), self.read(sp + 4), self.read(sp + 8), self.read(sp + 12)))
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 16)

    def setup(self, slots, place=7, kit=2, statistics=True, listener=True):
        """A user at `place` with `kit`, its artefact slots `slots` (True holds one)."""
        self.emu.mem_write(DATA, bytes(0x40000))
        self.word(USER + 0x10C, INVENTORY)
        self.emu.mem_write(USER + 0x114, bytes([place, kit]))
        self.word(INVENTORY + 0x168, SLOTS)
        self.emu.mem_write(INVENTORY + 0x16C, bytes([len(slots)]))
        for i, full in enumerate(slots):
            self.word(SLOTS + 4 * i, ARTEFACT if full else 0)
        self.word(CONTAINER + 0x54, CORE)
        self.word(CORE + 0x18FA8, HANDLER if statistics else 0)
        self.word(LISTENER, 0x12345 if listener else 0)

    def transfer(self):
        """The site to the push: what changed of DATA ({address: (before, after)} by u16)."""
        before = self.read(DATA, 0x40000)
        regs = {**hide.GAME, UC_X86_REG_EBX: USER, UC_X86_REG_EDI: CONTAINER}
        self.notified = []
        after, sp = self.run(SITE, (AFTER,), hide.STACK + 0x9000, regs)
        assert self.stopped == AFTER, hex(self.stopped or 0)
        assert sp == hide.STACK + 0x9000
        assert after == {**regs, UC_X86_REG_EAX: USER + 0x108}, after
        return diff(before, self.read(DATA, 0x40000))

    def count_artefact(self, place, kit):
        """The game's own count of a taken artefact, as use_execute calls it (0x31C845..0x31C85A)."""
        before = self.read(DATA, 0x40000)
        sp = hide.STACK + 0x9000 - 12
        self.emu.mem_write(sp, struct.pack("<III", RETURN, place, kit))
        self.notified = []
        _, sp_after = self.run(COUNT_ARTEFACT, (RETURN,), sp, {**hide.GAME, UC_X86_REG_ECX: HANDLER})
        assert self.stopped == RETURN and sp_after == sp + 12
        return diff(before, self.read(DATA, 0x40000))

    def take_search(self):
        """take_artefact to its branch: True for the empty slot's, False for the first slot's."""
        sp = hide.STACK + 0x9000 - 8
        self.emu.mem_write(sp, struct.pack("<II", RETURN, ARTEFACT))
        self.run(TAKE, (TAKE_FREE, TAKE_FULL), sp, {**hide.GAME, UC_X86_REG_EAX: INVENTORY})
        assert self.stopped in (TAKE_FREE, TAKE_FULL)
        return self.stopped == TAKE_FREE


def diff(before, after):
    return {DATA + i: (struct.unpack_from("<H", before, i)[0], struct.unpack_from("<H", after, i)[0])
            for i in range(0, len(before), 2) if before[i:i + 2] != after[i:i + 2]}


def main():
    original = open(sys.argv[1], 'rb').read()
    patched, _ = base.build(original, 'section', open(base.default_bin('section'), 'rb').read(), None,
                            open(base.late_join_bin(), 'rb').read())
    r = Run(patched)

    for label, slots in [('empty middle slot', [True, False, True]), ('every slot empty', [False, False]),
                         ('one slot, empty', [False]), ('last slot empty', [True, True, True, False])]:
        r.setup(slots)
        assert r.transfer() == {cell(7, 16, 2): (0, 1)}, label
        assert r.notified == [(LISTENER, 7, 16, 1)], (label, r.notified)
        assert r.take_search(), label
        print('ok', label, ': event 16 counted, the listener told; take_artefact takes the empty slot')

    for label, slots in [('every slot full', [True, True, True]), ('one slot, full', [True]), ('no slots', [])]:
        r.setup(slots)
        assert r.transfer() == {} and r.notified == [], label
        if slots:
            assert not r.take_search(), label
        print('ok', label, ': nothing counted' + ('; take_artefact replaces the first slot' if slots else ''))

    r.setup([False], listener=False)
    assert r.transfer() == {cell(7, 16, 2): (0, 1)} and r.notified == []
    print('ok no listener yet: counted, nobody told (as the game)')

    r.setup([False], statistics=False)
    assert r.transfer() == {} and r.notified == []
    print('ok no statistics handler: nothing (use_execute counts nothing either)')

    for place, kit in [(20, 0), (255, 0), (0, 3), (0, 255)]:
        r.setup([False], place=place, kit=kit)
        assert r.transfer() == {} and r.notified == [], (place, kit)
    print('ok a place or kit that is no record\'s: nothing')

    for place, kit in [(0, 0), (19, 2), (3, 1)]:
        r.setup([False], place=place, kit=kit)
        assert r.transfer() == {cell(place, 16, kit): (0, 1)}, (place, kit)
        assert r.count_artefact(place, kit) == {cell(place, 5, kit): (0, 1)}, (place, kit)
        assert r.notified == [(LISTENER, place, 5, 1)], r.notified
    print('ok the game\'s own count of event 5 lands where the hook\'s indexing puts event 5')

    r.setup([False])
    r.emu.mem_write(cell(7, 16, 2), struct.pack('<H', 0xFFFF))
    assert r.transfer() == {cell(7, 16, 2): (0xFFFF, 0)}
    print('ok the count wraps as the game\'s inc word')


if __name__ == '__main__':
    main()
