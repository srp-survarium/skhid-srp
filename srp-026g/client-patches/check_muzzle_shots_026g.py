#!/usr/bin/env python3
"""Run patch 200's shot (the Rust patch `muzzle_shots`) in the default build
beside the original, on a made-up weapon and user.

    python check_muzzle_shots_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, and
weapon_core::instant_fire (0x2E23A0) is entered at its copy of the user's
eyes' transform (0x2E243D: mov esi,[ebx+8] ... rep movsd) and stopped at its
call of get_dispersed_bullet_direction (0x2E245C), the copy in its frame
(ESP+0Ch there: the matrix argument). Two engine functions are stand-ins:
animation_player::compute_bone_matrix (0x391A20) gives each bone record the
matrix this file made for it, and bullet_physics_world::ray_test (0x75E20)
meets a wall or nothing.

The user: a skeleton with `Weapon` and `Item`, a tree, a transform in the
world. The gun: a skeleton of object_movement, a root and a body bone; its
aim locator on the root, its fire point on the body; an aim offset.

- From the hip the copy becomes: the fire point's place on the gun on the
  half-turned Weapon bone, looking along the barrel (the fire point's -z).
- Fully aimed: the gun where its aim (offset x half turn x locator x bone) is
  the Item bone; from the fire point, along the eye's line.
- Half raised: between them (the turn along its arc, the place along the line).
- A wall between the eye and the muzzle: from the eye, the same way.
- A gun not held ready (its barrel more than 5 degrees off the eye's line),
  a bare weapon_core, a user with no tree, a gun whose skeleton lacks the
  locator's bone, a user with no `Weapon` bone: the copy is the eye's, bit
  for bit.
- The bones are asked as the game asks them: the user's own animation player,
  each record in its own skeleton, the user or the weapon the animated
  object, an empty resolver; the ray with the shot collision's filter.
In every run the registers at the call are the original's.
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

COPY, SITE, CALL = 0x2E243D, 0x2E2456, 0x2E245C
SPAN = bytes.fromhex("8d4c242c51")                         # lea ecx,[esp+2Ch] / push ecx
BONE_MATRIX, RAY_TEST = 0x391A20, 0x75E20
WEAPON_VTABLE, WEAPON_CORE_VTABLE = 0xA04174, 0xA11094
THREAD_SLOT = 0x870118
HEAP, STUBS, STACK = 0x30000000, 0x31000000, 0x32000000
WEAPON, USER, TREE, SKELETON, GUN_SKELETON, NAMES, PHYSICS = (HEAP + 0x1000 * n for n in (0, 4, 0x14, 0x15, 0x16, 0x17, 0x18))
OBJECT, VTABLE, MATERIAL_OF = STUBS + 0x200, STUBS + 0x300, STUBS + 0xB0
SP = STACK + 0x8000                                        # instant_fire's ESP after its prologue
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP)
STEADY = {UC_X86_REG_EAX: 0x0A0A0A0A, UC_X86_REG_ECX: 0x0C0C0C0C, UC_X86_REG_EDX: 0x0D0D0D0D, UC_X86_REG_ESI: 0x05050505,
          UC_X86_REG_EDI: 0x0E0E0E0E, UC_X86_REG_EBP: STACK + 0x9000}
BONE, NO_BONE = 0x1C, 0xFFFF
GROUP, MASK = 0x10, 0x08                                   # the collision bullets hit

failed = 0


def check(ok, what):
    global failed
    failed += not ok
    print(f"  {'ok' if ok else 'FAIL'}  {what}")


def matrix(turn_x=0.0, turn_y=0.0, place=(0.0, 0.0, 0.0)):
    """A row-vector 4x4: a turn about x, one about y, then a move."""
    c, s = math.cos(turn_x), math.sin(turn_x)
    x = (1, 0, 0, 0, 0, c, s, 0, 0, -s, c, 0, 0, 0, 0, 1)
    c, s = math.cos(turn_y), math.sin(turn_y)
    y = (c, 0, -s, 0, 0, 1, 0, 0, s, 0, c, 0, 0, 0, 0, 1)
    out = list(product(x, y))
    out[12:15] = place
    return tuple(out)


def product(a, b):
    return tuple(sum(a[r * 4 + k] * b[k * 4 + c] for k in range(4)) for r in range(4) for c in range(4))


def inverse(m):
    """Of a turn and a move."""
    turn = [m[c * 4 + r] for r in range(3) for c in range(3)]            # the 3x3's transpose
    out = [0.0] * 16
    for r in range(3):
        out[r * 4:r * 4 + 3] = turn[r * 3:r * 3 + 3]
    out[12:15] = [-sum(m[12 + k] * turn[k * 3 + c] for k in range(3)) for c in range(3)]
    out[15] = 1.0
    return tuple(out)


def near(a, b, by=3e-5):
    return all(abs(x - y) <= by for x, y in zip(a, b))


def unit(v):
    n = math.sqrt(sum(x * x for x in v))
    return tuple(x / n for x in v)


HALF_TURN = (-1, 0, 0, 0, 0, 1, 0, 0, 0, 0, -1, 0, 0, 0, 0, 1)
WORLD = matrix(turn_y=0.7, place=(100.0, 6.5, -50.0))                     # the user in the world
HAND = matrix(0.15, math.pi, (0.18, 1.32, 0.31))                          # the Weapon bone, the user's space: held ready, 2 degrees off the eye's line
LOW = matrix(0.12, math.pi + 0.2, (0.18, 1.32, 0.31))                     # not held ready: 9.7 degrees off
ITEM = matrix(-0.15, 0.03, (0.0, 1.62, 0.13))                            # the Item bone: the eyes
EYE = product(matrix(-0.4, 0.05, (0.0, 1.6, 0.1)), WORLD)                # the copy the game makes: an older eye
AIM_OFFSET = matrix(place=(0.0, 0.0, -0.11))
AIM = matrix(0.009, math.pi, (0.0089, 0.1019, 0.1328))                   # the aim's place on the gun's root
BARREL = matrix(0.004, math.pi, (0.0, 0.031, 0.62))                      # the fire point's on its body
GUN_ROOT = matrix()
GUN_BODY = matrix(-0.0148, 0.0, (0.0088, 0.0214, 0.05))                   # as a clip poses the body


def expected(raised, blocked=False):
    """The transform a shot leaves from (muzzle_shots.rs, in this file's own arithmetic)."""
    hip = product(HALF_TURN, HAND)
    sight = product(product(AIM_OFFSET, HALF_TURN), product(AIM, GUN_ROOT))
    aimed = product(inverse(sight), ITEM)
    if raised <= 0:
        held = hip
    elif raised >= 1:
        held = aimed
    else:
        held = blend(hip, aimed, raised)
    eye = product(ITEM, WORLD)
    fire = product(product(BARREL, GUN_BODY), product(held, WORLD))
    barrel, look = unit(tuple(-x for x in fire[8:11])), unit(eye[8:11])
    along = unit(tuple(b * (1 - raised) + e * raised for b, e in zip(barrel, look)))
    dot = lambda a, b: sum(x * y for x, y in zip(a, b))
    across = lambda v, of: tuple(x - o * dot(v, of) for x, o in zip(v, of))
    right = unit(across(eye[0:3], along))
    up = unit(across(across(eye[4:7], along), right))
    place = eye[12:15] if blocked else fire[12:15]
    return (*right, 0, *up, 0, *along, 0, *place, 1), eye[12:15], fire[12:15]


def quaternion(m):
    """Of a turn's rows (the images of x, y, z)."""
    x, y, z = m[0:3], m[4:7], m[8:11]
    trace = x[0] + y[1] + z[2]
    assert trace > 0, "the made-up poses' turns are small"
    s = math.sqrt(trace + 1.0) * 2.0
    return ((y[2] - z[1]) / s, (z[0] - x[2]) / s, (x[1] - y[0]) / s, 0.25 * s)


def rotate(q, v):
    u, w = q[:3], q[3]
    cross = lambda a, b: (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])
    t = tuple(2 * c for c in cross(u, v))
    return tuple(a + b * w + c for a, b, c in zip(v, t, cross(u, t)))


