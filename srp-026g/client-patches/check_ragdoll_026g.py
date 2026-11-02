#!/usr/bin/env python3
"""Run patch 76's nine hooks (ragdoll and headwear; ../docs/notes/ragdoll.md)
in the built exe, with Unicorn (2.1.4).

    python check_ragdoll_026g.py <original survarium.exe>

The exe is built as patch_026g.py builds it (with the late join and the
arena); the build without the late join keeps the original's nine sites.
The hooks are the Rust patch `ragdoll` (rust/patches/src/ragdoll.rs, its
lohooks ragdoll.toml), each site a jmp to its generated entry
(hook_entries_026g.asm), which saves the machine, calls the handler and
replays the site's instructions (or skips them: the cull of a corpse, the
held item's read for a corpse holding nothing).
Each hook runs from its site in the build to the game's next instruction:
game_world::tick 0x1F7C3C (the step), mt_draw 0x24754F (the held item's
read; 0x24781E when it is 0), 0x24757C (the cull) and 0x247716 (the
bones), update_camera 0x1F3643 (the eye), bullet::
check_collision 0x32D627 (a bullet's ray; the original's code at the same
site runs too, from the original exe, for the same state). The step reaches the
places through the game's own network client code (match_options 0x219990,
get_active_player 0x27A9C0 and active_player_ptr 0x2D0AB0, from the client's
vtable 0x9FFB14) on a made-up client, world core and match options; the
players are made up with the fields the step reads (ragdoll.md section 8),
their skeleton is scavengers_01 in the cook's order (ragdoll.md section 8,
`add_bone` 0x3ACDA0), the bones a standing body. Stubs: ray_test (0x75E20,
a walker floor at y = 0 and the hittable floor 4 cm over it, as Rudnya's
collision over its walker mesh; the filter checked: group 434h, mask 20Ah,
no delegate: the level's two collisions and the batteries), QueryPerformanceCounter / -Frequency (60
frames a second), GetCurrentThreadId (the patches' lock: one thread),
and the bullet manager's play_particle (0x3223E0) and
play_sound (0x3222F0), which record their arguments. The material pair
(0x306D50) is the game's own code on a made-up material manager.

Checked:
- the nine sites: the original's bytes in the original and the build
  without the late join, a jmp to the hook's entry in patches.code here;
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
- the ragdoll falls onto the floor and settles there, on the hittable
  floor over the walker one, through the grenades' and the players' slabs
  (their groups and masks the game's: the filter meets neither); the level
  is read only through ray_test with the level's filter;
- a bullet's ray: the build's hook does the original's two moves (the same
  registers, frame and XMM0 as the original exe's code from the same
  state) and changes no byte of the bullet, its result or the frame; with
  no corpse, or a ray that misses the corpse, or one the game's own ray
  stopped short of it (a wall), nothing is played; a ray through the
  settled corpse plays the pair bullet x actor's particle (at the hit on
  the body, along the ray, the body's normal, the pair's parameter) and its
  sound, and the next step wakes the corpse, whose bones then move; the
  same ray traced again (a rollback's re-simulated tick) plays and pushes
  nothing more; another bullet does; and the corpse settles again;
- a body that falls onto batteries rests on them (the stub's filter is
  Bullet's: the batteries' group 0A18h, mask 418h);
- a bullet in a living player's hit model (its body a bt_animated_rigid_body,
  actor) is kept by the hook (registers, frame and XMM0 the original exe's)
  once, and the death that follows falls along it; the players' inventories
  (a torso armour, a stack) are read for the kit's weight;
- 464f: a battery carrier's ragdoll has no gun; once its held item is 0
  (the simulation dropped the battery) mt_draw's read of it gives the
  ragdoll's 69 bones to update_skeleton (0x22490, stubbed: it records them)
  and goes on at 0x24781E, and the corpse falls and settles so; a living
  player holding nothing takes the original's je.
"""
import math
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_HOOK_BLOCK, UC_HOOK_CODE, UC_MODE_32, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EFLAGS, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP,
                               UC_X86_REG_XMM0)

import patch_026g as base

STEP_SITE, STEP_NEXT = 0x1F7C3C, 0x1F7C41
CULL_SITE, CULL_NEXT = 0x24757C, 0x247583
APPLY_SITE, APPLY_NEXT = 0x247716, 0x24771C
EYE_SITE, EYE_NEXT = 0x1F3643, 0x1F3649
BULLET_SITE, BULLET_NEXT = 0x32D627, 0x32D631
HELD_SITE, HELD_NEXT, NO_BONES = 0x24754F, 0x24755D, 0x24781E      # mt_draw's read of the held item, its je's target
UPDATE_SKELETON = 0x22490
POST_SITE, POST_NEXT = 0x2477DF, 0x2477E8
RENDER_SITE, RENDER_NEXT = 0x15A950, 0x15A957
KILLED_SITE, KILLED_NEXT = 0x2DA3DF, 0x2DA3E5
VICTORY_ITEM_VTABLE = 0xA042AC
RAY_TEST, QPC_SLOT, QPF_SLOT, THREAD_SLOT = 0x75E20, 0x870158, 0x870150, 0x870118
CREATE_FILE_SLOT, WRITE_FILE_SLOT, USER_DATA = 0x870048, 0x870108, 0x18BD0
CHANGE_TRAJECTORY = 0x32EE20
PLAY_PARTICLE, PLAY_SOUND = 0x3223E0, 0x3222F0
CLIENT_VTABLE = 0x9FFB14
HITTABLE_FLOOR = 0.04                         # the collision bullets hit, over the walker floor at 0
ZF = 1 << 6

