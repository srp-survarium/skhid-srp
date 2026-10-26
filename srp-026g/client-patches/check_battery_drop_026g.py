#!/usr/bin/env python3
"""Run patch 80 (a carried battery drops at its carrier's death, from the
hand, with the hand's velocity; ../docs/notes/battery-drop.md) in the
original exe and in the built one, with Unicorn (2.1.4).

    python check_battery_drop_026g.py <original survarium.exe>

The exe is built as patch_026g.py builds it (with the late join and the
arena). The game's own code runs on made-up objects:

- victory_item_core::is_ready_to_be_deactivated 0x301270 (in the build a
  jmp to the Rust patch battery_drop's splice entry; its Original the
  trampoline, the original's 9-byte prologue then a jmp to 0x301279);
- victory_item_core_base_state::execute 0x344B30 (in the build its je at
  0x344B89 is nops) with the states' real vtables: take 0xA1BF84 and carry
  0xA1BFB8 (vt+28h is_user_dead 0x344C90), put 0xA1BFE8 (vt+28h 0x372420,
  always 1), and the physics world's velocity function (vt+5Ch, the game's
  0x75220, Bullet's calculateVelocity);
- finalize 0x344C30, set_transform 0x3009B0, deactivate 0x300F40 up to its
  logic's leave (0x300FCF; the made-up logic's leave calls the state's
  vt+0Ch, the carry state's finalize, as the game's FSM does), drop
  0x300E50 (deactivate's vt+B4h for a battery not put into a container),
  release_body 0x300DC0 (the body into the world, its impulses);
- the state's deserialize 0x344D70 (a rollback's load of the state).

Stubbed, recording: compute_carried_transform 0x344E40 (it gives the hand
pose of the simulation's "Weapon" bone, get_bone_transform 0x2DA540 on the
player's own animation player +750h; here a pose a test picks), the
battery's insert (vt+9Ch: the matrix it is put at) and its body's two
impulses (vt+24h central, vt+28h torque), the hands' expression's vt+20h
(done or not) and vt+0Ch.

Checked:
- the two sites: the original's bytes in the original and the build
  without the late join; a jmp to the entry and nops, and six nops, here;
- a living carrier (take and carry): not ready (the hands' expression not
  done), as the original, and the original's answer for every expression
  and state flag (the trampoline runs the original's prologue and body);
  nothing of the battery or the player changes; the original leaves the
  state as it was, the build tracks the hand in it (+2Ch, +6Ch, +ACh, +B8h,
  +0C4h, the bytes the world serializes) and nothing else;
- a dead carrier: ready on its first tick in the build (the original says
  no, and would wait); the drop puts the battery at the hand pose execute
  tracked on that tick, and its body's central impulse is 10 x the hand's
  linear velocity ((pose - last tick's pose) / dt), the torque impulse the
  tracked angular velocity;
- a living carrier's drop: the battery's velocities are zero (deactivate
  zeroes them, finalize gives none for a living user), so release_body's
  impulses are zero, in the original and the build alike;
- a put into a container (the put state): execute and finalize change the
  same bytes in the original and the build;
- the same tick from the same world twice, and from a world whose state
  was loaded by the game's own deserialize from the tick before's fields
  (a rollback's re-simulation): the same bytes and the same drop, bit for
  bit.
"""
import os
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_HOOK_CODE, UC_MODE_32, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_ESP

if len(sys.argv) > 1:
    os.environ.setdefault("SRP_026G_EXE", sys.argv[1])    # a span hook's bytes (patch_026g.RUST_HOOKS)
import patch_026g as base

IS_READY, EXECUTE, FINALIZE, DEACTIVATE = 0x301270, 0x344B30, 0x344C30, 0x300F40
DEACTIVATE_LEFT, DROP, RELEASE_BODY = 0x300FCF, 0x300E50, 0x300DC0
DESERIALIZE, COMPUTE, GATE, PROLOGUE = 0x344D70, 0x344E40, 0x344B89, 9
SET_TRANSFORM, VELOCITY = 0x3009B0, 0x75220
TAKE_VTABLE, CARRY_VTABLE, PUT_VTABLE = 0xA1BF84, 0xA1BFB8, 0xA1BFE8
IS_USER_DEAD, ALWAYS = 0x344C90, 0x372420

