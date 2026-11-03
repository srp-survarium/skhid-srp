#!/usr/bin/env python3
"""Exercise patch 87's shared semantic validator at both native world loads.

    python check_world_check_026g.py ORIGINAL_EXE [--drift VERDICTS.tsv]

Uses current player pickup records, statistics and rules. Valid snapshots preserve
stock call arguments/registers; refusals consume the reader and close once. The
host drift file is srp-validate's drift_dump (including inferred match context).
Native deserialization/resource creation are stubbed: this is a boundary check,
not evidence that a complete match or weapon resource cook succeeded.
Each section may pass the original's inline buffer (patch 98 grows them); the body
stops at 0xF000 bytes. Each load leaves one World entry in the crash report's ring.
"""
import argparse
import json
from pathlib import Path
import struct
import sys

from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import patch_026g as base

RAW_SITE, UNPACKED_SITE = 0x27C0FF, 0x27C0D0
LOAD, LOADED, EPILOGUE, CLOSE = 0x2D0B70, 0x27C11B, 0x27C1E0, 0x27A790
CREATE_FILE, WRITE_FILE, CLOSE_HANDLE = 0x870048, 0x870108, 0x870040   # the import slots
QPC, QPF = 0x870158, 0x870150                                            # QueryPerformanceCounter / -Frequency
BREADCRUMB_CHECK = int.from_bytes(b"BRCS", "little")                     # vostok/breadcrumbs.rs CHECK
CLIENT, GAME, READER = 0x40000000, 0x40400000, 0x40500000
WORLD, STACK, STUBS = 0x41000000, 0x42000000, 0x43000000
HOLDER, POOL, PACKET = 0x44000000, 0x44100000, 0x44200000
WORLD_ROOM, STACK_ROOM = 0x20000, 0x10000
FRAME = STACK + 0x8000                     # the handler's EBP
CORE = GAME + 0x1F8                        # [client+1F1C40h] + 1F8h
KEPT = (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)
MUTATIONS = 16
# The sections' inline buffers in the original's history item (patch 98
# grows them), the state's cap and the body's (world-frame's MAX_BODY).
CAPS = (0x2400, 0xC00, 0x2000, 0x4000, 0x800, 0xC00)
STATE_CAP = 0x1000
MAX_BODY = 0xF000


