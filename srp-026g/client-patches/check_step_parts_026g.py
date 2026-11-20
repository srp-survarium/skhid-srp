#!/usr/bin/env python3
"""Run the original's and patch 166's bullet_character_controller::updateAction
(0x7B730) on open ground, with Unicorn (2.1.4).

    python check_step_parts_026g.py <original survarium.exe>

The exe is built as patch_026g.py builds it (with the late join); the build
without the late join keeps the original's updateAction. updateAction runs
from its entry to its `ret 8`, with the game's own player_step (0x7BDA0),
can_skeep_player_step, get_corrected_walk_vector, get_slide_acceleration and
need_slide_on_supporting_surface, on a made-up controller: standing, on
level ground (its supporting normal the up axis), no velocity, no push. The
three sweeps of a step are stubs for open, level ground: step_up (0x7C900)
and step_down (0x7E1B0) leave the step's position where it is, step_forward
(0x7D590) adds the step's move to it. So the check covers what decides how
far a body goes in a step: updateAction's gate, player_step's own
arithmetic (the move from the walk asked for, its share dt / 0.05 of the
tick's step) and the body with nothing to do. Stairs, walls, slopes, the
jump and the landing are the sweeps' and are not run here.

Before each step the walk asked for is set as the game sets it
(base_player::set_physics_controller_walk_vector 0x2D9060: a displacement
for the step's own time, at most the player's speed x that time), then
updateAction zeroes it, as in the game. The rule: ../docs/notes/fall_damage.md
"A part of a tick moves a body at rest".

- whole ticks: the build's positions are the original's, bit for bit, from
  rest, walking, sprinting, slow (under 3 m/s: player_step's longer probe)
  and strafing;
- ticks cut in parts (one precise input, a burst's cuts as recorded, 50 a
  second, 1 ms parts): the original leaves a body at rest where it is until
  a whole tick; the build moves it by each part's own displacement, and
  after every tick it is where whole ticks put it, to a float's rounding
  (under 1 mm in 5 s; 50 parts of 1 ms a tick, 5000 steps, a few mm): the
  same distance, the same speed from the first tick. Never further: a part
  asked for under 3.2 mm (player_step's own least walk: 1 or 2 ms at a slow
  1.4 m/s) is not made, in the build as in the original, so cutting ticks
  can only lose distance;
- a body already moving: the build's positions are the original's bit for
  bit, however the ticks are cut, unless a part is too short to walk (the
  original then stops until a whole tick, the build goes on);
- a body at rest that is asked nothing stays where it is in both, its
  memory but m_last_pos and the step's working position untouched; a body
  that stops is not moved again; a body at rest asked to crouch or to
  stand up changes its capsule in the part it is asked in (the original:
  at the next whole tick), and stays where it is;
- the stack and the callee-saved registers as updateAction leaves them.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_HOOK_CODE, UC_MODE_32, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

UPDATE_ACTION, STEP_UP, STEP_FORWARD, STEP_DOWN, SHAPE_DIM = 0x7B730, 0x7C900, 0x7D590, 0x7E1B0, 0x7F100
SETUP_CROUCH = 0x7F1A0                        # setup_crouch_state: EDI the controller, [esp+4] crouched; `ret 4`
SLIDE_COS, WALK_SLIDE_COS = 0x21E51D8, 0x21E51DC     # statics the game sets at its start (0x84D78F, 0x84D80F)
CONTROLLER, STACK, RET = 0x41000000, 0x41128000, 0x41130000
WALK, AIR, STEP_POSITION, LAST_POSITION, POSITION = 0x20, 0x30, 0x40, 0x50, 0xB0
AXIS, DIMS, FLAGS, TICK, GRAVITY, JUMPING, NORMAL, PUSH, VELOCITY = (0x1F0, 0x1D0, 0x200, 0x2C704, 0x2C708, 0x2C70C,
                                                                     0x2C710, 0x2C730, 0x2C770)
CONTROLLER_SIZE = 0x2C790                     # its allocation (0x356A66)
START = (12.5, 1.375, -40.25)


def f32(x):
    return struct.unpack("<f", struct.pack("<f", x))[0]


def ms(n):
    """The dt updateActions gets for a step of n ms (0x75640: n x 0.001f, as a float)."""
    return f32(n * f32(0.001))


class Body:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        headers = base.Headers(data)
        for _, rva, vsize, raw, rsize, _ in headers.sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(CONTROLLER, (CONTROLLER_SIZE + 0xFFF) & ~0xFFF, UC_PROT_ALL)
        for at in (STACK - 0x8000, RET):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        emu.mem_write(SHAPE_DIM, b"\xC3")
        self.floats(SLIDE_COS, 0.6427876)              # cos 50 deg
        self.floats(WALK_SLIDE_COS, 0.7660444)         # cos 40 deg
        self.floats(CONTROLLER + TICK, 0.05)
        self.floats(CONTROLLER + GRAVITY, 9.8)
        emu.mem_write(CONTROLLER + AXIS, struct.pack("<I", 1))
        self.floats(CONTROLLER + DIMS, 0.4, 0.5, 0.4, 0.0)
        self.floats(CONTROLLER + NORMAL, 0.0, 1.0, 0.0, 0.0)
        self.floats(CONTROLLER + POSITION, *START, 1.0)
        self.floats(CONTROLLER + LAST_POSITION, *START, 1.0)
        self.steps = 0                                 # player_step's sweeps run (a step made)
        emu.hook_add(UC_HOOK_CODE, self.stub, begin=STEP_UP, end=STEP_UP)
        emu.hook_add(UC_HOOK_CODE, self.stub, begin=STEP_FORWARD, end=STEP_FORWARD)
        emu.hook_add(UC_HOOK_CODE, self.stub, begin=STEP_DOWN, end=STEP_DOWN)
        emu.hook_add(UC_HOOK_CODE, self.stub, begin=SETUP_CROUCH, end=SETUP_CROUCH)
        self.crouches = []                             # setup_crouch_state's calls: crouched or not

    def floats(self, at, *values):
        self.emu.mem_write(at, struct.pack(f"<{len(values)}f", *values))

    def read(self, at, count=3):
        return struct.unpack(f"<{count}f", self.emu.mem_read(at, 4 * count))

    def stub(self, emu, address, size, _):
        """Open, level ground: only step_forward changes the step's position."""
        popped = {STEP_UP: 4, STEP_FORWARD: 4, STEP_DOWN: 0x1C, SETUP_CROUCH: 4}[address]
        if address == SETUP_CROUCH:                   # the capsule's change is Bullet's: only its flag (+203h)
            crouched = emu.mem_read(emu.reg_read(UC_X86_REG_ESP) + 4, 1)[0]
            emu.mem_write(CONTROLLER + FLAGS + 3, bytes([crouched]))
            self.crouches.append(crouched)
        if address == STEP_FORWARD:                   # esi the move, [esp+4] the controller
            move = struct.unpack("<3f", emu.mem_read(emu.reg_read(UC_X86_REG_ESI), 12))
            at = CONTROLLER + STEP_POSITION
            now = self.read(at)
            self.floats(at, *(f32(p + m) for p, m in zip(now, move)))
            self.steps += 1
        esp = emu.reg_read(UC_X86_REG_ESP)
        back = struct.unpack("<I", emu.mem_read(esp, 4))[0]
        emu.reg_write(UC_X86_REG_ESP, esp + 4 + popped)
        emu.reg_write(UC_X86_REG_EIP, back)

    @property
    def position(self):
        return self.read(CONTROLLER + POSITION)

    def step(self, dt, walk):
        """One updateAction of dt seconds, `walk` (a displacement) asked for it."""
        emu = self.emu
        self.floats(CONTROLLER + WALK, *walk, 0.0)
        esp = STACK - 12
        emu.mem_write(esp, struct.pack("<II", RET, 0) + struct.pack("<f", dt))   # the world (not read), dt
        emu.reg_write(UC_X86_REG_ESP, esp)
        emu.reg_write(UC_X86_REG_ECX, CONTROLLER)
        emu.reg_write(UC_X86_REG_EBP, 0x4444)
        emu.reg_write(UC_X86_REG_EBX, 0x1111)
        emu.reg_write(UC_X86_REG_ESI, 0x2222)
        emu.reg_write(UC_X86_REG_EDI, 0x3333)
        emu.emu_start(UPDATE_ACTION, RET, count=200000)
        assert emu.reg_read(UC_X86_REG_EIP) == RET, f"stopped at {emu.reg_read(UC_X86_REG_EIP):#x}"
        assert emu.reg_read(UC_X86_REG_ESP) == STACK, "updateAction did not pop its 8 bytes"
        assert (emu.reg_read(UC_X86_REG_EBP), emu.reg_read(UC_X86_REG_EBX), emu.reg_read(UC_X86_REG_ESI),
                emu.reg_read(UC_X86_REG_EDI)) == (0x4444, 0x1111, 0x2222, 0x3333), "callee-saved registers"
        assert self.read(CONTROLLER + WALK, 8) == (0.0,) * 8, "the movement asked for is zeroed after the step"
        return self.position