OBJECTS, STACK, STUBS, SENTINEL = 0x20000000, 0x21000000, 0x22000000, 0x22FF0000
BATTERY, PLAYER, STATE, EXPRESSION, LOGIC, BODY, WORLD = (OBJECTS + 0x1000 * i for i in range(7))
BVT, EVT_DONE, EVT_BUSY, LVT, BODY_VT, WORLD_VT = (OBJECTS + 0x10000 + 0x400 * i for i in range(6))
# stubs: each a `ret`/`ret n` (a hook records what it is given) or small code
S_RET, S_RET4, S_RET0C = STUBS, STUBS + 0x10, STUBS + 0x20
S_INSERT, S_IMPULSE, S_TORQUE, S_COMPUTE = STUBS + 0x30, STUBS + 0x40, STUBS + 0x50, STUBS + 0x60
S_DONE, S_BUSY, S_LEAVE = STUBS + 0x70, STUBS + 0x80, STUBS + 0x90
STATE_SIZE = 0xD0
TRACKED = (0x2C, 0xC8)                     # +2Ch..+0C7h: the two matrices, the velocities, the time
T0, T1 = 1000, 1050                        # ms: the state's last tick, the user's now (dt 0.05 s)


def f32s(data, n):
    return list(struct.unpack(f"<{n}f", data))


def matrix(x, y, z, turn=0.0):
    """A row-vector 4x4 (vostok's): a turn about the up axis, then (x, y, z)."""
    import math
    c, s = math.cos(turn), math.sin(turn)
    return struct.pack("<16f", c, 0, -s, 0, 0, 1, 0, 0, s, 0, c, 0, x, y, z, 1)


