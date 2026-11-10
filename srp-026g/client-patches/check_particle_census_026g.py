#!/usr/bin/env python3
"""Patch 138 (particle_census): the census of live particles by effect name,
dormant unless the game starts with SKHID_PARTICLE_CENSUS=1
(docs/notes/client_patch.md, "Particle census").

    python check_particle_census_026g.py <original survarium.exe>

A particle world, four system instances and their emitter instances, a
simulation core, a history item and a bullet manager are made up in memory
as the 0.26g layouts have them (fields_026g.tsv), and the six sites' spans
run in the original and in the patched exe: the starts of
particle_world::tick (0x35AE10), of
particle_system_instance_cook::on_materials_loaded (0x3627F0) and of
game_world_core::atomic_tick (0x2D1E20), move's two loads of a kept state
(0x2D2FE9, 0x2D3568) and bullet_manager::tick past its grid test (0x321D40):
1. the built exe has the original's bytes at the six sites (installed on demand);
2. the variable unset: allocate_region's tail (memory.arenas_ready) protects
   nothing, writes nothing and logs nothing; every span runs the original's
   instructions, as many, with every register, flag and stack word the same;
3. SKHID_PARTICLE_CENSUS=1 but not at the process's start, or a site's bytes
   not the game's: nothing written, one line logged;
4. SKHID_PARTICLE_CENSUS=1: the six jumps written (each page made writable
   and given its protection back, the instruction cache flushed), one line
   logged; every span still leaves every register, flag and stack word the
   original's, and the made-up objects' bytes are never written;
5. three instances cooked (two names, one by the query's resolved path) and
   eleven samples of the world over ten seconds: the report's lines, each
   name's means (the current lod's emitters only, the hidden instance apart),
   the unnamed template as `?`, the creations as rates; and the simulation's
   line: two grid steps and a part, two rollbacks of two and three grid
   steps, a load of a newer state (one of the state's own time is none),
   five and two live bullets at two grid ticks. The bullet rays (counted in
   patch 103's handler of bullet.ray_traced) are not run here: 0.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EFLAGS, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

TICK, TICK_ON = 0x35AE10, 0x35AE16                  # push ebp / mov ebp,esp / and esp,-8, then sub esp,4Ch
LOADED, LOADED_ON = 0x3627F0, 0x3627F8              # sub esp,14h / push ebx / mov ebx,[esp+1Ch], then push ebp
STEP, STEP_ON = 0x2D1E20, 0x2D1E26                  # push ecx / mov eax,51EB851Fh, then mul edi
RELOAD, RELOAD_ON = 0x2D2FE9, 0x2D2FF0              # push 8 / push esi / push ebx / mov [ebp-0Ch],edx, then the call
LATE, LATE_ON = 0x2D3568, 0x2D356E                  # mov ecx,[esi+0C0BB0h], then cmp ecx,[ebx+18FC0h]
BULLETS, BULLETS_ON = 0x321D40, 0x321D47            # mov [esp+2Ch],ecx / sar ebx,2, then test ebx,ebx
ARENAS_READY, ALLOCATE_REGION_RETURN = 0xE12BF, 0x13BF5
ON_DEMAND = {TICK: 6, LOADED: 8, STEP: 6, RELOAD: 7, LATE: 6, BULLETS: 7}
ENTRIES = {TICK: "particle_census_on_tick", LOADED: "particle_census_on_created",
           STEP: "particle_census_on_atomic_tick", RELOAD: "particle_census_on_reload",
           LATE: "particle_census_on_reload_late", BULLETS: "particle_census_on_bullets"}
IMPORTS = {0x8701B4: ("GetEnvironmentVariableA", 3), 0x870118: ("GetCurrentThreadId", 0),
           0x870090: ("GetTickCount", 0), 0x870048: ("CreateFileA", 7), 0x870108: ("WriteFile", 5),
           0x870040: ("CloseHandle", 1), 0x870200: ("VirtualProtect", 4), 0x87003C: ("GetModuleHandleA", 1),
           0x870084: ("GetProcAddress", 2)}
KERNEL32 = 0x7B000000

DATA = 0x30000000
WORLD, RESULTS, QUERY, STRINGS, STUBS = (DATA + 0x1000 * i for i in range(1, 6))
INSTANCES, EMITTERS = DATA + 0x8000, DATA + 0xC000
MANAGER = DATA + 0x7000
CORE, ITEM = 0x32000000, 0x33000000                 # the simulation core (its time +18FC0h), a history item (+0C0BB0h)
FLUSH = STUBS + 0x800
STACK = 0x21000000
OTHER = STUBS + 0x20                                # a caller that is not engine_world's constructor
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP, UC_X86_REG_ESP)
FLAGS = 0x8D5                                       # CF PF AF ZF SF OF
GLOW, MUZZLE, UNNAMED = 0x31000100, 0x31000200, 0x31000300      # templates: identities, never read


class Run:
    def __init__(self, data, variable=None):
        self.variable = variable
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(DATA, 0x10000, UC_PROT_ALL)
        emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
        emu.mem_map(CORE, 0x20000, UC_PROT_ALL)
        emu.mem_map(ITEM, 0x100000, UC_PROT_ALL)
        self.stubs = {FLUSH: ("FlushInstructionCache", 3)}
        for i, (slot, call) in enumerate(sorted(IMPORTS.items())):
            self.stubs[STUBS + 0x100 + 0x10 * i] = call
            self.word(slot, STUBS + 0x100 + 0x10 * i)
        for at in self.stubs:
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=at, end=at)
        for stop in (TICK_ON, LOADED_ON, STEP_ON, RELOAD_ON, LATE_ON, BULLETS_ON, ALLOCATE_REGION_RETURN, OTHER):
            emu.hook_add(UC_HOOK_CODE, self.done, begin=stop, end=stop)
        emu.hook_add(UC_HOOK_CODE, self.step)
        self.steps, self.tick = 0, 0
        self.protects, self.flushes, self.files, self.written = [], [], {}, {}
        self.world()

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def string(self, at):
        out = bytearray()
        while (b := self.emu.mem_read(at + len(out), 1)[0]) != 0:
            out.append(b)
        return out.decode()

    def world(self):
        """The world's ticked list: glow shown (150 particles in two
        emitters), glow hidden on lod 1 (30; its lod 0's 999 not counted),
        muzzle (7), a template never named (1)."""
        def emitters(counts):
            first = 0
            for live in reversed(counts):
                at = EMITTERS + 0x400 * self.made
                self.made += 1
                self.word(at + 0x1EC, first)
                self.word(at + 0x1F0, live)
                first = at
            return first

        self.made = 0
        instances = [(GLOW, 1, 0, {0: [100, 50]}), (GLOW, 0, 1, {0: [999], 1: [30]}),
                     (MUZZLE, 1, 0, {0: [7]}), (UNNAMED, 1, 0, {0: [1]})]
        for index, (template, visible, lod, lods) in enumerate(instances):
            at = INSTANCES + 0x400 * index
            for n in range(10):
                self.word(at + 0x108 + 0x20 * n, template)
            for n, counts in lods.items():
                self.word(at + 0x114 + 0x20 * n, emitters(counts))
            self.word(at + 0x2D4, at + 0x400 if index + 1 < len(instances) else 0)
            self.word(at + 0x2DC, lod)
            self.emu.mem_write(at + 0x303, bytes([visible, 1]))
        self.word(WORLD + 0x19C, INSTANCES)
        self.word(WORLD + 0x1AC, 188)
        self.word(WORLD + 0x1B0, 50000)

    def memory(self):
        return (bytes(self.emu.mem_read(DATA, 0x5000)) + bytes(self.emu.mem_read(MANAGER, 0x9000))
                + bytes(self.emu.mem_read(CORE + 0x18000, 0x2000)) + bytes(self.emu.mem_read(ITEM + 0xC0000, 0x1000)))

    def step(self, emu, address, size, _):
        self.steps += 1

    def done(self, emu, address, size, _):
        emu.emu_stop()

    def stub(self, emu, address, size, _):
        name, count = self.stubs[address]
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 + 4 * i) for i in range(count)]
        result = 0
        if name == "GetEnvironmentVariableA":
            # patch 127 reads its own switch at the same site: never set here
            assert self.string(args[0]) in ("SKHID_MEMORY_PROFILE", "SKHID_PARTICLE_CENSUS"), self.string(args[0])
            if self.string(args[0]) == "SKHID_PARTICLE_CENSUS" and self.variable is not None \
                    and len(self.variable) < args[2]:
                emu.mem_write(args[1], self.variable.encode() + b"\0")
                result = len(self.variable)
        elif name == "GetCurrentThreadId":
            result = 7
        elif name == "GetTickCount":
            result = self.tick
        elif name == "GetModuleHandleA":
            result = KERNEL32 if self.string(args[0]).lower() == "kernel32.dll" else 0
        elif name == "GetProcAddress":
            result = FLUSH if self.string(args[1]) == "FlushInstructionCache" else 0
        elif name == "VirtualProtect":
            address_, length, protect, old = args
            self.protects.append((address_, length, protect))
            self.word(old, 0x20)                     # PAGE_EXECUTE_READ, what the exe's code has
            result = 1
        elif name == "FlushInstructionCache":
            self.flushes.append(tuple(args))
            result = 1
        elif name == "CreateFileA":
            result = self.files.setdefault(self.string(args[0]), 0x100 + 4 * len(self.files))
            self.written.setdefault(result, bytearray())
        elif name == "WriteFile":
            handle, data, length, written, _ = args
            self.written[handle] += bytes(emu.mem_read(data, length))
            if written:
                self.word(written, length)
            result = 1
        elif name == "CloseHandle":
            result = 1
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + 4 * count)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def log(self):
        return bytes(self.written[self.files["srp_patches.log"]]).decode() if "srp_patches.log" in self.files else ""

    def sites(self):
        return {va: bytes(self.emu.mem_read(va, n)) for va, n in ON_DEMAND.items()}

    def start(self, caller=ALLOCATE_REGION_RETURN):
        """allocate_region's tail, memory.arenas_ready the patched exe's: its
        frame made up (called by engine_world's constructor, or `caller`),
        its three pops and its return."""
        frame = STACK + 0x4000
        self.emu.mem_write(frame - 0x58, struct.pack("<3I", 0x1E1E1E1E, 0x15151515, 0x1B1B1B1B))
        self.emu.mem_write(frame, struct.pack("<2I", 0x0019FF00, caller))
        self.emu.reg_write(UC_X86_REG_EBP, frame)
        self.emu.reg_write(UC_X86_REG_ESP, frame - 0x60)
        self.emu.emu_start(ARENAS_READY, 0, count=5_000_000)
        assert self.emu.reg_read(UC_X86_REG_EIP) == caller
        assert [self.emu.reg_read(r) for r in (UC_X86_REG_EDI, UC_X86_REG_ESI, UC_X86_REG_EBX, UC_X86_REG_EBP)] \
            == [0x1E1E1E1E, 0x15151515, 0x1B1B1B1B, 0x0019FF00]

    def span(self, start, stop, stack, regs, pushed=4):
        """A site's span, to the instruction after it: the registers, the
        flags, the `pushed` bytes it left at ESP and the stack from the
        site's ESP up after it."""
        sp = STACK + 0x8004                         # tick pushes EBP onto an 8-aligned place: no padding under it
        self.emu.mem_write(sp - 0x44, bytes(0x84))
        self.emu.mem_write(sp, struct.pack(f"<{len(stack)}I", *stack))
        values = {UC_X86_REG_EAX: 0xA0A0A0A0, UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_ECX: 0x0C0C0C0C,
                  UC_X86_REG_EDX: 0x0D0D0D0D, UC_X86_REG_ESI: 0x05050505, UC_X86_REG_EDI: 0x0E0E0E0E,
                  UC_X86_REG_EBP: 0x0019FF04, UC_X86_REG_ESP: sp, **regs}
        for reg, value in values.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_EFLAGS, 0x202 | FLAGS)
        self.steps = 0
        self.emu.emu_start(start, 0, count=20_000_000)
        assert self.emu.reg_read(UC_X86_REG_EIP) == stop, hex(self.emu.reg_read(UC_X86_REG_EIP))
        # the stack the span wrote (its pushes, at ESP) and all above the site; the room the span
        # only reserves (on_materials_loaded locals) is not yet the function own: a detour may use it
        end = self.emu.reg_read(UC_X86_REG_ESP)
        return ([self.emu.reg_read(reg) for reg in REGS], self.emu.reg_read(UC_X86_REG_EFLAGS) & FLAGS,
                bytes(self.emu.mem_read(end, pushed)) + bytes(self.emu.mem_read(sp, 0x40)), self.steps)

    def atomic_tick(self, now):
        """atomic_tick's start for a step ending at `now` ms."""
        return self.span(STEP, STEP_ON, [0x2D22E7, CORE, 50], {UC_X86_REG_EDI: now})

    def reload(self, late, time, state):
        """One of move's two loads: the core at `time`, the item's state of `state`."""
        self.word(CORE + 0x18FC0, time)
        self.word(ITEM + 0xC0BB0, state)
        regs = {UC_X86_REG_EBX: CORE, UC_X86_REG_ESI: ITEM, UC_X86_REG_EBP: STACK + 0x8024, UC_X86_REG_EDX: state}
        if late:
            return self.span(LATE, LATE_ON, [0] * 8, regs, pushed=0)
        return self.span(RELOAD, RELOAD_ON, [0] * 8, regs, pushed=12)

    def bullets(self, live):
        """bullet_manager::tick past its grid test, `live` bullets in its vector."""
        self.emu.mem_write(MANAGER + 8, struct.pack("<2I", 0x31004000, 0x31004000 + 4 * live))
        return self.span(BULLETS, BULLETS_ON, [0] * 16,
                         {UC_X86_REG_ESI: MANAGER, UC_X86_REG_ECX: MANAGER + 8, UC_X86_REG_EBX: 4 * live}, pushed=0)

    def world_tick(self, at=None):
        """particle_world::tick's start: ECX the world, the delta and the view."""
        if at is not None:
            self.tick = at
        return self.span(TICK, TICK_ON, [0x1F7C00, 0x3C888889, 0x0019FE00], {UC_X86_REG_ECX: WORLD})

    def loaded(self, template, path, emitters, resolved=False):
        """on_materials_loaded's start for a new instance of `template`
        whose query asked for `path`."""
        instance = INSTANCES + 0x4000
        self.word(instance + 0x108, template)
        self.emu.mem_write(STRINGS, path.encode() + b"\0")
        self.emu.mem_write(STRINGS + 0x200, b"never/this\0")
        self.word(QUERY + 0xF8, STRINGS + 0x200 if resolved else STRINGS)
        self.word(QUERY + 0xFC, STRINGS if resolved else 0)
        self.word(RESULTS + 0x20, QUERY)
        self.word(RESULTS + 0x38, emitters)
        return self.span(LOADED, LOADED_ON, [0xE7654, RESULTS, instance, 0x31002000, 0x31003000],
                         {UC_X86_REG_ECX: 0x00AE6000})

    def play(self):
        """Three instances cooked, then eleven samples over ten seconds
        (and ticks between that are no samples): what each start left."""
        out = [self.loaded(GLOW, "particles/fx/artifacts/glow", 3),
               self.loaded(MUZZLE, "particles/fx/muzzle", 2),
               self.loaded(MUZZLE, "particles/fx/muzzle", 2, resolved=True),
               self.atomic_tick(1050), self.atomic_tick(1070), self.atomic_tick(1100),
               self.reload(False, 1100, 1000), self.reload(False, 1100, 980),
               self.reload(True, 1100, 1100), self.reload(True, 1100, 1150),
               self.bullets(5), self.bullets(2)]
        for second in range(11):
            out.append(self.world_tick(5000 + 1000 * second))
            if second < 10:
                out.append(self.world_tick(5016 + 1000 * second))
        return out


EXPECTED = """\
srp: particle census on: a report every 10 s of a particle world's ticks
srp: particle census: world 0x30001000 11 samples in 10.0 s; systems 4.0 (hidden 1.0) emitters 5.0 (hidden 1.0) \
particles 188.0 (hidden 30.0); world particles 188 peak 188 of 50000; made 0.3 systems/s 0.7 emitters/s; 2 names; \
other worlds' ticks skipped 0
srp: particle census: 1. particles/fx/artifacts/glow: particles 180.0 (hidden 30.0) emitters 3.0 systems 2.0 made 0.1/s
srp: particle census: 2. particles/fx/muzzle: particles 7.0 (hidden 0.0) emitters 1.0 systems 1.0 made 0.2/s
srp: particle census: 3. ?: particles 1.0 (hidden 0.0) emitters 1.0 systems 1.0 made 0.0/s
srp: particle census: made 1. particles/fx/muzzle: 0.2 systems/s 0.4 emitters/s, particles 7.0
srp: particle census: made 2. particles/fx/artifacts/glow: 0.1 systems/s 0.3 emitters/s, particles 180.0
srp: particle census: simulation in 10.0 s: steps 3 (grid 2, parts 1); rollbacks 2 re-ran 5 grid steps (max 3); \
other loads 1; bullets 3.5 max 5 over 2 grid ticks; rays 0
"""


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    old = Run(original)
    stock = old.sites()
    expected = old.play()
    made_up = old.memory()              # as the check itself left it (the last query made up): the same in every run
    assert [steps for *_, steps in expected] == [3] * 3 + [2] * 3 + [4] * 2 + [1] * 2 + [2] * 2 + [3] * 21, \
        "each span is its own instructions and no more"

    new = Run(bytes(built))
    assert new.sites() == stock, "the built exe keeps the game's bytes at the six sites"
    new.start()
    assert (new.protects, new.files, new.sites()) == ([], {}, stock)
    assert new.play() == expected and new.memory() == made_up
    for value in ("0", "off", "yes"):
        other = Run(bytes(built), value)
        other.start()
        assert (other.protects, other.files, other.sites()) == ([], {}, stock), value
    print("ok the built exe and an unset or off game: the original's bytes at the six sites, nothing protected, "
          "written or logged; every span the original's instructions, as many (registers, flags, the stack)")

    elsewhere = Run(bytes(built), "1")
    elsewhere.start(caller=OTHER)
    assert (elsewhere.protects, elsewhere.sites()) == ([], stock)
    assert "particle census: not at the start; off, nothing written" in elsewhere.log(), elsewhere.log()
    changed = Run(bytes(built), "1")
    changed.emu.mem_write(LOADED, b"\xCC")
    changed.start()
    assert changed.protects == [] and all(bytes(changed.emu.mem_read(va, n)) == stock[va]
                                          for va, n in ON_DEMAND.items() if va != LOADED)
    assert "not the game's at 0x003627f0; off, nothing written" in changed.log(), changed.log()
    print("ok refused (not allocate_region's frame at the start; a site not the game's bytes): no jump written, "
          "no page touched, one line logged")

    for value in ("1", "on"):
        on = Run(bytes(built), value)
        on.start()
        written, linked = on.sites(), base.arena().symbols
        for va in ON_DEMAND:
            assert written[va] == base.hook_bytes(linked[ENTRIES[va]], va, stock[va], 0, 0xE9), (hex(va), written[va])
        assert sorted(on.protects) == sorted([(va, n, 0x40) for va, n in ON_DEMAND.items()]
                                             + [(va, n, 0x20) for va, n in ON_DEMAND.items()]), on.protects
        assert sorted(on.flushes) == sorted((0xFFFFFFFF, va, n) for va, n in ON_DEMAND.items()), on.flushes
    on.start()
    assert on.sites() == written and "not at the start" in on.log(), "a second start writes nothing more"
    on = Run(bytes(built), "1")
    on.start()
    got = on.play()
    for index, (new_state, old_state) in enumerate(zip(got, expected)):
        assert new_state[:3] == old_state[:3], (index, new_state[:3], old_state[:3])
    assert on.memory() == made_up, "the world, its instances and the query are only read"
    assert on.log() == EXPECTED.replace("\n", "\r\n"), on.log()
    print("ok on: the six jumps written (each page writable for it, its protection given back, the cache "
          "flushed), one line logged; three instances cooked, the simulation's steps, loads and bullet ticks "
          "and eleven samples over ten seconds leave every register, flag and stack word the original's and "
          "the made-up objects unwritten; the report: the totals, glow 180.0 particles (30.0 hidden, lod 1's "
          "emitter alone), muzzle 7.0, the unnamed template as ?, the creations as rates, the simulation's "
          "line (3 steps, 2 rollbacks re-running 5 grid steps, 1 other load, bullets 3.5 max 5)")


if __name__ == "__main__":
    main()
