#!/usr/bin/env python3
"""Run patch 134 (the Rust patch `animation_math` and animation_math_026g.asm)
in the default build and the original exe, on the same inputs, and require
the same results.

    python check_animation_math_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are and the
game's own functions run whole, from their entry to a sentinel return:

- quaternion::quaternion(float3 angles) (0xED2E0; ESI the quaternion, the
  three angles on the stack, `ret 0Ch`): its six sin/cos calls are the
  patch's; 20000 angle triples (the animation's range, wider ones, 0, -0,
  multiples of pi, a NaN, huge ones), the four floats written compared bit
  for bit;
- quaternion::quaternion(axis, angle) (0x74290; ESI out, EDI the axis, the
  angle on the stack, `ret 4`): 5000 angles;
- quaternion::get_axis_and_angle (0x74300; ESI the quaternion, EDI the axis
  out, the angle's place on the stack, `ret 4`): 5000 quaternions, unit
  ones and near-identity ones, AL, the axis and the angle compared;
- time_channel::domain (0x3B3340; EAX the channel, XMM2 the time, the
  current domain's place on the stack, `ret 4`): 30000 made-up channels
  (1 to 40 knots, sorted, with equal knots, unsorted; whatever lies after
  the knots), times inside, outside, on a knot and NaN, current domains
  inside, past the count and near 2^32 (where current + i wraps): EAX, EDX,
  ECX, XMM0, XMM1, XMM2, the stored domain, EBX ESI EDI EBP and ESP compared;
  a search that never ends (an unsorted channel can have no domain) must
  not end in either;
- evaluate_frame (0x3B3180, the only caller of domain; EAX the values, ECX
  the channels, the time, the current domains and the count on the stack):
  a bone's nine channels, 2000 times, the values and current domains
  compared.

The x87's sin, cos and atan2 here are Unicorn's (QEMU's), not a CPU's: what
this shows is that the patched calls give what the original's give through
the same callers, both ways (fast or the game's own function). That the fast
answers are a real x87's is vostok::trig's tests (every float, on the CPU
they run on). The instructions each run took are printed (domain: no `div`
per step).
"""
import math
import os
import random
import struct
import sys

from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_XMM0, \
    UC_X86_REG_XMM1, UC_X86_REG_XMM2

if len(sys.argv) > 1:
    os.environ.setdefault("SRP_026G_EXE", sys.argv[1])
import patch_026g as base

QUATERNION_ANGLES, QUATERNION_AXIS, AXIS_AND_ANGLE = 0xED2E0, 0x74290, 0x74300
DOMAIN, EVALUATE_FRAME = 0x3B3340, 0x3B3180
DATA, STACK, SENTINEL = 0x20000000, 0x21000000, 0x22000000
OUT, AXIS, ANGLE, CURRENT, CHANNEL = DATA, DATA + 0x100, DATA + 0x200, DATA + 0x300, DATA + 0x1000
GAME = {UC_X86_REG_EBX: 0x11111111, UC_X86_REG_EBP: 0x22222222, UC_X86_REG_ESI: 0x33333333,
        UC_X86_REG_EDI: 0x44444444}
LIMIT = 20000                   # instructions: a found domain takes under 2n tries of 9
F = struct.Struct("<f")


def f32(x):
    return F.unpack(F.pack(x))[0]


def bits(x):
    return struct.unpack("<I", F.pack(x))[0]


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

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at, n=4):
        return bytes(self.emu.mem_read(at, n))

    def call(self, start, regs, args):
        """`start` called with `regs` and the dwords `args` on the stack,
        run to its return: (returned, how far ESP moved, the registers it
        must keep); returned is False when it did not return in LIMIT
        instructions, the error's text when it faulted."""
        emu = self.emu
        sp = STACK + 0x80000 - 4 * (len(args) + 1)
        self.word(sp, SENTINEL)
        for i, a in enumerate(args):
            self.word(sp + 4 + 4 * i, a)
        for reg, value in {**GAME, **regs}.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, sp)
        emu.reg_write(UC_X86_REG_EAX, regs.get(UC_X86_REG_EAX, 0x77777777))
        emu.reg_write(UC_X86_REG_EDX, 0x55555555)
        emu.reg_write(UC_X86_REG_ECX, regs.get(UC_X86_REG_ECX, 0x66666666))
        try:
            emu.emu_start(start, SENTINEL, count=LIMIT)
        except UcError as error:
            return f"fault: {error}", None, None
        if emu.reg_read(UC_X86_REG_EIP) != SENTINEL:
            return False, None, None
        kept = {reg: emu.reg_read(reg) for reg in GAME if reg not in regs}
        return True, emu.reg_read(UC_X86_REG_ESP) - sp, kept