# made-up memory
HEAP, HEAP_SIZE = 0x40000000, 0x01000000
CLIENT = HEAP                                  # +1F1C40h, +1F1C48h: 2 MiB in
WORLD, GAME, OPTIONS, NC_CORE = HEAP + 0x300000, HEAP + 0x301000, HEAP + 0x310000, HEAP + 0x380000
PLACES_ARRAY, NAMES, SKELETON, OTHER_SKELETON = HEAP + 0x3B0000, HEAP + 0x3B1000, HEAP + 0x3B2000, HEAP + 0x3B3000
MODEL, OTHER_MODEL, ARRAY, FRAME = HEAP + 0x3B4000, HEAP + 0x3B5000, HEAP + 0x3B6000, HEAP + 0x3B9000
PLAYERS = HEAP + 0x400000                      # 0x10000 each
# a bullet and what it points to; the bullet's ray's result; check_collision's frame
BULLET, SHOOTER, DAMAGE, MANAGER, BULLET_MAT, ACTOR_MAT = (HEAP + 0x800000 + 0x400 * i for i in range(6))
MATERIALS, PAIR, RESULT, SHOT_FRAME = HEAP + 0x810000, HEAP + 0x830000, HEAP + 0x831000, HEAP + 0x840000
PARTICLE_SLOTS, SOUND_RESOURCE = HEAP + 0x832000, 0x50D50000
# what the game's bullet ray hit: a wall (a static body) or a living player's hit model
WALL_OBJECT, HIT_BODIES, HIT_MODELS, INVENTORIES = HEAP + 0x833000, HEAP + 0x834000, HEAP + 0x835000, HEAP + 0x836000
ANIMATED_BODY_VTABLE = 0x9E2AF4
# a held weapon and a held battery (a victory_item: its vtable); the renderer
# update_skeleton is given (the game's renderer +200064h)
WEAPON_ITEM, BATTERY_ITEM, RENDERER, SCENE = HEAP + 0x900000, HEAP + 0x900100, HEAP + 0xA00000, 0x5CE0E000
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
        self.ticks, self.rays, self.hits = 0, [], {}
        # the level: its walker collision at y = 0 (cgroup 2, cmask 4) and the
        # collision bullets hit 4 cm over it (cgroup 8, cmask 16), as Rudnya's
        # drawn ground over its walker mesh; a grenade's (818h/8) and a
        # player's (824h/3) slabs where the body falls, which it must not meet;
        # and batteries lying around x = 20 (a slab 0.3 m high
        # with a gap where the player stands: group 0A18h, mask 418h,
        # victory_item_core 0x301CB0)
        near = lambda p: abs(p[0]) < 5 and abs(p[2]) < 5
        self.objects = [("walker floor", (0x02, 0x04), 0.0, lambda p: True),
                        ("hittable floor", (0x08, 0x10), HITTABLE_FLOOR, lambda p: True),
                        ("grenade", (0x818, 0x08), 0.3, near),
                        ("player", (0x824, 0x03), 0.5, near),
                        ("battery", (0xA18, 0x418), 0.3,
                         lambda p: 18 <= p[0] <= 22 and -2 <= p[2] <= 2
                         and not (19.6 < p[0] < 20.4 and -0.4 < p[2] < 0.5))]
        # the imports: QueryPerformanceCounter and -Frequency (stdcall, 4)
        emu.mem_write(QPC_SLOT, struct.pack("<I", STUBS))
        emu.mem_write(QPF_SLOT, struct.pack("<I", STUBS + 0x10))
        emu.mem_write(STUBS, b"\xC2\x04\x00")
        emu.mem_write(STUBS + 0x10, b"\xC2\x04\x00")
        emu.hook_add(UC_HOOK_CODE, self.counter, begin=STUBS, end=STUBS)
        emu.hook_add(UC_HOOK_CODE, self.frequency, begin=STUBS + 0x10, end=STUBS + 0x10)
        # GetCurrentThreadId (stdcall, none): one thread, id 1
        emu.mem_write(THREAD_SLOT, struct.pack("<I", STUBS + 0x80))
        emu.mem_write(STUBS + 0x80, b"\xB8\x01\x00\x00\x00\xC3")
        # the trace's file: CreateFileA, WriteFile (stdcall 7 and 5), the user data folder
        self.files, self.written = [], b""
        emu.mem_write(CREATE_FILE_SLOT, struct.pack("<I", STUBS + 0x20))
        emu.mem_write(WRITE_FILE_SLOT, struct.pack("<I", STUBS + 0x30))
        emu.mem_write(STUBS + 0x20, b"\xC2\x1C\x00")
        emu.mem_write(STUBS + 0x30, b"\xC2\x14\x00")
        emu.mem_write(STUBS + 0x40, b"c:\\users\\test\\documents\\survarium\0")
        emu.hook_add(UC_HOOK_CODE, self.create_file, begin=STUBS + 0x20, end=STUBS + 0x20)
        emu.hook_add(UC_HOOK_CODE, self.write_file, begin=STUBS + 0x30, end=STUBS + 0x30)
        emu.hook_add(UC_HOOK_CODE, self.user_data, begin=USER_DATA, end=USER_DATA)
        emu.hook_add(UC_HOOK_CODE, self.ray_test, begin=RAY_TEST, end=RAY_TEST)
        self.particles, self.sounds = [], []
        # update_skeleton (EDI the scene renderer; the instance, the bones
        # twice, the count; pops 16): records its arguments and the matrices
        self.skeletons = []
        emu.hook_add(UC_HOOK_CODE, self.update_skeleton, begin=UPDATE_SKELETON, end=UPDATE_SKELETON)
        self.u32(WEAPON_ITEM, 0x11110000)
        self.u32(BATTERY_ITEM, VICTORY_ITEM_VTABLE)
        self.u32(GAME + 0x5360, RENDERER)
        self.u32(RENDERER + 0x200064, SCENE)
        emu.hook_add(UC_HOOK_CODE, self.play_particle, begin=PLAY_PARTICLE, end=PLAY_PARTICLE)
        emu.hook_add(UC_HOOK_CODE, self.play_sound, begin=PLAY_SOUND, end=PLAY_SOUND)
        self.make_bullet()
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
    def create_file(self, emu, *_):
        name = bytes(emu.mem_read(self.u32(emu.reg_read(UC_X86_REG_ESP) + 4), 120)).split(b"\0")[0]
        self.files.append(name.decode())
        emu.reg_write(UC_X86_REG_EAX, 0x7E57)

    def write_file(self, emu, *_):
        esp = emu.reg_read(UC_X86_REG_ESP)
        handle, text, size, out = (self.u32(esp + 4 * i) for i in range(1, 5))
        assert handle == 0x7E57, "WriteFile on the trace's file"
        self.written += bytes(emu.mem_read(text, size))
        self.u32(out, size)
        emu.reg_write(UC_X86_REG_EAX, 1)

    def user_data(self, emu, *_):
        esp = emu.reg_read(UC_X86_REG_ESP)
        emu.reg_write(UC_X86_REG_EAX, STUBS + 0x40)
        emu.reg_write(UC_X86_REG_ESP, esp + 4)
        emu.reg_write(UC_X86_REG_EIP, self.u32(esp))

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
        assert (group, mask, delegate) == (0x434, 0x20A, 0), f"the level's filter: {group:#x} {mask:#x} {delegate}"
        o = self.floats(origin, 3)
        d = self.floats(direction, 3)
        assert abs(math.sqrt(sum(x * x for x in d)) - 1) < 1e-3, "a unit direction"
        self.rays.append((o, d, length))
        # Bullet's filter (needsCollision 0x75BD0): an object is hit when its
        # mask has a bit of the ray's group and its group a bit of the mask
        best = None
        for name, (obj_group, obj_mask), top, inside in self.objects:
            if not (obj_group & mask and obj_mask & group) or not (d[1] < 0 and o[1] >= top):
                continue
            t = (top - o[1]) / d[1]
            p = (o[0] + d[0] * t, top, o[2] + d[2] * t)
            if t <= length and inside(p) and (best is None or t < best[0]):
                best = (t, p, name)
        if best is not None:
            t, p, name = best
            self.hits[name] = self.hits.get(name, 0) + 1
            emu.mem_write(result, struct.pack("<3ff3fI", *p, t / length, 0.0, 1.0, 0.0, 0x77770000))
        else:
            emu.mem_write(result + 0x1C, struct.pack("<I", 0))
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, esp + 4 + 32)
        emu.reg_write(UC_X86_REG_EIP, ret)

    def play_particle(self, emu, *_):
        esp = emu.reg_read(UC_X86_REG_ESP)
        ret, manager, particle, direction, normal, parameter = (self.u32(esp + 4 * i) for i in range(6))
        self.particles.append((self.floats(emu.reg_read(UC_X86_REG_EAX), 3), manager, particle,
                               self.floats(direction, 3), self.floats(normal, 3), parameter))
        emu.reg_write(UC_X86_REG_ESP, esp + 4 + 0x14)
        emu.reg_write(UC_X86_REG_EIP, ret)

    def update_skeleton(self, emu, *_):
        esp = emu.reg_read(UC_X86_REG_ESP)
        ret, instance, bones, second, count = (self.u32(esp + 4 * i) for i in range(5))
        self.skeletons.append((emu.reg_read(UC_X86_REG_EDI), instance, bones, second, count,
                               [list(self.floats(bones + 64 * i, 16)) for i in range(min(count, 80))]))
        emu.reg_write(UC_X86_REG_ESP, esp + 4 + 0x10)
        emu.reg_write(UC_X86_REG_EIP, ret)

    def play_sound(self, emu, *_):
        esp = emu.reg_read(UC_X86_REG_ESP)
        ret, manager, sound, position = (self.u32(esp + 4 * i) for i in range(4))
        self.sounds.append((manager, sound, self.floats(position, 3)))
        emu.reg_write(UC_X86_REG_ESP, esp + 4 + 0xC)
        emu.reg_write(UC_X86_REG_EIP, ret)

    # -- a bullet --------------------------------------------------------------
    def make_bullet(self):
        """A bullet (ragdoll_bullet's fields) and the game materials: bullet
        (7) and actor (10), each its own base (+66h 0FFFFh), their pair."""
        self.u32(BULLET + 0x4, 4242)
        self.u32(BULLET + 0x8, SHOOTER)
        self.u8(SHOOTER + 0x4, 1)
        self.u32(BULLET + 0x40, DAMAGE)                  # the ammo's data: +14h mass (a 5.45 mm round's)
        self.emu.mem_write(DAMAGE + 0x14, struct.pack("<f", 0.02))
        self.emu.mem_write(BULLET + 0x1C, struct.pack("<3f", 800.0, 0.0, 0.0))  # its velocity
        self.u32(BULLET + 0x44, MANAGER)
        self.u32(BULLET + 0x48, BULLET_MAT)
        self.u8(BULLET + 0x80, 0)
        self.u32(BULLET + 0x84, 0)
        self.u32(MANAGER + 0x90, 0x0E0E0000)
        self.u32(MANAGER + 0x94, MATERIALS)
        for material, ident in ((BULLET_MAT, 7), (ACTOR_MAT, 10)):
            self.emu.mem_write(material + 0x64, struct.pack("<HH", ident, 0xFFFF))
            self.u32(MATERIALS + 0x108 + 4 * ident, material)
        self.u32(MATERIALS + 0x308 + 4 * (7 * 128 + 10), PAIR)
        self.u32(PAIR + 0x18, SOUND_RESOURCE)
        self.u32(PAIR + 0x24, PARTICLE_SLOTS)
        self.u32(PAIR + 0x28, PARTICLE_SLOTS + 4)
        self.u32(PAIR + 0x74, 1)

    def shoot(self, origin, direction, length, hit=None, exe=None, body=None):
        """check_collision at 0x32D627 with the ray (origin, unit direction,
        length) just traced: `hit` the point the game's own ray hit, if any.
        Returns the registers, the frame, XMM0 and the bullet's and the
        result's bytes after."""
        emu = self.emu if exe is None else exe
        frame = bytearray(0xA0)
        struct.pack_into("<f", frame, 0x14, length)
        struct.pack_into("<3f", frame, 0x20, *direction)
        struct.pack_into("<3f", frame, 0x90, *origin)
        struct.pack_into("<I", frame, 0x38, 0x5EED5EED)
        if hit is None:
            result = struct.pack("<3ff3fIIB3x", 0, 0, 0, 0.0, 0, 0, 0, 0, 0xFFFFFFFF, 0)
        else:
            result = struct.pack("<3ff3fIIB3x", *hit, 0.5, -direction[0], -direction[1], -direction[2],
                                 WALL_OBJECT if body is None else body, 3, 0)
        emu.mem_write(RESULT, result)
        emu.mem_write(SHOT_FRAME, bytes(frame))
        regs = {UC_X86_REG_EAX: RESULT, UC_X86_REG_EBX: 0, UC_X86_REG_ECX: 0xC1, UC_X86_REG_EDX: 0xD1,
                UC_X86_REG_ESI: 0x51, UC_X86_REG_EDI: BULLET, UC_X86_REG_EBP: 0x5A5A0001}
        bullet = bytes(emu.mem_read(BULLET, 0x90))
        emu.reg_write(UC_X86_REG_ESP, SHOT_FRAME)
        for reg, value in regs.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_XMM0, 0)
        emu.emu_start(BULLET_SITE, BULLET_NEXT, count=40_000_000)
        assert emu.reg_read(UC_X86_REG_EIP) == BULLET_NEXT and emu.reg_read(UC_X86_REG_ESP) == SHOT_FRAME
        out = {r: emu.reg_read(r) for r in regs}
        return (out, bytes(emu.mem_read(SHOT_FRAME, 0xA0)), emu.reg_read(UC_X86_REG_XMM0) & (1 << 64) - 1,
                bytes(emu.mem_read(BULLET, 0x90)) == bullet and bytes(emu.mem_read(RESULT, 0x28)) == result)

    # -- the made-up match -----------------------------------------------------
    def add_player(self, place, x, z, model=MODEL):
        player = PLAYERS + 0x10000 * len(self.players)
        self.players.append(player)
        self.u32(player + REFS, 1)
        self.u8(player + PLACE, place)
        self.u32(player + HELD, WEAPON_ITEM)
        self.u32(player + 0xF474, WORLD)                # its game world (the renderer's way)
        self.u32(player + PHYSICS_WORLD, PHYSICS)
        self.u32(player + MODEL_FIELD, model)
        self.u32(player + INPUT, 0x1A2B3C4D)
        self.u8(player + INSERTED, 1)
        # its hit model's body in the physics world (actor), and its inventory:
        # a 9 kg torso armour (slot 2) and a stack of 3 x 0.5 kg (slot 9)
        index = len(self.players) - 1
        body, hit_model, inventory = HIT_BODIES + 0x40 * index, HIT_MODELS + 0x200 * index, INVENTORIES + 0x1000 * index
        self.u32(body, ANIMATED_BODY_VTABLE)
        self.emu.mem_write(body + 0x38, struct.pack("<H", 10))
        self.u32(hit_model + 0x12C, body)
        self.u32(player + 0x498, hit_model)
        self.u32(player + 0x10C, inventory)
        armour, stack, entry = inventory + 0x200, inventory + 0x500, inventory + 0x800
        self.u32(inventory + 0x10C + 4 * 2, armour)
        self.emu.mem_write(armour + 0x218, struct.pack("<f", 9.0))
        self.u32(inventory + 0x10C + 4 * 9, stack)
        self.emu.mem_write(stack + 0x218, struct.pack("<f", 0.5))
        self.u32(stack + 0x25C, entry)
        self.u8(entry + 0x22A, 1)
        self.emu.mem_write(stack + 0x260, struct.pack("<H", 3))
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

    def hit_body(self, player):
        return self.u32(self.u32(player + 0x498) + 0x12C)

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
            assert out[reg] == value, f"on_step changed register {reg}"
        assert bool(out[UC_X86_REG_EFLAGS] & ZF) == (flag == 0), "on_step's compare of [esp+0Fh]"

    def cull(self, player, not_seen):
        """mt_draw's cull: True when the je draws."""
        self.u8(player + NOT_SEEN, not_seen)
        regs = {**self.MARKS, UC_X86_REG_EBX: player}
        out = self.run(CULL_SITE, CULL_NEXT, regs)
        for reg, value in regs.items():
            assert out[reg] == value, f"on_cull changed register {reg}"
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
        assert out[UC_X86_REG_EBX] == player and out[UC_X86_REG_EBP] == FRAME, "on_bones kept EBX and EBP"
        assert out[UC_X86_REG_ECX] == self.u32(player + MODEL_FIELD), "on_bones did the move of [ebx+0F290h]"
        drawn = [list(self.floats(ARRAY + 64 * i, 16)) for i in range(len(COOKED))]
        return drawn, drawn != [[struct.unpack("<f", struct.pack("<f", v))[0] for v in m] for m in game]

    def held_item(self, player):
        """mt_draw's read of the held item: (where it went on, ESI, the
        update_skeleton calls it made)."""
        before = len(self.skeletons)
        stop = NO_BONES if self.u32(player + HELD) == 0 else HELD_NEXT
        regs = {UC_X86_REG_EBX: player, UC_X86_REG_ESI: 0x51, UC_X86_REG_EBP: FRAME, UC_X86_REG_EDI: 0xD7}
        out = self.run(HELD_SITE, stop, regs)
        assert out[UC_X86_REG_EBX] == player and out[UC_X86_REG_EBP] == FRAME, "on_held_item kept EBX and EBP"
        return stop, out[UC_X86_REG_ESI], self.skeletons[before:]

    def eye(self, player):
        """update_camera's eye: the transform the view gets."""
        given = player + EYE
        regs = {UC_X86_REG_EAX: given, UC_X86_REG_EBX: player, UC_X86_REG_EDX: 0xD1, UC_X86_REG_ESI: 0x51,
                UC_X86_REG_EDI: 0xD7, UC_X86_REG_EBP: 0xB9}
        out = self.run(EYE_SITE, EYE_NEXT, regs)
        assert out[UC_X86_REG_ECX] == self.u32(player + INPUT), "on_eye did the move of [ebx+0F470h]"
        assert (out[UC_X86_REG_EDX], out[UC_X86_REG_ESI], out[UC_X86_REG_EDI], out[UC_X86_REG_EBP]) == \
            (0xD1, 0x51, 0xD7, 0xB9), "on_eye kept EDX, ESI, EDI and EBP"
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
    block = linked.blocks["patches.code"]
    entries = {h["entry"] for h in base.hooks_026g.hooks(exe=base.hooks_026g.original_reader(original))
               if any(m["patch"] == "ragdoll" for m in h.get("chain", [h]))}
    hooks = [h for h in base.rust_hooks(original) if h[2] in entries]
    check(len(hooks) == 9 and {h[0] for h in hooks} == {STEP_SITE, CULL_SITE, APPLY_SITE, EYE_SITE, BULLET_SITE, HELD_SITE,
                                                       POST_SITE, RENDER_SITE, KILLED_SITE},
          "the Rust patch ragdoll's nine lohooks (ragdoll.toml, sites_026g.tsv)")
    for va, old, symbol, opcode in hooks:
        at = base.file_offset(va)
        target = linked.symbols[symbol]
        new = base.hook_bytes(target, va, old, 0, opcode)
        check(opcode == 0xE9 and original[at:at + len(old)] == old == before[at:at + len(old)]
              and built[at:at + len(new)] == new and block.va <= target < block.va + block.size,
              f"{va:#x}: {old.hex()} in the original and the build without the late join, `jmp {symbol}` here")

    game = Game(built)
    stock = Game(original)
    # the bullet's site: the build's hook against the original's two moves, all alive
    ray = ((-3.0, 0.5, 0.0), (1.0, 0.0, 0.0), 6.0)
    mine, theirs = game.shoot(*ray), stock.shoot(*ray, exe=stock.emu)
    check(mine == theirs and mine[3] and struct.unpack_from("<I", mine[1], 0x38)[0] == 0 and mine[2] == 0,
          "a bullet's ray: the hook does the original's two moves (registers, frame, XMM0 the original exe's), "
          "the bullet and its result untouched")
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
    # (no bullet: a standing body that only tips over, the clip held standing;
    # the 5 s cap at the latest)
    for _ in range(330):
        game.step()
        drawn, _ = game.draw(a)
        lowest.append(min(m[13] for m in drawn))
    hip = drawn[1][13]
    check(rays > 0 and all(r[2] > 0 for r in game.rays), f"the level read through ray_test ({len(game.rays)} rays)")
    check(hip < 0.5 and min(lowest) > -0.12, f"the body fell onto the floor (the hip at {hip:.2f} m, the lowest "
          f"bone at {min(lowest):.2f} m)")
    check(game.hits.get("hittable floor", 0) > 0 and not game.hits.get("grenade") and not game.hits.get("player"),
          f"it fell onto the hittable floor, through the grenade's and the player's slabs ({game.hits})")
    settled = len(game.rays)
    for _ in range(10):
        game.step()
    check(len(game.rays) == settled, "a settled ragdoll asks the level nothing")

    # bullets through the settled corpse (bone 7 is Spin: the torso)
    lying = game.draw(a)[0]
    spin = lying[7][12:15]
    through = ((spin[0] - 3.0, spin[1], spin[2]), (1.0, 0.0, 0.0), 6.0)
    game.shoot((spin[0] - 3.0, spin[1] + 1.5, spin[2]), (1.0, 0.0, 0.0), 6.0)
    game.shoot(*through[:2], 2.0)
    game.shoot(*through, hit=(spin[0] - 1.0, spin[1], spin[2]))
    game.step()
    check(not game.particles and not game.sounds and game.draw(a)[0] == lying,
          "a ray above the corpse, one short of it, and one the game's ray stopped at a wall before it: "
          "nothing played, nothing pushed")
    out, theirs = game.shoot(*through), stock.shoot(*through)
    check(out == theirs and out[3], "a ray through the corpse: the hook's registers, frame and XMM0 are the "
          "original exe's, the bullet and its result untouched")
    check(len(game.particles) == 1 and len(game.sounds) == 1, f"a ray through the corpse: one particle, one sound "
          f"({len(game.particles)}, {len(game.sounds)})")
    point, manager, particle, direction, normal, parameter = game.particles[0]
    check(manager == MANAGER and particle == PARTICLE_SLOTS and parameter == 1 and direction == (1.0, 0.0, 0.0)
          and normal[0] < -0.3 and spin[0] - 0.4 < point[0] < spin[0] and abs(point[1] - spin[1]) < 0.2,
          f"the particle: the pair bullet x actor's first, at the hit on the body ({point[0] - spin[0]:+.2f} m "
          f"before the bone), along the ray, the body's normal {normal[0]:+.2f}, the pair's parameter")
    check(game.sounds[0][:2] == (MANAGER, PAIR + 0x18) and game.sounds[0][2] == point,
          "the sound: the pair's, at the hit")
    game.step()
    woken = []
    for _ in range(20):
        game.step()
        woken.append(game.draw(a)[0])
    check(woken[-1] != lying and len(game.rays) > settled, "the next steps wake the corpse: its bones move and it "
          "asks the level again")
    # the same ray traced again (a rollback re-simulates the tick)
    game.shoot(*through)
    check(len(game.particles) == 1 and len(game.sounds) == 1, "the same shot traced again plays nothing more")
    game.u32(BULLET + 0x4, 4243)
    shifted = game.draw(a)[0][7][12:15]
    game.shoot((shifted[0] - 3.0, shifted[1], shifted[2]), (1.0, 0.0, 0.0), 6.0)
    check(len(game.particles) == 2, "another bullet's shot plays again")
    for _ in range(600):
        game.step()
    game.draw(a)
    resting = len(game.rays)
    for _ in range(10):
        game.step()
    check(len(game.rays) == resting, "the corpse settles again")

    # a body that falls onto a battery rests on it (the level's rays hit it)
    c = game.add_player(3, 20.0, 0.0)
    game.set_alive(c, False, 1000)
    for _ in range(300):
        game.step()
        drawn_c, _ = game.draw(c)
    head = drawn_c[[n for n, _ in COOKED].index("Head")][13]
    check(game.hits.get("battery", 0) > 0 and head > 0.3,
          f"a body that falls onto batteries lying by its feet rests on them, not through them (the head at "
          f"{head:.2f} m, the batteries' top 0.3 m; {game.hits.get('battery', 0)} rays hit them)")
    game.set_alive(c, True, 1000)
    game.step()
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
    # the killing shot pushes the body: a bullet in a living player's hit model
    # (the game's ray hit its body) is kept, and its death falls along it
    pushed = game.add_player(4, 30.0, 0.0)
    calm = game.add_player(5, 40.0, 0.0)
    game.step()
    game.u32(BULLET + 0x4, 5000)
    out = game.shoot((27.0, 1.3, 0.0), (1.0, 0.0, 0.0), 6.0, hit=(29.88, 1.3, 0.0), body=game.hit_body(pushed))
    theirs = stock.shoot((27.0, 1.3, 0.0), (1.0, 0.0, 0.0), 6.0, hit=(29.88, 1.3, 0.0), body=WALL_OBJECT)
    check(out[:3] == theirs[:3] and out[3], "a bullet in a living player: the hook's registers, frame and XMM0 are "
          "the original exe's, the bullet and the result untouched")
    # the same ray traced again (a rollback): kept once
    game.shoot((27.0, 1.3, 0.0), (1.0, 0.0, 0.0), 6.0, hit=(29.88, 1.3, 0.0), body=game.hit_body(pushed))
    for player in (pushed, calm):
        game.set_alive(player, False, 1000)
    hips = {}
    for _ in range(45):
        game.step()
        for player in (pushed, calm):
            drawn, _ = game.draw(player)
            hips[player] = drawn[1][12:15]
    # (the drawn bones are in model space: the player's own place is 0)
    check(hips[pushed][0] > 0.25 and abs(hips[calm][0]) < hips[pushed][0],
          f"the killing shot knocks its body along it (+x): the hip {hips[pushed][0]:+.2f} m, the "
          f"unshot death's {hips[calm][0]:+.2f} m")
    check(not game.files and not game.written, "the build's diagnostics are off: no file opened, nothing written")

    # the bullet's +1Ch: its segment's velocity (the original exe's change_trajectory
    # writes a pierce's new one there and to +34h, as the bullet's ctor does)
    emu = stock.emu
    probe = HEAP + 0x900000
    emu.mem_write(probe, bytes(0x100))
    emu.mem_write(probe + 0xC0, struct.pack("<3f3f", 1.0, 2.0, 3.0, 610.0, -20.0, 35.0))
    esp = STACK - 0x2000
    emu.mem_write(esp, struct.pack("<If", RET, 0.25))
    emu.reg_write(UC_X86_REG_ESP, esp)
    emu.reg_write(UC_X86_REG_ESI, probe)
    emu.reg_write(UC_X86_REG_ECX, probe + 0xC0)
    emu.reg_write(UC_X86_REG_EAX, probe + 0xCC)
    emu.mem_write(RET, b"\xF4")
    emu.emu_start(CHANGE_TRAJECTORY, RET, count=1_000_000)
    check(stock.floats(probe + 0x1C, 3) == (610.0, -20.0, 35.0) and stock.floats(probe + 0x34, 3) == (610.0, -20.0, 35.0)
          and stock.u32(probe + 0x84) == 1,
          "the bullet's +1Ch is its segment's velocity: the original exe's change_trajectory (0x32EE20) writes a "
          "pierce's new velocity to +1Ch and +34h and counts the segment (+84h)")

    # the diagnostics build: a living hit's line, then its death's
    traced = Game(base.with_diagnostics(built))
    runner = traced.add_player(0, 0.0, 0.0)
    traced.set_local(runner)
    traced.step()
    traced.u32(BULLET + 0x4, 6000)
    traced.shoot((-3.0, 1.3, 0.0), (1.0, 0.0, 0.0), 6.0, hit=(-0.12, 1.3, 0.0), body=traced.hit_body(runner))
    traced.step()
    traced.set_alive(runner, False, 1000)
    for _ in range(4):
        traced.step()
        traced.draw(runner)
    lines = traced.written.decode().splitlines()
    hit = [line.split(",") for line in lines if line.startswith("hit,")]
    death = [line.split(",") for line in lines if line.startswith("death,")]
    check(traced.files == ["c:\\users\\test\\documents\\survarium\\srp_ragdoll.csv"] and lines[0].startswith("# srp ragdoll")
          and len(hit) == 1 and hit[0][2:5] == ["0", "0.0200", "800.0"] and abs(float(hit[0][5]) - 12.8) < 0.01
          and len(death) == 1 and death[0][2:5] == ["0", "1", "1"] and abs(float(death[0][5]) - 12.8) < 0.01
          and float(death[0][6]) > 0.2,
          f"the diagnostics build writes srp_ragdoll.csv in the user data folder: the hit (place 0, mass 0.02, "
          f"speed 800, push 12.8 N s) and its death (a body shot, one hit, the push, the knock): {lines[1:]}")
    # 464f: three 7.62x51 rounds (the config's 0.1 kg, 850 m/s), each traced
    # at three segments through the same body (a pierce), then the death:
    # one hit a bullet, each pushing as 0.04 kg; the knock of the three a spike
    heavy = traced.add_player(1, 5.0, 0.0)
    traced.emu.mem_write(DAMAGE + 0x14, struct.pack("<f", 0.1))
    traced.emu.mem_write(DAMAGE + 0x3C, struct.pack("<f", 850.0))
    traced.emu.mem_write(BULLET + 0x1C, struct.pack("<3f", 850.0, 0.0, 0.0))
    for bullet in (7001, 7002, 7003):
        traced.u32(BULLET + 0x4, bullet)
        for segment in (0, 1, 2):
            traced.u32(BULLET + 0x84, segment)
            traced.shoot((2.0, 1.3, 0.0), (1.0, 0.0, 0.0), 6.0, hit=(4.88, 1.3, 0.0), body=traced.hit_body(heavy))
    traced.u32(BULLET + 0x84, 0)
    traced.step()
    traced.set_alive(heavy, False, 1000)
    for _ in range(4):
        traced.step()
        traced.draw(heavy)
    lines = traced.written.decode().splitlines()
    hit = [line.split(",") for line in lines if line.startswith("hit,") and line.split(",")[2] == "1"]
    death = [line.split(",") for line in lines if line.startswith("death,") and line.split(",")[2] == "1"]
    spike = [line.split(",") for line in lines if line.startswith("spike,")]
    check(len(hit) == 3 and all(h[3:5] == ["0.1000", "850.0"] and abs(float(h[5]) - 27.2) < 0.01 and h[6] == "850"
                                and h[7] == "0" for h in hit) and [h[8] for h in hit] == ["7001", "7002", "7003"]
          and len(death) == 1 and death[0][4] == "3" and float(death[0][6]) > 1.0
          and len(spike) == 1 and spike[0][2:8] == ["1", "death", "0.1000", "850.0", "850", "0"]
          and spike[0][8] == "3",
          f"three bullets through one body, three segments each: three hits (the first segment's, 0.1 kg at 850 m/s "
          f"pushing 27.2 N s, as 0.04 kg), one death of three hits and its spike line: {lines[-6:]}")

    # a running player's death carries its speed: 5 m/s along +z while alive
    runner, calm = game.add_player(6, 50.0, 0.0), game.add_player(7, 60.0, 0.0)
    for k in range(12):
        game.emu.mem_write(runner + TRANSFORM, struct.pack("<16f", *matrix((50.0, 0, 5.0 * k / 60.0))))
        game.step()
    for player in (runner, calm):
        game.set_alive(player, False, 1000)
    for _ in range(40):
        game.step()
        for player in (runner, calm):
            drawn, _ = game.draw(player)
            hips[player] = drawn[1][12:15]
    check(hips[runner][2] > 0.5 and abs(hips[calm][2]) < 0.2,
          f"a running player's death carries its speed: the hip {hips[runner][2]:+.2f} m ahead (in its model's "
          f"space), a standing one's {hips[calm][2]:+.2f} m")
    # 464f: a battery carrier's death drops no gun; once the simulation drops
    # the battery (+128h 0, mt_draw then updates no bone) the corpse is posed
    # from its ragdoll alone, and settles
    carrier, gunner = game.add_player(8, 70.0, 0.0), game.add_player(9, 80.0, 0.0)
    game.u32(carrier + HELD, BATTERY_ITEM)
    for player in (carrier, gunner):
        game.set_alive(player, False, 1000)
    weapon = [n for n, _ in COOKED].index("Weapon")
    bones = {carrier: [], gunner: []}
    for _ in range(40):
        game.step()
        for p in bones:
            bones[p].append(game.draw(p)[0][weapon])
    check(bones[carrier][5] == bones[carrier][-1] and bones[gunner][5] != bones[gunner][-1],
          "a battery carrier's ragdoll has no gun (its Weapon bone stays with the root, where it was at the "
          "death); a gunner's gun drops (its Weapon bone falls)")
    check(game.held_item(gunner)[0] == HELD_NEXT and game.held_item(carrier)[2] == [],
          "holding an item, mt_draw goes on as the original (no update_skeleton of ours)")
    game.u32(carrier + HELD, 0)
    living = game.add_player(10, 90.0, 0.0)
    game.u32(living + HELD, 0)
    game.step()
    normal, _ = game.draw(carrier)
    where, esi, calls = game.held_item(carrier)
    plain = game.held_item(living)
    check(where == NO_BONES and esi == 0 and len(calls) == 1 and calls[0][:2] == (SCENE, MODEL + 0x108)
          and calls[0][2] == calls[0][3] and calls[0][4] == len(COOKED)
          and all(max(abs(a - b) for a, b in zip(m, n)) < 1e-4 for m, n in zip(calls[0][5], normal))
          and plain[0] == NO_BONES and plain[1] == 0 and plain[2] == [],
          "the battery dropped (+128h 0): the corpse's 69 bones, its ragdoll's, go to update_skeleton (EDI the "
          "scene renderer, the model's +108h, the bones twice) and the draw goes on at 0x24781E with ESI 0; a "
          "living player holding nothing takes the original's je, nothing of ours")
    frames = []
    for _ in range(400):
        game.step()
        frames.append(game.held_item(carrier)[2][0][5])
    still = all(max(abs(a - b) for a, b in zip(m, n)) < 1e-5 for m, n in zip(frames[-1], frames[-2]))
    check(still and frames[0] != frames[-1] and min(m[13] for m in frames[-1]) > -0.2,
          f"held nothing, it falls and settles: the hip at {frames[-1][1][13]:.2f} m, the bones still")
    check_headwear(built)
    print("all checks passed")


