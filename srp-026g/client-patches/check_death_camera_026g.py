#!/usr/bin/env python3
"""Run the original's and patch 46's mouse moves through the game world's
input handler (0x3B9F0A, 0x25D0E0) and update_camera's call of the view's
update (0x1F3649..0x1F3650, 0x2422B0) on made-up objects, with Unicorn.

    python check_death_camera_026g.py <original survarium.exe>

The moves run as check_kit_change_026g.py runs them (its Look: the input
world's loop, the game world's on_mouse_move with the build's death_look,
patch 31's warmup_look and the original; stubs for the hud's pointer, the
system's cursor and the player's input handler), with a game (+53ACh its
network client, whose +4 is the local player; +5348h the render window,
whose vtable+1Ch gives 1920 x 1080). The view's update runs as machine
code (0x2422B0 with the game's own rotations and products) on a made-up
player, input handler and eye transform. The rules:
../docs/notes/death-camera.md.

The build, with the kits shown, the player's input handler in the first
person mode and the local player dead (the death screen): a move without a
button and the wheel without a button are the cursor's, as in the
original; a left press that moves little is the cursor's (a click); past
that, or with the lobby's buttons at once, a drag takes the hud's pointer
off the screen, gives the player's handler nothing, and turns a yaw and a
pitch by the player's look's step (the mouse's sensitivity, the window's
height over its width, an inverted mouse), the pitch within 80 degrees; the
wheel in a drag changes nothing; the first move after it puts the cursor
back where the drag began. A live player, a dead one in another mode (the
warmup's before the start: patch 31's drag; a spectator's), the kits
hidden, no local player: the original's (or patch 31's). The view: while
the player is dead, in the first person mode, the kits shown and the turn
not 0, the eyes' transform turned as the update's own third-person step
turns it with the distance 0 (bit for bit), the handler's warmup yaw,
pitch and distance as they were; a live player gets the original's view
and the turn is cleared; any other case the original's view. The original
does none of it. Says nothing about what the real hud, the render window
and the animation of a corpse do.
"""
import math
import struct
import sys

from unicorn import UC_PROT_ALL, UcError
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EIP, UC_X86_REG_ESP

import check_kit_change_026g as kc
import patch_026g as base

LEFT, RIGHT = kc.LEFT, kc.RIGHT
GAME, CLIENT, LOCAL = kc.CLIENTS + 0x100000, kc.CLIENTS + 0x200000, kc.CLIENTS + 0x300000
RENDER, RVT = kc.CLIENTS + 0x380000, kc.CLIENTS + 0x381000
WINDOW_SIZE = kc.STUB + 0x200
SENSITIVITY, INVERT = 0xA74AEC, 0x1F1D9D3
W, H = 1920.0, 1080.0
VERBOSE = "-v" in sys.argv


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def bits(value):
    return struct.unpack("<I", struct.pack("<f", value))[0]


def state_va(data):
    late_join = open(base.late_join_bin(), "rb").read()
    return base.late_join_state_va(late_join) + base.DEATH_CAMERA_STATE_OFFSET


def map_state(emu, state):
    """The state page, zeroed, also in the original (which has none; it
    never reads it), so that both runs read the turn the same way."""
    if not any(begin <= state <= end for begin, end, _ in emu.mem_regions()):
        emu.mem_map(state & ~0xFFF, 0x1000, UC_PROT_ALL)


class Look(kc.Look):
    """kc.Look with a game, its network client and local player."""

    def __init__(self, data, kits=1, mode=0, cursor=1, alive=0, local=True, invert=0, sensitivity=1.0):
        super().__init__(data, kits, mode, cursor)
        self.state = state_va(data)
        map_state(self.emu, self.state)
        self.word(kc.LOOK_WORLD + 0x9C, GAME)
        self.word(GAME + 0x53AC, CLIENT)
        self.word(CLIENT + 4, LOCAL if local else 0)
        self.byte(LOCAL + 0x33C, alive)
        self.word(GAME + 0x5348, RENDER)
        self.word(RENDER, RVT)
        self.word(RVT + 0x1C, WINDOW_SIZE)
        self.byte(INVERT, invert)
        self.emu.mem_write(SENSITIVITY, struct.pack("<f", sensitivity))

    def stub(self, emu, address, size, user):
        if address == WINDOW_SIZE:                 # thiscall (render, &out): out = (w, h); `ret 4`
            sp = emu.reg_read(UC_X86_REG_ESP)
            assert emu.reg_read(UC_X86_REG_ECX) == RENDER
            out = self.read(sp + 4)
            self.emu.mem_write(out, struct.pack("<ff", W, H))
            self.calls.append(("window size",))
            self.returns(out, 4)
        else:
            super().stub(emu, address, size, user)

    def turn(self):
        return struct.unpack("<ff", self.emu.mem_read(self.state, 8))


