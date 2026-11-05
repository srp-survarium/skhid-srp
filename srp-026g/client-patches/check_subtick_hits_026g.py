#!/usr/bin/env python3
"""Patch 103 (rust/patches/src/subtick_hits.rs, docs/notes/subtick_bullet_hits.md):
a bullet meets a player where he stood when it crossed him. Runs
bullet::check_collision's ray test (0x32D5E5, after the ray's end is made,
to 0x32D631, past the site `bullet.ray_traced` 0x32D627 and its two moves)
in the original exe and in the patched one, with Unicorn, from the same
state.

    python check_subtick_hits_026g.py <original survarium.exe>

The ray test is the game's own code end to end: bullet_physics_world::
ray_test 0x75E20 (its closest-ray callback 0x75B40, needsCollision
0x75BD0, addSingleResult 0x75C40), btCollisionWorld::rayTest 0xA33A0,
btSingleRayCallback 0xA3050 / process 0xA3310 and Bullet's rayTestSingle
0xA02D0 with its GJK cast. Made up: the collision world's broadphase (its
rayTest a stub that hands every object to process, in order: the culling
of btDbvtBroadphase only skips objects the ray misses), the objects
(btCollisionObject +10h transform made by the game's from_vostok 0x74470,
+0C8h proxy, +0CCh a btSphereShape of the exe's vtable 0x9E2FCC, +0F8h the
game object), a hit model's delegate (the boost function needsCollision
calls; a stub answering 2, "posed", or 0 for the shooter's own body, as
recompute_damage_collision_bones 0x2DA5B0 does), the players (their place,
world core, spawn time, alive, character controller's m_last_pos +50h and
position +0B0h, hit model +498h -> +12Ch), the world core (the places,
the active mask, its time +18FC0h), the bullet (its owner, the player's
+110h; its manager -> the physics world; its time +60h) and
check_collision's frame (origin, direction, length, the flight's three
times).

Checked, each case's result (the 28h bytes check_collision then reads),
registers and frame, and every byte of the made-up world after the run:
- a still target the ray meets: both exes the same, byte for byte; with a
  moving player elsewhere in the world too; with a moving player near the
  ray (the patch then traces the level again and tests every body near it,
  its own way): still byte for byte the original's;
- a target moving across the ray at 5 m/s that the original misses (it
  tests him where the step left him): the patch hits him where he stood at
  the crossing ms, at the point of the ray on his sphere there;
- a wall in front of him: both hit the wall alike; a wall behind him: the
  original the wall, the patch him;
- the shooter's own body moving on the ray: neither hits it;
- a player spawned in the step, or dead: never put back (the original's);
- a step the world took in parts (20 ms): put back by its part's whole
  move, no further;
- the world's bytes (bodies, transforms, proxies, shapes, players, the
  core) are the same after the run as before, in both exes: nothing is
  moved; the result is the patch's only write.
"""
import math
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_HOOK_BLOCK, UC_MODE_32, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EFLAGS, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

START, STOP, SITE = 0x32D5E5, 0x32D631, 0x32D627
FROM_VOSTOK = 0x74470
PHYSICS_VTABLE, SPHERE_VTABLE, ANIMATED_BODY_VTABLE = 0x9E2EBC, 0x9E2FCC, 0x9E2AF4
COLLISION_WORLD_RAY_TEST = 0xA33A0
THREAD_SLOT, QPC_SLOT, QPF_SLOT = 0x870118, 0x870158, 0x870150

HEAP, HEAP_SIZE = 0x40000000, 0x00400000
STUBS, STACK = 0x48000000, 0x48200000
RET = STUBS + 0xF00                              # a return address: the stop of a function run
# made-up memory
PW, CW, CW_VT, BP, BP_VT, LIST = (HEAP + 0x100 * i for i in range(6))
BULLET, MANAGER, TIMES, MATRIX = HEAP + 0x1000, HEAP + 0x1200, HEAP + 0x1400, HEAP + 0x1500
CORE = HEAP + 0x10000                            # +18458h places .. +18FD8h mask: 0x19000 bytes
PLACES = HEAP + 0x40000
OBJECTS = HEAP + 0x50000                         # 0x1000 each: object, proxy, shape, game object, hit model
PLAYERS = HEAP + 0x80000                         # 0x1000 each, +0x800 the controller and its holder
WORLD_END = HEAP + 0xA0000
G = 1050                                         # the step's end (ms)
BOOST_POSED, BOOST_SELF = STUBS + 0x100, STUBS + 0x110
BOOST_TABLE_POSED, BOOST_TABLE_SELF = STUBS + 0x120, STUBS + 0x130
TRAMPOLINE = STUBS + 0x200                       # fld dword [length]; jmp START
LENGTH_AT = STUBS + 0x240

