#!/usr/bin/env python3
"""Run patch 53 (Research, mode 5: research_026g.asm, research_native_026g.c,
research_rule_026g.c) in the default build on made-up games, worlds and huds.

    python check_research_026g.py <original survarium.exe>

Requires Python `unicorn` (and the research-mode scripts' bcw.py beside, for a
mode config). The built exe's sections are loaded as they are and its code
runs as machine code; the game's own code runs where the patch calls it: the
cook table's register_cook (0x102D10), game_match_rule_base's constructor
(0x313900), the config's look-ups (0xF6B10, 0xF6C00, the CRC table they build)
and the hooks' sites. Intercepted (answered here): the allocator (0xE4F90),
GetCurrentThreadId, the call the constructor's hook replaces (0x1EC170), a
buffer's resize (0x7AB30, under the game's field writers), the movie's Invoke (0x511600), the hud's message
(0x2375C0) and update_player_icons (0x23AA10, under update_ui's call our hook takes). The game's own loops over a world's rules (game_world_core's
serialize and deserialize, 0x2D2661 and 0x2D2B42) run with the rule in a
world's rules, so a slot that pops other than the game expects shows as esp
moved. What each case checks is printed.
"""
import os
import struct
import sys

from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import patch_026g as base

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "scripts", "research-mode"))
import bcw  # noqa: E402

# Research's code and state as the build links them (link_026g.py).
ARENA = base.arena()
SYMBOLS = {name: va for name, va in ARENA.symbols.items() if name.startswith(("rn_", "rr_"))}
VA, STATE = ARENA.blocks["research.code"].va, ARENA.blocks["research.state"].va
HEAP, STACK, SENTINEL, CORE, CONFIG, FAKE = 0x30000000, 0x21000000, 0x22000000, 0x40000000, 0x50000000, 0x60000000
STOP = SENTINEL + 0x10
COOKS, REGISTERED, THREAD_ID = 0xA86E00, STATE + 0x24, 0x870118
ALLOCATE, CTOR_CALL, RESIZE, INVOKE, MESSAGE = 0xE4F90, 0x1EC170, 0x7AB30, 0x511600, 0x2375C0
PLAYER_ICONS = 0x23AA10                                # game_world_ui::update_player_icons(ui), ret 4
# the field writers' hash mismatch dump: its switches, its record's allocation, type_info::name
DUMP_ON, DUMP_RECORD, TYPE_NAME = 0x1F1D32C, 0x80450, 0x420724
# [core+18FA8h] is the statistics handler; the shared statistics at +8 in it:
# the players' records (u16 events[28][3] first, 1F8h each) and the listener
# (shared +31A0h), called through 0x2F5D40 (place, event, amount)
EVENTS_AT, LISTENER, NOTIFY = 0x8, 0x31A8, 0x2F5D40
# the handler's recalculate_scores (EDI the handler): m_scores (u16, +31F0h)
# from the records, for the places under +3240h, by the weights at
# [+323Ch] + 577BCh
RECALCULATE, SCORES, SCORE_COUNT, OPTIONS, WEIGHTS = 0x26DC30, 0x31F0, 0x3240, 0x323C, 0x577BC
# rn_rule (research_native_026g.c's _Static_asserts)
R_CORE, R_VALID, R_PARAMS, R_STATE, R_SIZE = 0x128, 0x134, 0x138, 0x7E0, 0x918
P_ZONES, ZONE_DEF = 0x64, 0x214
S_ZONES, ZONE = 0x18, 0x60
FULL = 1000000000
# the world
PLACES, TICK_LIST, TICK_ORDER, STATISTICS, MATCH_STATE, LOCAL_PLACE, RULES = \
    0x18458, 0x18850, 0x18FB0, 0x18FA8, 0x18FE4, 0x18FDC, 0x18824
PLAYER, PROFILE = 0x800, 0x4600


def config_bytes():
    """A mode config as make_pack.py makes them: 0.27d's parameters, three units."""
    def box(x, half):
        return {"type": 1, "position": (x, 0.0, 0.0), "axis_x": (1.0, 0.0, 0.0), "axis_y": (0.0, 1.0, 0.0),
                "axis_z": (0.0, 0.0, 1.0), "half": half}
    units = [{"cp_id": i, "position": (100.0 * i, 0.0, 10.0), "shapes": [box(100.0 * i, (5.0, 2.0, 5.0))]}
             for i in range(3)]
    return bcw.dump({"mode": {
        "parameters": {"match_time": 900, "wait_time": 5, "countdown_time": 20, "wait_player_percent": 0.8,
                       "respawn_time": 10, "resources_count": 100, "restore_multiplier": 0.5,
                       "capture_time_in_s": [20, 18, 15, 14.9, 14.8, 14.7, 14.6, 14.5],
                       "resource_per_s": [0.075, 0.15, 0.35], "capture_zones": units, "capture_award": 100,
                       "capture_reach": 0.38, "capture_half_height": 0.9},
        "ui": {"loading_screen_id": 0, "icon": 1, "mode_name": "st_mode_05_name"},
        "name": "level_01_day_5", "rules": [93, 91, base.RESEARCH_CLASS, 94], "mode": 5}})


