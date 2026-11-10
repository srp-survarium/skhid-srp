#!/usr/bin/env python3
"""Patch 127 (memory_profile): the memory profiler, dormant unless the game
starts with SKHID_MEMORY_PROFILE=on (docs/notes/memory-profile.md).

    python check_memory_profile_026g.py <original survarium.exe>

dlmalloc's own code runs, in the original and in the patched exe, on a heap
the check makes with the game's create_mspace_with_base (0x101280) over 1 MiB
of its own, the doug_lea_allocator over it registered as the engine's only
arena (the list at 0xA808FC). The same allocations, frees and reallocations
(mspace_malloc, mspace_realloc and mspace_free called as the game calls them,
and doug_lea_allocator::malloc_impl over them) are made in both:
1. the built exe has the original's bytes at the six sites installed on
   demand (the allocator's five gates, out_of_memory_with_crash's entry);
2. the variable unset: allocate_region's tail (memory.arenas_ready) reserves
   nothing, writes nothing, logs nothing and leaves those bytes alone; each
   call runs the original's instructions, as many, with every register and
   flag and the heap's bytes the same;
3. SKHID_MEMORY_PROFILE=on: the tables are reserved (VirtualAlloc, stubbed),
   the six jumps written (each page made writable and given its protection
   back, the instruction cache flushed), one line logged; each call still
   gives the original's result, registers and heap bytes (flags aside: the
   gate's own `add` comes last);
4. Scroll Lock down (GetAsyncKeyState, stubbed) at a gate's poll: a snapshot;
5. the heap asked for more than it holds: out_of_memory_with_crash's entry
   (memory.out_of_memory) writes a snapshot to srp_memory_profile.txt (the
   file stubbed) before the game's own body goes on; scripts/memory-profile/
   analyze.py reads it: the arena's heap use as the walk finds it, its holes,
   each call site's live bytes as the heap's chunks say (malloc_impl's
   caller, not malloc_impl; a reallocation's caller; nothing of
   mspace_realloc's own moves), the bytes asked, the address space's regions.
"""
import importlib.util
import struct
import sys
from pathlib import Path

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EFLAGS, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

HERE = Path(__file__).resolve().parent
MSPACE_MALLOC, MSPACE_REALLOC, MSPACE_FREE = 0x101330, 0x1005A0, 0x1015E0
CREATE_MSPACE, MALLOC_IMPL = 0x101280, 0xE4F90
ARENAS_READY, OUT_OF_MEMORY, OUT_OF_MEMORY_ON = 0xE12BF, 0xE4D70, 0xE4D76
ALLOCATE_REGION_RETURN = 0x13BF5        # engine_world's constructor after its call of allocate_region
USER_DATA_DIRECTORY = 0x18BD0
ARENA_LIST_SLOT, DOUG_LEA_VTABLE = 0xA808FC, 0x9E68DC
IMPORTS = {0x8701B4: ("GetEnvironmentVariableA", 3), 0x87015C: ("VirtualAlloc", 4),
           0x870118: ("GetCurrentThreadId", 0), 0x870090: ("GetTickCount", 0),
           0x8701B8: ("GetCurrentProcessId", 0), 0x870048: ("CreateFileA", 7), 0x870108: ("WriteFile", 5),
           0x870040: ("CloseHandle", 1), 0x870168: ("VirtualQuery", 3), 0x87008C: ("GetSystemInfo", 1),
           0x870200: ("VirtualProtect", 4), 0x87003C: ("GetModuleHandleA", 1), 0x870084: ("GetProcAddress", 2),
           0x870080: ("LoadLibraryA", 1), 0x870160: ("VirtualFree", 3)}
# what GetProcAddress gives: stubs of their own
DYNAMIC = {"FlushInstructionCache": 3, "GetAsyncKeyState": 1}
# the sites patch 127 installs on demand: the five gates, out_of_memory_with_crash's entry
ON_DEMAND = {0x101330: 5, 0x1005A0: 5, 0x1015E0: 6, 0x10700A: 6, 0x106DAC: 6, OUT_OF_MEMORY: 6}
KERNEL32, USER32 = 0x7B000000, 0x7B100000