def expected_turn(turn, x, y, invert=0, sensitivity=1.0):
    """The build's step, as float32 at each operation."""
    yaw, pitch = turn
    k = f32(f32(sensitivity) * f32(math.pi / 180 * 0.1))
    v = f32(f32(f32(H) / f32(W)) * k)
    v = f32(v * f32(0.95492965))
    if invert:
        v = -v
    yaw = f32(yaw - f32(f32(x) * k))
    if y:
        pitch = f32(pitch - f32(f32(y) * v))
        limit = f32(math.radians(80))
        if abs(pitch) > limit or math.isnan(pitch):
            pitch = math.copysign(limit, pitch)
    return yaw, pitch


def look_cases():
    """(name, Look's keywords, [(buttons, x, y, wheel, the original's calls,
    the build's, the build turns)]): moves in one game, in order."""
    def cursor(x, y):
        return [("pointer", x, y, 0)]

    def player(x, y, z=0):
        return [("player", x, y, z)]
    away = [("pointer", -128, -128, 0)]
    size = [("window size",)]
    dead = dict(kits=1, mode=0, cursor=1, alive=0)
    yield "the death screen: moves", dead, [
        (0, 5, 0, 0, cursor(305, 200), cursor(305, 200), False),
        (0, 3, 4, 0, cursor(308, 204), cursor(308, 204), False)]
    yield "the death screen: the wheel", dead, [
        (0, 0, 0, 120, cursor(300, 200), cursor(300, 200), False),
        (0, 4, 2, -120, cursor(304, 202), cursor(304, 202), False)]
    yield "the death screen: a click on a kit", dead, [
        (LEFT, 2, 1, 0, cursor(302, 201), cursor(302, 201), False),
        (LEFT, 1, -2, 0, cursor(303, 199), cursor(303, 199), False),
        (0, 0, 1, 0, cursor(303, 200), cursor(303, 200), False)]
    yield "the death screen: a right drag", dead, [
        (RIGHT, 3, 0, 0, cursor(303, 200), away + size, True),
        (RIGHT, 1, 1, 0, cursor(304, 201), size, True),
        (RIGHT, -40, 25, 0, cursor(264, 226), size, True),
        (RIGHT, 0, 0, 120, cursor(264, 226), size, True),       # the wheel in a drag: nothing
        (0, 1, 0, 0, cursor(265, 226), [("cursor back", 300, 200)] + cursor(300, 200), False),
        (0, 2, 2, 0, cursor(267, 228), cursor(302, 202), False)]
    yield "the death screen: a left drag", dead, [
        (0, 10, 0, 0, cursor(310, 200), cursor(310, 200), False),
        (LEFT, 5, 0, 0, cursor(315, 200), cursor(315, 200), False),
        (LEFT, 6, 0, 0, cursor(321, 200), away + size, True),
        (LEFT, 10, -4, 0, cursor(331, 196), size, True),
        (0, 1, 0, 0, cursor(332, 196), [("cursor back", 315, 200)] + cursor(315, 200), False)]
    yield "the death screen: an inverted mouse", dict(dead, invert=1), [
        (RIGHT, 7, -9, 0, cursor(307, 191), away + size, True),
        (RIGHT, -2, 30, 0, cursor(305, 221), size, True)]
    yield "the death screen: a slow mouse", dict(dead, sensitivity=0.25), [
        (RIGHT, 12, 6, 0, cursor(312, 206), away + size, True)]
    yield "the death screen: the pitch's limit", dead, [
        (RIGHT, 0, -3000, 0, cursor(300, -2800), away + size, True),
        (RIGHT, 0, -500, 0, cursor(300, -3300), size, True),
        (RIGHT, 0, 9000, 0, cursor(300, 5700), size, True)]
    for name, keywords in (("a live player (the start's frame)", dict(dead, alive=1)),
                           ("a spectator (the input off)", dict(dead, mode=4)),
                           ("third person", dict(dead, mode=2)),
                           ("no local player", dict(dead, local=False))):
        yield f"{name}: a right drag and the wheel", keywords, [
            (RIGHT, 3, 0, 0, cursor(303, 200), cursor(303, 200), False),
            (RIGHT, 1, 1, 120, cursor(304, 201), cursor(304, 201), False),
            (0, 1, 0, 0, cursor(305, 201), cursor(305, 201), False)]
    yield "a dead player before the start (the warmup's mode): patch 31's drag", dict(dead, mode=3), [
        (RIGHT, 3, 0, 0, cursor(303, 200), away + player(3, 0), False),
        (RIGHT, 1, 1, 120, cursor(304, 201), player(1, 1, 120), False),
        (0, 1, 0, 0, cursor(305, 201), [("cursor back", 300, 200)] + cursor(300, 200), False),
        (0, 0, 0, 120, cursor(305, 201), player(0, 0, 120), False)]
    yield "the kits hidden, no cursor", dict(dead, kits=0, cursor=0), [
        (0, 5, 0, 0, cursor(305, 200) + player(5, 0), cursor(305, 200) + player(5, 0), False),
        (RIGHT, 5, 0, 120, cursor(310, 200) + player(5, 0, 120), cursor(310, 200) + player(5, 0, 120), False)]


