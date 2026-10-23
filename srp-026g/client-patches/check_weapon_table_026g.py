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
- the camera flies at the window's start (round 5, 2026-10-23: no wait for
  the weapon), its targets set by the start, the point moving from the
  first tick; every part of it (the point, the yaw, the pitch, and the
  camera's own side angle, height and distance steps, whose speeds the
  flight sets each tick) eases out together: per tick the eye and the view
  move less and less, never faster than the tick before, with no sudden
  stop mid-flight, nothing at READY, and no snap at the end (round 5b: the
  owner's "the camera jumps a bit at the end of the transition"), there and
  back;
- the weapon's middle (its model's box through bone 1's inverse bind and
  pose, as the data of the AK-12 and the PM has them) is drawn at the bench;
  while the character makes the window's player the held item is drawn
  with the decal pose's transform (10 m under: neither in the hand nor at
  the bench), and at the bench from the first draw once it is made;
- the left button drags with the table on when its press is off the movie;
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
    rows = re.findall(r"^(?:middles:)?\s+GUN (\d+), (-?\d+), (-?\d+), (-?\d+)", text[text.index("middles:"):], re.M)
    return {int(d): (int(x) / 1000, int(y) / 1000, int(z) / 1000) for d, x, y, z in rows}


TABLE_MIDDLES = table_middles()

# weapon_table_026g.asm's constants and state
BENCH = (4.25, 1.30, 3.05)
VIEW_YAW, VIEW_PITCH, MIDDLE_Z, TILT_MAX = -math.pi / 2, -0.25, 0.2, 1.2
# The bench's view (round 5d): the original's 65 degrees (vertical) whatever uro_fov, the weapon's
# middle in the middle of the screen's part left of the upgrade window, as near as keeps its ends on
# the screen (2% in) at any spin, at least 0.3 m away (weapon_table_026g.asm's frame_view)
ORIGINAL_FOV, MARGIN, NEAR_VIEW, TILT_ROOM = 65.0, 0.02, 0.3, 0.962
URO_FOV = 0xA6FA28                              # uro_fov's value (patch 22)


FRAME_SHIFT, PANEL_RIGHT, SQUAD_RIGHT = 365, 882, 88   # lobby movie units (weapon_table_026g.asm's)
COS_PITCH, SIN_PITCH = math.cos(-0.25), math.sin(-0.25)


def frame(length, aspect=16 / 9):
    """(the camera's distance, its side angle, the weapon's middle (of the width), the upgrade window's left
    edge, the Squad strip's right edge): weapon_table_026g.asm's frame_view."""
    layout = 1080 * (aspect if aspect >= 1.85 else 16 / 9)   # the lobby movie's width in units
    s, p = FRAME_SHIFT / layout, PANEL_RIGHT / layout
    big = math.tan(math.radians(ORIGINAL_FOV / 2)) * aspect
    t = 2 * s * big
    left = math.sqrt(1 / big ** 2 + (1 - 2 * MARGIN) ** 2) / (4 * (0.5 - s - MARGIN))
    right = math.sqrt(1 / big ** 2 + (1 - 2 * MARGIN - 2 * p) ** 2) / (4 * (0.5 - p - MARGIN + s))
    d = max(left, right, TILT_ROOM) * length
    distance = max(d * math.sqrt(1 + t * t), NEAR_VIEW)
    side = math.atan(t * COS_PITCH) + math.asin(t * SIN_PITCH ** 2 / (COS_PITCH * math.sqrt(1 + (t * COS_PITCH) ** 2)))
    return distance, side, 0.5 - s, 1 - p, SQUAD_RIGHT / layout


def zoom_angle(zoom, uro):
    """patch 22's projection: the vertical field of view (degrees) for a camera zoom."""
    return 65 * zoom + (uro - 65) * max(0.0, (zoom - 0.25) / 0.75)


