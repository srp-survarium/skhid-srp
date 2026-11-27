#!/usr/bin/env python3
"""Run patch 216 (the Rust patch `scaleform_temp_target`: a Scaleform filter
whose temporary render target Direct3D did not make never leaves a null
target on the HAL's render-target stack) in the default build on a made-up
D3D1x HAL, the original beside, and two broken builds (each site's bytes the
original's again: its hook not installed) that must fail.

    python check_scaleform_temp_target_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are; the
game's own code runs: HAL::PushFilters (0x2B3A50), PushRenderTarget
(0x2B3240) and PopRenderTarget (0x2B35F0), the base HAL's PopFilters
(0x527AC0) with drawUncachedFilter (0x2B3E40), and drawCachedFilter
(0x2B44E0). Answered here (each call kept): the HAL's vtable slots GetEvent
(+D8h), shouldRenderFilters (+10Ch), CreateTempRenderTarget (+50h: the
target each case gives, 0 for none), updateViewport (+3Ch), +120h; the
device context's IASetVertexBuffers (+48h), OMSetRenderTargets (+84h) and
OMSetDepthStencilState (+90h); the matrices', the render event's and the
render targets' vtables; the strings (0x5074C0), the render-target stack's
resize and entry copy (0x2B62D0, 0x2B6530; its entries 2F0h each at
HAL+1E8h), the filter stack's push and pop (0x2B4E00, 0x527A40; at
HAL+1F4h), applyBlendMode (0x525E20), AddRef/Release (0x505CA0/0x505CB0),
FilterPrimitive's Get/SetCacheResults (0x529110, 0x5298C0), the filter
passes' helpers (0x2B5C60, 0x2B4CC0, 0x2B54C0, 0x2BDB10, 0x11100), memset,
InterlockedExchangeAdd and the patches' log's kernel32 imports (its lines
kept).

The cases, each from PushFilters' or drawCachedFilter's call to its return
(the stack, EBX, ESI, EDI and EBP as the caller had them):
  unfiltered  an uncached filter, no mask, whose target is not made, then
              its PopFilters: the stacks as before (render targets 1,
              filters 0), HS_DrawingFilter cleared, the filter stack given
              {primitive, 0}, one log line with the size; the original
              leaves a null target on the stack (2)
  next pop    then a filter target pushed and popped (the report's order):
              back to the main target; the original faults at 0x2B3777
              reading 10h, EAX=2 EDX=0 ESI=3 EBP=0 (revision 73's crash)
  masked      the same under a mask (MaskStackTop 2, a stencil): the masks'
              state set again with the stencil's state and 2 after
              PushFilters disabled them; the original sets it once, leaks
  own mask    a primitive with a mask of its own: no fault; the original
              faults at 0x2B3D7A reading 10h
  nested      inside a filter being drawn (its target on the stack): the
              stacks as before, HS_DrawingFilter kept, then the outer
              target's pop back to the main one; the original's pop faults
              at 0x2B3655 reading 8 (the null target on top)
  made        a target made: both builds push it, the stack 2 and the
              filter stack {primitive, target}, no log line
  cached      a primitive cached one pass from its target whose last
              target is not made: no pass drawn, its cache let go
              (SetCacheResults 0, none, 0), the render-target stack 1, its
              temporaries let go, a log line; the original faults at 0x2B3655
"""
import os
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE, UcError
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

if len(sys.argv) > 1:
    os.environ.setdefault("SRP_026G_EXE", sys.argv[1])    # a span hook's bytes (patch_026g.RUST_HOOKS)
import patch_026g as base

PUSH_FILTERS, POP_FILTERS, POP_RT, DRAW_CACHED = 0x2B3A50, 0x527AC0, 0x2B35F0, 0x2B44E0
HAL_VTABLE = 0xA0DD0C
SITES = {"scaleform.filter_target": (0x2B3CFC, 7), "scaleform.cached_filter_target": (0x2B4863, 6)}
XADD = 0x870094
ENTRY = 0x2F0
DRAWING_FILTER = 0x80

