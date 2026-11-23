#!/usr/bin/env python3
"""Run patch 158's first-person eye (the Rust patch `head_clearance`) in the
default build beside the original, on a made-up player.

    python check_head_clearance_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, and
player::update_camera (0x1F3480, `ret 4`: the player) is entered as the game
world enters it, a frame a call; the game's own code picks the eye, runs the
hooks at its join (0x1F3643: the ragdoll's, then ours) and the view's
update (0x2422B0, in the build through patch 46's death_view). A run stops
after it, at 0x1F3650 (camera.viewed, before patch 133's hook there), the
handler's view (+48h) written as camera_director::apply then takes it. ray_test (0x75E20) is a stand-in: planes square to the ray, each of a game
material (the nearest is met), or nothing; the performance counter is a clock of ours (1000 ticks a second).

The player stands at (100, 5, -50) turned 30 degrees about up, crouched: its
eye 1.06 m over the feet and 0.13 m ahead.

- The first person (0), nothing in the way: the view is the original's, bit
  for bit; one ray, from 0.4 m under the eye on the body's up over its feet
  toward the eye, the way 0.12 m longer, on the player's physics world with
  the lobby camera's filter (group 10h, mask 8, no delegate).
- A surface across the way: the view's turn is the original's, its eye at
  once 0.12 m short of it on the way (a surface nearer than 0.12 m to the
  start puts the eye at the start); one past the eye and its clearance
  leaves the original's view.
- It goes: the eye back by 1 - e^(-6 t) of the way in t s, then the
  original's view again, bit for bit.
- Leaves across the way (game materials 24, 44, 49): the ray goes on from
  1 mm behind them, to the surface behind (one they lie on too) or to nothing (the original's
  view); through 16 of them at most.
- A spectator (4): the same as the first person.
- The warmup's orbit (3): the original's view, no ray.
- A player with no physics world: the original's view, no ray.
In every run the registers there are the original's.
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

UPDATE_CAMERA, SITE, VIEWED, RAY_TEST = 0x1F3480, 0x1F3643, 0x1F3650, 0x75E20
SPAN = bytes.fromhex("8b8b70f40000")                       # mov ecx,[ebx+0F470h]
THREAD_SLOT, QPC_SLOT, QPF_SLOT = 0x870118, 0x870158, 0x870150
HEAP, STUBS, STACK = 0x30000000, 0x31000000, 0x32000000
PLAYER, HANDLER, PRESENTERS = HEAP, HEAP + 0x10000, HEAP + 0x20000
PHYSICS = 0x5A5A0000                                      # a made-up physics world (ray_test's ECX)
DONE = STUBS + 0xF00
SP = STACK + 0x8000
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP, UC_X86_REG_ESP)
STEADY = dict(zip(REGS[:7], (0x0A0A0A0A, 0x0B0B0B0B, 0x0C0C0C0C, 0x0D0D0D0D, 0x05050505, 0x0E0E0E0E, 0x0019FF00)))
CLEARANCE, BELOW_EYE, OUT_RATE = 0.12, 0.4, 6.0
OBJECT, VTABLE, MATERIAL_OF = STUBS + 0x200, STUBS + 0x300, STUBS + 0xB0   # the object a ray meets
WALL, LEAVES, PAST_LEAF, MOST_LEAVES = 11, (24, 44, 49), 0.001, 16

# the player: turned 30 degrees about up, its feet at (100, 5, -50); crouched, its eye 1.06 m up and 0.13 m ahead
C, S = math.cos(math.radians(30)), math.sin(math.radians(30))
FEET = (100.0, 5.0, -50.0)
TRANSFORM = (C, 0, -S, 0, 0, 1, 0, 0, S, 0, C, 0, *FEET, 1)
EYE_AT = (FEET[0] + 0.13 * S, FEET[1] + 1.06, FEET[2] + 0.13 * C)
EYES = (C, 0, -S, 0, 0, 1, 0, 0, S, 0, C, 0, *EYE_AT, 1)   # player+44Ch, the eyes' transform
START = (FEET[0], FEET[1] + 1.06 - BELOW_EYE, FEET[2])

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
        # GetEnvironmentVariableA (patch 197's SKHID_CAMERA_TRAIL, read at the same site): unset
        self.u32(0x8701B4, STUBS + 0xC0)
        emu.mem_write(STUBS + 0xC0, bytes.fromhex("31c0c20c00"))
        # QueryPerformanceCounter / Frequency (stdcall 1): our clock, 1000 ticks a second
        for slot, stub, hook in ((QPC_SLOT, STUBS + 0x90, self.counter), (QPF_SLOT, STUBS + 0xA0, self.frequency)):
            self.u32(slot, stub)
            emu.mem_write(stub, b"\xC2\x04\x00")
            emu.hook_add(UC_HOOK_CODE, hook, begin=stub, end=stub)
        emu.hook_add(UC_HOOK_CODE, self.ray_test, begin=RAY_TEST, end=RAY_TEST)
        # get_triangle_material (the object's vtable +10h, thiscall, `ret 8`): the met surface's
        self.u32(OBJECT, VTABLE)
        self.u32(VTABLE + 0x10, MATERIAL_OF)
        emu.mem_write(MATERIAL_OF, b"\xC2\x08\x00")
        emu.hook_add(UC_HOOK_CODE, self.material_of, begin=MATERIAL_OF, end=MATERIAL_OF)
        self.ms = 1000
        self.surfaces = []
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
        """bullet_physics_world::ray_test (thiscall, `ret 20h`): the nearest
        of the surfaces (planes through `point` facing `normal`), or nothing."""
        esp = emu.reg_read(UC_X86_REG_ESP)
        ret, result, origin, direction = (self.u32(esp + 4 * i) for i in range(4))
        length = struct.unpack("<f", emu.mem_read(esp + 16, 4))[0]
        group, mask, delegate = (self.u32(esp + 20 + 4 * i) for i in range(3))
        o, d = self.floats(origin, 3), self.floats(direction, 3)
        self.rays.append((emu.reg_read(UC_X86_REG_ECX), o, d, length, group & 0xFFFF, mask & 0xFFFF, delegate))
        emu.mem_write(result, bytes(0x28))
        met = [(dot(sub(point, o), normal) / dot(d, normal), i) for i, (point, normal, _) in enumerate(self.surfaces)]
        met = [(t, i) for t, i in met if 0 <= t <= length]
        if met:
            t, i = min(met)
            hit = tuple(a + b * t for a, b in zip(o, d))
            emu.mem_write(result, struct.pack("<3f", *hit))
            emu.mem_write(result + 0xC, struct.pack("<f", t / length))
            self.u32(result + 0x1C, OBJECT)
            self.u32(result + 0x20, i)
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, esp + 4 + 0x20)
        emu.reg_write(UC_X86_REG_EIP, ret)

    def material_of(self, emu, *_):
        emu.reg_write(UC_X86_REG_EAX, self.surfaces[self.u32(emu.reg_read(UC_X86_REG_ESP) + 4)][2])

    def update_camera(self, mode, physics=PHYSICS, wall=None, leaves=(), ms=None):
        """A frame at `ms` (16 ms after the last by default): (the handler's
        view after its update (0x1F3650), the registers there, the rays)."""
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
        self.surfaces, self.rays = [*leaves, *([wall] if wall else [])], []
        emu.mem_write(SP, struct.pack("<2I", DONE, PLAYER))
        for reg, value in STEADY.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, SP)
        hook = emu.hook_add(UC_HOOK_CODE, lambda e, a, *_: e.emu_stop(), begin=VIEWED, end=VIEWED)
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
    installs = {h[0]: h for h in base.rust_hooks(original)}
    va, old, symbol, opcode = installs[SITE]
    at = base.file_offset(va)
    new = base.hook_bytes(linked.symbols[symbol], va, old, 0, opcode)
    check(opcode == 0xE9 and old == SPAN and original[at:at + 6] == SPAN and built[at:at + len(new)] == new,
          f"{va:#x}: {old.hex()} in the original, `jmp {symbol}` here (the ragdoll's eye, then ours)")

    stock, ours = Game(original), Game(built)
    way = unit(sub(EYE_AT, START))
    length = math.dist(EYE_AT, START)
    eye32 = struct.unpack("<3f", struct.pack("<3f", *EYE_AT))

    def surface_at(at, material=WALL):
        return tuple(p + w * at for p, w in zip(START, way)), tuple(-w for w in way), material

    def eye_out(view):
        return math.dist(view[12:15], START)

    def on_way(view):
        return math.dist(unit(sub(view[12:15], START)), way) < 1e-4

    for mode in (0, 4):
        s_view, s_regs, s_rays = stock.update_camera(mode)
        o_view, o_regs, rays = ours.update_camera(mode)
        check(s_regs[UC_X86_REG_EIP] == VIEWED and not s_rays and s_view[12:15] == eye32,
              f"mode {mode}, the original: the eyes' view, no ray")
        check(o_view == s_view and o_regs == s_regs, f"mode {mode}, nothing in the way: the original's view and registers")
        check(len(rays) == 1 and rays[0][0] == PHYSICS and rays[0][4:] == (0x10, 0x08, 0)
              and math.dist(rays[0][1], START) < 1e-4 and math.dist(rays[0][2], way) < 1e-4
              and abs(rays[0][3] - (length + CLEARANCE)) < 1e-4,
              f"mode {mode}: one ray from 0.4 m under the eye over the feet toward the eye, 0.12 m past it, "
              "group 10h mask 8 on its world")
        for at in (length + 0.05, length - 0.1, 0.05):
            expect = max(0.0, at - CLEARANCE)
            o_view, o_regs, rays = ours.update_camera(mode, wall=surface_at(at))
            check(o_view[:12] == s_view[:12] and o_view[15] == s_view[15] and abs(eye_out(o_view) - expect) < 1e-4
                  and (expect == 0 or on_way(o_view)) and o_regs == s_regs and len(rays) == 1,
                  f"mode {mode}, a surface {at:.3f} m out: the eye at once {expect:.3f} m out on the way, "
                  "its turn the original's")
        for n in range(100):
            o_view, _, _ = ours.update_camera(mode)
        check(o_view == s_view, f"mode {mode}: after a while with nothing in the way, the original's view again")

    # leaves across the way: the ray goes on behind them
    s_view, s_regs, _ = stock.update_camera(0)
    behind = tuple(p + w * (0.2 + PAST_LEAF) for p, w in zip(START, way))
    for material in LEAVES:
        o_view, o_regs, rays = ours.update_camera(0, leaves=[surface_at(0.2, material)])
        check(o_view == s_view and o_regs == s_regs and len(rays) == 2 and math.dist(rays[1][1], behind) < 1e-4
              and math.dist(rays[1][2], way) < 1e-4 and abs(rays[1][3] - (length + CLEARANCE - 0.2 - PAST_LEAF)) < 1e-4,
              f"leaves ({material}) 0.200 m out: a second ray from 1 mm behind them, the original's view")
        lain = length - 0.1 + 0.005
        o_view, o_regs, rays = ours.update_camera(0, leaves=[surface_at(length - 0.1, material)], wall=surface_at(lain))
        check(abs(eye_out(o_view) - (lain - CLEARANCE)) < 1e-4 and on_way(o_view) and len(rays) == 2,
              f"leaves ({material}) lying 5 mm over a surface: the eye 0.12 m short of the surface")
        o_view, o_regs, rays = ours.update_camera(0, leaves=[surface_at(0.2, material)], wall=surface_at(length - 0.1))
        check(abs(eye_out(o_view) - (length - 0.1 - CLEARANCE)) < 1e-4 and on_way(o_view) and o_regs == s_regs
              and len(rays) == 2, f"leaves ({material}) before a surface: the eye 0.12 m short of the surface")
        for n in range(100):
            o_view, _, _ = ours.update_camera(0)
    hedge = [surface_at(0.02 * k, LEAVES[1]) for k in range(1, 21)]
    o_view, o_regs, rays = ours.update_camera(0, leaves=hedge, wall=surface_at(length))
    check(o_view == s_view and o_regs == s_regs and len(rays) == MOST_LEAVES,
          f"more than {MOST_LEAVES} leaves on the way: {MOST_LEAVES} rays, the original's view")

    # a surface comes, then goes 100 ms later
    s_view, s_regs, _ = stock.update_camera(0)
    ours.update_camera(0, ms=5000)
    o_view, _, _ = ours.update_camera(0, wall=surface_at(length + 0.05), ms=5016)
    pull = CLEARANCE - 0.05
    check(abs(eye_out(o_view) - (length - pull)) < 1e-4, f"a surface comes: the eye {pull:.3f} m back at once")
    o_view, o_regs, _ = ours.update_camera(0, ms=5116)
    left = pull * math.exp(-OUT_RATE * 0.1)
    check(abs(eye_out(o_view) - (length - left)) < 1e-4 and on_way(o_view) and o_regs == s_regs,
          f"it goes: in 100 ms the eye {left:.4f} m back (e^-0.6 of it), not out at once")

    s_view, s_regs, _ = stock.update_camera(3)
    o_view, o_regs, rays = ours.update_camera(3, wall=surface_at(0.2))
    check(o_view == s_view and o_regs == s_regs and not rays, "the warmup's orbit (3): the original's view, no ray")
    s_view, s_regs, _ = stock.update_camera(0, physics=0)
    o_view, o_regs, rays = ours.update_camera(0, physics=0)
    check(o_view == s_view and o_regs == s_regs and not rays, "no physics world: the original's view, no ray")
    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