DATA = 0x30000000
ALLOCATOR, LIST, RECORD, STRINGS, STUBS = (DATA + 0x1000 * i for i in range(5))
RETURN, CALLER_A, CALLER_B, CALLER_C = STUBS, STUBS + 0x10, STUBS + 0x20, STUBS + 0x30
IMPORT_STUBS = STUBS + 0x100
HEAP, HEAP_SIZE = 0x40000000, 0x100000
ROOM, ROOM_SIZE = 0x50000000, 0x2000000
STACK = 0x21000000
KEPT = {UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_EDI: 0x0E0E0E0E, UC_X86_REG_EBP: 0x0019FF00,
        UC_X86_REG_ECX: 0x0C0C0C0C, UC_X86_REG_EDX: 0x0D0D0D0D}
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP, UC_X86_REG_ESP)
FLAGS = 0x8D5                                    # CF PF AF ZF SF OF
# the address space VirtualQuery tells: (base, size, state, protect, type)
SPACE = [(0, HEAP, 0x10000, 1, 0), (HEAP, HEAP_SIZE, 0x1000, 4, 0x20000),
         (HEAP + HEAP_SIZE, ROOM - HEAP - HEAP_SIZE, 0x10000, 1, 0), (ROOM, ROOM_SIZE, 0x1000, 4, 0x20000),
         (ROOM + ROOM_SIZE, 0x7FFF0000 - ROOM - ROOM_SIZE, 0x10000, 1, 0)]


