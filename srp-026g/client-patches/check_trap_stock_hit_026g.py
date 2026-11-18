#!/usr/bin/env python3
"""Patch 151 (trap_stock_hit): a gun-stock hit disarms a trap
(docs/notes/trap-stock-hit.md).

    python check_trap_stock_hit_026g.py <original survarium.exe>

booby_trap_core::hit (0x305540, the slot bullet::try_pierce calls) run from
its entry on a made-up trap whose set defuses by a ranged hit, in the
original and in the patched exe, to one of its three ends: defuse_completed
(0x3050C0, EAX the trap), the trap's slot +40h (its byte +220h set: the
initiator pushed) or its return (ret 28h). Original: ranged defuses, every
other type returns. Patched: blunt goes where ranged does, by the byte +220h
too; every other type as before; the registers the game's code reads and the
objects' bytes unchanged.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

HIT, DEFUSE = 0x305540, 0x3050C0
THREAD_SLOT = 0x870118
RANGED, BLUNT, NONE = 0, 1, 8

DATA = 0x30000000
TRAP, SET, VTABLE, THREAD, SLOT_40, RETURN = (DATA + 0x1000 * i for i in range(6))
DATA_SIZE = 0x10000
STACK = 0x21000000
INITIATOR = 0x11112222
KEPT = {UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_ESI: 0x05050505, UC_X86_REG_EDI: 0x0D1D1D1D,
        UC_X86_REG_EBP: 0x0019FF00}


class Hit:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(DATA, DATA_SIZE, UC_PROT_ALL)
        emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
        emu.hook_add(UC_HOOK_CODE, self.stop)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def stop(self, emu, address, size, _):
        if address in (DEFUSE, SLOT_40, RETURN):
            self.stopped = address
            emu.emu_stop()

    def run(self, kind, explodes=False):
        """The hit of `kind` on a trap defused by ranged: where it ended."""
        emu = self.emu
        emu.mem_write(DATA, bytes(DATA_SIZE))
        emu.mem_write(THREAD, bytes.fromhex("b801000000c3"))          # GetCurrentThreadId: mov eax,1 / ret
        self.word(THREAD_SLOT, THREAD)
        self.word(TRAP, VTABLE)
        self.word(VTABLE + 0x40, SLOT_40)
        self.word(TRAP + 0x194, SET)
        self.word(TRAP + 0x198, 1)                                       # armed
        emu.mem_write(TRAP + 0x220, bytes([explodes]))
        self.word(SET + 0x298, RANGED)
        sp = STACK + 0x8000
        emu.mem_write(sp - 0x100, bytes(0x200))
        arguments = [INITIATOR, 2, 3, kind, 5, 6, 7, 8, 9, 10]
        emu.mem_write(sp, struct.pack("<11I", RETURN, *arguments))
        before = bytes(emu.mem_read(DATA, DATA_SIZE))
        for reg, value in KEPT.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ECX, TRAP)
        emu.reg_write(UC_X86_REG_ESP, sp)
        self.stopped = None
        emu.emu_start(HIT, 0, count=500000)
        assert self.stopped, hex(emu.reg_read(UC_X86_REG_EIP))
        assert {reg: emu.reg_read(reg) for reg in KEPT} == KEPT, "a register the caller keeps"
        assert bytes(emu.mem_read(DATA, DATA_SIZE)) == before, "the hit wrote the objects"
        assert bytes(emu.mem_read(sp, 44)) == struct.pack("<11I", RETURN, *arguments), "the arguments"
        esp = emu.reg_read(UC_X86_REG_ESP)
        if self.stopped == DEFUSE:
            assert emu.reg_read(UC_X86_REG_EAX) == TRAP and esp == sp - 4, "defuse_completed's trap"
            return "defused"
        if self.stopped == SLOT_40:
            assert emu.reg_read(UC_X86_REG_ECX) == TRAP and esp == sp - 8, "slot +40h's trap"
            assert struct.unpack("<I", bytes(emu.mem_read(esp + 4, 4)))[0] == INITIATOR, "slot +40h's initiator"
            return "slot +40h"
        assert esp == sp + 4 + 0x28, "ret 28h"
        return "dropped"


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    others = [kind for kind in range(2, 10) if kind != NONE] + [NONE, 0xFFFFFFFF]

    print("the original exe (booby_trap_core::hit 0x305540)")
    h = Hit(original)
    assert h.run(RANGED) == "defused" and h.run(RANGED, explodes=True) == "slot +40h"
    assert h.run(BLUNT) == "dropped" and h.run(BLUNT, explodes=True) == "dropped"
    assert all(h.run(kind) == "dropped" for kind in others)
    print("  ok ranged defuses (slot +40h with the byte +220h); blunt and every other type: dropped")

    print("the patched exe")
    h = Hit(bytes(built))
    assert h.run(BLUNT) == "defused" and h.run(BLUNT, explodes=True) == "slot +40h"
    print("  ok blunt (a gun-stock hit): defused, slot +40h with the byte +220h, as ranged")
    assert h.run(RANGED) == "defused" and h.run(RANGED, explodes=True) == "slot +40h"
    assert all(h.run(kind) == "dropped" and h.run(kind, explodes=True) == "dropped" for kind in others)
    print("  ok ranged defuses; slash, explosive, toxic, radiation, fire, shock and unnamed types: dropped")


if __name__ == "__main__":
    main()
