#!/usr/bin/env python3
"""Run patch 45 (weapon_table_026g.asm: a weapon's upgrade window shows the
weapon in the air above the lobby's workbench; client_patch.md section
3.42) in the build, with the game's own code: the movie's start_item_upgrade
and exit_item_upgrade handlers (0x29A870, 0x29A890) through patch 32 to the
lobby camera's setter (0x2845C0), the lobby camera's constructor (0x283BC0)
and whole tick (0x283EF0), and player::mt_draw's two sites (0x247576,
0x24778F), on check_weapon_preview_026g.py's made-up lobby.

    python check_weapon_table_026g.py <original survarium.exe>

Requires Python `unicorn` (2.1.4). Each image is loaded as is and again 1 MiB
higher with its base relocations applied. The tick's camera collision ray
(0x2844D0) and the lobby menu's projection of the character (0x21DF00, which
gets the camera's matrix at +48h) are stubs; the second writes the matrix
down. What must hold:
- with no weapon's window the build's tick is the original's: every byte of
  the camera and every matrix, with drags and the wheel;
- a weapon's window: the camera's point, yaw and pitch fly to the bench's in
  a few seconds (the height and the distance by the camera's own setter),
  the eye then stands in front of the bench, at the distance, looking along
  +x from a little above; a drag turns the weapon (the spin by the yaw's
  move, the tilt by the pitch's, the tilt within its limit) and not the
  camera; the yaw it flies to is the nearest turn of -pi/2 to the camera's;
- the close flies the camera back to the origin and the yaw and pitch it had,
  exactly, and from then on the tick is the original's again; a window over
  a weapon's (armour) flies back too, a weapon's over a weapon's keeps the
  yaw and pitch kept at the first;
- the camera holds at the window's start until the weapon is drawn at the
  bench (at most 2 s), then flies with the setter's speeds;
- the weapon's middle (its model's box through bone 1's inverse bind and
  pose, as the data of the AK-12 and the PM has them) is drawn at the bench;
  the held item stays in the hand while the character makes its player;
- the left button drags at the bench when its press is off the movie;
- mt_draw's culled test sends the table's player (the lobby character's,
  drawn by its update, its window open, the table on) to the draw path and
  everyone else where the original does; its held item's transform is the
  bench's for the table's player (rows orthonormal, the weapon's middle at
  the bench, turned by the spin and the tilt) and the decal pose's or the
  hand's as the original otherwise; every register kept (eax the transform
  for the table's), the x87 stack as it was.
Says nothing about what the game draws.
"""
import math
import struct
import sys

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EIP, \
    UC_X86_REG_ESP, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_FPSW, \
    UC_X86_REG_FPTAG, UC_X86_REG_FPCW

import patch_026g as base
import check_weapon_preview_026g as lobby_check
from check_weapon_preview_026g import Lobby, KITS, AK, SVD, VEST, START_HANDLER, EXIT_HANDLER, CAMERA, \
    CHARACTER, SQUAD, MENU, PLAYER, STACK, STUBS, UI, f32, from_f32

CTOR, TICK = 0x283BC0, 0x283EF0
RAY, PROJECT = 0x2844D0, 0x21DF00              # both `ret 4`
LAST_TIME = 0x2260AEC                          # the tick's last time (ms)
TIME = 0x20500400 + 0x584C                     # [[menu+9Ch]+584Ch]: the menu's +9Ch is the lobby's OWNER
CULL, CULL_ON, DRAW = 0x247576, 0x24757C, 0x247592
MATRIX, DECAL_PATH, HAND, DRAW_ITEM = 0x24778F, 0x247798, 0x2477C2, 0x2477C8
LOBBY_DRAW = 0x284A83
FRAME = STACK - 0x2000                         # a made-up mt_draw frame (ebp)
OTHER_PLAYER = SQUAD                           # another player (64 KiB of memory, as PLAYER)
LEFT_SITE, LEFT_END = 0x283D81, 0x283D88       # on_mouse_move's drag test, then `test al,al`
OVER_UI = STUBS + 0x100                        # the lobby menu's vtable+24h, is_mouse_over_ui (a stub)
WEAPON_VTABLE, SKELETON_VTABLE = 0xA04174, 0x9F0F24
ITEM, MODEL, RENDER, SKELETON = UI + 0x2000, UI + 0x4000, UI + 0x5000, UI + 0xC000   # a weapon's objects
INVERSE_BINDS, BONE_POSES = UI + 0xD000, STACK - 0x3000