class Run:
    """One exe's image under Unicorn, kept for every case (its translated
    code with it)."""

    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((CLIENT, 0x200000), (GAME, 0x20000), (READER, 0x1000), (WORLD, WORLD_ROOM),
                         (STACK, STACK_ROOM), (STUBS, 0x1000), (HOLDER, 0x60000), (POOL, 0x1000), (PACKET, 0x1000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        self.word(CLIENT + 0x1F1C40, GAME)
        self.word(CLIENT, STUBS + 0x100)
        # weapon_pickup's before_world_load asks the client its match
        # (network_client::match_options 0x219990, the vtable's +34h): the
        # options at the holder's +0Ch, their seed at +577B0h.
        self.word(STUBS + 0x100 + 0x34, 0x219990)
        self.word(STUBS + 0x150, STUBS + 0x80)
        emu.hook_add(UC_HOOK_CODE, self.player_lookup, begin=STUBS + 0x80, end=STUBS + 0x80)
        self.word(CLIENT+0x1F1C64, 2)
        self.word(CLIENT+0x1F1C68, 5)
        self.word(CLIENT+0x1F1C48, HOLDER)
        self.word(HOLDER, 0xA08864)
        self.word(HOLDER+0x577BC, 0x81234567)
        self.word(HOLDER+0x578B0, POOL)
        self.word(POOL+0x34, 100)
        self.match_context()
        for slot, stub in ((CREATE_FILE, STUBS), (WRITE_FILE, STUBS + 0x10), (CLOSE_HANDLE, STUBS + 0x20),
                           (0x870118, STUBS + 0x30),  # GetCurrentThreadId, pickup cache's owner
                           (0x8701B8, STUBS + 0x40),  # GetTickCount, pickup cache deadlines
                           (QPC, STUBS + 0x50), (QPF, STUBS + 0x60)):  # the crash report's ring
            self.word(slot, stub)
        for address in (LOAD, CLOSE, 0x279FE0, 0x27A2A0, 0x7AC10, STUBS, STUBS + 0x10, STUBS + 0x20, STUBS + 0x30, STUBS + 0x40,
                        STUBS + 0x50, STUBS + 0x60):
            handle = emu.hook_add(UC_HOOK_CODE, self.stub, begin=address, end=address)
            if address == CLOSE: self.close_stub = handle
        for address in (LOADED, EPILOGUE):
            emu.hook_add(UC_HOOK_CODE, self.stop, begin=address, end=address)

    def match_context(self, mode=4, batteries=0, points=3):
        table, rule, respawn = GAME+0x1a000, GAME+0x1b000, GAME+0x1c000
        self.word(CORE+0x18824, table)
        self.word(CORE+0x18828, table+8)
        self.word(table, rule)
        self.word(table+4, respawn)
        self.word(rule, {2:0xA06AB4,3:0xA06C04,4:0xA06CBC,5:0xA06A54}[mode])
        self.word(rule+0x10c, 5 if mode==5 else 0)
        self.word(rule+0x110, table+0x100)
        self.word(rule+0x114, table+0x100+batteries*4)
        self.word(respawn, 0xA06D1C)
        self.word(respawn+0x194, points)
        self.word(respawn+0x198, 0)
        self.word(respawn+0x19c, 0)

    def player_lookup(self, emu, address, size, _):
        # Pickup preparation still runs; this fixture has no loaded players
        # or gun resources. Native cooking is tested by the pickup harness.
        sp = emu.reg_read(UC_X86_REG_ESP)
        out = self.read(sp+4)
        self.word(out, 0)
        emu.reg_write(UC_X86_REG_EAX, out)
        emu.reg_write(UC_X86_REG_ESP, sp+12)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 * i) for i in range(1, 6)]
        if address == LOAD:                 # (core, reader, 1, 8), ret 10h
            reader = args[1]
            self.calls.append(("load", tuple(args[:4]), tuple(self.read(reader + 4 * i) for i in range(3))))
            pops, result = 16, 1
        elif address == 0x279FE0:
            assert emu.reg_read(UC_X86_REG_ECX) == HOLDER and args[0] == 0xe0
            self.packet_allocations = getattr(self, 'packet_allocations', 0) + 1
            self.word(PACKET+0x98, 0x40)
            pops, result = 4, PACKET
        elif address == 0x7AC10:
            assert args[0] == PACKET+0x2a8 and emu.reg_read(UC_X86_REG_EDI) == 20
            self.pending_packet = bytes(emu.mem_read(args[1], 20))
            pops, result = 8, 0
        elif address == 0x27A2A0:
            assert emu.reg_read(UC_X86_REG_ECX) == HOLDER and args[0] == PACKET
            assert not self.read(PACKET+0x98) & 0x40
            self.receipts.append(self.pending_packet)
            pops, result = 4, 0
        elif address == CLOSE:              # thiscall (reason), ret 4
            self.calls.append(("close", emu.reg_read(UC_X86_REG_ECX), args[0]))
            pops, result = 4, 0
        elif address == STUBS:              # CreateFileA: 7 arguments, a handle
            pops, result = 28, 0x1234
        elif address == STUBS + 0x10:       # WriteFile(file, bytes, size, &written, 0)
            self.lines.append(bytes(emu.mem_read(args[1], args[2])))
            self.word(args[3], args[2])
            pops, result = 20, 1
        elif address == STUBS + 0x30:        # GetCurrentThreadId
            pops, result = 0, 1
        elif address == STUBS + 0x40:        # GetTickCount
            self.ticks = getattr(self, 'ticks', 0) + getattr(self, 'tick_step', 100)
            pops, result = 0, self.ticks
        elif address in (STUBS + 0x50, STUBS + 0x60):   # QueryPerformanceCounter / -Frequency(&out)
            self.counter = getattr(self, "counter", 0) + 1
            value = self.counter if address == STUBS + 0x50 else 10_000_000
            self.emu.mem_write(args[0], struct.pack("<Q", value))
            pops, result = 4, 1
        else:                               # CloseHandle
            pops, result = 4, 1
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def stop(self, emu, address, size, _):
        self.stopped = address
        emu.emu_stop()

    def start(self, at, regs, sp):
        self.calls, self.lines, self.stopped, self.receipts = [], [], None, []
        self.ticks = getattr(self, "ticks", 0) + getattr(self, "advance", 2000)
        for reg, value in regs.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        try:
            self.emu.emu_start(at, 0, count=20_000_000)
        except UcError as error:
            raise RuntimeError(f"native execution at {self.emu.reg_read(UC_X86_REG_EIP):#x}") from error
        kept = {reg: self.emu.reg_read(reg) for reg in KEPT}
        return self.stopped, kept, self.emu.reg_read(UC_X86_REG_ESP)

    def raw(self, state):
        """The raw load: the packet's reader at the world (after the
        generation and the flag)."""
        assert len(state) + 2 <= WORLD_ROOM
        self.emu.mem_write(WORLD, b"\x05\x00" + state)
        for i, value in enumerate((WORLD, WORLD + 2, len(state) + 2)):
            self.word(READER + 4 * i, value)
        regs = {UC_X86_REG_EDI: CLIENT, UC_X86_REG_ESI: READER, UC_X86_REG_EBP: FRAME,
                UC_X86_REG_EBX: WORLD + 2, UC_X86_REG_EAX: 0, UC_X86_REG_ECX: 0, UC_X86_REG_EDX: 0}
        stopped, kept, sp = self.start(RAW_SITE, regs, FRAME - 0x30)
        return stopped, kept, sp, self.read(READER + 4)

    def unpacked(self, state):
        """The unpacked load: the world in the frame's mutable buffer
        {allocator 0, begin, capacity 8000h, size}, ESI its begin, EDX its
        size, ECX the core, the arguments pushed (8, 1, the frame's reader,
        the core)."""
        assert len(state) <= WORLD_ROOM
        self.emu.mem_write(WORLD, state)
        for at, value in ((-0x24, 0), (-0x20, WORLD), (-0x1C, 0x8000), (-0x18, len(state)),
                          (-0x14, 0), (-0x10, 0), (-0xC, 0)):
            self.word(FRAME + at, value)
        sp = FRAME - 0x30 - 16
        for i, value in enumerate((CORE, FRAME - 0x14, 1, 8)):
            self.word(sp + 4 * i, value)
        regs = {UC_X86_REG_EDI: CLIENT, UC_X86_REG_ESI: WORLD, UC_X86_REG_EBP: FRAME,
                UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_EAX: FRAME - 0x14, UC_X86_REG_ECX: CORE,
                UC_X86_REG_EDX: len(state)}
        stopped, kept, sp = self.start(UNPACKED_SITE, regs, sp)
        return stopped, kept, sp, None