def check_look(data, patched):
    checks = 0
    for name, keywords, moves in look_cases():
        look = Look(data, **keywords)
        look.word(kc.LOOK_WORLD + 0x8C, 300)
        look.word(kc.LOOK_WORLD + 0x90, 200)
        turn = (0.0, 0.0)
        for i, (buttons, x, y, z, original, build, turns) in enumerate(moves):
            calls = look.move(buttons, x, y, z)
            assert calls == (build if patched else original), (patched, name, i, calls)
            if patched and turns:
                turn = expected_turn(turn, x, y, keywords.get("invert", 0), keywords.get("sensitivity", 1.0))
            got = look.turn()
            assert [bits(v) for v in got] == [bits(v) for v in turn], (patched, name, i, got, turn)
            checks += 1
    return checks


# update_camera's call of the view's update: 0x1F3649 `push eax` (the
# transform) / `push ecx` (the handler) / `call 0x2422B0` (the build:
# death_view) to 0x1F3650; ebx the player.
VIEW_FROM, VIEW_TO = 0x1F3649, 0x1F3650
PLAYER, HANDLER, WORLD, TRANSFORM = (kc.HEAP + 0x50000 + 0x1000 * i for i in range(4))
WARMUP = (0.3, -0.2, 2.5)                          # the handler's warmup yaw, pitch and distance


def eyes():
    """A made-up eye transform: rows i, j, k (right, up, forward) of a head
    turned 40 degrees, looking 25 degrees down and rolled 10, then its place."""
    def rot(axis, a):
        c, s = math.cos(a), math.sin(a)
        x, y, z = axis
        return [[c + x * x * (1 - c), x * y * (1 - c) + z * s, x * z * (1 - c) - y * s],
                [y * x * (1 - c) - z * s, c + y * y * (1 - c), y * z * (1 - c) + x * s],
                [z * x * (1 - c) + y * s, z * y * (1 - c) - x * s, c + z * z * (1 - c)]]

    def mul(a, b):
        return [[sum(a[r][t] * b[t][c] for t in range(3)) for c in range(3)] for r in range(3)]
    m = mul(mul(rot((0, 0, 1), math.radians(10)), rot((1, 0, 0), math.radians(25))), rot((0, 1, 0), math.radians(40)))
    rows = [m[0] + [0.0], m[1] + [0.0], m[2] + [0.0], [12.5, 1.2, -7.75, 1.0]]
    return struct.pack("<16f", *[v for row in rows for v in row])


