#!/usr/bin/env python3
"""Run patch 42's choice (dynamic_anomalies_026g.asm, milestone 1) on made-up
generic_anomaly_cores and compare it with the rules of
../docs/notes/dynamic_anomalies.md, written again here in Python.

    python check_dynamic_anomalies_026g.py [--nasm /path/to/nasm]

Requires NASM and Python `unicorn`. Needs no exe: the routine is not in the
build yet (milestone 2 calls it from the world's tick). A core here is the
fields the routine reads and writes, at the 0.26g offsets (load 0x33BD50):
pending +138h, its time +13Ch, energy_enabled +14Ch, energy +150h,
energy_maximum +154h, the states +180h..+184h (a state's threshold at +8).
Every other byte of a core is a pattern that must stay as it was.

Checked: no try before the start or before start + 20 s; one try in each
30 s window, at start + 30 s + 30k s + [-10, 10) s, falling in the tick that
reaches it; a try charges one idle timed core (energy = its top state's
threshold, an artefact due 24 s later) and nothing else changes; 16 active
cores stop it; a busy timed core blocks within 15 m (14.99 m blocks, 15.01 m
does not), an idle one does not; a core with none of its artefacts free is
not chosen; stock cores (no energy_maximum, energy off,
no states) are never chosen, counted or blocking; the choice is the
reference's for many seeds and tables, and every candidate gets chosen.
"""
import argparse
import os
import random
import struct
import subprocess
import sys
import tempfile

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ESP, UC_X86_REG_EBX, UC_X86_REG_ESI, \
    UC_X86_REG_EDI, UC_X86_REG_EBP

HERE = os.path.dirname(os.path.abspath(__file__))
ASM = os.path.join(HERE, "dynamic_anomalies_026g.asm")

CAVE = 0x10000000
DATA, STACK, STOP = 0x20000000, 0x21008000, 0x22000000
CORE_SIZE, STATE_SIZE = 0x200, 0x40
PENDING, PENDING_AT, ENABLED, ENERGY, MAXIMUM, BEGIN, END, THRESHOLD = \
    0x138, 0x13C, 0x14C, 0x150, 0x154, 0x180, 0x184, 0x08
ARTEFACTS, ARTEFACTS_COUNT, HOLDER, ARTEFACT_SIZE = 0x1B0, 0x1B4, 0x280, 0x300

DELAY, INTERVAL, VARIATION, MAX_ACTIVE, ARTEFACT_AFTER = 30000, 30000, 10000, 16, 24000
LCG_MUL, TRY_MIX = 0x08088405, 0x9E3779B9
TICK = 50
M = 0xFFFFFFFF


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def lcg(x):
    return (x * LCG_MUL + 1) & M


# --- the rules, as the note gives them ------------------------------------

def try_time(start, seed, k):
    x = lcg(((k * TRY_MIX) & M) ^ seed)
    return start + DELAY - VARIATION + k * INTERVAL + ((x * 2 * VARIATION) >> 32), x


def reference(cores, now, delta, start, seed):
    """The index of the core the rules charge now, or None."""
    if start == M or now < start + DELAY - VARIATION:
        return None
    k = (now - (start + DELAY - VARIATION)) // INTERVAL
    t, x = try_time(start, seed, k)
    if not (t <= now and now - t < delta):
        return None
    timed = [c.timed() for c in cores]
    if sum(1 for c, on in zip(cores, timed) if on and f32(c.energy) > f32(c.maximum)) >= MAX_ACTIVE:
        return None

    def free(i):
        c = cores[i]
        if not timed[i] or f32(c.energy) != 0.0 or all(c.held):
            return False
        for j, o in enumerate(cores):
            if timed[j] and f32(o.energy) > 0.0:
                dx, dy, dz = (f32(a - b) for a, b in zip(o.pos, c.pos))
                if f32(f32(f32(dx * dx) + f32(dy * dy)) + f32(dz * dz)) < 225.0:
                    return False
        return True

    candidates = [i for i in range(len(cores)) if free(i)]
    if not candidates:
        return None
    return candidates[(lcg(x) * len(candidates)) >> 32]


