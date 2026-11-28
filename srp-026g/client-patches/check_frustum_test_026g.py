#!/usr/bin/env python3
"""Run patch 220 (frustum_test_026g.asm, a code hook at cuboid::test_inexact)
in the default build and the original exe, on the same cuboids and boxes, and
require the same verdicts.

    python check_frustum_test_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are and
cuboid::test_inexact (0xF2BC0; EAX the cuboid, [esp+4] the box, `ret 4`) runs
whole in both, from its entry to a sentinel return:

- frusta the game makes (frustum::frustum 0xEF940 on made-up view-projection
  matrices, its planes normalized by aabb_plane::normalize 0x1072E0) and
  boxes in and around them;
- made-up planes of every float kind (0, -0, infinities, NaNs, denormals,
  the largest float) with the bits normalize writes or any under 40h, and
  boxes of every float kind, also inverted ones;
- a plane with bits of 40h or more (the game reads index triples past its
  table: both go through the game's own loop; a page read that is not mapped
  reads as zeros in both);

each also with the MXCSR's FTZ and DAZ set as the game may run. EAX, EBX,
ESI, EDI, EBP, how far ESP moved and the cuboid's and box's bytes compared
(ECX, EDX, XMM0..7 and the flags are what the game's leaves undefined to its
callers: it changes them all). The instructions each run took are printed.
"""
import math
import os
import random
import struct
import sys

from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE, UC_HOOK_MEM_UNMAPPED
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_MXCSR

if len(sys.argv) > 1:
    os.environ.setdefault("SRP_026G_EXE", sys.argv[1])
import patch_026g as base

TEST_INEXACT, FRUSTUM = 0xF2BC0, 0xEF940
DATA, STACK, SENTINEL = 0x20000000, 0x21000000, 0x22000000
CUBOID, BOX, MATRIX = DATA, DATA + 0x100, DATA + 0x200
GAME = {UC_X86_REG_EBX: 0x11111111, UC_X86_REG_EBP: 0x22222222, UC_X86_REG_ESI: 0x33333333,
        UC_X86_REG_EDI: 0x44444444}
KEPT = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI)
LIMIT = 20000
PLAIN, FLUSH = 0x1F80, 0x1F80 | 0x8040      # MXCSR: exceptions masked; with DAZ and FTZ
SPECIAL = [0.0, -0.0, math.inf, -math.inf, 1.0, -1.0, 3.4028234663852886e38, 1e-45, -1e-45, 1.1754943508222875e-38]
NANS = (0x7FC00000, 0xFFC00001, 0x7F800001)


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
        emu.hook_add(UC_HOOK_MEM_UNMAPPED, self._zeros)

    def _step(self, *_):
        self.steps += 1

    def _zeros(self, emu, access, address, size, value, data):
        page = address & ~0xFFF
        emu.mem_map(page, 0x1000 * (1 + ((address + size - 1) >> 12) - (address >> 12)), UC_PROT_ALL)
        return True

    def call(self, start, regs, args, mxcsr=PLAIN):
        emu = self.emu
        sp = STACK + 0x80000 - 4 * (len(args) + 1)
        emu.mem_write(sp, struct.pack(f"<{len(args) + 1}I", SENTINEL, *args))
        for reg, value in {**GAME, UC_X86_REG_ECX: 0x66666666, UC_X86_REG_EDX: 0x55555555, **regs}.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, sp)
        emu.reg_write(UC_X86_REG_MXCSR, mxcsr)
        try:
            emu.emu_start(start, SENTINEL, count=LIMIT)
        except UcError as error:
            return f"fault: {error}"
        if emu.reg_read(UC_X86_REG_EIP) != SENTINEL:
            return "no end"
        return tuple(emu.reg_read(r) for r in KEPT) + (emu.reg_read(UC_X86_REG_ESP) - sp,)


def frustum(run, rng):
    """The cuboid frustum::frustum makes of a made-up view-projection."""
    eye = [rng.uniform(-300, 300), rng.uniform(-5, 40), rng.uniform(-300, 300)]
    yaw, pitch = rng.uniform(0, 2 * math.pi), rng.uniform(-1.2, 1.2)
    f = [math.sin(yaw) * math.cos(pitch), math.sin(pitch), math.cos(yaw) * math.cos(pitch)]
    r = [math.cos(yaw), 0.0, -math.sin(yaw)]
    u = [f[1] * r[2] - f[2] * r[1], f[2] * r[0] - f[0] * r[2], f[0] * r[1] - f[1] * r[0]]
    dot = lambda a, b: sum(x * y for x, y in zip(a, b))
    view = [r[0], u[0], f[0], 0, r[1], u[1], f[1], 0, r[2], u[2], f[2], 0, -dot(r, eye), -dot(u, eye), -dot(f, eye), 1]
    ys = 1 / math.tan(rng.uniform(0.6, 1.5) / 2)
    xs, near, far = ys / rng.uniform(1.25, 2.4), rng.choice([0.05, 0.1, 0.5]), rng.uniform(50, 2000)
    proj = [xs, 0, 0, 0, 0, ys, 0, 0, 0, 0, far / (far - near), 1, 0, 0, -near * far / (far - near), 0]
    m = [sum(view[i * 4 + k] * proj[k * 4 + j] for k in range(4)) for i in range(4) for j in range(4)]
    if rng.random() < 0.5:
        m = [m[j * 4 + i] for i in range(4) for j in range(4)]
    run.emu.mem_write(MATRIX, struct.pack("<16f", *m))
    if run.call(FRUSTUM, {UC_X86_REG_EAX: MATRIX, UC_X86_REG_ESI: CUBOID}, []) is None:
        raise SystemExit("frustum::frustum did not run")
    return run.emu.mem_read(CUBOID, 0x78), eye