def analyzer():
    path = HERE.parent / "scripts" / "memory-profile" / "analyze.py"
    spec = importlib.util.spec_from_file_location("memory_profile_analyze", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class Run:
    def __init__(self, data, variable=None):
        self.variable = variable
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(DATA, 0x10000, UC_PROT_ALL)
        emu.mem_map(HEAP, HEAP_SIZE, UC_PROT_ALL)
        emu.mem_map(ROOM, ROOM_SIZE, UC_PROT_ALL)
        emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
        self.stubs = {}
        for i, (slot, (name, count)) in enumerate(sorted(IMPORTS.items())):
            self.stubs[IMPORT_STUBS + 0x10 * i] = (name, count)
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=IMPORT_STUBS + 0x10 * i, end=IMPORT_STUBS + 0x10 * i)
            self.word(slot, IMPORT_STUBS + 0x10 * i)
        self.dynamic = {}
        for i, (name, count) in enumerate(sorted(DYNAMIC.items())):
            at = IMPORT_STUBS + 0x800 + 0x10 * i
            self.stubs[at] = (name, count)
            self.dynamic[name] = at
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=at, end=at)
        self.protects, self.flushes, self.scroll, self.tick = [], [], False, 123456
        self.freed, self.rooms_left = [], None      # VirtualFree's; how many VirtualAllocs succeed (None: all)
        for stop in (RETURN, CALLER_A, CALLER_B, CALLER_C, OUT_OF_MEMORY_ON, ALLOCATE_REGION_RETURN):
            emu.hook_add(UC_HOOK_CODE, self.done, begin=stop, end=stop)
        emu.hook_add(UC_HOOK_CODE, self.user_data, begin=USER_DATA_DIRECTORY, end=USER_DATA_DIRECTORY)
        self.room = ROOM
        self.steps = self.gates = 0
        emu.hook_add(UC_HOOK_CODE, self.step)
        for gate in (MSPACE_MALLOC, MSPACE_REALLOC, MSPACE_FREE):
            emu.hook_add(UC_HOOK_CODE, self.gate, begin=gate, end=gate)
        self.reserved, self.files, self.written = [], {}, {}
        emu.mem_write(STRINGS, b"test heap\0")
        emu.mem_write(STRINGS + 0x100, b"C:\\Users\\test\\Documents\\Survarium\0")
        # the allocator: doug_lea's vtable, its flags (crash after out of
        # memory, no null, not out of memory, no guards), the mspace below
        self.word(ALLOCATOR, DOUG_LEA_VTABLE)
        emu.mem_write(ALLOCATOR + 0x28, bytes([1, 0, 0, 0]))
        self.word(ALLOCATOR + 0x0C, STRINGS)
        self.mspace = self.call(CREATE_MSPACE, [HEAP, HEAP_SIZE, 0, OUT_OF_MEMORY, ALLOCATOR])[0]
        assert HEAP < self.mspace < HEAP + 0x100, hex(self.mspace)
        self.word(ALLOCATOR + 0x14, self.mspace)
        self.word(ARENA_LIST_SLOT, LIST)
        emu.mem_write(LIST, struct.pack("<3I", RECORD, RECORD + 0x18, RECORD + 0x18))
        emu.mem_write(RECORD, struct.pack("<6I", ALLOCATOR, 0, HEAP_SIZE, 0, HEAP, STRINGS))

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def string(self, at):
        out = bytearray()
        while (b := self.emu.mem_read(at + len(out), 1)[0]) != 0:
            out.append(b)
        return out.decode()

    def step(self, emu, address, size, _):
        self.steps += 1

    def gate(self, emu, address, size, _):
        self.gates += 1

    def done(self, emu, address, size, _):
        emu.emu_stop()

    def user_data(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        emu.reg_write(UC_X86_REG_EAX, STRINGS + 0x100)
        emu.reg_write(UC_X86_REG_ESP, sp + 4)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def stub(self, emu, address, size, _):
        name, count = self.stubs[address]
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 + 4 * i) for i in range(count)]
        result = 0
        if name == "GetEnvironmentVariableA":
            # patch 138 reads its own switch at the same site: never set here
            assert self.string(args[0]) in ("SKHID_MEMORY_PROFILE", "SKHID_PARTICLE_CENSUS"), self.string(args[0])
            if self.string(args[0]) == "SKHID_MEMORY_PROFILE" and self.variable is not None \
                    and len(self.variable) < args[2]:
                emu.mem_write(args[1], self.variable.encode() + b"\0")
                result = len(self.variable)
        elif name == "VirtualAlloc" and self.rooms_left == 0:
            result = 0
        elif name == "VirtualAlloc":
            address_, length, kind, protect = args
            assert (address_, kind, protect) == (0, 0x3000, 4), args
            if self.rooms_left is not None:
                self.rooms_left -= 1
            result = self.room
            self.room += (length + 0xFFFF) & ~0xFFFF
            assert self.room <= ROOM + ROOM_SIZE
            self.reserved.append((result, length))
        elif name in ("GetCurrentThreadId", "GetCurrentProcessId"):
            result = 7
        elif name == "GetTickCount":                 # 250 ms a call: each gate's key poll is due
            self.tick += 250
            result = self.tick
        elif name == "VirtualFree":
            assert args[1:] == [0, 0x8000], args       # MEM_RELEASE of a whole reservation
            self.freed.append(args[0])
            result = 1
        elif name == "GetModuleHandleA":
            result = KERNEL32 if self.string(args[0]).lower() == "kernel32.dll" else 0
        elif name == "LoadLibraryA":
            result = USER32 if self.string(args[0]).lower() == "user32.dll" else 0
        elif name == "GetProcAddress":
            result = self.dynamic.get(self.string(args[1]), 0)
        elif name == "VirtualProtect":
            address_, length, protect, old = args
            self.protects.append((address_, length, protect))
            self.word(old, 0x20)                     # PAGE_EXECUTE_READ, what the exe's code has
            result = 1
        elif name == "FlushInstructionCache":
            self.flushes.append(tuple(args))
            result = 1
        elif name == "GetAsyncKeyState":
            assert args == [0x91], args              # VK_SCROLL
            result = 0x8000 if self.scroll else 0
        elif name == "CreateFileA":
            path = self.string(args[0])
            result = self.files.setdefault(path, 0x100 + 4 * len(self.files))
            self.written.setdefault(result, bytearray())
        elif name == "WriteFile":
            handle, data, length, written, _ = args
            self.written[handle] += bytes(emu.mem_read(data, length))
            if written:
                self.word(written, length)
            result = 1
        elif name == "GetSystemInfo":                # init_mparams: the page and the granularity
            emu.mem_write(args[0], struct.pack("<9I", 0, 0x1000, 0x10000, 0x7FFEFFFF, 1, 1, 586, 0x10000, 0))
        elif name == "CloseHandle":
            result = 1
        elif name == "VirtualQuery":
            at, info, length = args
            assert length == 28
            region = next((r for r in SPACE if r[0] <= at < r[0] + r[1]), None)
            if region is not None:
                region_base, region_size, state, protect, kind = region
                allocation = 0 if state == 0x10000 else region_base
                emu.mem_write(info, struct.pack("<7I", region_base, allocation, protect, region_size, state,
                                                protect, kind))
                result = 28
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + 4 * count)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def call(self, function, args, regs=None, returns=RETURN):
        """Run one function to its return (to `returns`, the caller's): the
        registers after it."""
        sp = STACK + 0x8000
        self.emu.mem_write(sp, struct.pack(f"<{1 + len(args)}I", returns, *args))
        for reg, value in {**KEPT, UC_X86_REG_ESI: 0x05050505, UC_X86_REG_EAX: 0xA0A0A0A0, **(regs or {})}.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.reg_write(UC_X86_REG_EFLAGS, 0x202)
        self.emu.emu_start(function, 0, count=5_000_000)
        assert self.emu.reg_read(UC_X86_REG_EIP) == returns, hex(self.emu.reg_read(UC_X86_REG_EIP))
        return [self.emu.reg_read(reg) for reg in REGS] + [self.emu.reg_read(UC_X86_REG_EFLAGS) & FLAGS]

    def start(self, caller=ALLOCATE_REGION_RETURN):
        """allocate_region's tail, memory.arenas_ready the patched exe's: its
        frame made up (called by engine_world's constructor, or `caller`),
        its three pops and its return."""
        frame = STACK + 0x4000
        self.emu.mem_write(frame - 0x58, struct.pack("<3I", 0x1E1E1E1E, 0x15151515, 0x1B1B1B1B))
        self.emu.mem_write(frame, struct.pack("<2I", 0x0019FF00, caller))
        self.emu.reg_write(UC_X86_REG_EBP, frame)
        self.emu.reg_write(UC_X86_REG_ESP, frame - 0x60)
        self.emu.emu_start(ARENAS_READY, 0, count=60_000_000)
        assert self.emu.reg_read(UC_X86_REG_EIP) == caller
        assert [self.emu.reg_read(r) for r in (UC_X86_REG_EDI, UC_X86_REG_ESI, UC_X86_REG_EBX, UC_X86_REG_EBP)] \
            == [0x1E1E1E1E, 0x15151515, 0x1B1B1B1B, 0x0019FF00]

    def sites(self):
        return {va: bytes(self.emu.mem_read(va, n)) for va, n in ON_DEMAND.items()}

    def heap(self):
        return bytes(self.emu.mem_read(HEAP, HEAP_SIZE))

    def chunk(self, memory):
        return self.read(memory - 4) & ~7

    # the game's calls
    def malloc(self, n, caller=CALLER_B):
        return self.call(MSPACE_MALLOC, [n], {UC_X86_REG_EAX: self.mspace}, caller)

    def realloc(self, memory, n, caller=CALLER_C):
        return self.call(MSPACE_REALLOC, [memory, n], {UC_X86_REG_EAX: self.mspace}, caller)

    def free(self, memory, caller=CALLER_B):
        return self.call(MSPACE_FREE, [], {UC_X86_REG_ESI: self.mspace, UC_X86_REG_EAX: memory}, caller)

    def malloc_impl(self, n, caller=CALLER_A):
        return self.call(MALLOC_IMPL, [0], {UC_X86_REG_ESI: ALLOCATOR, UC_X86_REG_EAX: n}, caller)