# Weapons as the data has them (resources.db, read 2026-10-21): the items_dict
# id, the render model's box (render/export_properties, min and max), bone 1's
# (`offset`) inverse bind (render/bind_pose) and pose of the 3rd-view idle
# (animations/.../item/stand/3rd_view/idle.b-spline over the skeleton), as the
# game keeps matrices (rows, v * M); and the middle of the gun's mesh in that
# pose (each vertex by its bone, the box of them, parts parked beside it left
# out: the VSS's second magazine), which the build must put at the bench
# within 1 cm.
FLIP = (-1.0, 0, 0, 0, 0, 1.0, 0, 0, 0, 0, -1.0, 0, 0, 0, 0, 1.0)
WEAPONS = {
    "AK-12": (None, (-0.0224, -0.1585, -0.6726), (0.0315, 0.1114, 0.2654), FLIP, FLIP, (-0.0045, -0.0235, 0.2036)),
    "PM": (89, (-0.083, -0.072, -0.124), (0.021, 0.051, 0.034), FLIP,
           (-0.999839, 0.012316, -0.012998, 0, 0.012093, 0.999780, 0.017123, 0, 0.013206, 0.016963, -0.999769, 0,
            -0.013804, -0.000793, 0.200544, 1.0), (-0.0212, -0.0136, 0.2457)),
    "VSS": (308, (-0.2957, -0.0813, -0.5669), (0.021, 0.0833, 0.3269), FLIP,
            (-0.999994, 0.003439, 0.000239, 0, 0.003440, 0.999987, 0.003642, 0, -0.000227, 0.003643, -0.999993, 0,
             0.005773, 0.019322, 0.026358, 1.0), (0.0134, 0.0201, 0.1465)),
    "AK-74U": (13, (-1.0, -1.0, -1.0), (1.0, 1.0, 1.0), FLIP, FLIP, (0.0096, -0.0373, 0.0882)),
    # the Vepr: a default box; its bone 1 is 7 cm along in the 3rd-view idle, at the origin in
    # its `preview` (which the lobby's weapon shows: the owner saw round 3b off by those 7 cm)
    "Vepr (idle)": (216, (-1.0, -1.0, -1.0), (1.0, 1.0, 1.0), FLIP,
                    (-0.999976, -0.006868, 0.000511, 0, -0.006862, 0.999920, 0.010598, 0, -0.000583, 0.010595,
                     -0.999944, 0, 0.013903, -0.021218, 0.069836, 1.0), (0.0089, -0.0527, 0.0904)),
    "Vepr (preview)": (216, (-1.0, -1.0, -1.0), (1.0, 1.0, 1.0), FLIP,
                       (-0.999976, -0.006868, 0.000511, 0, -0.006862, 0.999920, 0.010598, 0, -0.000583, 0.010595,
                        -0.999944, 0, 0.0, 0.0, 0.0, 1.0), (-0.0050, -0.0315, 0.0206)),
}
DICTIONARY_ITEM = UI + 0xE000                  # the weapon's items_dict entry (+0 its u16 id)
# weapon_table_026g.asm's MIDDLES (items_dict id: the gun's middle in the bind space, m), read from its source
def table_middles():
    import os
    import re
    text = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "weapon_table_026g.asm")).read()
    rows = re.findall(r"^(?:middles:)?\s+dw (\d+), (-?\d+), (-?\d+), (-?\d+)", text[text.index("middles:"):], re.M)
    return {int(d): (int(x) / 1000, int(y) / 1000, int(z) / 1000) for d, x, y, z in rows}


TABLE_MIDDLES = table_middles()

# weapon_table_026g.asm's constants and state
BENCH = (4.25, 1.30, 3.05)
VIEW_YAW, VIEW_PITCH, VIEW_DISTANCE, MIDDLE_Z, TILT_MAX, HOLD_MAX = -math.pi / 2, -0.25, 1.2, 0.2, 1.2, 2.0
TABLE, FLY, TARGET_X, TARGET_Z, TARGET_YAW, TARGET_PITCH, SAVED_YAW, SAVED_PITCH, SPIN, TILT, READY, HOLD, \
    WAITED, LEFT = range(0, 56, 4)
STAMP, BENCH_MATRIX = base.WEAPON_TABLE_STAMP_OFFSET, base.WEAPON_TABLE_MATRIX_OFFSET   # the interface
SPEEDS = (0xE0, 0xE4, 0xE8)                    # the camera's three steps' speeds: side, height, distance
STEPS = ((0x98, 0xAC), (0x9C, 0xA4), (0xC0, 0xC4))  # their targets and values
POINT_X, POINT_Y, POINT_Z, YAW, PITCH, TURN_YAW, TURN_PITCH, WHEEL = 0xA0, 0xA4, 0xA8, 0xB8, 0xBC, 0xCC, 0xD0, 0xC8
SIDE, HEIGHT_TARGET, DISTANCE, DISTANCE_TARGET = 0xAC, 0x9C, 0xC4, 0xC0

HOOKED = {}


