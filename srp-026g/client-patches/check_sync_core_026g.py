#!/usr/bin/env python3
"""The sync core's differential check (patch 74, rollback pacing;
../docs/notes/sync_core.md sections 1.3, 7 (a) and 9), with Unicorn (2.1.4).

    python check_sync_core_026g.py <original survarium.exe> [--quiet]

The exe is built as patch_026g.py builds it (with the late join and the
arena). In one emulated game the code is the game's own: move (0x2D2E40),
tick_events (0x2D21E0), insert_impl (0x2D1100), erase_impl (0x2D1340), the
history ring and the change-node list, snap_inputs, the world-event trim;
and, for the patched path, the arena's sc_render_move and the Rust
sync_render_move with the engine library's thunk of move. The world is a
model: atomic_tick (0x2D1E20) steps a hash chain of (from, to, every place's
input in effect), process_events (0x2D2180) adds the node's places' masks,
serialize (0x2D2570) and deserialize (0x2D2770) keep and load that hash in
the item, new_game_state_history_item (0x2D09C0) gives the pool's eleven
items and then the oldest again, as 0x2D3F90 does; the logger and the timer
are stubs.

A timeline is frames of game time (the render move to the clock, as
game_world::tick calls it), the inputs, roster changes and discards that
arrive before each, and on_idle's move to the event horizon when its own
test (0x27D035..0x27D08D) asks for one. The same timeline runs three times:
the original exe's move, the build's move (the call site's old target) and
the build's sc_render_move (the call site's new one). Checked:
- the call site: `call 0x2D2E40` in the original and the build without the
  late join, `call sc_render_move` in the build; sc_render_move pops 4 and
  keeps EBX, ESI, EDI and EBP;
- every frame, the event-horizon item (what 0xDF hashes and 0xF6 uploads):
  its time and world the same in all three, and never changed once it was
  the horizon item (no stale world reaches the outside);
- after the timeline, a last second of frames: every item of the ring the
  same in all three (time and world), and the world's time and rollback
  point;
- every input taken or refused alike (insert_impl's "too old");
- the original's and the build's move do exactly the same loads and steps;
- the pacing's loads, steps and re-simulated steps, against the stock's.

Timelines: no late input; inputs 120-260 ms late (eight players, 100, 60
and 30 frames a second); 60-420 ms late (twelve players); bursts of thirty
at once; a 700 ms stall and its catch-up in 16 ms logic ticks; precise
inputs at odd milliseconds, two in one tick; roster changes late; inputs too
old for the ring; discards (erase_impl) of taken inputs.
"""
import hashlib
import random
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

MOVE, TICK_EVENTS, INSERT, ERASE, SERIALIZE_FIRST = 0x2D2E40, 0x2D21E0, 0x2D1100, 0x2D1340, 0x2D04F0
DESERIALIZE, SERIALIZE, ATOMIC_TICK, PROCESS_EVENTS, NEW_ITEM = 0x2D2770, 0x2D2570, 0x2D1E20, 0x2D2180, 0x2D09C0
NODE_NEW = 0x2D3D70
LOG, LOG_PASSES, LOG_DONE, ONCE, OPTION, TIMER_MAKE, TIMER_START, TIMER_MS = (
    0x111ED0, 0x111860, 0x11140, 0xDF020, 0xDDE70, 0x14610, 0x18120, 0x1B810)
RENDER_SITE = 0x1F7C37

HEAP, HEAP_SIZE, STACK, SENTINEL = 0x40000000, 0x3000000, 0x44000000, 0x45000000
# game_world_core (sync_core.md 1.1)
NODES, RING, HORIZON, START, NOW, NEWEST, POINT, OWNER = 0x0, 0x2FB8, 0x18FB4, 0x18FBC, 0x18FC0, 0x18FC4, 0x18FC8, 0x18FDC
ITEM_PREV, ITEM_NEXT, ITEM_TIME, ITEM_SIZE = 0xC0BA8, 0xC0BAC, 0xC0BB0, 0xC0C00
NODE_TIME, NODE_SIZE = 0x1BC, 0x1C0
POOL = 11                                  # (max_ping 250 + 250) / 50 + 1
EVENT_HORIZON = 500
KIND_ENTERED, KIND_LEFT, KIND_INPUT = 0, 1, 2


