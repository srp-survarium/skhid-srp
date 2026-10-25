#!/usr/bin/env python3
"""Run patch 76's four hooks (the death ragdoll; ../docs/notes/ragdoll.md)
in the built exe, with Unicorn (2.1.4).

    python check_ragdoll_026g.py <original survarium.exe>

The exe is built as patch_026g.py builds it (with the late join and the
arena); the build without the late join keeps the original's four sites.
Each hook runs from its site in the build to the game's next instruction:
game_world::tick 0x1F7C3C (the step), mt_draw 0x24757C (the cull) and
0x247716 (the bones), update_camera 0x1F3643 (the eye). The step reaches the
places through the game's own network client code (match_options 0x219990,
get_active_player 0x27A9C0 and active_player_ptr 0x2D0AB0, from the client's
vtable 0x9FFB14) on a made-up client, world core and match options; the
players are made up with the fields the step reads (ragdoll.md section 8),
their skeleton is scavengers_01 in the cook's order (ragdoll.md section 8,
`add_bone` 0x3ACDA0), the bones a standing body. Stubs: ray_test (0x75E20,
a floor at y = 0; the filter checked: group 24h, mask 2, no delegate) and
QueryPerformanceCounter / -Frequency (60 frames a second).

Checked:
- the four sites: the original's bytes in the original and the build
  without the late join, a call into ragdoll.code here;
- every hook keeps the registers the game's code after it reads, and the
  flags of the two compares it stands in for (the step's `[esp+0Fh]`, the
  cull's `[ebx+0F4E2h]`): the original's answer whenever no corpse of ours
  is the player;
- a death: the clip is drawn as the game's for two frames (captured), then
  the ragdoll's bones replace it; the cull draws the corpse; the local
  player's view gets the ragdoll's eye (level: no roll), never +44Ch, and
  another player's never;
- a rollback that undoes the death (the world shows the player alive again)
  drops the ragdoll at once: the bones and the eye are the game's;
- a death simulated again as the same death (the same life, +328h) keeps
  the ragdoll as it was (no new start from the clip); another death (a new
  life) starts from the clip again;
- a skeleton that is not ours (a bone missing) gets no ragdoll;
- the ragdoll falls onto the floor and settles there; the level is read
  only through ray_test with the level's filter.
"""
import math
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_HOOK_BLOCK, UC_HOOK_CODE, UC_MODE_32, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EFLAGS, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

STEP_SITE, STEP_NEXT = 0x1F7C3C, 0x1F7C41
CULL_SITE, CULL_NEXT = 0x24757C, 0x247583
APPLY_SITE, APPLY_NEXT = 0x247716, 0x24771C
EYE_SITE, EYE_NEXT = 0x1F3643, 0x1F3649
RAY_TEST, QPC_SLOT, QPF_SLOT = 0x75E20, 0x870158, 0x870150
CLIENT_VTABLE = 0x9FFB14
ZF = 1 << 6

# made-up memory
HEAP, HEAP_SIZE = 0x40000000, 0x01000000
CLIENT = HEAP                                  # +1F1C40h, +1F1C48h: 2 MiB in
WORLD, GAME, OPTIONS, NC_CORE = HEAP + 0x300000, HEAP + 0x301000, HEAP + 0x310000, HEAP + 0x380000
PLACES_ARRAY, NAMES, SKELETON, OTHER_SKELETON = HEAP + 0x3B0000, HEAP + 0x3B1000, HEAP + 0x3B2000, HEAP + 0x3B3000
MODEL, OTHER_MODEL, ARRAY, FRAME = HEAP + 0x3B4000, HEAP + 0x3B5000, HEAP + 0x3B6000, HEAP + 0x3B9000
PLAYERS = HEAP + 0x400000                      # 0x10000 each
STUBS, STACK, RET = 0x48000000, 0x48300000, 0x48400000
PHYSICS = 0x5A5A0000                           # a made-up physics world (ray_test's ECX)