class Table:
    """A Lobby with the camera built by its constructor, the tick's stubs
    and the sites' stops."""

    def __init__(self, data, delta):
        self.data, self.delta = data, delta
        self.lobby = lobby = Lobby(data, delta, KITS)
        self.emu = emu = lobby.emu
        key = id(emu)
        if key not in HOOKED:
            HOOKED[key] = {"matrices": [], "stop": None, "stops": set()}
            box = HOOKED[key]

            def ret4(e, address, size, _):
                sp = e.reg_read(UC_X86_REG_ESP)
                if address == PROJECT + delta:
                    box["matrices"].append(bytes(e.mem_read(e.reg_read(UC_X86_REG_EAX) + 0, 4)))
                e.reg_write(UC_X86_REG_EIP, struct.unpack("<I", e.mem_read(sp, 4))[0])
                e.reg_write(UC_X86_REG_ESP, sp + 8)

            def stop(e, address, size, _):
                if address in box["stops"]:
                    box["stop"] = address - delta
                    e.emu_stop()

            for at in (RAY, PROJECT):
                emu.hook_add(UC_HOOK_CODE, ret4, begin=at + delta, end=at + delta)
            for at in (CULL_ON, DRAW, DECAL_PATH, HAND, DRAW_ITEM, LEFT_END):
                emu.hook_add(UC_HOOK_CODE, stop, begin=at + delta, end=at + delta)

            def over_ui(e, address, size, _):        # thiscall, no argument: al
                assert e.reg_read(UC_X86_REG_ECX) == MENU
                box["over_calls"] += 1
                e.reg_write(UC_X86_REG_EAX, 0xBADC0D00 | box["over"])
                e.reg_write(UC_X86_REG_ECX, 0xBADC0DE1)
                e.reg_write(UC_X86_REG_EDX, 0xBADC0DE2)
                sp = e.reg_read(UC_X86_REG_ESP)
                e.reg_write(UC_X86_REG_EIP, struct.unpack("<I", e.mem_read(sp, 4))[0])
                e.reg_write(UC_X86_REG_ESP, sp + 4)

            emu.hook_add(UC_HOOK_CODE, over_ui, begin=OVER_UI, end=OVER_UI)
        self.box = HOOKED[key]
        self.box["matrices"].clear()
        self.box["stops"] = set()
        self.box["over"], self.box["over_calls"] = 0, 0
        lobby.word(MENU, MENU + 0x800)                # the lobby menu's vtable: +24h is_mouse_over_ui
        lobby.word(MENU + 0x800 + 0x24, OVER_UI)
        late_join = open(base.late_join_bin(), "rb").read()
        self.has_table = any(name == base.WEAPON_CAMO_NAME for name, *_ in base.Headers(data).sections(data))
        self.state = base.late_join_state_va(late_join) + base.WEAPON_TABLE_STATE_OFFSET + delta
        # The camera as its constructor makes it: ecx = the lobby menu, the
        # collision scene on the stack.
        emu.mem_write(CAMERA, bytes(0x100))
        lobby.call(CTOR, [0x5CE7E000], ecx=MENU, eax=CAMERA)
        self.time = 100000
        lobby.word(TIME, self.time)
        lobby.word(LAST_TIME + delta, self.time - 16)

    def f(self, at):
        return from_f32(self.lobby.read(at))

    def set_f(self, at, value):
        self.lobby.word(at, f32(value))

    def s(self, field):
        return from_f32(self.lobby.read(self.state + field))

    def flag(self, field):
        return self.lobby.read(self.state + field)

    def frame(self, ms=16):
        """One tick; the camera's world matrix the menu was given."""
        self.time += ms
        self.lobby.word(TIME, self.time)
        self.box["matrices"].clear()
        self.lobby.call(TICK, [], ecx=CAMERA + 4)
        return bytes(self.emu.mem_read(CAMERA + 0x48, 0x40))

    def matrix(self):
        return struct.unpack("<16f", self.emu.mem_read(CAMERA + 0x48, 0x40))

    def camera_bytes(self):
        return bytes(self.emu.mem_read(CAMERA, 0xF0)), bytes(self.emu.mem_read(LAST_TIME + self.delta, 4))

    def site(self, start, stops, regs):
        """Runs a site of mt_draw from `start` to the first of `stops`."""
        emu = self.emu
        self.box["stops"] = {at + self.delta for at in stops}
        self.box["stop"] = None
        for register, value in regs.items():
            emu.reg_write(register, value)
        emu.reg_write(UC_X86_REG_ESP, STACK - 0x100)
        # The x87 stack empty, as the compiler leaves it between statements.
        emu.reg_write(UC_X86_REG_FPCW, 0x27F)
        emu.reg_write(UC_X86_REG_FPSW, 0)
        emu.reg_write(UC_X86_REG_FPTAG, 0xFFFF)
        fpu = emu.reg_read(UC_X86_REG_FPSW), emu.reg_read(UC_X86_REG_FPTAG)
        emu.emu_start(start + self.delta, 0, count=2000)
        self.box["stops"] = set()
        assert self.box["stop"] is not None, "it reaches one of the original's places"
        assert emu.reg_read(UC_X86_REG_ESP) == STACK - 0x100, "the stack as it was"
        assert (emu.reg_read(UC_X86_REG_FPSW) >> 11 & 7, emu.reg_read(UC_X86_REG_FPTAG)) \
            == (fpu[0] >> 11 & 7, fpu[1]), "the x87 stack as it was"
        return self.box["stop"], {r: emu.reg_read(r) for r in regs}


    def weapon(self, name, vtable=WEAPON_VTABLE, box=None, bad_record=None):
        """The player holds a weapon whose model is `name`'s (WEAPONS), its
        bones of the frame in mt_draw's frame; returns the middle it has."""
        dict_id, lower, upper, inverse_bind, pose, middle = WEAPONS[name]
        if box is not None:
            lower, upper = box
        lobby, d = self.lobby, self.delta
        lobby.word(PLAYER + 0x128, ITEM)
        lobby.word(ITEM, vtable + d)
        lobby.word(ITEM + 0x1810, MODEL)
        # weapon+25Ch is no pointer for a held weapon (2 for the VSS in the
        # owner's crash of 2026-10-21): the build must not read it
        lobby.word(ITEM + 0x25C, 2)
        # the record the player was built from: the character's copy 1, its
        # kit 2's weapon 2 (no weapon 1), the record at index 5, +4 the dict id
        copy = CHARACTER + 0x18 + 0x4634 * 1
        self.emu.mem_write(CHARACTER + 0xD2B4, bytes([1, 2]))
        self.emu.mem_write(copy + 0x57, bytes([3]))
        self.emu.mem_write(copy + 0x1564, bytes([6]))
        kit = copy + 0x58 + 2 * 0x704 + 0x40
        self.emu.mem_write(kit + 4 * 7, struct.pack("<HH", 0xFFFF, 0))
        self.emu.mem_write(kit + 4 * 10, struct.pack("<HH", 5, 1))
        self.emu.mem_write(copy + 0x1568 + 5 * 0xB4 + 4, struct.pack("<H", dict_id or 0))
        if bad_record == "copy":                  # an index out of its range: not read, the box's way
            self.emu.mem_write(CHARACTER + 0xD2B4, bytes([3]))
        elif bad_record == "kit":
            self.emu.mem_write(copy + 0x57, bytes([2]))
        elif bad_record == "record":
            self.emu.mem_write(copy + 0x1564, bytes([5]))
        lobby.word(MODEL + 0x108, RENDER)
        lobby.word(RENDER, SKELETON_VTABLE + d)
        lobby.word(RENDER + 0x62B4, SKELETON)
        self.emu.mem_write(SKELETON + 0x108, struct.pack("<6f", *lower, *upper))
        lobby.word(SKELETON + 0x140, INVERSE_BINDS)
        self.emu.mem_write(INVERSE_BINDS, struct.pack("<16f", *FLIP) + struct.pack("<16f", *inverse_bind))
        self.emu.mem_write(BONE_POSES, struct.pack("<16f", *FLIP) + struct.pack("<16f", *pose))
        lobby.word(FRAME - 0x0C, BONE_POSES)
        lobby.word(FRAME - 0x18, 2)
        if dict_id in TABLE_MIDDLES and box is None and not bad_record:   # the build's table (bind space)
            v = list(TABLE_MIDDLES[dict_id])
        elif lower == (-1.0, -1.0, -1.0) and upper == (1.0, 1.0, 1.0):
            v = None
        else:                                   # the box's centre, flipped, x 0
            centre = [(a + b) / 2 for a, b in zip(lower, upper)]
            v = [0.0, centre[1], -centre[2]]
        if v is None:
            v = [0.0, 0.0, MIDDLE_Z]
        else:                                   # through bone 1's inverse bind and pose of the frame
            for mat in (inverse_bind, pose):
                v = [sum(v[i] * mat[4 * i + k] for i in range(3)) + mat[12 + k] for k in range(3)]
        if box is None and not bad_record:
            assert math.dist(v, middle) < 0.01, (name, v, middle)   # the gun's middle within 1 cm
        return v

    def left(self, buttons, over=0):
        """on_mouse_move's drag test with the mouse's buttons: al."""
        emu = self.emu
        self.emu.mem_write(STUBS + 0x200 + 0x0C, bytes([buttons]))
        self.box["over"] = over
        regs = {UC_X86_REG_EAX: STUBS + 0x200, UC_X86_REG_EBX: 0xB0B0B0B0, UC_X86_REG_ECX: 0xC0C0C0C0,
                UC_X86_REG_EDX: 0xD0D0D0D0, UC_X86_REG_ESI: CAMERA, UC_X86_REG_EDI: 0xD1D1D1D1,
                UC_X86_REG_EBP: 0xE0E0E0E0}
        self.box["stops"] = {LEFT_END + self.delta}
        self.box["stop"] = None
        for register, value in regs.items():
            emu.reg_write(register, value)
        emu.reg_write(UC_X86_REG_ESP, STACK - 0x100)
        emu.emu_start(LEFT_SITE + self.delta, 0, count=2000)
        self.box["stops"] = set()
        assert self.box["stop"] == LEFT_END and emu.reg_read(UC_X86_REG_ESP) == STACK - 0x100
        assert all(emu.reg_read(r) == v for r, v in regs.items() if r != UC_X86_REG_EAX), \
            "every register but eax kept (only al is read after)"
        return emu.reg_read(UC_X86_REG_EAX) & 0xFF