def run(data, speed, direction, cuts, ticks, asked=lambda tick: True):
    """The body's position after each of `ticks` ticks. `cuts(tick)` gives the tick's parts in ms (their
    sum 50); in the ticks `asked` says so, the walk asked for each part is speed x its time along
    `direction`, as the game asks; in the others nothing."""
    body = Body(data)
    out = []
    for tick in range(ticks):
        for part in cuts(tick):
            dt = ms(part)
            walk = tuple(f32(speed * dt * d) for d in direction) if asked(tick) else (0.0, 0.0, 0.0)
            body.step(dt, walk)
        out.append(body.position)
    return out, body


def made(speed, direction, cuts, ticks):
    """Where the steps asked for put the body after each tick: each part's displacement, but a part
    asked for less than player_step takes for a walk at all (can_skeep_player_step 0x7C7F8: a walk
    vector whose square is under 1e-5, 3.2 mm; 0x9CBA48)."""
    at, out = list(START), []
    for tick in range(ticks):
        for part in cuts(tick):
            walk = [f32(speed * ms(part) * d) for d in direction]
            if f32(sum(f32(w * w) for w in walk)) >= f32(1e-5):
                at = [a + w for a, w in zip(at, walk)]
        out.append(tuple(at))
    return out


def whole(tick):
    return [50]


