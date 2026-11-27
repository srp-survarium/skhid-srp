#!/usr/bin/env python3
"""Run patch 132's orbit camera (the Rust patch `camera_walls`) in the
default build beside the original, on a made-up player.

    python check_camera_walls_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, and
player::update_camera (0x1F3480, `ret 4`: the player) is entered as the game
world enters it, a frame a call; the game's own code makes the pivot and
runs the view's update (0x2422B0, in the build through patch 46's
death_view), the math included. A run stops at camera_director::apply
(0x243440), which gives the handler's view (+48h) to the view, so the site's
later handlers (patch 133's `where`, patch 156's lines, off) run too. ray_test
(0x75E20) is a stand-in: planes (a wall square to the orbit, upright walls,
a ceiling), or nothing; the performance counter is a clock of ours (1000
ticks a second), CreateFileA a file that does not open (the patches' log a
first-person frame opens).

- The warmup mode (3), nothing in the way: the view is the original's, bit
  for bit; one ray, from the pivot (the player's place raised 1.4 m) along
  the unit way to the original's eye, 0.2 m longer than the orbit, on the
  player's physics world with the lobby camera's filter (group 10h, mask 8,
  no delegate).
- A new orbit (after a frame in the first person) with a wall across it
  1.3 m or more out: the view's turn and yaw the original's, its eye at
  once 0.2 m in front of the wall on the way to the original's eye.
- A spawn (a new orbit) with less room: a wall 0.6 m across the way, a
  corner (walls 0.3 m behind and to the left), walls behind and to both
  sides, a box 1.5 m wide: the handler's yaw turned by 5-degree steps to
  the first way with the room, the view the original's at that yaw, the eye
  0.75 m out or more, in front of the walls; a cupboard 0.8 m wide (none
  has it): the way with the most room. The next frame (no drag yet) the
  same view; the first drag the original's turn on from there, no snap.
- An orbit going on that turns onto a way 0.5 m from a wall: the player's
  turn, the eye at once 0.3 m out on it (no nearest once the player has
  the controls: the owner, 2026-11-27).
- An orbit going on: a wall that comes has the eye in front of it at once,
  one that goes lets it out by 1 - e^(-3 t) of the way in t s.
- An orbit that turns: a second ray, where its way will be 0.3 s on at the
  turn since the last frame; a wall only that ray meets pulls the eye in by
  1 - e^(-10 t) of what it would need, on the way of this frame.
- The player drags the orbit round and round from its spawn's yaw (3
  degrees a frame twice, 1 back once) by a wall, in a corner, a dead end, a box, under a ceiling
  and in a cupboard: every frame the turn is the original's at that yaw and
  the eye on the original's way, no further out, in front of every wall,
  its steps smooth. Revision 73's patch held the view in all of them (up to
  324 frames on end in the box).
- The first person (0) and a spectator (4): the view is the original's (the
  eyes' transform), no ray of the orbit's (patch 158's own, from under the eye,
  with nothing in its way).
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
THREAD_SLOT, QPC_SLOT, QPF_SLOT, TICK_SLOT, CREATE_SLOT = 0x870118, 0x870158, 0x870150, 0x870090, 0x870048
HEAP, STUBS, STACK = 0x30000000, 0x31000000, 0x32000000
PLAYER, HANDLER, PRESENTERS = HEAP, HEAP + 0x10000, HEAP + 0x20000
PHYSICS = 0x5A5A0000                                      # a made-up physics world (ray_test's ECX)
DONE = STUBS + 0xF00
SP = STACK + 0x8000
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP, UC_X86_REG_ESP)
STEADY = dict(zip(REGS[:7], (0x0A0A0A0A, 0x0B0B0B0B, 0x0C0C0C0C, 0x0D0D0D0D, 0x05050505, 0x0E0E0E0E, 0x0019FF00)))
MARGIN, UP, IN_RATE, OUT_RATE, LEAD, NEAREST = 0.2, 1.4, 10.0, 3.0, 0.3, 0.75
# the sweeps' bound: a frame's step at most (the orbit's own at 3 m and 3 degrees a frame is 0.16 m)
LONGEST_STEP = 0.3

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
        # GetTickCount, for patch 133's handler after ours at the site (it keeps the view it reads)
        self.u32(TICK_SLOT, STUBS + 0xB0)
        emu.mem_write(STUBS + 0xB0, b"\x31\xC0\xC3")
        # CreateFileA (stdcall 7): INVALID_HANDLE_VALUE, for the patches' log a first-person frame opens
        self.u32(CREATE_SLOT, STUBS + 0xC0)
        emu.mem_write(STUBS + 0xC0, bytes.fromhex("b8ffffffffc21c00"))
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
        """bullet_physics_world::ray_test (thiscall, `ret 20h`): the nearest
        of the walls, each a plane through `point` facing `normal` (one, a
        list, or nothing); `wall` a function gives each ray of the frame (0
        the first) its own."""
        esp = emu.reg_read(UC_X86_REG_ESP)
        ret, result, origin, direction = (self.u32(esp + 4 * i) for i in range(4))
        length = struct.unpack("<f", emu.mem_read(esp + 16, 4))[0]
        group, mask, delegate = (self.u32(esp + 20 + 4 * i) for i in range(3))
        o, d = self.floats(origin, 3), self.floats(direction, 3)
        self.rays.append((emu.reg_read(UC_X86_REG_ECX), o, d, length, group & 0xFFFF, mask & 0xFFFF, delegate))
        emu.mem_write(result, bytes(0x28))
        wall = self.wall(len(self.rays) - 1) if callable(self.wall) else self.wall
        walls = [] if wall is None else wall if isinstance(wall, list) else [wall]
        ts = [dot(sub(point, o), normal) / dot(d, normal) for point, normal in walls if dot(d, normal) != 0]
        ts = [t for t in ts if 0 <= t <= length]
        if ts:
            hit = tuple(a + b * min(ts) for a, b in zip(o, d))
            emu.mem_write(result, struct.pack("<3f", *hit))
            self.u32(result + 0x1C, 0x77770000)
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, esp + 4 + 0x20)
        emu.reg_write(UC_X86_REG_EIP, ret)

    def update_camera(self, mode, physics=PHYSICS, wall=None, ms=None, yaw=0.7):
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
        emu.mem_write(HANDLER + 0x34C, struct.pack("<3f", yaw, -0.3, 3.0))   # yaw, pitch, distance
        self.wall, self.rays = wall, []
        emu.mem_write(SP, struct.pack("<2I", DONE, PLAYER))
        for reg, value in STEADY.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, SP)
        hook = emu.hook_add(UC_HOOK_CODE, lambda e, a, *_: e.emu_stop(), begin=APPLY, end=APPLY)
        emu.emu_start(UPDATE_CAMERA, DONE, count=200000)
        emu.hook_del(hook)
        regs = {r: emu.reg_read(r) for r in REGS + (UC_X86_REG_EIP,)}
        self.yaw = self.floats(HANDLER + 0x34C, 1)[0]                    # the yaw the frame leaves
        return self.floats(HANDLER + 0x48, 16), regs, self.rays


def sweeps(stock, ours, s_view):
    """The player turns the orbit round and round (a drag: the handler's yaw
    3 or 1 degrees a 16 ms frame, both ways) by walls, in corners and in
    tight spaces. Every frame: the view's turn is the original's at that yaw
    (the drag always turns it), the eye on the original's way and no further
    out, on the pivot's side of every wall, and no further than
    `LONGEST_STEP` m from the frame before's."""
    way = unit(sub(s_view[12:15], PIVOT))
    behind, side = unit((way[0], 0.0, way[2])), unit((s_view[0], 0.0, s_view[2]))
    front, left = tuple(-b for b in behind), tuple(-s for s in side)

    def upright(along, at):
        return tuple(p + a * at for p, a in zip(PIVOT, along)), tuple(-a for a in along)

    ceiling = ((PIVOT[0], PIVOT[1] + 0.5, PIVOT[2]), (0.0, -1.0, 0.0))
    box = [upright(behind, 0.4), upright(front, 0.4), upright(left, 0.4), upright(side, 0.4)]
    cases = (("a wall 0.5 m behind", [upright(behind, 0.5)]),
             ("a corner: walls 0.3 m behind and to the left", [upright(behind, 0.3), upright(left, 0.3)]),
             ("a dead end: walls 0.3 m behind and 0.35 m to both sides",
              [upright(behind, 0.3), upright(left, 0.35), upright(side, 0.35)]),
             ("a box: walls 0.4 m all round", box),
             ("a ceiling 0.5 m over the pivot and a wall 0.5 m behind", [ceiling, upright(behind, 0.5)]),
             ("a cupboard: walls 0.4 m all round and a ceiling 0.5 m over the pivot", box + [ceiling]))
    stock_views = {}

    def original(yaw):
        if yaw not in stock_views:
            stock_views[yaw] = stock.update_camera(3, yaw=yaw)[0]
        return stock_views[yaw]

    for what, walls in cases:
        for step, turns in ((3, 2), (-1, 1)):
            ours.update_camera(0)                              # a new orbit: its spawn, then the drag from its yaw
            ours.update_camera(3, wall=walls)
            frames, start = 360 * turns // abs(step), ours.yaw
            yaws = [struct.unpack("<f", struct.pack("<f", start + math.radians(step * k)))[0] for k in range(1, frames + 2)]
            turned, off_way, behind_wall, longest, last, nearest, clear_least = 0, 0, 0, 0.0, None, 9.0, 9.0
            for yaw in yaws:
                o_view = ours.update_camera(3, wall=walls, yaw=yaw)[0]
                theirs = original(yaw)
                eye, out = o_view[12:15], math.dist(o_view[12:15], PIVOT)
                turned += o_view[:12] != theirs[:12]
                off_way += out > 1e-4 and (math.dist(unit(sub(eye, PIVOT)), unit(sub(theirs[12:15], PIVOT))) > 1e-4
                                           or out > math.dist(theirs[12:15], PIVOT) + 1e-5)
                clear = min(dot(sub(eye, p), n) for p, n in walls)
                clear_least, nearest = min(clear_least, clear), min(nearest, out)
                behind_wall += clear <= 0
                if last is not None:
                    longest = max(longest, math.dist(eye, last))
                last = eye
            wrong = (["turn not the player's"] * bool(turned) + ["off the way"] * bool(off_way)
                     + ["behind a wall"] * bool(behind_wall) + ["jumps"] * (longest > LONGEST_STEP))
            check(not wrong,
                  f"{what}, the orbit turned {360 * turns} degrees at {step} a frame: the turn the player's and the "
                  f"eye on its way in {frames + 1 - max(turned, off_way)}/{frames + 1} frames, {clear_least:.3f} m "
                  f"in front of the walls at the least, {nearest:.2f} m from the pivot at the least, "
                  f"{longest:.3f} m its longest step" + (f" (wrong: {', '.join(wrong)})" if wrong else ""))


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

    for at in (1.5, 2.9, 3.1):
        ours.update_camera(0)                                    # the orbit ends: the next is new
        expect = at - MARGIN
        o_view, o_regs, rays = ours.update_camera(3, wall=wall_at(at))
        check(o_view[:12] == s_view[:12] and o_view[15] == s_view[15] and abs(eye_out(o_view) - expect) < 1e-3
              and on_way(o_view) and o_regs == s_regs and len(rays) == 1 and ours.yaw == stock.yaw,
              f"a new orbit, a wall {at} m out: the eye at once {expect:.2f} m out on the original's way, "
              "its turn the original's")

    def upright(along, at):
        """An upright wall `at` m from the pivot along the level `along`, facing the pivot."""
        return tuple(p + a * at for p, a in zip(PIVOT, along)), tuple(-a for a in along)

    def on_its_way(view, original, walls):
        """The view's turn the original's (`original`'s), the eye on its way
        no further out, on the pivot's side of every wall."""
        eye, out = view[12:15], eye_out(view)
        wrong = {"turn": view[:12] != original[:12] or view[15] != original[15],
                 "off its way": out > 1e-4 and math.dist(unit(sub(eye, PIVOT)), unit(sub(original[12:15], PIVOT))) > 1e-4,
                 "behind a wall": not all(dot(sub(eye, p), n) > 0 for p, n in walls)}
        return [what for what, bad in wrong.items() if bad]

    def same(a, b):
        """Two views alike but for the rounding of a turn about up."""
        return max(abs(x - y) for x, y in zip(a, b)) < 1e-5

    behind, side = unit((way[0], 0.0, way[2])), unit((s_view[0], 0.0, s_view[2]))
    front, left = tuple(-b for b in behind), tuple(-s for s in side)
    room = [upright(behind, 0.75), upright(front, 0.75), upright(left, 0.75), upright(side, 0.75)]
    cupboard = [upright(behind, 0.4), upright(front, 0.4), upright(left, 0.4), upright(side, 0.4)]
    # a spawn: out of the body, turned to a side by the least 5-degree steps whose way has the room
    spawns = (([wall_at(0.6)], "a wall 0.6 m out across the way", True),
              ([upright(behind, 0.3), upright(left, 0.3)], "a corner: walls 0.3 m behind and to the left", True),
              ([upright(behind, 0.3), upright(left, 0.3), upright(side, 0.3)], "walls 0.3 m behind and to both sides",
               True),
              (room, "a box 1.5 m wide: walls 0.75 m all round", True),
              (cupboard, "a cupboard 0.8 m wide (no way has the room: the one with the most)", False))
    for walls, what, has_room in spawns:
        ours.update_camera(0)
        o_view, o_regs, _ = ours.update_camera(3, wall=walls)
        turned = math.degrees(ours.yaw - 0.7)
        theirs = stock.update_camera(3, yaw=ours.yaw)[0]
        out = eye_out(o_view)
        wrong = {"not the game's orbit at the yaw left": not same(o_view[:12], theirs[:12]),
                 "off that way": math.dist(unit(sub(o_view[12:15], PIVOT)), unit(sub(theirs[12:15], PIVOT))) >= 1e-4,
                 "in the body": has_room and out < NEAREST - 1e-4,
                 "behind a wall": not all(dot(sub(o_view[12:15], p), n) > 0 for p, n in walls),
                 "registers": o_regs != s_regs}
        wrong = [w for w, bad in wrong.items() if bad]
        check(not wrong, f"a spawn, {what}: turned {turned:+.0f} degrees (its yaw), the eye {out:.2f} m out, "
              "in front of the walls" + (f" (wrong: {', '.join(wrong)})" if wrong else ""))
        yaw = ours.yaw
        o_again, _, _ = ours.update_camera(3, wall=walls, yaw=yaw)
        check(same(o_again, o_view), "the next frame, no drag yet (the game's orbit at that yaw): the same view")
        o_drag, _, _ = ours.update_camera(3, wall=walls, yaw=yaw + 0.02)
        theirs = stock.update_camera(3, yaw=yaw + 0.02)[0]
        check(o_drag[:12] == theirs[:12] and math.dist(o_drag[12:15], o_again[12:15]) < 0.1
              and all(dot(sub(o_drag[12:15], p), n) > 0 for p, n in walls),
              "the first drag: the game's turn, on from the spawn's (no snap), the eye in front of the walls")

    # an orbit going on turns onto a way 0.5 m from a wall: the player's turn, the eye in front of it
    ours.update_camera(0)
    o_free, _, _ = ours.update_camera(3, ms=12000)
    c_view, _, _ = stock.update_camera(3, yaw=0.75)
    corner = unit(sub(c_view[12:15], PIVOT))
    tight = (tuple(p + c * 0.5 for p, c in zip(PIVOT, corner)), tuple(-c for c in corner))
    walled = lambda n: tight if math.dist(ours.rays[n][2], corner) < 1e-3 else None   # only rays on that way
    o_view, o_regs, _ = ours.update_camera(3, wall=walled, ms=12016, yaw=0.75)
    wrong = on_its_way(o_view, c_view, [tight]) + ["0.3 m out"] * (abs(eye_out(o_view) - 0.3) >= 1e-3)
    check(o_free == s_view and not wrong and o_regs == s_regs,
          "a turn onto a way 0.5 m from a wall: the player's turn, the eye at once 0.3 m out on it"
          + (f" (wrong: {', '.join(wrong)})" if wrong else ""))
    ours.update_camera(0)
    o_view, _, rays = ours.update_camera(3, wall=wall_at(distance + MARGIN + 0.05))
    check(o_view == s_view and len(rays) == 1, "a wall past the eye and its margin: the original's view")

    # an orbit going on: a wall comes 100 ms after a free frame, then goes
    ours.update_camera(0)
    ours.update_camera(3, ms=5000)
    wanted = distance - (1.5 - MARGIN)
    o_view, o_regs, _ = ours.update_camera(3, wall=wall_at(1.5), ms=5100)
    pulled = wanted
    check(abs(eye_out(o_view) - (distance - pulled)) < 2e-3 and on_way(o_view) and o_regs == s_regs,
          f"a wall comes: the eye {pulled:.3f} m in at once, in front of it")
    o_view, _, _ = ours.update_camera(3, ms=5200)
    left = pulled * math.exp(-OUT_RATE * 0.1)
    check(abs(eye_out(o_view) - (distance - left)) < 2e-3 and on_way(o_view),
          f"it goes: in 100 ms the eye {left:.3f} m in (e^-0.3 of it), not out at once")
    for n in range(200):
        o_view, _, _ = ours.update_camera(3)
    check(o_view == s_view, "after a while with nothing in the way: the original's view again, bit for bit")

    # the orbit turns (0.01 rad of yaw in 10 ms): a second ray where its way will be in 0.3 s
    t_view, _, _ = stock.update_camera(3, yaw=0.71)
    turned = unit(sub(t_view[12:15], PIVOT))
    ahead = unit(tuple(w + (w - v) * (LEAD / 0.01) for w, v in zip(turned, way)))
    there = (tuple(p + a * 1.5 for p, a in zip(PIVOT, ahead)), tuple(-a for a in ahead))
    ours.update_camera(3, ms=9000)
    o_view, o_regs, rays = ours.update_camera(3, wall=lambda n: there if n == 1 else None, ms=9010, yaw=0.71)
    pulled = (distance - (1.5 - MARGIN)) * (1 - math.exp(-IN_RATE * 0.01))
    check(len(rays) == 2 and math.dist(rays[0][2], turned) < 1e-4 and math.dist(rays[1][2], ahead) < 1e-3
          and rays[1][3:] == rays[0][3:] and math.dist(rays[1][1], PIVOT) < 1e-4,
          "a turning orbit: a second ray from the pivot, where the way will be in 0.3 s, as long, the same filter")
    check(abs(eye_out(o_view) - (distance - pulled)) < 1e-3
          and math.dist(unit(sub(o_view[12:15], PIVOT)), turned) < 1e-4 and o_view[:12] == t_view[:12],
          f"a wall only that ray meets: the eye {pulled:.3f} m in after 10 ms (1 - e^-0.1 of the pull it would need)")
    o_view, _, rays = ours.update_camera(3, ms=9020, yaw=0.71)
    check(len(rays) == 1 and eye_out(o_view) > distance - pulled, "the turn ends: one ray again, the eye on its way out")
    for n in range(200):
        o_view, _, _ = ours.update_camera(3, yaw=0.71)
    check(o_view == t_view, "and the original's view again, bit for bit")

    sweeps(stock, ours, s_view)

    for mode in (0, 4):
        s_view, s_regs, _ = stock.update_camera(mode)
        o_view, o_regs, rays = ours.update_camera(mode)
        eyes = struct.unpack("<3f", struct.pack("<3f", *EYES[12:15]))
        ours_rays = [r for r in rays if math.dist(r[1], PIVOT) < 1e-3]     # the rest: patch 158's, from under the eye
        check(o_view == s_view and o_regs == s_regs and not ours_rays and len(rays) <= 1 and o_view[12:15] == eyes,
              f"mode {mode}: the eyes' view, the original's registers, no ray from the pivot")
    s_view, s_regs, _ = stock.update_camera(3, physics=0)
    o_view, o_regs, rays = ours.update_camera(3, physics=0)
    check(o_view == s_view and o_regs == s_regs and not rays, "no physics world: the original's view, no ray")
    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
