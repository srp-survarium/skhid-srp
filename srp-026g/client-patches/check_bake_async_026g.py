#!/usr/bin/env python3
"""Run patch 111 (the Rust patch bake_async: the camouflage/decal bake's GPU
work not waited for) in the default build, from the game's own code, on a
made-up renderer, D3D11 context and bakes.

    python check_bake_async_026g.py <original survarium.exe>

Requires Python `unicorn` (2.1.4) and the arena's tools (`nix develop
./srp-026g`). The exe's sections are loaded as they are (check_bake_cache_026g's
Game: the baker's list, patch 70's sites, kernel32 through the IAT). Made up:
the D3D11 immediate context (Map, Unmap, Flush: a staging maps once its
"GPU" has done it, or with D3D11_MAP_FLAG_DO_NOT_WAIT gives 0 and
DXGI_ERROR_WAS_STILL_DRAWING), the textures and render targets the
resource manager makes, the draws (apply_pass, fill_surface: nothing), and
the tail's string and path helpers. Run for real: process_compression from
its entry (its prologue, the texture getters, then bake.issue), the
compressor's block_mip_count, data_size, write_dds_header, make_mips and
compress_mip (with compress_mip.readback), map2D, the dispatch's end
(resources.dispatched) and process_compression again (bake.resume, its
tail from 0x12F00B: the texture's slot, patch 70's sharing, the resource,
the save descriptor (bake.save), the CRC, the record, bake.free, finish).

- issued: process_compression returns (ret 28h) with its query not
  finished, no Map at all, level 0 a DEFAULT texture with no initial data
  and the target copied into it on the GPU, backend::reset once before
  make_mips (as 0x11F3F0 at 0x11F4D7), the levels drawn, each level's
  staging copied and kept; the original maps 1 + the levels, blocking;
- polled with the GPU busy: one Map with the no-wait flag, nothing else;
- polled with the GPU done (the resumed frame's stack filled with 0CDh): the
  tail's compress ms the real timer's (elapsed_ms 0x106A00 and elapsed_ticks
  0x1B7B0, fs:[2Ch] a made-up GDT's segment, the scale 1.0 the resume
  writes), each level read into the DDS at its place,
  each staging unmapped and let go, the texture made from that DDS (its
  name, size, pair and value), in the surface's slot, the query given its
  resource and finished with success, the save's descriptor owning that
  DDS (no copy: make_save_descriptor not called, the descriptor's +8 1),
  the record listed with the DDS's CRC, the DDS not freed to the render
  allocator, the model's reference let go;
- patch 70's waiter (a second query of the path, looked up while the bake
  is pending): it waits, and gets the texture with the owner;
- late: busy past 1 s, read back waiting, a log line;
- lost: the blocking Map fails: the query finished with an error (1, 1,
  0Bh), patch 70 makes its waiter again, the DDS freed (MT), the model let
  go, a log line;
- the places: 8 bakes in flight, the next 8 queued (level 0 kept, nothing
  compressed), the 17th the stock bake (its blocking Maps); the 8 done
  finished 2 a dispatch, oldest first, and 2 queued compressed in their
  places each time;
- at most 2 finished a dispatch: 5 done at once finish 2, 2, 1 over three
  dispatches, oldest first; a done bake behind a busy one waits for it and
  finishes after it;
- game::exit: every pending bake finished, the queued ones too.
"""
import struct
import sys

from unicorn import UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_FS, UC_X86_REG_GDTR, \
    UC_X86_REG_SS, UC_X86_REG_DS, UC_X86_REG_ES

import check_bake_cache_026g as cache
import patch_026g as base

