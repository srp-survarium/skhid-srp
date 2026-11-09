#!/usr/bin/env python3
"""Run patch 121 (the Rust patch bake_async: the camouflage/decal bake's GPU
work not waited for) in the default build, from the game's own code, on a
made-up renderer, D3D11 context and bakes.

    python check_bake_async_026g.py <original survarium.exe>

Requires Python `unicorn` (2.1.4) and the arena's tools (`nix develop
./srp-026g`). The exe's sections are loaded as they are (check_bake_cache_026g's
Game: the baker's list, patch 70's sites, kernel32 through the IAT). Made up:
the D3D11 immediate context (Map, Unmap, Flush: a staging maps once its
"GPU" has done it, or with D3D11_MAP_FLAG_DO_NOT_WAIT gives 0 and
DXGI_ERROR_WAS_STILL_DRAWING), the textures and render targets the
resource manager makes, the draws (apply_pass, fill_surface: nothing), the
tail's string and path helpers, MT's allocation (which can refuse), the
threads' ids and the clock (QueryPerformanceCounter a step a read). Run for
real: process_compression from its entry (its prologue, the texture
getters, then bake.issue), the compressor's block_mip_count, data_size,
write_dds_header, make_mips and compress_mip (with compress_mip.readback),
map2D, the dispatch's end (resources.dispatched), engine_world::tick's
start (engine.tick), game::exit from its start (game.exit_start),
finish_query (patch 70's) and process_compression again (bake.resume, its
tail from 0x12F00B: the texture's slot, patch 70's sharing, the resource,
the save descriptor (bake.save, allocate_save_descriptor), the CRC, the
record, bake.free, finish).

- issued: process_compression returns (ret 28h) with its query not
  finished, no Map, no MT memory taken, level 0 a DEFAULT texture with no
  initial data and the target copied into it on the GPU, backend::reset
  once before make_mips, the levels drawn, each level's staging copied and
  kept, a Flush after the last copy; the original maps 1 + the levels,
  blocking;
- polled busy: one Map with the no-wait flag, nothing else;
- polled done: the DDS allocated (MT) then, each level read into it at its
  place, each staging unmapped and let go, the texture made from it, in the
  surface's slot and patch 70's waiter's, the query given its resource and
  finished with success, the save's descriptor owning that DDS (no copy),
  the record listed, the DDS not freed, the model let go;
- late (busy past 1 s, read back waiting, logged); lost (the blocking Map
  fails: an error finish, patch 70's waiter made again, the DDS freed, the
  model let go, logged); no MT memory at the readback (an error finish, no
  copy of the game's, logged); no memory for the save's descriptor (the
  texture made, the query finished with success and no save, the DDS freed,
  logged; the game's make_save_descriptor not called);
- a compressor that does not fit the DDS, and no texture for level 0: the
  query failed, nothing left on the GPU, no stock bake, not tried again;
- the places: 8 on the GPU; a 9th completes the oldest first, on the spot
  (its no-wait poll, then its levels read back waiting, its tail, patch
  70's waiter given its texture), then is issued in its place (no Map of
  its own, its query not finished); a 10th with the oldest done completes it
  without waiting; then the rest complete at a dispatch, oldest first;
- another thread's bake while the GPU has this thread's: the stock bake
  (logged with both threads), its dispatches never touch this thread's
  bakes; once this thread's are done, the other thread's next bake is
  issued and completed on its own thread;
- the bakes of a thread that ticks no frames (engine_world::tick is MAIN's):
  the frame thread's dispatches touch none of them (logged with both
  threads); that thread's next dispatch completes every one before it ends,
  waiting for the GPU where it has not done them (logged); before the first
  frame, any thread's dispatch does the same;
- the saves not written yet: with 64 descriptors never destroyed, the 65th
  bake's texture made and used, no save, its DDS freed at once (logged);
  the game's ~save_descriptor (0x10A050) on one frees its DDS and a place;
- the budget: done bakes complete while the frame's 6 ms last (one at
  least), the rest in the next frames (engine_world::tick's start), oldest
  first; with no tick (a wait loop) a frame counts as ended after 100 ms; a
  done bake behind a busy one waits for it;
- game::exit from its start, through the branch that skips its delete
  (0x1051E0 failing): every bake on the GPU completed, logged; then a bake
  is the stock one.
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
DISPATCH_END = 0x1024FA
TICK, TICK_PAST = 0x13CA0, 0x13CA6                   # engine_world::tick's start, past the span
EXIT, EXIT_JOIN = 0x1EC730, 0x1EC837                 # game::exit's start; the join past its delete
DEVICE_GLOBAL, MANAGER_GLOBAL, RENDERER_GLOBAL, BACKEND_GLOBAL = 0x2260478, 0x226047C, 0x2260480, 0x2260490
RENDER_ALLOCATOR, MT_TRACKING, MT_SLOT, NO_MEMORY = 0x1F1D960, 0xA8002C, 0x1F1D308, 0x1F1D61B
CREATE_TEXTURE, CREATE_TARGET, RELEASE_TARGET, COPY2D = 0x3BEA70, 0x3BF930, 0x3BFBC0, 0x3C0E00
APPLY_PASS, BIND, CONSTANTS, FILL, RESET = 0x3D7230, 0x3D6960, 0x132A90, 0x11D950, 0x11B740
APPENDF, PHYSICAL, CONVERT, FROM_MEMORY = 0xE4590, 0x1051E0, 0x12780, 0x3BD030
MT_ALLOC, MT_FREE, RENDER_FREE, MAKE_SAVE, TRIM = 0x106EF0, 0x106FC0, 0x1539D0, 0x109ED0, 0x131F90
BLOCK_FREE, DESTROY_COPY = 0x1015E0, 0x29C40
DESCRIPTOR_ALLOCATED = 0x109F6F    # allocate_save_descriptor's mt_allocate returns here
SAVE_FREED = 0x10A050              # save_descriptor::~save_descriptor (save.freed)
QPC, QPF, THREAD_ID = 0x870158, 0x870150, 0x870118
SAVE_COUNTER = 0x1B3A2AC
WAS_STILL_DRAWING, DEVICE_REMOVED = 0x887A000A, 0x887A0005
NO_WAIT = 0x100000
HEAP = 0x50000000
TEB = 0x60000000                   # the thread's block: fs:[2Ch] its TLS array (elapsed_ms's ticks a ms)
TICKS_A_MS = 1000                  # QueryPerformanceFrequency's 1e6 / 1000
COMPRESS_TIMED = 0x12F014          # past the tail's `fstp dword [esp+48h]`: the compress ms
ELAPSED_QPC_BACK = 0x1B7C3        # elapsed_ticks's QueryPerformanceCounter returns here
MAIN, OTHER = 0x1D0C, 0x2E1C       # two threads' ids
FAILED = (1, 1, 0xB)
SUCCESS = (3, 1, 0)
NAME = "renesanse_heavy_com_tank_01_torso_s1d"
PATH = cache.PREFIX + NAME + ".dds"


class Game(cache.Game):
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_map(HEAP, 0x4000000, UC_PROT_ALL)
        self.free_at = HEAP
        self.qpc = 1_000_000
        self.qpc_step = 1000               # the counter's ticks a read (1 MHz: 1 ms)
        self.thread_id = MAIN
        self.mt_refuse = lambda size, back: False     # (its size, its return address): give 0
        self.level0_refused = False
        self.log = []
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
                         (BLOCK_FREE, "block free"), (DESTROY_COPY, "destroy copy")):
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

    def stagings(self):
        return {t for t, info in self.textures.items() if info["usage"] == 3}

    def code(self, emu, address, size, user):
        name = self.stubs.get(address)
        if name == "GetCurrentThreadId":             # this game's threads (the cache's Game has one)
            self.returns(self.thread_id)
            return
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
            refused = self.level0_refused and arg(5) == 0 and arg(4) == 0x1C
            t = 0 if refused else self.texture(reg(UC_X86_REG_EAX), arg(2), arg(5))
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
        elif name == "physical":                     # eax the virtual path, ecx the physical out; al 0: it fails
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
            size = reg(UC_X86_REG_EAX)
            at = 0 if self.mt_refuse(size, self.read(sp)) else self.new(size + 0x10)
            self.calls[-1] = (name, size, at)
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
        elif name == "destroy copy":                 # on_textures_loaded's parameters copy; `ret 4`
            self.calls[-1] = (name, arg(0))
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
            self.calls[-1] = (name, arg(0))
            self.returns(0, 4)
        elif name == "QueryPerformanceCounter":
            self.qpc += self.qpc_step
            self.qpcs.append((self.read(sp), self.qpc))
            self.word(arg(0), self.qpc & 0xFFFFFFFF)
            self.word(arg(0) + 4, self.qpc >> 32)
            self.returns(1, 4)
        elif name == "QueryPerformanceFrequency":
            self.word(arg(0), 1_000_000)
            self.word(arg(0) + 4, 0)
            self.returns(1, 4)
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
    """A made-up bake request: its query, surface, model, parameters, targets,
    and on_textures_loaded's frame."""
    def __init__(self, game, kind=0, size=64, name=NAME, dds_size=None):
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
        # on_textures_loaded's frame (EBP, process_compression's saved EBP): [ebp] the saved EBP, [ebp+4]
        # its return, [ebp+18h] the parameters
        self.caller = g.new(0x40)
        for at, value in ((0, 0xEB9), (4, cache.STOP), (0x18, self.parameters)):
            g.word(self.caller + at, value)
        levels = 0
        while (size // 4) >> levels:
            levels += 1
        self.levels = levels
        block = 16 if kind == 1 else 8
        data = sum(max(4, size >> i) ** 2 // 16 * block for i in range(levels))
        self.dds_size = dds_size or 0x80 + data + 1


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


def tick(game):
    """engine_world::tick from its start to past its span (the frame begun)."""
    sp = cache.STACK + 0xD8000
    game.word(sp, cache.STOP)
    eip, esp = game.run(TICK, sp, {UC_X86_REG_ECX: 0xE3}, until=[TICK_PAST])
    return eip == TICK_PAST and game.emu.reg_read(UC_X86_REG_EBP) == sp - 4


def fresh(built):
    """A game of the build whose main thread has ticked a frame (engine_world::
    tick's start): the thread that ticks frames is MAIN."""
    game = Game(bytes(built))
    assert tick(game)
    return game


def destroy(game, descriptor):
    """save_descriptor::~save_descriptor (0x10A050, ESI the descriptor), as
    ~query_result calls it once the save is written: (back, calls)."""
    sp = cache.STACK + 0xD4000
    game.word(sp, cache.STOP)
    before = len(game.calls)
    eip, _ = game.run(SAVE_FREED, sp, {UC_X86_REG_ESI: descriptor}, count=2_000_000)
    return eip == cache.STOP, game.calls[before:]


def exiting(game):
    """game::exit from its start (ECX the game, its argument) to the join
    past its delete, through the branch 0x1EC7AE takes when 0x1051E0 fails
    (the stub's al 0): (there, and with EDI the game; calls)."""
    g = game
    sp = cache.STACK + 0xD0000
    g.word(sp, cache.STOP)
    g.word(sp + 4, 0)
    before = len(g.calls)
    eip, esp = g.run(EXIT, sp, {UC_X86_REG_ECX: 0x6A3E, UC_X86_REG_EDI: 0xD1, UC_X86_REG_ESI: 0x51},
                     until=[EXIT_JOIN], count=80_000_000)
    return eip == EXIT_JOIN and g.emu.reg_read(UC_X86_REG_EDI) == 0x6A3E, g.calls[before:]


def names(calls, what):
    return [c for c in calls if c[0] == what]


def finishes(calls, how):
    """The queries finished with `how` (result, assert, error), in order."""
    return [c[1] for c in names(calls, "finish") if c[2:] == how]


def issue(game, bake):
    """process_compression for `bake`: (back, calls, its stagings in the order made)."""
    before = set(game.textures)
    ok, calls = process(game, bake)
    return ok, calls, [t for t in game.textures if t not in before and game.textures[t]["usage"] == 3]


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
           and finishes(calls, SUCCESS) == [bake.query],
           f"the original maps {len(maps)} times (the target, then {bake.levels} levels), each blocking, and "
           "finishes the query")

    print("issued, polled busy, polled done (the build)")
    game = fresh(built)
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
    report(ok and not names(calls, "Map") and not names(calls, "finish") and not names(calls, "mt alloc")
           and game.read(bake.model + 0xD0) == 6,
           "process_compression returns (ret 28h, registers kept): no Map, its query not finished, no MT memory "
           "taken, the model held (6 references)")
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
    last_copy = max(i for i, c in enumerate(calls) if c[0] == "copy2D")
    report(any(c == ("Flush", game.context) for c in calls[last_copy:]),
           "the immediate context flushed after the last level's copy to its staging")
    ok, calls = dispatched(game)
    maps = names(calls, "Map")
    report(ok and maps == [("Map", stagings[-1], NO_WAIT)] and not names(calls, "Unmap")
           and not names(calls, "finish") and not names(calls, "mt alloc"),
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
    allocs = [c for c in names(calls, "mt alloc") if c[1] == bake.dds_size]
    report(ok and maps[0] == ("Map", stagings[-1], NO_WAIT) and sorted(m[1] for m in maps[1:]) == sorted(stagings)
           and len(unmaps) == len(maps) and len(allocs) == 1 and calls.index(allocs[0]) < calls.index(maps[1]),
           f"a dispatch, the GPU done: the last staging asked without waiting, the DDS allocated (MT, "
           f"{bake.dds_size} bytes) then, each level mapped and unmapped ({len(maps)} Maps, {len(unmaps)} Unmaps)")
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
    report(made and made[0][1] == PATH[:-4] and made[0][3:6] == (bake.dds_size, 0x5A1, 0x5A2) and expected
           and allocs and made[0][2] == allocs[0][2],
           "the texture made from that DDS (create_texture_from_memory: its name, size, pair; the DDS's header, "
           "each level's rows from its staging, the diffuse byte last)")
    texture = made[0][8] if made else 0
    report(game.read(bake.surface) == texture and game.read(waiter_surface) == texture
           and bake.query in finishes(calls, SUCCESS) and waiter_query in finishes(calls, SUCCESS),
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

    print("late, lost, no MT memory, no save descriptor")
    game = fresh(built)
    bake = Bake(game)
    process(game, bake)
    game.qpc += 2_000_000
    ok, calls = dispatched(game)
    report(ok and names(calls, "from memory") and any("1 s" in line for line in game.log),
           "busy 2 s: read back waiting, finished, logged: " + "; ".join(game.log))
    game = fresh(built)
    bake = Bake(game)
    game.lookup(PATH, bake.query, bake.surface, bake.parameters, bake.model)
    w_query, w_surface, w_parameters, w_model = cache.Objects(game).request(0)
    game.lookup(PATH, w_query, w_surface, w_parameters, w_model)
    process(game, bake)
    game.lost = True
    game.qpc += 2_000_000
    ok, calls = dispatched(game)
    report(ok and finishes(calls, FAILED) == [bake.query]
           and ("translate", cache.COOK, w_query) in names(calls, "translate")
           and len(names(calls, "mt free")) == 1 and game.read(bake.model + 0xD0) == 5
           and any("could not be read back" in line for line in game.log),
           "the device lost: the query finished with an error (1, 1, 0Bh), patch 70 makes the waiter again, the "
           "DDS freed (MT), the model let go, logged")
    game = fresh(built)
    bake = Bake(game)
    process(game, bake)
    game.done.update(game.stagings())
    game.mt_refuse = lambda size, back: size == bake.dds_size
    ok, calls = dispatched(game)
    report(ok and finishes(calls, FAILED) == [bake.query] and not names(calls, "from memory")
           and not names(calls, "make save") and not names(calls, "Map")[1:] and game.read(bake.model + 0xD0) == 5
           and any("no MT memory for its DDS of " + str(bake.dds_size) in line for line in game.log),
           "no MT memory for the DDS at the readback: the query failed, nothing mapped past the poll, no texture, "
           "no copy of the game's, the model let go, logged")
    game = fresh(built)
    bake = Bake(game)
    process(game, bake)
    game.done.update(game.stagings())
    game.mt_refuse = lambda size, back: back == DESCRIPTOR_ALLOCATED
    ok, calls = dispatched(game)
    frees = names(calls, "mt free")
    allocs = [c for c in names(calls, "mt alloc") if c[1] == bake.dds_size]
    report(ok and names(calls, "from memory") and finishes(calls, SUCCESS) == [bake.query]
           and game.read(bake.query + 0xE0) == 0 and not names(calls, "make save") and allocs
           and frees == [("mt free", allocs[0][2])] and game.read(bake.surface) != 0
           and any("not saved" in line for line in game.log),
           "no memory for the save's descriptor: the texture made and in its slot, the query finished with success "
           "and no save (+0E0h 0, make_save_descriptor not called), the DDS freed, logged")

    print("a compressor that does not fit, no texture for level 0")
    game = fresh(built)
    bake = Bake(game, dds_size=0x100)
    ok, calls = process(game, bake)
    report(ok and finishes(calls, FAILED) == [bake.query] and not names(calls, "Map")
           and not names(calls, "create texture") and game.read(bake.model + 0xD0) == 5
           and any("do not fit its DDS of 256" in line for line in game.log),
           "a DDS too small for its levels: the query failed (1, 1, 0Bh), no texture made, no Map (no stock bake), "
           "the model let go, logged: " + "; ".join(game.log))
    game = fresh(built)
    game.level0_refused = True
    bake = Bake(game)
    ok, calls = process(game, bake)
    report(ok and finishes(calls, FAILED) == [bake.query] and not names(calls, "Map")
           and not names(calls, "create target") and game.read(bake.model + 0xD0) == 5
           and any("level 0" in line for line in game.log),
           "no texture for level 0: the query failed (1, 1, 0Bh), nothing drawn, no Map (no stock bake), the model "
           "let go, logged: " + "; ".join(game.log))
    game.level0_refused = False
    ok, calls = dispatched(game)
    report(ok and not names(calls, "finish") and not names(calls, "Map"),
           "the next dispatch: nothing of it is left (never tried again)")

    print("the places: 8 on the GPU, the oldest completed for a new bake")
    game = fresh(built)
    game.qpc_step = 10                       # 10 us a read: every done bake inside the budget
    bakes = [Bake(game, name=f"{NAME}_{i}") for i in range(10)]
    oldest_path = cache.PREFIX + bakes[0].name + ".dds"
    game.lookup(oldest_path, bakes[0].query, bakes[0].surface, bakes[0].parameters, bakes[0].model)
    w_query, w_surface, w_parameters, w_model = cache.Objects(game).request(0)
    waiting = game.lookup(oldest_path, w_query, w_surface, w_parameters, w_model)
    staged = [issue(game, b)[2] for b in bakes[:8]]
    game.log.clear()
    ok, calls, ninth = issue(game, bakes[8])
    staged.append(ninth)
    maps = names(calls, "Map")
    made = names(calls, "from memory")
    texture = made[0][8] if made else 0
    first_copy = next((i for i, c in enumerate(calls) if c[0] == "copy2D"), len(calls))
    report(ok and waiting[0] == "waits" and maps and maps[0] == ("Map", staged[0][-1], NO_WAIT)
           and sorted(m[1] for m in maps[1:]) == sorted(staged[0]) and all(m[2] == 0 for m in maps[1:])
           and max(i for i, c in enumerate(calls) if c[0] == "Map") < first_copy
           and bakes[0].query in finishes(calls, SUCCESS) and w_query in finishes(calls, SUCCESS)
           and game.read(bakes[0].surface) == texture and game.read(w_surface) == texture
           and game.read(bakes[0].model + 0xD0) == 5 and any("read back waiting" in line for line in game.log),
           "a 9th, every place taken: the oldest completed first, on the spot (its no-wait poll busy, then its "
           "levels read back waiting, its texture in its slot and patch 70's waiter's, both finished, its model "
           "let go), logged: " + "; ".join(game.log))
    report(ok and len(ninth) == bakes[8].levels and not any(m[1] in ninth for m in maps)
           and bakes[8].query not in [c[1] for c in names(calls, "finish")] and game.read(bakes[8].model + 0xD0) == 6,
           "then the 9th issued in its place: its levels copied to stagings, none mapped, its query not finished, "
           "its model held")
    game.done.update(staged[1])
    game.log.clear()
    ok, calls, tenth = issue(game, bakes[9])
    staged.append(tenth)
    maps = names(calls, "Map")
    report(ok and maps and maps[0] == ("Map", staged[1][-1], NO_WAIT) and len(maps) == 1 + bakes[1].levels
           and finishes(calls, SUCCESS) == [bakes[1].query] and not any(m[1] in tenth for m in maps),
           "a 10th, the oldest done: completed without waiting (its poll maps), then the 10th issued (its line "
           "within the second the 9th's went out is counted, not written)")
    for s in staged:
        game.done.update(s)
    tick(game)
    ok, calls = dispatched(game)
    report(ok and finishes(calls, SUCCESS) == [b.query for b in bakes[2:10]]
           and all(game.read(b.model + 0xD0) == 5 for b in bakes),
           "the GPU done: the other 8 completed at a dispatch, oldest first (the 9th and 10th last); every model "
           "let go")

    print("another thread's bake")
    game = fresh(built)
    game.qpc_step = 10
    mine, theirs, later = (Bake(game, name=NAME + what) for what in ("_main", "_other", "_later"))
    _, _, mine_staged = issue(game, mine)
    game.thread_id = OTHER
    game.log.clear()
    ok, calls = process(game, theirs)
    maps = names(calls, "Map")
    report(ok and len(maps) == 1 + theirs.levels and all(f == 0 for _, _, f in maps)
           and not any(m[1] in mine_staged for m in maps) and finishes(calls, SUCCESS) == [theirs.query]
           and any("thread 0x00002e1c while the GPU has thread 0x00001d0c" in line for line in game.log),
           "a bake of another thread while the GPU has this thread's: the stock bake (its blocking Maps, finished), "
           "logged with both threads: " + "; ".join(game.log))
    ok, calls = dispatched(game)
    report(ok and not names(calls, "Map") and not names(calls, "finish"),
           "that thread's dispatch touches no bake of this one's")
    game.thread_id = MAIN
    game.done.update(game.stagings())
    ok, calls = dispatched(game)
    report(ok and finishes(calls, SUCCESS) == [mine.query], "this thread's dispatch: its bake completed")
    game.thread_id = OTHER
    ok, calls, later_staged = issue(game, later)
    report(ok and not names(calls, "Map") and not names(calls, "finish") and len(later_staged) == later.levels,
           "the GPU's places free again: the other thread's next bake issued, not waited for")
    game.done.update(later_staged)
    ok, calls = dispatched(game)
    report(ok and finishes(calls, SUCCESS) == [later.query], "and completed at its own thread's dispatch")
    game.thread_id = MAIN

    print("the bakes of a thread that ticks no frames")
    game = fresh(built)                      # MAIN ticks frames
    game.qpc_step = 10
    game.thread_id = OTHER
    bakes = [Bake(game, name=f"{NAME}_other_{i}") for i in range(3)]
    staged = [issue(game, b)[2] for b in bakes]
    game.thread_id = MAIN
    game.log.clear()
    ok, calls = dispatched(game)
    report(ok and not names(calls, "Map") and not names(calls, "finish")
           and any("thread 0x00002e1c's, not the frame thread 0x00001d0c's" in line for line in game.log),
           "3 bakes issued on a thread that ticks no frames: the frame thread's dispatch touches none of them, "
           "logged with both threads: " + "; ".join(game.log))
    game.thread_id = OTHER
    game.done.update(staged[0])              # the GPU has done the first only
    game.log.clear()
    ok, calls = dispatched(game)
    blocking = [m for m in names(calls, "Map") if m[2] == 0]
    report(ok and finishes(calls, SUCCESS) == [b.query for b in bakes]
           and sorted(m[1] for m in blocking) == sorted(s for each in staged for s in each)
           and all(game.read(b.model + 0xD0) == 5 for b in bakes)
           and any("ticks no frames" in line and "its 3 bakes completed at its dispatch's end, 2 read back waiting"
                   in line for line in game.log),
           "that thread's next dispatch: all 3 completed before it ends (the 2 the GPU had not done read back "
           "waiting), oldest first, not left for a dispatch that may not come: " + "; ".join(game.log))
    ok, calls = dispatched(game)
    report(ok and not names(calls, "Map") and not names(calls, "finish"), "nothing of them left after it")
    game = Game(bytes(built))                # no frame ticked yet: no thread is known to come back
    bake = Bake(game)
    _, _, mine = issue(game, bake)
    ok, calls = dispatched(game)
    report(ok and finishes(calls, SUCCESS) == [bake.query] and sorted(m[1] for m in names(calls, "Map")
                                                                      if m[2] == 0) == sorted(mine),
           "before the first frame, a dispatch completes its thread's bakes too (read back waiting)")

    print("the saves not written yet: at most 64")
    game = fresh(built)
    game.qpc_step = 10
    bakes = [Bake(game, name=f"{NAME}_save_{i}") for i in range(66)]
    held, last_calls = [], []
    for b in bakes[:65]:
        issue(game, b)
        game.done.update(game.stagings())
        tick(game)
        game.log.clear()
        ok, last_calls = dispatched(game)
        held.append(game.read(b.query + 0xE0))
    last = bakes[64]
    allocs = [c for c in names(last_calls, "mt alloc") if c[1] == last.dds_size]
    report(all(held[:64]) and held[64] == 0 and game.read(last.surface) != 0
           and last.query in finishes(last_calls, SUCCESS) and allocs
           and ("mt free", allocs[0][2]) in names(last_calls, "mt free")
           and any("used, not saved: 64 saves not written yet" in line for line in game.log),
           "64 bakes' saves not written yet (their descriptors never destroyed): the 65th's texture made and "
           "used, its query finished, no save (+0E0h 0), its DDS freed at once, logged: " + "; ".join(game.log))
    data = game.read(held[0])
    ok, calls = destroy(game, held[0])
    report(ok and ("mt free", data) in names(calls, "mt free"),
           "the game's ~save_descriptor (0x10A050) on the first, as ~query_result runs it once written: its DDS "
           "freed (mt_free)")
    issue(game, bakes[65])
    game.done.update(game.stagings())
    tick(game)
    ok, calls = dispatched(game)
    report(ok and game.read(bakes[65].query + 0xE0) != 0,
           "that place free: the next bake is saved again (its descriptor owns its DDS)")

    print("the budget: 6 ms a frame, oldest first")
    game = fresh(built)
    bakes = [Bake(game, name=f"{NAME}_{i}") for i in range(5)]
    for b in bakes:
        process(game, b)
    game.done.update(game.stagings())
    game.qpc_step = 4000                     # 4 ms a read: one finish spends the budget
    tick(game)
    rounds = []
    for what in ("dispatch", "dispatch", "tick", "dispatch", "tick", "dispatch"):
        if what == "tick":
            tick(game)
        else:
            rounds.append(dispatched(game))
    report(all(ok for ok, _ in rounds)
           and [finishes(calls, SUCCESS) for _, calls in rounds] == [[bakes[0].query], [], [bakes[1].query],
                                                                    [bakes[2].query]],
           "5 done, each finish past the 6 ms: one a frame (the frame's second dispatch none), oldest first")
    starts, rounds = [], []
    while len(rounds) < 100 and len([q for _, calls in rounds for q in finishes(calls, SUCCESS)]) < 2:
        starts.append(game.qpc)
        rounds.append(dispatched(game))
    done = [q for _, calls in rounds for q in finishes(calls, SUCCESS)]
    at = [i for i, (_, calls) in enumerate(rounds) if finishes(calls, SUCCESS)]
    gap = (starts[at[1]] - starts[at[0]]) // 1000 if len(at) == 2 else 0
    report(all(ok for ok, _ in rounds) and done == [bakes[3].query, bakes[4].query] and at[0] > 0 and gap >= 100,
           f"no tick (a wait loop's dispatches): the frame counts as ended after 100 ms (completions at dispatches "
           f"{at}, {gap} ms apart), the rest finish, oldest first")
    game = fresh(built)
    game.qpc_step = 10
    first, second = Bake(game, name=NAME + "_a"), Bake(game, name=NAME + "_b")
    process(game, first)
    before = set(game.textures)
    process(game, second)
    game.done.update(t for t in game.stagings() if t not in before)
    ok_busy, busy = dispatched(game)
    game.done.update(game.stagings())
    tick(game)
    ok_done, done = dispatched(game)
    report(ok_busy and ok_done and finishes(busy, SUCCESS) == [] and len(names(busy, "Map")) == 1
           and finishes(done, SUCCESS) == [first.query, second.query],
           "a done bake behind a busy one: nothing finished while the older is busy (one no-wait Map), then both, "
           "the older first")

    print("game::exit")
    game = fresh(built)
    game.qpc_step = 10
    bakes = [Bake(game, name=f"{NAME}_{i}") for i in range(9)]
    for b in bakes[:8]:
        process(game, b)
    game.log.clear()
    ok, calls = exiting(game)
    report(ok and finishes(calls, SUCCESS)[:8] == [b.query for b in bakes[:8]]
           and any("the game exits: bakes on the GPU 8" in line for line in game.log)
           and len([c for c in names(calls, "Map") if c[2] == 0]) >= 8,
           "game::exit from its start, through the branch that skips its delete (0x1051E0 fails): the 8 on the "
           "GPU completed waiting, logged")
    ok, calls = process(game, bakes[8])
    maps = names(calls, "Map")
    report(ok and len(maps) == 1 + bakes[8].levels and all(f == 0 for _, _, f in maps)
           and finishes(calls, SUCCESS) == [bakes[8].query],
           "after the exit, a bake is the stock one (its blocking Maps, finished)")
    print("all cases passed" if not failed else f"{failed} case(s) failed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
