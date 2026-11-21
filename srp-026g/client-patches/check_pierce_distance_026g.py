#!/usr/bin/env python3
"""Patch 173 (PIERCE_DISTANCE): a bullet's armour penetration does not fall with distance.

python check_pierce_distance_026g.py ORIGINAL

Runs the game's update_bullet_position (0x32EE90, entry to its `ret 8`) on a made-up bullet,
weapon and round in a level's bounds (check_bullet_bounds_026g's world), in the original and in
the build, for several weapon and round floors: the damage +70h falls by speed bit for bit as the
original's; the piercing +74h falls in the original and is kept in the build, at any speed and below
the falloff too (after a wall).
"""
import struct
import sys

from unicorn.x86_const import UC_X86_REG_EAX
import patch_026g as base
from check_bullet_bounds_026g import Game, LEVEL, HEAP, BULLET, AMMO, ACCEL

UPDATE = 0x32EE90
ITEM = HEAP + 0x20000                     # the weapon item; the weapon is item - 10h (0x32F091)
WEAPON = ITEM - 0x10
D, P = 0.5, 0.4                           # bullet_damage +51Ch, bullet_pierce +520h
# A weapon's floor is its min_damage_amount (+534h) x its round's k_min_damage_amount (ammo +20h):
# the weapon table's usual 0.6, its lowest and highest, and rounds that raise it.
FLOORS = ((0.6, 1.0), (0.35, 1.0), (0.8, 1.0), (0.6, 1.5), (0.5, 1.2))


def f32(x): return struct.unpack('<f', struct.pack('<f', x))[0]


def step(data, floor, damage, pierce, fast, slow):
    """One move of 0.5 s (the speed a little under 1 m/s) of a bullet whose falloff runs from
    `fast` to `slow` m/s: (its damage, its piercing) after it."""
    game = Game(data)
    game.add(8, LEVEL)
    u, (m, k) = game.u, floor
    u.mem_write(WEAPON + 0x51C, struct.pack('<2f', D, P))
    u.mem_write(WEAPON + 0x534, struct.pack('<f', m))
    u.mem_write(AMMO + 0x08, struct.pack('<f', 1.0))          # k_damage
    u.mem_write(AMMO + 0x20, struct.pack('<2f', k, 1.0))      # k_min_damage_amount, k_arp
    game.word(BULLET + 0x0C, ITEM)
    for at in (0x10, 0x28):
        u.mem_write(BULLET + at, struct.pack('<3f', 0., 0., 0.))
    game.word(BULLET + 0x68, 0)
    u.mem_write(BULLET + 0x70, struct.pack('<4f', damage, pierce, fast, slow))
    game.call(UPDATE, (struct.unpack('<I', struct.pack('<f', 0.5))[0], ACCEL), {UC_X86_REG_EAX: BULLET})
    assert u.reg_read(UC_X86_REG_EAX) & 0xFF == 1, "the bullet goes on"
    return struct.unpack('<2f', u.mem_read(BULLET + 0x70, 8))


def main():
    original = open(sys.argv[1], 'rb').read()
    code = open(base.default_bin('section'), 'rb').read()
    built, _ = base.build(original, 'section', code, None, open(base.late_join_bin(), 'rb').read())
    va, old, new = base.PIERCE_DISTANCE
    at = base.file_offset(va)
    assert original[at:at + len(old)] == old, f'{va:#x}: the original'
    assert built[at:at + len(old)] == new, f'{va:#x}: the build'

    near = lambda a, b: abs(a - b) < 1e-6
    for m, k in FLOORS:
        dmin, pmin = D * m * k, P * m * k
        for name, fast, slow in (('within the falloff', 1.5, 0.5), ('past it', 3.0, 2.0), ('before it', 0.5, 0.1)):
            for damage, pierce in ((D, P), (D, 0.1), (0.15, P)):      # from the muzzle; after walls
                d0, p0 = step(original, (m, k), damage, pierce, fast, slow)
                d1, p1 = step(built, (m, k), damage, pierce, fast, slow)
                what = f'floor {m} x {k}, {name}, D {damage} P {pierce}'
                assert struct.pack('<f', d0) == struct.pack('<f', d1), f'{what}: damage {d0} -> {d1}'
                assert p1 == f32(pierce), f'{what}: the build changed piercing to {p1}'
                if name == 'within the falloff' and damage == D:
                    assert dmin < d0 < D, f'{what}: the original\'s damage {d0}'
                    u = (d0 - dmin) / (D - dmin)
                    assert near(p0, min(pierce, pmin + (P - pmin) * u)), f'{what}: the original\'s {p0} at u {u}'
                    assert pierce > pmin + (P - pmin) * u or p0 == f32(pierce)
                if name == 'past it':
                    assert near(d0, min(damage, dmin)) and near(p0, min(pierce, pmin)), f'{what}: {d0}, {p0}'
                if name == 'before it':
                    # u = 1: the original's min + (max - min) x 1 can round a step below max
                    assert near(d0, damage) and near(p0, pierce), f'{what}: {d0}, {p0}'
    print(f'ok: update_bullet_position, {len(FLOORS)} weapon and round floors: damage falls as the '
          'original\'s, piercing kept at every speed')


if __name__ == '__main__':
    main()