def near_box(rng, eye):
    c = [e + rng.uniform(-600, 600) * (0.1 if rng.random() < 0.2 else 1) for e in eye]
    s = rng.uniform(20, 500) if rng.random() < 0.15 else rng.uniform(0.05, 15)
    h = [s * rng.uniform(0.1, 1) for _ in range(3)]
    if rng.random() < 0.05:
        h[rng.randrange(3)] = 0.0
    return [f32(c[k] - h[k]) for k in range(3)] + [f32(c[k] + h[k]) for k in range(3)]


def odd(rng):
    pick = rng.random()
    if pick < 0.25:
        return bits(rng.choice(SPECIAL))
    if pick < 0.32:
        return rng.choice(NANS)
    if pick < 0.45:
        return rng.getrandbits(32)
    return bits(f32(rng.uniform(-200, 200)))


def odd_cuboid(rng):
    words = []
    for _ in range(6):
        plane = [odd(rng) for _ in range(4)]
        if rng.random() < 0.6:
            far = sum((((plane[k] >> 31) ^ 1) << k) for k in range(3))
            plane.append(far | (far ^ 7) << 3)
        else:
            plane.append(rng.randrange(64))
        words += plane
    return struct.pack("<30I", *words)


def odd_box(rng):
    b = [odd(rng) for _ in range(6)]
    if rng.random() < 0.5:
        f = [struct.unpack("<f", struct.pack("<I", w))[0] for w in b]
        for k in range(3):
            if f[k] > f[k + 3]:
                b[k], b[k + 3] = b[k + 3], b[k]
    return struct.pack("<6I", *b)


def case(run, cuboid, box, mxcsr):
    run.emu.mem_write(CUBOID, bytes(cuboid))
    run.emu.mem_write(BOX, bytes(box))
    result = run.call(TEST_INEXACT, {UC_X86_REG_EAX: CUBOID}, [BOX], mxcsr)
    return result, bytes(run.emu.mem_read(CUBOID, 0x78)), bytes(run.emu.mem_read(BOX, 24))


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    stock, patched = Run(original), Run(bytes(built))
    rng = random.Random(220)
    failed = 0

    def check(what, count, make):
        nonlocal failed
        bad, steps, verdicts = [], [0, 0], {}
        for i in range(count):
            cuboid, box = make()
            mxcsr = FLUSH if i % 4 == 3 else PLAIN
            stock.steps = patched.steps = 0
            a, b = case(stock, cuboid, box, mxcsr), case(patched, cuboid, box, mxcsr)
            steps[0] += stock.steps
            steps[1] += patched.steps
            if isinstance(a[0], tuple):
                verdicts[a[0][0]] = verdicts.get(a[0][0], 0) + 1
            if a != b or not isinstance(a[0], tuple):
                bad.append((cuboid.hex(), box.hex(), a[0], b[0]))
        failed += bool(bad)
        print(f"  {'ok' if not bad else 'FAIL'}  {what}: {count} cases (verdicts {dict(sorted(verdicts.items()))}), "
              f"{len(bad)} differ; instructions {steps[0]} original, {steps[1]} patched")
        for args in bad[:5]:
            print(f"        {args}")

    def game_frustum():
        cuboid, eye = frustum(stock, rng)
        return cuboid, struct.pack("<6f", *near_box(rng, eye))
    check("frusta of frustum::frustum, boxes around the eye", 12000, game_frustum)

    def made_up():
        if rng.random() < 0.3:
            cuboid, eye = frustum(stock, rng)
        else:
            cuboid, eye = odd_cuboid(rng), [0.0, 0.0, 0.0]
        return cuboid, (odd_box(rng) if rng.random() < 0.6 else struct.pack("<6f", *near_box(rng, eye)))
    check("made-up planes and boxes, every float kind", 12000, made_up)

    def wide_bits():
        cuboid, eye = frustum(stock, rng)
        words = list(struct.unpack("<30I", cuboid))
        words[5 * rng.randrange(6) + 4] = rng.choice([0x40, 0x47, 0x78, 0x1FF, rng.getrandbits(32) | 0x40])
        return struct.pack("<30I", *words), struct.pack("<6f", *near_box(rng, eye))
    check("a plane's bits of 40h or more (the game's loop)", 1500, wide_bits)

    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