def angles_case(run, x, y, z):
    run.emu.mem_write(OUT, bytes(16))
    done, moved, kept = run.call(QUATERNION_ANGLES, {UC_X86_REG_ESI: OUT}, [bits(x), bits(y), bits(z)])
    return (done, moved, kept, run.read(OUT, 16))


def axis_case(run, axis, angle):
    run.emu.mem_write(OUT, bytes(16))
    run.emu.mem_write(AXIS, b"".join(F.pack(c) for c in axis))
    done, moved, kept = run.call(QUATERNION_AXIS, {UC_X86_REG_ESI: OUT, UC_X86_REG_EDI: AXIS}, [bits(angle)])
    return (done, moved, kept, run.read(OUT, 16), run.emu.reg_read(UC_X86_REG_EAX))


def axis_angle_case(run, q):
    run.emu.mem_write(OUT, b"".join(F.pack(c) for c in q))
    run.emu.mem_write(AXIS, bytes(12))
    run.word(ANGLE, 0xDEADBEEF)
    done, moved, kept = run.call(AXIS_AND_ANGLE, {UC_X86_REG_ESI: OUT, UC_X86_REG_EDI: AXIS}, [ANGLE])
    return (done, moved, kept, run.emu.reg_read(UC_X86_REG_EAX) & 0xFF, run.read(AXIS, 12), run.read(ANGLE))


def write_channel(run, at, knots, after, offset):
    """A time_channel<float4> at `at`: {n, offset}; at `at` + offset n domains
    of 16 bytes {c0, c1, c2, c3}, then n knots, then `after` (what lies
    beyond the last knot)."""
    n = len(knots)
    run.emu.mem_write(at, struct.pack("<II", n, offset))
    domains = b"".join(struct.pack("<4f", i, 0.25, -0.5, 0.125) for i in range(n))
    run.emu.mem_write(at + offset, domains + b"".join(F.pack(k) for k in knots)
                      + b"".join(F.pack(a) for a in after))


def domain_case(run, knots, after, offset, t, current):
    emu = run.emu
    emu.mem_write(CHANNEL, b"\xAB" * 0x400)
    write_channel(run, CHANNEL, knots, after, offset)
    run.word(CURRENT, current)
    emu.reg_write(UC_X86_REG_XMM2, bits(t) | (0x0BADF00D << 32))
    emu.reg_write(UC_X86_REG_XMM0, 0x1234)
    emu.reg_write(UC_X86_REG_XMM1, 0x5678)
    run.steps = 0
    done, moved, kept = run.call(DOMAIN, {UC_X86_REG_EAX: CHANNEL}, [CURRENT])
    steps = run.steps
    if done is not True:
        return ("no end" if done is False else done), steps
    return (moved, kept, emu.reg_read(UC_X86_REG_EAX), emu.reg_read(UC_X86_REG_EDX),
            emu.reg_read(UC_X86_REG_ECX), emu.reg_read(UC_X86_REG_XMM0) & 0xFFFFFFFF,
            emu.reg_read(UC_X86_REG_XMM1) & 0xFFFFFFFF, emu.reg_read(UC_X86_REG_XMM2),
            run.read(CURRENT)), steps


def evaluate_case(run, channels, t, currents):
    """evaluate_frame on `channels` (each (knots, offset)): the 9 values, the
    current domains after."""
    emu = run.emu
    table = DATA + 0x8000
    for i, (knots, offset) in enumerate(channels):
        write_channel(run, table + 8 * i, knots, [9e9], offset - 8 * i)
    for i, c in enumerate(currents):
        run.word(CURRENT + 4 * i, c)
    emu.mem_write(OUT, bytes(64))
    done, moved, kept = run.call(EVALUATE_FRAME, {UC_X86_REG_EAX: OUT, UC_X86_REG_ECX: table},
                                 [bits(t), CURRENT, len(channels)])
    return (done, moved, kept, run.read(OUT, 4 * len(channels)), run.read(CURRENT, 4 * len(channels)))


