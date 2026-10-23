#!/usr/bin/env python3
"""Run patch 42 (dynamic anomalies) in the built exe on made-up worlds, with
the game's own anomaly code, and compare every tick with the rules of
../docs/notes/dynamic_anomalies.md written again here in Python.

    python check_dynamic_anomalies_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe is built here (patch_026g.build, the
default build with the late join, the checked-in .bin files) and its
sections are loaded as they are. A world tick runs as
game_world_core::atomic_tick runs it: the patched entry 0x2D1E20 (the hook
and the rule, stopped where it goes back to the original at 0x2D1E26),
then each anomaly's own generic_anomaly_core::tick 0x31B5B0 (the game's
select_state, anomaly_state::initialize/execute/finalize), in the tick
list's order. A pick-up is the patched on_artefact_container_use 0x33BCD0.
Made up: the world's fields the rule reads (the rules vector with a
timelimit_rule_core, the static info's seed, tick list 6), the cores'
fields (load's offsets), their states (no zone groups), artefacts,
containers and the containers' use boxes down to the bullet object's
origin. Stubs: spawn_artefact (the core's slot 15: an artefact of its own
goes into the container, nothing more is due) and each artefact's tick.

Checked, every tick and every core (energy, state, its end, artefact due
and when): the build against the rules' model, over a 15-minute London-like
match with players taking artefacts; the first try at start + 30 s, each
next one 20 to 40 s after the tick of the last; at most 16 active; a busy
anomaly blocks within 15 m (14.99 m blocks, 15.01 m does not); one with no
free artefact is never chosen; an artefact lands 24 s after its anomaly is
chosen; stock cores (level_03's timer, level_02's cloud, energy off) are
never chosen or counted and their pick-up is the original's, byte for
byte; the hooks write nothing but the chosen core's energy, pending and its
time (a memory-write hook: no state of their own), so a world copied at any
tick (a late joiner, a resync, a rollback) goes on exactly as the world it
came from, which is run too; two worlds at other addresses with the same
order choose alike.
"""
import math
import random
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE, UC_HOOK_MEM_WRITE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP

import patch_026g as P

ATOMIC_TICK, ATOMIC_TICK_ON = 0x2D1E20, 0x2D1E26
PICKUP = 0x33BCD0
GENERIC_TICK = 0x31B5B0
# The time rule a game has is the client's timelimit_rule (0xA06E8C, set at
# 0x267A61 over the core's 0xA06E2C); the core's own is what a world without
# the client class would hold. Both must start the rule; tdm's must not.
TIMELIMIT_RULE_VTABLE, TIMELIMIT_CORE_VTABLE, TDM_VTABLE = 0xA06E8C, 0xA06E2C, 0xA15DA4

W_RULES, W_ANOMALIES_COUNT, W_STATIC_INFO, INFO_SEED = 0x18824, 0x188A0, 0x18FAC, 0x577B0
RULE_START, RULE_STATE = 0x12C, 0x148
ENABLED_ART, MAX_ART, RESPAWN, CHANCE, PENDING, PENDING_AT, CONTAINERS = 0x124, 0x128, 0x12C, 0x134, 0x138, 0x13C, 0x140
ENERGY_ENABLED, ENERGY, MAXIMUM, DECREASE, AF_USE = 0x14C, 0x150, 0x154, 0x158, 0x15C
STATES, CURRENT, NODE, ARTEFACTS, ARTEFACTS_COUNT, RANDOM = 0x180, 0x18C, 0x194, 0x1B0, 0x1B4, 0x1B8
HOLDER = 0x280
S_ENABLED, S_THRESHOLD, S_ACTIVE, S_EXIT, S_GROUPS, S_CORE, S_END = 0x00, 0x08, 0x10, 0x14, 0x1C, 0x28, 0x2C
CORE_SIZE = 0x2000
CONTAINER_AT = 0x1700                      # in a core's made-up block

DELAY, VARIATION, MAX_ACTIVE, TICK = 30000, 10000, 16, 50
LCG_MUL, TRY_SALT, PICK_SALT = 0x08088405, 0x7A11ED42, 0x51C4E5A3
M = 0xFFFFFFFF