def one_cut(tick):
    return [17, 33]


def fifty_a_second(tick):
    """A precise input every 20 ms: cuts at 20 and 40, then 10 and 30."""
    return [20, 20, 10] if tick % 2 == 0 else [10, 20, 20]


def a_burst(tick):
    """A burst's cuts as a recording has them: two precise inputs 7 to 9 ms apart every 34 to 44 ms."""
    marks, t = [], 5
    while t < 5000:
        marks += [t, t + 7 + t % 3]
        t += 34 + (t * 7) % 11
    inside = sorted({m - tick * 50 for m in marks if tick * 50 < m < tick * 50 + 50})
    edges = [0] + inside + [50]
    return [b - a for a, b in zip(edges, edges[1:])]


def one_ms(tick):
    return [1] * 50


def bits(positions):
    return [struct.pack("<3f", *p) for p in positions]


def furthest(a, b):
    return max(abs(x - y) for p, q in zip(a, b) for x, y in zip(p, q))


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    built, _ = base.build(original, "section", code, None, late_join)
    before, _ = base.build(original, "section", code, None, None)
    va, old, new = base.STEP_PARTS
    at = base.file_offset(va)
    assert original[at:at + len(old)] == old and before[at:at + len(old)] == old, f"{va:#x}: the original"
    assert built[at:at + len(old)] == new, f"{va:#x}: the build's bytes"
    assert ms(50) == f32(0.05) and all(sum(c(t)) == 50 for c in (one_cut, fifty_a_second, a_burst, one_ms)
                                       for t in range(100))
    assert sum(len(a_burst(t)) > 1 for t in range(100)) >= 90, "the burst cuts nearly every tick"

    ticks = 100                                        # 5 s
    gaits = (("walking", 3.74, (1.0, 0.0, 0.0)), ("sprinting", 5.28, (0.0, 0.0, -1.0)),
             ("slow (crouched, aiming)", 1.4, (0.6, 0.0, 0.8)), ("strafing", 3.2, (-0.70710678, 0.0, 0.70710678)))
    worst = 0.0
    for name, speed, direction in gaits:
        stock, _ = run(original, speed, direction, whole, ticks)
        ours, _ = run(built, speed, direction, whole, ticks)
        assert bits(stock) == bits(ours), f"{name}: whole ticks differ from the original"
        went = sum((a - b) ** 2 for a, b in zip(stock[-1], START)) ** 0.5
        assert abs(went - speed * 5.0) < 2e-3, f"{name}: {went} m in 5 s at {speed} m/s"
        first = sum((a - b) ** 2 for a, b in zip(stock[0], START)) ** 0.5
        assert abs(first - speed * 0.05) < 1e-5, f"{name}: the first tick's {first} m"
        # A float's rounding at each step: 5000 steps of 1 ms over 5 s add up to millimetres, either way.
        for label, cuts, near in (("one cut a tick", one_cut, 1e-3), ("a burst's cuts", a_burst, 1e-3),
                                  ("50 cuts a second", fifty_a_second, 1e-3), ("1 ms parts", one_ms, 5e-3)):
            # From rest: the original does not move in a cut tick; the build is where whole ticks put it.
            stuck, body = run(original, speed, direction, cuts, ticks)
            cut_ticks = [t for t in range(ticks) if len(cuts(t)) > 1]
            first_whole = next((t for t in range(ticks) if len(cuts(t)) == 1), ticks)
            assert all(stuck[t] == START for t in range(first_whole)), f"{name}, {label}: the original moved from rest"
            moved, body = run(built, speed, direction, cuts, ticks)
            off = furthest(moved, made(speed, direction, cuts, ticks))
            worst = max(worst, off)
            assert off < near, f"{name}, {label}: {off} m from where its steps put it"
            gain = (sum((a - b) ** 2 for a, b in zip(moved[-1], START)) ** 0.5) - went
            # Never ahead of whole ticks: the parts too short to count as a walk are lost, not gained.
            lost = sum(speed * part / 1000 for t in range(ticks) for part in cuts(t)
                       if f32(sum(f32(f32(speed * ms(part) * d) ** 2) for d in direction)) < f32(1e-5))
            assert -lost - near < gain < near, f"{name}, {label}: {gain} m gained in 5 s ({lost} m of short parts)"
            print(f"{name}, {label}: {len(cut_ticks)} of {ticks} ticks cut; the original at rest "
                  f"{first_whole} ticks, the build at most {off * 1000:.3f} mm from its steps' sum, "
                  f"{gain * 1000:+.3f} mm against whole ticks in 5 s ({lost * 1000:.1f} mm in parts too short to walk)")
            # Already moving (a whole tick first): the build is the original, bit for bit.
            moving = lambda t, cuts=cuts: whole(t) if t == 0 else cuts(t)
            a, _ = run(original, speed, direction, moving, ticks)
            b, _ = run(built, speed, direction, moving, ticks)
            # (The original stops for good at the first part too short to count as a walk: it "did not
            # move" then; the build goes on at the next part.)
            if lost == 0:
                assert bits(a) == bits(b), f"{name}, {label}: a moving body differs from the original"
                assert furthest(a, stock) < near
            else:
                assert furthest(b, made(speed, direction, moving, ticks)) < near
                assert sum((x - y) ** 2 for x, y in zip(a[-1], START)) <= sum((x - y) ** 2 for x, y in zip(b[-1], START))

    # Nothing asked: the body stays, in whole ticks and in parts, and no step is made for it.
    for data in (original, built):
        for cuts in (whole, a_burst, fifty_a_second):
            body = Body(data)
            image = bytes(body.emu.mem_read(CONTROLLER, CONTROLLER_SIZE))
            for tick in range(40):
                for part in cuts(tick):
                    assert body.step(ms(part), (0.0, 0.0, 0.0)) == START
            after = bytearray(body.emu.mem_read(CONTROLLER, CONTROLLER_SIZE))
            for at, size in ((STEP_POSITION, 16), (LAST_POSITION, 16)):
                after[at:at + size] = image[at:at + size]
            assert bytes(after) == image and body.steps == 0, "a body asked nothing was changed"

    # The crouch of a body at rest (+201h asked, +203h the capsule's; player_step's start): the original
    # takes it at the next whole tick, the build in the part it is asked in; standing up the same (+202h:
    # it may). The body stays where it is.
    for data, at_once in ((original, False), (built, True)):
        body = Body(data)
        body.emu.mem_write(CONTROLLER + FLAGS + 1, b"\x01")
        assert body.step(ms(17), (0.0, 0.0, 0.0)) == START and body.crouches == ([1] if at_once else [])
        assert body.step(ms(33), (0.0, 0.0, 0.0)) == START and body.crouches == ([1] if at_once else [])
        assert body.step(ms(50), (0.0, 0.0, 0.0)) == START and body.crouches == [1]
        body.emu.mem_write(CONTROLLER + FLAGS + 1, b"\x00\x01")
        assert body.step(ms(20), (0.0, 0.0, 0.0)) == START and body.crouches == ([1, 0] if at_once else [1])
        assert body.step(ms(50), (0.0, 0.0, 0.0)) == START and body.crouches == [1, 0]

    # A body that stops stays: asked to walk for 20 ticks, then nothing, under a burst's cuts.
    asked = lambda tick: tick < 20
    moved, body = run(built, 3.74, (1.0, 0.0, 0.0), a_burst, 60, asked)
    assert all(p == moved[19] for p in moved[20:]), "a body that stopped went on"
    stock, _ = run(built, 3.74, (1.0, 0.0, 0.0), whole, 60, asked)
    assert furthest(moved, stock) < 1e-3
    # And starts again inside the burst (the original waits for a whole tick).
    again = lambda tick: tick < 10 or tick >= 30
    moved, _ = run(built, 3.74, (1.0, 0.0, 0.0), a_burst, 60, again)
    stock, _ = run(built, 3.74, (1.0, 0.0, 0.0), whole, 60, again)
    stuck, _ = run(original, 3.74, (1.0, 0.0, 0.0), a_burst, 60, again)
    assert furthest(moved, stock) < 1e-3 and stuck[40] == stuck[29] and moved[40] != moved[29]
    print(f"ok: whole ticks bit for bit the original's; a body at rest moves in a cut tick and is at most "
          f"{worst * 1000:.3f} mm from its steps' sum (never ahead of whole ticks); a moving body bit for bit the original's; "
          f"a body asked nothing untouched")


if __name__ == "__main__":
    main()