def angles(rng):
    pick = rng.random()
    if pick < 0.6:
        return f32(rng.uniform(-math.pi, math.pi))
    if pick < 0.8:
        return f32(rng.uniform(-200.0, 200.0))
    if pick < 0.85:
        return f32(rng.choice([-1, 1]) * rng.randint(1, 40) * math.pi / 2)
    if pick < 0.9:
        return f32(rng.uniform(-1e-5, 1e-5))
    return rng.choice([0.0, -0.0, 1e-30, -1e-30, float("nan"), 1e30, -1e30, 64.0, -64.0, 64.00001])


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    stock, patched = Run(original), Run(bytes(built))
    rng = random.Random(134)
    failed = 0

    def check(what, count, make, case):
        nonlocal failed
        bad, steps = [], [0, 0]
        for _ in range(count):
            args = make()
            stock.steps = patched.steps = 0
            a, b = case(stock, *args), case(patched, *args)
            steps[0] += stock.steps
            steps[1] += patched.steps
            if a != b or a[0] is not True:
                bad.append((args, a, b))
        failed += bool(bad)
        print(f"  {'ok' if not bad else 'FAIL'}  {what}: {count} cases, {len(bad)} differ; "
              f"instructions {steps[0]} original, {steps[1]} patched")
        for args, a, b in bad[:5]:
            print(f"        {args}: original {a}, patched {b}")

    check("quaternion(angles) 0xED2E0, its six sin/cos", 20000,
          lambda: (angles(rng), angles(rng), angles(rng)), angles_case)

    def axis():
        v = [rng.uniform(-1, 1) for _ in range(3)]
        n = math.sqrt(sum(c * c for c in v)) or 1.0
        return [f32(c / n) for c in v], angles(rng)
    check("quaternion(axis, angle) 0x74290, its sin and cos", 5000, axis, axis_case)

    def quaternion():
        pick = rng.random()
        if pick < 0.7:
            q = [rng.uniform(-1, 1) for _ in range(4)]
        elif pick < 0.9:
            q = [rng.uniform(-1e-4, 1e-4) for _ in range(3)] + [rng.choice([-1, 1])]
        else:
            q = [rng.uniform(-1, 1) for _ in range(3)] + [0.0]
        n = math.sqrt(sum(c * c for c in q)) or 1.0
        return ([f32(c / n) for c in q],)
    check("get_axis_and_angle 0x74300, its atan2", 5000, quaternion, axis_angle_case)

    # time_channel::domain, alone
    bad, steps, ended = [], [0, 0], 0
    for i in range(30000):
        n = rng.choice([1, 2, 2, 3, 4, 5, 8, 13, 21, 40])
        kind = rng.random()
        knots = sorted(f32(rng.uniform(0, 100)) for _ in range(n))
        if kind < 0.15 and n > 2:
            k = rng.randrange(n - 1)
            knots[k + 1] = knots[k]                      # equal knots
        elif kind < 0.25:
            rng.shuffle(knots)                           # unsorted (bad data)
        after = [rng.choice([f32(rng.uniform(-10, 110)), float("nan"), 1e30, -1e30])]
        offset = 8 + 4 * rng.randrange(0, 8)
        t = rng.choice([f32(rng.uniform(-5, 105)), rng.choice(knots), float("nan"), -1e30, 1e30,
                        knots[0], knots[-1]])
        current = rng.choice([0, rng.randrange(n), rng.randrange(n + 5), n, 0xFFFFFFFF - rng.randrange(50),
                              rng.randrange(1 << 32)])
        args = (knots, after, offset, t, current)
        a, sa = domain_case(stock, *args)
        b, sb = domain_case(patched, *args)
        if isinstance(a, tuple):                         # a search that ended: its instructions
            steps[0] += sa
            steps[1] += sb
            ended += 1
        same = a == b and not str(a).startswith("fault")
        if not same:
            bad.append((args, a, b))
    failed += bool(bad)
    print(f"  {'ok' if not bad else 'FAIL'}  time_channel::domain 0x3B3340: 30000 cases ({ended} end, the rest "
          f"search forever in both), {len(bad)} differ; instructions of those that end {steps[0]} original, "
          f"{steps[1]} patched")
    for args, a, b in bad[:5]:
        print(f"        {args}: original {a}, patched {b}")

    def frame():
        channels = []
        for i in range(9):
            n = rng.choice([2, 2, 3, 5, 9, 30])
            channels.append((sorted(f32(rng.uniform(0, 60)) for _ in range(n)), 0x400 + 0x400 * i))
        t = f32(rng.uniform(-2, 62))
        currents = [rng.choice([0, rng.randrange(len(k)), rng.randrange(len(k) + 3)]) for k, _ in channels]
        return channels, t, currents
    check("evaluate_frame 0x3B3180, a bone's nine channels", 2000, frame, evaluate_case)

    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
