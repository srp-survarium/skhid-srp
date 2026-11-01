#!/usr/bin/env python3
"""Execute the installed Rust hook at the native quick-slot update.
Use native medkit timing fields; stub only GFx Invoke and the skill modifier.
Check delay, effect, last stack, death, slot reuse and missing movie, a drug a
bullet stopped (patch 84: phase 3 for 1000 ms of game time, never for a natural
end, a death, a stale drug or another one), and that
all simulation objects, caller registers and stack survive unchanged.
"""
import struct
import sys
from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import *
import patch_026g as base
import check_hide_ui_026g as hide

DATA, PLAYER, INVENTORY, ITEM, PROFILE = 0x30000000, 0x30001000, 0x30003000, 0x30005000, 0x30008000
ITEM2 = 0x3000A000

class Run(hide.Run):
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_map(DATA, 0x10000, UC_PROT_ALL)
        self.movies(hide.HUD, True, False)
        self.word(PLAYER + 0x6b8, PROFILE)
        self.emu.mem_write(PLAYER + 0x115, b'\x02')
        self.word(INVENTORY + 0x10c + 4*7, ITEM)
        self.word(ITEM, 0xA1C79C)
        self.word(ITEM + 0x29c, 5000)
        self.word(ITEM + 0x2a4, 1000)
        self.emu.mem_write(ITEM + 0x2a0, struct.pack('<f', 0.5))
        self.modifiers = []
        self.emu.hook_add(UC_HOOK_CODE, self.modifier, begin=0x2d5ae0, end=0x2d5ae0)

    def modifier(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        assert emu.reg_read(UC_X86_REG_EAX) == PROFILE + 2*0x704 + 0xf4
        modifier = emu.reg_read(UC_X86_REG_ESI)
        assert modifier in (33, 34)
        assert struct.unpack('<ff', self.read(sp+4, 8)) == ((1000.0, 1.0) if modifier == 33 else (5000.0, 0.5))
        self.modifiers.append(modifier)
        emu.reg_write(UC_X86_REG_XMM0, struct.unpack('<I', struct.pack('<f', 1500.0 if modifier == 33 else 7500.0))[0])
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 12)

    def update(self, active, delay, remaining, alive=True, count=1, index=7, time=0):
        self.emu.mem_write(PLAYER + 0x33c, bytes([alive]))
        self.word(PLAYER + 0x324, time)
        self.emu.mem_write(ITEM + 0x2b0, bytes([active]))
        self.emu.mem_write(ITEM + 0x260, struct.pack('<H', count))
        self.word(ITEM + 0x2ac, delay)
        self.word(ITEM + 0x2a8, remaining)
        before = self.read(DATA, 0x10000)
        regs = {**hide.GAME, UC_X86_REG_ESI: hide.HUD, UC_X86_REG_EBP: PLAYER,
                UC_X86_REG_EBX: INVENTORY, UC_X86_REG_EDI: 1, UC_X86_REG_EAX: index}
        after, sp = self.run(0x23b7a0, (0x23b7a7,), hide.STACK + 0x9000, regs)
        assert self.stopped == 0x23b7a7
        assert sp == hide.STACK + 0x9000
        assert after == {**regs, UC_X86_REG_EAX: (INVENTORY + 0x10c + 4*index) & 0xffffffff}, after
        assert self.read(DATA, 0x10000) == before, 'presentation modified simulation'
        if self.calls:
            assert self.calls[0][2] == b'root.set_quick_slot_activity'
            assert self.calls[0][1] == hide.HUD_MOVIE
        return self.invoked