class Core:
    def __init__(self, pos, energy=0.0, maximum=50.0, enabled=1, thresholds=(3000, 2000, 1000, 50, 0),
                 held=(0, 0, 0, 0, 0)):
        self.pos, self.energy, self.maximum, self.enabled = tuple(f32(v) for v in pos), energy, maximum, enabled
        self.thresholds, self.held = thresholds, held
        self.pending, self.pending_at = 0, 0xDEADBEEF

    def timed(self):
        return self.enabled != 0 and f32(self.maximum) > 0 and len(self.thresholds) > 0


# --- the routine ------------------------------------------------------------

def assemble(nasm):
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([nasm, "-f", "bin", f"-DCAVE={CAVE:#x}", ASM, "-o", out], check=True)
        return open(out, "rb").read()


class Machine:
    def __init__(self, code):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        emu.mem_map(CAVE, 0x1000, UC_PROT_ALL)
        emu.mem_write(CAVE, code)
        emu.mem_map(DATA, 0x100000, UC_PROT_ALL)
        emu.mem_map(STACK - 0x8000, 0x8000, UC_PROT_ALL)
        emu.mem_map(STOP, 0x1000, UC_PROT_ALL)

    def run(self, cores, now, delta, start, seed):
        """Calls try once; returns (the chosen index or None, cores' bytes before, after)."""
        emu = self.emu
        n = len(cores)
        core_at = [DATA + 0x1000 + i * CORE_SIZE for i in range(n)]
        states_at = DATA + 0x1000 + n * CORE_SIZE
        artefacts_at = states_at + n * 8 * STATE_SIZE
        table_at = artefacts_at + n * 8 * (4 + ARTEFACT_SIZE)
        ctx = table_at + n * 16
        for i, c in enumerate(cores):
            body = bytearray((i * 7 + b * 13) & 0xFF for b in range(CORE_SIZE))
            struct.pack_into("<B", body, PENDING, c.pending)
            struct.pack_into("<I", body, PENDING_AT, c.pending_at)
            struct.pack_into("<B", body, ENABLED, c.enabled)
            struct.pack_into("<f", body, ENERGY, c.energy)
            struct.pack_into("<f", body, MAXIMUM, c.maximum)
            pointers = states_at + i * 8 * STATE_SIZE
            for s, threshold in enumerate(c.thresholds):
                state = bytearray(STATE_SIZE)
                struct.pack_into("<I", state, THRESHOLD, threshold)
                emu.mem_write(pointers + (s + 1) * STATE_SIZE, bytes(state))
                emu.mem_write(pointers + 4 * s, struct.pack("<I", pointers + (s + 1) * STATE_SIZE))
            struct.pack_into("<II", body, BEGIN, pointers, pointers + 4 * len(c.thresholds))
            own = artefacts_at + i * 8 * (4 + ARTEFACT_SIZE)
            for a, holder in enumerate(c.held):
                artefact = own + 32 + a * ARTEFACT_SIZE
                emu.mem_write(artefact + HOLDER, struct.pack("<I", holder))
                emu.mem_write(own + 4 * a, struct.pack("<I", artefact))
            struct.pack_into("<II", body, ARTEFACTS, own, len(c.held))
            emu.mem_write(core_at[i], bytes(body))
            emu.mem_write(table_at + 16 * i, struct.pack("<Ifff", core_at[i], *c.pos))
        emu.mem_write(ctx, struct.pack("<IIIIII", now & M, delta, start & M, seed & M, n, table_at))
        before = [bytes(emu.mem_read(a, CORE_SIZE)) for a in core_at]
        esp = STACK - 0x100
        emu.mem_write(esp, struct.pack("<II", STOP, ctx))
        kept = {UC_X86_REG_EBX: 0x11111111, UC_X86_REG_ESI: 0x22222222, UC_X86_REG_EDI: 0x33333333,
                UC_X86_REG_EBP: 0x44444444}
        for reg, value in kept.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, esp)
        emu.emu_start(CAVE, STOP, count=2_000_000)
        assert emu.reg_read(UC_X86_REG_ESP) == esp + 4, "the stack is not as it was"
        for reg, value in kept.items():
            assert emu.reg_read(reg) == value, "a kept register changed"
        after = [bytes(emu.mem_read(a, CORE_SIZE)) for a in core_at]
        eax = emu.reg_read(UC_X86_REG_EAX)
        chosen = core_at.index(eax) if eax else None
        return chosen, before, after


