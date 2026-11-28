#!/usr/bin/env python3
"""Run patch 221 (box_transform_026g.asm, a code hook at aabb::modify) in the
default build and the original exe, on the same boxes and transforms, and
require the same bytes.

    python check_box_transform_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are and
aabb::modify (0xF1FD0; EAX the box, ECX the transform, a plain `ret`) runs
whole in both, from its entry to a sentinel return, on:

- rotations (yaw, pitch, roll), some scaled or mirrored, with translations,
  some of their terms 0 or -0, and boxes around them, some flat;
- transforms and boxes of every float kind (0, -0, infinities, NaNs of
  several payloads, denormals, the largest float), also inverted boxes;

each also with the MXCSR's FTZ and DAZ set as the game may run. The box's 24
bytes, EAX, ECX and EDX (the signs the game's last two tests leave), EBX,
ESI, EDI, EBP and how far ESP moved compared (XMM0..7 and the flags the
game's changes all of). The instructions each run took are printed.
"""
import math
import os
import random
import struct
import sys

from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_MXCSR

if len(sys.argv) > 1:
    os.environ.setdefault("SRP_026G_EXE", sys.argv[1])
import patch_026g as base

MODIFY = 0xF1FD0
DATA, STACK, SENTINEL = 0x20000000, 0x21000000, 0x22000000
BOX, MATRIX = DATA, DATA + 0x100
GAME = {UC_X86_REG_EBX: 0x11111111, UC_X86_REG_EBP: 0x22222222, UC_X86_REG_ESI: 0x33333333,
        UC_X86_REG_EDI: 0x44444444}
KEPT = (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, UC_X86_REG_EBP, UC_X86_REG_ESI,
        UC_X86_REG_EDI)
LIMIT = 5000
PLAIN, FLUSH = 0x1F80, 0x1F80 | 0x8040      # MXCSR: exceptions masked; with DAZ and FTZ
SPECIAL = [0.0, -0.0, math.inf, -math.inf, 1.0, -1.0, 3.4028234663852886e38, 1e-45, -1e-45, 1.1754943508222875e-38,
           -2.9e-39]
NANS = (0x7FC00000, 0xFFC00001, 0x7F800001, 0xFFBFFFFF)


def bits(x):
    return struct.unpack("<I", struct.pack("<f", x))[0]


def f32(x):
    return struct.unpack("<f", struct.pack("<f", x))[0]


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at in (DATA, STACK, SENTINEL):
            emu.mem_map(at, 0x100000, UC_PROT_ALL)
        self.steps = 0
        emu.hook_add(UC_HOOK_CODE, self._step)

    def _step(self, *_):
        self.steps += 1

    def case(self, box, matrix, mxcsr):
        emu = self.emu
        emu.mem_write(BOX, box)
        emu.mem_write(MATRIX, matrix)
        sp = STACK + 0x80000 - 4
        emu.mem_write(sp, struct.pack("<I", SENTINEL))
        for reg, value in {**GAME, UC_X86_REG_EAX: BOX, UC_X86_REG_ECX: MATRIX, UC_X86_REG_EDX: 0x55555555}.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, sp)
        emu.reg_write(UC_X86_REG_MXCSR, mxcsr)
        try:
            emu.emu_start(MODIFY, SENTINEL, count=LIMIT)
        except UcError as error:
            return f"fault: {error}"
        if emu.reg_read(UC_X86_REG_EIP) != SENTINEL:
            return "no end"
        return (tuple(emu.reg_read(r) for r in KEPT) + (emu.reg_read(UC_X86_REG_ESP) - sp,),
                bytes(emu.mem_read(BOX, 24)), bytes(emu.mem_read(MATRIX, 64)))


def odd(rng):
    pick = rng.random()
    if pick < 0.25:
        return bits(rng.choice(SPECIAL))
    if pick < 0.3:
        return rng.choice(NANS)
    if pick < 0.42:
        return rng.getrandbits(32)
    return bits(f32(rng.uniform(-500, 500)))


def rotation(rng):
    a, b, c = (rng.uniform(-math.pi, math.pi) for _ in range(3))
    if rng.random() < 0.3:
        b = c = 0.0
    ca, sa, cb, sb, cc, sc = math.cos(a), math.sin(a), math.cos(b), math.sin(b), math.cos(c), math.sin(c)
    rows = [[ca * cc + sa * sb * sc, cb * sc, -sa * cc + ca * sb * sc],
            [-ca * sc + sa * sb * cc, cb * cc, sa * sc + ca * sb * cc],
            [sa * cb, -sb, ca * cb]]
    scale = rng.choice([1.0, 1.0, 1.0, rng.uniform(0.01, 3), -1.0])
    m = []
    for r in rows:
        m += [f32(x * scale) for x in r] + [0.0]
    for _ in range(rng.choice([0, 0, 1, 2])):
        i = rng.randrange(12)
        if i % 4 != 3:
            m[i] = rng.choice([0.0, -0.0])
    m += [f32(rng.uniform(-800, 800)), f32(rng.uniform(-60, 60)), f32(rng.uniform(-800, 800)), 1.0]
    return struct.pack("<16f", *m)


def near_box(rng):
    c = [rng.uniform(-50, 50) for _ in range(3)]
    h = [rng.uniform(0.01, 30) for _ in range(3)]
    if rng.random() < 0.1:
        h[rng.randrange(3)] = 0.0
    return struct.pack("<6f", *([f32(c[k] - h[k]) for k in range(3)] + [f32(c[k] + h[k]) for k in range(3)]))


def odd_box(rng):
    b = [odd(rng) for _ in range(6)]
    if rng.random() < 0.5:
        f = [struct.unpack("<f", struct.pack("<I", w))[0] for w in b]
        for k in range(3):
            if f[k] > f[k + 3]:
                b[k], b[k + 3] = b[k + 3], b[k]
    return struct.pack("<6I", *b)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    stock, patched = Run(original), Run(bytes(built))
    rng = random.Random(221)
    failed = 0

    def check(what, count, make):
        nonlocal failed
        bad, steps = [], [0, 0]
        for i in range(count):
            box, matrix = make()
            mxcsr = FLUSH if i % 4 == 3 else PLAIN
            stock.steps = patched.steps = 0
            a, b = stock.case(box, matrix, mxcsr), patched.case(box, matrix, mxcsr)
            steps[0] += stock.steps
            steps[1] += patched.steps
            if a != b or isinstance(a, str):
                bad.append((box.hex(), matrix.hex(), a, b))
        failed += bool(bad)
        print(f"  {'ok' if not bad else 'FAIL'}  {what}: {count} cases, {len(bad)} differ; "
              f"instructions {steps[0]} original, {steps[1]} patched")
        for args in bad[:5]:
            print(f"        {args}")

    check("rotations, scales and translations, boxes around them", 20000, lambda: (near_box(rng), rotation(rng)))
    check("transforms and boxes of every float kind", 20000,
          lambda: (odd_box(rng) if rng.random() < 0.7 else near_box(rng),
                   struct.pack("<16I", *(odd(rng) for _ in range(16))) if rng.random() < 0.7 else rotation(rng)))

    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