# (what, arguments): each result's memory named by the step's index
STEPS = [("malloc", 0x40), ("malloc", 0x3000), ("malloc_impl", 0x200), ("malloc", 0x18), ("free", 1),
         ("malloc_impl", 0x2000), ("realloc", (3, 0x8000)),       # grows: mspace_realloc's own move
         ("malloc", 0x100), ("realloc", (5, 0x80)),                # shrinks: its own cut tail freed
         ("malloc_impl", 0x10), ("free", 0), ("malloc", 0x9000), ("free", 7)]


def play(run, flags=True):
    """Every step: the registers and flags after it and the heap's bytes."""
    memory, out = {}, []
    for index, (what, value) in enumerate(STEPS):
        if what == "free":
            regs = run.free(memory.pop(value))
        elif what == "realloc":
            old, n = value
            regs = run.realloc(memory.pop(old), n)
        else:
            regs = getattr(run, what)(value)
        if what != "free":
            memory[index] = regs[0]
            assert regs[0], (index, what)
        out.append((regs if flags else regs[:-1], run.heap()))
    return out, memory


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    stock = Run(original).sites()

    old = Run(original)
    old.steps = old.gates = 0
    expected, _ = play(old)
    new = Run(bytes(built))
    assert new.sites() == stock, "the built exe keeps the game's bytes where the gates go"
    new.start()
    assert (new.reserved, new.files, new.protects, new.sites()) == ([], {}, [], stock)
    new.steps = new.gates = 0
    got, _ = play(new)
    assert got == expected
    assert (new.steps, new.gates) == (old.steps, old.gates), (old.steps, new.steps)
    print(f"ok the built exe and an unset game: the original's bytes at the {len(ON_DEMAND)} sites, nothing "
          f"reserved, written or logged; every call the original's (registers, flags, the heap) and its "
          f"instructions, as many ({old.steps} over {old.gates} allocator entries)")

    def log_of(run):
        return bytes(run.written[run.files["srp_patches.log"]]).decode() if "srp_patches.log" in run.files else ""

    # refusals leave the game as it was: no gate, no page left writable, no room kept
    elsewhere = Run(bytes(built), "on")
    elsewhere.start(caller=CALLER_A)
    assert (elsewhere.reserved, elsewhere.protects, elsewhere.sites()) == ([], [], stock)
    assert "not at the start; off, nothing written" in log_of(elsewhere), log_of(elsewhere)
    changed = Run(bytes(built), "on")
    changed.emu.mem_write(0x1015E0, b"\xCC")
    changed.start()
    assert (changed.reserved, changed.protects) == ([], [])
    assert "not the game's at 0x001015e0; off, nothing written" in log_of(changed), log_of(changed)
    roomless = Run(bytes(built), "on")
    roomless.rooms_left = 1
    roomless.start()
    assert roomless.sites() == stock and len(roomless.reserved) == 1
    assert roomless.freed == [roomless.reserved[0][0]], (roomless.freed, roomless.reserved)
    assert sorted(roomless.protects) == sorted([(va, n, 0x40) for va, n in ON_DEMAND.items()]
                                               + [(va, n, 0x20) for va, n in ON_DEMAND.items()]), roomless.protects
    assert "no room for its tables; off, nothing written" in log_of(roomless), log_of(roomless)
    got, _ = play(roomless)
    assert got == expected
    print("ok refused (not allocate_region's frame; a site not the game's bytes; no room for the second table): "
          "no gate written, each page made writable given its protection back, the room made given back, one line "
          "logged; every call the original's")

    on = Run(bytes(built), "on")
    on.start()
    lengths = [length for _, length in on.reserved]   # 2 Mi blocks of 8 bytes, 16 Ki sites, the text
    assert len(lengths) == 3 and lengths[0] == (1 << 21) * 8 and lengths[1] % (1 << 14) == 0 \
        and lengths[2] == 1 << 16, on.reserved
    written = on.sites()
    linked = base.arena().symbols
    gates = {0x101330: "memory_profile_malloc", 0x1005A0: "memory_profile_realloc", 0x1015E0: "memory_profile_free",
             0x10700A: "memory_profile_mapped_free", 0x106DAC: "memory_profile_hook_mapped_free",
             OUT_OF_MEMORY: "memory_profile_on_out_of_memory"}
    for va, n in ON_DEMAND.items():
        assert written[va] == base.hook_bytes(linked[gates[va]], va, stock[va], 0, 0xE9), (hex(va), written[va])
    assert sorted(on.protects) == sorted([(va, n, 0x40) for va, n in ON_DEMAND.items()]
                                         + [(va, n, 0x20) for va, n in ON_DEMAND.items()]), on.protects
    assert sorted(on.flushes) == sorted((0xFFFFFFFF, va, n) for va, n in ON_DEMAND.items()), on.flushes
    assert list(on.files) == ["srp_patches.log"], on.files
    log = bytes(on.written[on.files["srp_patches.log"]]).decode()
    assert "memory profile on" in log and "srp_memory_profile.txt" in log, log
    on.start()
    assert on.sites() == written and "not at the start" in log_of(on), "a second start writes nothing more"
    got, live = play(on, flags=False)
    assert got == [(regs[:-1], heap) for regs, heap in expected]
    print(f"ok on: the tables reserved, the {len(ON_DEMAND)} jumps written (each page writable for it, its "
          "protection given back, the cache flushed), one line logged; every call the original's (registers, "
          "the heap)")

    profile = "C:\\Users\\test\\Documents\\Survarium\\srp_memory_profile.txt"
    assert profile not in on.files, "no snapshot without a moment"
    on.scroll = True
    pressed = [on.malloc(0x20, CALLER_B)[0] for _ in range(2)]
    on.scroll = False
    text = bytes(on.written[on.files[profile]]).decode()
    keyed = analyzer().parse(text)[0].snapshots
    assert [s.moment for s in keyed] == ["key"] and keyed[0].complete, text[:300]
    print("ok Scroll Lock down at a gate's poll: one snapshot (not one a poll while it stays down)")

    # what the heap holds: each live block's chunk, by who asked
    sites = {}
    for index, memory in live.items():
        what = STEPS[index][0]
        caller = {"malloc": CALLER_B, "realloc": CALLER_C, "malloc_impl": CALLER_A}[what]
        sites.setdefault(caller, []).append(on.chunk(memory))
    sites[CALLER_B] += [on.chunk(memory) for memory in pressed]
    asked = 0x200000
    sp = STACK + 0x8000
    on.emu.mem_write(sp, struct.pack("<2I", CALLER_A, 0))
    on.emu.reg_write(UC_X86_REG_ESI, ALLOCATOR)
    on.emu.reg_write(UC_X86_REG_EAX, asked)
    on.emu.reg_write(UC_X86_REG_ESP, sp)
    on.emu.emu_start(MALLOC_IMPL, 0, count=60_000_000)
    assert on.emu.reg_read(UC_X86_REG_EIP) == OUT_OF_MEMORY_ON, hex(on.emu.reg_read(UC_X86_REG_EIP))
    text = bytes(on.written[on.files["C:\\Users\\test\\Documents\\Survarium\\srp_memory_profile.txt"]]).decode()
    runs = analyzer().parse(text)
    assert len(runs) == 1 and runs[0].process == 7 and len(runs[0].snapshots) == 2, text[:400]
    run = runs[0]
    assert sorted(run.rooms) == sorted(on.reserved), (run.rooms, on.reserved)
    snapshot = run.snapshots[1]
    assert snapshot.complete and snapshot.moment == "out_of_memory", snapshot
    assert snapshot.detail == {"arena": 0, "asked": asked}, snapshot.detail
    arena = snapshot.arenas[0]
    assert (arena.name, arena.start, arena.size, arena.usage) == ("test heap", HEAP, HEAP_SIZE, "heap"), arena
    in_use = sum(sum(chunks) for chunks in sites.values())
    assert arena.used == in_use, (arena.used, in_use)
    assert arena.tracked.live == in_use and arena.tracked.blocks == len(live) + len(pressed), arena.tracked
    told = {address: c.live for address, c in arena.sites}
    assert told == {caller: sum(chunks) for caller, chunks in sites.items()}, (told, sites)
    assert all(not (base.IMAGE_BASE <= address < 0x1000000) for address in told), "no allocator's own address"
    assert arena.holes and max(arena.holes) == arena.largest_free.bit_length() - 1, (arena.holes, arena.largest_free)
    assert len(snapshot.regions) == len(SPACE) and snapshot.free == sum(r[1] for r in SPACE if r[2] == 0x10000)
    print(f"ok out of memory: the snapshot written before the game's own end ({len(text)} bytes): "
          f"{len(told)} sites, each its caller's live chunks (malloc_impl's caller, a reallocation's), the heap's "
          f"use {in_use} bytes as its walk finds it, {sum(c for c, _ in arena.holes.values())} holes, the "
          f"{asked} bytes asked, {len(snapshot.regions)} regions; the analyzer reads it")


if __name__ == "__main__":
    main()
