#!/usr/bin/env python3
"""Patch 108 (hand_kill): a gun-stock hit to a hand kills when the attacker
carries the Anomalous Force Protein (docs/notes/force-protein-hand-kill.md).

    python check_hand_kill_026g.py <original survarium.exe>

The installed chain at hit_body_part's `medical.hit` (0x2E50C7: patch 108,
then patch 94) run in the patched exe to the end of its replayed span
(0x2E50CC), on made-up objects, with the game's own apply_modifier
(0x2D5AE0) reading the attacker's kit: a blunt hit on each of the four hand
zones by an attacker whose current kit sums modifier 47 above 0 deals 1000
(the frame's local, [esp+8]); nothing for any other zone, any other hit
type, an attacker with 0 or below, no attacker (0FFh), a place past the
places' end, the victim's own place, a place's other kit; patch 94's rule
left out (the hit has no owner, +20h 0); the replayed `lea ecx,[esp+10h] /
push ecx`, every other register and the objects' bytes unchanged.
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
HAND_ZONES = (b"right_arm", b"left_arm", b"right_forearm", b"left_forearm")
OTHER_ZONES = (b"back", b"front", b"brain", b"face", b"right_hip", b"left_foot", b"pain", b"right_hand",
               b"right_arms", b"right_ar")

DATA = 0x30000000
VICTIM, MODEL, HIT, CORE = DATA, DATA + 0x1000, DATA + 0x2000, DATA + 0x3000
PLACES = DATA + 0x20000                   # 30h a place
PLAYERS = DATA + 0x22000                  # player i at PLAYERS + 0x1000 x i
PROFILES = DATA + 0x30000                 # profile i at PROFILES + 0x5000 x i
ZONE = DATA + 0x50000
THREAD = DATA + 0x51000
DATA_SIZE = 0x60000
STACK = 0x21000000
AMOUNT = 0.75
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

    def read(self, at, size=4):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0] if size == 4 else bytes(self.emu.mem_read(at, size))

    def stop(self, emu, address, size, _):
        if address == AFTER and self.started:
            self.stopped = address
            emu.emu_stop()
        self.started = True

    def setup(self, zone=b"right_forearm", kind=BLUNT, attacker=1, places=3, kits=((0.0,), (0.0, 1.0), (0.0,)),
              current=None, victim_place=0):
        """`kits`: per place, its kits' sums of modifier 47 (a player each); `current`: per place, its
        current kit (0 by default); the victim is the place `victim_place`'s kit 0 player."""
        self.emu.mem_write(DATA, bytes(DATA_SIZE))
        self.emu.mem_write(THREAD, bytes.fromhex("b801000000c3"))     # GetCurrentThreadId: mov eax,1 / ret
        self.word(THREAD_SLOT, THREAD)
        current = current or [1 if place == 1 else 0 for place in range(len(kits))]
        n = 0
        for place, sums in enumerate(kits):
            entry = PLACES + 0x30 * place
            for kit, value in enumerate(sums):
                player = PLAYERS + 0x1000 * n
                profile = PROFILES + 0x5000 * n
                n += 1
                self.word(entry + 4 * kit, player)
                self.emu.mem_write(player + 0x115, bytes([kit]))
                self.word(player + 0x6B8, profile)
                self.word(player + 0x120, CORE)
                modifiers = profile + 0xF4 + 0x704 * kit
                self.emu.mem_write(modifiers + 4 * 47, struct.pack("<f", value))
                self.emu.mem_write(modifiers + 0x120 + 20 * 47, b"\x01")   # its list summed: no refresh
            self.emu.mem_write(entry + 0xC, bytes([current[place]]))
        victim = self.read(PLACES + 0x30 * victim_place)
        self.word(MODEL + 0x3B0, victim)
        self.word(CORE + 0x18458, PLACES)
        self.word(CORE + 0x1845C, PLACES + 0x30 * places)
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
        assert self.read(sp - 4) == sp + 0x10
        after = {reg: self.emu.reg_read(reg) for reg in REGS}
        assert after == {reg: regs[reg] for reg in REGS}, {hex(k): hex(v) for k, v in after.items()}
        assert self.read(DATA, DATA_SIZE) == before, "the hook wrote the objects"
        assert self.read(sp, 8) == bytes(8) and self.read(sp + 12, 0x14) == bytes(0x14), "the frame's other bytes"
        return struct.unpack("<f", bytes(self.emu.mem_read(sp + 8, 4)))[0]


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())

    print("the chain (patched exe, medical.hit 0x2E50C7; the game's apply_modifier)")
    h = Hook(bytes(built))
    for zone in HAND_ZONES:
        h.setup(zone=zone)
        assert h.hit() == LETHAL, zone
    print("  ok a blunt hit by a carrier on right_arm, left_arm, right_forearm, left_forearm: 1000")

    for zone in OTHER_ZONES:
        h.setup(zone=zone)
        assert h.hit() == AMOUNT, zone
    print("  ok any other zone (the body's, the head's, a leg's, a pool's, near names): the hit's own")

    for kind in [0] + list(range(2, 9)):
        h.setup(kind=kind)
        assert h.hit() == AMOUNT, kind
    print("  ok ranged, slash, explosive, toxic, radiation, fire, shock and an unnamed type: the hit's own")

    for value in (0.0, -1.0):
        h.setup(kits=((0.0,), (0.0, value), (0.0,)))
        assert h.hit() == AMOUNT, value
    h.setup(kits=((0.0,), (0.0, 0.25), (0.0,)))
    assert h.hit() == LETHAL
    print("  ok the attacker's kit at 0 or below: the hit's own; any sum above 0: 1000")

    h.setup(attacker=0xFF)
    assert h.hit() == AMOUNT
    h.setup(attacker=3)
    assert h.hit() == AMOUNT
    h.setup(attacker=2, places=2)
    assert h.hit() == AMOUNT
    print("  ok no attacker, a place past the places' end: the hit's own")

    h.setup(attacker=1, victim_place=1, current=[0, 0, 0], kits=((0.0,), (1.0,), (0.0,)))
    assert h.hit() == AMOUNT
    print("  ok the victim's own place (a carrier): the hit's own")

    h.setup(kits=((0.0,), (0.0, 1.0), (0.0,)), current=[0, 0, 0])
    assert h.hit() == AMOUNT
    h.setup(kits=((0.0,), (1.0, 0.0), (0.0,)), current=[0, 0, 0])
    assert h.hit() == LETHAL
    h.setup(kits=((0.0,), (0.0, 0.0, 1.0), (0.0,)), current=[0, 2, 0])
    assert h.hit() == LETHAL
    print("  ok the place's current kit's player decides, not another of its kits")


if __name__ == "__main__":
    main()
