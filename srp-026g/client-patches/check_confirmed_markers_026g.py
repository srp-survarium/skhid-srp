#!/usr/bin/env python3
"""Run patch 73 (confirmed_markers_026g.asm: the kill marker and the hit
marker shown only for what every game agrees on) in the default build on
made-up game objects, next to the original.

    python check_confirmed_markers_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them; the game's own code runs:

- the hit's event: player::generate_hit_event (0x1F3C70) whole, with
  base_player::generate_hit_event (0x2D9F40), the event's allocation
  (0x2D43B0) from a made-up pool, push_world_event (0x2D0A60) and the
  damage model's part lookup (0x2E58B0), on a made-up victim, attacker
  (its inflicted_damage_collector +0F534h), bullet and core; three
  one-instruction routines stand for the game's getters (the attacker by
  place, the damage model, the attacker's position). The event in the core's
  list must be the original's but +8, which holds the collector's sum after
  the hit (0 for no bullet; none while a world loads, the event freed);
  the collector, the registers and the stack as the original's. Cases: a
  first hit, a second on the same victim, one on another victim, a sum
  clamped at 1, no bullet, a world loading, no attacker after an event kept
  by another hit (not written);
- the markers: game_world::tick from 0x1F7FB4 to 0x1F7FFB (both event
  players, 0x1FB660 to the hud and 0x1FB6C0 to the kill marker, with their
  handlers 0x23B5F0 and 0x233770, the kill marker 0x23CF20 and the hit
  marker's draw 0x238230), frame after frame on a made-up world whose clock
  moves 50 ms a frame, its events pushed by push_world_event and taken back
  by the rollback's drop (0x2D3700) as move() does. Stubs record
  on_player_killed (0x237C70: a kill feed row), on_hit_from_pos (0x2372F0:
  the local player hit) and the movie's Invoke (0x511600:
  root.crosshair_enemy_hit, the marker). Scenarios: a kill predicted and
  taken back, a kill that stays, one made again three times, one made
  again at another tick, a hit taken back, a hit that stays, a hit made
  again with another sum, two hits, the same sum twice, someone else's hit,
  a hit on the local player, the marker option off, a hit and the kill at
  one tick. The build: a marker and a row exactly once for what stays, at
  the event horizon (500 ms after the tick), none for what was taken back;
  the original: the kill marker at the predicted tick, also when taken
  back; the rows and on_hit_from_pos alike in both.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import patch_026g as base

GENERATE_HIT = 0x1F3C70                    # player::generate_hit_event, `ret 1Ch`
PUSH, DROP = 0x2D0A60, 0x2D3700            # push_world_event (eax, edx; ret), drop_world_events_after (edx; ret 4)
TICK_FROM, TICK_TO = 0x1F7FB4, 0x1F7FFB    # game_world::tick: the two event players
ROW, FROM_POS, INVOKE = 0x237C70, 0x2372F0, 0x511600
OPTION = 0xA70EC6                          # show_enemy_killed_marker's byte
OBJECTS, STACK, CODE = 0x20000000, 0x21000000, 0x22000000
RETURN = CODE + 0xF00                      # the return address the runs stop at
REGS = (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP)
KEPT = (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)
# made-up objects, 0x1000 apart unless said
(VICTIM, ATTACKER, BULLET, SOURCE, POSITION, MODEL, HOLDER, GAME, LOCAL, POOL, GW, CORE_HOLDER,
 MOVIE_HOLDER, MOVIE_UI) = (OBJECTS + 0x1000 * i for i in range(14))
GW = OBJECTS + 0x80000                      # the game world: its fields reach +4D4Ch
CORE_HOLDER_SIZE = 0x20000                  # the core holder's fields reach +191BCh: its own pages
CORE_HOLDER = OBJECTS + 0x100000
CORE = CORE_HOLDER + 0x1F8
PLACES = OBJECTS + 0x40000                  # the core's places (+18458h), 30h each
PARTS = OBJECTS + 0x50000                   # the damage model's pools, 100h apart
NODES = OBJECTS + 0x60000                   # the events' pool, 30h apart
TEXTS = OBJECTS + 0x70000
EVENTS, LOADING, NOW, HORIZON, STATISTICS = 0x12740, 0x18FE0, 0x18FC4, 0x18FB4, 0x18FA8
COLLECTOR = 0xF534
LOCAL_PLACE, VICTIM_PLACE, OTHER_PLACE = 5, 3, 7


def f32(value):
    return struct.unpack("<I", struct.pack("<f", value))[0]


def from_f32(bits):
    return struct.unpack("<f", struct.pack("<I", bits))[0]


class Run:
    def __init__(self, data, hit_event=None):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(OBJECTS, 0x200000, UC_PROT_ALL)
        emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
        emu.mem_map(CODE, 0x1000, UC_PROT_ALL)
        self.hit_event = hit_event
        # the getters: the attacker by place (thiscall: &out, place; `ret 8`), the damage model's holder,
        # the attacker's position object
        emu.mem_write(CODE + 0x00, b"\x8b\x44\x24\x04" + b"\xc7\x00" + struct.pack("<I", ATTACKER) + b"\xc2\x08\x00")
        emu.mem_write(CODE + 0x20, b"\xb8" + struct.pack("<I", HOLDER) + b"\xc3")
        emu.mem_write(CODE + 0x40, b"\xb8" + struct.pack("<I", POSITION) + b"\xc3")
        emu.mem_write(RETURN, b"\xf4")
        emu.hook_add(UC_HOOK_CODE, self.stub, begin=ROW, end=ROW)
        emu.hook_add(UC_HOOK_CODE, self.stub, begin=FROM_POS, end=FROM_POS)
        emu.hook_add(UC_HOOK_CODE, self.stub, begin=INVOKE, end=INVOKE)
        self.seen = []

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at, size=4):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0] if size == 4 else bytes(self.emu.mem_read(at, size))

    def text(self, at):
        return bytes(self.emu.mem_read(at, 64)).split(b"\0")[0].decode()

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        if address == ROW:                         # on_player_killed(hud, victim, killer, icon, cause); ret 14h
            args = [self.read(sp + 4 * i) for i in range(1, 6)]
            self.seen.append(("row", args[1], args[2]))
            pops = 0x14
        elif address == FROM_POS:                  # on_hit_from_pos(ecx the bullet; hud, &position); ret 8
            self.seen.append(("hit_player", emu.reg_read(UC_X86_REG_ECX)))
            pops = 8
        else:                                      # Invoke(ecx movie; name, result, args, count); ret 10h
            name, _, args, count = (self.read(sp + 4 * i) for i in range(1, 5))
            values = []
            for i in range(count):
                kind = self.read(args + 0x18 * i + 4) & 0x8F
                if kind == 5:
                    values.append(round(struct.unpack("<d", bytes(emu.mem_read(args + 0x18 * i + 8, 8)))[0], 4))
                elif kind == 2:
                    values.append(bool(self.read(args + 0x18 * i + 8) & 0xFF))
                else:
                    values.append(self.read(args + 0x18 * i + 8))
            self.seen.append((self.text(name), values[0], values[2]) if count == 4 else (self.text(name), values))
            emu.reg_write(UC_X86_REG_EAX, 1)
            pops = 0x10
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def call(self, start, sp, regs, args=()):
        for i, value in enumerate(args):
            self.word(sp + 4 + 4 * i, value)
        self.word(sp, RETURN)
        for reg, value in regs.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.emu_start(start, RETURN, count=200000)
        return {reg: self.emu.reg_read(reg) for reg in REGS}, self.emu.reg_read(UC_X86_REG_ESP) - sp

    # -- the world: a core and its events' pool ------------------------------------------------------------

    def world(self):
        emu = self.emu
        emu.mem_write(OBJECTS, bytes(0x200000))
        self.word(CORE + EVENTS + 0x10, POOL)       # the events' pool: +20h the free nodes, +24h count, +28h room
        for i in range(64):
            self.word(NODES + 0x30 * i, NODES + 0x30 * (i + 1) if i < 63 else 0)
        self.word(POOL + 0x20, NODES)
        self.word(POOL + 0x28, 1000)
        self.word(CORE + HORIZON, 500)
        if self.hit_event is not None:
            self.word(self.hit_event, 0)

    def events(self):
        """The core's events in list order: (node, its 2Ch bytes)."""
        out, node = [], self.read(CORE + EVENTS + 4)
        while node and len(out) < 100:
            out.append((node, self.read(node, 0x2C)))
            node = self.read(node + 0x1C)
        return out

    def new_node(self):
        node = self.read(POOL + 0x20)
        self.word(POOL + 0x20, self.read(node))
        self.emu.mem_write(node, bytes(0x2C))
        return node

    def push(self, time, kind, first, second, word4=0, amount=0.0):
        """An event pushed by the game's push_world_event."""
        node = self.new_node()
        self.emu.mem_write(node, bytes([first, second]))
        self.word(node + 4, word4)
        self.word(node + 8, f32(amount))
        self.word(node + 0x18, kind)
        self.word(node + 0x24, time)
        self.emu.mem_write(node + 0x28, bytes([1 if kind == 2 else 0]))
        out, depth = self.call(PUSH, STACK + 0x8000, {UC_X86_REG_EAX: node, UC_X86_REG_EDX: CORE})
        assert depth == 4, depth

    def drop(self, time):
        """A rollback: every event after `time` back to the pool."""
        out, depth = self.call(DROP, STACK + 0x8000, {UC_X86_REG_EDX: CORE}, (time,))
        assert depth == 8, depth

    # -- the hit's event ---------------------------------------------------------------------------------

    def hit(self, collector, damage, bullet=True, loading=False, attacker_place=LOCAL_PLACE, stale=None):
        """generate_hit_event(time 1000, the attacker's place, the victim's 3, 0.5, damage, bullet, 9),
        whole. collector: (victim, time, sum) before. Out: (registers, depth, the events, the collector,
        the stale node's bytes, the arena's kept event after)."""
        self.world()
        emu = self.emu
        self.word(VICTIM + 0x120, CORE)
        self.word(VICTIM + 0xF478, GAME)
        self.word(VICTIM + 0x108, VICTIM + 0x800)           # the damage model holder's vtable
        self.word(VICTIM + 0x800 + 8, CODE + 0x20)
        self.word(HOLDER, MODEL)
        names = ("body", "pain", "head")
        for i, name in enumerate(names):
            node = PARTS + 0x100 * i
            emu.mem_write(TEXTS + 0x40 * i, name.encode() + b"\0")
            self.word(node, PARTS + 0x100 * (i + 1) if i + 1 < len(names) else 0)
            self.word(node + 0x80, TEXTS + 0x40 * i)
            self.word(node + 0x9C, f32(100.0))
            self.word(node + 0xA0, f32(60.0))
        self.word(MODEL + 0x108 + 8, PARTS)
        self.word(GAME + 0x53AC, GAME + 0x800)              # the game's player lookup: vtable +4Ch
        self.word(GAME + 0x800, GAME + 0x900)
        self.word(GAME + 0x900 + 0x4C, CODE + 0x00)
        place = PLACES + 0x30 * LOCAL_PLACE                 # the core's places: [+0Ch] which slot, the slots
        self.word(CORE + 0x18458, PLACES)
        emu.mem_write(place + 0xC, bytes([0]))
        self.word(place, ATTACKER)
        self.word(ATTACKER + 0x118, ATTACKER + 0x800)       # its position object's vtable: +28h
        self.word(ATTACKER + 0x800 + 0x28, CODE + 0x40)
        self.word(ATTACKER + 0x128, 0)                      # no weapon for a hit with no bullet
        self.word(ATTACKER + 0xD0, 9)                       # the reference count the lookup's release lowers
        self.word(POSITION + 0x30, f32(1.5))
        self.word(POSITION + 0x34, f32(2.5))
        self.word(POSITION + 0x38, f32(3.5))
        self.word(BULLET + 4, 77)                           # its id
        self.word(BULLET + 0xC, SOURCE)                     # its damage source: +254h 17h (no weapon row)
        self.word(SOURCE + 0x254, 0x17)
        self.word(BULLET + 0x40, 0x1234)
        self.word(BULLET + 0x58, 0x55)
        victim, time, amount = collector
        emu.mem_write(ATTACKER + COLLECTOR, bytes([victim]))
        self.word(ATTACKER + COLLECTOR + 4, time)
        self.word(ATTACKER + COLLECTOR + 8, f32(amount))
        emu.mem_write(CORE + LOADING, bytes([1 if loading else 0]))
        if stale is not None and self.hit_event is not None:
            self.word(self.hit_event, stale)
            emu.mem_write(stale, bytes(0x2C))
            emu.mem_write(stale + 1, bytes([VICTIM_PLACE]))
            self.word(stale + 0x24, 950)
        sp = STACK + 0x8000
        emu.mem_write(STACK, bytes(0x10000))
        regs = {UC_X86_REG_EAX: 0x0A0A0A0A, UC_X86_REG_ECX: VICTIM, UC_X86_REG_EDX: 0x0D0D0D0D,
                UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_ESI: 0x05050505, UC_X86_REG_EDI: 0x0E0E0E0E,
                UC_X86_REG_EBP: 0x19FF00}
        out, depth = self.call(GENERATE_HIT, sp, regs, (1000, attacker_place, VICTIM_PLACE, f32(0.5), f32(damage),
                                                       BULLET if bullet else 0, 9))
        kept = self.read(self.hit_event) if self.hit_event is not None else 0
        return out, depth, [bytes(b) for _, b in self.events()], \
            (self.read(ATTACKER + COLLECTOR, 1)[0], self.read(ATTACKER + COLLECTOR + 4),
             round(from_f32(self.read(ATTACKER + COLLECTOR + 8)), 4)), \
            self.read(stale, 0x2C) if stale is not None else None, kept, bytes(self.emu.mem_read(ATTACKER + COLLECTOR + 8, 4))

    # -- the markers, frame by frame -------------------------------------------------------------------

    def match(self, option=True):
        self.world()
        ui = GW + 0x1FC
        self.word(GW + 0x4D4C, CORE_HOLDER)                 # the core holder: core = +1F8h
        self.word(GW + 0x9C, GAME)
        self.word(GAME + 0x53AC, GAME + 0x800)
        self.word(GAME + 0x800 + 4, LOCAL)                  # the local player
        self.emu.mem_write(LOCAL + 0x114, bytes([LOCAL_PLACE]))
        self.word(LOCAL + 0xD0, 1000)
        self.word(ui + 0x18, GW)                            # the hud's game world
        self.word(ui + 0x8, MOVIE_HOLDER)                   # the hud's movie: [[ui+8]+108h]+4
        self.word(MOVIE_HOLDER + 0x108, MOVIE_UI)
        self.word(MOVIE_UI + 4, 0x3000000)
        self.word(GW + 0x560, 0)                            # ui+364h: nothing writes it
        self.word(GW + 0x568, GW)                           # the kill marker's owner (ui+368h): +4 the game world
        self.word(ui + 0x340, 0xFFFFFFFF)
        self.emu.mem_write(OPTION, bytes([1 if option else 0]))
        self.seen = []

    def frame(self, now):
        """game_world::tick's two event players at the clock `now`: what they showed."""
        self.word(CORE + NOW, now)
        before = len(self.seen)
        sp = STACK + 0x8000
        regs = {UC_X86_REG_EBX: GW, UC_X86_REG_EBP: 0x19FF00, UC_X86_REG_ESI: 0, UC_X86_REG_EDI: 0}
        for reg, value in regs.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.emu_start(TICK_FROM, TICK_TO, count=200000)
        assert self.emu.reg_read(UC_X86_REG_ESP) == sp
        return [(now,) + what for what in self.seen[before:]]


