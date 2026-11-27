#!/usr/bin/env python3
"""Patch 175 (PIERCE_WALLS): every wall costs a bullet something, k = 1 - (A / P)^2.

python check_pierce_walls_026g.py ORIGINAL

Runs the game's try_pierce (0x32D960) from its entry on a made-up bullet against static geometry
(no receiver) of a made-up material, in the original and in the build, for walls from glass (A 0)
to ground (A 20) and piercing from 0 up: a bullet the wall stops returns 0 (`ret 10h`) in both; one
it lets through is run to just after the stores of its damage +70h and piercing +74h (0x32DAA6),
both k times what they were, k the original's 1 - (1 - clamp((P - A) / A, 0, 1))^2 and the build's
1 - (A / P)^2, in single precision as the game computes them.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ESP, UC_X86_REG_EIP
import patch_026g as base

TRY_PIERCE, STORED = 0x32D960, 0x32DAA6
HEAP, STACK, STOP = 0x30000000, 0x30100000, 0x30200000
BULLET, MATERIAL = HEAP, HEAP + 0x1000
D = 0.5
WALLS = (0.0, 0.08, 0.1, 0.12, 0.2, 0.23, 0.7, 2.0, 20.0)   # glass .. ground, game.materials' armor
PIERCES = (0.0, 0.05, 0.1, 0.2, 0.23, 0.3, 0.4, 0.46, 0.5, 0.6, 0.84, 1.0, 1.4, 2.0)


def f32(x): return struct.unpack('<f', struct.pack('<f', x))[0]


def k_original(p, a):
    p, a = f32(p), f32(a)
    t = 1.0 if a == 0 else min(1.0, max(0.0, f32(f32(p - a) / a)))
    return f32(1 - f32(f32(1 - t) * f32(1 - t)))


def k_built(p, a):
    p, a = f32(p), f32(a)
    if a == 0:
        return 1.0
    q = f32(a / p) if p else float('inf')
    return f32(1 - f32(q * q))


class Game:
    def __init__(self, data):
        self.u = u = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vs, raw, size, _ in base.Headers(data).sections(data):
            u.mem_map(base.IMAGE_BASE + rva, (max(vs, size) + 4095) & ~4095)
            u.mem_write(base.IMAGE_BASE + rva, bytes(data[raw:raw + min(size, vs or size)]))
        u.mem_map(HEAP, 0x10000)
        u.mem_map(STACK, 0x10000)
        u.mem_map(STOP, 0x1000)
        u.hook_add(UC_HOOK_CODE, lambda u, *_: u.emu_stop(), begin=STORED, end=STORED)

    def pierce(self, p, a):
        """(pierced, damage, piercing) after try_pierce of a bullet (D, p) at a wall of armour a."""
        u = self.u
        u.mem_write(BULLET + 0x4C, struct.pack('<I', MATERIAL))
        u.mem_write(BULLET + 0x70, struct.pack('<2f', D, p))
        u.mem_write(MATERIAL + 0x4C, struct.pack('<f', a))
        sp = STACK + 0x8000
        u.mem_write(sp, struct.pack('<5I', STOP, BULLET, HEAP + 0x2000, HEAP + 0x2000, 0))
        u.reg_write(UC_X86_REG_ESP, sp)
        u.reg_write(UC_X86_REG_EAX, 0)                              # static geometry: no receiver
        u.emu_start(TRY_PIERCE, STOP, count=1000)
        damage, pierce = struct.unpack('<2f', u.mem_read(BULLET + 0x70, 8))
        if u.reg_read(UC_X86_REG_EIP) == STOP:
            assert u.reg_read(UC_X86_REG_ESP) == sp + 4 + 0x10 and u.reg_read(UC_X86_REG_EAX) & 0xFF == 0
            assert (damage, pierce) == (f32(D), f32(p)), 'a stopped bullet keeps its numbers'
            return False, damage, pierce
        assert u.reg_read(UC_X86_REG_EIP) == STORED
        return True, damage, pierce


def main():
    original = open(sys.argv[1], 'rb').read()
    code = open(base.default_bin('section'), 'rb').read()
    built, _ = base.build(original, 'section', code, None, open(base.late_join_bin(), 'rb').read())
    va, old, new = base.PIERCE_WALLS
    at = base.file_offset(va)
    assert original[at:at + len(old)] == old, f'{va:#x}: the original'
    assert built[at:at + len(old)] == new, f'{va:#x}: the build'

    games = Game(original), Game(built)
    for a in WALLS:
        for p in PIERCES:
            for game, k_of in zip(games, (k_original, k_built)):
                k = k_of(p, a)
                what = f'{"build" if k_of is k_built else "original"}, P {p} A {a}'
                pierced, damage, pierce = game.pierce(p, a)
                assert pierced == (k > 0) == (a == 0 or p > a), f'{what}: pierced {pierced}, k {k}'
                if pierced:
                    assert (damage, pierce) == (f32(k * f32(D)), f32(k * f32(p))), f'{what}: {damage}, {pierce} (k {k})'
    # The new rule: no wall but A = 0 is free; through brick and concrete at twice their armour and more.
    assert all(k_built(p, a) < 1 for a in WALLS if a for p in PIERCES if p)
    assert k_original(0.46, 0.23) == 1.0 and abs(k_built(0.46, 0.23) - 0.75) < 1e-6
    print(f'ok: try_pierce, {len(WALLS)} walls x {len(PIERCES)} piercings: the original\'s k = 1 - (1 - t)^2, '
          'the build\'s 1 - (A / P)^2, the same bullets stopped')


if __name__ == '__main__':
    main()