M0, N0 = matrix(10.0, 1.2, 20.0), matrix(0.3, 1.2, 0.4)          # the hand at the last tick
M1, N1 = matrix(10.3, 1.1, 20.2, 0.1), matrix(0.35, 1.15, 0.45, 0.1)   # and now


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at in (OBJECTS, STACK, STUBS):
            emu.mem_map(at, 0x100000, UC_PROT_ALL)
        emu.mem_map(SENTINEL, 0x1000, UC_PROT_ALL)
        self.image = bytes(data)
        self.data = data
        self.calls = []
        self.executed = set()
        emu.hook_add(UC_HOOK_CODE, self._stub, begin=STUBS, end=STUBS + 0xFF)
        emu.hook_add(UC_HOOK_CODE, lambda uc, at, size, _: self.executed.add(at), begin=0x301270, end=0x3012A0)
        # the stubs
        for at, code in ((S_RET, b"\xC3"), (S_RET4, b"\xC2\x04\x00"), (S_RET0C, b"\xC2\x0C\x00"),
                         (S_INSERT, b"\xC2\x04\x00"), (S_IMPULSE, b"\xC2\x04\x00"), (S_TORQUE, b"\xC2\x04\x00"),
                         (S_COMPUTE, b"\xC2\x0C\x00"), (S_DONE, b"\xB0\x01\xC3"), (S_BUSY, b"\xB0\x00\xC3"),
                         # the logic's leave (thiscall, 1 argument): the state's vt+0Ch, as the FSM does
                         (S_LEAVE, b"\xB9" + struct.pack("<I", STATE) + b"\x8B\x01\xFF\x50\x0C\xC2\x04\x00")):
            emu.mem_write(at, code)
        # compute_carried_transform: the stub (the test's hand pose)
        emu.mem_write(COMPUTE, b"\xE9" + struct.pack("<i", S_COMPUTE - (COMPUTE + 5)))
        self.hand = (M1, N1)

    def _stub(self, uc, at, size, _):
        sp = uc.reg_read(UC_X86_REG_ESP)
        arg = lambda i: self.u32(sp + 4 + 4 * i)
        if at == S_COMPUTE:
            uc.mem_write(arg(1), self.hand[0])
            uc.mem_write(arg(2), self.hand[1])
            self.calls.append(("compute", arg(0)))
        elif at == S_INSERT:
            self.calls.append(("insert", bytes(uc.mem_read(arg(0), 64))))
        elif at == S_IMPULSE:
            self.calls.append(("impulse", bytes(uc.mem_read(arg(0), 12))))
        elif at == S_TORQUE:
            self.calls.append(("torque", bytes(uc.mem_read(arg(0), 12))))
        elif at in (S_RET, S_RET4):
            self.calls.append(("ret", uc.reg_read(UC_X86_REG_ECX)))

    def u32(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def table(self, at, slots):
        self.emu.mem_write(at, struct.pack("<256I", *[S_RET] * 256))
        for offset, target in slots.items():
            self.word(at + offset, target)

    def world(self, alive, vtable, done=False, flag=False):
        """A battery carried by a player (alive or dead) in a state of
        `vtable`, its hand tracked last at T0 (M0, N0)."""
        emu = self.emu
        emu.mem_write(OBJECTS, bytes(0x20000))
        self.calls = []
        self.table(BVT, {0x9C: S_INSERT, 0xA4: SET_TRANSFORM, 0xAC: S_RET, 0xB0: S_RET, 0xB4: DROP, 0xB8: S_RET4})
        self.table(EVT_DONE, {0x0C: S_RET, 0x20: S_DONE})
        self.table(EVT_BUSY, {0x0C: S_RET, 0x20: S_BUSY})
        self.table(LVT, {0x0C: S_LEAVE})
        self.table(BODY_VT, {0x24: S_IMPULSE, 0x28: S_TORQUE})
        self.table(WORLD_VT, {0x5C: VELOCITY})
        self.word(BATTERY, BVT)
        self.word(BATTERY + 8, PLAYER)
        self.word(BATTERY + 0x178, EXPRESSION)
        self.word(BATTERY + 0x1AC, LOGIC)
        self.word(BATTERY + 0x1B0, WORLD)
        self.word(BATTERY + 0x2A8, BODY)
        self.word(EXPRESSION, EVT_DONE if done else EVT_BUSY)
        self.word(LOGIC, LVT)
        self.word(LOGIC + 0x20, STATE)
        self.word(BODY, BODY_VT)
        self.word(WORLD, WORLD_VT)
        self.word(PLAYER + 0x324, T1)
        emu.mem_write(PLAYER + 0x33C, bytes([1 if alive else 0]))
        self.word(STATE, vtable)
        self.word(STATE + 0x1C, BATTERY)
        emu.mem_write(STATE + 0x2B, bytes([1 if flag else 0]))
        emu.mem_write(STATE + 0x2C, M0)
        emu.mem_write(STATE + 0x6C, N0)
        self.word(STATE + 0xC4, T0)

    def call(self, start, ecx, args=(), until=SENTINEL):
        sp = STACK + 0x80000
        for i, value in enumerate((SENTINEL, *args)):
            self.word(sp + 4 * i, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.reg_write(UC_X86_REG_ECX, ecx)
        self.emu.reg_write(UC_X86_REG_EAX, 0x77777777)
        self.emu.emu_start(start, until, count=2000000)
        return self.emu.reg_read(UC_X86_REG_EAX)

    def snapshot(self):
        return {name: bytes(self.emu.mem_read(at, 0x400)) for name, at in
                (("battery", BATTERY), ("player", PLAYER), ("state", STATE))}

    def tick(self):
        """execute (the hand tracked), then the item switch's question."""
        self.call(EXECUTE, STATE)
        return self.call(IS_READY, BATTERY) & 0xFF

    def drop(self, insert=True):
        """deactivate(1) to its logic's leave (the state's finalize), then the
        drop (deactivate's vt+B4h): what the item switch does once ready. A
        living user's drop defers release_body (0x303350, 0x2D9570): run to
        its decision only (`insert` False)."""
        self.call(DEACTIVATE, BATTERY, (1,), until=DEACTIVATE_LEFT)
        velocities = bytes(self.emu.mem_read(BATTERY + 0x290, 24))
        if insert:
            self.call(DROP, BATTERY)
        return velocities


failed = 0


def check(ok, what):
    global failed
    failed += not ok
    print(f"  {'ok' if ok else 'FAIL'}  {what}")


def changed(a, b):
    return [i for i in range(len(a)) if a[i] != b[i]]


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    built, _ = base.build(original, "section", code, None, late_join)
    before, _ = base.build(original, "section", code, None, None)
    stock, patched = Run(original), Run(bytes(built))

    # the sites
    hook = next(h for h in base.rust_hooks(original) if h[2] == "battery_drop_on_is_ready")
    write = next(w for w in base.rust_writes(original) if w[0] == GATE)
    read = lambda data, va, n: bytes(data[base.file_offset(va):base.file_offset(va) + n])
    check(hook[0] == IS_READY and hook[1] == bytes.fromhex("568bf18b8e78010000") == read(original, IS_READY, 9)
          == read(before, IS_READY, 9) and read(built, IS_READY, 1) == b"\xE9"
          and read(built, IS_READY + 5, 4) == b"\x90" * 4,
          "0x301270: `push esi / mov esi,ecx / mov ecx,[esi+178h]` in the original and the build without the late "
          "join, a jmp to battery_drop_on_is_ready and nops here")
    check(write[1] == bytes.fromhex("0f8494000000") == read(original, GATE, 6) == read(before, GATE, 6)
          and write[2] == read(built, GATE, 6) == b"\x90" * 6,
          "0x344B89: `je 0x344C23` in the original and the build without the late join, six nops here")

    # a living carrier: the original's answer for every expression and flag
    for vtable, name in ((CARRY_VTABLE, "carry"), (TAKE_VTABLE, "take")):
        for done in (False, True):
            for flag in (False, True):
                answers, tracked = [], []
                for run in (stock, patched):
                    run.world(True, vtable, done, flag)
                    start = run.snapshot()
                    run.executed.clear()
                    answers.append(run.tick())
                    end = run.snapshot()
                    tracked.append((changed(start["state"], end["state"]),
                                    start["battery"] == end["battery"] and start["player"] == end["player"]))
                ok = (answers[0] == answers[1] == (1 if done and flag else 0) and not tracked[0][0]
                      and tracked[0][1] and tracked[1][1] and tracked[1][0]
                      and all(TRACKED[0] <= i < TRACKED[1] for i in tracked[1][0])
                      and {0x301279, 0x301280} <= patched.executed and (0x301284 in patched.executed) == done)
                check(ok, f"a living carrier ({name} state, the hands' expression {'done' if done else 'not done'}, "
                          f"the state's flag {int(flag)}): ready {answers[1]} as the original ({answers[0]}), the "
                          "trampoline's original prologue and body run; the battery and the player unchanged; the "
                          "original leaves the state as it was, the build tracks the hand in +2Ch..+0C7h alone")

    # a dead carrier: ready on its first tick; dropped at the hand, with its velocity
    results = []
    for run in (stock, patched):
        run.world(False, CARRY_VTABLE)
        ready = run.tick()
        tracked = bytes(run.emu.mem_read(STATE + 0x2C, TRACKED[1] - TRACKED[0]))
        velocities = run.drop()
        results.append((ready, tracked, velocities, list(run.calls)))
    (ready0, _, _, _), (ready1, tracked, velocities, calls) = results
    lin = f32s(tracked[0xAC - 0x2C:0xB8 - 0x2C], 3)
    ang = f32s(tracked[0xB8 - 0x2C:0xC4 - 0x2C], 3)
    want = [(a - b) / 0.05 for a, b in zip(f32s(M1, 16)[12:15], f32s(M0, 16)[12:15])]
    inserts = [c[1] for c in calls if c[0] == "insert"]
    impulses = [f32s(c[1], 3) for c in calls if c[0] == "impulse"]
    torques = [f32s(c[1], 3) for c in calls if c[0] == "torque"]
    check(ready0 == 0 and ready1 == 1,
          f"a dead carrier, the hands' expression not done: ready at once in the build ({ready1}); the original says "
          f"{ready0} (it waits for the death's expression)")
    check(tracked[:64] == M1 and tracked[0x6C - 0x2C:0xAC - 0x2C] == N1
          and all(abs(a - b) < 1e-3 for a, b in zip(lin, want))
          and struct.unpack("<I", tracked[0xC4 - 0x2C:0xC8 - 0x2C])[0] == T1,
          f"its tick tracks the hand: the pose, the linear velocity {[round(v, 3) for v in lin]} m/s "
          f"((pose - last tick's) / 0.05 s: {[round(v, 3) for v in want]}), the time")
    check(velocities[:12] == tracked[0xAC - 0x2C:0xB8 - 0x2C] and velocities[12:24] == tracked[0xB8 - 0x2C:0xC4 - 0x2C],
          "deactivate zeroes the battery's velocities, then the state's finalize gives it the hand's (+290h, +29Ch)")
    check(inserts == [M1], "the drop puts the battery at the tracked hand pose (its insert's matrix, translate(+284h) "
                           "x +204h, +204h the hand's)")
    check(len(impulses) == 1 and all(abs(i - 10.0 * v) < 1e-4 for i, v in zip(impulses[0], lin))
          and torques == [ang],
          f"release_body's central impulse {[round(v, 3) for v in impulses[0]] if impulses else None} is 10 x the "
          f"hand's velocity, its torque impulse the tracked angular velocity {[round(v, 3) for v in ang]}")

    # a living carrier's drop: zero velocity, as the original's
    zero = []
    for run in (stock, patched):
        run.world(True, CARRY_VTABLE, True, True)
        run.tick()
        velocities = run.drop(insert=False)
        run.calls = []
        run.call(RELEASE_BODY, BATTERY)        # what drop defers for a living user, later
        zero.append((velocities, [f32s(c[1], 3) for c in run.calls if c[0] in ("impulse", "torque")]))
    check(zero[0] == zero[1] and zero[1][0] == bytes(24) and zero[1][1] == [[0.0] * 3, [0.0] * 3],
          "a living carrier's drop: the battery's velocities zero after deactivate, release_body's impulses zero, "
          "in the original and the build alike")

    # a put into a container: the put state, the same in both
    puts = []
    for run in (stock, patched):
        run.world(True, PUT_VTABLE)
        run.call(EXECUTE, STATE)
        run.call(FINALIZE, STATE)
        puts.append(run.snapshot())
    check(puts[0] == puts[1] and puts[1]["battery"][0x290:0x29C] == puts[1]["state"][0xAC:0xB8],
          "a put into a container (the put state, vt+28h always 1): execute and finalize write the same bytes in "
          "the original and the build (the put state tracked the hand before; unchanged)")

    # determinism: the same tick twice; and after the game's own deserialize
    runs = []
    for _ in range(2):
        patched.world(False, CARRY_VTABLE)
        ready = patched.tick()
        state = bytes(patched.emu.mem_read(STATE, STATE_SIZE))
        velocities = patched.drop()
        runs.append((ready, state, velocities, list(patched.calls), patched.snapshot()))
    check(runs[0] == runs[1], "the same tick from the same world twice: the same state, the same drop, bit for bit")
    # a rollback: the state's fields as serialize (0x344CB0) writes them,
    # loaded by deserialize 0x344D70 into a state with nothing tracked
    patched.world(False, CARRY_VTABLE)
    s = runs[0][1]
    stream = (s[0x28:0x29] + s[0xB8:0xC4] + s[0xAC:0xB8] + s[0x2C:0x6C] + s[0x6C:0xAC] + s[0xC4:0xC8])
    patched.emu.mem_write(STATE + 0x28, bytes(0xA0))
    reader = OBJECTS + 0x18000
    patched.emu.mem_write(reader + 0x100, stream)
    patched.word(reader, 0)
    patched.word(reader + 4, reader + 0x100)
    patched.call(DESERIALIZE, STATE, (reader, 0, 0))
    loaded = bytes(patched.emu.mem_read(STATE, STATE_SIZE))
    check(loaded[0x28:0xC8] == s[0x28:0xC8] and patched.u32(reader + 4) == reader + 0x100 + len(stream),
          f"deserialize loads the tracked hand ({len(stream)} bytes: +28h, +B8h, +ACh, +2Ch, +6Ch, +0C4h) as the "
          "tick left it")
    # the tick after it, from the loaded state and from the original one
    after = []
    for load in (False, True):
        patched.world(False, CARRY_VTABLE)
        patched.emu.mem_write(STATE + 0x28, (loaded if load else s)[0x28:0xC8])
        patched.word(PLAYER + 0x324, T1 + 50)
        patched.hand = (matrix(10.5, 0.9, 20.3, 0.2), matrix(0.4, 1.0, 0.5, 0.2))
        ready = patched.tick()
        after.append((ready, bytes(patched.emu.mem_read(STATE, STATE_SIZE)), patched.drop(), list(patched.calls)))
    patched.hand = (M1, N1)
    check(after[0] == after[1], "a rollback: the next tick from the state the game loaded is the one from the state "
                                "it saved, bit for bit (the same pose, velocity, drop and impulses)")

    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