KILL, HIT = 0, 2
MARKER = "root.crosshair_enemy_hit"


def scenario(run, steps, option=True, last=1700):
    """steps: {now: [("kill", T, victim, killer) | ("hit", T, attacker, victim, sum) | ("drop", T)]}, done
    before that frame. All that was shown, frame by frame."""
    run.match(option)
    shown = []
    for now in range(1000, last + 1, 50):
        for step in steps.get(now, ()):
            if step[0] == "kill":
                run.push(step[1], KILL, step[2], step[3], word4=1)
            elif step[0] == "hit":
                run.push(step[1], HIT, step[2], step[3], word4=40 + step[1] // 50, amount=step[4])
            else:
                run.drop(step[1])
        shown += run.frame(now)
    return shown


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    linked = base.arena()
    stock, patched = Run(original), Run(bytes(built), linked.symbols["hit_event"])
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    print("the hit's event (player::generate_hit_event 0x1F3C70, whole, with base_player's 0x2D9F40)")
    cases = (
        ("a first hit: 30 damage of a 100 pain pool", (0xFF, 0, 0.0), 30.0, {}, 0.3),
        ("a second hit on the same victim", (VICTIM_PLACE, 950, 0.3), 30.0, {}, 0.6),
        ("a hit on another victim: the sum starts again", (4, 950, 0.5), 30.0, {}, 0.3),
        ("a sum clamped at 1", (VICTIM_PLACE, 950, 0.9), 30.0, {}, 1.0),
        ("no bullet: no sum", (0xFF, 0, 0.0), 30.0, {"bullet": False}, None),
        ("a world loading: the event freed, nothing kept", (0xFF, 0, 0.0), 30.0, {"loading": True}, None),
    )
    for what, collector, damage, extra, want in cases:
        a = stock.hit(collector, damage, **extra)
        b = patched.hit(collector, damage, **extra)
        events_a, events_b = a[2], b[2]
        same = len(events_a) == len(events_b) == (0 if extra.get("loading") else 1)
        if same and events_a:
            node = bytearray(events_a[0])
            if want is not None:                       # the collector's own float, bit for bit
                node[8:12] = b[6]
            same = events_b[0] == bytes(node) and events_a[0][8:12] == bytes(4) \
                and events_a[0][0] == LOCAL_PLACE and events_a[0][1] == VICTIM_PLACE \
                and struct.unpack_from("<I", events_a[0], 0x24)[0] == 1000 and events_a[0][0x18] == HIT
        ok = same and a[3] == b[3] and all(a[0][r] == b[0][r] for r in KEPT) and a[1] == b[1] == 4 + 0x1C \
            and b[5] == 0 and (want is None or abs(a[3][2] - want) < 1e-5)
        report(ok, f"{what}: the event's +8 "
                   + (f"{from_f32(struct.unpack_from('<I', events_b[0], 8)[0]):.4f}" if events_b else "(none)")
                   + f", the collector {a[3]}; the rest the original's")
    stale = NODES + 0x30 * 60
    a = stock.hit((0xFF, 0, 0.0), 30.0, attacker_place=0xFF, stale=stale)
    b = patched.hit((0xFF, 0, 0.0), 30.0, attacker_place=0xFF, stale=stale)
    report(a[2] == b[2] == [] and b[4][8:12] == bytes(4) and b[5] == 0 and a[3] == b[3]
           and all(a[0][r] == b[0][r] for r in KEPT) and a[1] == b[1],
           "no attacker: no event; an event kept from another hit (another time) not written, the kept one let go")

    print("the markers (game_world::tick 0x1F7FB4..0x1F7FFB frame after frame, the clock 50 ms a frame)")
    V, L, O = VICTIM_PLACE, LOCAL_PLACE, OTHER_PLACE
    marker = lambda now, value, killed: (now, MARKER, value, killed)
    scenarios = (
        ("a kill predicted, then taken back by a rollback",
         {1000: [("kill", 1000, V, L)], 1050: [("drop", 950)]}, {},
         [], [marker(1000, 1.0, True)]),
        ("a kill that stays",
         {1000: [("kill", 1000, V, L)]}, {},
         [(1500, "row", V, L), marker(1500, 1.0, True)], [marker(1000, 1.0, True), (1500, "row", V, L)]),
        ("a kill made again by three re-simulations",
         {1000: [("kill", 1000, V, L)], 1050: [("drop", 950), ("kill", 1000, V, L)],
          1100: [("drop", 950), ("kill", 1000, V, L)], 1150: [("drop", 950), ("kill", 1000, V, L)]}, {},
         [(1500, "row", V, L), marker(1500, 1.0, True)], [marker(1000, 1.0, True), (1500, "row", V, L)]),
        ("a kill a re-simulation moves to another tick",
         {1000: [("kill", 1000, V, L)], 1100: [("drop", 950), ("kill", 1100, V, L)]}, {},
         [(1600, "row", V, L), marker(1600, 1.0, True)],
         [marker(1000, 1.0, True), marker(1100, 1.0, True), (1600, "row", V, L)]),
        ("a hit predicted, then taken back",
         {1000: [("hit", 1000, L, V, 0.3)], 1050: [("drop", 950)]}, {},
         [], []),
        ("a hit that stays",
         {1000: [("hit", 1000, L, V, 0.3)]}, {},
         [marker(1500, 0.3, False)], []),
        ("a hit a re-simulation gives another sum",
         {1000: [("hit", 1000, L, V, 0.3)], 1050: [("drop", 950), ("hit", 1000, L, V, 0.45)]}, {},
         [marker(1500, 0.45, False)], []),
        ("two hits on the same victim",
         {1000: [("hit", 1000, L, V, 0.3)], 1050: [("hit", 1050, L, V, 0.6)]}, {},
         [marker(1500, 0.3, False), marker(1550, 0.6, False)], []),
        ("the same sum twice (clamped at 1)",
         {1000: [("hit", 1000, L, V, 1.0)], 1050: [("hit", 1050, L, V, 1.0)]}, {},
         [marker(1500, 1.0, False), marker(1550, 1.0, False)], []),
        ("someone else's hit and kill",
         {1000: [("hit", 1000, O, V, 0.3), ("kill", 1000, V, O)]}, {},
         [(1500, "row", V, O)], [(1500, "row", V, O)]),
        ("the marker option off",
         {1000: [("hit", 1000, L, V, 0.3), ("kill", 1000, V, L)]}, {"option": False},
         [(1500, "row", V, L), marker(1500, 0.3, False)], [(1500, "row", V, L)]),
        ("a hit and the kill at one tick",
         {1000: [("hit", 1000, L, V, 0.8), ("kill", 1000, V, L)]}, {},
         [(1500, "row", V, L), marker(1500, 0.8, False), marker(1500, 1.0, True)],
         [marker(1000, 1.0, True), (1500, "row", V, L)]),
    )
    for what, steps, extra, want_built, want_stock in scenarios:
        a = scenario(stock, steps, **extra)
        b = scenario(patched, steps, **extra)
        ok = b == want_built and a == want_stock
        report(ok, f"{what}: the build {b or 'nothing'}" + ("" if ok else f" (want {want_built}); "
                                                                         f"the original {a} (want {want_stock})"))
    # A hit on the local player: on_hit_from_pos played every frame until confirmed, alike in both; no marker.
    steps = {1000: [("hit", 1000, V, L, 0.3)]}
    a, b = scenario(stock, steps), scenario(patched, steps)
    report(a == b and a and all(what[1] == "hit_player" for what in a),
           f"a hit on the local player: on_hit_from_pos {len(b)} times (frames 1000 to 1500), as the original; "
           "no marker")
    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