# player fields (fields_026g.tsv)
REFS, PLACE, CORE_FIELD, SPAWNED, ALIVE, CONTROLLER, HIT_MODEL = 0xD0, 0x114, 0x120, 0x328, 0x33C, 0x494, 0x498


def f32(x):
    return struct.unpack("<f", struct.pack("<f", x))[0]


class World:
    """The made-up world in one Unicorn, the exe's sections mapped."""

    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(HEAP, HEAP_SIZE, UC_PROT_ALL)
        emu.mem_map(STUBS, 0x100000, UC_PROT_ALL)
        emu.mem_map(STACK - 0x100000, 0x100000, UC_PROT_ALL)
        # the broadphase's rayTest (thiscall: from, to, the ray callback,
        # aabb min, max; ret 14h): process (its vtable +4) for each object
        loop = bytes([0x8B, 0x06, 0x85, 0xC0, 0x74, 0x0D, 0x50, 0x8B, 0xCB, 0x8B, 0x11, 0xFF, 0x52, 0x04,
                      0x83, 0xC6, 0x04, 0xEB, 0xED])
        stub = bytes([0x53, 0x56, 0x8B, 0x5C, 0x24, 0x14, 0xBE]) + struct.pack("<I", LIST) + loop \
            + bytes([0x5E, 0x5B, 0xC2, 0x14, 0x00])
        emu.mem_write(STUBS, stub)
        self.u32(BP, BP_VT)
        self.u32(BP_VT + 0x14, STUBS)
        self.u32(CW, CW_VT)
        self.u32(CW_VT + 0x18, COLLISION_WORLD_RAY_TEST)
        self.u32(CW + 0x4C, BP)
        self.u32(PW, PHYSICS_VTABLE)
        self.u32(PW + 0x3C, CW)
        # a hit model's delegate (boost::function: [+0]&~1 its table, +4 the
        # invoker, cdecl): 2 "posed", 0 the shooter's own
        emu.mem_write(BOOST_POSED, b"\xB8\x02\x00\x00\x00\xC3")
        emu.mem_write(BOOST_SELF, b"\x31\xC0\xC3")
        self.u32(BOOST_TABLE_POSED + 4, BOOST_POSED)
        self.u32(BOOST_TABLE_SELF + 4, BOOST_SELF)
        # the other hooks at the site (the ragdoll's, the lamps') take a lock:
        # GetCurrentThreadId (one thread, id 1); QueryPerformanceCounter and
        # -Frequency (stdcall 4: nothing)
        emu.mem_write(THREAD_SLOT, struct.pack("<I", STUBS + 0x80))
        emu.mem_write(STUBS + 0x80, b"\xB8\x01\x00\x00\x00\xC3")
        for slot in (QPC_SLOT, QPF_SLOT):
            emu.mem_write(slot, struct.pack("<I", STUBS + 0x90))
        emu.mem_write(STUBS + 0x90, b"\xC2\x04\x00")
        emu.mem_write(TRAMPOLINE, b"\xD9\x05" + struct.pack("<I", LENGTH_AT) + b"\xE9"
                      + struct.pack("<i", START - (TRAMPOLINE + 11)))
        # the core: twenty places, none active yet
        self.u32(CORE + 0x18458, PLACES)
        self.u32(CORE + 0x1845C, PLACES + 20 * 0x30)
        self.u32(MANAGER + 0x98, PW)
        self.objects, self.players = 0, 0

    # -- memory -----------------------------------------------------------
    def u32(self, at, value=None):
        if value is None:
            return struct.unpack("<I", self.emu.mem_read(at, 4))[0]
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def floats(self, at, values=None, count=3):
        if values is None:
            return list(struct.unpack(f"<{count}f", self.emu.mem_read(at, 4 * count)))
        self.emu.mem_write(at, struct.pack(f"<{len(values)}f", *values))

    def call(self, va, regs):
        esp = STACK - 0x2000
        self.u32(esp, RET)
        self.emu.reg_write(UC_X86_REG_ESP, esp)
        for reg, value in regs.items():
            self.emu.reg_write(reg, value)
        self.emu.emu_start(va, RET, count=1_000_000)

    # -- the world --------------------------------------------------------
    def body(self, position, radius, group, mask, boost=None, user=None):
        """A collision object with a sphere at `position` (game frame); its
        game object a hit model's (with `boost`, its delegate) or `user`."""
        at = OBJECTS + 0x1000 * self.objects
        self.objects += 1
        obj, proxy, shape, game, model = at, at + 0x200, at + 0x300, at + 0x400, at + 0x600
        # the transform, by the game's own from_vostok (EDX a game matrix, ECX out)
        self.floats(MATRIX, [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, *position, 1])
        self.call(FROM_VOSTOK, {UC_X86_REG_ECX: obj + 0x10, UC_X86_REG_EDX: MATRIX})
        self.u32(obj + 0xC8, proxy)
        self.u32(obj + 0xCC, shape)
        self.u32(proxy, obj)
        self.emu.mem_write(proxy + 4, struct.pack("<HH", group, mask))
        # btSphereShape(radius) as Bullet 2.78 lays it out (rayTestSingle's
        # own point shape at 0xA033B: vtable, type 8, user 0, scaling 1)
        self.u32(shape, SPHERE_VTABLE)
        self.u32(shape + 4, 8)
        self.floats(shape + 0x10, [1, 1, 1, 0])
        self.floats(shape + 0x20, [radius, 0, 0, 0])
        self.floats(shape + 0x30, [radius])
        if boost is not None:
            self.u32(game, ANIMATED_BODY_VTABLE)
            self.u32(game + 0x10, boost)
            self.u32(game + 0x30, obj)
            self.emu.mem_write(game + 0x38, struct.pack("<H", 10))
            self.u32(model + 0x12C, game)
        else:
            self.u32(game, user)                         # the game object's vtable
        self.u32(obj + 0xF8, game)
        self.u32(LIST + 4 * (self.objects - 1), proxy)
        self.u32(LIST + 4 * self.objects, 0)
        return {"object": obj, "game": game, "model": model}

    def player(self, place, root, moved=(0, 0, 0), spawned=0, alive=True, own=False, radius=0.3):
        """A player at `place` whose hit model is a sphere around his root
        (game frame), his controller's last step `moved` (to the root)."""
        p = PLAYERS + 0x1000 * self.players
        self.players += 1
        body = self.body(root, radius, 0x40, 0x11, BOOST_TABLE_SELF if own else BOOST_TABLE_POSED)
        self.u32(p + REFS, 3)
        self.emu.mem_write(p + PLACE, bytes([place]))
        self.u32(p + CORE_FIELD, CORE)
        self.u32(p + SPAWNED, spawned)
        self.emu.mem_write(p + ALIVE, bytes([1 if alive else 0]))
        holder, controller = p + 0x800, p + 0x900
        self.u32(p + CONTROLLER, holder)
        self.u32(holder, controller)
        centre = [root[0], root[1] + 0.8, root[2]]              # the capsule's centre: any height
        self.floats(controller + 0xB0, centre)
        self.floats(controller + 0x50, [c - m for c, m in zip(centre, moved)])
        self.u32(p + HIT_MODEL, body["model"])
        self.u32(PLACES + 0x30 * place, p)
        self.u32(CORE + 0x18FD8, self.u32(CORE + 0x18FD8) | 1 << place)
        return {**body, "player": p}

    def wall(self, position, radius):
        return self.body(position, radius, 0x08, 0x10, user=0x12340000)    # a vtable no hit model has

    def shoot(self, origin, direction, length, flight, shooter, step_from=G - 50):
        """check_collision's ray test from 0x32D5E5 to 0x32D631: the
        result (28h bytes), the registers, the frame, the world's bytes
        before and after."""
        emu = self.emu
        self.u32(BULLET + 8, shooter + 0x110)
        self.u32(BULLET + 0x44, MANAGER)
        self.u32(BULLET + 0x60, G)
        self.u32(CORE + 0x18FC0, step_from)
        self.floats(TIMES, list(flight))
        self.floats(LENGTH_AT, [length])
        esp = STACK - 0x1000
        frame = bytearray(0xC0)
        struct.pack_into("<f", frame, 0x14, length)
        struct.pack_into("<3f", frame, 0x20, *direction)
        struct.pack_into("<3f", frame, 0x90, *origin)
        struct.pack_into("<3I", frame, 0x9C, TIMES, TIMES + 4, TIMES + 8)
        emu.mem_write(esp, bytes(frame))
        emu.reg_write(UC_X86_REG_ESP, esp)
        regs = {UC_X86_REG_EBP: PW, UC_X86_REG_EDI: BULLET, UC_X86_REG_EAX: 0xA1, UC_X86_REG_ECX: 0xC1,
                UC_X86_REG_EDX: 0xD1, UC_X86_REG_EBX: 0xB1, UC_X86_REG_ESI: 0x51}
        for reg, value in regs.items():
            emu.reg_write(reg, value)
        before = bytes(emu.mem_read(HEAP, WORLD_END - HEAP))
        trail = []
        hook = emu.hook_add(UC_HOOK_BLOCK, lambda _e, address, *_: trail.append(address))
        try:
            emu.emu_start(TRAMPOLINE, STOP, count=20_000_000)
        except Exception as error:
            sys.exit(f"FAIL: {error} at {emu.reg_read(UC_X86_REG_EIP):#x}; the last blocks "
                     + " ".join(f"{a:#x}" for a in trail[-16:]))
        emu.hook_del(hook)
        assert emu.reg_read(UC_X86_REG_EIP) == STOP
        assert emu.reg_read(UC_X86_REG_ESP) == esp, "the stack is balanced"
        after = bytes(emu.mem_read(HEAP, WORLD_END - HEAP))
        out = {r: emu.reg_read(r) for r in (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX,
                                             UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)}
        result = bytes(emu.mem_read(out[UC_X86_REG_EAX], 0x28))
        return {"result": result, "regs": out, "frame": bytes(emu.mem_read(esp, 0xC0)), "same": before == after}