PROCESS, ISSUE_SITE, EPILOGUE_RET = 0x12EC40, 0x12EEA7, 0x12F59F
DISPATCH_END, EXIT_SITE, EXIT_PAST = 0x1024FA, 0x1EC82B, 0x1EC837
DEVICE_GLOBAL, MANAGER_GLOBAL, RENDERER_GLOBAL, BACKEND_GLOBAL = 0x2260478, 0x226047C, 0x2260480, 0x2260490
RENDER_ALLOCATOR, MT_TRACKING, MT_SLOT, NO_MEMORY = 0x1F1D960, 0xA8002C, 0x1F1D308, 0x1F1D61B
CREATE_TEXTURE, CREATE_TARGET, RELEASE_TARGET, COPY2D = 0x3BEA70, 0x3BF930, 0x3BFBC0, 0x3C0E00
APPLY_PASS, BIND, CONSTANTS, FILL, RESET = 0x3D7230, 0x3D6960, 0x132A90, 0x11D950, 0x11B740
APPENDF, PHYSICAL, CONVERT, FROM_MEMORY = 0xE4590, 0x1051E0, 0x12780, 0x3BD030
MT_ALLOC, MT_FREE, RENDER_FREE, MAKE_SAVE, TRIM = 0x106EF0, 0x106FC0, 0x1539D0, 0x109ED0, 0x131F90
BLOCK_FREE = 0x1015E0
QPC, QPF, THREAD_ID = 0x870158, 0x870150, 0x870118
SAVE_COUNTER = 0x1B3A2AC
WAS_STILL_DRAWING, DEVICE_REMOVED = 0x887A000A, 0x887A0005
NO_WAIT = 0x100000
HEAP = 0x50000000
TEB = 0x60000000                   # the thread's block: fs:[2Ch] its TLS array (elapsed_ms's ticks a ms)
TICKS_A_MS = 1000                  # QueryPerformanceFrequency's 1e6 / 1000
COMPRESS_TIMED = 0x12F014          # past the tail's `fstp dword [esp+48h]`: the compress ms
ELAPSED_QPC_BACK = 0x1B7C3        # elapsed_ticks's QueryPerformanceCounter returns here
NAME = "renesanse_heavy_com_tank_01_torso_s1d"
PATH = cache.PREFIX + NAME + ".dds"


