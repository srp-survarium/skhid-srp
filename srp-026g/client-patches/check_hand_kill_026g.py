#!/usr/bin/env python3
"""Patch 108 (hand_kill): a gun-stock hit kills, wherever it lands
(docs/notes/hand-kill.md).

    python check_hand_kill_026g.py <original survarium.exe>

The installed chain at hit_body_part's `medical.hit` (0x2E50C7: patch 108,
then patch 94) run in the patched exe to the end of its replayed span
(0x2E50CC), on made-up objects: a blunt hit on any zone (hit_params' every
zone, and names it has not) by another player deals 1000 (the frame's local,
[esp+8]); nothing for any other hit type, no attacker (0FFh) or the
victim's own place (a fall's); patch 94's rule left out (the hit has no owner, +20h 0); the
replayed `lea ecx,[esp+10h] / push ecx`, every other register and the
objects' bytes unchanged.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

SITE, AFTER = 0x2E50C7, 0x2E50CC          # lea ecx,[esp+10h] / push ecx (5 bytes)
THREAD_SLOT = 0x870118
LETHAL = 1000.0
BLUNT = 1
ZONES = (b"back", b"front", b"brain", b"face", b"right_hip", b"left_hip", b"right_foot", b"left_foot",
         b"right_arm", b"left_arm", b"right_forearm", b"left_forearm", b"pain", b"right_hand", b"",
         b"right_arms")

DATA = 0x30000000
VICTIM, MODEL, HIT, ZONE, THREAD = (DATA + 0x1000 * i for i in range(5))
DATA_SIZE = 0x10000
STACK = 0x21000000
AMOUNT = 0.75
VICTIM_PLACE = 4
GAME = {UC_X86_REG_EAX: 0x0A0A0A0A, UC_X86_REG_ECX: 0x0C0C0C0C, UC_X86_REG_EDX: 0x0D0D0D0D,
        UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_EBP: 0x0019FF00}
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)


class Hook:
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

    def read(self, at, size):
        return bytes(self.emu.mem_read(at, size))

    def stop(self, emu, address, size, _):
        if address == AFTER and self.started:
            self.stopped = address
            emu.emu_stop()
        self.started = True

    def setup(self, zone=b"right_forearm", kind=BLUNT, attacker=1):
        self.emu.mem_write(DATA, bytes(DATA_SIZE))
        self.emu.mem_write(THREAD, bytes.fromhex("b801000000c3"))     # GetCurrentThreadId: mov eax,1 / ret
        self.word(THREAD_SLOT, THREAD)
        self.emu.mem_write(VICTIM + 0x114, bytes([VICTIM_PLACE]))
        self.word(MODEL + 0x3B0, VICTIM)
        self.emu.mem_write(ZONE, zone + b"\0")
        self.word(HIT, ZONE)
        self.word(HIT + 0x1C, kind)
        self.emu.mem_write(HIT + 0x44, bytes([attacker]))

    def hit(self):
        """The site to the span's end: the frame's damage."""
        sp = STACK + 0x8000
        self.emu.mem_write(sp, bytes(0x20))
        self.emu.mem_write(sp + 8, struct.pack("<f", AMOUNT))
        before = self.read(DATA, DATA_SIZE)
        regs = {**GAME, UC_X86_REG_ESI: HIT, UC_X86_REG_EDI: MODEL}
        for reg, value in regs.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.stopped, self.started = None, False
        self.emu.emu_start(SITE, 0, count=500000)
        assert self.stopped == AFTER, hex(self.emu.reg_read(UC_X86_REG_EIP))
        assert self.emu.reg_read(UC_X86_REG_ESP) == sp - 4, "the replayed push"
        assert self.emu.reg_read(UC_X86_REG_ECX) == sp + 0x10, "the replayed lea"
        assert struct.unpack("<I", self.read(sp - 4, 4))[0] == sp + 0x10
        after = {reg: self.emu.reg_read(reg) for reg in REGS}
        assert after == {reg: regs[reg] for reg in REGS}, {hex(k): hex(v) for k, v in after.items()}
        assert self.read(DATA, DATA_SIZE) == before, "the hook wrote the objects"
        assert self.read(sp, 8) == bytes(8) and self.read(sp + 12, 0x14) == bytes(0x14), "the frame's other bytes"
        return struct.unpack("<f", self.read(sp + 8, 4))[0]


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, True)

    print("the chain (patched exe, medical.hit 0x2E50C7)")
    h = Hook(bytes(built))
    for zone in ZONES:
        for attacker in (0, 1, 19):
            h.setup(zone=zone, attacker=attacker)
            assert h.hit() == LETHAL, (zone, attacker)
    print("  ok a blunt hit by another player on any zone (torso, head, hips, feet, arms, others): 1000")

    for kind in [0] + list(range(2, 9)):
        h.setup(kind=kind)
        assert h.hit() == AMOUNT, kind
    print("  ok ranged, slash, explosive, toxic, radiation, fire, shock and an unnamed type: the hit's own")

    for attacker in (0xFF, VICTIM_PLACE):
        h.setup(attacker=attacker)
        assert h.hit() == AMOUNT, attacker
    print("  ok no attacker, the victim's own place (a fall): the hit's own")


if __name__ == "__main__":
    main()