def loaded(data, at):
    """The config as the game holds it: every offset of the file a pointer."""
    out = bytearray(data)
    record = struct.Struct("<QQIHH")

    def fix(offset):
        payload, name, crc, kind, count = record.unpack_from(out, offset)
        if kind in (3, 4, 5, 6, 7, 8):
            payload = at + payload
        record.pack_into(out, offset, payload, at + name if name else 0, crc, kind, count)
        if kind in (3, 4):
            for i in range(count):
                fix(payload - at + 24 * i)
    fix(0)
    return bytes(out)


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((HEAP, 0x1000000), (STACK, 0x10000), (SENTINEL, 0x1000), (CORE, 0x20000),
                         (CONFIG, 0x10000), (FAKE, 0x100000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        self.heap = HEAP
        self.invokes, self.messages, self.stopped, self.notified = [], [], None, []
        self.player_icons = []
        # GetCurrentThreadId: a stub returning 77h
        emu.mem_write(SENTINEL + 0x100, bytes.fromhex("b877000000c3"))
        self.word(THREAD_ID, SENTINEL + 0x100)
        emu.hook_add(UC_HOOK_CODE, self.intercept)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at, size=4):
        return struct.unpack({1: "<B", 2: "<H", 4: "<I"}[size], self.emu.mem_read(at, size))[0]

    def allocate(self, size):
        at = self.heap
        self.heap += (size + 0xF) & ~0xF
        self.emu.mem_write(at, bytes(size))
        return at

    def ret(self, value, cleanup):
        emu = self.emu
        sp = emu.reg_read(UC_X86_REG_ESP)
        back = self.read(sp)
        emu.reg_write(UC_X86_REG_EAX, value)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + cleanup)
        emu.reg_write(UC_X86_REG_EIP, back)

    def intercept(self, emu, address, size, user):
        sp = emu.reg_read(UC_X86_REG_ESP)
        if address == ALLOCATE:
            self.ret(self.allocate(emu.reg_read(UC_X86_REG_EAX)), 4)
        elif address == CTOR_CALL:
            self.stopped = ("constructor", emu.reg_read(UC_X86_REG_EAX), self.read(sp))
            emu.emu_stop()
        elif address == RESIZE:
            buffer, size = emu.reg_read(UC_X86_REG_EAX), self.read(sp + 4)
            if size > self.read(buffer + 8):
                fresh = self.allocate(size + 0x100)
                old = self.read(buffer + 0xC)
                emu.mem_write(fresh, bytes(emu.mem_read(self.read(buffer + 4), old)))
                self.word(buffer + 4, fresh)
                self.word(buffer + 8, size + 0x100)
            self.word(buffer + 0xC, size)
            self.ret(0, 4)
        elif address == INVOKE:
            name = self.string(self.read(sp + 4))
            args, count = self.read(sp + 0xC), self.read(sp + 0x10)
            values = []
            for i in range(count):
                kind = self.read(args + 0x18 * i + 4)
                values.append(struct.unpack("<d", emu.mem_read(args + 0x18 * i + 8, 8))[0] if kind == 5 else None)
            self.invokes.append((emu.reg_read(UC_X86_REG_ECX), name, self.read(sp + 8), values))
            self.ret(1, 0x10)
        elif address == PLAYER_ICONS:
            self.player_icons.append(self.read(sp + 4))
            self.ret(0, 4)
        elif address == DUMP_RECORD:
            self.ret(self.allocate(0x20), 0)
        elif address == TYPE_NAME:
            self.ret(SENTINEL + 0x200, 4)
        elif address == NOTIFY:
            self.notified.append((emu.reg_read(UC_X86_REG_EAX), self.read(sp + 4), self.read(sp + 8),
                                  self.read(sp + 0xC)))
            self.ret(0, 0xC)
        elif address == MESSAGE:
            self.messages.append((self.string(emu.reg_read(UC_X86_REG_ECX)), self.read(sp + 4), self.read(sp + 8),
                                  self.read(sp + 0xC), self.read(sp + 0x10)))
            self.ret(0, 0x10)

    def string(self, at):
        out = bytearray()
        while True:
            b = self.emu.mem_read(at + len(out), 1)[0]
            if not b:
                return out.decode()
            out.append(b)

    def call(self, function, args=(), regs=None, start=None, until=STOP):
        """`function`(args), pushed right to left, returning to STOP."""
        emu = self.emu
        sp = STACK + 0x8000 - 4 * len(args) - 4
        self.word(sp, STOP)
        for i, value in enumerate(args):
            self.word(sp + 4 + 4 * i, value)
        emu.reg_write(UC_X86_REG_ESP, sp)
        for reg, value in (regs or {}).items():
            emu.reg_write(reg, value)
        self.stopped = None
        emu.emu_start(start or function, until, count=5_000_000)
        return emu.reg_read(UC_X86_REG_EAX), emu.reg_read(UC_X86_REG_ESP) - sp - 4

    def thiscall(self, function, this, *args):
        return self.call(function, args, {UC_X86_REG_ECX: this})

    def span(self, start, until, regs, stack=()):
        """The game's own code from `start` to `until` with esp at a frame
        of its own (`stack`: dwords at esp, esp+4, ...); (eip, esp moved by)."""
        emu = self.emu
        sp = STACK + 0x6000
        for i, value in enumerate(stack):
            self.word(sp + 4 * i, value)
        emu.reg_write(UC_X86_REG_ESP, sp)
        for reg, value in regs.items():
            emu.reg_write(reg, value)
        self.stopped = None
        emu.emu_start(start, until, count=5_000_000)
        return emu.reg_read(UC_X86_REG_EIP), emu.reg_read(UC_X86_REG_ESP) - sp


def vtable(run, obj, slot):
    return run.read(run.read(obj) + 4 * slot)


def world(run):
    """A world of twenty places, kit 0 each, in play; its statistics."""
    run.emu.mem_write(CORE, bytes(0x20000))
    places, statistics, counter = run.allocate(20 * 0x30), run.allocate(0x3250), run.allocate(4)
    run.word(CORE + PLACES, places)
    run.word(CORE + STATISTICS, statistics)
    run.word(CORE + TICK_ORDER, counter)
    run.word(counter, 7)
    run.word(CORE + MATCH_STATE, 3)
    players = []
    for i in range(20):
        player, profile = run.allocate(PLAYER), run.allocate(PROFILE)
        run.word(places + 0x30 * i, player)
        run.word(player + 0x6B8, profile)
        run.word(profile + 0x45EC, i % 2)
        run.emu.mem_write(player + 0x33C, bytes([1, 0, 0, 1]))
        run.emu.mem_write(player + 0x43C, struct.pack("<3f", 1000.0, 0.0, 0.0))
        players.append(player)
    return players, statistics


def put(run, player, x, y=0.9, z=0.0):
    run.emu.mem_write(player + 0x43C, struct.pack("<3f", x, y, z))


def zone(run, rule, z):
    at = rule + R_STATE + S_ZONES + ZONE * z
    return {"progress": run.read(at), "capturing": run.read(at + 4, 1), "owner": run.read(at + 5, 1),
            "count": (run.read(at + 6, 1), run.read(at + 7, 1))}