def main():
    original = open(sys.argv[1], 'rb').read()
    patched, _ = base.build(original, 'section', open(base.default_bin('section'), 'rb').read(), None,
                            open(base.late_join_bin(), 'rb').read())
    r = Run(patched)
    for label, args, expected in [
        ('idle', (False, 0, 0), (1, 0, 0, 0)),
        ('use delay', (True, 1000, 7500), (1, 1, 1000, 1500)),
        ('last delay millisecond', (True, 1, 7500), (1, 1, 1, 1500)),
        ('death during delay', (True, 500, 7500, False), (1, 0, 0, 0)),
        ('cancel during delay', (False, 500, 7500), (1, 0, 0, 0)),
        ('effect starts', (True, 0, 7500), (1, 2, 7500, 7500)),
        ('last item remains active', (True, 0, 4501, True, 0), (1, 2, 4501, 7500)),
        ('last active millisecond', (True, 0, 1), (1, 2, 1, 7500)),
        ('finished', (True, 0, 0), (1, 0, 0, 0)),
        ('death', (True, 0, 4000, False), (1, 0, 0, 0)),
        ('cancelled', (False, 0, 4000), (1, 0, 0, 0)),
    ]:
        assert r.update(*args) == [expected], label
        print('ok', label)
    # Native artefact quick slots encode offsets to a different allocation,
    # including wrapped negative offsets; the medical reader must not follow them.
    assert r.update(True, 0, 7500, index=0x1000) == [(1, 0, 0, 0)]
    assert r.update(True, 0, 7500, index=0xfffffff0) == [(1, 0, 0, 0)]
    print('ok external artefact slot offsets excluded from medical reads')
    r.word(ITEM, 0xA042AC)
    assert r.update(True, 0, 7500) == [(1, 0, 0, 0)]
    r.word(INVENTORY + 0x10c + 4*7, 0)
    assert r.update(True, 0, 7500) == [(1, 0, 0, 0)]
    r.word(hide.HUD + 8, 0)
    assert r.update(True, 0, 7500) == []
    assert r.modifiers == [33, 33, 34, 34, 34]
    print('ok slot reuse, no item, missing movie; registers/stack and simulation unchanged')

    # Patch 84: a drug a bullet stopped (set_active(0) leaves its timers) shows
    # phase 3, red, for 1000 ms of the local player's simulation clock (+324h).
    r.word(hide.HUD + 8, hide.HUD_HOLDER)
    r.word(INVENTORY + 0x10c + 4*7, ITEM)
    r.word(ITEM, 0xA1C79C)
    red, off = (1, 3, 0, 0), (1, 0, 0, 0)
    for label, using, stopped in [('in the use time', (True, 1000, 7500), (False, 600, 7500)),
                                  ('in the active time', (True, 0, 4000), (False, 0, 3500)),
                                  ('before the first active tick', (True, 0, 7500), (False, 0, 7500))]:
        assert r.update(*using, time=10000)[0][1] in (1, 2), label
        assert r.update(*stopped, time=10050) == [red], label
        assert r.update(*stopped, time=11049) == [red], label
        assert r.update(*stopped, time=11050) == [off], label
        assert r.update(*stopped, time=20000) == [off], label
        assert r.update(False, 0, 0, time=20050) == [off], label
        print('ok stopped', label, ': red for 1000 ms of game time, then off for good')
    assert r.update(True, 0, 4000, count=0, time=30000)[0][1] == 2
    assert r.update(False, 0, 3500, count=0, time=30050) == [red]
    assert r.update(False, 0, 3500, count=0, time=31050) == [off]
    print('ok the last unit: red, then off as the native end')
    assert r.update(True, 0, 50, time=40000)[0][1] == 2
    assert r.update(False, 0, 0, time=40050) == [off]
    print('ok a natural end (timers spent): no red')
    assert r.update(True, 0, 4000, time=50000)[0][1] == 2
    assert r.update(True, 0, 4000, alive=False, time=50050) == [off]
    assert r.update(False, 0, 4000, alive=False, time=50100) == [off]
    assert r.update(False, 0, 4000, time=58000) == [off]
    print('ok a death, and the dead life\'s drug after the respawn: no red')
    assert r.update(True, 0, 4000, time=60000)[0][1] == 2
    assert r.update(False, 0, 3500, time=60050) == [red]
    assert r.update(True, 0, 3450, time=60100)[0][:2] == (1, 2)
    print('ok a rollback that takes the hit back: in use again, no red')
    assert r.update(True, 0, 4000, time=70000)[0][1] == 2
    r.word(ITEM2 + 0x0, 0xA1C79C)
    r.word(INVENTORY + 0x10c + 4*7, ITEM2)
    r.emu.mem_write(ITEM2 + 0x2b0, b'\x00')
    r.word(ITEM2 + 0x2ac, 0)
    r.word(ITEM2 + 0x2a8, 900)
    r.emu.mem_write(PLAYER + 0x33c, b'\x01')
    r.word(PLAYER + 0x324, 70050)
    regs = {**hide.GAME, UC_X86_REG_ESI: hide.HUD, UC_X86_REG_EBP: PLAYER,
            UC_X86_REG_EBX: INVENTORY, UC_X86_REG_EDI: 1, UC_X86_REG_EAX: 7}
    r.run(0x23b7a0, (0x23b7a7,), hide.STACK + 0x9000, regs)
    assert r.invoked == [off], r.invoked
    print('ok another drug in the slot, ended with time left: no red')

if __name__ == '__main__': main()