def body(blocks, sections, mask=None):
    """A body of `blocks` (slot, state, animation bytes or None) and six
    sections."""
    out = bytearray(struct.pack("<I", sum(1 << s for s, _, _ in blocks) if mask is None else mask))
    for _, state, animation in blocks:
        out += struct.pack("<H", len(state)) + state
        out += b"\x01" + animation if animation else b"\x00"
    for section in sections:
        out += struct.pack("<H", len(section)) + section
    return bytes(out)


def state(world, time=4500):
    return struct.pack("<I", time) + world


# T = 0, no times, one float, not indexed: 69 bits, 9 bytes (world-frame's
# test of animation_len).
ANIMATION = bytes([0, 0, 0, 0, 0x08, 0, 0, 0, 0])
def sections(time=4500):
    # Twenty current zero-event PlayerStatistics (40 bytes each), then tail.
    statistics = bytes(40*20) + struct.pack('<HHBBBI',0,0,0,0,2,0)
    # Timelimit, respawn (three points), TDM, 20 kd records, blackboard flags.
    rules = struct.pack('<IIIB',0,time,0,3) + struct.pack('<II',0,42) + bytes(6)
    rules += struct.pack('<HHII',100,100,0,0) + bytes(5*20) + b'\xff\xff'
    effects = bytes(12) + struct.pack('<fI',1.0,0)
    return [bytes(16), statistics, b'', b'', rules, effects]