# The release's states on 0.26g (docs/notes/dynamic_anomalies.md):
# (threshold, active_time_sec, energy_on_exit), highest first.
TIMED_STATES = ((3000, 20, 2000), (2000, 4, 1000), (1000, 0, 0), (50, 60, 0), (0, 0, 0))
ARTEFACT_STATE = 2

DATA, DATA_SIZE, STACK, STUB = 0x40000000, 0x400000, 0x3F000000, 0x3E000000
SPAWN_STUB, ARTEFACT_TICK_STUB, RET_STUB, STOP = STUB, STUB + 0x10, STUB + 0x20, STUB + 0x30


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def lcg(x):
    return (x * LCG_MUL + 1) & M


# --- the rules ---------------------------------------------------------------

class Core:
    """One anomaly as the rules see it, and as a world is made of."""

    def __init__(self, place, states=TIMED_STATES, respawn=100000.0, energy_enabled=1, af_use=50,
                 artefacts=5, energy=0.0, decrease=0):
        self.place = tuple(f32(v) for v in place)
        self.states, self.respawn, self.energy_enabled, self.af_use = states, respawn, energy_enabled, af_use
        self.energy, self.decrease = f32(energy), decrease
        self.held = [0] * artefacts
        self.current, self.end = None, 0
        self.pending, self.pending_at = 0, 0xFFFFFFF0

    def timed(self):
        return self.energy_enabled != 0 and f32(self.respawn) >= 100000.0 and len(self.states) > 0

    def snapshot(self):
        return (round(self.energy, 3), self.current, self.end, self.pending, self.pending_at if self.pending else None)


class Rules:
    def __init__(self, cores, seed):
        self.cores, self.seed, self.choices = cores, seed, []

    def try_at(self, now, delta, start):
        """The try that falls in (now - delta, now]: (its number, its draw), or None."""
        if start is None:
            return None
        t, x = start + DELAY, self.seed ^ TRY_SALT
        for j in range(4096):
            if t > now:
                return None
            x = lcg(x)
            if now - t < delta:
                return j, x
            t = (t + TICK - 1) // TICK * TICK + 2 * VARIATION + ((x * 2 * VARIATION) >> 32)
        return None

    def candidate(self, c):
        if f32(c.energy) != 0.0 or all(c.held):
            return False
        for o in self.cores:
            if o.timed() and f32(o.energy) > 0.0:
                dx, dy, dz = (f32(a - b) for a, b in zip(o.place, c.place))
                if f32(f32(f32(dx * dx) + f32(dy * dy)) + f32(dz * dz)) < 225.0:
                    return False
        return True

    def schedule(self, now, delta, start):
        found = self.try_at(now, delta, start)
        if found is None:
            return None
        j, x = found
        timed = [c for c in self.cores if c.timed()]
        candidates = [c for c in timed if self.candidate(c)]
        if sum(1 for c in timed if f32(c.energy) > f32(c.af_use)) >= MAX_ACTIVE or not candidates:
            self.choices.append((now, j, None))
            return None
        c = candidates[(lcg(x ^ PICK_SALT) * len(candidates)) >> 32]
        c.energy = f32(float(c.states[0][0]))
        c.pending = 1
        c.pending_at = (now + 1000 * sum(a for t, a, _ in c.states if t > c.af_use)) & M
        self.choices.append((now, j, self.cores.index(c)))
        return c

    @staticmethod
    def tick(c, now, delta):
        """generic_anomaly_core::tick as the rules read it (no zone groups); True when an artefact lands."""
        if c.energy_enabled:
            c.energy = f32(c.energy - min(f32(c.decrease * delta * 0.001), c.energy))
        if c.current is None or c.end == 0:
            chosen = next((i for i, (threshold, _, _) in enumerate(c.states) if c.energy >= threshold), 0)
            if chosen != c.current:
                c.current = chosen
                active = c.states[chosen][1]
                c.end = (now + active * 1000) if active else 0
        if c.end and now > c.end:
            c.energy = f32(float(c.states[c.current][2]))
            c.end = 0
        if c.pending and c.pending_at <= now:
            c.held[c.held.index(0)] = 1
            c.pending = 0
            return True
        return False

    @staticmethod
    def pickup(c):
        if c.timed():
            c.energy = f32(float(c.af_use))
            c.pending = 0


