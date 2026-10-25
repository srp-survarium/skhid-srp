#!/usr/bin/env python3
"""Run the original's and patch 72's bullet_character_controller::step_down
(0x7E1B0) in free fall, with Unicorn (2.1.4).

    python check_fall_gravity_026g.py <original survarium.exe>

The exe is built as patch_026g.py builds it (with the late join); the build
without the late join keeps the original's step_down. step_down runs from
its entry to its `ret 1Ch` on a made-up controller (gravity 9.8 at +2C708h,
the tick 0.05 at +2C704h, the capsule's axis 1 at +1F0h, standing, no
landing callback) under a made-up player_step frame whose +8 is the step's
dt (player_step's argument, read through step_down's saved ebp). Stubs:
step_down_sweep_test_impl (0x7EA80) finds nothing (`xor al,al / ret 18h`:
the free fall) and setup_shape_dim (0x7F100) returns. The rule:
../docs/notes/fall_damage.md "Split ticks".

- whole ticks (dt 0.05, 40 of them): the build's velocity is the
  original's, bit for bit, at every tick (-0.49 m/s a tick);
- a tick split in two (dt 0.02 then 0.03, the floats updateActions gets for
  20 and 30 ms) or in five (5 x 0.01): the original adds a whole tick of
  gravity at each part, the build one tick in all (within float rounding);
- a dt above the tick (not reachable: a step ends at the tick) adds one
  tick;
- the velocity's other two parts are kept as they are; the stack, ebx, ebp,
  esi and edi as step_down leaves them.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_EDI, UC_X86_REG_EIP,
                               UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

STEP_DOWN, SWEEP, SHAPE_DIM = 0x7E1B0, 0x7EA80, 0x7F100
CONTROLLER, ARGS, FRAME, STACK, RET = 0x41000000, 0x41100000, 0x41110000, 0x41128000, 0x41130000
TICK, GRAVITY, AXIS, DIMS, CROUCH, CALLBACK, VELOCITY = 0x2C704, 0x2C708, 0x1F0, 0x1D0, 0x203, 0x2C748, 0x2C770
CONTROLLER_SIZE = 0x2C790                     # its allocation (0x356A66)


def f32(x):
    return struct.unpack("<f", struct.pack("<f", x))[0]


def ms(n):
    """The dt updateActions gets for a step of n ms (0x75640: n x 0.001f, as a float)."""
    return f32(n * f32(0.001))


class Controller:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        headers = base.Headers(data)
        for _, rva, vsize, raw, rsize, _ in headers.sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(CONTROLLER, (CONTROLLER_SIZE + 0xFFF) & ~0xFFF, UC_PROT_ALL)
        for at in (ARGS, FRAME, STACK - 0x8000, RET):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        emu.mem_write(SWEEP, bytes.fromhex("30c0c21800"))     # xor al,al / ret 18h: nothing below
        emu.mem_write(SHAPE_DIM, b"\xC3")
        self.float(CONTROLLER + TICK, 0.05)
        self.float(CONTROLLER + GRAVITY, 9.8)
        emu.mem_write(CONTROLLER + AXIS, struct.pack("<I", 1))
        for i, v in enumerate((0.4, 0.5, 0.4, 0.0)):
            self.float(CONTROLLER + DIMS + 4 * i, v)
        emu.mem_write(CONTROLLER + CROUCH - 3, bytes(6))       # +200h..+205h: no crouch, no jump
        emu.mem_write(CONTROLLER + CALLBACK, bytes(4))          # no landing callback
        self.velocity = (1.5, 0.0, -0.25)

    def float(self, at, value):
        self.emu.mem_write(at, struct.pack("<f", value))

    @property
    def velocity(self):
        return struct.unpack("<3f", self.emu.mem_read(CONTROLLER + VELOCITY, 12))

    @velocity.setter
    def velocity(self, v):
        self.emu.mem_write(CONTROLLER + VELOCITY, struct.pack("<4f", *v, 0.0))

    def step(self, dt):
        """One step_down of a player_step whose dt is `dt`."""
        emu = self.emu
        self.float(FRAME + 8, dt)
        # Its arguments (`ret 1Ch`): the controller, the step-up offset (0),
        # not slow (0), and four vectors (zeros).
        args = [CONTROLLER, 0, 0, ARGS, ARGS + 0x10, ARGS + 0x20, ARGS + 0x30]
        esp = STACK - 4 * len(args) - 4
        emu.mem_write(esp, struct.pack("<I", RET) + b"".join(struct.pack("<I", a) for a in args))
        emu.reg_write(UC_X86_REG_ESP, esp)
        emu.reg_write(UC_X86_REG_EBP, FRAME)
        emu.reg_write(UC_X86_REG_EBX, 0x1111)
        emu.reg_write(UC_X86_REG_ESI, 0x2222)
        emu.reg_write(UC_X86_REG_EDI, 0x3333)
        emu.emu_start(STEP_DOWN, RET, count=20000)
        assert emu.reg_read(UC_X86_REG_EIP) == RET, f"stopped at {emu.reg_read(UC_X86_REG_EIP):#x}"
        assert emu.reg_read(UC_X86_REG_ESP) == STACK, "step_down did not pop its 1Ch bytes"
        assert (emu.reg_read(UC_X86_REG_EBP), emu.reg_read(UC_X86_REG_EBX), emu.reg_read(UC_X86_REG_ESI),
                emu.reg_read(UC_X86_REG_EDI)) == (FRAME, 0x1111, 0x2222, 0x3333), "callee-saved registers"
        return self.velocity


def ticks(data, parts, count):
    """The velocity after `count` ticks, each split in `parts` (ms)."""
    body = Controller(data)
    out = []
    for _ in range(count):
        for n in parts:
            v = body.step(ms(n) if isinstance(n, int) else n)
        out.append(v)
    return out


def bits(v):
    return struct.pack("<3f", *v)


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    built, _ = base.build(original, "section", code, None, late_join)
    before, _ = base.build(original, "section", code, None, None)
    for va, old, new in base.FALL_GRAVITY:
        at = base.file_offset(va)
        assert original[at:at + len(old)] == old and before[at:at + len(old)] == old, f"{va:#x}: the original"
        assert built[at:at + len(old)] == new, f"{va:#x}: the build's bytes"

    g_tick = f32(f32(9.8) * f32(0.05))
    # Whole ticks: bit for bit the original's.
    stock, ours = ticks(original, [50], 40), ticks(built, [50], 40)
    assert [bits(v) for v in stock] == [bits(v) for v in ours], "whole ticks differ from the original"
    assert ticks(before, [50], 40) == stock
    assert abs(ours[-1][1] + 40 * 0.49) < 1e-3 and ours[-1][0] == 1.5 and ours[-1][2] == -0.25, ours[-1]
    assert ms(50) == f32(0.05)

    for parts in ([20, 30], [10] * 5, [7, 43]):
        stock, ours = ticks(original, parts, 20), ticks(built, parts, 20)
        whole = ticks(built, [50], 20)
        for n, (s, o, w) in enumerate(zip(stock, ours, whole), 1):
            assert abs(s[1] + n * len(parts) * g_tick) < 1e-3, (parts, n, s)     # k ticks of gravity a tick
            assert abs(o[1] - w[1]) < 1e-4 * n, (parts, n, o, w)                  # one tick in all
            assert (s[0], s[2], o[0], o[2]) == (1.5, -0.25, 1.5, -0.25)
    over = ticks(built, [0.07], 3)
    assert [bits(v) for v in over] == [bits(v) for v in ticks(built, [50], 3)], over

    print("patch 72: in free fall a whole tick adds 0.49 m/s to the fall, bit for bit as the original; a tick "
          "the world steps in parts (20+30, 5x10, 7+43 ms) adds 0.49 m/s in all where the original added it "
          "at every part (2x, 5x); a dt over the tick adds one tick; the other two parts of the velocity, "
          "the stack and the saved registers as the original leaves them")


if __name__ == "__main__":
    main()