def check_headwear(built):
    game = Game(built)
    player = game.add_player(0, 0.0, 0.0)
    instance, model, binds = (HEAP + x for x in (0xC00000, 0xC10000, 0xC20000))
    current, shadow, previous = (HEAP + x for x in (0xC30000, 0xC34000, 0xC38000))
    game.u32(MODEL + 0x108, instance)
    game.u32(instance + 0x62B4, model)
    game.u32(model + 0x140, binds)
    game.u32(model + 0x144, binds + 70 * 64)
    identity = struct.pack('<16f', *matrix((0, 0, 0)))
    game.emu.mem_write(binds, identity * 70)
    for offset, array in ((0x229C, current), (0x42A8, shadow), (0x290, previous)):
        game.u32(instance + offset, array)
        game.emu.mem_write(array, identity * 80)
        game.emu.mem_write(array + 44 * 64, struct.pack('<16f', *matrix((offset, 2, 3))))
    snapshot = {a: bytes(game.emu.mem_read(a, 80 * 64)) for a in (current, shadow, previous)}
    for first in (0, 1):
        for a, b in snapshot.items(): game.emu.mem_write(a, b)
        game.u8(instance + 0x62D8, first)
        out = game.run(RENDER_SITE, RENDER_NEXT, {UC_X86_REG_EDI: instance, UC_X86_REG_EDX: 69})
        for a in snapshot:
            source = current if a == previous and first else a
            check(game.floats(a + 69 * 64, 16) == game.floats(source + 44 * 64, 16)
                  and bytes(game.emu.mem_read(a, 69 * 64)) == snapshot[a][:69 * 64]
                  and bytes(game.emu.mem_read(a + 70 * 64, 10 * 64)) == snapshot[a][70 * 64:],
                  f'headwear render initialization: array {a:#x}, first={first}, only bone 69 changes')
        check(bool(out[UC_X86_REG_EFLAGS] & ZF) == (first == 0), 'render hook preserves the first-update compare')
    before = bytes(game.emu.mem_read(current, 80 * 64))
    game.run(RENDER_SITE, RENDER_NEXT, {UC_X86_REG_EDI: instance, UC_X86_REG_EDX: 70})
    check(bytes(game.emu.mem_read(current, 80 * 64)) == before, 'an explicit detached pose is not overwritten')
    game.u32(model + 0x144, binds + 69 * 64)
    game.run(RENDER_SITE, RENDER_NEXT, {UC_X86_REG_EDI: instance, UC_X86_REG_EDX: 69})
    check(bytes(game.emu.mem_read(current, 80 * 64)) == before, 'stock render binds are left untouched')
    game.draw(player)
    game.run(POST_SITE, POST_NEXT, {UC_X86_REG_EBX: player, UC_X86_REG_EBP: FRAME})
    check(not game.skeletons, 'stock content keeps the original skeleton command path')
    game.u32(model + 0x144, binds + 70 * 64)

    def post():
        drawn, _ = game.draw(player)
        game.run(POST_SITE, NO_BONES, {UC_X86_REG_EBX: player, UC_X86_REG_EBP: FRAME})
        call = game.skeletons[-1]
        check(call[4] == 70 and call[5][:69] == drawn, 'extended command preserves all 69 animation matrices')
        return call[5]

    worn = post()
    check(worn[69] == worn[44], 'a living player wears the helmet on Head')

    def killed(cause, victim=player):
        regs = {UC_X86_REG_ESI: victim, UC_X86_REG_EBX: 0x123400 | cause,
                UC_X86_REG_EDI: 0xDA, UC_X86_REG_ECX: 0xCB, UC_X86_REG_EDX: 0xDC}
        out = game.run(KILLED_SITE, KILLED_NEXT, regs)
        check(all(out[r] == value for r, value in regs.items())
              and out[UC_X86_REG_EAX] == game.u32(victim + 0x120),
              'fatal-hit hook preserves registers and replays the world read')

    # Unknown, body hit and suicide/environmental death all stay attached.
    # A classification from another life or another place cannot authorize it.
    other = game.add_player(1, 5.0, 0.0)
    for cause in (None, 0, 3, 'old-life', 'other-player', 'corrected'):
        game.set_alive(player, True, 999)
        game.step()
        if cause == 'old-life':
            killed(1)
        game.set_alive(player, False, 1000)
        if cause == 'other-player':
            game.set_alive(other, False, 1000)
            killed(1, other)
        elif cause == 'corrected':
            killed(1)
            killed(0)
        elif isinstance(cause, int):
            killed(cause)
        for _ in range(40):
            game.step()
            game.draw(player)
        attached = post()
        check(attached[69] == attached[44], f'{cause}: helmet stays on the ragdoll head')
        game.u32(player + HELD, 0)
        calls = game.held_item(player)[2]
        check(calls[-1][5][69] == calls[-1][5][44], 'unheld non-headshot corpse keeps its helmet')
        game.u32(player + HELD, WEAPON_ITEM)

    game.set_alive(player, True, 1000)
    game.step()
    game.set_alive(player, False, 1000)
    killed(1)
    for _ in range(40):
        game.step()
        game.draw(player)
    dropped = post()
    check(math.dist(dropped[69][12:15], dropped[44][12:15]) > 0.15, 'the helmet moves separately from the dead head')
    game.u32(player + HELD, 0)
    calls = game.held_item(player)[2]
    check(len(calls) == 1 and calls[0][4] == 70
          and math.dist(calls[0][5][69][12:15], calls[0][5][44][12:15]) > 0.15,
          'a corpse holding nothing retains its detached helmet')
    killed(0)
    corrected = post()
    check(corrected[69] == corrected[44], 'a corrected non-headshot event immediately reattaches the helmet')
    game.set_alive(player, True, 1000)
    game.step()
    restored = post()
    check(restored[69] == restored[44], 'rollback or respawn restores the worn helmet')
    game.set_alive(player, False, 1000)
    for _ in range(40):
        game.step()
        game.draw(player)
    restored = post()
    check(restored[69] == restored[44], 'a rolled-back headshot does not authorize a later death')


if __name__ == "__main__":
    main()