# --- the build in Unicorn ----------------------------------------------------

class World:
    """A world in Unicorn: an exe, the cores, the time rule, the seed."""

    def __init__(self, image, cores, seed, base=DATA, rule_vtable=TIMELIMIT_RULE_VTABLE):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in P.Headers(image).sections(image):
            start = P.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(image[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(base, DATA_SIZE, UC_PROT_ALL)
        emu.mem_map(STACK - 0x10000, 0x10000, UC_PROT_ALL)
        emu.mem_map(STUB, 0x1000, UC_PROT_ALL)
        emu.mem_write(SPAWN_STUB, bytes.fromhex("c20400"))         # ret 4
        emu.mem_write(ARTEFACT_TICK_STUB, bytes.fromhex("c20800"))  # ret 8
        emu.mem_write(RET_STUB, bytes.fromhex("c3"))
        self.world, self.info = base, base + 0x20000
        self.rule, self.rules = base + 0x80000, base + 0x80800
        self.vtable, self.artefact_vtable = base + 0x81000, base + 0x81400
        emu.mem_write(self.vtable, struct.pack("<64I", *([RET_STUB] * 15 + [SPAWN_STUB] + [RET_STUB] * 48)))
        emu.mem_write(self.artefact_vtable, struct.pack("<64I", *([ARTEFACT_TICK_STUB] * 64)))
        emu.mem_write(self.rule, struct.pack("<I", rule_vtable))
        emu.mem_write(self.rules, struct.pack("<I", self.rule))
        emu.mem_write(self.world + W_RULES, struct.pack("<II", self.rules, self.rules + 4))
        emu.mem_write(self.world + W_STATIC_INFO, struct.pack("<I", self.info))
        emu.mem_write(self.info + INFO_SEED, struct.pack("<I", seed))
        self.core_at = [base + 0x90000 + i * CORE_SIZE for i in range(len(cores))]
        for i, c in enumerate(cores):
            self.write_core(i, c)
        first = self.core_at[0] + NODE if cores else 0
        emu.mem_write(self.world + W_ANOMALIES_COUNT, struct.pack("<II", len(cores), first))
        self.set_rule(None)
        self.spawned, self.writes, self.watch = [], [], False
        emu.hook_add(UC_HOOK_CODE, self.on_spawn, begin=SPAWN_STUB, end=SPAWN_STUB)
        emu.hook_add(UC_HOOK_MEM_WRITE, self.on_write)

    def next_node(self, i):
        return self.core_at[i + 1] + NODE if i + 1 < len(self.core_at) else 0

    def write_core(self, i, c):
        a = self.core_at[i]
        body = bytearray(CORE_SIZE)
        struct.pack_into("<I", body, 0, self.vtable)
        struct.pack_into("<B", body, ENABLED_ART, 1)
        struct.pack_into("<I", body, MAX_ART, 1)
        struct.pack_into("<f", body, RESPAWN, c.respawn)
        struct.pack_into("<f", body, CHANCE, 1.0)
        struct.pack_into("<B", body, PENDING, c.pending)
        struct.pack_into("<I", body, PENDING_AT, c.pending_at)
        struct.pack_into("<B", body, ENERGY_ENABLED, c.energy_enabled)
        struct.pack_into("<f", body, ENERGY, c.energy)
        struct.pack_into("<I", body, DECREASE, c.decrease)
        struct.pack_into("<I", body, AF_USE, c.af_use)
        struct.pack_into("<I", body, RANDOM, 0x1234567 + i)
        struct.pack_into("<IIII", body, NODE, a, GENERIC_TICK, 0, self.next_node(i))
        for k, (threshold, active, exit_) in enumerate(c.states):          # states at +400h
            s = 0x400 + k * 0x40
            struct.pack_into("<B", body, s + S_ENABLED, 1)
            struct.pack_into("<I", body, s + S_THRESHOLD, threshold)
            struct.pack_into("<II", body, s + S_ACTIVE, active, exit_)
            struct.pack_into("<II", body, s + S_GROUPS, a + s + 0x38, a + s + 0x38)
            struct.pack_into("<I", body, s + S_CORE, a)
            struct.pack_into("<I", body, 0x380 + 4 * k, a + s)
        struct.pack_into("<II", body, STATES, a + 0x380, a + 0x380 + 4 * len(c.states))
        for k, held in enumerate(c.held):                                   # artefacts at +800h
            struct.pack_into("<I", body, 0x800 + k * 0x300, self.artefact_vtable)
            struct.pack_into("<I", body, 0x800 + k * 0x300 + HOLDER, held)
            struct.pack_into("<I", body, 0x7C0 + 4 * k, a + 0x800 + k * 0x300)
        struct.pack_into("<II", body, ARTEFACTS, a + 0x7C0, len(c.held))
        # one container -> its use box -> ghost -> bullet object -> origin
        struct.pack_into("<II", body, CONTAINERS, a + 0x16F0, a + 0x16F4)
        struct.pack_into("<I", body, 0x16F0, a + CONTAINER_AT)
        struct.pack_into("<II", body, CONTAINER_AT + 0x38, a + 0x16F8, 1)
        struct.pack_into("<I", body, 0x16F8, a + 0x1780)
        struct.pack_into("<I", body, 0x1780 + 0x18, a + 0x17C0)
        struct.pack_into("<I", body, 0x17C0 + 0x18, a + 0x1800)
        struct.pack_into("<fff", body, 0x1800 + 0x40, *c.place)
        self.emu.mem_write(a, bytes(body))

    def set_rule(self, start):
        self.emu.mem_write(self.rule + RULE_START, struct.pack("<I", M if start is None else start))
        self.emu.mem_write(self.rule + RULE_STATE, struct.pack("<I", 1 if start is None else 3))

    def on_spawn(self, emu, address, size, user):
        core = emu.reg_read(UC_X86_REG_ECX)
        now = struct.unpack("<I", emu.mem_read(emu.reg_read(UC_X86_REG_ESP) + 4, 4))[0]
        self.spawned.append((self.core_at.index(core), now))
        for k in range(struct.unpack("<I", emu.mem_read(core + ARTEFACTS_COUNT, 4))[0]):
            art = struct.unpack("<I", emu.mem_read(core + 0x7C0 + 4 * k, 4))[0]
            if struct.unpack("<I", emu.mem_read(art + HOLDER, 4))[0] == 0:
                emu.mem_write(art + HOLDER, struct.pack("<I", 1))
                break
        emu.mem_write(core + PENDING, b"\0")

    def on_write(self, emu, access, address, size, value, user):
        if self.watch and not STACK - 0x10000 <= address < STACK:
            self.writes.append((address, size))

    def call(self, address, ecx=None, args=(), stop_at=STOP, regs=None):
        emu = self.emu
        esp = STACK - 0x400
        emu.mem_write(esp, struct.pack(f"<{1 + len(args)}I", STOP, *args))
        emu.reg_write(UC_X86_REG_ESP, esp)
        if ecx is not None:
            emu.reg_write(UC_X86_REG_ECX, ecx)
        for reg, value in (regs or {}).items():
            emu.reg_write(reg, value)
        emu.emu_start(address, stop_at, count=5_000_000)
        return esp

    def tick(self, now, delta):
        """A world tick: the patched atomic_tick's entry (to 0x2D1E26), then each core's tick.
        Returns (the entry left as the original's would, the entry's writes)."""
        kept = {UC_X86_REG_EBX: 0x1111, UC_X86_REG_ESI: 0x2222, UC_X86_REG_EBP: 0x3333, UC_X86_REG_EDI: now,
                UC_X86_REG_ECX: 0x4444, UC_X86_REG_EDX: 0x5555}
        self.writes, self.watch = [], True
        esp = self.call(ATOMIC_TICK, args=(self.world, delta), stop_at=ATOMIC_TICK_ON, regs=kept)
        self.watch = False
        emu = self.emu
        ok = emu.reg_read(UC_X86_REG_ESP) == esp - 4 \
            and struct.unpack("<I", emu.mem_read(esp - 4, 4))[0] == 0x4444 \
            and emu.reg_read(UC_X86_REG_EAX) == 0x51EB851F \
            and all(emu.reg_read(r) == v for r, v in kept.items())
        writes = self.writes
        for a in self.core_at:
            self.call(GENERIC_TICK, ecx=a, args=(delta, now, 1))
        return ok, writes

    def pickup(self, i, now):
        a = self.core_at[i]
        self.writes, self.watch = [], True
        self.call(PICKUP, ecx=a, args=(a + CONTAINER_AT, now))
        self.watch = False
        return self.writes

    def state(self, i):
        emu, a = self.emu, self.core_at[i]
        energy = struct.unpack("<f", emu.mem_read(a + ENERGY, 4))[0]
        current = struct.unpack("<I", emu.mem_read(a + CURRENT, 4))[0]
        index = None if current == 0 else (current - a - 0x400) // 0x40
        end = struct.unpack("<I", emu.mem_read(current + S_END, 4))[0] if current else 0
        pending = emu.mem_read(a + PENDING, 1)[0]
        pending_at = struct.unpack("<I", emu.mem_read(a + PENDING_AT, 4))[0]
        return (round(energy, 3), index, end, pending, pending_at if pending else None)

    def copy_from(self, other):
        """Every core as another world has it, moved to this world's addresses."""
        for i, (a, b) in enumerate(zip(self.core_at, other.core_at)):
            body = bytearray(other.emu.mem_read(b, CORE_SIZE))
            for off in range(0, CORE_SIZE, 4):
                v = struct.unpack_from("<I", body, off)[0]
                if b <= v < b + CORE_SIZE:
                    struct.pack_into("<I", body, off, v - b + a)
                elif v == other.vtable:
                    struct.pack_into("<I", body, off, self.vtable)
                elif v == other.artefact_vtable:
                    struct.pack_into("<I", body, off, self.artefact_vtable)
            struct.pack_into("<I", body, NODE + 12, self.next_node(i))
            self.emu.mem_write(a, bytes(body))


class Checks:
    def __init__(self):
        self.failed = self.count = 0

    def check(self, ok, what):
        self.count += 1
        if not ok:
            self.failed += 1
            if self.failed <= 40:
                print("FAIL", what)


def layout(rng, places=27):
    """London-like: four kinds at each place (within 0.8 m), every fifth place 12 to 15 m from the last."""
    points = []
    while len(points) < places:
        if points and len(points) % 5 == 0:
            d, a = rng.uniform(12.0, 14.9), rng.uniform(0, 2 * math.pi)
            base = points[-1]
            points.append((base[0] + d * math.cos(a), base[1] + rng.uniform(-1, 1), base[2] + d * math.sin(a)))
        else:
            points.append((rng.uniform(-150, 150), rng.uniform(0, 20), rng.uniform(-150, 150)))
    return [Core((p[0] + rng.uniform(-0.4, 0.4), p[1] + rng.uniform(-0.2, 0.2), p[2] + rng.uniform(-0.4, 0.4)))
            for p in points for _ in range(4)]


def only(world, writes, cores):
    fields = set()
    for i in cores:
        a = world.core_at[i]
        fields |= {(a + ENERGY, 4), (a + PENDING, 1), (a + PENDING_AT, 4)}
    return all(w in fields for w in writes)


def run_match(image, checks, seed, cores, start, seconds, rng=None, what="match", copy_at=None):
    """A match in the build and by the rules side by side, tick by tick. Players take a
    landed artefact at random (rng; none without). With copy_at, a second world copied
    from the first after that tick runs beside it from then on."""
    rules, world, late = Rules(cores, seed), World(image, cores, seed), None
    landed = set()
    for now in range(0, seconds * 1000, TICK):
        started = now >= start
        if now == start:
            for w in (world, late):
                if w:
                    w.set_rule(start)
        chosen = rules.schedule(now, TICK, start if started else None)
        ok, writes = world.tick(now, TICK)
        checks.check(ok, f"{what} {now}: the hook goes on at 0x2D1E26 with the registers and the stack as they "
                         "were, `push ecx; mov eax,51EB851Fh` done")
        checks.check(only(world, writes, [cores.index(chosen)] if chosen else []),
                     f"{what} {now}: the hook wrote {[(hex(a), s) for a, s in writes][:4]}")
        for i, c in enumerate(cores):
            if Rules.tick(c, now, TICK):
                landed.add(i)
        if late is not None:
            late.tick(now, TICK)
        bad = [(i, world.state(i), c.snapshot()) for i, c in enumerate(cores) if world.state(i) != c.snapshot()]
        checks.check(not bad, f"{what} {now}: core {bad[:1]} (build, rules)")
        if late is not None:
            apart = [i for i in range(len(cores)) if late.state(i) != world.state(i)]
            checks.check(not apart, f"{what} {now}: the copied world's cores {apart[:3]} differ")
        if bad or (late is not None and apart):
            return rules, world
        if rng is not None:
            for i in sorted(landed):
                if cores[i].current == ARTEFACT_STATE and rng.random() < 0.01:
                    landed.discard(i)
                    checks.check(only(world, world.pickup(i, now), [i]), f"{what} {now}: the pick-up's writes")
                    Rules.pickup(cores[i])
                    if late is not None:
                        late.pickup(i, now)
        if now == copy_at:
            late = World(image, cores, seed, base=DATA + DATA_SIZE)
            late.copy_from(world)
            late.set_rule(start if started else None)
    return rules, world


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    image, _ = P.build(original, "section", open(P.default_bin("section"), "rb").read(), None,
                       open(P.late_join_bin(), "rb").read())
    checks = Checks()
    check = checks.check

    # A London match: 27 places of four, 15 minutes, players taking artefacts.
    cores = layout(random.Random(42))
    start = 20_000
    rules, world = run_match(image, checks, 0x8123ABCD, cores, start, 900, random.Random(1), what="London")
    tries = rules.choices
    check(tries[:1] and tries[0][0] == start + DELAY, f"the first try at the start + 30 s: {tries[:1]}")
    gaps = [b[0] - a[0] for a, b in zip(tries, tries[1:])]
    check(gaps and min(gaps) >= 20000 and max(gaps) < 40000 + TICK, f"tries {min(gaps)} to {max(gaps)} ms apart")
    chosen = [(now, c) for now, _, c in tries if c is not None]
    check(len(chosen) >= 20 and len({cores[c].place for _, c in chosen}) >= 15,
          f"{len(chosen)} anomalies charged in 15 minutes, at many places")
    for when, i in chosen[:8]:
        check((i, when + 24000) in world.spawned, f"core {i} charged at {when}: its artefact landed at "
              f"{[n for k, n in world.spawned if k == i]}")
    blocked = [(now, c) for now, _, c in tries if c is None]
    check(len(blocked) <= len(tries), f"{len(tries)} tries, {len(blocked)} without a choice")

    # A late joiner, a resync or a rollback: a world copied at 5 minutes goes on alike.
    run_match(image, checks, 0x1357, layout(random.Random(7)), 10_000, 420, random.Random(3),
              what="late joiner", copy_at=300_000)

    # 16 active stop the choice; 15 do not.
    for active, expect in ((16, False), (15, True)):
        cores = [Core((i * 100.0, 0.0, 0.0), energy=1000.0) for i in range(active)]
        cores += [Core((5000.0 + i * 100.0, 0.0, 0.0)) for i in range(6)]
        rules, _ = run_match(image, checks, 99, cores, 0, 31, what=f"{active} active")
        check(rules.choices[:1] and (rules.choices[0][2] is not None) == expect,
              f"{active} active: the first try chose {rules.choices[:1]}")

    # A busy one blocks within 15 m, also after its pick-up; an idle one does not.
    for distance, energy, blocks in ((14.99, 50.0, True), (15.01, 50.0, False), (10.0, 3000.0, True),
                                     (3.0, 1000.0, True), (1.0, 0.0, False)):
        cores = [Core((0.0, 0.0, 0.0)), Core((0.0, distance, 0.0), energy=energy, states=((int(energy), 0, 0),))]
        rules, _ = run_match(image, checks, 5, cores, 0, 31, what=f"block {distance}")
        got = [c for _, _, c in rules.choices][:1]
        check(got == [None] if blocks else got in ([0], [1]), f"{distance} m from a busy one (energy {energy}): {got}")

    # No free artefact: never chosen.
    cores = [Core((0.0, 0.0, 0.0)), Core((100.0, 0.0, 0.0))]
    cores[0].held = [1] * 5
    rules, _ = run_match(image, checks, 11, cores, 0, 31, what="no free artefact")
    check([c for _, _, c in rules.choices][:1] == [1], f"the core with its artefacts held was left: {rules.choices[:1]}")

    # Stock cores: never chosen or counted, and their pick-up is the original's.
    stock = [Core((0.0, 0.0, 0.0), states=((1, 0, 0),), respawn=60.0, energy=1.0),                 # level_03's timer
             Core((1.0, 0.0, 0.0), states=((3000, 1, 2000), (20, 0, 0), (0, 0, 0)), respawn=80.0,
                  decrease=25, af_use=100),                                                       # level_02's cloud
             Core((2.0, 0.0, 0.0), energy_enabled=0)]                                             # energy off
    rules, world = run_match(image, checks, 3, stock, 0, 40, what="stock")
    check(all(c is None for _, _, c in rules.choices), "a stock core was chosen")
    plain = World(original, stock, 3)
    for i in range(len(stock)):
        world.emu.mem_write(world.core_at[i] + ENERGY, struct.pack("<f", 500.0))
        plain.emu.mem_write(plain.core_at[i], bytes(world.emu.mem_read(world.core_at[i], CORE_SIZE)))
        for w in (world, plain):
            w.call(PICKUP, ecx=w.core_at[i], args=(w.core_at[i] + CONTAINER_AT, 40000))
        check(bytes(world.emu.mem_read(world.core_at[i], 0x400)) == bytes(plain.emu.mem_read(plain.core_at[i], 0x400)),
              f"stock core {i}: the pick-up is not the original's")

    # Two worlds at other addresses, the same order: the same choices.
    ra, _ = run_match(image, checks, 77, layout(random.Random(9)), 0, 200, what="world A")
    cores_b = layout(random.Random(9))
    wb = World(image, cores_b, 77, base=DATA + DATA_SIZE, rule_vtable=TIMELIMIT_CORE_VTABLE)
    wb.set_rule(0)
    got = []
    for now in range(0, 200_000, TICK):
        before = [wb.state(i)[0] for i in range(len(cores_b))]
        wb.tick(now, TICK)
        got += [i for i in range(len(cores_b)) if before[i] == 0 and wb.state(i)[0] == 3000]
    check(got == [c for _, _, c in ra.choices if c is not None], f"the same choices at other addresses (the time "
          f"rule timelimit_rule_core's {TIMELIMIT_CORE_VTABLE:#x} there, the client's {TIMELIMIT_RULE_VTABLE:#x} in "
          f"world A): {got[:5]}")

    # The first rule not a time rule (tdm's vtable): nothing is ever chosen.
    wt = World(image, layout(random.Random(9)), 77, rule_vtable=TDM_VTABLE)
    wt.set_rule(0)
    for now in range(0, 200_000, TICK):
        wt.tick(now, TICK)
    check(all(wt.state(i)[0] == 0 for i in range(len(wt.core_at))) and not wt.spawned,
          "a first rule that is not the time rule (tdm's vtable): no anomaly charges")

    if checks.failed:
        sys.exit(f"{checks.failed} of {checks.count} checks failed")
    print(f"all {checks.count} checks passed")


if __name__ == "__main__":
    main()