class Checks:
    def __init__(self, machine):
        self.machine, self.failed, self.count = machine, 0, 0

    def check(self, ok, what):
        self.count += 1
        if not ok:
            self.failed += 1
            print("FAIL", what)

    def call(self, cores, now, start, seed, delta=TICK, what=""):
        """Runs the routine, checks it against the rules and that only the chosen core's
        energy, pending and its time changed (as the rules say). Applies it to `cores`."""
        chosen, before, after = self.machine.run(cores, now, delta, start, seed)
        expected = reference(cores, now, delta, start, seed)
        self.check(chosen == expected, f"{what}: chose {chosen}, the rules {expected} (now {now})")
        for i, (b, a) in enumerate(zip(before, after)):
            if i != chosen:
                self.check(a == b, f"{what}: core {i} changed and was not chosen")
                continue
            c = cores[i]
            want = bytearray(b)
            struct.pack_into("<f", want, ENERGY, float(c.thresholds[0]))
            struct.pack_into("<B", want, PENDING, 1)
            struct.pack_into("<I", want, PENDING_AT, (now + ARTEFACT_AFTER) & M)
            self.check(a == bytes(want), f"{what}: the chosen core's bytes")
            c.energy, c.pending, c.pending_at = float(c.thresholds[0]), 1, (now + ARTEFACT_AFTER) & M
        return chosen