def nearest_turn(target, yaw):
    return yaw + math.remainder(target - yaw, 2 * math.pi)


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    patched, _ = base.build(original, "section", code, None, late_join)
    checks = 0

    def say(text):
        nonlocal checks
        checks += 1
        print(f"  ok    {text}")

    for delta in (0, 0x100000):
        rebased = " (rebased)" if delta else ""

        # No weapon's window: the build's tick is the original's.
        runs = []
        for data in (original, patched):
            table = Table(data, delta)
            out = []
            for n in range(90):
                table.set_f(CAMERA + TURN_YAW, 0.03 if n < 30 else 0.0)
                table.set_f(CAMERA + TURN_PITCH, -0.02 if 10 <= n < 40 else 0.0)
                table.set_f(CAMERA + WHEEL, -0.2 if 50 <= n < 55 else 0.0)
                out.append((table.frame(), table.camera_bytes()))
            runs.append(out)
        assert runs[0] == runs[1]
        say(f"no weapon's window{rebased}: 90 ticks of the build, with a drag and the wheel, give every byte of "
            "the camera and every matrix of the original's")

        # The weapon never drawn there: the camera goes anyway after HOLD_MAX.
        other = Table(patched, delta)
        other.lobby.handler(START_HANDLER, AK)
        frames = 0
        while other.flag(HOLD) and frames < 400:
            other.frame()
            frames += 1
        assert other.flag(HOLD) == 0 and math.isclose(frames * 0.016, HOLD_MAX, abs_tol=0.02), frames
        for n in range(10):
            other.frame()
        assert other.f(CAMERA + POINT_X) > 0.5
        say(f"a weapon never drawn at the bench{rebased}: the camera goes after {frames} ticks "
            f"({frames * 0.016:.2f} s, HOLD_MAX {HOLD_MAX})")

        # A weapon's window: the camera flies to the bench.
        table = Table(patched, delta)
        table.set_f(CAMERA + YAW, 0.4)
        table.set_f(CAMERA + PITCH, 0.1)
        table.lobby.handler(START_HANDLER, AK)
        assert (table.flag(TABLE), table.flag(FLY)) == (1, 1)
        assert (table.s(TARGET_X), table.s(TARGET_Z), table.s(TARGET_PITCH)) == \
            tuple(from_f32(f32(v)) for v in (BENCH[0], BENCH[2], VIEW_PITCH))
        assert (table.s(SAVED_YAW), table.s(SAVED_PITCH)) == (from_f32(f32(0.4)), from_f32(f32(0.1)))
        assert math.isclose(table.s(TARGET_YAW), nearest_turn(VIEW_YAW, 0.4), abs_tol=1e-5)
        assert (round(table.f(CAMERA + HEIGHT_TARGET), 6), round(table.f(CAMERA + DISTANCE_TARGET), 6),
                round(table.f(CAMERA + 0x98), 6)) == (BENCH[1], VIEW_DISTANCE, 0.37)
        # The weapon not at the bench yet: the camera holds.
        held = [table.f(CAMERA + at) for at in (POINT_X, POINT_Z, YAW, PITCH)]
        for n in range(30):
            table.frame()
        assert table.flag(HOLD) == 1 and [table.f(CAMERA + at) for at in (POINT_X, POINT_Z, YAW, PITCH)] == held
        assert [table.f(CAMERA + at) for at in SPEEDS] == [0.0, 0.0, 0.0]
        # The weapon drawn there (mt_draw's matrix sets READY): it flies, the
        # three steps with the setter's speeds again.
        table.lobby.word(table.state + READY, 1)
        table.frame()
        assert table.flag(HOLD) == 0
        for at, (target, now) in zip(SPEEDS, STEPS):
            expected = abs(table.f(CAMERA + target) - table.f(CAMERA + now)) * from_f32(f32(1.1111112))
            # the step after the speeds were set has moved `now` by one frame of them
            assert math.isclose(table.f(CAMERA + at), expected, rel_tol=0.05, abs_tol=1e-6), (hex(at), expected)
        seconds = None
        for n in range(300):
            table.frame()
            point = (table.f(CAMERA + POINT_X), table.f(CAMERA + POINT_Y), table.f(CAMERA + POINT_Z))
            if seconds is None and all(abs(a - b) < 0.01 for a, b in zip(point, BENCH)):
                seconds = (n + 1) * 0.016
        assert seconds is not None and seconds < 1.5, seconds
        point = (table.f(CAMERA + POINT_X), table.f(CAMERA + POINT_Y), table.f(CAMERA + POINT_Z))
        # x and z exactly (this code); the height within 0.001 (the camera's own
        # step, 0x283F62..0x284018, stops there)
        assert (point[0], point[2]) == (from_f32(f32(BENCH[0])), from_f32(f32(BENCH[2]))) \
            and abs(point[1] - BENCH[1]) < 1e-3, point
        assert (table.f(CAMERA + YAW), table.f(CAMERA + PITCH)) == (table.s(TARGET_YAW), table.s(TARGET_PITCH))
        matrix = table.matrix()
        eye = matrix[12:15]
        assert math.isclose(math.dist(eye, point), VIEW_DISTANCE, abs_tol=1e-3), eye
        expected_eye = (BENCH[0] - VIEW_DISTANCE * math.cos(VIEW_PITCH), point[1] - VIEW_DISTANCE * math.sin(VIEW_PITCH),
                        BENCH[2])
        assert all(abs(a - b) < 1e-3 for a, b in zip(eye, expected_eye)), (eye, expected_eye)
        assert math.isclose(table.f(CAMERA + SIDE), 0.37, abs_tol=1e-5)
        say(f"a weapon's window{rebased}: the camera holds (point, yaw and pitch kept, its steps' speeds 0) "
            f"until the weapon is drawn at the bench, then gets the setter's speeds back; "
            f"the camera's point is at the bench {BENCH} within 1 cm after "
            f"{seconds:.2f} s, then x and z exactly and the height within 0.001 (the camera's own step); its eye at ({eye[0]:.3f}, {eye[1]:.3f}, {eye[2]:.3f}): "
            f"{VIEW_DISTANCE} m in front of it along -x, {-VIEW_DISTANCE * math.sin(VIEW_PITCH):.3f} m above; "
            "the side angle the stock 0.37; the yaw and pitch it had kept")

        # A drag turns the weapon, not the camera.
        yaw, pitch = table.f(CAMERA + YAW), table.f(CAMERA + PITCH)
        table.set_f(CAMERA + TURN_YAW, 0.1)
        table.set_f(CAMERA + TURN_PITCH, 0.05)
        table.frame()
        assert (table.f(CAMERA + YAW), table.f(CAMERA + PITCH)) == (yaw, pitch)
        assert (table.f(CAMERA + TURN_YAW), table.f(CAMERA + TURN_PITCH)) == (0.0, 0.0)
        assert math.isclose(table.s(SPIN), 0.1, rel_tol=1e-6) and math.isclose(table.s(TILT), 0.05, rel_tol=1e-6)
        table.set_f(CAMERA + TURN_PITCH, 5.0)
        table.frame()
        assert table.s(TILT) == from_f32(f32(TILT_MAX))
        table.set_f(CAMERA + TURN_PITCH, -50.0)
        table.frame()
        assert table.s(TILT) == from_f32(f32(-TILT_MAX))
        say(f"a drag at the bench{rebased}: the yaw's move turns the weapon's spin, the pitch's its tilt (within "
            f"+-{TILT_MAX}), the camera's yaw and pitch stay")

        # The close: back to the origin and the yaw and pitch it had.
        table.lobby.handler(EXIT_HANDLER, 0)
        assert (table.flag(TABLE), table.flag(FLY)) == (0, 1)
        for n in range(400):
            table.frame()
            if not table.flag(FLY):
                break
        assert not table.flag(FLY), "it gets back"
        back = n + 1
        assert (table.f(CAMERA + POINT_X), table.f(CAMERA + POINT_Z)) == (0.0, 0.0)
        assert (table.f(CAMERA + YAW), table.f(CAMERA + PITCH)) == (from_f32(f32(0.4)), from_f32(f32(0.1)))
        # From then on the original's: the same camera in the original image.
        stock = Table(original, delta)
        stock.emu.mem_write(CAMERA, bytes(table.emu.mem_read(CAMERA, 0x100)))
        stock.time = table.time
        stock.lobby.word(LAST_TIME + delta, table.lobby.read(LAST_TIME + delta))
        for n in range(60):
            for t in (table, stock):
                t.set_f(CAMERA + TURN_YAW, 0.02 if n < 20 else 0.0)
            assert table.frame() == stock.frame() and table.camera_bytes() == stock.camera_bytes()
        say(f"the close{rebased}: the camera is back after {back} ticks of 16 ms at the origin's point and the "
            "yaw and pitch it had, exactly; then 60 ticks (with a drag) as the original's")

        # Windows over windows.
        table = Table(patched, delta)
        table.set_f(CAMERA + YAW, -2.0)
        table.lobby.handler(START_HANDLER, SVD)
        for n in range(20):
            table.frame()
        table.lobby.word(table.state + READY, 1)          # the SVD at the bench
        table.lobby.handler(START_HANDLER, AK)
        assert (table.flag(TABLE), table.s(SAVED_YAW), table.flag(READY)) == (1, from_f32(f32(-2.0)), 1)
        table.frame()
        assert table.flag(HOLD) == 0                       # the bench keeps its weapon: no wait
        assert math.isclose(table.s(TARGET_YAW), nearest_turn(VIEW_YAW, table.f(CAMERA + YAW)), abs_tol=1e-5)
        table.lobby.handler(START_HANDLER, VEST)
        assert (table.flag(TABLE), table.flag(FLY), table.s(TARGET_X), table.s(TARGET_Z), table.s(TARGET_YAW)) \
            == (0, 1, 0.0, 0.0, from_f32(f32(-2.0)))
        assert round(table.f(CAMERA + HEIGHT_TARGET), 6) == 1.3       # the vest's own
        table.lobby.handler(EXIT_HANDLER, 0)
        for n in range(400):
            table.frame()
        assert not table.flag(FLY) and table.f(CAMERA + YAW) == from_f32(f32(-2.0))
        say(f"windows over windows{rebased}: the AK's over the SVD's keeps the yaw kept at the first and the "
            "bench its weapon (READY, no hold); a vest's "
            "over them flies back (the vest's own camera); the close leaves the stock camera")

        # The yaw it flies to: the nearest turn of -pi/2.
        for yaw in (0.0, 3.0, -3.0, -7.5, 20.25, 100.0):
            table = Table(patched, delta)
            table.set_f(CAMERA + YAW, yaw)
            table.lobby.handler(START_HANDLER, AK)
            target, start = table.s(TARGET_YAW), from_f32(f32(yaw))
            assert abs(target - start) <= math.pi + 1e-4 and \
                abs(math.remainder(target - VIEW_YAW, 2 * math.pi)) < 1e-4, (yaw, target)
        say(f"the bench's yaw{rebased}: -pi/2 the nearest turn of it to the camera's yaw (0, 3, -3, -7.5, 20.25, "
            "100)")

        # mt_draw's two sites.
        cases = []
        for local in (0, 1):
            for on in (0, 1):
                for caller in (LOBBY_DRAW, 0x1F7DD3):
                    for player in (PLAYER, OTHER_PLAYER):
                        for opened in (0, 1):
                            for decal in (0, 1):
                                cases.append((local, on, caller, player, opened, decal))
        culled = drawn = 0
        for local, on, caller, player, opened, decal in cases:
            ends = []
            for data in (original, patched):
                table = Table(data, delta)
                lobby = table.lobby
                if table.has_table:
                    lobby.word(table.state + TABLE, on)
                    lobby.word(table.state + SPIN, f32(0.7))
                    lobby.word(table.state + TILT, f32(-0.3))
                table.emu.mem_write(FRAME - 1, bytes([local]))
                lobby.word(FRAME + 4, caller + delta)
                lobby.word(FRAME - 0xF0, CHARACTER)
                lobby.word(CHARACTER + 0xD2B8, PLAYER)
                lobby.word(CHARACTER + 0xD2C4, -1)
                table.emu.mem_write(CHARACTER + 0xD515, bytes([opened]))
                table.emu.mem_write(player + 0xF49C, bytes([decal]))
                regs = {UC_X86_REG_EAX: 0xA0A0A0A0, UC_X86_REG_EBX: player, UC_X86_REG_ECX: 0xC0C0C0C0,
                        UC_X86_REG_EDX: 0xD0D0D0D0, UC_X86_REG_ESI: 0x5E5E5E5E, UC_X86_REG_EDI: 0xD1D1D1D1,
                        UC_X86_REG_EBP: FRAME}
                cull_end, cull_regs = table.site(CULL, (CULL_ON, DRAW), regs)
                matrix_end, matrix_regs = table.site(MATRIX, (DECAL_PATH, HAND, DRAW_ITEM), regs)
                transform = struct.unpack("<16f", table.emu.mem_read(FRAME - 0xE8, 0x40))
                ends.append((cull_end, cull_regs, matrix_end, matrix_regs, transform))
            (o_cull, o_cull_regs, o_matrix, o_matrix_regs, _), (p_cull, p_cull_regs, p_matrix, p_matrix_regs, m) = ends
            assert o_cull_regs == regs and p_cull_regs == regs, "every register kept"
            mine = on and caller == LOBBY_DRAW and player == PLAYER and opened
            assert o_cull == (DRAW if local else CULL_ON)
            assert p_cull == (DRAW if local or mine else CULL_ON), (local, on, caller, player, opened, p_cull)
            assert o_matrix == (DECAL_PATH if decal else HAND) and o_matrix_regs == regs
            if not mine:
                assert p_matrix == o_matrix and p_matrix_regs == regs
                continue
            assert p_matrix == DRAW_ITEM and p_matrix_regs[UC_X86_REG_EAX] == FRAME - 0xE8
            assert all(p_matrix_regs[r] == v for r, v in regs.items() if r != UC_X86_REG_EAX)
            rows = [m[0:3], m[4:7], m[8:11]]
            for i in range(3):
                for j in range(3):
                    dot = sum(a * b for a, b in zip(rows[i], rows[j]))
                    assert math.isclose(dot, 1.0 if i == j else 0.0, abs_tol=1e-5), (i, j, dot)
            assert (m[3], m[7], m[11], m[15]) == (0.0, 0.0, 0.0, 1.0)
            # No held item: (0, 0, MIDDLE_Z) of the model at the bench; up and the length turned by the spin and the tilt.
            middle = [MIDDLE_Z * m[8 + k] + m[12 + k] for k in range(3)]
            assert all(abs(a - b) < 1e-5 for a, b in zip(middle, BENCH)), middle
            spin, tilt = from_f32(f32(0.7)), from_f32(f32(-0.3))
            length = (-math.sin(spin) * math.cos(tilt), -math.sin(spin) * math.sin(tilt), math.cos(spin))
            assert all(abs(a - b) < 1e-5 for a, b in zip(rows[2], length)), rows[2]
            assert all(abs(a - b) < 1e-5 for a, b in zip(rows[1], (-math.sin(tilt), math.cos(tilt), 0.0)))
            drawn += 1
            culled += p_cull == DRAW and not local
        assert drawn == 4 and culled == 2
        say(f"mt_draw's sites{rebased}, {len(cases)} cases: the culled test and the held item's transform as the "
            "original's, every register kept, but for the table's player (the lobby character's, drawn by its "
            "update, its window open, the table on): the draw path even when not seen, and the bench's "
            "transform in eax (orthonormal rows, the weapon's middle at the bench, its length along (-sin spin "
            "cos tilt, -sin spin sin tilt, cos spin)); the x87 stack as it was")

        # The weapon's middle, and the character still making its player.
        def bench_case(setup, building=None):
            table = Table(patched, delta)
            lobby = table.lobby
            lobby.word(table.state + TABLE, 1)
            lobby.word(table.state + SPIN, f32(0.9))
            lobby.word(table.state + TILT, f32(0.4))
            lobby.word(FRAME + 4, LOBBY_DRAW + delta)
            lobby.word(FRAME - 0xF0, CHARACTER)
            lobby.word(CHARACTER + 0xD2B8, PLAYER)
            lobby.word(CHARACTER + 0xD2C4, -1)
            table.emu.mem_write(CHARACTER + 0xD515, b"\x01")
            if building == "query":
                lobby.word(CHARACTER + 0xD2C4, 7)
            elif building == "again":
                table.emu.mem_write(CHARACTER + 0xD2C8, b"\x01")
            middle = setup(table)
            regs = {UC_X86_REG_EAX: 0xA0A0A0A0, UC_X86_REG_EBX: PLAYER, UC_X86_REG_ECX: 0xC0C0C0C0,
                    UC_X86_REG_EDX: 0xD0D0D0D0, UC_X86_REG_ESI: 0x5E5E5E5E, UC_X86_REG_EDI: 0xD1D1D1D1,
                    UC_X86_REG_EBP: FRAME}
            end, out = table.site(MATRIX, (DECAL_PATH, HAND, DRAW_ITEM), regs)
            m = struct.unpack("<16f", table.emu.mem_read(FRAME - 0xE8, 0x40))
            return table, end, out, m, middle, regs

        for building in ("query", "again"):
            table, end, out, _, _, regs = bench_case(lambda t: t.weapon("AK-12"), building)
            assert end == HAND and out == regs and table.flag(READY) == 0, (building, hex(end))

            def shown(t):
                t.lobby.word(t.state + READY, 1)
                return t.weapon("AK-12")
            table, end, out, m, middle, regs = bench_case(shown, building)
            assert end == DRAW_ITEM and out[UC_X86_REG_EAX] == FRAME - 0xE8 and table.flag(READY) == 1
            at = [sum(middle[i] * m[4 * i + k] for i in range(3)) + m[12 + k] for k in range(3)]
            assert all(abs(a - b) < 1e-5 for a, b in zip(at, BENCH)), at
        say(f"the character still making its player{rebased} (+D2C4h a query, or +D2C8h a setup_profile "
            "waiting): before the bench showed a weapon, the held item stays in the hand, READY 0, every "
            "register kept; once it did (a refresh: a scope or a camouflage put on), the current player's "
            "weapon stays at the bench")
        for name, setup in (("AK-12", lambda t: t.weapon("AK-12")), ("PM", lambda t: t.weapon("PM")),
                            ("the VSS (a spare magazine parked beside it in its box)", lambda t: t.weapon("VSS")),
                            ("the AK-74U (a default box; the build's table)", lambda t: t.weapon("AK-74U")),
                            ("the Vepr in its 3rd-view idle (the table, bind space)", lambda t: t.weapon("Vepr (idle)")),
                            ("the Vepr in its preview (the same entry, bone 1 elsewhere)",
                             lambda t: t.weapon("Vepr (preview)")),
                            ("the AK-74U, its copy's index out of range", lambda t: t.weapon("AK-74U", bad_record="copy")),
                            ("the AK-74U, its kit past the count", lambda t: t.weapon("AK-74U", bad_record="kit")),
                            ("the AK-74U, its record past the table", lambda t: t.weapon("AK-74U", bad_record="record")),
                            ("a default box", lambda t: (t.weapon("AK-12", box=((-1.0, -1.0, -1.0), (1.0, 1.0, 1.0))),
                                                         (0.0, 0.0, MIDDLE_Z))[1]),
                            ("not a weapon", lambda t: (t.weapon("AK-12", vtable=0xA04174 + 0x40),
                                                        (0.0, 0.0, MIDDLE_Z))[1])):
            table, end, out, m, middle, regs = bench_case(setup)
            assert end == DRAW_ITEM and out[UC_X86_REG_EAX] == FRAME - 0xE8 and table.flag(READY) == 1
            assert all(out[r] == v for r, v in regs.items() if r != UC_X86_REG_EAX)
            at = [sum(middle[i] * m[4 * i + k] for i in range(3)) + m[12 + k] for k in range(3)]
            assert all(abs(a - b) < 1e-5 for a, b in zip(at, BENCH)), (name, at)
            # the interface: the transform drawn with, at BENCH_MATRIX, and the stamp
            assert struct.unpack("<16f", table.emu.mem_read(table.state + BENCH_MATRIX, 64)) == m
            assert table.flag(STAMP) == 1
            say(f"{name}{rebased}: its middle ({', '.join(f'{c:.4f}' for c in middle)}) of the model is drawn at "
                "the bench, READY 1, every register but eax kept; the transform at the state's +50h, its stamp +1")

        # The left button at the bench.
        table = Table(patched, delta)
        stock = Table(original, delta)
        for buttons in (0x00, 0x01, 0x02, 0x10, 0x40, 0x03):
            assert table.left(buttons) == stock.left(buttons) == (1 if buttons & 0x52 else 0), hex(buttons)
        assert table.box["over_calls"] == 0
        lobby = table.lobby
        lobby.word(table.state + TABLE, 1)
        lobby.word(table.state + HOLD, 1)
        assert table.left(0x01) == 0 and table.box["over_calls"] == 0          # still flying there
        lobby.word(table.state + HOLD, 0)
        assert table.left(0x01, over=0) == 1 and table.flag(LEFT) == 1         # a press off the movie
        assert table.left(0x01, over=1) == 1 and table.box["over_calls"] == 1  # asked once, at the first move
        assert table.left(0x00) == 0 and table.flag(LEFT) == 0                 # let go: the drag ends
        assert table.left(0x01, over=1) == 0 and table.flag(LEFT) == 2         # a press on the movie: an item, a control
        assert table.left(0x01, over=0) == 0 and table.box["over_calls"] == 2  # not until it is let go
        assert table.left(0x00) == 0 and table.flag(LEFT) == 0
        assert table.left(0x02, over=1) == 1                                   # the camera's own buttons as before
        lobby.word(table.state + TABLE, 0)
        assert table.left(0x01, over=0) == 0
        say(f"the drag's buttons{rebased}: 52h as the original's, the left button not; at the bench (the "
            "camera there) the left button too when its press is off the movie (is_mouse_over_ui asked once, at "
            "its first move), not when it is on it (an item dragged, a control) until it is let go; every "
            "register but eax kept")

    print(f"patch 45: {checks} checks passed")


if __name__ == "__main__":
    main()
