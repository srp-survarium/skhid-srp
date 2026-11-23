#!/usr/bin/env python3
"""Patch 179 (shot_protection): a shot ends the shooter's protection, and a
base's zone gives none back (docs/notes/shot-protection.md).

    python check_shot_protection_026g.py <original survarium.exe>

Requires Python `unicorn`. On a made-up player and damage model:

1. The hook at base_player::on_fire's `weapon_stats.fired` (0x2D7843, after
   patch 165's handler there) run to on_fire's epilogue (0x2D78A7, a match
   without statistics): the shooter's damage model's
   m_invulnerability_time_in_ms (+32Ch) is -1, whatever it was; a player
   without a damage model is left alone; the registers and the stack kept.
   The original leaves the time as it is.
2. The effect zone's stores (`effect_zone.protect`, 0x31AD0F to 0x31AD1B):
   a protection that runs or ran out is moved on as the original does (the
   fade +330h, the end +32Ch); an ended one (-1) is left alone, where the
   original starts it again.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

FIRED, FIRED_END = 0x2D7843, 0x2D78A7
ZONE, ZONE_END = 0x31AD0F, 0x31AD1B
THREAD_SLOT = 0x870118
NONE = 0xFFFFFFFF

DATA = 0x30000000
PLAYER, MODEL, CORE = DATA, DATA + 0x2000, DATA + 0x4000
THREAD = DATA + 0x3F000
STACK = 0x21000000
SP = STACK + 0x8000
GAME = {UC_X86_REG_EAX: 0x0A0A0A0A, UC_X86_REG_ECX: 0x0C0C0C0C, UC_X86_REG_EDX: 0x0D0D0D0D,
        UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_ESI: 0x05050505, UC_X86_REG_EDI: 0x0E0E0E0E,
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
        emu.hook_add(UC_HOOK_CODE, self.stop)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def fresh(self, until, model=MODEL):
        self.emu.mem_write(DATA, bytes(0x40000))
        self.emu.mem_write(THREAD, bytes.fromhex("b801000000c3"))     # GetCurrentThreadId: mov eax,1 / ret
        self.word(THREAD_SLOT, THREAD)
        self.word(PLAYER + 0x120, CORE)       # its world core, no statistics handler (+18FA8h 0)
        self.word(PLAYER + 0x2E0, model)
        self.word(MODEL + 0x32C, until)
        self.word(MODEL + 0x330, 7)

    def stop(self, emu, address, size, _):
        if address == self.end and self.started:
            self.stopped = True
            emu.emu_stop()
        self.started = True

    def run(self, start, end, regs):
        """From `start` to `end`: what the damage model holds after, the game's registers and stack kept."""
        before = {**GAME, **regs}
        for reg, value in before.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, SP)
        frame = bytes(self.emu.mem_read(SP, 0x40))
        self.end, self.stopped, self.started = end, False, False
        self.emu.emu_start(start, 0, count=500000)
        assert self.stopped, hex(start)
        assert self.emu.reg_read(UC_X86_REG_ESP) == SP and bytes(self.emu.mem_read(SP, 0x40)) == frame
        return {reg: self.emu.reg_read(reg) for reg in before}, self.read(MODEL + 0x32C), self.read(MODEL + 0x330)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    r, o = Run(bytes(built)), Run(original)

    print("a shot (weapon_stats.fired 0x2D7843)")
    shooter = {UC_X86_REG_ESI: PLAYER + 0x110}
    for until in (9000, 100, 0, NONE):
        for run, after in ((r, NONE), (o, until)):
            run.fresh(until)
            regs, end, fade = run.run(FIRED, FIRED_END, shooter)
            assert (end, fade) == (after, 7), (hex(until), hex(end), fade)
            # the replayed mov ecx,[esi+10h] / mov ebx,[ecx+18FA8h]
            assert regs == {**GAME, **shooter, UC_X86_REG_ECX: CORE, UC_X86_REG_EBX: 0}, regs
    print("  ok the shooter's protection ended (-1), running, run out or none; the original keeps it")
    r.fresh(9000, model=0)
    assert r.run(FIRED, FIRED_END, shooter)[1:] == (9000, 7)
    print("  ok a player without a damage model: nothing")

    print("a zone's player (effect_zone.protect 0x31AD0F)")
    zone = {UC_X86_REG_EAX: MODEL, UC_X86_REG_ECX: 2000, UC_X86_REG_EDX: 52000}
    for until in (51000, 100, 0):
        for run in (r, o):
            run.fresh(until)
            regs, end, fade = run.run(ZONE, ZONE_END, zone)
            assert (end, fade) == (52000, 2000) and regs == {**GAME, **zone}, (until, end, fade, regs)
    print("  ok a protection that runs or ran out: the end and the fade stored, as the original")
    for run, after in ((r, (NONE, 7)), (o, (52000, 2000))):
        run.fresh(NONE)
        regs, end, fade = run.run(ZONE, ZONE_END, zone)
        assert (end, fade) == after and regs == {**GAME, **zone}, (end, fade, regs)
    print("  ok an ended protection: left ended; the original starts it again")


if __name__ == "__main__":
    main()
