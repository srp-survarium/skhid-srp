#!/usr/bin/env python3
"""Run patch 132's orbit camera (the Rust patch `camera_walls`) in the
default build beside the original, on a made-up player.

    python check_camera_walls_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, and
player::update_camera (0x1F3480, `ret 4`: the player) is entered as the game
world enters it, a frame a call; the game's own code makes the pivot and
runs the view's update (0x2422B0, in the build through patch 46's
death_view), the math included. A run stops at camera_director::apply
(0x243440), which gives the handler's view (+48h) to the view. ray_test
(0x75E20) is a stand-in: a plane square to the orbit, or nothing; the
performance counter is a clock of ours (1000 ticks a second).

- The warmup mode (3), nothing in the way: the view is the original's, bit
  for bit; one ray, from the pivot (the player's place raised 1.4 m) along
  the unit way to the original's eye, 0.2 m longer than the orbit, on the
  player's physics world with the lobby camera's filter (group 10h, mask 8,
  no delegate).
- A new orbit (after a frame in the first person) with a wall across it:
  the view's turn is the original's, its eye at once 0.2 m in front of the
  wall on the way to the original's eye; a wall nearer than 0.2 m to the
  pivot puts the eye at it.
- An orbit going on: a wall that comes pulls the eye in by 1 - e^(-8 t) of
  the way in t s, one that goes lets it out by 1 - e^(-3 t).
- The first person (0) and a spectator (4): the view is the original's (the
  eyes' transform), no ray.
- A player with no physics world: the original's view, no ray.
In every run the registers at camera_director::apply are the original's.
"""
import math
import os
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI,
                               UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP)

if len(sys.argv) > 1:
    os.environ.setdefault("SRP_026G_EXE", sys.argv[1])    # a span hook's bytes (patch_026g.RUST_HOOKS)
import patch_026g as base

UPDATE_CAMERA, SITE, APPLY, RAY_TEST = 0x1F3480, 0x1F3650, 0x243440, 0x75E20
SPAN = bytes.fromhex("8b9378f40000")                       # mov edx,[ebx+0F478h]
THREAD_SLOT, QPC_SLOT, QPF_SLOT = 0x870118, 0x870158, 0x870150
HEAP, STUBS, STACK = 0x30000000, 0x31000000, 0x32000000
PLAYER, HANDLER, PRESENTERS = HEAP, HEAP + 0x10000, HEAP + 0x20000
PHYSICS = 0x5A5A0000                                      # a made-up physics world (ray_test's ECX)
DONE = STUBS + 0xF00
SP = STACK + 0x8000
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP, UC_X86_REG_ESP)
STEADY = dict(zip(REGS[:7], (0x0A0A0A0A, 0x0B0B0B0B, 0x0C0C0C0C, 0x0D0D0D0D, 0x05050505, 0x0E0E0E0E, 0x0019FF00)))
MARGIN, UP, IN_RATE, OUT_RATE = 0.2, 1.4, 8.0, 3.0

# the player: turned 30 degrees about up, its feet at (100, 5, -50)
C, S = math.cos(math.radians(30)), math.sin(math.radians(30))
FEET = (100.0, 5.0, -50.0)
TRANSFORM = (C, 0, -S, 0, 0, 1, 0, 0, S, 0, C, 0, *FEET, 1)
PIVOT = (FEET[0], FEET[1] + UP, FEET[2])
EYES = (1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 101, 6.6, -49, 1)   # player+44Ch, the eyes' transform

failed = 0


def check(ok, what):
    global failed
    failed += not ok
    print(f"  {'ok' if ok else 'FAIL'}  {what}")


def sub(a, b):
    return tuple(x - y for x, y in zip(a, b))


def dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def unit(a):
    n = math.sqrt(dot(a, a))
    return tuple(x / n for x in a)