# player (ragdoll.md section 8)
REFS, PLACE, HELD, SPAWNED, ALIVE, INSERTED, PHYSICS_WORLD = 0xD0, 0x114, 0x128, 0x328, 0x33C, 0x33F, 0x350
TRANSFORM, EYE, MODEL_FIELD, INPUT, NOT_SEEN = 0x40C, 0x44C, 0xF290, 0xF470, 0xF4E2

# scavengers_01 in the cook's order: (name, parent's array index); the
# array index is the skeleton's index - 1 (object_movement has no matrix)
COOKED = [
    ("Root", -1), ("Hip", 0), ("Weapon", 0), ("Pistol", 0), ("Item", 0), ("LeftUpLeg", 1), ("RightUpLeg", 1),
    ("Spin", 1), ("LeftUpLeg_Roll", 5), ("LeftKnee", 5), ("LeftLeg", 9), ("LeftFoot", 10), ("LeftToe", 11),
    ("RightUpLeg_Roll", 6), ("RightKnee", 6), ("RightLeg", 14), ("RightFoot", 15), ("RightToe", 16),
    ("Spin_1", 7), ("Chest", 18), ("RightClavicle", 19), ("Neck", 19), ("LeftClavicle", 19), ("RightArm", 20),
    ("RightShoulder", 20), ("RightForeArm", 23), ("RightArm_Roll", 23), ("RightForeArm_Roll", 25),
    ("RightHand", 25), ("RightIndex1", 28), ("RightThumb1", 28), ("RightRing1", 28), ("RightMiddle1", 28),
    ("RightPinky1", 28), ("RightIndex2", 29), ("RightIndex3", 34), ("RightThumb2", 30), ("RightThumb3", 36),
    ("RightRing2", 31), ("RightRing3", 38), ("RightMiddle2", 32), ("RightMiddle3", 40), ("RightPinky2", 33),
    ("RightPinky3", 42), ("Head", 21), ("LeftEye", 44), ("Jaw", 44), ("RightEye", 44), ("LeftShoulder", 22),
    ("LeftArm", 22), ("LeftArm_Roll", 49), ("LeftForeArm", 49), ("LeftHand", 51), ("LeftForeArm_Roll", 51),
    ("LeftRing1", 52), ("LeftIndex1", 52), ("LeftThumb1", 52), ("LeftMiddle1", 52), ("LeftPinky1", 52),
    ("LeftRing2", 54), ("LeftRing3", 59), ("LeftIndex2", 55), ("LeftIndex3", 61), ("LeftThumb2", 56),
    ("LeftThumb3", 63), ("LeftMiddle2", 57), ("LeftMiddle3", 65), ("LeftPinky2", 58), ("LeftPinky3", 67),
]
# a standing body (model space, y up): the places the ragdoll's parts use
STANDING = {
    "Root": (0, 0, 0), "Hip": (0, 1.0, 0), "LeftUpLeg": (0.1, 0.95, 0), "LeftKnee": (0.1, 0.53, 0),
    "LeftLeg": (0.1, 0.52, 0), "LeftFoot": (0.1, 0.09, 0), "LeftToe": (0.1, 0.03, 0.14), "Spin": (0, 1.1, 0),
    "Spin_1": (0, 1.22, 0), "Chest": (0, 1.34, 0), "LeftClavicle": (0.04, 1.48, 0), "LeftArm": (0.19, 1.45, 0),
    "LeftForeArm": (0.21, 1.17, 0.03), "LeftHand": (0.22, 0.93, 0.06), "Neck": (0, 1.53, 0),
    "Head": (0, 1.62, 0), "Weapon": (-0.2, 1.0, 0.3), "Item": (0, 1.64, 0.08),
}


def standing_place(index):
    name, parent = COOKED[index]
    if name in STANDING:
        return STANDING[name]
    if name.startswith("Right") and "Left" + name[5:] in STANDING:
        x, y, z = STANDING["Left" + name[5:]]
        return (-x, y, z)
    px, py, pz = standing_place(parent)
    return (px, py, pz + 0.02)