def london_like(rng, places=27):
    cores = []
    for _ in range(places):
        pos = (rng.uniform(-150, 150), rng.uniform(0, 20), rng.uniform(-150, 150))
        cores.extend(Core(pos) for _ in range(4))
    return cores


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--nasm", default=os.path.join(os.environ.get("NASM_DIR", ""), "nasm"))
    args = parser.parse_args()
    checks = Checks(Machine(assemble(args.nasm)))
    check = checks.check
    rng = random.Random(42)
    start, seed = 412_350, 0x8123ABCD

    # Before the match, and before its first window: nothing, ever.
    cores = london_like(rng)
    check(checks.call(cores, 600_000, M, seed, what="not started") is None, "not started: a choice")
    for now in range(start, start + DELAY - VARIATION, 7 * TICK):
        if checks.call(cores, now, start, seed, what="before the first try") is not None:
            check(False, "a choice before start + 20 s")
            break

    # A whole match of idle cores: one try per window, at the rules' time, in its tick;
    # each try charges one core. Every 3rd charged core is "picked up" (energy to its
    # maximum, busy) and every 5th goes back to idle, so blocks and returns are played.
    # The routine runs in the ticks around each try and in every 41st tick (Unicorn is
    # slow); in every other tick the rules say nothing happens.
    cores = london_like(random.Random(7))
    tries, charged = [], []
    for now in range(start, start + 900_000, TICK):
        k = (now - (start + DELAY - VARIATION)) // INTERVAL if now >= start + DELAY - VARIATION else None
        near = k is not None and abs(now - try_time(start, seed, k)[0]) <= TICK
        if near or (now // TICK) % 41 == 0:
            chosen = checks.call(cores, now, start, seed, what="match")
        else:
            chosen = reference(cores, now, TICK, start, seed)
            check(chosen is None, f"the rules choose at {now}, away from a try")
        if chosen is not None:
            tries.append((k, now))
            charged.append(chosen)
            if len(charged) % 3 == 0:
                cores[charged[-1]].energy = cores[charged[-1]].maximum
            if len(charged) % 5 == 0:
                cores[charged[-2]].energy = 0.0
    ks = [k for k, _ in tries]
    check(len(ks) == len(set(ks)), "two tries in one window")
    for k, now in tries:
        t, _ = try_time(start, seed, k)
        check(t <= now < t + TICK, f"try {k} at {now}, its time {t}")
        check(start + DELAY + k * INTERVAL - VARIATION <= t < start + DELAY + k * INTERVAL + VARIATION,
              f"try {k}'s time {t} out of its window")
    check(len(tries) >= 16, f"only {len(tries)} tries in a match")
    active = sum(1 for c in cores if f32(c.energy) > f32(c.maximum))
    check(active <= MAX_ACTIVE, f"{active} active")

    # The tick that reaches a try's time is the one: one tick earlier nothing, a long tick
    # (delta 1 s) still finds it.
    cores = london_like(random.Random(3))
    t, _ = try_time(start, seed, 4)
    check(checks.call(cores, t - 1, start, seed, delta=TICK, what="just before") is None, "a try before its time")
    check(checks.call(cores, t + 999, start, seed, delta=1000, what="a long tick") is not None, "a long tick missed it")
    check(checks.call(cores, t + 50, start, seed, delta=TICK, what="the tick after") is None, "a try taken twice")

    # 16 active stop the choice; 15 do not.
    t, _ = try_time(start, seed, 2)
    for active, chosen in ((16, False), (15, True)):
        cores = [Core((i * 100.0, 0.0, 0.0), energy=1000.0) for i in range(active)]
        cores += [Core((5000.0 + i * 100.0, 0.0, 0.0)) for i in range(10)]
        got = checks.call(cores, t, start, seed, what=f"{active} active")
        check((got is not None) == chosen, f"{active} active: chose {got}")

    # A busy timed core blocks within 15 m (after the pick-up too); an idle one or a stock
    # core never does.
    for distance, energy, blocks in ((14.99, 50.0, True), (15.01, 50.0, False), (0.0, 0.0, False),
                                     (10.0, 3000.0, True), (3.0, 1000.0, True)):
        cores = [Core((0.0, 0.0, 0.0)), Core((0.0, distance, 0.0), energy=energy)]
        got = checks.call(cores, t, start, seed, what=f"block at {distance} m")
        check((got is None) == blocks, f"a busy core at {distance} m, energy {energy}: chose {got}")
    stock = [Core((0.0, 0.0, 1.0), energy=500.0, maximum=0.0),       # a stock energy core
             Core((0.0, 1.0, 0.0), energy=1.0, enabled=0),          # energy off
             Core((1.0, 0.0, 0.0), energy=0.0, maximum=0.0),        # stock, idle
             Core((2.0, 0.0, 0.0), energy=0.0, thresholds=())]      # no states
    # An artefact of its own must be free (held: in a container or a player's): none free,
    # not chosen; one free, chosen.
    for held, chosen in (((1, 2, 3, 4, 5), None), ((1, 2, 0, 4, 5), 0), ((), None)):
        got = checks.call([Core((0.0, 0.0, 0.0), held=held)], t, start, seed, what=f"artefacts {held}")
        check(got == chosen, f"artefacts held {held}: chose {got}")
    got = checks.call(stock + [Core((0.0, 0.0, 0.0))], t, start, seed, what="stock cores")
    check(got == 4, f"stock cores near: chose {got}")
    check(checks.call(stock, t, start, seed, what="stock only") is None, "a stock core chosen")

    # Many seeds and tables: the rules' choice each time, and every candidate of a small
    # table gets its turn.
    seen = set()
    for n in range(300):
        s = random.Random(n).getrandbits(32)
        cores = [Core((i * 20.0, 0.0, 0.0)) for i in range(6)]
        t, _ = try_time(start, s, 1)
        got = checks.call(cores, t, start, s, what=f"seed {s:#x}")
        seen.add(got)
    check(seen == set(range(6)), f"chosen over 300 seeds: {sorted(x for x in seen if x is not None)}")
    for n in range(40):
        r = random.Random(1000 + n)
        cores = london_like(r, places=r.randint(1, 30))
        for c in cores:
            c.energy = r.choice((0.0, 0.0, 0.0, 50.0, 1000.0, 2000.0, 3000.0, 0.0))
            c.held = tuple(r.choice((0, 0, 0, 1)) for _ in range(5))
        s = r.getrandbits(32)
        k = r.randint(0, 25)
        tk, _ = try_time(start, s, k)
        checks.call(cores, tk + r.randrange(0, TICK), start, s, what=f"table {n}")

    if checks.failed:
        sys.exit(f"{checks.failed} of {checks.count} checks failed")
    print(f"all {checks.count} checks passed")


if __name__ == "__main__":
    main()