def blend(a, b, t):
    qa, qb = quaternion(a), quaternion(b)
    if sum(x * y for x, y in zip(qa, qb)) < 0:
        qb = tuple(-x for x in qb)
    q = unit(tuple(x + (y - x) * t for x, y in zip(qa, qb)))
    rows = [rotate(q, axis) for axis in ((1, 0, 0), (0, 1, 0), (0, 0, 1))]
    place = tuple(x + (y - x) * t for x, y in zip(a[12:15], b[12:15]))
    return (*rows[0], 0, *rows[1], 0, *rows[2], 0, *place, 1)


class Game:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(HEAP, 0x20000, UC_PROT_ALL)
        for at in (STUBS, STACK):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        self.u32(THREAD_SLOT, STUBS + 0x80)
        emu.mem_write(STUBS + 0x80, b"\xB8\x01\x00\x00\x00\xC3")
        emu.hook_add(UC_HOOK_CODE, self.bone_matrix, begin=BONE_MATRIX, end=BONE_MATRIX)
        emu.hook_add(UC_HOOK_CODE, self.ray_test, begin=RAY_TEST, end=RAY_TEST)
        self.u32(OBJECT, VTABLE)
        self.u32(VTABLE + 0x10, MATERIAL_OF)
        emu.mem_write(MATERIAL_OF, b"\xB8\x0B\x00\x00\x00\xC2\x08\x00")   # get_triangle_material: 11, a wall's
        self.bones, self.asked, self.rays, self.wall = {}, [], [], False

    def u32(self, at, value=None):
        if value is None:
            return struct.unpack("<I", self.emu.mem_read(at, 4))[0]
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def floats(self, at, n=16):
        return struct.unpack(f"<{n}f", self.emu.mem_read(at, 4 * n))

    def put(self, at, m):
        self.emu.mem_write(at, struct.pack("<16f", *m))

    def skeleton(self, at, names, texts):
        """A skeleton's records: object_movement first, its child the next, each with its name (at `texts`)."""
        self.u32(at + 0x108, len(names))
        for n, name in enumerate(names):
            text = texts + 0x40 * n
            self.emu.mem_write(text, name.encode() + b"\0")
            record = at + 0x110 + BONE * n
            self.u32(record, text)
            self.u32(record + 4, at + 0x110 + BONE * max(n - 1, 0))
            self.u32(record + 8, at + 0x110 + BONE * (n + 1))
        return [at + 0x110 + BONE * n for n in range(len(names))]

    def shot(self, raised, hand=HAND, vtable=WEAPON_VTABLE, tree=True, user_bones=("object_movement", "Root", "Weapon", "Item"),
             gun_bones=("object_movement", "root", "offset"), wall=False):
        """A shot of a new weapon: (the copy in the frame at the call, the registers there)."""
        emu = self.emu
        emu.mem_write(HEAP, bytes(0x20000))
        self.bones, self.asked, self.rays, self.wall = {}, [], [], wall
        self.u32(WEAPON, vtable)
        self.u32(WEAPON + 0x8, USER)
        self.u32(WEAPON + 0x590, GUN_SKELETON)
        emu.mem_write(WEAPON + 0x5FC, struct.pack("<f", raised))
        self.put(WEAPON + 0x6A0, AIM_OFFSET)
        for locator, place, bone in ((0x6E0, BARREL, 1), (0x7A8, AIM, 0)):
            self.put(WEAPON + locator + 0x20, place)
            emu.mem_write(WEAPON + locator + 0x60, struct.pack("<H", bone))
        self.u32(USER + 0x350, PHYSICS)
        self.u32(USER + 0x408, SKELETON)
        self.put(USER + 0x40C, WORLD)
        self.put(USER + 0x44C, EYE)
        self.u32(USER + 0xE850, TREE if tree else 0)
        user = dict(zip(user_bones, self.skeleton(SKELETON, user_bones, NAMES)))
        self.bones.update({user.get("Weapon"): (hand, SKELETON, USER), user.get("Item"): (ITEM, SKELETON, USER)})
        gun = self.skeleton(GUN_SKELETON, gun_bones, NAMES + 0x800)
        self.bones.update({record: (pose, GUN_SKELETON, WEAPON) for record, pose in zip(gun[1:], (GUN_ROOT, GUN_BODY))})
        emu.mem_write(SP - 0x100, bytes(0x300))
        for reg, value in STEADY.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_EBX, WEAPON)
        emu.reg_write(UC_X86_REG_ESP, SP)
        emu.emu_start(COPY, CALL, count=2000000)
        regs = {r: emu.reg_read(r) for r in REGS}
        return self.floats(self.u32(regs[UC_X86_REG_ESP] + 8)), regs

    def bone_matrix(self, emu, *_):
        """animation_player::compute_bone_matrix (`ret 10h`): the record's matrix as this file made it."""
        esp = emu.reg_read(UC_X86_REG_ESP)
        ret, out, skeleton, animated, resolver = (self.u32(esp + 4 * i) for i in range(5))
        record = emu.reg_read(UC_X86_REG_EDX)
        self.asked.append((emu.reg_read(UC_X86_REG_EAX), record, skeleton, animated, self.u32(resolver)))
        self.put(out, self.bones[record][0])
        emu.reg_write(UC_X86_REG_EAX, out)
        emu.reg_write(UC_X86_REG_ESP, esp + 4 + 0x10)
        emu.reg_write(UC_X86_REG_EIP, ret)

    def ray_test(self, emu, *_):
        """bullet_physics_world::ray_test (thiscall, `ret 20h`): the wall halfway, or nothing."""
        esp = emu.reg_read(UC_X86_REG_ESP)
        ret, result, origin, direction = (self.u32(esp + 4 * i) for i in range(4))
        length = struct.unpack("<f", emu.mem_read(esp + 16, 4))[0]
        group, mask = self.u32(esp + 20), self.u32(esp + 24)
        o, d = self.floats(origin, 3), self.floats(direction, 3)
        self.rays.append((emu.reg_read(UC_X86_REG_ECX), o, d, length, group & 0xFFFF, mask & 0xFFFF))
        emu.mem_write(result, bytes(0x28))
        if self.wall:
            emu.mem_write(result, struct.pack("<3f", *(a + b * length / 2 for a, b in zip(o, d))))
            emu.mem_write(result + 0xC, struct.pack("<f", 0.5))
            self.u32(result + 0x1C, OBJECT)
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, esp + 4 + 0x20)
        emu.reg_write(UC_X86_REG_EIP, ret)


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
    check(opcode == 0xE9 and old == SPAN and original[at:at + len(SPAN)] == SPAN and built[at:at + len(new)] == new,
          f"{va:#x}: {old.hex()} in the original, `jmp {symbol}` here")

    stock, ours = Game(original), Game(built)
    eye = struct.unpack("<16f", struct.pack("<16f", *EYE))

    for what, raised in (("from the hip", 0.0), ("fully aimed", 1.0), ("half raised", 0.5)):
        s_out, s_regs = stock.shot(raised)
        o_out, o_regs = ours.shot(raised)
        want, eye_place, muzzle = expected(raised)
        check(s_out == eye and s_regs[UC_X86_REG_EIP] == CALL, f"{what}: the original leaves the eye's copy")
        check(o_regs == s_regs, f"{what}: the registers at the call the original's")
        check(near(o_out, want), f"{what}: from the muzzle, the way the gun looks")
        if raised == 0.0:
            fire = product(product(BARREL, GUN_BODY), product(product(HALF_TURN, HAND), WORLD))
            check(near(o_out[8:11], unit(tuple(-x for x in fire[8:11]))) and near(o_out[12:15], fire[12:15]),
                  "from the hip: along the fire point's -z, from its place on the half-turned Weapon bone")
        if raised == 1.0:
            look = product(ITEM, WORLD)
            check(near(o_out[8:11], unit(look[8:11])), "fully aimed: along the Item bone's forward now, not the copy's")
            under = sum((m - e) * u for m, e, u in zip(o_out[12:15], look[12:15], look[4:7]))
            check(-0.12 < under < -0.03, f"fully aimed: the muzzle {-under * 100:.1f} cm under the eye's line")
        asked = sorted(ours.asked)
        player = [a for a in asked if a[3] == USER]
        gun = [a for a in asked if a[3] == WEAPON]
        check(len(player) == 2 and len(gun) == 2 and all(a[0] == USER + 0x750 and a[4] == 0 for a in asked)
              and all(a[2] == SKELETON for a in player) and all(a[2] == GUN_SKELETON for a in gun),
              f"{what}: Weapon and Item of the user, the two locators' bones of the gun, each from the user's animation player with an empty resolver")
        check(sorted(a[1] for a in gun) == [GUN_SKELETON + 0x110 + BONE, GUN_SKELETON + 0x110 + 2 * BONE],
              f"{what}: a locator's bone is its index past the first child's record")
        ray = ours.rays
        length = math.dist(eye_place, muzzle)
        check(len(ray) == 1 and ray[0][0] == PHYSICS and ray[0][4:] == (GROUP, MASK) and near(ray[0][1], eye_place, 1e-4)
              and abs(ray[0][3] - length) < 1e-4 and near(ray[0][2], unit(tuple(m - e for m, e in zip(muzzle, eye_place))), 1e-4),
              f"{what}: one ray of the shot collision from the eye to the muzzle ({length * 100:.0f} cm)")
        check(not stock.asked and not stock.rays, f"{what}: the original asks nothing")

    o_out, o_regs = ours.shot(0.0, wall=True)
    want, _, _ = expected(0.0, blocked=True)
    check(near(o_out, want) and near(o_out[12:15], product(ITEM, WORLD)[12:15]), "a wall between the eye and the muzzle: from the eye, the same way")

    for what, shot in (("a bare weapon_core", dict(vtable=WEAPON_CORE_VTABLE)),
                       ("a user with no tree", dict(tree=False)),
                       ("a user with no Weapon bone", dict(user_bones=("object_movement", "Root", "Item"))),
                       ("a gun whose skeleton ends before the fire point's bone", dict(gun_bones=("object_movement", "root")))):
        s_out, s_regs = stock.shot(0.5, **shot)
        o_out, o_regs = ours.shot(0.5, **shot)
        check(o_out == eye and o_out == s_out and o_regs == s_regs, f"{what}: the eye's copy bit for bit, the registers the original's")
    # Not held ready from the hip (9.7 degrees off the eye's line): the game's own shot.
    s_out, s_regs = stock.shot(0.0, hand=LOW)
    o_out, o_regs = ours.shot(0.0, hand=LOW)
    check(o_out == eye and o_out == s_out and o_regs == s_regs,
          "a gun not held ready: the eye's copy bit for bit, the registers the original's")
    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