def player():
    # Two kit bytes, version-8 empty pickup record, zero-item head and a
    # native empty inventory header. Bodies after this header are opaque.
    head = bytes(2) + b'\x08' + bytes(28) + bytes(27)
    inventory = bytes(4) + b'\xff\xff' + bytes(29+5) + b'\xff\x00\x00\x00'
    return head+inventory


EMPTY = [b""] * 6

def cases():
    """(what, the state, whether a game can load it)."""
    small = [b"\x01" * 7, b"\x02" * 9, b"\x03" * 3, b"", b"\x05" * 11, b"\x06" * 2]
    two = [(0, b"\xAB" * 33, None), (19, b"\xCD" * 40, ANIMATION)]
    yield "no players", state(body([], small)), True
    yield "slots 0 and 19, one animated", state(body(two, small)), True
    for i, name in enumerate(("bullets", "statistics", "objects", "physics", "rules", "effects")):
        for extra in (0, 1):
            sections = list(EMPTY)
            sections[i] = bytes(CAPS[i] + extra)
            yield f"{name} {'at' if not extra else 'one over'} the original's inline buffer", \
                state(body(two[:1], sections)), True
    for extra, ok in ((0, True), (1, False)):
        fill = MAX_BODY + extra - len(body([], EMPTY))
        yield f"a body of {MAX_BODY + extra:#x} bytes", state(body([], [bytes(fill)] + EMPTY[1:])), ok
    yield "a state at 0x1000", state(body([(3, bytes(STATE_CAP), None)], EMPTY)), True
    yield "a state one over", state(body([(3, bytes(STATE_CAP + 1), None)], EMPTY)), False
    yield "slot 20", state(body([(0, b"\xAB" * 33, None)], EMPTY, mask=1 | 1 << 20)), False
    whole = state(body(two, small))
    yield "cut short", whole[:-1], False
    yield "a byte too many", whole + b"\x00", False
    yield "the body without its time", body(two, small), False
    yield "shorter than a time", b"\x01\x02\x03", False


def compare(stock, ours, path, world, can_load):
    """Failures of `ours` against `stock` on one world at one load."""
    load = getattr(stock, path)(world)
    calls = stock.calls
    fails = []
    if load[0] != LOADED or [c[0] for c in calls] != ["load"]:
        fails.append(f"the original does not load it ({load[0]}, {calls})")
    got = getattr(ours, path)(world)
    if can_load:
        if got[:3] != load[:3] or ours.calls != calls or got[3] != load[3]:
            fails.append(f"loaded otherwise: {got}, {ours.calls} against {load}, {calls}")
        if ours.lines:
            fails.append(f"a log line: {ours.lines}")
        return fails
    stopped, kept, _, cursor = got
    if stopped != EPILOGUE:
        fails.append(f"not at the epilogue: {stopped}")
    if ours.calls:
        fails.append(f"calls {ours.calls}")
    entry = {UC_X86_REG_EDI: CLIENT, UC_X86_REG_EBP: FRAME,
             UC_X86_REG_ESI: READER if path == "raw" else WORLD,
             UC_X86_REG_EBX: WORLD + 2 if path == "raw" else 0x0B0B0B0B}
    if kept != entry:
        fails.append(f"registers {kept}")
    if path == "raw" and cursor != WORLD + 2 + len(world):
        fails.append(f"the reader not at its end: {cursor:#x}")
    if len(ours.receipts) != 1 or ours.receipts[0][13] != 1:
        fails.append(f"no rejection receipt: {ours.receipts}")
    if len(ours.lines) != 1 or b"world_check: a world refused" not in ours.lines[0]:
        fails.append(f"the log: {ours.lines}")
    return fails