VIEW_DISTANCE, VIEW_SIDE = frame(1.0)[:2]     # a 1 m weapon (until the weapon's own length), 16:9
LOOK = BENCH                                    # the camera looks at the weapon's middle (round 5e)
TABLE, FLY, TARGET_X, TARGET_Z, TARGET_YAW, TARGET_PITCH, SAVED_YAW, SAVED_PITCH, SPIN, TILT, READY = range(0, 44, 4)
LEFT, LENGTH = 0x34, 0x30                      # (+2Ch EASE; +30h the weapon's length, round 5d)
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
                if address == RAY + delta:
                    box["rays"] += 1
                    if box["hit"] is not None:    # something in the way: the hit's distance over [edi]
                        e.mem_write(e.reg_read(UC_X86_REG_EDI), struct.pack("<f", box["hit"]))
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
        self.box["hit"], self.box["rays"] = None, 0
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

        # A weapon's window: the camera flies to the bench.
        table = Table(patched, delta)
        table.set_f(CAMERA + YAW, 0.4)
        table.set_f(CAMERA + PITCH, 0.1)
        table.lobby.handler(START_HANDLER, AK)
        assert (table.flag(TABLE), table.flag(FLY)) == (1, 1)
        assert (table.s(TARGET_X), table.s(TARGET_PITCH)) == tuple(from_f32(f32(v)) for v in (BENCH[0], VIEW_PITCH))
        assert table.s(TARGET_Z) == from_f32(f32(BENCH[2])), table.s(TARGET_Z)
        assert (table.s(SAVED_YAW), table.s(SAVED_PITCH)) == (from_f32(f32(0.4)), from_f32(f32(0.1)))
        assert math.isclose(table.s(TARGET_YAW), nearest_turn(VIEW_YAW, 0.4), abs_tol=1e-5)
        assert round(table.f(CAMERA + HEIGHT_TARGET), 6) == BENCH[1] \
            and math.isclose(table.f(CAMERA + 0x98), VIEW_SIDE, abs_tol=1e-5), table.f(CAMERA + 0x98)
        assert math.isclose(table.f(CAMERA + DISTANCE_TARGET), VIEW_DISTANCE, abs_tol=1e-5)
        # The weapon not at the bench yet (READY 0): the camera flies anyway,
        # from the first tick.
        before = [table.f(CAMERA + at) for at in (POINT_X, POINT_Z, YAW, PITCH, POINT_Y, DISTANCE)]
        table.frame()
        after = [table.f(CAMERA + at) for at in (POINT_X, POINT_Z, YAW, PITCH, POINT_Y, DISTANCE)]
        assert table.flag(READY) == 0 and all(a != b for a, b in zip(before, after)), (before, after)
        # the camera's own steps' speeds 0 while the flight moves them itself (round 5c)
        assert [table.f(CAMERA + at) for at in SPEEDS] == [0.0, 0.0, 0.0] and table.flag(0x2C) == 1
        seconds = None
        for n in range(300):
            table.frame()
            point = (table.f(CAMERA + POINT_X), table.f(CAMERA + POINT_Y), table.f(CAMERA + POINT_Z))
            if seconds is None and all(abs(a - b) < 0.01 for a, b in zip(point, LOOK)):
                seconds = (n + 2) * 0.016
        assert seconds is not None and seconds < 1.5, seconds
        point = (table.f(CAMERA + POINT_X), table.f(CAMERA + POINT_Y), table.f(CAMERA + POINT_Z))
        # every value exactly (the flight lands each on its target)
        assert point == (from_f32(f32(BENCH[0])), from_f32(f32(BENCH[1])), table.s(TARGET_Z)), point
        assert (table.f(CAMERA + YAW), table.f(CAMERA + PITCH)) == (table.s(TARGET_YAW), table.s(TARGET_PITCH))
        matrix = table.matrix()
        eye = matrix[12:15]
        assert math.isclose(math.dist(eye, point), VIEW_DISTANCE, abs_tol=1e-4), eye
        expected_eye = (BENCH[0] - VIEW_DISTANCE * math.cos(VIEW_PITCH), point[1] - VIEW_DISTANCE * math.sin(VIEW_PITCH),
                        LOOK[2])
        assert all(abs(a - b) < 1e-4 for a, b in zip(eye, expected_eye)), (eye, expected_eye)
        assert table.f(CAMERA + SIDE) == table.f(CAMERA + 0x98) and table.f(CAMERA + 0x94) == 1.0  # uro_fov 65: zoom 1
        say(f"a weapon's window{rebased}: the camera flies at the start, before the weapon is at the bench "
            f"(its point, yaw, pitch, height and distance move at the first tick, moved by it, the steps' speeds 0); "
            f"the camera's point is at the bench {BENCH} (the weapon's middle) within 1 cm after {seconds:.2f} s, "
            f"then exactly; its eye at ({eye[0]:.3f}, {eye[1]:.3f}, {eye[2]:.3f}): {VIEW_DISTANCE:.3f} m (a 1 m "
            f"weapon's) in front of it along -x, {-VIEW_DISTANCE * math.sin(VIEW_PITCH):.3f} m above; the side angle "
            f"{VIEW_SIDE:.4f} (the weapon left of the view's middle); the yaw and pitch it had kept")

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

        # (a Table of its own: the emulator is the lobby's, made again)
        # The flight is smooth, there and back: every tick the eye and the view
        # move no more than the tick before (an ease-out), with no sudden stop
        # mid-flight (READY at the 12th tick changes nothing) and no snap at
        # the end.
        def moves(t, ticks, ready_at=None):
            out, last = [], None
            for n in range(ticks):
                if n == ready_at:
                    t.lobby.word(t.state + READY, 1)
                t.frame()
                m = t.matrix()
                pose = (m[12:15], m[8:11])
                if last is not None:
                    out.append(math.dist(pose[0], last[0]) + VIEW_DISTANCE * math.dist(pose[1], last[1]))
                last = pose
            return out

        def smooth(d):
            moving = [x for x in d if x > 0]
            last = max(i for i, x in enumerate(d) if x > 0)
            return max(d) < 0.8 and d[last] < 5e-4 and all(
                d[n] <= d[n - 1] * 1.01 + 2e-4 and (d[n - 1] < 2e-3 or d[n] >= 0.75 * d[n - 1])
                for n in range(2, len(d))), (max(moving), last, d[last])

        flight = Table(patched, delta)
        flight.set_f(CAMERA + YAW, 0.4)
        flight.set_f(CAMERA + PITCH, 0.1)
        flight.lobby.handler(START_HANDLER, AK)
        there = moves(flight, 300, ready_at=12)
        ok_there, (top, end, last_move) = smooth(there)
        assert ok_there, (top, end, last_move)
        assert flight.flag(0x2C) == 0                      # the easing over: the wheel's speeds the stock's
        flight.lobby.handler(EXIT_HANDLER, 0)
        back = moves(flight, 300)
        ok_back, (top_back, end_back, last_back) = smooth(back)
        assert ok_back and not flight.flag(FLY), (top_back, end_back, last_back)
        say(f"the flight{rebased}, there and back: the eye and the view move at most {top:.3f} (there) and "
            f"{top_back:.3f} (back) a 16 ms tick, each tick no more than the one before and never less than "
            f"3/4 of it while moving (no sudden stop of a step mid-flight, nothing at READY), the last move "
            f"{last_move * 1000:.2f} mm there and {last_back * 1000:.2f} mm back (no snap at the end)")

        # The collision ray (round 5c, the owner's log of 2026-10-23): the
        # window's player, once made, put something in the ray's way by the
        # bench and the tick cut the distance to 0.6. Here the ray reports a
        # hit at 0.6 from READY on: while the camera is the table's no ray is
        # cast, the flight is as smooth as without it and rests at exactly
        # the bench's distance, height and side angle; back at the stock
        # camera the ray is the original's again.
        hit = Table(patched, delta)
        hit.set_f(CAMERA + YAW, 0.4)
        hit.set_f(CAMERA + PITCH, 0.1)
        hit.lobby.handler(START_HANDLER, AK)
        clear = moves(hit, 12)
        hit.box["hit"] = 0.6
        hit.lobby.word(hit.state + READY, 1)
        cut = moves(hit, 300)
        ok_hit, (_, _, last_hit) = smooth(clear + cut)
        resting = [hit.f(CAMERA + at) for at in (DISTANCE, POINT_Y, SIDE)]
        assert ok_hit and hit.box["rays"] == 0 and resting[1] == from_f32(f32(BENCH[1])) \
            and math.isclose(resting[2], VIEW_SIDE, abs_tol=1e-5) \
            and resting[0] == hit.f(CAMERA + DISTANCE_TARGET) and math.isclose(resting[0], VIEW_DISTANCE, abs_tol=1e-5), \
            (ok_hit, hit.box["rays"], resting)
        eye = hit.matrix()[12:15]
        assert math.isclose(math.dist(eye, LOOK), VIEW_DISTANCE, abs_tol=1e-4), eye
        hit.lobby.handler(EXIT_HANDLER, 0)
        flown = 0
        while hit.flag(FLY) and flown < 400:
            back = moves(hit, 2)                # (one move a call: the tick before the last)
            flown += 2
        # the tick that gave the camera back casts the stock ray already
        assert not hit.flag(FLY) and hit.box["rays"] <= 1, (flown, hit.box["rays"])
        hit.frame()
        # the stock camera's ray cuts it, to the least (1.5 again once back)
        assert hit.box["rays"] >= 1 and hit.f(CAMERA + DISTANCE) == from_f32(f32(1.5)), hit.f(CAMERA + DISTANCE)
        say(f"the collision ray{rebased}: with a hit at 0.6 from READY on, no ray while the camera is the "
            "table's (there, at the bench, back): the flight as smooth (each tick no more than the one before), "
            "at rest the distance exactly its target, the height 1.3 and the side angle frame()'s, the eye that far from the "
            "point; once the stock camera is back its ray is cast and cuts the distance (to the least, 1.5) as the original's")

        # The bench's view on the screen (round 5d, the owner: the weapon "a bit too far", "off center to
        # the right", "front and center", "fov ... just like we did with scopes"): at any uro_fov (50, 65,
        # 100) and aspect (16:9, 16:10, 21:9) the camera's zoom gives patch 22's projection the original's
        # 65 degrees, the weapon's middle is projected to the middle of the screen's part left of the
        # upgrade window (and the screen's middle in height), the camera is frame()'s distance for the
        # weapon's length, and over a full spin (tilts 0 and +-1.2) its ends stay on the screen; after the
        # close the zoom is 1 again (the player's field of view).
        SVD_BOX = ((-0.03, -0.15, -0.937), (0.03, 0.11, 0.284))       # the SVD's render box (1.22 m)

        def on_screen(t, uro, aspect, point):
            m = t.matrix()
            right, up, fwd, eye = m[0:3], m[4:7], m[8:11], m[12:15]
            rel = [a - b for a, b in zip(point, eye)]
            x, y, z = (sum(a * b for a, b in zip(rel, axis)) for axis in (right, up, fwd))
            tv = math.tan(math.radians(zoom_angle(t.f(CAMERA + 0x94), uro) / 2))
            return 0.5 + x / (2 * z * tv * aspect), 0.5 - y / (2 * z * tv), z

        views = []
        for width, height in ((1280, 720), (1920, 1200), (3440, 1440), (3840, 1080), (5120, 1440)):
            aspect = width / height
            for uro in (50.0, 65.0, 100.0):
                for name, box in (("AK-12", None), ("SVD", SVD_BOX), ("PM", None)):
                    t = Table(patched, delta)
                    t.lobby.word(URO_FOV + delta, f32(uro))
                    viewport = UI + 0xF800
                    t.lobby.word(lobby_check.OWNER + 0x53B0, viewport)
                    t.lobby.word(viewport + 0x108, width)
                    t.lobby.word(viewport + 0x10C, height)
                    t.set_f(CAMERA + YAW, 0.4)
                    t.lobby.handler(START_HANDLER, AK)
                    for n in range(15):
                        t.frame()
                    t.lobby.word(FRAME + 4, LOBBY_DRAW + delta)       # mt_draw for the character's update
                    t.lobby.word(FRAME - 0xF0, CHARACTER)
                    t.lobby.word(CHARACTER + 0xD2B8, PLAYER)
                    t.lobby.word(CHARACTER + 0xD2C4, -1)
                    middle = t.weapon("AK-12" if name == "SVD" else name, box=box)
                    regs = {UC_X86_REG_EAX: 0, UC_X86_REG_EBX: PLAYER, UC_X86_REG_ECX: 0, UC_X86_REG_EDX: 0,
                            UC_X86_REG_ESI: 0, UC_X86_REG_EDI: 0, UC_X86_REG_EBP: FRAME}
                    t.site(MATRIX, (DECAL_PATH, HAND, DRAW_ITEM), regs)    # the weapon at the bench: READY, its length
                    for n in range(300):
                        t.frame()
                    lower, upper = box or WEAPONS[name][1:3]
                    length = upper[2] - lower[2]
                    d, side, fx, window, squad = frame(length, aspect)
                    assert t.flag(READY) == 1 and math.isclose(t.f(CAMERA + DISTANCE), d, rel_tol=1e-5) \
                        and t.s(TARGET_Z) == from_f32(f32(BENCH[2])) \
                        and math.isclose(t.f(CAMERA + SIDE), side, abs_tol=2e-5), (name, uro, aspect)
                    assert abs(zoom_angle(t.f(CAMERA + 0x94), uro) - ORIGINAL_FOV) < 1e-3, (uro, t.f(CAMERA + 0x94))
                    sx, sy, _ = on_screen(t, uro, aspect, BENCH)
                    # across exactly (the side angle's); in height about the middle (the side angle turns the
                    # pitched view about the vertical: the middle about 2% of the height lower)
                    assert abs(sx - fx) < 1e-4 and abs(sy - 0.5) < 0.03, (name, uro, aspect, sx, sy, fx)
                    # the ends over a full spin and the tilts, through bone 1 as the middle
                    inverse_bind, pose = WEAPONS["AK-12" if name == "SVD" else name][3:5]
                    cy = (lower[1] + upper[1]) / 2
                    ends = []
                    for z_end in (-lower[2], -upper[2]):
                        v = [0.0, cy, z_end]
                        for mat in (inverse_bind, pose):
                            v = [sum(v[i] * mat[4 * i + k] for i in range(3)) + mat[12 + k] for k in range(3)]
                        ends.append(v)
                    lowest, highest = 1.0, 0.0
                    for tilt in (0.0, 1.2, -1.2):
                        for step in range(36):
                            t.lobby.word(t.state + SPIN, f32(step * math.pi / 18))
                            t.lobby.word(t.state + TILT, f32(tilt))
                            t.site(MATRIX, (DECAL_PATH, HAND, DRAW_ITEM), regs)
                            m = struct.unpack("<16f", t.emu.mem_read(FRAME - 0xE8, 0x40))
                            for v in ends:
                                world = [sum(v[i] * m[4 * i + k] for i in range(3)) + m[12 + k] for k in range(3)]
                                ex, ey, ez = on_screen(t, uro, aspect, world)
                                assert ez > 0.05 and MARGIN - 0.002 <= ey <= 1 - MARGIN + 0.002, \
                                    (name, uro, aspect, tilt, step, ey)
                                lowest, highest = min(lowest, ex), max(highest, ex)
                    # clear of the screen's left edge and of the upgrade window's left edge by the margin
                    assert lowest >= MARGIN - 0.002 and highest <= window - MARGIN + 0.002, \
                        (name, uro, aspect, lowest, highest, window)
                    t.lobby.word(t.state + SPIN, 0)
                    t.lobby.word(t.state + TILT, 0)
                    t.site(MATRIX, (DECAL_PATH, HAND, DRAW_ITEM), regs)
                    m = struct.unpack("<16f", t.emu.mem_read(FRAME - 0xE8, 0x40))
                    across = [on_screen(t, uro, aspect, [sum(v[i] * m[4 * i + k] for i in range(3)) + m[12 + k]
                                                         for k in range(3)])[0] for v in ends]
                    share = abs(across[0] - across[1]) / (window - squad)    # of the part between the Squad strip and the window
                    views.append((width, height, uro, name, d, share))
                    # the close: the player's field of view again (the zoom 1), exactly
                    t.lobby.handler(EXIT_HANDLER, 0)
                    for n in range(600):
                        t.frame()
                        if not t.flag(FLY):
                            break
                    assert not t.flag(FLY) and t.f(CAMERA + 0x94) == 1.0, (uro, t.f(CAMERA + 0x94))
        same = all(abs(a[4] - b[4]) < 1e-5 and abs(a[5] - b[5]) < 1e-3 for a in views for b in views
                   if (a[0], a[1], a[3]) == (b[0], b[1], b[3]))
        assert same, views
        shares = {(w, h, n): (d, sh) for w, h, u, n, d, sh in views if u == 65.0}
        say(f"the bench's view{rebased} at uro_fov 50, 65 and 100 and 16:9, 16:10, 21:9 and 32:9 (3840x1080, "
            "5120x1440): the zoom gives the original's 65 degrees, the weapon's middle at FRAME_SHIFT left of the "
            "screen's middle (0.310, 0.310, 0.359, 0.405 of the width) within 0.0001, about the middle of the height, "
            "the distance and side angle frame()'s for its length; over a full spin at tilts 0 and +-1.2 its ends "
            "2% clear of the screen's left edge, the upgrade window's left edge, the top and the bottom; the same "
            "whatever uro_fov, and the zoom 1 again after the close. At rest, of the part between the Squad strip "
            "and the upgrade window: " + "; ".join(
                f"{w}x{h} {n} {sh * 100:.0f}% at {d:.2f} m" for (w, h, n), (d, sh) in shares.items()))

        def draw(t, name, box=None):
            t.lobby.word(FRAME + 4, LOBBY_DRAW + delta)
            t.lobby.word(FRAME - 0xF0, CHARACTER)
            t.lobby.word(CHARACTER + 0xD2B8, PLAYER)
            t.weapon(name, box=box)
            regs = {UC_X86_REG_EAX: 0, UC_X86_REG_EBX: PLAYER, UC_X86_REG_ECX: 0, UC_X86_REG_EDX: 0,
                    UC_X86_REG_ESI: 0, UC_X86_REG_EDI: 0, UC_X86_REG_EBP: FRAME}
            return t.site(MATRIX, (DECAL_PATH, HAND, DRAW_ITEM), regs)[0]

        # The wheel at the bench (round 5e, the owner: "when scrolling the wheel to move the camera closer, it
        # is not focused on the center of the weapon"): the camera looks at the weapon's middle and its side
        # angle places it, so the wheel moves the eye along the line through the middle: the middle stays at
        # the same point of the screen, in and out; the distance between NEAR_MIN (0.3) and FAR_SHARE (1.25)
        # of the framed one; the close glides back from wherever the wheel left it.
        wheel = Table(patched, delta)
        wheel.set_f(CAMERA + YAW, 0.4)
        wheel.set_f(CAMERA + PITCH, 0.1)
        wheel.lobby.handler(START_HANDLER, AK)
        wheel.lobby.word(CHARACTER + 0xD2C4, -1)
        draw(wheel, "AK-12")
        for n in range(300):
            wheel.frame()
        framed = wheel.f(CAMERA + DISTANCE)
        length = WEAPONS["AK-12"][2][2] - WEAPONS["AK-12"][1][2]
        assert math.isclose(framed, frame(length)[0], rel_tol=1e-5), framed
        at_rest = on_screen(wheel, 65.0, 16 / 9, BENCH)[:2]
        spots, nearest, farthest = [], framed, framed
        for move, ticks in ((-0.2, 6), (0.0, 60), (0.3, 12), (0.0, 80), (-0.15, 3), (0.0, 60)):
            for n in range(ticks):
                wheel.set_f(CAMERA + WHEEL, move)
                wheel.frame()
                spots.append(on_screen(wheel, 65.0, 16 / 9, BENCH)[:2])
                nearest = min(nearest, wheel.f(CAMERA + DISTANCE))
                farthest = max(farthest, wheel.f(CAMERA + DISTANCE))
        wheel.set_f(CAMERA + WHEEL, 0.0)
        drift = max(max(abs(a - b) for a, b in zip(spot, at_rest)) for spot in spots)
        assert drift < 1e-4, drift
        assert math.isclose(nearest, NEAR_VIEW, abs_tol=1e-6) \
            and math.isclose(farthest, max(1.25 * framed, framed + 0.5), rel_tol=1e-5), \
            (nearest, farthest, framed)
        wheel.lobby.handler(EXIT_HANDLER, 0)
        for n in range(600):
            wheel.frame()
            if not wheel.flag(FLY):
                break
        assert not wheel.flag(FLY) and wheel.f(CAMERA + 0x94) == 1.0 \
            and (wheel.f(CAMERA + POINT_X), wheel.f(CAMERA + POINT_Z)) == (0.0, 0.0) \
            and (wheel.f(CAMERA + YAW), wheel.f(CAMERA + PITCH)) == (from_f32(f32(0.4)), from_f32(f32(0.1)))
        say(f"the wheel at the bench{rebased}: in to the least {nearest:.2f} m, out to {farthest:.2f} m (the framed "
            f"{framed:.2f} m and 0.5 m, or 1.25 of it if more) and in again, the weapon's middle stays where it is on the screen (at most "
            f"{drift:.1e} of the width or height); the close glides back from there to the stock camera, the "
            "zoom 1 and the yaw and pitch it had")

        # The framing's distance changes glide (review of 5e): with the weapon drawn (not READY poked), every
        # frame's distance step is the glide's (RATE dt of the way left, FLOOR dt more) or the wheel's, never a
        # snap: a pistol and a rifle drawn early in the flight and after it settled, a pistol's window over a
        # rifle's, and the wheel scrolled out before the weapon is framed; then the wheel's range.
        def run(steps):
            """[(distance, its target)] each frame; steps: ("start", item), ("draw", name, box, building),
            ("frames", n, wheel)."""
            t = Table(patched, delta)
            t.set_f(CAMERA + YAW, 0.4)
            out, drawn = [], None
            for step in steps:
                if step[0] == "start":
                    t.lobby.handler(START_HANDLER, step[1])
                    t.lobby.word(CHARACTER + 0xD2C4, 7 if step[2] else -1)
                elif step[0] == "draw":
                    drawn = step[1:3]
                    t.lobby.word(CHARACTER + 0xD2C4, -1)
                else:
                    for n in range(step[1]):
                        if drawn:
                            draw(t, *drawn)
                        t.set_f(CAMERA + WHEEL, step[2])
                        t.frame()
                        out.append((t.f(CAMERA + DISTANCE), t.f(CAMERA + DISTANCE_TARGET)))
            t.set_f(CAMERA + WHEEL, 0.0)
            return t, out

        def glided(out, wheel=0.0):
            """each step at most the glide's from the way left before or after it (or the wheel's)"""
            worst = 0.0
            for (d0, t0), (d1, t1) in zip(out, out[1:]):
                way = max(abs(d0 - t0), abs(d0 - t1))
                step = abs(d1 - d0)
                if step > 0.08 * way + 0.0002 + 1e-6 + abs(wheel):
                    return False, step
                worst = max(worst, step)
            return True, worst

        long = ((-0.03, -0.15, -0.937), (0.03, 0.11, 0.284))
        cases = (("a pistol drawn at the third frame", [("start", AK, False), ("frames", 3, 0.0),
                                                        ("draw", "PM", None), ("frames", 300, 0.0)], 0.0),
                 ("a rifle drawn at the third frame", [("start", AK, False), ("frames", 3, 0.0),
                                                       ("draw", "AK-12", long), ("frames", 300, 0.0)], 0.0),
                 ("a pistol drawn after the flight settled", [("start", AK, False), ("frames", 300, 0.0),
                                                              ("draw", "PM", None), ("frames", 300, 0.0)], 0.0),
                 ("a pistol's window over a rifle's", [("start", AK, False), ("draw", "AK-12", long),
                                                       ("frames", 300, 0.0), ("start", lobby_check.PISTOL2, True),
                                                       ("frames", 20, 0.0), ("draw", "PM", None),
                                                       ("frames", 300, 0.0)], 0.0),
                 ("the wheel out before the framing", [("start", AK, False), ("frames", 20, 0.4),
                                                       ("frames", 2, 0.0), ("draw", "PM", None),
                                                       ("frames", 300, 0.0)], 0.4))
        steps = []
        for name, script, wheel_move in cases:
            t, out = run(script)
            ok, worst = glided(out, wheel_move)
            assert ok, (name, worst)
            steps.append(f"{name} {worst * 100:.1f} cm")
        pistol, out = run([("start", AK, False), ("draw", "PM", None), ("frames", 300, 0.0)])
        framed = pistol.f(CAMERA + DISTANCE)
        for move, ticks in ((0.4, 20), (0.0, 60)):
            for n in range(ticks):
                pistol.set_f(CAMERA + WHEEL, move)
                pistol.frame()
        assert math.isclose(framed, NEAR_VIEW, abs_tol=1e-6) \
            and math.isclose(pistol.f(CAMERA + DISTANCE), framed + 0.5, rel_tol=1e-5), pistol.f(CAMERA + DISTANCE)
        say(f"the framing glides{rebased} (each frame's step at most the glide's: the largest " + ", ".join(steps) +
            f"); a pistol framed at {framed:.2f} m can be wheeled out to {pistol.f(CAMERA + DISTANCE):.2f} m")

        # A weapon's window over a weapon's (review of 5d): the bench keeps the first weapon (READY) and draws
        # it while the character makes the new player; that old weapon is not measured for the new view:
        # the SVD's window, then a pistol's over it, the SVD drawn through the rebuild, then the pistol.
        over = Table(patched, delta)
        over.lobby.handler(START_HANDLER, SVD)
        over.lobby.word(CHARACTER + 0xD2C4, -1)
        draws = [draw(over, "AK-12", SVD_BOX)]
        for n in range(200):
            over.frame()
        rifle = over.f(CAMERA + DISTANCE)
        over.lobby.handler(START_HANDLER, lobby_check.PISTOL2)          # setup_profile: the new player's query
        over.lobby.word(CHARACTER + 0xD2C4, 7)
        for n in range(20):                                 # the rebuild: the SVD still at the bench
            draws.append(draw(over, "AK-12", SVD_BOX))
            over.frame()
        waiting = over.flag(LENGTH)
        over.lobby.word(CHARACTER + 0xD2C4, -1)             # the pistol's player
        draws.append(draw(over, "PM"))
        for n in range(300):
            over.frame()
        pistol_length = WEAPONS["PM"][2][2] - WEAPONS["PM"][1][2]
        assert draws == [DRAW_ITEM] * len(draws) and waiting == 0, (draws, waiting)
        assert math.isclose(rifle, frame(1.221)[0], rel_tol=1e-4), rifle
        assert math.isclose(over.f(CAMERA + DISTANCE), frame(pistol_length)[0], rel_tol=1e-4), over.f(CAMERA + DISTANCE)
        say(f"a pistol's window over the SVD's{rebased}, the SVD drawn at the bench through the rebuild: not "
            f"measured then (the length waits); the pistol, once its player comes, is framed for itself: "
            f"{rifle:.2f} m, then {over.f(CAMERA + DISTANCE):.2f} m")

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
            "bench its weapon (READY); a vest's "
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
            assert end == DECAL_PATH and out == regs and table.flag(READY) == 0, (building, hex(end))
            # the player made (+D2C4h -1, +D2C8h 0): the first draw is at the bench
            table.lobby.word(CHARACTER + 0xD2C4, -1)
            table.emu.mem_write(CHARACTER + 0xD2C8, b"\x00")
            end, out = table.site(MATRIX, (DECAL_PATH, HAND, DRAW_ITEM), regs)
            m = struct.unpack("<16f", table.emu.mem_read(FRAME - 0xE8, 0x40))
            middle = table.weapon("AK-12")
            at = [sum(middle[i] * m[4 * i + k] for i in range(3)) + m[12 + k] for k in range(3)]
            assert end == DRAW_ITEM and table.flag(READY) == 1 and all(abs(a - b) < 1e-5 for a, b in zip(at, BENCH))

            def shown(t):
                t.lobby.word(t.state + READY, 1)
                return t.weapon("AK-12")
            table, end, out, m, middle, regs = bench_case(shown, building)
            assert end == DRAW_ITEM and out[UC_X86_REG_EAX] == FRAME - 0xE8 and table.flag(READY) == 1
            at = [sum(middle[i] * m[4 * i + k] for i in range(3)) + m[12 + k] for k in range(3)]
            assert all(abs(a - b) < 1e-5 for a, b in zip(at, BENCH)), at
        say(f"the character still making its player{rebased} (+D2C4h a query, or +D2C8h a setup_profile "
            "waiting): before the bench showed a weapon, the held item takes the decal pose's transform (10 m "
            "under: not in the hand, not at the bench), READY 0, every register kept; the first draw once the "
            "player is made puts it at the bench, READY 1; once it did (a refresh: a scope or a camouflage put on), the current player's "
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
        lobby.word(table.state + TABLE, 1)                                     # flying there or there
        assert table.left(0x01, over=0) == 1 and table.flag(LEFT) == 1         # a press off the movie
        assert table.left(0x01, over=1) == 1 and table.box["over_calls"] == 1  # asked once, at the first move
        assert table.left(0x00) == 0 and table.flag(LEFT) == 0                 # let go: the drag ends
        assert table.left(0x01, over=1) == 0 and table.flag(LEFT) == 2         # a press on the movie: an item, a control
        assert table.left(0x01, over=0) == 0 and table.box["over_calls"] == 2  # not until it is let go
        assert table.left(0x00) == 0 and table.flag(LEFT) == 0
        assert table.left(0x02, over=1) == 1                                   # the camera's own buttons as before
        lobby.word(table.state + TABLE, 0)
        assert table.left(0x01, over=0) == 0
        say(f"the drag's buttons{rebased}: 52h as the original's, the left button not; with the table on "
            "the left button too when its press is off the movie (is_mouse_over_ui asked once, at "
            "its first move), not when it is on it (an item dragged, a control) until it is let go; every "
            "register but eax kept")

    print(f"patch 45: {checks} checks passed")


if __name__ == "__main__":
    main()
