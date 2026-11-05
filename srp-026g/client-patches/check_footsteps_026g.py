#!/usr/bin/env python3
"""Patch 101 (footsteps): a step first made by a re-simulation is heard
(docs/notes/footsteps.md).

    python check_footsteps_026g.py <original survarium.exe>

player_equipment_sound_effect::on_player_sound_event (0x2A6470) run whole,
in the original and in the patched exe, on a made-up effect, player, world
core and event, on_foot_step (0x2A6520) and the three rustles (0x2A6580,
0x2A6630, 0x2A66C0) stubbed to record their calls:
1. a step past the core's newest played time (+18FC4h) plays in both;
2. a step at or before it (a re-simulation): the original drops it; the
   patched exe plays it when its foot played nothing in the 250 ms before,
   so a copy of a step played (the same time, moved up to 249 ms either
   way) stays single and a new one (none played, or 250 ms on) is heard;
   the two feet and two effects are their own;
3. in both: a rustle at a played time is dropped, a new one plays; an
   event of another object, or with the feet not read (+44h), plays nothing;
   a player without a world core (+120h) plays every step;
4. the handler returns 0 and pops its argument, ESI, EDI, EBX and EBP kept.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

HANDLER = 0x2A6470
FOOT_STEP, RUSTLES = 0x2A6520, (0x2A6580, 0x2A6630, 0x2A66C0)
THREAD_SLOT = 0x870118

DATA = 0x30000000
EFFECT, EFFECT2, PLAYER, CORE, EVENT, OTHER = (DATA + 0x1000 * i for i in range(6))
CORE_SIZE = 0x20000
STUBS = DATA + 0x30000
THREAD, RETURN = STUBS, STUBS + 0x20
STACK = 0x21000000
KEPT = {UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_ESI: 0x05050505, UC_X86_REG_EDI: 0x0E0E0E0E,
        UC_X86_REG_EBP: 0x0019FF00}


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(DATA, 0x40000, UC_PROT_ALL)
        emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
        emu.mem_write(DATA, bytes(0x40000))
        emu.mem_write(THREAD, bytes.fromhex("b801000000c3"))     # GetCurrentThreadId: mov eax,1 / ret
        self.word(THREAD_SLOT, THREAD)
        for effect in (EFFECT, EFFECT2):
            self.word(effect + 0x3C, PLAYER)
            emu.mem_write(effect + 0x44, b"\x01")
        self.word(PLAYER + 0x120, CORE)
        self.calls = []
        for stub in (FOOT_STEP,) + RUSTLES:
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=stub, end=stub)
        emu.hook_add(UC_HOOK_CODE, self.done, begin=RETURN, end=RETURN)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        if address == FOOT_STEP:                 # ESI the effect; position, direction; ret 8
            effect = emu.reg_read(UC_X86_REG_ESI)
            self.calls.append(("step", effect, self.read(sp + 4) - effect))
            pops = 8
        else:
            self.calls.append(("rustle", RUSTLES.index(address)))
            pops = 4 if address == RUSTLES[1] else 0
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def done(self, emu, address, size, _):
        emu.emu_stop()

    def event(self, domain, time, maximum, effect=EFFECT, obj=PLAYER, feet=1, core=True):
        """One event through the handler: what it called."""
        self.emu.mem_write(effect + 0x44, bytes([feet]))
        self.word(PLAYER + 0x120, CORE if core else 0)
        self.word(CORE + 0x18FC4, maximum)
        self.emu.mem_write(EVENT, bytes(0x40))
        self.word(EVENT, obj)
        self.word(EVENT + 0x10, time)
        self.emu.mem_write(EVENT + 0x20, bytes([domain]))
        sp = STACK + 0x8000
        self.emu.mem_write(sp, struct.pack("<II", RETURN, EVENT))
        for reg, value in KEPT.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ECX, effect)
        self.emu.reg_write(UC_X86_REG_EAX, 0xA0A0A0A0)
        self.emu.reg_write(UC_X86_REG_EDX, 0xD0D0D0D0)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.calls = []
        self.emu.emu_start(HANDLER, 0, count=2000000)
        assert self.emu.reg_read(UC_X86_REG_EIP) == RETURN
        assert self.emu.reg_read(UC_X86_REG_ESP) == sp + 8, "pops its argument"
        assert self.emu.reg_read(UC_X86_REG_EAX) == 0
        for reg, value in KEPT.items():
            assert self.emu.reg_read(reg) == value, hex(reg)
        return self.calls


LEFT, RIGHT = 0x0, 0x18                          # the feet's positions: effect +0 / +18h


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    old, new = Run(original), Run(bytes(built))

    for run in (old, new):
        assert run.event(0, 1000, 950) == [("step", EFFECT, LEFT)]
        assert run.event(1, 1300, 1250) == [("step", EFFECT, RIGHT)]
    print("ok a step past the played time plays (both)")

    # the patched rule from a fresh state (the patch's table is in the image)
    new = Run(bytes(built))
    assert new.event(0, 1000, 950) == [("step", EFFECT, LEFT)]              # played the first time
    for t in (1000, 1249, 900, 760):                                        # its copies, moved
        assert new.event(0, t, 1500) == [], t
        assert old.event(0, t, 1500) == [], t
    assert new.event(0, 1250, 1500) == [("step", EFFECT, LEFT)]             # a new step, 250 ms on
    assert old.event(0, 1250, 1500) == []
    assert new.event(1, 1100, 1500) == [("step", EFFECT, RIGHT)]            # the other foot: none played
    assert new.event(0, 1300, 1500, effect=EFFECT2) == [("step", EFFECT2, LEFT)]
    assert old.event(1, 1100, 1500) == [] and old.event(0, 1300, 1500, effect=EFFECT2) == []
    print("ok re-simulated steps: the original drops them all; patched, a copy stays single, a new one plays,"
          " each foot and effect its own")

    for run in (old, new):
        assert run.event(2, 5000, 5000) == []
        assert run.event(2, 5001, 5000) == [("rustle", 0)]
        assert run.event(3, 5001, 5000) == [("rustle", 1)]
        assert run.event(4, 5001, 5000) == [("rustle", 2)]
        assert run.event(0, 9000, 8000, obj=OTHER) == []
        assert run.event(0, 9000, 9500, obj=OTHER) == []
        assert run.event(1, 9000, 8000, feet=0) == []
        assert run.event(1, 9000, 9500, feet=0) == []
        assert run.event(0, 9000, 9500, core=False) == [("step", EFFECT, LEFT)]
    print("ok rustles keep the game's rule; another object, feet not read: nothing; no world core: plays (both)")
    print("ok every run returned 0, popped 4, kept EBX ESI EDI EBP")


if __name__ == "__main__":
    main()