DATA, STACK, STUBS, SENTINEL = 0x30000000, 0x31000000, 0x32000000, 0x33000000
HAL = DATA                                   # fields up to +FC30h
HAL_VT, CTX, CTX_VT, EVENT, EVENT_VT, MATRICES, MATRICES_VT = (DATA + 0x10000 + 0x1000 * i for i in range(7))
RT_VT, STRING, RT_DATA, FS_DATA, RECT = (DATA + 0x17000 + 0x1000 * i for i in range(5))
RT_STACK = DATA + 0x20000                    # 8 entries of 2F0h
MAIN_RT, OUTER_RT, FILTER_RT, CACHED_RT, NEW_RT = (DATA + 0x30000 + 0x100 * i for i in range(5))
PRIM, OUTER, AREA, FILTERS = (DATA + 0x31000 + 0x400 * i for i in range(4))
STENCIL_STATE, DEPTH_STATE = 0x5E5E0001, 0xDE0E0002
CALLER = {UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_ESI: 0x05050505, UC_X86_REG_EDI: 0x0E0E0E0E,
          UC_X86_REG_EBP: 0x0019FF00}


def imports(data):
    """{IAT slot VA: name} of the image's import directory."""
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    rva = struct.unpack_from("<I", data, pe + 0x80)[0]
    sections = [(base.IMAGE_BASE + r, v, raw) for _, r, v, raw, _, _ in base.Headers(data).sections(data)]

    def off(r):
        va = base.IMAGE_BASE + r
        return next(raw + va - s for s, v, raw in sections if s <= va < s + v)
    out, at = {}, off(rva)
    while True:
        lookup, _, _, _, iat = struct.unpack_from("<IIIII", data, at)
        if not iat:
            return out
        i = 0
        while True:
            entry = struct.unpack_from("<I", data, off(lookup or iat) + 4 * i)[0]
            if not entry:
                break
            if not entry & 0x80000000:
                name = data[off(entry) + 2:].split(b"\0", 1)[0].decode()
                out[base.IMAGE_BASE + iat + 4 * i] = name
            i += 1
        at += 20


