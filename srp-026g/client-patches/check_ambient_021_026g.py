#!/usr/bin/env python3
"""Execute installed original-lighting hooks and their actual-scene isolation."""
import math
import os
import struct
import sys
from pathlib import Path
from unicorn.x86_const import *
import patch_026g as P
from check_artefacts_026g import Game, STACK


def main():
    original = Path(sys.argv[1]).read_bytes()
    revision = os.environ.get('SKHID_AMBIENT_PREVIEW_REVISION')
    image, _ = P.build(original, 'section', Path(P.default_bin('section')).read_bytes(),
                       None, Path(P.late_join_bin()).read_bytes(),
                       revision=int(revision) if revision is not None else None)
    g = Game(image)
    game, obj, lobby = g.block(0x5900), g.block(0x180), g.block(0x200)
    world = game + 0x18
    g.word(world + 0x9c, game)
    g.word(lobby + 0x9c, game)
    g.word(game + 0x55e0, game + 0x55ec)
    g.emu.mem_write(obj + 0x16d, b'\xa5')
    count = 0
    names = [(f'v021d_level_0{level}/day_{mode}', True)
             for level in (1, 2, 3, 5, 6) for mode in (2, 4, 8)]
    names += [(name, False) for name in ('', 'v021d', 'level_01/day_2',
              'v021d_level_04/day_2', 'v021d_level_01/rain_2',
              'v021d_level_01/day_20', 'v021d_level_01/day_2/extra')]
    for name, legacy in names:
        g.emu.mem_write(game + 0x55ec, name.encode() + b'\0')
        for scene in (world, lobby):
            g.word(obj + 0x108, scene)
            for intensity in (0.0, 0.5, 1.0, 1.0001, 2.7, 3.9):
                sp = STACK - 0x200
                before = bytes([0x6b]) * 0x100
                g.emu.mem_write(sp, before)
                rgb = (0.13, 0.29, 0.71)
                g.emu.mem_write(sp + 0x6c, struct.pack('<4f', *rgb, intensity))
                regs = {UC_X86_REG_EAX: 0x12345678, UC_X86_REG_EBX: obj,
                        UC_X86_REG_ECX: 0x23456789, UC_X86_REG_EDX: 0x3456789a,
                        UC_X86_REG_ESI: 0x456789ab, UC_X86_REG_EDI: 0x56789abc,
                        UC_X86_REG_EBP: 0x6789abcd, UC_X86_REG_ESP: sp,
                        UC_X86_REG_EFLAGS: 0x246,
                        UC_X86_REG_XMM0: 0x123456789abcdef00123456789abcdef,
                        UC_X86_REG_XMM7: 0x76543210fedcba9876543210fedcba98}
                for reg, value in regs.items(): g.emu.reg_write(reg, value)
                g.emu.emu_start(0x290a3f, 0x290a45, count=10000)
                assert g.emu.reg_read(UC_X86_REG_EIP) == 0x290a45
                for reg, value in regs.items():
                    expected = 0x123456a5 if reg == UC_X86_REG_EAX else value
                    assert g.emu.reg_read(reg) == expected, (name, reg)
                actual = struct.unpack('<4f', g.emu.mem_read(sp + 0x6c, 16))
                converted = legacy and scene == world and intensity > 1
                if converted:
                    # The original 0.21 renderer's uploaded linear product,
                    # represented in 0.26's multiplicative pass (intensity 1).
                    expected = tuple(c * intensity for c in rgb) + (1.0,)
                else:
                    expected = rgb + (intensity,)
                assert all(math.isclose(a, b, abs_tol=1e-6) for a, b in zip(actual, expected)), (name, actual, expected)
                assert bytes(g.emu.mem_read(sp, 0x6c)) == before[:0x6c]
                assert bytes(g.emu.mem_read(sp + 0x7c, 0x84)) == before[0x7c:]
                count += 1
    ambient_count = count
    engine, particle = g.block(0x300), g.block(0x10)
    render, other_render = g.block(0x100), g.block(0x100)
    # Reuse one game through load, lobby, replacement-scene and teardown
    # states, so a stale map name alone can never select the original gain.
    states = (
        ('world', engine, game, render, render, True),
        ('other scene', engine, game, render, other_render, False),
        ('old scene after replacement', engine, game, other_render, render, False),
        ('scene removed', engine, game, 0, render, False),
        ('particle scene removed', engine, game, render, 0, False),
        ('both scenes removed', engine, game, 0, 0, False),
        ('game removed', engine, 0, render, render, False),
        ('engine removed', 0, game, render, render, False),
    )
    for name, legacy in names + [('v021d_level_01/day_2', True), ('level_01/day_2', False)]:
        g.emu.mem_write(game + 0x55ec, name.encode() + b'\0')
        for state, engine_ptr, game_ptr, active_scene, particle_scene, own_world in states:
            g.word(0x1f1d310, engine_ptr)
            g.word(engine + 0x2d0, game_ptr)
            g.word(world + 4, active_scene)
            g.word(particle + 8, particle_scene)
            for intensity in (0.0, -1.0, 0.5, 1.0, 1.25, 8.0):
                sp = STACK - 0x200
                before = bytearray([0x6b]) * 0x100
                before[0x64:0x68] = struct.pack('<f', intensity)
                g.emu.mem_write(sp, bytes(before))
                low = int.from_bytes(struct.pack('<f', intensity), 'little')
                upper = 0x123456789abcdef001234567 << 32
                regs = {UC_X86_REG_EAX: 0x12345678, UC_X86_REG_EBX: particle,
                        UC_X86_REG_ECX: 0, UC_X86_REG_EDX: 0x3456789a,
                        UC_X86_REG_ESI: 0x456789ab, UC_X86_REG_EDI: 0x56789abc,
                        UC_X86_REG_EBP: sp, UC_X86_REG_ESP: sp,
                        UC_X86_REG_EFLAGS: 0x246, UC_X86_REG_MXCSR: 0x1f80,
                        UC_X86_REG_XMM0: upper | low,
                        UC_X86_REG_XMM7: 0x76543210fedcba9876543210fedcba98}
                for reg, value in regs.items(): g.emu.reg_write(reg, value)
                g.emu.emu_start(0x14f305, 0x14f312, count=10000)
                assert g.emu.reg_read(UC_X86_REG_EIP) == 0x14f312
                expected = intensity if legacy and own_world else intensity * 3
                expected_bits = int.from_bytes(struct.pack('<f', expected), 'little')
                for reg, value in regs.items():
                    want = expected_bits if reg == UC_X86_REG_XMM0 else value
                    assert g.emu.reg_read(reg) == want, (name, state, intensity, reg)
                assert bytes(g.emu.mem_read(sp, 0x100)) == before
                count += 1
    if output := os.environ.get('SKHID_AMBIENT_PREVIEW_EXE'):
        Path(output).write_bytes(image)
    print(f'PASS: {ambient_count} ambient and {count - ambient_count} particle hook cases; '
          '0.21 products/intensity, actual-scene isolation, load/teardown transitions, '
          'native gain, registers and frame preserved')


if __name__ == '__main__': main()