def matrix(position, turn=0.0):
    """A row-vector affine matrix: a turn about y, then the place."""
    c, s = math.cos(turn), math.sin(turn)
    return [c, 0, -s, 0, 0, 1, 0, 0, s, 0, c, 0, position[0], position[1], position[2], 1]


class Game:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(HEAP, HEAP_SIZE, UC_PROT_ALL)
        for at in (STUBS, STACK - 0x100000, RET):
            emu.mem_map(at, 0x100000, UC_PROT_ALL)
        self.ticks, self.rays = 0, []
        # the imports: QueryPerformanceCounter and -Frequency (stdcall, 4)
        emu.mem_write(QPC_SLOT, struct.pack("<I", STUBS))
        emu.mem_write(QPF_SLOT, struct.pack("<I", STUBS + 0x10))
        emu.mem_write(STUBS, b"\xC2\x04\x00")
        emu.mem_write(STUBS + 0x10, b"\xC2\x04\x00")
        emu.hook_add(UC_HOOK_CODE, self.counter, begin=STUBS, end=STUBS)
        emu.hook_add(UC_HOOK_CODE, self.frequency, begin=STUBS + 0x10, end=STUBS + 0x10)
        emu.hook_add(UC_HOOK_CODE, self.ray_test, begin=RAY_TEST, end=RAY_TEST)
        # the world, the game, the network client (the game's own vtable)
        self.u32(WORLD + 0x9C, GAME)
        self.u32(GAME + 0x53AC, CLIENT)
        self.u32(CLIENT, CLIENT_VTABLE)
        self.u32(CLIENT + 0x1F1C48, OPTIONS)
        self.u32(CLIENT + 0x1F1C40, NC_CORE)
        self.core = NC_CORE + 0x1F8
        # the skeleton: object_movement, then the cooked bones
        names = NAMES
        for skeleton, missing in ((SKELETON, None), (OTHER_SKELETON, "LeftKnee")):
            base_at = skeleton + 0x110
            self.u32(skeleton + 0x108, len(COOKED) + 1)
            entries = [("object_movement", None)] + [(n, p + 1) for n, p in COOKED]
            for g, (name, parent) in enumerate(entries):
                name = "Unrelated" if name == missing else name
                emu.mem_write(names, name.encode() + b"\0")
                self.u32(base_at + 0x1C * g, names)
                self.u32(base_at + 0x1C * g + 4, 0 if parent is None else base_at + 0x1C * parent)
                names += len(name) + 1
        self.u32(MODEL + 0x10C, SKELETON)
        self.u32(OTHER_MODEL + 0x10C, OTHER_SKELETON)
        self.players = []

    # -- memory --------------------------------------------------------------
    def u32(self, at, value=None):
        if value is None:
            return struct.unpack("<I", self.emu.mem_read(at, 4))[0]
        self.emu.mem_write(at, struct.pack("<I", value))

    def u8(self, at, value):
        self.emu.mem_write(at, bytes([value]))

    def floats(self, at, count):
        return struct.unpack(f"<{count}f", self.emu.mem_read(at, 4 * count))

    # -- stubs ---------------------------------------------------------------
    def counter(self, emu, *_):
        out = self.u32(emu.reg_read(UC_X86_REG_ESP) + 4)
        emu.mem_write(out, struct.pack("<q", self.ticks))
        emu.reg_write(UC_X86_REG_EAX, 1)

    def frequency(self, emu, *_):
        out = self.u32(emu.reg_read(UC_X86_REG_ESP) + 4)
        emu.mem_write(out, struct.pack("<q", 6_000_000))
        emu.reg_write(UC_X86_REG_EAX, 1)

    def ray_test(self, emu, *_):
        esp = emu.reg_read(UC_X86_REG_ESP)
        ret, result, origin, direction = (self.u32(esp + 4 * i) for i in range(4))
        length = struct.unpack("<f", emu.mem_read(esp + 16, 4))[0]
        group, mask, delegate, unflipped = (self.u32(esp + 20 + 4 * i) for i in range(4))
        assert emu.reg_read(UC_X86_REG_ECX) == PHYSICS, "ray_test on the player's physics world"
        assert (group, mask, delegate) == (0x24, 2, 0), f"the level's filter: {group:#x} {mask:#x} {delegate}"
        o = self.floats(origin, 3)
        d = self.floats(direction, 3)
        assert abs(math.sqrt(sum(x * x for x in d)) - 1) < 1e-3, "a unit direction"
        self.rays.append((o, d, length))
        hit = 0
        if d[1] < 0 and o[1] >= 0:
            t = -o[1] / d[1]
            if t <= length:
                p = (o[0] + d[0] * t, 0.0, o[2] + d[2] * t)
                emu.mem_write(result, struct.pack("<3ff3fI", *p, t / length, 0.0, 1.0, 0.0, 0x77770000))
                hit = 1
        if not hit:
            emu.mem_write(result + 0x1C, struct.pack("<I", 0))
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, esp + 4 + 32)
        emu.reg_write(UC_X86_REG_EIP, ret)

    # -- the made-up match -----------------------------------------------------
    def add_player(self, place, x, z, model=MODEL):
        player = PLAYERS + 0x10000 * len(self.players)
        self.players.append(player)
        self.u32(player + REFS, 1)
        self.u8(player + PLACE, place)
        self.u32(player + HELD, 0x12340000)
        self.u32(player + PHYSICS_WORLD, PHYSICS)
        self.u32(player + MODEL_FIELD, model)
        self.u32(player + INPUT, 0x1A2B3C4D)
        self.u8(player + INSERTED, 1)
        self.set_alive(player, True, 1000)
        self.emu.mem_write(player + TRANSFORM, struct.pack("<16f", *matrix((x, 0, z))))
        self.emu.mem_write(player + EYE, struct.pack("<16f", *matrix((x, 1.64, z + 0.08))))
        record = PLACES_ARRAY + 0x30 * place
        self.u32(record, player)
        self.u8(record + 0xC, 0)
        count = max(p for p in range(32) if self.u32(PLACES_ARRAY + 0x30 * p) != 0) + 1
        self.u32(self.core + 0x18458, PLACES_ARRAY)
        self.u32(self.core + 0x1845C, PLACES_ARRAY + 0x30 * count)
        self.u32(self.core + 0x18FD8, (1 << count) - 1)
        self.u8(OPTIONS + 0xC + 0x577B5, count)
        return player

    def set_alive(self, player, alive, spawned):
        self.u8(player + ALIVE, 1 if alive else 0)
        self.u32(player + SPAWNED, spawned)

    def set_local(self, player):
        self.u32(CLIENT + 4, player)

    # -- the hooks -------------------------------------------------------------
    def run(self, site, stop, regs, stack=b""):
        emu = self.emu
        esp = STACK - 0x1000
        emu.mem_write(esp, stack.ljust(0x40, b"\0"))
        emu.reg_write(UC_X86_REG_ESP, esp)
        for reg, value in regs.items():
            emu.reg_write(reg, value)
        try:
            emu.emu_start(site, stop, count=40_000_000)
        except Exception as error:
            trail = []
            hook = emu.hook_add(UC_HOOK_BLOCK, lambda _e, address, *_: trail.append(address))
            emu.reg_write(UC_X86_REG_ESP, esp)
            for reg, value in regs.items():
                emu.reg_write(reg, value)
            try:
                emu.emu_start(site, stop, count=40_000_000)
            except Exception:
                pass
            emu.hook_del(hook)
            sys.exit(f"FAIL: {site:#x}: {error} at {emu.reg_read(UC_X86_REG_EIP):#x} "
                     f"(esp {emu.reg_read(UC_X86_REG_ESP):#x}); the last blocks "
                     + " ".join(f"{a:#x}" for a in trail[-12:]))
        assert emu.reg_read(UC_X86_REG_EIP) == stop, f"{site:#x}: stopped at {emu.reg_read(UC_X86_REG_EIP):#x}"
        assert emu.reg_read(UC_X86_REG_ESP) == esp, f"{site:#x}: the stack is not balanced"
        return {r: emu.reg_read(r) for r in (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX,
                                              UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_EFLAGS)}

    MARKS = {UC_X86_REG_EAX: 0xA1, UC_X86_REG_ECX: 0xC1, UC_X86_REG_EDX: 0xD1, UC_X86_REG_ESI: 0x51,
             UC_X86_REG_EDI: 0xD7, UC_X86_REG_EBP: 0xB9}

    def step(self, flag=0):
        """A frame of game_world::tick at the join after move: 1/60 s."""
        self.ticks += 100_000
        regs = {**self.MARKS, UC_X86_REG_EBX: WORLD}
        out = self.run(STEP_SITE, STEP_NEXT, regs, bytes(0xF) + bytes([flag]))
        for reg, value in regs.items():
            assert out[reg] == value, f"rg_step changed register {reg}"
        assert bool(out[UC_X86_REG_EFLAGS] & ZF) == (flag == 0), "rg_step's compare of [esp+0Fh]"

    def cull(self, player, not_seen):
        """mt_draw's cull: True when the je draws."""
        self.u8(player + NOT_SEEN, not_seen)
        regs = {**self.MARKS, UC_X86_REG_EBX: player}
        out = self.run(CULL_SITE, CULL_NEXT, regs)
        for reg, value in regs.items():
            assert out[reg] == value, f"rg_cull changed register {reg}"
        return bool(out[UC_X86_REG_EFLAGS] & ZF)

    def draw(self, player, turn=0.0):
        """mt_draw's bones: the game's standing pose in, the bones out."""
        game = [matrix(standing_place(i), turn) for i in range(len(COOKED))]
        for i, m in enumerate(game):
            self.emu.mem_write(ARRAY + 64 * i, struct.pack("<16f", *m))
        self.u32(FRAME - 0x14, ARRAY)
        self.u32(FRAME - 0x1C, len(COOKED))
        regs = {UC_X86_REG_EBX: player, UC_X86_REG_EBP: FRAME}
        out = self.run(APPLY_SITE, APPLY_NEXT, regs)
        assert out[UC_X86_REG_EBX] == player and out[UC_X86_REG_EBP] == FRAME, "rg_apply kept EBX and EBP"
        assert out[UC_X86_REG_ECX] == self.u32(player + MODEL_FIELD), "rg_apply did the move of [ebx+0F290h]"
        drawn = [list(self.floats(ARRAY + 64 * i, 16)) for i in range(len(COOKED))]
        return drawn, drawn != [[struct.unpack("<f", struct.pack("<f", v))[0] for v in m] for m in game]

    def eye(self, player):
        """update_camera's eye: the transform the view gets."""
        given = player + EYE
        regs = {UC_X86_REG_EAX: given, UC_X86_REG_EBX: player, UC_X86_REG_EDX: 0xD1, UC_X86_REG_ESI: 0x51,
                UC_X86_REG_EDI: 0xD7, UC_X86_REG_EBP: 0xB9}
        out = self.run(EYE_SITE, EYE_NEXT, regs)
        assert out[UC_X86_REG_ECX] == self.u32(player + INPUT), "rg_eye did the move of [ebx+0F470h]"
        assert (out[UC_X86_REG_EDX], out[UC_X86_REG_ESI], out[UC_X86_REG_EDI], out[UC_X86_REG_EBP]) == \
            (0xD1, 0x51, 0xD7, 0xB9), "rg_eye kept EDX, ESI, EDI and EBP"
        return out[UC_X86_REG_EAX]


