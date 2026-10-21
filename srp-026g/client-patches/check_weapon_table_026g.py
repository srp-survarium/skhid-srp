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
    CHARACTER, SQUAD, MENU, PLAYER, STACK, f32, from_f32

CTOR, TICK = 0x283BC0, 0x283EF0
RAY, PROJECT = 0x2844D0, 0x21DF00              # both `ret 4`
LAST_TIME = 0x2260AEC                          # the tick's last time (ms)
TIME = 0x20500400 + 0x584C                     # [[menu+9Ch]+584Ch]: the menu's +9Ch is the lobby's OWNER
CULL, CULL_ON, DRAW = 0x247576, 0x24757C, 0x247592
MATRIX, DECAL_PATH, HAND, DRAW_ITEM = 0x24778F, 0x247798, 0x2477C2, 0x2477C8
LOBBY_DRAW = 0x284A83
FRAME = STACK - 0x2000                         # a made-up mt_draw frame (ebp)
OTHER_PLAYER = SQUAD                           # another player (64 KiB of memory, as PLAYER)

# weapon_table_026g.asm's constants and state
BENCH = (4.25, 1.30, 3.05)
VIEW_YAW, VIEW_PITCH, VIEW_DISTANCE, CENTRE_Z, TILT_MAX = -math.pi / 2, -0.25, 1.2, -0.18, 1.2
TABLE, FLY, TARGET_X, TARGET_Z, TARGET_YAW, TARGET_PITCH, SAVED_YAW, SAVED_PITCH, SPIN, TILT = range(0, 40, 4)
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
            for at in (CULL_ON, DRAW, DECAL_PATH, HAND, DRAW_ITEM):
                emu.hook_add(UC_HOOK_CODE, stop, begin=at + delta, end=at + delta)
        self.box = HOOKED[key]
        self.box["matrices"].clear()
        self.box["stops"] = set()
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
        assert (table.s(TARGET_X), table.s(TARGET_Z), table.s(TARGET_PITCH)) == \
            tuple(from_f32(f32(v)) for v in (BENCH[0], BENCH[2], VIEW_PITCH))
        assert (table.s(SAVED_YAW), table.s(SAVED_PITCH)) == (from_f32(f32(0.4)), from_f32(f32(0.1)))
        assert math.isclose(table.s(TARGET_YAW), nearest_turn(VIEW_YAW, 0.4), abs_tol=1e-5)
        assert (round(table.f(CAMERA + HEIGHT_TARGET), 6), round(table.f(CAMERA + DISTANCE_TARGET), 6),
                round(table.f(CAMERA + 0x98), 6)) == (BENCH[1], VIEW_DISTANCE, 0.37)
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
        say(f"a weapon's window{rebased}: the camera's point is at the bench {BENCH} within 1 cm after "
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
        table.lobby.handler(START_HANDLER, AK)
        assert (table.flag(TABLE), table.s(SAVED_YAW)) == (1, from_f32(f32(-2.0)))
        assert math.isclose(table.s(TARGET_YAW), nearest_turn(VIEW_YAW, table.f(CAMERA + YAW)), abs_tol=1e-5)
        table.lobby.handler(START_HANDLER, VEST)
        assert (table.flag(TABLE), table.flag(FLY), table.s(TARGET_X), table.s(TARGET_Z), table.s(TARGET_YAW)) \
            == (0, 1, 0.0, 0.0, from_f32(f32(-2.0)))
        assert round(table.f(CAMERA + HEIGHT_TARGET), 6) == 1.3       # the vest's own
        table.lobby.handler(EXIT_HANDLER, 0)
        for n in range(400):
            table.frame()
        assert not table.flag(FLY) and table.f(CAMERA + YAW) == from_f32(f32(-2.0))
        say(f"windows over windows{rebased}: the AK's over the SVD's keeps the yaw kept at the first; a vest's "
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
            # (0, 0, CENTRE_Z) of the model is at the bench; up and the length turned by the spin and the tilt.
            middle = [CENTRE_Z * m[8 + k] + m[12 + k] for k in range(3)]
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

    print(f"patch 45: {checks} checks passed")


if __name__ == "__main__":
    main()