# kernel32 imports the run may call: (pops, result)
KERNEL32 = {"InterlockedExchangeAdd": (8, None), "GetCurrentProcessId": (0, 0x2A), "GetCurrentThreadId": (0, 0x77),
            "CreateFileA": (0x1C, 0x44), "WriteFile": (0x14, 1), "CloseHandle": (4, 1),
            "GetEnvironmentVariableA": (0xC, 0), "QueryPerformanceCounter": (4, 1),
            "QueryPerformanceFrequency": (4, 1), "GetLastError": (0, 0), "SetLastError": (4, 0),
            "CreateDirectoryA": (8, 1), "GetFileAttributesA": (4, 0xFFFFFFFF), "GetLocalTime": (4, 0),
            "GetSystemTimeAsFileTime": (4, 0), "FindFirstFileA": (8, 0xFFFFFFFF), "FindClose": (4, 1),
            "GetFileSizeEx": (8, 0), "SetFilePointerEx": (0x14, 1), "GetModuleFileNameA": (0xC, 0)}


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at in (DATA, STACK, STUBS, SENTINEL):
            emu.mem_map(at, 0x100000, UC_PROT_ALL)
        self.stubs, self.lines = {}, []
        for slot, name in imports(data).items():
            self.word(slot, self.stub(name, *KERNEL32.get(name, (None, None))))
        # the game's functions answered here: address -> (name, pops, action)
        self.at = {
            0x5074C0: ("string", 4, lambda: self.word(self.reg(UC_X86_REG_ECX), STRING)),
            0x2B2DF0: ("push_rt_helper", 0, lambda: self.word(self.reg(UC_X86_REG_ECX), 0)),
            0x5264E0: ("matrix_copy", 4, None),
            0x424500: ("memset", 0, self.memset),
            0x2B62D0: ("rt_resize", 8, lambda: self.word(self.arg(1) + 4, self.arg(2))),
            0x2B6530: ("rt_copy", 4, lambda: self.emu.mem_write(self.reg(UC_X86_REG_ECX),
                                                              bytes(self.emu.mem_read(self.arg(1), ENTRY)))),
            0x505C70: ("rt_entry_dtor", 0, None), 0x2B6500: ("rt_destroy", 0, None),
            0x2B4E00: ("filters_push", 0, self.filters_push), 0x527A40: ("filters_pop", 4, self.filters_pop),
            0x525E20: ("blend", 0xC, None), 0x505CA0: ("addref", 0, None), 0x505CB0: ("release", 0, None),
            0x5298C0: ("set_cache_results", 0xC, self.set_cache_results),
            0x529110: ("get_cache_results", 8, self.get_cache_results),
            0x11100: ("construct", 0x10, lambda: self.emu.mem_write(self.arg(1), bytes(self.arg(2) * self.arg(3)))),
            0x2B5C60: ("setup_filter", 4, lambda: 1), 0x2B4CC0: ("pass", 0, None), 0x2B54C0: ("pass_draw", 0x20, None),
            0x2BDB10: ("samplers", 0x10, None),
        }
        for address in self.at:
            emu.hook_add(UC_HOOK_CODE, self.answer, begin=address, end=address)
        emu.hook_add(UC_HOOK_CODE, self.stubbed, begin=STUBS, end=STUBS + 0xFFFFF)
        # PushRenderTarget of a made target (case `made`): its arguments kept, the entry pushed
        emu.hook_add(UC_HOOK_CODE, self.push_made, begin=0x2B3240, end=0x2B3240)
        self.made = 0                         # what CreateTempRenderTarget gives
        # the HAL's vtable: the game's, a few slots answered here
        for slot in range(0, 0x140, 4):
            self.word(HAL_VT + slot, self.read(HAL_VTABLE + slot))
        for slot, name, pops, result in ((0xD8, "GetEvent", 4, EVENT), (0x10C, "shouldRenderFilters", 4, 1),
                                         (0x50, "CreateTempRenderTarget", 8, lambda: self.made),
                                         (0x3C, "updateViewport", 0, 0), (0x120, "hal+120h", 8, 0)):
            self.word(HAL_VT + slot, self.stub(name, pops, result))
        for slot, pops in ((0x48, 0x18), (0x84, 0x10), (0x90, 0xC)):
            self.word(CTX_VT + slot, self.stub(f"context+{slot:#x}", pops, 0))
        self.word(CTX, CTX_VT)
        for slot in range(0, 0x20, 4):
            self.word(EVENT_VT + slot, self.stub(f"event+{slot:#x}", 4 if slot == 8 else 0, 0))
            self.word(MATRICES_VT + slot, self.stub(f"matrices+{slot:#x}", 4, 0))
            self.word(RT_VT + slot, self.stub(f"target+{slot:#x}", 4 if slot == 0x14 else 0, 0))
        self.word(EVENT, EVENT_VT)
        self.word(EVENT + 8, EVENT_VT)
        self.word(MATRICES, MATRICES_VT)

    # ---- memory and calls ----

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def reg(self, r):
        return self.emu.reg_read(r)

    def arg(self, n):
        return self.read(self.reg(UC_X86_REG_ESP) + 4 * n)

    def stub(self, name, pops, result):
        at = STUBS + 0x10 * len(self.stubs)
        self.stubs[at] = (name, pops, result)
        return at

    def leave(self, value, pops):
        sp = self.reg(UC_X86_REG_ESP)
        back = self.read(sp)
        self.emu.reg_write(UC_X86_REG_EAX, value or 0)
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        self.emu.reg_write(UC_X86_REG_EIP, back)

    def stubbed(self, emu, address, size, user):
        name, pops, result = self.stubs[address]
        if pops is None:
            self.calls.append((f"unexpected import {name}", []))
            raise SystemExit(f"the run called {name}, which this check does not answer")
        args = [self.arg(n) for n in range(1, 1 + pops // 4)]
        self.calls.append((name, args))
        if name == "InterlockedExchangeAdd":
            old = self.read(args[0])
            self.word(args[0], old + args[1])
            result = old
        elif name == "WriteFile":
            self.lines.append(bytes(emu.mem_read(args[1], args[2])).decode(errors="replace").strip())
        self.leave(result() if callable(result) else result, pops)

    def answer(self, emu, address, size, user):
        name, pops, action = self.at[address]
        self.calls.append((name, [self.arg(n) for n in range(1, 1 + pops // 4)]))
        self.leave(action() if action else 0, pops)

    def push_made(self, emu, address, size, user):
        if not self.made or self.arg(2) != self.made:
            return
        self.calls.append(("PushRenderTarget", [self.arg(1), self.arg(2), self.arg(3)]))
        self.push_target(self.arg(2))
        self.leave(0, 0xC)

    def memset(self):
        self.emu.mem_write(self.arg(1), bytes([self.arg(2) & 0xFF]) * self.arg(3))

    def filters_push(self):                   # EAX the array (HAL+1F4h), EDI the entry
        array, entry = self.reg(UC_X86_REG_EAX), self.reg(UC_X86_REG_EDI)
        n = self.read(array + 4)
        self.emu.mem_write(self.read(array) + 8 * n, bytes(self.emu.mem_read(entry, 8)))
        self.word(array + 4, n + 1)
        self.pushed.append((self.read(entry), self.read(entry + 4)))

    def filters_pop(self):                    # ECX the array, the entry out
        array, out = self.reg(UC_X86_REG_ECX), self.arg(1)
        n = self.read(array + 4) - 1
        self.emu.mem_write(out, bytes(self.emu.mem_read(self.read(array) + 8 * n, 8)))
        self.word(array + 4, n)
        return out

    def set_cache_results(self):
        self.word(self.reg(UC_X86_REG_ECX) + 0x18, self.arg(1))

    def get_cache_results(self):
        primitive, out, count = self.reg(UC_X86_REG_ECX), self.arg(1), self.arg(2)
        for i in range(count):
            self.word(out + 4 * i, self.read(primitive + 0x1C + 4 * i))

    # ---- the made-up HAL ----

    def setup(self, stack=(MAIN_RT,), filters=(), state=0x23F, masks=0, own_mask=False, cache=0, made=0):
        emu = self.emu
        emu.mem_write(HAL, bytes(0x10000))
        self.word(HAL, HAL_VT)
        self.word(HAL + 8, state)
        self.word(HAL + 0x18, MATRICES)
        self.word(HAL + 0x1E4, masks)
        self.word(HAL + 0x1E8, RT_STACK)
        self.word(HAL + 0x1EC, len(stack))
        self.word(HAL + 0x1F0, 8)
        self.word(HAL + 0x1F4, FS_DATA)
        self.word(HAL + 0x1F8, len(filters))
        self.word(HAL + 0x1FC, 8)
        self.word(HAL + 0xF9D4, CTX)
        self.word(HAL + 0xFB70, CTX)
        emu.mem_write(HAL + 0xFB79, b"\x01\x01")
        self.word(HAL + 0xFC24, STENCIL_STATE)
        self.word(HAL + 0xFC28, DEPTH_STATE)
        emu.mem_write(RT_STACK, bytes(ENTRY * 8))
        for i, target in enumerate(stack):
            self.word(RT_STACK + ENTRY * i, target)
        for i, (primitive, target) in enumerate(filters):
            self.word(FS_DATA + 8 * i, primitive)
            self.word(FS_DATA + 8 * i + 4, target)
        for target, kind in ((MAIN_RT, 1), (OUTER_RT, 3), (FILTER_RT, 3), (CACHED_RT, 3), (NEW_RT, 3)):
            emu.mem_write(target, bytes(0x40))
            self.word(target, RT_VT)
            self.word(target + 8, kind)
            self.word(target + 0x10, RT_DATA)
            self.word(target + 0x24, 45)  # its rectangle (drawCachedFilter's size: +1Ch..+28h)
            self.word(target + 0x28, 45)
        emu.mem_write(RT_DATA, bytes(0x40))
        # the primitive: its filters (+10h, one), its area (+14h: a 45 x 45 matrix), its cache
        emu.mem_write(PRIM, bytes(0x40))
        self.word(PRIM, RT_VT)
        self.word(PRIM + 0x10, FILTERS)
        self.word(PRIM + 0x14, AREA)
        self.word(PRIM + 0x18, cache)
        self.word(PRIM + 0x1C, CACHED_RT if cache else 0)
        emu.mem_write(PRIM + 0x24, b"\x01" if own_mask else b"\x00")
        emu.mem_write(FILTERS, bytes(0x20))
        self.word(FILTERS + 8, FILTERS + 0x10)
        self.word(FILTERS + 0xC, 1)
        self.word(FILTERS + 0x10, FILTERS + 0x18)
        emu.mem_write(AREA, bytes(0x200))
        self.word(AREA, AREA + 0x100)
        for at, value in ((0x10, 45.0), (0x24, 45.0), (0x1C, 100.0), (0x2C, 100.0)):
            emu.mem_write(AREA + 0x100 + at, struct.pack("<f", value))
        self.word(STRING + 4, 1000)           # a counted string's count, never 1 here
        self.made = made
        self.calls, self.pushed, self.lines = [], [], []

    def call(self, function, args=(), ecx=HAL):
        """`function`(args) thiscall on `ecx`: None, or the fault and its registers."""
        emu = self.emu
        sp = STACK + 0x80000 - 4 * len(args) - 4
        self.word(sp, SENTINEL)
        for i, value in enumerate(args):
            self.word(sp + 4 + 4 * i, value)
        for r, value in CALLER.items():
            emu.reg_write(r, value)
        emu.reg_write(UC_X86_REG_ESP, sp)
        emu.reg_write(UC_X86_REG_ECX, ecx)
        try:
            emu.emu_start(function, SENTINEL, count=2_000_000)
        except UcError as error:
            return {"error": str(error), **{n: emu.reg_read(r) for n, r in (
                ("eip", UC_X86_REG_EIP), ("eax", UC_X86_REG_EAX), ("edx", UC_X86_REG_EDX),
                ("esi", UC_X86_REG_ESI), ("ebp", UC_X86_REG_EBP))}}
        kept = all(emu.reg_read(r) == v for r, v in CALLER.items())
        if emu.reg_read(UC_X86_REG_EIP) != SENTINEL or not kept or \
                emu.reg_read(UC_X86_REG_ESP) != sp + 4 + 4 * len(args):
            return {"error": f"not back at the caller as it called (eip {emu.reg_read(UC_X86_REG_EIP):#x})"}
        return None

    def stacks(self):
        n = self.read(HAL + 0x1EC)
        return ([self.read(RT_STACK + ENTRY * i) for i in range(n)], self.read(HAL + 0x1F8), self.read(HAL + 8))

    def depth_stencil(self):
        return [tuple(args[1:]) for name, args in self.calls if name == "context+0x90"]

    def push_pop(self, primitive=PRIM):
        fault = self.call(PUSH_FILTERS, [primitive])
        return fault or self.call(POP_FILTERS)

    def push_target(self, target):
        """A filter target's push as PushRenderTarget leaves the stack (its own bytes run in `made`)."""
        n = self.read(HAL + 0x1EC)
        self.word(RT_STACK + ENTRY * n, target)
        self.word(HAL + 0x1EC, n + 1)


def faulted(fault, at, read=None):
    return bool(fault) and fault.get("eip") == at and "UNMAPPED" in fault["error"]


def cases(data):
    """Each case's result on one build: {name: (ok as the fixed build, ok as the original, what)}.
    One machine for the PushFilters cases, in order (the log's 1st, 10th, ... failure of a run is
    said: its first only here), a new one for the cached pass (its first failure)."""
    out = {}
    run = Run(data)

    run.setup(made=0)
    fault = run.push_pop()
    rt, fs, state = run.stacks()
    lines = [line for line in run.lines if "scaleform_temp_target" in line]
    fixed = (not fault and rt == [MAIN_RT] and fs == 0 and not state & DRAWING_FILTER
             and run.pushed == [(PRIM, 0)] and len(lines) == 1 and "45 x 45" in lines[0])
    out["unfiltered"] = (fixed, not fault and rt == [MAIN_RT, 0],
                         f"fault {fault}, render targets {[hex(t) for t in rt]}, filters {fs}, state {state:#x}, "
                         f"filter stack given {[(hex(p), hex(t)) for p, t in run.pushed]}, log {lines}")

    run.push_target(FILTER_RT)
    fault = run.call(POP_RT, [0])
    rt, _, _ = run.stacks()
    stock = faulted(fault, 0x2B3777) and (fault["eax"], fault["edx"], fault["esi"], fault["ebp"]) == (2, 0, 3, 0)
    out["next pop"] = (not fault and rt == [MAIN_RT], stock, f"fault {fault}, render targets {[hex(t) for t in rt]}")

    run.setup(masks=2, made=0)
    fault = run.push_pop()
    rt, fs, _ = run.stacks()
    sets = run.depth_stencil()
    out["masked"] = (not fault and rt == [MAIN_RT] and fs == 0 and sets == [(STENCIL_STATE, 2), (STENCIL_STATE, 2)],
                     not fault and rt == [MAIN_RT, 0] and sets == [(STENCIL_STATE, 2)],
                     f"fault {fault}, render targets {[hex(t) for t in rt]}, the masks' states set {sets}")

    run.setup(own_mask=True, made=0)
    fault = run.push_pop()
    rt, fs, _ = run.stacks()
    out["own mask"] = (not fault and rt == [MAIN_RT] and fs == 0, faulted(fault, 0x2B3D7A),
                       f"fault {fault}, render targets {[hex(t) for t in rt]}")

    run.setup(stack=(MAIN_RT, OUTER_RT), filters=((OUTER, OUTER_RT),), state=0x2BF, made=0)
    fault = run.push_pop()
    rt, fs, state = run.stacks()
    inner = (not fault and rt == [MAIN_RT, OUTER_RT] and fs == 1 and state & DRAWING_FILTER)
    after = run.call(POP_RT, [0])
    rt_after, _, _ = run.stacks()
    out["nested"] = (bool(inner) and not after and rt_after == [MAIN_RT],
                     not fault and rt == [MAIN_RT, OUTER_RT, 0] and faulted(after, 0x2B3655),
                     f"fault {fault}, render targets {[hex(t) for t in rt]}, filters {fs}, state {state:#x}; "
                     f"the outer pop: fault {after}, render targets {[hex(t) for t in rt_after]}")

    run.setup(made=NEW_RT)
    fault = run.call(PUSH_FILTERS, [PRIM])
    rt, fs, state = run.stacks()
    lines = [line for line in run.lines if "scaleform_temp_target" in line]
    pushes = [args[1] for name, args in run.calls if name == "PushRenderTarget"]
    both = (not fault and pushes == [NEW_RT] and rt == [MAIN_RT, NEW_RT] and fs == 1 and run.pushed == [(PRIM, NEW_RT)]
            and state & DRAWING_FILTER and not lines)
    out["made"] = (both, both, f"fault {fault}, render targets {[hex(t) for t in rt]}, filters {fs}, log {lines}")

    run = Run(data)
    run.setup(state=0x33F, cache=1, made=0)
    fault = run.call(DRAW_CACHED, [PRIM])
    rt, _, _ = run.stacks()
    caches = [args for name, args in run.calls if name == "set_cache_results"]
    in_use = [args for name, args in run.calls if name == "target+0x14"]
    lines = [line for line in run.lines if "scaleform_temp_target" in line]
    out["cached"] = (not fault and rt == [MAIN_RT] and caches == [[0, 0, 0]] and in_use == [[0]]
                     and run.read(PRIM + 0x18) == 0 and len(lines) == 1,
                     faulted(fault, 0x2B3655),
                     f"fault {fault}, render targets {[hex(t) for t in rt]}, SetCacheResults {caches}, "
                     f"SetInUse {in_use}, log {lines}")
    return out


def without(built, original, site):
    """The build with `site`'s bytes the original's: its hook not installed."""
    va, span = SITES[site]
    broken = bytearray(built)
    for _, rva, vsize, raw, rsize, _ in base.Headers(broken).sections(broken):
        start = base.IMAGE_BASE + rva
        if start <= va < start + vsize:
            at = raw + va - start
            broken[at:at + span] = original[at:at + span]
            return bytes(broken)
    raise SystemExit(f"{site}: not in a section")


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    built = bytes(built)
    for site, (va, span) in SITES.items():
        at = va - base.TEXT_VA + base.TEXT_FILE
        if built[at:at + span] == original[at:at + span]:
            sys.exit(f"{site}: the build leaves its bytes the original's")
    stock, fixed = cases(original), cases(built)
    failed = 0
    for name, (ok, _, what) in fixed.items():
        stock_ok = stock[name][1]
        good = ok and stock_ok
        failed += not good
        print(f"  {'ok' if good else 'FAIL'}  {name}: {what}\n        the original: {stock[name][2]}")
    # broken builds: each site's hook removed must fail its cases
    for site, names in (("scaleform.filter_target", ("unfiltered", "next pop", "masked", "own mask", "nested")),
                        ("scaleform.cached_filter_target", ("cached",))):
        broke = cases(without(built, original, site))
        passed = [name for name in names if broke[name][0]]
        failed += bool(passed)
        print(f"  {'FAIL' if passed else 'ok'}  a build without {site}'s hook fails {', '.join(names)}"
              + (f": it passed {passed}" if passed else ""))
    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