def driven(run, cook, query, check):
    """Each slot of ours, the tick and the cook's create and size, reached
    through the game's own call site with what the game passes there, so a
    slot that takes its arguments in another order shows (patch_026g.py's
    INSTALLED_VTABLES and CALLBACKS name these cases: the self-test fails a
    slot of ours with none). Serialize and deserialize: section 6b."""
    print("  10. through the game's own call sites")
    players, statistics = world(run)
    run.word(statistics + LISTENER, 1)                     # the statistics' event listener is set
    run.word(CORE + 0x18FC0, 0x2A000)                      # the world's time: [core+18FC0h]
    # the cook: translate (0x2F4290) calls create with the query it made
    # (0x2F42B8..0x2F42C2) and then size (0x2F42C6..0x2F42CD)
    run.emu.reg_write(UC_X86_REG_ESI, 0)
    cached = run.read(STATE + 0x28)                        # rn_globals.hud.rule: section 8's rule
    eip, moved = run.span(0x2F42B8, 0x2F42CF, {UC_X86_REG_EBX: cook, UC_X86_REG_EDI: query})
    rule, size = run.emu.reg_read(UC_X86_REG_ESI), run.emu.reg_read(UC_X86_REG_EAX)
    check("driven: the cook's create and size through translate 0x2F4290's calls (0x2F42B8..0x2F42CF)",
          eip == 0x2F42CF and moved == 0 and size == R_SIZE and run.read(rule) == SYMBOLS["rn_rule_vtable"] + 4
          and run.read(rule + R_VALID) == 1, f"esp moved {moved}, size {size:#x}")
    check("a new rule clears the hud's cache of the last one (a rule made where the last one was has no stale "
          "owners: no \"Unit A lost\" at a match's first frame)", cached != 0 and run.read(STATE + 0x28) == 0)
    rules = run.allocate(4)
    run.word(rules, rule)
    run.word(CORE + RULES, rules)
    run.word(CORE + RULES + 4, rules + 4)
    eip, moved = run.span(0x2D3B2B, 0x2D3B51, {UC_X86_REG_EBP: CORE})
    check("driven: slot 11, set_game_world_core, through the world's loop 0x2D3B40",
          eip == 0x2D3B51 and moved == 0 and run.read(rule + R_CORE) == CORE
          and run.read(CORE + TICK_LIST + 4) == rule + 0x110, f"esp moved {moved}")
    eip, moved = run.span(0x2D58F0, 0x2D5917, {UC_X86_REG_EDI: CORE})
    check("driven: slot 12, the blackboard, through the world's loop 0x2D5906",
          eip == 0x2D5917 and moved == 0 and run.read(rule + R_CORE + 4) == CORE + 0x188B0, f"esp moved {moved}")
    ok = True
    for place in (0, 1, 2):
        eip, moved = run.span(0x2D1FFF, 0x2D202A, {UC_X86_REG_EDI: CORE}, [0] * 8 + [place, 0x2A000])
        ok = ok and eip == 0x2D202A and moved == 0
    check("driven: slot 8, entered(place, time), through the world's loop 0x2D2010",
          ok and run.read(rule + R_STATE + 0x10) == 0b111)
    # the tick: the second tick list's runner calls the delegate with
    # (dt, time, step), as atomic_tick 0x2D1E20 has it call 0x2D1D20
    put(run, players[0], 1.0)
    time = 0x2A000
    for _ in range(400):
        time += 50
        run.call(0x2D1D20, args=(CORE + TICK_LIST, 50, time, 1))
    a = zone(run, rule, 0)
    check("driven: the tick (the delegate, as timelimit's) through the tick list's runner 0x2D1D20",
          a["owner"] == 0 and a["progress"] == FULL and run.emu.reg_read(UC_X86_REG_ESP) == STACK + 0x8000,
          str(a))
    # The tick's (dt, time) in their places: one tick that reaches the goal
    # sets the end at the time it was given + 3000 (rr_tick clamps dt, so a
    # count alone cannot tell the two apart); with the two swapped by a shim
    # in the delegate, the end is wrong.
    def goal_tick(function=None):
        state = bytes(run.emu.mem_read(rule + R_STATE, 0x18 + 3 * ZONE))
        delegate = rule + 0x110 + 4
        real = run.read(delegate)
        if function is not None:
            run.word(delegate, function)
        for z in range(3):
            at = rule + R_STATE + S_ZONES + ZONE * z
            run.word(at, FULL)
            run.emu.mem_write(at + 4, bytes([0, 0]))
        run.word(rule + R_STATE, 100 * 1000000 - 1)
        run.word(rule + R_STATE + 12, 0xFFFFFFFF)
        run.call(0x2D1D20, args=(CORE + TICK_LIST, 50, 0x2B000, 1))
        end = run.read(rule + R_STATE + 12)
        run.word(delegate, real)
        run.emu.mem_write(rule + R_STATE, state)
        return end
    shim_tick = run.allocate(0x20)
    run.emu.mem_write(shim_tick, bytes.fromhex("8b442404874424088944240" "4") + b"\xE9"
                      + struct.pack("<i", SYMBOLS["rn_tick"] - (shim_tick + 12 + 5)))
    end_right, end_swapped = goal_tick(), goal_tick(shim_tick)
    check("driven: the tick's (dt, time) through the tick list's runner 0x2D1D20: the goal's end at the time "
          "given + 3000", end_right == 0x2B000 + 3000, f"end {end_right:#x}")
    check("... with dt and time swapped (a shim in the delegate) the end is not that", end_swapped != 0x2B000 + 3000,
          f"end {end_swapped:#x}")
    # a kill through the dispatcher 0x2D1EA0, as the kill site 0x2D9EEE gives
    # it (1, &{killer, victim}): team 1's place 1 inside A, which team 0 owns
    put(run, players[1], 2.0)
    time += 50
    run.call(0x2D1D20, args=(CORE + TICK_LIST, 50, time, 1))
    data = run.allocate(4)
    run.emu.mem_write(data, bytes([0, 1]))

    def kill(slot7=None):
        if slot7 is not None:
            run.emu.mem_write(rule, struct.pack("<I", slot7))
        before = (run.read(statistics + EVENTS_AT + 6 * 10, 2), len(run.notified))
        try:
            run.call(0x2D1EA0, args=(1, data), regs={UC_X86_REG_EDI: CORE})
            fine = run.emu.reg_read(UC_X86_REG_ESP) == STACK + 0x8000
        except UcError:
            fine = False
        return fine and run.read(statistics + EVENTS_AT + 6 * 10, 2) == before[0] + 1 \
            and run.notified[before[1]:] == [(statistics + LISTENER, 0, 10, 1)]
    good = kill()
    check("driven: slot 7, on_event(type, data, time), through the dispatcher 0x2D1EA0: kill invader for "
          "place 0, the statistics' listener told (0, event 10, 1)", good, str(run.notified[-1:]))
    # the first build's order, (type, time, data): a shim that swaps the
    # last two arguments before our slot 7, through the same dispatcher
    vtable_copy = run.allocate(0x60)
    run.emu.mem_write(vtable_copy, bytes(run.emu.mem_read(run.read(rule), 0x50)))
    shim = run.allocate(0x20)
    target = vtable(run, rule, 7)
    run.emu.mem_write(shim, bytes.fromhex("8b44240887442 40c89442408".replace(" ", "")) + b"\xE9"
                      + struct.pack("<i", target - (shim + 12 + 5)))
    run.word(vtable_copy + 4 * 7, shim)
    real = run.read(rule)
    bad = kill(vtable_copy)
    run.word(rule, real)
    check("... the first build's order (type, time, data) fails the same case", not bad)
    # A late joiner's first world: the time rule's deserialize sent event 0
    # (publish: the fresh rule's 0 / 0, a draw); our deserialize, through
    # the world's loop, leaves the loaded state's points in the tail.
    writer, buffer = CORE + 0x1F000, run.allocate(0x10)
    run.emu.mem_write(writer, bytes(0x20))
    run.word(writer + 0x14, buffer)
    run.word(buffer + 4, run.allocate(0x10))
    run.word(buffer + 8, 0x10)
    run.word(buffer + 0xC, 0)
    run.word(rule + R_STATE + 8, 41 | 7 << 16)              # shown 41 / 7
    run.span(0x2D2661, 0x2D2688, {UC_X86_REG_EBP: CORE, UC_X86_REG_ESI: CORE + 0x1F000 - 0xB7EF0})
    blob = bytes(run.emu.mem_read(run.read(buffer + 4), run.read(buffer + 0xC)))
    run.emu.mem_write(statistics + 0x31D4, bytes(4))
    run.word(statistics + 0x31E4, 2)
    source = run.allocate(512)
    run.emu.mem_write(source, blob)
    snapshot2 = run.allocate(0xB7F00)
    run.word(snapshot2 + 0xB7EE4, source)
    run.word(snapshot2 + 0xB7EEC, source + len(blob))
    eip, moved = run.span(0x2D2B1A, 0x2D2B5F, {UC_X86_REG_EBP: CORE, UC_X86_REG_EBX: snapshot2},
                          [0] * 0x21 + [0x7E57])
    check("a loaded world's statistics tail is its state's: 41 / 7, team 0 leads (deserialize publishes, after "
          "the time rule's event 0)", eip == 0x2D2B5F and moved == 0 and run.read(statistics + 0x31D4, 2) == 41
          and run.read(statistics + 0x31D6, 2) == 7 and run.read(statistics + 0x31E4) == 0)
    eip, moved = run.span(0x2D2119, 0x2D214A, {UC_X86_REG_EBP: CORE}, [0] * 8 + [2, time])
    check("driven: slot 9, left(place, time), through the world's loop 0x2D2130",
          eip == 0x2D214A and moved == 0 and run.read(rule + R_STATE + 0x10) == 0b011, f"esp moved {moved}")
    hud = run.allocate(0x100)
    run.emu.mem_write(hud + 0x90, b"\xff\xff\x00\x00\xff\xff")
    _, moved = run.call(0x2D36B0, args=(CORE, hud, 0), until=0x2D36E2)
    check("driven: slot 17, fill_match_hud_state(hud, ?), through the hud's loop 0x2D36D0",
          moved == -20 and run.read(hud + 0x90, 2) == run.read(rule + R_STATE + 8, 2)
          and run.read(hud + 0x94, 2) == 0xFFFF, f"esp {moved}")
    snapshot = run.allocate(0xC0C00)
    run.word(rule + R_STATE + 12, 0x2A000 + 3000)
    run.word(CORE + 0x18FC0, 0x2A000 + 2999)
    early, moved_early = run.span(0x2D271D, 0x2D274B, {UC_X86_REG_EBP: CORE, UC_X86_REG_ESI: snapshot})
    run.word(CORE + 0x18FC0, 0x2A000 + 3000)
    late, moved_late = run.span(0x2D271D, 0x2D2753, {UC_X86_REG_EBP: CORE, UC_X86_REG_ESI: snapshot})
    run.word(rule + R_STATE + 12, 0xFFFFFFFF)
    check("driven: slot 18, rule_completed(time), through the world's loop 0x2D2730: on at the end time",
          early == 0x2D274B and late == 0x2D2753 and moved_early == 0 and moved_late == 0)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    run, stock = Run(bytes(built)), Run(original)
    results = []

    def check(what, ok, detail=""):
        results.append(ok)
        print(f"  {'ok' if ok else 'FAIL'}  {what}" + (f": {detail}" if detail else ""))

    # 1. The game's constructor registers the cook, once; the replaced call goes on.
    table = run.allocate(0x206 * 4)
    run.word(COOKS, table)
    run.word(COOKS + 4, table + 0x206 * 4)
    game = run.allocate(0x100)
    run.call(0x1E9B4C, regs={UC_X86_REG_EBX: game}, until=STOP)
    cook = STATE
    check("the constructor's hook registers the cook at its class",
          run.stopped == ("constructor", game, 0x1E9B53) and run.read(table + 4 * base.RESEARCH_CLASS) == cook
          and run.read(cook) == SYMBOLS["rn_cook_vtable"] + 4 and run.read(cook + 8) == base.RESEARCH_CLASS
          and run.read(cook + 0x14) == 0x77 and run.read(cook + 0x20) == game and run.read(REGISTERED) == 1,
          f"stopped {run.stopped}")
    run.word(table + 4 * base.RESEARCH_CLASS, 0)
    run.call(0x1E9B4C, regs={UC_X86_REG_EBX: game}, until=STOP)
    check("a second construction registers nothing", run.read(table + 4 * base.RESEARCH_CLASS) == 0)
    taken = Run(bytes(built))
    other = taken.allocate(0x206 * 4)
    taken.word(COOKS, other)
    taken.word(COOKS + 4, other + 0x206 * 4)
    taken.word(other + 4 * base.RESEARCH_CLASS, 0x12345678)
    taken.call(0x1E9B4C, regs={UC_X86_REG_EBX: game}, until=STOP)
    check("a taken class is left to its cook (and Research not registered)",
          taken.read(other + 4 * base.RESEARCH_CLASS) == 0x12345678 and taken.read(REGISTERED) == 2
          and taken.stopped == ("constructor", game, 0x1E9B53))
    stock_table = stock.allocate(0x206 * 4)
    stock.word(COOKS, stock_table)
    stock.word(COOKS + 4, stock_table + 0x206 * 4)
    stock.emu.mem_write(0x1EC170, b"\xC3")
    stock.call(0x1E9B4C, regs={UC_X86_REG_EBX: game}, until=0x1E9B53)
    check("the original at the site: mov eax,ebx and the call, as the hook does",
          stock.emu.reg_read(UC_X86_REG_EAX) == game)

    # 2. The cook makes the rule from the mode config.
    config = config_bytes()
    run.emu.mem_write(CONFIG, loaded(config, CONFIG))
    query = run.allocate(0x10)
    options = run.allocate(0x60000)
    run.word(query, options)
    run.word(query + 0xC, CONFIG)
    rule, cleanup = run.thiscall(vtable(run, cook, 9), cook, query)
    size, _ = run.thiscall(vtable(run, cook, 10), cook)
    params = rule + R_PARAMS
    check("the cook's create makes a rule of type 5 with the config's parameters",
          cleanup == 4 and size == R_SIZE and run.read(rule) == SYMBOLS["rn_rule_vtable"] + 4
          and run.read(rule + 0x108) == SYMBOLS["rn_resolver_vtable"] + 4 and run.read(rule + 0x10C) == 5
          and run.read(rule + R_VALID) == 1 and run.read(params) == 8 and run.read(params + 4) == 20000
          and run.read(params + 0x20) == 14500 and run.read(params + 0x24) == FULL // 20000
          and run.read(params + 0x4C) == 300 and run.read(params + 0x54) == 1400
          and run.read(params + 0x58) == 100 * 1000000 and run.read(params + 0x5C) == 100
          and run.read(params + 0x60) == 3 and run.read(params + P_ZONES + ZONE_DEF + 4) == 1,
          f"rule {rule:#x}")

    # 3. The world: the tick a delegate of the second list, after the one there.
    players, statistics = world(run)
    first = run.allocate(0x18)
    run.word(CORE + TICK_LIST, 1)
    run.word(CORE + TICK_LIST + 4, first)
    run.word(CORE + TICK_LIST + 8, first)
    run.thiscall(vtable(run, rule, 11), rule, CORE)
    tick = rule + 0x110
    check("set_game_world_core links the tick after the time rule's, in order",
          run.read(rule + R_CORE) == CORE and run.read(CORE + TICK_LIST) == 2
          and run.read(CORE + TICK_LIST + 8) == tick and run.read(first + 0xC) == tick
          and run.read(tick + 8) == first and run.read(tick) == rule and run.read(tick + 4) == SYMBOLS["rn_tick"]
          and run.read(tick + 0x10) == 7 and run.read(run.read(CORE + TICK_ORDER)) == 8 and run.read(tick + 0x14, 1) == 1)

    # 4. Ticks: a player of team 0 alone in unit A for 20 s takes it.
    for place in range(4):
        run.thiscall(vtable(run, rule, 8), rule, place, 0)
    put(run, players[0], 1.0)
    time = 100000
    for _ in range(399):
        time += 50
        run.thiscall(SYMBOLS["rn_tick"], rule, 50, time, 1)
    a = zone(run, rule, 0)
    check("19.95 s of one player: unit A still taking", a["owner"] == 2 and a["capturing"] == 0 and a["count"] == (1, 0)
          and a["progress"] == 399 * 50 * (FULL // 20000), str(a))
    time += 50
    run.thiscall(SYMBOLS["rn_tick"], rule, 50, time, 1)
    a = zone(run, rule, 0)
    capture = run.read(statistics + EVENTS_AT + 6 * 6, 2)
    check("at 20 s team 0 owns unit A; its player's capture event 6 is 100 (hundredths, kit 0)",
          a["owner"] == 0 and a["progress"] == FULL and capture == 100, f"{a}, event 6 {capture}")
    # The game's own recalculate_scores reads the cell we wrote as event 6
    # (srp's weights: 6 is 30, 7 and 19 are 2.5, the cells revision 30's
    # build wrote for kits 1 and 2): 100 hundredths x 30 = 30.
    weights = run.allocate(4 * 28)
    for event, weight in ((6, 30.0), (7, 2.5), (10, 10.0), (18, 30.0), (19, 2.5)):
        run.emu.mem_write(weights + 4 * event, struct.pack("<f", weight))
    run.word(statistics + OPTIONS, weights - WEIGHTS)
    run.emu.mem_write(statistics + SCORE_COUNT, bytes([20]))
    run.call(RECALCULATE, regs={UC_X86_REG_EDI: statistics})
    score = run.read(statistics + SCORES, 2)
    check("the game's recalculate_scores 0x26DC30 counts the capture as event 6: score 30", score == 30,
          f"score {score}")
    run.word(CORE + MATCH_STATE, 2)
    before = bytes(run.emu.mem_read(rule + R_STATE, 0x138))
    run.thiscall(SYMBOLS["rn_tick"], rule, 50, time + 50, 1)
    check("nothing moves outside play (the time rule's state 2)", bytes(run.emu.mem_read(rule + R_STATE, 0x138)) == before)
    run.word(CORE + MATCH_STATE, 3)
    for _ in range(1200):
        time += 50
        run.thiscall(SYMBOLS["rn_tick"], rule, 50, time, 1)
    check("a minute of one unit: 5 points in the statistics' tail, team 0 leads",
          run.read(statistics + 0x31D4, 2) == 5 and run.read(statistics + 0x31D6, 2) == 0
          and run.read(statistics + 0x31E4) == 0 and run.read(rule + R_STATE + 8, 2) == 5)

    # 5. A kill: the victim of team 1 inside unit A (team 0's) is an invader.
    put(run, players[1], 2.0)
    time += 50
    run.thiscall(SYMBOLS["rn_tick"], rule, 50, time, 1)
    data = run.allocate(4)
    run.emu.mem_write(data, bytes([0, 1]))
    run.word(run.read(CORE + PLACES) + 4, players[0])              # the killer plays kit 1
    run.emu.mem_write(run.read(CORE + PLACES) + 0xC, bytes([1]))
    _, cleanup = run.thiscall(vtable(run, rule, 7), rule, 1, data, time)
    check("killing an enemy inside one's unit counts event 10 for the killer, in its kit 1's cell",
          cleanup == 12 and run.read(statistics + EVENTS_AT + 6 * 10 + 2, 2) == 1
          and run.read(statistics + EVENTS_AT + 6 * 11, 2) == 0)
    run.emu.mem_write(run.read(CORE + PLACES) + 0xC, bytes([0]))
    run.emu.mem_write(data, bytes([1, 0]))
    run.thiscall(vtable(run, rule, 7), rule, 1, data, time)
    check("a kill in one's own unit counts nothing", run.read(statistics + EVENTS_AT + 0x1F8 + 6 * 10, 2) == 0)

    # 6. The state saved and loaded.
    writer, buffer = run.allocate(0x20), run.allocate(0x10)
    run.word(writer + 0x14, buffer)
    run.word(buffer + 4, run.allocate(0x40))
    run.word(buffer + 8, 0x40)
    run.word(buffer + 0xC, 0x23)
    run.thiscall(vtable(run, rule, 15), rule, writer)
    written = run.read(buffer + 0xC) - 0x23
    blob = bytes(run.emu.mem_read(run.read(buffer + 4) + 0x23, written))
    state = bytes(run.emu.mem_read(rule + R_STATE, 0x18 + 3 * ZONE))
    expected = state[:20] + state[20:21] + b"".join(state[0x18 + ZONE * z:0x18 + ZONE * z + 96] for z in range(3))
    check("serialize appends 21 + 96 x 3 bytes through the game's writers: rr_write's layout, the server's",
          written == 309 and blob == expected and blob[20] == 3 and blob[21 + 5] == 0
          and struct.unpack_from("<I", blob, 21)[0] == FULL, f"{written} bytes")
    run.emu.mem_write(DUMP_ON, b"\x01\x01")
    dumped, dump_buffer = run.allocate(0x20), run.allocate(0x10)
    run.word(dumped + 0x14, dump_buffer)
    run.word(dump_buffer + 4, run.allocate(0x10))
    run.word(dump_buffer + 8, 0x10)
    run.thiscall(vtable(run, rule, 15), rule, dumped)
    run.emu.mem_write(DUMP_ON, b"\x00\x00")
    records, at = [], run.read(dumped + 8)
    while at and len(records) < 200:
        records.append((run.string(run.read(at + 8)), run.read(at + 0x18), run.string(run.read(at + 0xC))))
        at = run.read(at)
    zone_names = lambda u: ([f"{u}.progress", f"{u}.capturing", f"{u}.owner", f"{u}.count[0]", f"{u}.count[1]",
                             f"{u}.present[0]", f"{u}.present[1]"] + [f"{u}.marks[{i}]" for i in range(20)])
    names = ["m_points[0]", "m_points[1]", "m_score[0]", "m_score[1]", "m_end_time", "m_entered", "m_zone_count"] \
        + zone_names("A") + zone_names("B") + zone_names("C")
    sizes = [4, 4, 2, 2, 4, 4, 1] + ([4, 1, 1, 1, 1, 4, 4] + [4] * 20) * 3
    dumped_bytes = bytes(run.emu.mem_read(run.read(dump_buffer + 4), run.read(dump_buffer + 0xC)))
    check("with the hash mismatch dump on, the writers record the 88 fields by name, size and function",
          [r[0] for r in records] == names and [r[1] for r in records] == sizes
          and {r[2] for r in records} == {"srp::research_rule::serialize"} and run.read(dumped) == 88
          and dumped_bytes == blob, f"{len(records)} records")
    other, _ = run.thiscall(vtable(run, cook, 9), cook, query)
    reader = run.allocate(8)
    source = run.allocate(512)
    run.emu.mem_write(source, blob)
    run.word(reader + 4, source)
    _, cleanup = run.thiscall(vtable(run, other, 16), other, reader, 0x5A5A5A5A)
    check("deserialize(reader, dword) reads it back, the same state, and pops both (ret 8, as the game's rules)",
          cleanup == 8 and run.read(reader + 4) == source + 309
          and bytes(run.emu.mem_read(other + R_STATE, 0x138)) == bytes(run.emu.mem_read(rule + R_STATE, 0x138)),
          f"cleanup {cleanup}")

    # 6b. The game's own loops over the world's rules (game_world_core's
    # serialize 0x2D2661..0x2D2688 and deserialize 0x2D2B1A..0x2D2B5F, in
    # 0x2D2770), with our rule in the world's rules: esp as it was after
    # each, the bytes the same.
    snapshot = FAKE
    rules = run.allocate(4)
    run.word(rules, rule)
    run.word(CORE + RULES, rules)
    run.word(CORE + RULES + 4, rules + 4)
    loop_writer, loop_buffer = snapshot + 0xB7EF0, run.allocate(0x10)
    run.emu.mem_write(loop_writer, bytes(0x20))
    run.word(loop_writer + 0x14, loop_buffer)
    run.word(loop_buffer + 4, run.allocate(0x10))
    run.word(loop_buffer + 8, 0x10)
    run.word(loop_buffer + 0xC, 0)
    eip, moved = run.span(0x2D2661, 0x2D2688, {UC_X86_REG_EBP: CORE, UC_X86_REG_ESI: snapshot})
    looped = bytes(run.emu.mem_read(run.read(loop_buffer + 4), run.read(loop_buffer + 0xC)))
    check("the game's serialize loop (0x2D2661): esp as before, our 309 bytes in the snapshot's writer",
          eip == 0x2D2688 and moved == 0 and looped == blob, f"esp moved {moved}, {len(looped)} bytes")
    third = run.allocate(512)
    run.emu.mem_write(third, blob)
    run.word(snapshot + 0xB7EE4, third)
    run.word(snapshot + 0xB7EEC, third + 309)
    fresh, _ = run.thiscall(vtable(run, cook, 9), cook, query)
    run.word(rules, fresh)
    stack = [0] * 0x21 + [0x7E57]                          # [esp+84h]: the dword the loop passes on
    eip, moved = run.span(0x2D2B1A, 0x2D2B5F, {UC_X86_REG_EBP: CORE, UC_X86_REG_EBX: snapshot}, stack)
    check("the game's deserialize loop (0x2D2B42): esp as before, the cursor past our bytes, the same state",
          eip == 0x2D2B5F and moved == 0 and run.read(STACK + 0x6000 + 0x58) == third + 309
          and bytes(run.emu.mem_read(fresh + R_STATE, 0x138)) == bytes(run.emu.mem_read(rule + R_STATE, 0x138)),
          f"esp moved {moved}, cursor {run.read(STACK + 0x6000 + 0x58):#x} of {third:#x}")
    run.word(CORE + RULES, 0)
    run.word(CORE + RULES + 4, 0)

    # 6c. Inside: the player's transform is his feet, his capsule stands on
    # it (1.8 m standing, half its height from his controller as the game's
    # get_transform 0x7F5C0 takes it), 0.4 m round. Unit A: a box at y 0,
    # 2 m up and down.
    inner, _ = run.thiscall(vtable(run, cook, 9), cook, query)
    run.thiscall(vtable(run, inner, 11), inner, CORE)
    run.thiscall(vtable(run, inner, 8), inner, 2, 0)

    def counted(feet_y, controller=None):
        put(run, players[2], 1.0, feet_y)
        run.word(players[2] + 0x494, controller or 0)
        run.emu.mem_write(inner + R_STATE + 0x10, struct.pack("<I", 1 << 2))
        run.thiscall(SYMBOLS["rn_tick"], inner, 50, time, 1)
        return zone(run, inner, 0)["count"][0] == 1
    crouched, body = run.allocate(4), run.allocate(0x200)
    run.word(crouched, body)
    run.emu.mem_write(body + 0x1D0, struct.pack("<3f", 0.275, 0.275, 0.275))
    run.word(body + 0x1F0, 1)
    check("inside: feet on the unit's top in, 0.1 m over it (a floor above) out; a standing head from 1.8 m "
          "below in; the same crouched (half height 0.55 from the controller) out",
          counted(1.9) and not counted(2.1) and counted(-3.5) and not counted(-3.5, crouched) and counted(-2.5, crouched))
    put(run, players[2], 1000.0)
    run.word(players[2] + 0x494, 0)

    # 7. The hud's points and completed.
    hud = run.allocate(0x100)
    run.emu.mem_write(hud + 0x94, b"\xff\xff")             # the hud state's constructor's (0x278D86)
    _, cleanup = run.thiscall(vtable(run, rule, 17), rule, hud, 0)
    check("fill_match_hud_state: the teams' points; the alive counts left as the hud state has them",
          cleanup == 8 and run.read(hud + 0x90, 2) == 5 and run.read(hud + 0x92, 2) == 0
          and run.read(hud + 0x94, 2) == 0xFFFF)
    run.word(rule + R_STATE + 12, time + 3000)
    done, _ = run.thiscall(vtable(run, rule, 18), rule, time + 2999)
    later, _ = run.thiscall(vtable(run, rule, 18), rule, time + 3000)
    check("rule_completed at the end time", (done & 0xFF) == 0 and (later & 0xFF) == 1)
    run.word(rule + R_STATE + 12, 0xFFFFFFFF)

    # 8. The hud each frame: root.research_update, messages at changes.
    ui, hold, view, movie = run.allocate(0x400), run.allocate(0x200), run.allocate(0x10), run.allocate(0x10)
    game2 = run.allocate(0x5000)
    run.word(ui + 0x18, game2)
    run.word(game2 + 0x4D4C, CORE - 0x1F8)
    rules = run.allocate(8)
    run.word(rules, first)
    run.word(rules + 4, rule)
    run.word(CORE + RULES, rules)
    run.word(CORE + RULES + 4, rules + 8)
    run.word(ui + 8, hold)
    run.word(hold + 0x108, view)
    run.word(view + 4, movie)
    run.word(ui + 0x20, 5)
    run.emu.mem_write(CORE + LOCAL_PLACE, b"\0")
    run.call(0x23B3DE, regs={UC_X86_REG_EDI: ui}, until=0x23B3FC)
    sent = run.invokes[-1] if run.invokes else None
    check("mode 5: research_update with 35 numbers (nine a unit, then the units' ms left: A contested, none), "
          "then past the Team Deathmatch part",
          sent is not None and sent[0] == movie and sent[1] == "root.research_update" and len(sent[3]) == 35
          and sent[3][:5] == [0, 5, 0, 100, 3] and sent[3][5:14] == [0, 0, -10, 0, 0, 100, 1, 1, 1]
          and sent[3][32] == 0 and not run.messages, str(sent))
    count = len(run.invokes)
    run.call(0x23B3DE, regs={UC_X86_REG_EDI: ui}, until=0x23B3FC)
    check("the same state again: nothing sent", len(run.invokes) == count)
    run.emu.mem_write(rule + R_STATE + S_ZONES + 5, b"\x01")
    run.call(0x23B3DE, regs={UC_X86_REG_EDI: ui}, until=0x23B3FC)
    check("unit A taken by the other team: 'Unit A lost', and the movie hears of it",
          run.messages and run.messages[-1] == ("st_research_unit_a_lost", ui, 0x21, 0x14, 6000)
          and len(run.invokes) == count + 1)
    run.word(rule + R_STATE + 12, time + 3000)
    run.call(0x23B3DE, regs={UC_X86_REG_EDI: ui}, until=0x23B3FC)
    check("the end, team 0 ahead: the winner's message",
          run.messages[-1][0] == "st_finish_match_mode_05_winner_message")
    run.word(ui + 0x20, 4)
    sent = len(run.invokes)
    run.call(0x23B3DE, regs={UC_X86_REG_EDI: ui}, until=0x23B3E4)
    check("mode 4: nothing sent, on to its last players", len(run.invokes) == sent
          and run.emu.reg_read(UC_X86_REG_EIP) == 0x23B3E4)

    # 8b. The units' markers, through update_ui itself (0x23B200: eax the ui;
    # dt, time, a flag; `ret 0Ch`): its call of update_player_icons
    # (0x23B207, answered here) comes before its 40 ms gate (time - [ui+33Ch]
    # under 28h: out at 0x23B53B) and its hud-state checksum, so the markers
    # move every frame. The player-icons movie ([[ui+10h]+108h]+4, its
    # vtable's +5Ch the frame's advance, answered by a stub); the game's own
    # point_to_screen (0x23DEC0: its camera +0Ch, projection +4Ch, viewport
    # [[+9Ch]+53B0h]+108h) on a camera at the origin looking along +z, 90
    # degrees, a 1000 x 1000 screen; the distance from the camera director's
    # position ([+98h]+34h). Unit B taken alone, half done.
    icons_hold, icons_view, icons = run.allocate(0x200), run.allocate(0x10), run.allocate(0x10)
    advance, advance_vtable = SENTINEL + 0x300, run.allocate(0x80)
    run.emu.mem_write(advance, bytes.fromhex("d9eec20c00"))     # fldz; ret 0Ch
    run.word(advance_vtable + 0x5C, advance)
    run.word(icons, advance_vtable)
    run.word(ui + 0x10, icons_hold)
    run.word(icons_hold + 0x108, icons_view)
    run.word(icons_view + 4, icons)
    near, far = 0.1, 1000.0

    def camera_at(x):
        run.emu.mem_write(game2 + 0xC, struct.pack("<16f", 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, x, 0, 0, 1))
        run.emu.mem_write(director + 0x34, struct.pack("<3f", x, 0, 0))
    run.emu.mem_write(game2 + 0x4C, struct.pack("<16f", 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, far / (far - near), 1,
                                                 0, 0, -near * far / (far - near), 0))
    scene, viewport, director = run.allocate(0x5400), run.allocate(0x110), run.allocate(0x40)
    run.word(game2 + 0x9C, scene)
    run.word(scene + 0x53B0, viewport)
    run.word(viewport + 0x108, 1000)
    run.word(viewport + 0x10C, 1000)
    run.word(game2 + 0x98, director)
    camera_at(0.0)
    b = rule + R_STATE + S_ZONES + ZONE
    run.word(b, FULL // 2)
    run.emu.mem_write(b + 4, bytes([0, 2, 1, 0]))
    run.word(b + 8, 1 << 5)
    run.word(b + 12, 0)
    run.word(rule + R_STATE + 12, 0xFFFFFFFF)
    run.word(CORE + MATCH_STATE, 3)
    run.word(ui + 0x20, 5)
    now = 100000
    run.word(ui + 0x33C, now)                                   # the 40 ms gate closed: past it, nothing

    def frame():
        start = len(run.invokes)
        run.player_icons = []
        _, moved = run.call(0x23B200, args=(16, now, 0), regs={UC_X86_REG_EAX: ui})
        return run.invokes[start:], moved

    sent, moved = frame()
    marks = [i for i in sent if i[1] == "root.research_markers"]
    values = marks[-1][3] if marks else []
    check("update_ui, its gates closed: update_player_icons(ui) once, then research_markers: A on the screen at "
          "(500, 400), 10 m away; B and C off it; B's 10 s left; research_update too (in play), B's 10 s",
          moved == 0xC and run.player_icons == [ui] and len(marks) == 1 and marks[0][0] == icons
          and len(values) == 32 and values[:2] == [0, 3]
          and values[2:7] == [0, 1, 500, 400, 10] and values[12:22] == [1, 0, 0, 0, 101, 2, 0, 50, 10000, 0]
          and values[22:24] == [2, 0] and values[26] == 200
          and sent[-1][1] == "root.research_update" and sent[-1][3][33] == 10000, f"{sent} esp {moved}")
    sent, _ = frame()
    check("the next frame, nothing changed: nothing sent", sent == [] and run.player_icons == [ui])
    camera_at(1.0)
    sent, _ = frame()
    check("the camera alone moved 1 m right (the gates still closed): the markers at once, A at (450, 400)",
          len(sent) == 1 and sent[0][1] == "root.research_markers" and sent[0][3][2:7] == [0, 1, 450, 400, 10],
          str(sent))
    run.word(b, FULL // 2 + 30000)
    sent, _ = frame()
    check("B's capture 0.6 ms further (9999.4 ms left: 10 s in whole seconds, the same tenth of a per cent): "
          "nothing sent", sent == [], str(sent))
    run.word(rule + R_STATE + 12, now + 3000)
    sent, _ = frame()
    check("the end (end_time set; the time rule stays in state 3): the markers hidden, once",
          [i for i in sent if i[1] == "root.research_markers"] == [(icons, "root.research_markers", 0, [0, 0])],
          str(sent))
    sent, _ = frame()
    check("then no markers", not any(i[1] == "root.research_markers" for i in sent), str(sent))
    run.word(rule + R_STATE + 12, 0xFFFFFFFF)
    sent, _ = frame()
    check("no end again (in play): shown", any(i[1] == "root.research_markers" and i[3][1] == 3 for i in sent))
    run.word(CORE + MATCH_STATE, 2)
    sent, _ = frame()
    check("the countdown (the time rule's state 2): the markers hidden, once",
          sent == [(icons, "root.research_markers", 0, [0, 0])], str(sent))
    sent, _ = frame()
    check("then nothing", sent == [])
    run.word(CORE + MATCH_STATE, 3)
    sent, _ = frame()
    check("in play again: shown", any(i[1] == "root.research_markers" and i[3][1] == 3 for i in sent))
    run.word(ui + 0x20, 4)
    sent, _ = frame()
    check("a match of mode 4 on the same ui: the markers hidden (none), once",
          sent == [(icons, "root.research_markers", 0, [0, 0])], str(sent))
    sent, _ = frame()
    check("then nothing", sent == [])
    stock.emu.mem_write(stock.allocate(4), b"")
    stock_ui = stock.allocate(0x100)
    stock.word(stock_ui + 0x20, 4)
    stock.call(0x23B3DE, regs={UC_X86_REG_EDI: stock_ui}, until=0x23B3E4)
    check("the original at the site goes the same way for mode 4", stock.emu.reg_read(UC_X86_REG_EIP) == 0x23B3E4)

    # 9. The match's first message by mode.
    for mode, until, key in ((4, 0x23B35B, 0xA03AAC), (5, 0x23B35B, None), (6, 0x23B36A, None)):
        run.call(0x23B351, regs={UC_X86_REG_EAX: mode, UC_X86_REG_EDI: ui}, until=until)
        ecx = run.emu.reg_read(UC_X86_REG_ECX)
        ok = run.emu.reg_read(UC_X86_REG_EIP) == until and (
            until == 0x23B36A or (ecx == key if key else run.string(ecx) == "st_start_match_welcome_message_5"))
        check(f"mode {mode}'s first message", ok, f"ecx {ecx:#x}")

    driven(run, cook, query, check)

    failed = results.count(False)
    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