class Game(cache.Game):
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_map(HEAP, 0x4000000, UC_PROT_ALL)
        self.free_at = HEAP
        self.qpc = 1_000_000
        self.log = []
        self.d3d = []                      # (what, ...) of the context's calls
        self.done = set()                  # stagings the "GPU" has done
        self.lost = False
        self.textures = {}                 # texture -> {"usage", "width", "height", "data"}
        self.tracked = {}
        # the globals the bake's code reads
        self.device, self.context = self.new(0x200), self.new(0x40)
        self.word(self.device + 0x180, self.context)
        self.word(DEVICE_GLOBAL, self.device)
        self.word(MANAGER_GLOBAL, self.new(0x100))
        self.renderer = self.new(0x200)
        self.effect = self.new(0x5700)
        self.word(self.effect + 0xD0, 5)
        self.word(self.renderer + 0x64, self.effect)
        self.word(self.renderer + 0x60, self.effect)
        self.word(RENDERER_GLOBAL, self.renderer)
        self.word(BACKEND_GLOBAL, self.new(0x3000))
        self.render_allocator = self.new(0x100)
        self.word(RENDER_ALLOCATOR, self.render_allocator)
        self.word(MT_SLOT, 0xA8001C)
        self.word(MT_TRACKING, 0)
        self.emu.mem_write(NO_MEMORY, b"\0")
        # the context's vtable and a resource's (GetType +1Ch)
        vtable = self.new(0x400)
        self.word(self.context, vtable)
        names = {0x38: "Map", 0x3C: "Unmap", 0x1BC: "Flush"}
        for slot, name in names.items():
            stub = cache.STUBS + 0x800 + slot
            self.word(vtable + slot, stub)
            self.stubs[stub] = name
        self.resource_vtable = self.new(0x40)
        self.word(self.resource_vtable + 0x1C, cache.STUBS + 0x700)
        self.stubs[cache.STUBS + 0x700] = "GetType"
        # the thread's block for elapsed_ms (0x106A23: fs:[2Ch], the TLS array, its first block +8): FS a
        # data segment of a made-up GDT
        self.emu.mem_map(TEB, 0x3000, UC_PROT_ALL)
        self.word(TEB + 0x2C, TEB + 0x100)
        self.word(TEB + 0x100, TEB + 0x200)
        self.word(TEB + 0x208, TICKS_A_MS)
        self.word(TEB + 0x20C, 0)
        gdt = TEB + 0x2000
        self.emu.mem_write(gdt, bytes(8) + struct.pack("<HHBBBB", 0xFFFF, TEB & 0xFFFF, (TEB >> 16) & 0xFF, 0x93,
                                                         0xCF, (TEB >> 24) & 0xFF)
                           + struct.pack("<HHBBBB", 0xFFFF, 0, 0, 0x93, 0xCF, 0))   # 2: flat, for SS, DS and ES
        self.emu.reg_write(UC_X86_REG_GDTR, (0, gdt, 23, 0))
        self.emu.reg_write(UC_X86_REG_FS, 1 << 3)
        for segment in (UC_X86_REG_SS, UC_X86_REG_DS, UC_X86_REG_ES):
            self.emu.reg_write(segment, 2 << 3)
        self.qpcs = []                     # (return address, value) of each QueryPerformanceCounter
        self.timed = []                    # (the compress ms, its timer's start, scale) at 0x12F014
        self.emu.hook_add(UC_HOOK_CODE, self.compress_timed, begin=COMPRESS_TIMED, end=COMPRESS_TIMED)
        for va, name in ((CREATE_TEXTURE, "create texture"), (CREATE_TARGET, "create target"),
                         (RELEASE_TARGET, "release target"), (COPY2D, "copy2D"), (APPLY_PASS, "apply pass"),
                         (BIND, "bind"), (CONSTANTS, "constants"), (FILL, "fill"), (RESET, "reset"),
                         (APPENDF, "appendf"), (PHYSICAL, "physical"), (CONVERT, "convert"),
                         (FROM_MEMORY, "from memory"), (MT_ALLOC, "mt alloc"), (MT_FREE, "mt free"),
                         (RENDER_FREE, "render free"), (MAKE_SAVE, "make save"), (TRIM, "trim"),
                         (BLOCK_FREE, "block free")):
            self.stubs[va] = name
        for slot, name in ((QPC, "QueryPerformanceCounter"), (QPF, "QueryPerformanceFrequency"),
                           (THREAD_ID, "GetCurrentThreadId")):
            stub = cache.STUBS + 0x900 + 0x10 * len(self.tracked)
            self.tracked[name] = stub
            self.word(slot, stub)
            self.stubs[stub] = name

    def compress_timed(self, emu, address, size, user):
        sp = emu.reg_read(UC_X86_REG_ESP)
        start = self.read(sp + 0x58) | self.read(sp + 0x5C) << 32
        ms, scale = (struct.unpack("<f", emu.mem_read(sp + at, 4))[0] for at in (0x48, 0x60))
        self.timed.append((ms, start, scale))

    def new(self, size):
        at, self.free_at = self.free_at, self.free_at + ((size + 0xF) & ~0xF)
        self.emu.mem_write(at, bytes(size))
        return at

    def text(self, at):
        return self.ascii(at, 0x200)

    def buffer_string(self, at):
        begin, end = self.read(at), self.read(at + 4)
        return bytes(self.emu.mem_read(begin, end - begin)).decode("latin-1")

    def set_buffer_string(self, at, text):
        begin = self.read(at)
        raw = text.encode("latin-1")
        self.emu.mem_write(begin, raw + b"\0")
        self.word(at + 4, begin + len(raw))

    def texture(self, width, height, usage):
        t = self.new(0x200)
        resource = self.new(0x20)
        self.word(resource, self.resource_vtable)
        self.word(t + 0x1AC, resource)
        self.emu.mem_write(t + 0x1BC, bytes([1 if usage == 3 else 0]))
        self.textures[t] = {"usage": usage, "width": width, "height": height, "resource": resource,
                            "data": None, "copied_from": None}
        return t

    def code(self, emu, address, size, user):
        name = self.stubs.get(address)
        if name is None or name in cache.POPS or name in ("IsDebuggerPresent", "wipe", "translate", "finish",
                                                           "resource", "set resource",
                                                           "release texture", "destroy model", "crc",
                                                           "log filter", "assertion"):
            if name in ("CreateFileA", "WriteFile", "CloseHandle") and self.logging(name):
                return
            return super().code(emu, address, size, user)
        sp = emu.reg_read(UC_X86_REG_ESP)
        arg = lambda n: self.read(sp + 4 + 4 * n)
        reg = emu.reg_read
        self.calls.append((name,))
        if name == "allocate":                       # esi the allocator, eax the size, the name; `ret 4`
            at = self.new(max(reg(UC_X86_REG_EAX), 0x10))
            self.calls[-1] = (name, reg(UC_X86_REG_ESI), reg(UC_X86_REG_EAX), arg(0))
            self.returns(at, 4)
        elif name == "create texture":                 # eax the width; manager, name, height, initial, format, usage, mips, bind
            t = self.texture(reg(UC_X86_REG_EAX), arg(2), arg(5))
            self.calls[-1] = (name, reg(UC_X86_REG_EAX), arg(2), arg(3), arg(4), arg(5), t)
            self.returns(t, 0x20)
        elif name == "create target":                # manager, name, width, height, format, ...: the target, its texture +1Ch
            target = self.new(0x40)
            self.word(target + 0x1C, self.texture(arg(2), arg(3), 0))
            self.calls[-1] = (name, arg(2), arg(3), arg(4))
            self.returns(target, 0x28)
        elif name == "copy2D":                       # eax width, edx height; destination, source
            self.textures.get(arg(0), {})["copied_from"] = arg(1)
            self.calls[-1] = (name, arg(0), arg(1), reg(UC_X86_REG_EAX), reg(UC_X86_REG_EDX))
            self.returns(0, 8)
        elif name in ("release target",):
            self.returns(0, 8)
        elif name == "apply pass":
            self.returns(1)
        elif name == "bind":
            self.returns(1, 0xC)
        elif name == "constants":
            self.returns(0, 4)
        elif name == "fill":
            self.returns(0, 0x1C)
        elif name == "reset":                        # edi the backend
            self.calls[-1] = (name, reg(UC_X86_REG_EDI))
            self.returns(0)
        elif name == "appendf":                      # eax the buffer_string; the format, its strings
            fmt = self.text(arg(0))
            out, n = "", 1
            i = 0
            while i < len(fmt):
                if fmt[i:i + 2] == "%s":
                    out += self.text(arg(n))
                    n += 1
                    i += 2
                else:
                    out += fmt[i]
                    i += 1
            self.set_buffer_string(reg(UC_X86_REG_EAX), out)
            self.returns(0)
        elif name == "physical":                     # eax the virtual path, ecx the physical out
            self.set_buffer_string(reg(UC_X86_REG_ECX), cache.FOLDER + "\\" + self.buffer_string(reg(UC_X86_REG_EAX))
                                   .rsplit("/", 1)[1])
            self.returns(0)
        elif name == "convert":                      # the descriptor's other path, into the span at [esp+4]
            span = arg(0)
            self.word(span, reg(UC_X86_REG_EDX))
            self.set_buffer_string(span, PATH)
            self.returns(0, 8)
        elif name == "from memory":                  # edi the name; manager, data, size, pair, value
            t = self.texture(0, 0, 0)
            self.calls[-1] = (name, self.text(reg(UC_X86_REG_EDI)), arg(1), arg(2), arg(3), arg(4), arg(5),
                              bytes(emu.mem_read(arg(1), arg(2))), t)
            self.returns(t, 0x18)
        elif name == "mt alloc":                     # eax the size
            at = self.new(reg(UC_X86_REG_EAX) + 0x10)
            self.calls[-1] = (name, reg(UC_X86_REG_EAX), at)
            self.returns(at)
        elif name == "mt free":
            self.calls[-1] = (name, reg(UC_X86_REG_EAX))
            self.returns(0)
        elif name == "render free":                  # ecx the allocator, eax the memory, the pointer
            self.calls[-1] = (name, reg(UC_X86_REG_EAX))
            self.returns(0)
        elif name == "block free":
            self.returns(0)
        elif name == "make save":                    # ecx, eax the paths; the bytes: the game's own copy
            self.calls[-1] = (name, self.read(arg(0)))
            self.returns(self.new(0x20))
        elif name == "trim":
            self.returns(0, 4)
        elif name == "GetType":                      # this, &type
            self.word(arg(1), 3)
            self.returns(0, 8)
        elif name == "Map":                          # this, resource, subresource, type, flags, &mapped
            staging = next(t for t, info in self.textures.items() if info["resource"] == arg(1))
            self.calls[-1] = (name, staging, arg(4))
            if self.lost or (arg(4) & NO_WAIT and staging not in self.done):
                self.word(arg(5), 0)
                self.returns(DEVICE_REMOVED if self.lost else WAS_STILL_DRAWING, 0x18)
                return
            self.done.add(staging)
            info = self.textures[staging]
            if info["data"] is None:                 # a level's blocks: a pattern of its own
                pitch = info["width"] * 16 + 16
                rows = bytearray()
                for r in range(info["height"]):
                    rows += bytes(((staging >> 4) + r * 7 + i) & 0xFF for i in range(pitch))
                info["data"], info["pitch"] = self.new(len(rows)), pitch
                self.emu.mem_write(info["data"], bytes(rows))
            self.word(arg(5), info["data"])
            self.word(arg(5) + 4, info["pitch"])
            self.returns(0, 0x18)
        elif name == "Unmap":
            self.calls[-1] = (name, next(t for t, i in self.textures.items() if i["resource"] == arg(1)))
            self.returns(0, 0xC)
        elif name == "Flush":
            self.returns(0, 4)
        elif name == "QueryPerformanceCounter":
            self.qpc += 1000
            self.qpcs.append((self.read(sp), self.qpc))
            self.word(arg(0), self.qpc & 0xFFFFFFFF)
            self.word(arg(0) + 4, self.qpc >> 32)
            self.returns(1, 4)
        elif name == "QueryPerformanceFrequency":
            self.word(arg(0), 1_000_000)
            self.word(arg(0) + 4, 0)
            self.returns(1, 4)
        elif name == "GetCurrentThreadId":
            self.returns(0x1D0C)
        else:
            raise AssertionError(name)

    def logging(self, name):
        """srp_patches.log's lines (vostok::log): taken here."""
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        arg = lambda n: self.read(sp + 4 + 4 * n)
        if name == "CreateFileA" and self.text(arg(0)) == "srp_patches.log":
            self.returns(0x7106, 28)
            return True
        if name == "WriteFile" and arg(0) == 0x7106:
            self.log.append(bytes(self.emu.mem_read(arg(1), arg(2))).decode("latin-1").strip())
            self.word(arg(3), arg(2))
            self.returns(1, 20)
            return True
        if name == "CloseHandle" and arg(0) == 0x7106:
            self.returns(1, 4)
            return True
        return False