class View(kc.Emu):
    def __init__(self, data, alive=0, mode=0, kits=1, turn=(0.0, 0.0), warmup=WARMUP):
        super().__init__(data)
        self.state = state_va(data)
        map_state(self.emu, self.state)
        self.byte(PLAYER + 0x33C, alive)
        self.word(HANDLER + 0x358, mode)
        self.word(HANDLER + 0x334, WORLD)
        self.byte(WORLD + 0x4DB, kits)
        self.emu.mem_write(HANDLER + 0x34C, struct.pack("<3f", *warmup))
        self.emu.mem_write(HANDLER + 0x48, b"\xEE" * 64)
        self.emu.mem_write(TRANSFORM, eyes())
        self.emu.mem_write(self.state, struct.pack("<2f", *turn))

    def run(self):
        emu = self.emu
        emu.reg_write(UC_X86_REG_ESP, kc.STACK)
        emu.reg_write(UC_X86_REG_EAX, TRANSFORM)
        emu.reg_write(UC_X86_REG_ECX, HANDLER)
        emu.reg_write(UC_X86_REG_EBX, PLAYER)
        emu.emu_start(VIEW_FROM, VIEW_TO, count=20000)
        assert emu.reg_read(UC_X86_REG_EIP) == VIEW_TO and emu.reg_read(UC_X86_REG_ESP) == kc.STACK
        assert emu.reg_read(UC_X86_REG_EBX) == PLAYER
        return (bytes(emu.mem_read(HANDLER + 0x48, 64)), struct.unpack("<3f", emu.mem_read(HANDLER + 0x34C, 12)),
                struct.unpack("<2f", emu.mem_read(self.state, 8)), emu.mem_read(HANDLER + 0x35C, 1)[0])


def third_person(original, yaw, pitch):
    """The original update's own third-person step (mode 2) on the eyes with
    this yaw and pitch and the distance 0: the view the build must give."""
    view = View(original, mode=2, warmup=(yaw, pitch, 0.0))
    return view.run()[0]


def view_cases(original):
    """(name, View's keywords, the view the original gives, the build's, the
    turn after it in the build)."""
    copy = eyes()
    for yaw, pitch in ((0.5, 0.0), (-1.25, 0.3), (0.0, -0.6), (3.0, 1.3962634)):
        turned = third_person(original, yaw, pitch)
        yield f"dead, the turn {yaw}, {pitch}", dict(turn=(yaw, pitch)), copy, turned, (yaw, pitch)
    yield "dead, no turn", dict(), copy, copy, (0.0, 0.0)
    yield "dead, a turn of -0.0", dict(turn=(-0.0, -0.0)), copy, copy, (-0.0, -0.0)
    yield "alive: the turn cleared", dict(alive=1, turn=(0.5, 0.2)), copy, copy, (0.0, 0.0)
    yield "dead, a spectator (mode 4)", dict(mode=4, turn=(0.5, 0.2)), copy, copy, (0.5, 0.2)
    yield "dead, the kits hidden", dict(kits=0, turn=(0.5, 0.2)), copy, copy, (0.5, 0.2)
    third = View(original, mode=2).run()[0]
    yield "dead, third person (a replay's)", dict(mode=2, turn=(0.5, 0.2)), third, third, (0.5, 0.2)


def check_view(data, patched, cases):
    checks = 0
    for name, keywords, original, build, after in cases:
        view, warmup, turn, changed = View(data, **keywords).run()
        assert view == (build if patched else original), (patched, name)
        assert [bits(v) for v in warmup] == [bits(v) for v in WARMUP], (patched, name, warmup)
        expected_after = after if patched else keywords.get("turn", (0.0, 0.0))
        assert [bits(v) for v in turn] == [bits(v) for v in expected_after], (patched, name, turn)
        assert changed == 0, (patched, name)
        checks += 1
    return checks