def check(ok, what):
    if not ok:
        sys.exit(f"FAIL: {what}")
    print(f"ok: {what}")


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    built, _ = base.build(original, "section", code, None, late_join)
    before, _ = base.build(original, "section", code, None, None)
    linked = base.arena()
    block = linked.blocks["ragdoll.code"]
    for va, old, symbol, opcode in base.RAGDOLL_HOOKS:
        at = base.file_offset(va)
        target = linked.symbols[symbol]
        new = base.hook_bytes(target, va, old, 0, opcode)
        check(original[at:at + len(old)] == old == before[at:at + len(old)] and built[at:at + len(new)] == new
              and block.va <= target < block.va + block.size,
              f"{va:#x}: {old.hex()} in the original and the build without the late join, `call {symbol}` here")

    game = Game(built)
    a = game.add_player(0, 0.0, 0.0)
    b = game.add_player(1, 5.0, 0.0)
    other = game.add_player(2, 10.0, 0.0, OTHER_MODEL)
    game.set_local(a)
    for flag in (0, 1):
        game.step(flag)
    check(not game.cull(a, 1) and game.cull(a, 0) and not game.draw(a)[1] and game.eye(a) == a + EYE,
          "all alive: the cull, the bones and the eye are the game's")

    # a's death: two frames of the clip, then the ragdoll
    game.set_alive(a, False, 1000)
    game.set_alive(other, False, 1000)
    changed = []
    for _ in range(3):
        game.step()
        changed.append(game.draw(a)[1])
    check(changed == [False, False, True], f"a death: the clip twice (captured), then the ragdoll's bones {changed}")
    moving = game.draw(a)[0]
    # the world rolled back and simulated again inside move, the same death
    # (the same life, +328h): the step sees it dead as before
    game.step()
    again, replaced = game.draw(a)
    check(replaced and again != moving, "a death simulated again as the same death keeps the moving ragdoll "
          "(no new start from the clip)")
    check(game.cull(a, 1), "the cull draws the corpse even when its model was not seen")
    check(not game.cull(b, 1), "a live player's cull is the original's")
    check(not game.draw(other)[1] and not game.cull(other, 1), "a skeleton not ours (a bone missing): no ragdoll")
    eye = game.eye(a)
    m = game.floats(eye, 16)
    right = m[0:3]
    check(eye != a + EYE and abs(right[1]) < 1e-3, "the local player's view gets the ragdoll's eye, level")
    check(game.eye(b) == b + EYE, "another player's eye is the game's")

    # the ragdoll falls and settles on the floor
    rays = len(game.rays)
    lowest = []
    for _ in range(240):
        game.step()
        drawn, _ = game.draw(a)
        lowest.append(min(m[13] for m in drawn))
    hip = drawn[1][13]
    check(rays > 0 and all(r[2] > 0 for r in game.rays), f"the level read through ray_test ({len(game.rays)} rays)")
    check(hip < 0.5 and min(lowest) > -0.12, f"the body fell onto the floor (the hip at {hip:.2f} m, the lowest "
          f"bone at {min(lowest):.2f} m)")
    settled = len(game.rays)
    for _ in range(10):
        game.step()
    check(len(game.rays) == settled, "a settled ragdoll asks the level nothing")
    before_resim = game.draw(a)[0]

    # the world re-simulated with the same death (the same life): kept
    game.step()
    check(game.draw(a)[0] == before_resim, "a death simulated again as the same death keeps the ragdoll as it was")

    # a rollback that undoes the death: dropped at once
    game.set_alive(a, True, 1000)
    game.step()
    check(not game.draw(a)[1] and game.eye(a) == a + EYE and not game.cull(a, 1),
          "a rollback that undoes the death: the bones, the eye and the cull are the game's at once")
    # the same death again after it (the next frames): a new start from the clip
    game.set_alive(a, False, 1000)
    changed = []
    for _ in range(3):
        game.step()
        changed.append(game.draw(a)[1])
    check(changed == [False, False, True], f"the death seen again starts from the clip {changed}")
    # a new life's death (another spawn time) while the ragdoll moves: from the clip
    game.set_alive(a, False, 2000)
    game.step()
    check(not game.draw(a)[1], "another death of the place (a new life) starts from the clip again")
    print("all checks passed")


if __name__ == "__main__":
    main()