class Bake:
    """A made-up bake request: its query, surface, model, parameters, targets."""
    def __init__(self, game, kind=0, size=64, name=NAME):
        self.game, self.kind, self.size, self.name = game, kind, size, name
        g = game
        self.query, self.surface = g.new(0x400), g.new(0x40)
        self.model = g.new(0x100)
        g.word(self.model + 0xD0, 5)
        self.parameters = g.new(0x800)
        g.word(self.parameters + 0x7A0, kind)
        g.word(self.parameters + 0x790, self.model)
        self.copy = g.new(0x200)                     # on_textures_loaded's copy: +13Ch the name, inline
        g.emu.mem_write(self.copy + 0x150, name.encode() + b"\0")
        g.word(self.copy + 0x13C, self.copy + 0x150)
        self.pair = g.new(8)
        g.word(self.pair, 0x5A1)
        g.word(self.pair + 4, 0x5A2)
        self.targets = g.new(0x10)                   # the result_struct: a target per type, its texture +1Ch
        self.source = g.texture(2048, 2048, 0)
        g.word(self.source + 4, 1)
        for k in range(4):
            target = g.new(0x40)
            g.word(target + 0x1C, self.source)
            g.word(self.targets + 4 * k, target)
        self.extent = g.new(8)
        g.word(self.extent, size)
        g.word(self.extent + 4, size)
        self.caller = g.new(0x40)                    # on_textures_loaded's frame: [ebp+18h] the parameters
        g.word(self.caller + 0x18, self.parameters)
        levels = 0
        while (size // 4) >> levels:
            levels += 1
        self.levels = levels
        block = 16 if kind == 1 else 8
        data = sum(max(4, size >> i) ** 2 // 16 * block for i in range(levels))
        self.dds_size = 0x80 + data + 1


def process(game, bake):
    """process_compression from its entry, as on_textures_loaded calls it,
    to its return: (back with ret 28h and the registers kept, calls)."""
    g = game
    sp = cache.STACK + 0xA0000
    args = (cache.COOK, bake.query, bake.targets, bake.dds_size, bake.surface, bake.copy, bake.pair, 0x7A1,
            bake.kind, struct.unpack("<I", struct.pack("<f", 2.5))[0])
    g.word(sp, cache.STOP)
    for i, value in enumerate(args):
        g.word(sp + 4 + 4 * i, value)
    before = len(g.calls)
    regs = {UC_X86_REG_EAX: bake.extent, UC_X86_REG_EBX: 0xB1, UC_X86_REG_ESI: 0x51, UC_X86_REG_EDI: 0xD1,
            UC_X86_REG_EBP: bake.caller}
    eip, esp = g.run(PROCESS, sp, regs, count=20_000_000)
    kept = all(g.emu.reg_read(r) == v for r, v in regs.items() if r != UC_X86_REG_EAX)
    return eip == cache.STOP and esp == sp + 4 + 40 and kept, g.calls[before:]


def dispatched(game):
    """resources_manager::dispatch_callbacks from its common end to its `ret 4`."""
    g = game
    ebp = cache.STACK + 0xC0000
    sp = ebp - 0x40
    for at, value in ((sp, 0xED1), (sp + 4, 0xE51), (sp + 8, 0xEB1), (ebp, 0xEB9), (ebp + 4, cache.STOP)):
        g.word(at, value)
    before = len(g.calls)
    eip, esp = g.run(DISPATCH_END, sp, {UC_X86_REG_EBP: ebp}, count=40_000_000)
    regs = [g.emu.reg_read(r) for r in (UC_X86_REG_EDI, UC_X86_REG_ESI, UC_X86_REG_EBX, UC_X86_REG_EBP)]
    return eip == cache.STOP and esp == ebp + 12 and regs == [0xED1, 0xE51, 0xEB1, 0xEB9], g.calls[before:]


def exiting(game):
    """game::exit from 0x1EC82B to past its (nopped) delete."""
    g = game
    sp = cache.STACK + 0xD0000
    before = len(g.calls)
    eip, esp = g.run(EXIT_SITE, sp, {UC_X86_REG_EBX: 0xB1, UC_X86_REG_EDI: 0xD1, UC_X86_REG_EBP: 0xEB9},
                     until=[EXIT_PAST], count=80_000_000)
    return eip == EXIT_PAST and esp == sp, g.calls[before:]


def names(calls, what):
    return [c for c in calls if c[0] == what]


def finished_queries(calls):
    """The queries finished with success, in order."""
    return [c[1] for c in names(calls, "finish") if c[2:] == (3, 1, 0)]


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    print("the original: a bake reads back, blocking")
    game = Game(bytes(original))
    bake = Bake(game)
    ok, calls = process(game, bake)
    maps = names(calls, "Map")
    report(ok and len(maps) == 1 + bake.levels and all(flags == 0 for _, _, flags in maps)
           and names(calls, "finish") == [("finish", bake.query, 3, 1, 0)],
           f"the original maps {len(maps)} times (the target, then {bake.levels} levels), each blocking, and "
           "finishes the query")

    print("issued, polled busy, polled done (the build)")
    game = Game(bytes(built))
    bake = Bake(game)
    owner = game.lookup(PATH, bake.query, bake.surface, bake.parameters, bake.model)
    waiter_query, waiter_surface, waiter_parameters, waiter_model = cache.Objects(game).request(0)
    waiter = game.lookup(PATH, waiter_query, waiter_surface, waiter_parameters, waiter_model)
    report(owner[0] == "on" and waiter[0] == "waits", "patch 70: the bake's query owns the path, a second waits")
    ok, calls = process(game, bake)
    textures = names(calls, "create texture")
    level0 = textures[0] if textures else None
    stagings = [c[6] for c in textures if c[5] == 3]
    copies = names(calls, "copy2D")
    report(ok and not names(calls, "Map") and not names(calls, "finish") and game.read(bake.model + 0xD0) == 6,
           "process_compression returns (ret 28h, registers kept): no Map, its query not finished, the model "
           "held (6 references)")
    report(level0 is not None and level0[1:6] == (bake.size, bake.size, 0, 0x1C, 0)
           and copies and copies[0][1:3] == (level0[6], bake.source),
           "level 0: a DEFAULT R8G8B8A8 texture of the bake's size, no initial data, the target copied into it "
           "on the GPU (copy2D)")
    order = [c[0] for c in calls]
    resets = names(calls, "reset")
    report(resets == [("reset", game.read(BACKEND_GLOBAL))] and level0 is not None
           and calls.index(level0) < order.index("reset") < order.index("create target"),
           "backend::reset (EDI the backend [2260490h]) once, after level 0 and before make_mips, as 0x11F3F0 at "
           "0x11F4D7")
    report(len(stagings) == bake.levels and len(names(calls, "create target")) == 2 * bake.levels - 1,
           f"the levels drawn (make_mips: {bake.levels - 1} targets) and compressed ({bake.levels} block targets, "
           f"{len(stagings)} stagings copied to, none mapped)")
    ok, calls = dispatched(game)
    maps = names(calls, "Map")
    report(ok and maps == [("Map", stagings[-1], NO_WAIT)] and not names(calls, "Unmap")
           and not names(calls, "finish"),
           "a dispatch, the GPU busy: one Map of the last staging with the no-wait flag (0 back), nothing else")
    game.done.update(stagings)
    # the resumed frame's stack garbage: a local the resume does not write shows
    game.emu.mem_write(cache.STACK + 0xB0000, b"\xCD" * 0xFF00)
    ok, calls = dispatched(game)
    timer_reads = [value for back, value in game.qpcs if back == ELAPSED_QPC_BACK]
    if game.timed and timer_reads:
        ms, start, scale = game.timed[-1]
        want = (timer_reads[-1] - start) * 1.0 / TICKS_A_MS
        report(scale == 1.0 and ms == want and 0 < ms < 1000,
               f"the tail's compress ms (elapsed_ms 0x106A00 and elapsed_ticks 0x1B7B0 for real, on the fake "
               f"QueryPerformanceCounter): {ms} = (its read {timer_reads[-1]} - the readback's start {start}) x the "
               f"scale {scale} / {TICKS_A_MS}")
    else:
        report(False, "the tail's compress ms: the timer ran (0x12F014 reached, 0x1B7B0 read the counter)")
    maps, unmaps = names(calls, "Map"), names(calls, "Unmap")
    made = names(calls, "from memory")
    report(ok and maps[0] == ("Map", stagings[-1], NO_WAIT) and sorted(m[1] for m in maps[1:]) == sorted(stagings)
           and len(unmaps) == len(maps),
           f"a dispatch, the GPU done: the last staging asked without waiting, then each level mapped and "
           f"unmapped ({len(maps)} Maps, {len(unmaps)} Unmaps)")
    dds = made[0][7] if made else b""
    expected = True
    if made:
        offset = 0x80
        for i, staging in enumerate(stagings):
            info = game.textures[staging]
            row = info["width"] * 8
            blocks = bytes(game.emu.mem_read(info["data"], info["pitch"] * info["height"]))
            want = b"".join(blocks[r * info["pitch"]:r * info["pitch"] + row] for r in range(info["height"]))
            expected &= dds[offset:offset + len(want)] == want
            offset += len(want)
        expected &= dds[:4] == b"DDS " and len(dds) == bake.dds_size and dds[-1] == 1
    report(made and made[0][1] == PATH[:-4] and made[0][3:6] == (bake.dds_size, 0x5A1, 0x5A2) and expected,
           "the texture made from the DDS (create_texture_from_memory: its name, size, pair; the DDS's header, "
           "each level's rows from its staging, the diffuse byte last)")
    texture = made[0][8] if made else 0
    finished = names(calls, "finish")
    report(game.read(bake.surface) == texture and game.read(waiter_surface) == texture
           and ("finish", bake.query, 3, 1, 0) in finished and ("finish", waiter_query, 3, 1, 0) in finished,
           "the texture in the surface's slot and the waiter's; both queries finished with success")
    descriptor = game.read(bake.query + 0xE0)
    report(descriptor and game.read(descriptor) == made[0][2] and game.read(descriptor + 4) == bake.dds_size
           and game.emu.mem_read(descriptor + 8, 1)[0] == 1 and not names(calls, "make save")
           and game.read(SAVE_COUNTER) == 1,
           "the save's descriptor (query +0E0h) owns that DDS (its data, its size, +8 1): no copy "
           "(make_save_descriptor not called), the generated saves 1")
    records = game.records()
    report([r[0] for r in records] == [PATH] and records[0][2] == bake.dds_size
           and not names(calls, "render free") and not names(calls, "mt free"),
           "the record listed (its path, size); the DDS neither freed to the render allocator nor to MT")
    report(game.read(bake.model + 0xD0) == 5 and all(game.read(s + 4) == 0 for s in stagings),
           "the model's reference let go (5), every staging let go (0)")

    print("late, lost")
    game = Game(bytes(built))
    bake = Bake(game)
    process(game, bake)
    game.qpc += 2_000_000
    ok, calls = dispatched(game)
    report(ok and names(calls, "from memory") and any("1 s" in line for line in game.log),
           "busy 2 s: read back waiting, finished, logged: " + "; ".join(game.log))
    game = Game(bytes(built))
    bake = Bake(game)
    game.lookup(PATH, bake.query, bake.surface, bake.parameters, bake.model)
    w_query, w_surface, w_parameters, w_model = cache.Objects(game).request(0)
    game.lookup(PATH, w_query, w_surface, w_parameters, w_model)
    process(game, bake)
    game.lost = True
    game.qpc += 2_000_000
    ok, calls = dispatched(game)
    report(ok and ("finish", bake.query, 1, 1, 0xB) in names(calls, "finish")
           and ("translate", cache.COOK, w_query) in names(calls, "translate")
           and len(names(calls, "mt free")) == 1 and game.read(bake.model + 0xD0) == 5
           and any("could not be read back" in line for line in game.log),
           "the device lost: the query finished with an error (1, 1, 0Bh), patch 70 makes the waiter again, the "
           "DDS freed (MT), the model let go, logged")

    print("the places, game::exit")
    game = Game(bytes(built))
    bakes = [Bake(game, name=f"{NAME}_{i}") for i in range(17)]
    results = [process(game, b) for b in bakes]
    stock = results[16][1]
    queued = results[8][1]
    report(all(ok for ok, _ in results) and not names(queued, "create target") and len(names(queued, "copy2D")) == 1
           and len(names(stock, "Map")) == 1 + bakes[16].levels
           and ("finish", bakes[16].query, 3, 1, 0) in names(stock, "finish"),
           "8 in flight; the 9th..16th queued (level 0 copied, nothing drawn); the 17th the stock bake (its "
           f"{len(names(stock, 'Map'))} blocking Maps, finished)")
    stock_descriptor = game.read(bakes[16].query + 0xE0)
    report(names(stock, "make save") and stock_descriptor,
           "the stock bake's save: make_save_descriptor, as the original")
    for t, info in game.textures.items():
        if info["usage"] == 3:
            game.done.add(t)
    rounds = [dispatched(game) for _ in range(4)]
    report(all(ok for ok, _ in rounds)
           and [finished_queries(calls) for _, calls in rounds] == [[b.query for b in bakes[i:i + 2]] for i in (0, 2, 4, 6)]
           and all(len(names(calls, "create target")) == 2 * (2 * bakes[0].levels - 1) for _, calls in rounds),
           "four dispatches, the GPU done: 2 of the 8 in flight finished each, oldest first, and 2 queued drawn and "
           "compressed in their places")
    ok, calls = exiting(game)
    finished = [c[1] for c in names(calls, "finish")]
    report(ok and all(b.query in finished for b in bakes[8:16]) and any("exit" in line for line in game.log),
           "game::exit: every pending bake finished (" + f"{len(finished)} queries), logged")

    print("at most 2 finished a dispatch, oldest first")
    game = Game(bytes(built))
    bakes = [Bake(game, name=f"{NAME}_{i}") for i in range(5)]
    staged = []
    for b in bakes:
        before = set(game.textures)
        process(game, b)
        staged.append([t for t in game.textures if t not in before and game.textures[t]["usage"] == 3])
    for stagings in staged:
        game.done.update(stagings)
    rounds = [dispatched(game) for _ in range(4)]
    report(all(ok for ok, _ in rounds)
           and [finished_queries(calls) for _, calls in rounds]
           == [[bakes[0].query, bakes[1].query], [bakes[2].query, bakes[3].query], [bakes[4].query], []],
           "5 done at once: 2, 2, 1 over three dispatches (oldest first), none left for the fourth")
    game = Game(bytes(built))
    first, second = Bake(game, name=NAME + "_a"), Bake(game, name=NAME + "_b")
    process(game, first)
    before = set(game.textures)
    process(game, second)
    game.done.update(t for t in game.textures if t not in before and game.textures[t]["usage"] == 3)
    ok_busy, busy = dispatched(game)
    game.done.update(t for t, info in game.textures.items() if info["usage"] == 3)
    ok_done, done = dispatched(game)
    report(ok_busy and ok_done and finished_queries(busy) == [] and len(names(busy, "Map")) == 1
           and finished_queries(done) == [first.query, second.query],
           "a done bake behind a busy one: nothing finished while the older is busy (one no-wait Map), then both, "
           "the older first")
    print("all cases passed" if not failed else f"{failed} case(s) failed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