def check_geometry(original):
    """The build's turned view keeps the eyes' place and an orthonormal
    basis, and turns the eyes' look by the yaw about the world's up, then by
    the pitch about its own right."""
    checks = 0
    m = struct.unpack("<16f", eyes())
    for yaw, pitch in ((0.5, 0.0), (-1.25, 0.3), (0.0, -0.6)):
        v = struct.unpack("<16f", third_person(original, yaw, pitch))
        rows = [v[0:3], v[4:7], v[8:11]]
        assert all(abs(a - b) < 1e-6 for a, b in zip(v[12:15], m[12:15])), (yaw, pitch, v[12:15])
        for a in range(3):
            for b in range(3):
                dot = sum(rows[a][t] * rows[b][t] for t in range(3))
                assert abs(dot - (a == b)) < 1e-5, (yaw, pitch, a, b, dot)
        # the forward row: the eyes' turned about the world's up (y) by the yaw ...
        c, s = math.cos(yaw), math.sin(yaw)
        k = m[8:11]
        i = m[0:3]
        spun = lambda r: (r[0] * c - r[2] * s, r[1], r[0] * s + r[2] * c)
        k1, i1 = spun(k), spun(i)
        if abs(sum(x * y for x, y in zip(k1, rows[2])) - 1) > 1e-5 and pitch == 0:
            spun = lambda r: (r[0] * c + r[2] * s, r[1], -r[0] * s + r[2] * c)
            k1, i1 = spun(k), spun(i)
        # ... then about its right (i1) by the pitch: the angle between them
        assert abs(sum(x * y for x, y in zip(i1, rows[0])) - 1) < 1e-5, (yaw, pitch)
        assert abs(sum(x * y for x, y in zip(k1, rows[2])) - math.cos(pitch)) < 1e-5, (yaw, pitch)
        checks += 1
    return checks


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    patched, _ = base.build(original, "section", code, None, late_join)
    cases = list(view_cases(original))
    checks = check_geometry(original)
    for data, is_patched in ((original, False), (patched, True)):
        checks += check_look(data, is_patched) + check_view(data, is_patched, cases)
    # A build that turns a live player's camera, one that gives the drag on
    # to the player's handler, one that drags without a button, one with no
    # limit on the pitch, one that leaves a borrowed field as it set it, one
    # that never clears the turn, and one that turns a spectator's view fail.
    view_at = base.death_camera_va(late_join) + base.DEATH_VIEW_OFFSET
    for old, new, check in (("80b83c030000007405", "80b83c03000000eb05", "look"),
                            ("b0015f5e5bc21000", "30c05f5e5bc21000", "look"),
                            ("a8530f84", "a8530f85", "look"),
                            ("81fac2b8b23f760a", "81fac2b8b23feb0a", "look"),
                            ("8f804c030000", "8f8000000000", "view"),
                            ("8325" + struct.pack("<I", state_va(patched)).hex() + "00",
                             "8325" + struct.pack("<I", state_va(patched)).hex() + "ff", "view"),
                            ("83b858030000000f8581000000", "83b85803000000" + "90" * 6, "view")):
        old, new = bytes.fromhex(old), bytes.fromhex(new)
        assert len(old) == len(new)
        broken = bytearray(patched)
        _, rva, vsize, raw, _, _ = base.Headers(broken).sections(broken)[-1]
        start = raw + (view_at - base.IMAGE_BASE - rva if check == "view" else 0)
        at = broken.find(old, start, raw + vsize)
        assert at >= 0 and broken.find(old, at + 1, raw + vsize) < 0, old.hex()
        broken[at:at + len(old)] = new
        try:
            if check == "look":
                check_look(bytes(broken), True)
            else:
                check_view(bytes(broken), True, cases)
        except (AssertionError, UcError) as failure:   # a wrong result, or a run gone astray
            if VERBOSE:
                print(f"  broken {new.hex()}: {type(failure).__name__} {str(failure)[:100]}")
            checks += 1
        else:
            sys.exit(f"a broken build ({new.hex()}) passed")
    print(f"patch 46: {checks} runs as expected (the original, the build and seven broken builds)")


if __name__ == "__main__":
    main()
