#!/usr/bin/env python3
"""Patch 109 (the artefact tree, docs/notes/artefact-skills.md) in Unicorn on the built exe.

Resistance: body_part_parameters::hit_by_type's armour slice (0x32A083 .. 0x32A0B0, the
game's own clamp and multiply) for every hit type, Resistance 0..5 and refused payloads, on
two kits. A constructed player, not a played match.
Usage: python check_artefact_skills_026g.py ORIGINAL_EXE
"""
import struct
import sys
from unicorn.x86_const import *
import patch_026g as base
from check_artefacts_026g import PLAYER, STACK
from check_medical_skills_026g import MedicalGame, bits, close

ARMOUR_AT = 0x32A083   # after `movss xmm0,[eax+8]` (the part's armour for the type), the hook
ARMOUR_END = 0x32A0B0  # after `movss [ebx],xmm1` (the damage written back)
RANKS = 40             # artefact_power: the packed ranks


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def hit(g, kind, armour, piercing, damage, pool):
    cell = g.block(16)
    g.word(cell, bits(damage))
    g.emu.reg_write(UC_X86_REG_ESP, STACK - 0x200)
    for reg, value in [(UC_X86_REG_XMM0, armour), (UC_X86_REG_XMM1, piercing), (UC_X86_REG_XMM3, damage)]:
        g.emu.reg_write(reg, bits(value))
    for reg, value in [(UC_X86_REG_EDI, kind), (UC_X86_REG_EBP, pool), (UC_X86_REG_EBX, cell)]:
        g.emu.reg_write(reg, value)
    g.emu.emu_start(ARMOUR_AT, ARMOUR_END, count=200_000)
    assert g.emu.reg_read(UC_X86_REG_EIP) == ARMOUR_END
    return struct.unpack('<f', g.emu.mem_read(cell, 4))[0]


def check_resistance(data, kit):
    outcomes = []
    for payload, rank in [(0, 0), (1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (5 + 8 + 256, 5),
                          (6, 0), (7, 0), (2.5, 0), (-1, 0), (4096, 0), (512 + 1, 0), (8 + 7 * 512 + 5, 0), (8 + 6 * 512 + 5, 5)]:
        g = MedicalGame(data)
        g.emu.mem_write(PLAYER + 0x115, bytes([kit]))
        g.ranks(payload, RANKS)
        for name in ['body', 'head', 'left_leg']:
            pool = g.pools[PLAYER][name]
            for kind in range(8):
                for armour, piercing in [(0, 0), (0.3, 0), (0.3, 0.1), (0.95, 0), (1.0, 0), (0.1, 0.5)]:
                    have = hit(g, kind, armour, piercing, 100.0, pool)
                    a = f32(f32(armour + 0.05 * rank) if kind >= 4 and rank else armour)
                    want = 100.0 * (1 - min(max(f32(a - piercing), 0), 1))
                    close(have, want)
                    outcomes.append((payload, name, kind, armour, piercing, round(have, 4)))
    print(f'Resistance kit {kit}: anomalous hits (4..7) A + 0.05 a rank 0..5 before piercing and the clamp, '
          'other types and refused payloads (6, 7, 2.5, -1, 4096, a draw without Keeper or of 7) unchanged: PASS')
    return outcomes


def main():
    original = open(sys.argv[1], 'rb').read()
    patched, _ = base.build(original, 'section', open(base.default_bin('section'), 'rb').read(), None,
                            open(base.late_join_bin(), 'rb').read())
    assert check_resistance(patched, 0) == check_resistance(patched, 2)
    print('Artefact skills: identical outcomes on two kits: PASS')


if __name__ == '__main__':
    main()