def breadcrumbs(ours, worlds):
    """Failures of the crash report's ring (vostok/breadcrumbs.rs) after
    `worlds` (each case's state, raw then unpacked, in order): found as the
    reporter finds it (its address, then that xor "BRCS"), one World entry a
    load, the last one's bytes, packed and refused."""
    lock = json.load(open(Path(__file__).resolve().parent / "arena_026g.lock.json"))
    start, size = int(lock["va"], 16), int(lock["size"])
    memory = bytes(ours.emu.mem_read(start, size))
    found = [start + at for at in range(0, size - 8, 4)
             if struct.unpack_from("<II", memory, at) == (start + at, (start + at) ^ BREADCRUMB_CHECK)]
    if len(found) != 1:
        return [f"the ring's header found {len(found)} times"]
    at = found[0] - start
    version, entry_size, capacity, count = struct.unpack_from("<HHII", memory, at + 8)
    fails = []
    if (version, entry_size, capacity) != (1, 32, 1024):
        fails.append(f"the header: version {version}, entry {entry_size}, capacity {capacity}")
    if count != len(worlds):
        fails.append(f"{count} entries for {len(worlds)} loads")
    entries = []
    for n in range(max(0, count - capacity), count):
        seq, kind, a, b, _, time, size_ = struct.unpack_from("<IBBHQII", memory, at + 64 + 32 * (n % capacity))
        entries.append((seq, kind, a, b, time, size_))
    want = [(n + 1, 2, (path == "unpacked") | (ok << 1), 0, struct.unpack("<I", world[:4])[0] if len(world) >= 4 else 0,
             len(world)) for n, (world, path, ok) in enumerate(worlds)]
    if entries != want:
        fails.append(f"the entries differ: first {next((e, w) for e, w in zip(entries, want) if e != w)}")
    return fails


def drift(ours, path):
    """(worlds, checks, the rows that differ)."""
    worlds = checks = 0
    differ = []
    for row in open(path):
        file, time, mode, batteries, points, wanted = row.rstrip("\n").split("\t")
        ours.match_context(int(mode), int(batteries), int(points))
        world = open(file, "rb").read()
        bodies = [world]
        for k in range(MUTATIONS):
            mutated = bytearray(world)
            mutated[len(world) * k // MUTATIONS] ^= 0xFF
            bodies.append(bytes(mutated))
        worlds += 1
        for k, (one, verdict) in enumerate(zip(bodies, wanted)):
            loads = ["raw", "unpacked"] if k == 0 else ["raw"]
            for load in loads:
                try:
                    stopped = getattr(ours, load)(state(one, int(time)))[0]
                except RuntimeError as error:
                    raise RuntimeError(f"{file} copy {k} ({load}): {error}") from error
                checks += 1
                got = "0" if stopped == LOADED else "1" if stopped == EPILOGUE else "?"
                if got != verdict:
                    differ.append(f"{file} copy {k} ({load}): the game's build {got}, the host's {verdict}")
    return worlds, checks, differ


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("exe")
    parser.add_argument("--drift")
    args = parser.parse_args()
    original = open(args.exe, "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    stock, ours = Run(original), Run(bytes(built))
    failed = 0
    worlds = []
    for what, world, can_load in cases():
        for path in ("raw", "unpacked"):
            worlds.append((world, path, can_load))
            fails = compare(stock, ours, path, world, can_load)
            failed += bool(fails)
            verdict = "loaded" if can_load else "ignored, recovery requested"
            print(f"  {'FAIL' if fails else 'ok'}  {path}: {what}: {verdict}" + "".join(f"\n      {f}" for f in fails))
    fails = breadcrumbs(ours, worlds)
    failed += bool(fails)
    print(f"  {'FAIL' if fails else 'ok'}  the crash report's ring: one World entry a load"
          + "".join(f"\n      {f}" for f in fails))
    if args.drift:
        worlds, checks, differ = drift(ours, args.drift)
        failed += len(differ)
        for line in differ[:20]:
            print(f"  FAIL  drift: {line}")
        print(f"  {'FAIL' if differ else 'ok'}  drift: {worlds} worlds, {checks} checks with the mutated copies, "
              f"{len(differ)} differ")
    print("all cases passed" if not failed else f"{failed} FAILED")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