def hit(shot):
    r = shot["result"]
    point = struct.unpack_from("<3f", r, 0)
    fraction, = struct.unpack_from("<f", r, 0xC)
    normal = struct.unpack_from("<3f", r, 0x10)
    obj, part = struct.unpack_from("<II", r, 0x1C)
    return {"point": point, "fraction": fraction, "normal": normal, "object": obj, "part": part, "kind": r[0x24]}


FAILED = []


def check(ok, what):
    print(("ok: " if ok else "FAIL: ") + what)
    if not ok:
        FAILED.append(what)


def scene(data, build):
    world = World(data)
    return world, build(world)


def both(original, built, build, origin, direction, length, flight, step_from=G - 50):
    """The same scene and shot in the original exe and the patched one."""
    shots = []
    for data in (original, built):
        world, parts = scene(data, build)
        shots.append((world, parts, world.shoot(origin, direction, length, flight, parts["shooter"]["player"],
                                                step_from)))
    return shots


# the ray: from 10 m in front, along -z through x = -0.2, 1.2 m up; 35 m in a step (700 m/s)
ORIGIN, DIRECTION, LENGTH = (-0.2, 1.2, 10.0), (0.0, 0.0, -1.0), 35.0
WHOLE = (0.0, 0.05, 0.05)                        # fired at the step's start


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    linked = base.arena()
    block = linked.blocks["patches.code"]
    hooks = [h for h in base.rust_hooks(original) if h[0] == SITE]
    va, old, symbol, opcode = hooks[0]
    target = linked.symbols[symbol]
    at = base.file_offset(va)
    jump = base.hook_bytes(target, va, old, 0, opcode)
    check(len(hooks) == 1 and opcode == 0xE9 and original[at:at + len(old)] == old
          and built[at:at + len(jump)] == jump and block.va <= target < block.va + block.size,
          f"{SITE:#x}: {old.hex()} in the original, `jmp {symbol}` (the site's chain) in the build")
    shooter = lambda w: w.player(5, (0.0, 0.0, 30.0), own=True)

    def stood(position, origin=ORIGIN, direction=DIRECTION):
        """The original's hit of a still target standing at `position` (the
        same objects, in the same order, as the cases')."""
        world = World(original)
        sh = shooter(world)
        world.player(0, position)
        return hit(world.shoot(origin, direction, LENGTH, WHOLE, sh["player"]))

    def as_stood(h, ref):
        """The same hit as the original's on him standing there (Bullet's GJK
        cast gives a sphere's point and normal to about 1e-3)."""
        return (h["object"] == ref["object"] != 0 and h["part"] == ref["part"] and h["kind"] == ref["kind"]
                and all(abs(a - b) < 1e-5 for a, b in zip(h["point"] + h["normal"], ref["point"] + ref["normal"]))
                and abs(h["fraction"] - ref["fraction"]) < 1e-6)

    def compare(name, build, flight=WHOLE, step_from=G - 50):
        (_, _, old), (_, _, new) = both(original, built, build, ORIGIN, DIRECTION, LENGTH, flight, step_from)
        check(old["result"] == new["result"] and old["regs"] == new["regs"] and old["frame"] == new["frame"]
              and old["same"] and new["same"], f"{name}: the original's result, registers and frame, byte for byte; "
                                               "the world untouched")
        return old, new

    # -- a still target --------------------------------------------------
    def still(w):
        return {"shooter": shooter(w), "target": w.player(0, (-0.1, 1.2, 0.0))}
    old, _ = compare("a still target the ray meets", still)
    check(hit(old)["object"] != 0, "  (the original hits him)")

    def still_and_far(w):
        parts = still(w)
        w.player(1, (40.0, 0.0, -50.0), moved=(0.25, 0.0, 0.0))
        return parts
    compare("a still target, a player moving far from the ray", still_and_far)

    def still_and_near(w):
        parts = still(w)
        w.player(1, (1.5, 0.0, -5.0), moved=(0.25, 0.0, 0.1))
        return parts
    compare("a still target, a player moving near the ray (the patch's own level trace and body tests)",
            still_and_near)

    # -- a moving target ------------------------------------------------
    # crossed at z = 0: 10 of the ray's 35 m, 14.3 ms into the step, 35.7 ms
    # before its end; he moved 25 cm along +x in the step (5 m/s): at its
    # end at x = 0.25 (0.45 m from the ray, his sphere 0.3: missed), at the
    # crossing 0.714 x 0.25 back, at x = 0.0714 (0.271 m: met)
    def moving(w):
        return {"shooter": shooter(w), "target": w.player(0, (0.25, 1.2, 0.0), moved=(0.25, 0.0, 0.0))}
    (_, _, old), (wn, parts, new) = both(original, built, moving, ORIGIN, DIRECTION, LENGTH, WHOLE)
    share = (0.05 - 0.05 * f32(10.0 / 35.0)) * 1000 / 50
    x = 0.25 - 0.25 * share
    dz = math.sqrt(0.3 ** 2 - (x - ORIGIN[0]) ** 2)
    expected = (ORIGIN[0], ORIGIN[1], dz)
    h = hit(new)
    check(hit(old)["object"] == 0, "a moving target: the original misses him (where the step left him)")
    normal = ((ORIGIN[0] - x) / 0.3, 0.0, dz / 0.3)
    check(h["object"] == parts["target"]["game"] and h["kind"] == 1 and h["part"] == 0
          and all(abs(a - b) < 1e-3 for a, b in zip(h["point"], expected))
          and all(abs(a - b) < 0.02 for a, b in zip(h["normal"], normal)),
          f"  the patch hits him where he stood at the crossing: {[round(v, 4) for v in h['point']]}, normal "
          f"{[round(v, 4) for v in h['normal']]} (the sphere at x = {x:.4f}: {[round(v, 4) for v in expected]}, "
          f"{[round(v, 4) for v in normal]}), fraction {h['fraction']:.5f}")
    check(as_stood(h, stood((x, 1.2, 0.0))), "  the original's own hit of him standing still there")
    check(new["same"] and old["same"], "  the world untouched: the body is tested at a copy of its transform")
    # the frame: ray_test's result at ESP+60h and check_collision's copy at +38h aside
    check(new["regs"] == old["regs"] and new["frame"][:0x38] == old["frame"][:0x38]
          and new["frame"][0x88:] == old["frame"][0x88:],
          "  the registers and the frame the original's (the result and its copy aside)")

    # -- walls ----------------------------------------------------------
    def wall_in_front(w):
        parts = moving(w)
        w.wall((-0.2, 1.2, 5.0), 0.5)
        return parts
    old, new = compare("a moving target behind a wall: the wall, in both", wall_in_front)
    check(hit(old)["object"] != 0, "  (the wall is hit)")

    def wall_behind(w):
        parts = moving(w)
        parts["wall"] = w.wall((-0.2, 1.2, -5.0), 0.5)
        return parts
    (_, _, old), (_, parts, new) = both(original, built, wall_behind, ORIGIN, DIRECTION, LENGTH, WHOLE)
    check(hit(old)["object"] == parts["wall"]["game"] and hit(new)["object"] == parts["target"]["game"],
          "a wall behind the moving target: the original the wall, the patch him")

    # -- who is never put back --------------------------------------------
    def own(w):
        return {"shooter": w.player(0, (0.25, 1.2, 0.0), moved=(0.25, 0.0, 0.0), own=True)}
    old, new = compare("the shooter's own body moving across the ray: never hit", own)
    check(hit(new)["object"] == 0, "  (nothing hit)")

    def spawned(w):
        return {"shooter": shooter(w), "target": w.player(0, (0.25, 1.2, 0.0), moved=(0.25, 0.0, 0.0),
                                                          spawned=G - 10)}
    compare("a target spawned in the step: not put back", spawned)

    def dead(w):
        return {"shooter": shooter(w), "target": w.player(0, (0.25, 1.2, 0.0), moved=(0.25, 0.0, 0.0),
                                                          alive=False)}
    compare("a dead target: not put back", dead)

    # -- a step in parts --------------------------------------------------
    # the world stepped 1030 -> 1050 last (an off-grid input at 1030); he
    # moved 0.2 m in it, at its end at x = 0.15 (0.35 m from the ray:
    # missed); crossed 35.7 ms before the end, before the part began: back
    # by the part's whole move, at x = -0.05 (on the ray), no further
    def split(w):
        return {"shooter": shooter(w), "target": w.player(0, (0.15, 1.2, 0.0), moved=(0.2, 0.0, 0.0))}
    (_, _, old), (_, parts, new) = both(original, built, split, ORIGIN, DIRECTION, LENGTH, WHOLE, step_from=G - 20)
    dz = math.sqrt(0.3 ** 2 - (-0.05 - ORIGIN[0]) ** 2)
    check(hit(old)["object"] == 0 and hit(new)["object"] == parts["target"]["game"]
          and abs(hit(new)["point"][2] - dz) < 2e-3 and as_stood(hit(new), stood((f32(0.15) - f32(0.2), 1.2, 0.0))),
          f"a step in parts (20 ms): back by the part's move, no further (hit at z {hit(new)['point'][2]:.4f}, "
          f"the sphere at x = -0.05: {dz:.4f})")

    # -- along z (the frame's sign) --------------------------------------
    # a ray along +x through z = 0.2: he moved 25 cm along +z, at its end
    # at z = 0.55 (0.35 m off: missed), at the crossing (x = 0, 10 m in)
    # back at z = 0.3714 (0.17 m: met)
    def along_z(w):
        return {"shooter": shooter(w), "target": w.player(0, (0.0, 1.2, 0.55), moved=(0.0, 0.0, 0.25))}
    (_, _, old), (_, parts, new) = both(original, built, along_z, (-10.0, 1.2, 0.2), (1.0, 0.0, 0.0), LENGTH, WHOLE)
    z = 0.55 - 0.25 * share
    dx = math.sqrt(0.3 ** 2 - (z - 0.2) ** 2)
    check(hit(old)["object"] == 0 and hit(new)["object"] == parts["target"]["game"]
          and abs(hit(new)["point"][0] + dx) < 2e-3 and abs(hit(new)["point"][2] - 0.2) < 1e-5
          and as_stood(hit(new), stood((0.0, 1.2, z), (-10.0, 1.2, 0.2), (1.0, 0.0, 0.0))),
          f"a move along z: put back along z (Bullet's z is the game's -z): hit at "
          f"{[round(v, 4) for v in hit(new)['point']]}")

    if FAILED:
        sys.exit(f"{len(FAILED)} failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