class Game:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(HEAP, 0x30000, UC_PROT_ALL)
        for at in (STUBS, STACK):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        self.u32(THREAD_SLOT, STUBS + 0x80)
        emu.mem_write(STUBS + 0x80, b"\xB8\x01\x00\x00\x00\xC3")
        # QueryPerformanceCounter / Frequency (stdcall 1): our clock, 1000 ticks a second
        for slot, stub, hook in ((QPC_SLOT, STUBS + 0x90, self.counter), (QPF_SLOT, STUBS + 0xA0, self.frequency)):
            self.u32(slot, stub)
            emu.mem_write(stub, b"\xC2\x04\x00")
            emu.hook_add(UC_HOOK_CODE, hook, begin=stub, end=stub)
        emu.hook_add(UC_HOOK_CODE, self.ray_test, begin=RAY_TEST, end=RAY_TEST)
        self.ms = 1000
        self.wall = None
        self.rays = []

    def u32(self, at, value=None):
        if value is None:
            return struct.unpack("<I", self.emu.mem_read(at, 4))[0]
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def floats(self, at, n):
        return struct.unpack(f"<{n}f", self.emu.mem_read(at, 4 * n))

    def counter(self, emu, *_):
        self.emu.mem_write(self.u32(emu.reg_read(UC_X86_REG_ESP) + 4), struct.pack("<q", self.ms))
        emu.reg_write(UC_X86_REG_EAX, 1)

    def frequency(self, emu, *_):
        self.emu.mem_write(self.u32(emu.reg_read(UC_X86_REG_ESP) + 4), struct.pack("<q", 1000))
        emu.reg_write(UC_X86_REG_EAX, 1)

    def ray_test(self, emu, *_):
        """bullet_physics_world::ray_test (thiscall, `ret 20h`): the wall, a
        plane through `point` facing `normal`, or nothing."""
        esp = emu.reg_read(UC_X86_REG_ESP)
        ret, result, origin, direction = (self.u32(esp + 4 * i) for i in range(4))
        length = struct.unpack("<f", emu.mem_read(esp + 16, 4))[0]
        group, mask, delegate = (self.u32(esp + 20 + 4 * i) for i in range(3))
        o, d = self.floats(origin, 3), self.floats(direction, 3)
        self.rays.append((emu.reg_read(UC_X86_REG_ECX), o, d, length, group & 0xFFFF, mask & 0xFFFF, delegate))
        emu.mem_write(result, bytes(0x28))
        if self.wall is not None:
            point, normal = self.wall
            t = dot(sub(point, o), normal) / dot(d, normal)
            if 0 <= t <= length:
                hit = tuple(a + b * t for a, b in zip(o, d))
                emu.mem_write(result, struct.pack("<3f", *hit))
                self.u32(result + 0x1C, 0x77770000)
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, esp + 4 + 0x20)
        emu.reg_write(UC_X86_REG_EIP, ret)

    def update_camera(self, mode, physics=PHYSICS, wall=None, ms=None):
        """A frame at `ms` (16 ms after the last by default): (the handler's
        view at camera_director::apply, the registers there, the rays)."""
        emu = self.emu
        self.ms = self.ms + 16 if ms is None else ms
        emu.mem_write(HEAP, bytes(0x30000))
        self.u32(PLAYER + 0xF470, HANDLER)
        self.u32(PLAYER + 0xF478, PRESENTERS)
        self.u32(PLAYER + 0x350, physics)
        emu.mem_write(PLAYER + 0x33C, b"\x01")                  # alive
        emu.mem_write(PLAYER + 0x40C, struct.pack("<16f", *TRANSFORM))
        emu.mem_write(PLAYER + 0x44C, struct.pack("<16f", *EYES))
        self.u32(HANDLER + 0x358, mode)
        emu.mem_write(HANDLER + 0x34C, struct.pack("<3f", 0.7, -0.3, 3.0))   # yaw, pitch, distance
        self.wall, self.rays = wall, []
        emu.mem_write(SP, struct.pack("<2I", DONE, PLAYER))
        for reg, value in STEADY.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, SP)
        hook = emu.hook_add(UC_HOOK_CODE, lambda e, a, *_: e.emu_stop(), begin=APPLY, end=APPLY)
        emu.emu_start(UPDATE_CAMERA, DONE, count=200000)
        emu.hook_del(hook)
        regs = {r: emu.reg_read(r) for r in REGS + (UC_X86_REG_EIP,)}
        return self.floats(HANDLER + 0x48, 16), regs, self.rays


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    built = bytes(built)
    linked = base.arena()
    block = linked.blocks["patches.code"]
    installs = {h[0]: h for h in base.rust_hooks(original)}
    va, old, symbol, opcode = installs[SITE]
    at = base.file_offset(va)
    new = base.hook_bytes(linked.symbols[symbol], va, old, 0, opcode)
    check(opcode == 0xE9 and old == SPAN and original[at:at + 6] == SPAN and built[at:at + len(new)] == new
          and block.va <= linked.symbols[symbol] < block.va + block.size,
          f"{va:#x}: {old.hex()} in the original, `jmp {symbol}` (patches.code) here")

    stock, ours = Game(original), Game(built)
    s_view, s_regs, s_rays = stock.update_camera(3)
    o_view, o_regs, rays = ours.update_camera(3)
    eye = s_view[12:15]
    distance = math.dist(eye, PIVOT)
    way = unit(sub(eye, PIVOT))
    check(s_regs[UC_X86_REG_EIP] == APPLY and not s_rays and distance > 2.9,
          f"the original's warmup orbit: eye {tuple(round(x, 3) for x in eye)}, {distance:.3f} m "
          "from the pivot, no ray")
    check(o_view == s_view and o_regs == s_regs, "warmup, nothing in the way: the original's view and registers")
    check(len(rays) == 1 and rays[0][0] == PHYSICS and rays[0][4:] == (0x10, 0x08, 0)
          and math.dist(rays[0][1], PIVOT) < 1e-4 and math.dist(rays[0][2], way) < 1e-4
          and abs(rays[0][3] - (distance + MARGIN)) < 1e-4,
          "one ray from the pivot toward the eye, the orbit and 0.2 m long, group 10h mask 8 on its world")

    def wall_at(at):
        return tuple(p + w * at for p, w in zip(PIVOT, way)), tuple(-w for w in way)

    def eye_out(view):
        return math.dist(view[12:15], PIVOT)

    def on_way(view):
        return math.dist(unit(sub(view[12:15], PIVOT)), way) < 1e-4

    for at in (1.5, 2.9, 3.1, 0.1):
        ours.update_camera(0)                                    # the orbit ends: the next is new
        expect = max(0.0, at - MARGIN)
        o_view, o_regs, rays = ours.update_camera(3, wall=wall_at(at))
        check(o_view[:12] == s_view[:12] and o_view[15] == s_view[15] and abs(eye_out(o_view) - expect) < 1e-3
              and (expect == 0 or on_way(o_view)) and o_regs == s_regs and len(rays) == 1,
              f"a new orbit, a wall {at} m out: the eye at once {expect:.2f} m out on the original's way, "
              "its turn the original's")
    ours.update_camera(0)
    o_view, _, rays = ours.update_camera(3, wall=wall_at(distance + MARGIN + 0.05))
    check(o_view == s_view and len(rays) == 1, "a wall past the eye and its margin: the original's view")

    # an orbit going on: a wall comes 100 ms after a free frame, then goes
    ours.update_camera(0)
    ours.update_camera(3, ms=5000)
    wanted = distance - (1.5 - MARGIN)
    o_view, o_regs, _ = ours.update_camera(3, wall=wall_at(1.5), ms=5100)
    pulled = wanted * (1 - math.exp(-IN_RATE * 0.1))
    check(abs(eye_out(o_view) - (distance - pulled)) < 2e-3 and on_way(o_view) and o_regs == s_regs,
          f"a wall comes: in 100 ms the eye {pulled:.3f} of {wanted:.3f} m in (1 - e^-0.8), not at once")
    o_view, _, _ = ours.update_camera(3, ms=5200)
    left = pulled * math.exp(-OUT_RATE * 0.1)
    check(abs(eye_out(o_view) - (distance - left)) < 2e-3 and on_way(o_view),
          f"it goes: in 100 ms the eye {left:.3f} m in (e^-0.3 of it), not out at once")
    for n in range(200):
        o_view, _, _ = ours.update_camera(3)
    check(o_view == s_view, "after a while with nothing in the way: the original's view again, bit for bit")

    for mode in (0, 4):
        s_view, s_regs, _ = stock.update_camera(mode)
        o_view, o_regs, rays = ours.update_camera(mode)
        eyes = struct.unpack("<3f", struct.pack("<3f", *EYES[12:15]))
        check(o_view == s_view and o_regs == s_regs and not rays and o_view[12:15] == eyes,
              f"mode {mode}: the eyes' view, the original's registers, no ray")
    s_view, s_regs, _ = stock.update_camera(3, physics=0)
    o_view, o_regs, rays = ours.update_camera(3, physics=0)
    check(o_view == s_view and o_regs == s_regs and not rays, "no physics world: the original's view, no ray")
    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