def digest(*parts):
    return hashlib.sha256(repr(parts).encode()).digest()[:8]


class Game:
    """One emulated game: the exe's code, a model world core."""

    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        headers = base.Headers(data)
        for _, rva, vsize, raw, rsize, _ in headers.sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(HEAP, HEAP_SIZE, UC_PROT_ALL)
        emu.mem_map(STACK - 0x100000, 0x100000, UC_PROT_ALL)
        emu.mem_map(SENTINEL, 0x1000, UC_PROT_ALL)
        emu.mem_write(SENTINEL, b"\xF4" * 16)
        self.top = HEAP
        self.world = {}                     # item -> its world's hash
        self.state = b"start"
        self.inputs = {}                    # place -> {time: value}
        self.log = []
        self.items = 0
        self.stubs = {DESERIALIZE: self.deserialize, SERIALIZE: self.serialize, ATOMIC_TICK: self.atomic_tick,
                      PROCESS_EVENTS: self.process_events, NEW_ITEM: self.new_item, NODE_NEW: self.new_node,
                      LOG: lambda: self.ret(0), LOG_PASSES: lambda: self.ret(0, 1), LOG_DONE: lambda: self.ret(0),
                      ONCE: lambda: self.ret(0), OPTION: lambda: self.ret(0, 0),
                      TIMER_MAKE: lambda: self.ret(0, self.reg(UC_X86_REG_ESI)), TIMER_START: lambda: self.ret(0),
                      TIMER_MS: lambda: self.ret(0, 0)}
        emu.hook_add(UC_HOOK_CODE, self.hook)
        self.core = self.alloc(0x19100)
        self.w32(self.core + HORIZON, EVENT_HORIZON)
        self.w32(self.core + START, 50)
        for at, value in ((NOW, 50), (NEWEST, 50), (POINT, 0xFFFFFFFF)):
            self.w32(self.core + at, value)
        self.emu.mem_write(self.core + OWNER, b"\x14")          # no local place: snap_inputs does nothing
        self.w32(self.core + NODES + 0x10, self.alloc(0x100))   # the node pool (made by the stub)
        self.call(SERIALIZE_FIRST, [], {UC_X86_REG_ESI: self.core})

    # memory and registers
    def alloc(self, size):
        at, self.top = self.top, (self.top + size + 0xFFF) & ~0xFFF
        assert self.top <= HEAP + HEAP_SIZE
        return at

    def r32(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def w32(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def reg(self, r):
        return self.emu.reg_read(r)

    def args(self, n):
        sp = self.reg(UC_X86_REG_ESP)
        return [self.r32(sp + 4 + 4 * i) for i in range(n)]

    def ret(self, pops, eax=None):
        sp = self.reg(UC_X86_REG_ESP)
        if eax is not None:
            self.emu.reg_write(UC_X86_REG_EAX, eax & 0xFFFFFFFF)
        self.emu.reg_write(UC_X86_REG_EIP, self.r32(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)

    def hook(self, emu, address, size, _):
        stub = self.stubs.get(address)
        if stub:
            stub()

    def call(self, function, stack, regs):
        sp = STACK - 0x100
        for value in reversed(stack):
            sp -= 4
            self.w32(sp, value)
        sp -= 4
        self.w32(sp, SENTINEL)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        kept = {UC_X86_REG_EBX: 0x1111, UC_X86_REG_ESI: 0x2222, UC_X86_REG_EDI: 0x3333, UC_X86_REG_EBP: 0x4444}
        for r, v in {**kept, **regs}.items():
            self.emu.reg_write(r, v)
        self.emu.emu_start(function, SENTINEL)
        return sp, kept

    # the model world
    def deserialize(self):
        core, item, _ = self.args(3)
        self.state = self.world[item]
        self.w32(core + NOW, self.r32(item + ITEM_TIME))
        self.log.append(("load", self.r32(item + ITEM_TIME)))
        self.ret(12)

    def serialize(self):
        core, item = self.args(2)
        self.w32(item + ITEM_TIME, self.r32(core + NOW))
        self.world[item] = self.state
        self.ret(8)

    def atomic_tick(self):
        core, dt = self.args(2)
        to, start, newest = self.reg(UC_X86_REG_EDI), self.r32(core + NOW), self.r32(core + NEWEST)
        assert to - start == dt and 0 < dt <= 50, (start, to, dt)
        held = tuple((p, v[max(t for t in v if t <= start)]) for p, v in sorted(self.inputs.items())
                     if any(t <= start for t in v))
        self.state = digest(self.state, start, to, held)
        self.w32(core + NOW, to)
        self.w32(core + NEWEST, max(newest, to))
        self.log.append(("step", start, to, to <= newest))
        self.ret(8)

    def process_events(self):
        core, node = self.args(2)
        masks = tuple(self.r32(node + 0x14 * p) for p in range(20))
        self.state = digest(self.state, "events", self.r32(node + NODE_TIME), masks)
        self.ret(8)

    def new_item(self):
        core = self.reg(UC_X86_REG_EAX)
        count = self.r32(core + RING)
        if self.items < POOL:
            self.items += 1
            item = self.alloc(ITEM_SIZE)
        else:                               # the oldest, unlinked (0x2D3F90 -> 0x2D4C30)
            assert count == POOL
            item = self.r32(core + RING + 4)
            after = self.r32(item + ITEM_NEXT)
            self.w32(core + RING, count - 1)
            self.w32(core + RING + 4, after)
            self.w32(after + ITEM_PREV, 0)
            self.w32(item + ITEM_NEXT, 0)
        self.ret(0, item)

    def new_node(self):
        self.ret(0, self.alloc(NODE_SIZE))

    # what a timeline does
    def move(self, target, function):
        self.log.clear()
        sp, kept = self.call(function, [self.core], {UC_X86_REG_EAX: target})
        assert self.reg(UC_X86_REG_ESP) == sp + 8, f"{function:#x} did not pop 4"
        assert all(self.reg(r) == v for r, v in kept.items()), f"{function:#x} did not keep ebx, esi, edi, ebp"
        return list(self.log)

    def insert(self, time, place, kind, value):
        self.log.clear()
        self.call(INSERT, [self.core, place, kind, 1, 1], {UC_X86_REG_EAX: time})
        taken = self.reg(UC_X86_REG_EAX) != 0
        if taken and kind == KIND_INPUT:
            self.inputs.setdefault(place, {})[time] = value
        return taken

    def erase(self, time, place, kind):
        self.call(ERASE, [self.core, place, 0], {UC_X86_REG_EAX: time, UC_X86_REG_ECX: kind})
        if kind == KIND_INPUT:
            self.inputs.get(place, {}).pop(time, None)

    def ring(self):
        out, item = [], self.r32(self.core + RING + 4)
        while item:
            out.append((self.r32(item + ITEM_TIME), self.world[item]))
            item = self.r32(item + ITEM_NEXT)
        return out

    def horizon_item(self):
        """The event-horizon item: the newest at or before newest - 500 (0x2D0B30)."""
        newest = self.r32(self.core + NEWEST)
        return next(((t, w) for t, w in reversed(self.ring()) if t + EVENT_HORIZON <= newest), None)

    def on_idle(self, now):
        """network_client::on_idle's horizon move (0x27D035..0x27D08D), the stock move."""
        point, newest, start = self.r32(self.core + POINT), self.r32(self.core + NEWEST), self.r32(self.core + START)
        if (point != 0xFFFFFFFF and point + EVENT_HORIZON < now) or (point == 0xFFFFFFFF and newest + EVENT_HORIZON < now):
            return self.move(start + (now - EVENT_HORIZON - start) // 50 * 50, MOVE)
        return []


def timeline(seed, frames, frame_ms, places, late, precise=False, burst=None, stall=None, roster=False,
             too_old=False, discards=False):
    """[(frame time, [(kind of event, args)])]: what reaches the game before each render move."""
    rnd = random.Random(seed)
    arrivals = []
    end = 50 + frames * frame_ms
    for place in range(1, places + 1):
        t = 100 + rnd.randrange(1, 50)
        while t < end + 1000:
            arrive = t + rnd.randrange(*late)
            arrivals.append((arrive, "input", (t, place, KIND_INPUT, rnd.randrange(1 << 30))))
            if precise and rnd.random() < 0.3 and t % 50 < 49:     # another change in the same tick
                second = t + rnd.randrange(1, 50 - t % 50)
                arrivals.append((arrive + rnd.randrange(0, 30), "input", (second, place, KIND_INPUT,
                                                                           rnd.randrange(1 << 30))))
            if discards and rnd.random() < 0.05:
                arrivals.append((arrive + rnd.randrange(20, 200), "erase", (t, place, KIND_INPUT)))
            t += rnd.randrange(1, 50) if precise else 50
    if roster:
        for i, at in enumerate(range(400, end, 900)):
            arrivals.append((at + rnd.randrange(150, 350), "insert", (at, 15 + i % 5, KIND_ENTERED if i % 2 == 0
                                                                       else KIND_LEFT, 0)))
    if too_old:
        for at in range(300, end, 700):
            arrivals.append((at + 600, "input", (at, 19, KIND_INPUT, at)))
    out, times = [], []
    clock = 50
    for f in range(frames):
        clock += frame_ms
        if stall is not None and f == stall:
            clock += 700
            times += [clock - 700 + 16 * k for k in range(1, 700 // 16)]   # the logic ticks of the catch-up
        times.append(clock)
    if burst is not None:
        at = 50 + burst * frame_ms
        arrivals += [(at, "input", (at - rnd.randrange(60, 300), 1 + n % places, KIND_INPUT, rnd.randrange(1 << 30)))
                     for n in range(30)]
    arrivals.sort(key=lambda a: a[0])
    i = 0
    for clock in times:
        events = []
        while i < len(arrivals) and arrivals[i][0] <= clock:
            events.append(arrivals[i][1:])
            i += 1
        out.append((clock, events))
    last = times[-1]
    out += [(last + 10 * k, []) for k in range(1, 100)]                       # a second with nothing new
    return out


def play(data, render, frames):
    """Run a timeline: per frame the horizon item, the work done, what was taken."""
    game = Game(data)
    seen, horizons, taken = {}, [], []
    work = {"loads": 0, "steps": 0, "again": 0, "rollbacks": 0}
    logs = []
    for clock, events in frames:
        for kind, args in events:
            if kind == "erase":
                game.erase(*args)
            else:
                taken.append(game.insert(*args))
        log = game.move(clock, render) + game.on_idle(clock)
        logs.append(log)
        steps = [e for e in log if e[0] == "step"]
        work["loads"] += sum(e[0] == "load" for e in log)
        work["steps"] += len(steps)
        work["again"] += sum(e[3] for e in steps)
        work["rollbacks"] += any(e[3] for e in steps)
        h = game.horizon_item()
        horizons.append(h)
        if h is not None:
            assert seen.setdefault(h[0], h[1]) == h[1], f"the horizon item {h[0]} changed after it was hashed"
    end = (game.ring(), game.r32(game.core + NOW), game.r32(game.core + POINT), game.r32(game.core + NEWEST))
    return {"horizons": horizons, "taken": taken, "end": end, "work": work, "logs": logs}


SCENARIOS = (
    ("inputs on time, 100 fps", dict(seed=1, frames=300, frame_ms=10, places=4, late=(0, 1))),
    ("8 players, 120-260 ms late, 100 fps", dict(seed=2, frames=500, frame_ms=10, places=8, late=(120, 260))),
    ("8 players, 120-260 ms late, 60 fps", dict(seed=3, frames=300, frame_ms=16, places=8, late=(120, 260))),
    ("8 players, 120-260 ms late, 30 fps", dict(seed=4, frames=150, frame_ms=33, places=8, late=(120, 260))),
    ("12 players, 60-420 ms late, 100 fps", dict(seed=5, frames=500, frame_ms=10, places=12, late=(60, 420))),
    ("bursts of 30 at once", dict(seed=6, frames=300, frame_ms=10, places=8, late=(100, 200), burst=150)),
    ("a 700 ms stall, caught up in 16 ms logic ticks", dict(seed=7, frames=300, frame_ms=10, places=8,
                                                            late=(120, 260), stall=120)),
    ("precise inputs, two in a tick", dict(seed=8, frames=300, frame_ms=10, places=6, late=(80, 300), precise=True)),
    ("roster changes late, inputs too old, discards", dict(seed=9, frames=400, frame_ms=10, places=6,
                                                           late=(100, 300), roster=True, too_old=True,
                                                           discards=True)),
)


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    quiet = "--quiet" in sys.argv
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    before, _ = base.build(original, "section", code, None, None)
    linked = base.arena()
    render = base.arena_target(linked, "sc_render_move")
    at = base.file_offset(RENDER_SITE)
    old = bytes.fromhex("e804b20d00")
    new = b"\xE8" + struct.pack("<i", render - (RENDER_SITE + 5))
    assert original[at:at + 5] == old and before[at:at + 5] == old, "the original's call of move at 0x1F7C37"
    assert built[at:at + 5] == new, "the build's call of sc_render_move at 0x1F7C37"
    failed = 0
    rows = []
    for name, kw in SCENARIOS:
        frames = timeline(**kw)
        stock = play(original, MOVE, frames)
        moved = play(bytes(built), MOVE, frames)
        paced = play(bytes(built), render, frames)
        problems = []
        if stock["logs"] != moved["logs"] or stock["end"] != moved["end"]:
            problems.append("the build's move is not the original's")
        if not (stock["taken"] == paced["taken"]):
            problems.append("an input taken or refused otherwise")
        if stock["horizons"] != paced["horizons"]:
            first = next(i for i, (a, b) in enumerate(zip(stock["horizons"], paced["horizons"])) if a != b)
            problems.append(f"the event-horizon item differs at frame {first}")
        if stock["end"] != paced["end"]:
            problems.append("the ring, the world's time or the rollback point differ at the end")
        refused = stock["taken"].count(False)
        compared = sum(h is not None for h in stock["horizons"])
        failed += bool(problems)
        s, p = stock["work"], paced["work"]
        rows.append((name, s, p))
        print(f"  {'ok' if not problems else 'FAIL'}  {name}: {compared} horizon items and {len(stock['end'][0])} "
              f"ring items the same, {len(stock['taken'])} inputs ({refused} refused) alike"
              + ("" if not problems else "; " + "; ".join(problems)))
    if not quiet:
        print("\n  work (stock -> paced): world loads, steps, re-simulated steps, frames with a rollback")
        for name, s, p in rows:
            gain = (1 - p["again"] / s["again"]) * 100 if s["again"] else 0.0
            print(f"    {name}: loads {s['loads']} -> {p['loads']}, steps {s['steps']} -> {p['steps']}, "
                  f"again {s['again']} -> {p['again']} ({gain:.0f}% fewer), rollbacks {s['rollbacks']} -> "
                  f"{p['rollbacks']}")
    if failed:
        sys.exit(f"patch 74: {failed} timeline(s) FAILED")
    print("patch 74: the render move's call goes to sc_render_move (pops 4, keeps ebx, esi, edi, ebp); on "
          f"{len(SCENARIOS)} timelines the paced rollbacks give the original's event-horizon item every frame, the "
          "original's ring, world time and rollback point at the end, and take or refuse every input alike; the "
          "build's own move is the original's")


if __name__ == "__main__":
    main()
