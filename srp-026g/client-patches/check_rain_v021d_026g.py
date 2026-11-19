#!/usr/bin/env python3
"""Patch 130 (v021d_render): the ported 0.21 maps render the 0.21 way
(../docs/notes/client_patch.md section 3.82).

    python check_rain_v021d_026g.py <original survarium.exe> [<0.21d survarium.exe>]

Runs the game's own code in the original exe and in the patched one (Unicorn):

- effect_wet_surface::compile (0x1CE100) and effect_rain::compile (0x1D3B90)
  whole, caching and building, on a made-up compiler, the engine's
  begin_pass, set_texture, end_pass, end_technique, set_stencil,
  set_alpha_blend and pass-vector release stubbed and recorded with every
  write to the compiler: the patched compile makes the original's techniques,
  then the twins (wet: its three again, technique 0's pixel shader
  wet_surface_v021d; rain: its technique 1 again, t_rain_drop
  ported/v021d/engine/rain_drop), each the original's calls and writes but
  for that one name; it returns as the original (ESP, EBX, ESI, EDI, EBP),
  and a second compile with the same compiler makes the same again;
- the map: project.insert (0x285750 to insert_static's return), remove_static
  (0x2855B0) and the destructor's entry (0x285330) on made-up projects,
  marked (srp_renderer_v021d true), unmarked, marked false; then
  stage_pre_rain::execute's gate (0x17CB16) and wet pass (0x17CB90),
  stage_rain::execute's drop scales (0x1ADF7D) and drops (0x1ADFA4), each
  from its site to where the original goes on: only while the context draws
  the marked project's scene (not the lobby's, not after the project is
  removed or destroyed) does the patched exe skip the shading-quality gate,
  apply technique 3 and 2, and store rain_drops_parameters1.zw as half floats
  -- with the 0.21d exe given, bit for bit what 0.21d's stage_rain::execute
  stores (0x123382..0x12343E run in it) for the same scales; otherwise the
  original's, bit for bit;
- the god-rays pass's entry (0x1A5520) for each atmosphere stage (+ACh 0 the
  sky's, 1 the geometry's): only for the sky's stage while the context draws
  the marked project's scene does the patched exe return at once (its ret 4,
  the argument popped, the registers kept); otherwise it runs the pass as the
  original does;
- effect_material_base::compile_begin (0x1CD0B0) to its begin_pass, for each
  grass vertex shader family and others, grassmesh input and not, with the
  material key srp_grass_v021d and without: the patched VS is the family's
  _v021d twin exactly for a marked grassmesh pass of a listed family;
- effect_fstage_simpe_water_materials::compile (0x1E5FE0) to its t_normal_map
  set_texture: ported/v021d/engine/water_waves2 for a material with
  srp_water_v021d, engine/water_waves2 otherwise.
"""
import importlib.util
from pathlib import Path
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_HOOK_MEM_WRITE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_XMM0)

import patch_026g as base

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('content_edit', ROOT / 'scripts/content-edit/content_edit.py')
content_edit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(content_edit)

HEAP, HEAP_SIZE = 0x40000000, 0x01000000
STACK, STACK_SIZE = 0x48000000, 0x00100000
RET = 0x4F000000                          # the callers' return: where a run stops
STUBS = 0x4F001000                        # imports' stand-ins
COMPILER, COMPILER_SIZE = HEAP, 0x62000
CFG = HEAP + 0x100000                     # a config tree, relocated there
PROJECT_CFG, PROJECT_ROOT = HEAP + 0x200000, HEAP + 0x200200
PROJECT, GAME_SCENE = HEAP + 0x300000, HEAP + 0x301000
RENDER_SCENE, LOBBY_SCENE = HEAP + 0x302000, HEAP + 0x303000
PRE_RAIN, RAIN, CONTEXT, VIEW, OPTIONS, ATMOSPHERE = (HEAP + 0x310000 + i * 0x1000 for i in range(6))
WET_EFFECT, DROPS_EFFECT = 0x5E7E0000, 0x0D2E0000   # stage effect addresses, passed through in EAX
SHADER_CONFIG, MATERIAL_OPTIONS, NAMES = HEAP + 0x320000, HEAP + 0x321000, HEAP + 0x330000
OPTIONS_SLOT = 0x2260498                  # the render options' pointer
THREAD_SLOT, CREATE_FILE_SLOT = 0x870118, 0x870048
SAVED = {UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_ESI: 0x05050505, UC_X86_REG_EDI: 0x0D1D1D1D,
         UC_X86_REG_EBP: 0x0E0E0E0E}

BEGIN_PASS, SET_TEXTURE, END_PASS, END_TECHNIQUE = 0x3D9F60, 0x3D98B0, 0x3DA450, 0x3DA6B0
SET_STENCIL, SET_ALPHA_BLEND, RELEASE_PASSES = 0x3E2E40, 0x3E2EA0, 0x3CC920
GRASS = ("vertex_base", "vertex_base_shadow", "vertex_base_shadow_batched", "vertex_base_lpv", "vertex_base_unwrap",
         "vertex_base_bake_decal_occlusion", "vertex_base_z_shifted")


def relocated_config(value, at):
    """`value` as a binary config whose offsets are addresses at `at` (the loader's relocation)."""
    b = bytearray(24)
    b[:24] = content_edit.value_record(b, 0, '', value)
    records = []

    def visit(off):
        d, name, crc, t, count = content_edit.rec(b, off)
        records.append((off, d, name, crc, t, count))
        if t in (3, 4):
            for kid in content_edit.children(b, off):
                visit(kid)
    visit(0)
    for off, d, name, crc, t, count in records:
        content_edit.REC.pack_into(b, off, d + at if t >= 3 and d else d, name + at if name else 0, crc, t, count)
    return bytes(b)


class Machine:
    def __init__(self, data, imports=True):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(HEAP, HEAP_SIZE, UC_PROT_ALL)
        emu.mem_map(STACK, STACK_SIZE, UC_PROT_ALL)
        emu.mem_map(RET, 0x10000, UC_PROT_ALL)
        emu.mem_write(STUBS, bytes.fromhex("b801000000c3"))                 # GetCurrentThreadId: 1
        emu.mem_write(STUBS + 0x10, bytes.fromhex("83c8ffc21c00"))          # CreateFileA: INVALID_HANDLE_VALUE
        if imports:                                                         # 0.26g's slots
            self.w(THREAD_SLOT, STUBS)
            self.w(CREATE_FILE_SLOT, STUBS + 0x10)
        self.stops = set()
        self.stubs = {}
        self.events = None
        self.names = {}
        emu.hook_add(UC_HOOK_CODE, self.code)
        emu.hook_add(UC_HOOK_MEM_WRITE, self.write, begin=COMPILER, end=COMPILER + COMPILER_SIZE - 1)

    def w(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def r(self, at):
        return struct.unpack("<I", bytes(self.emu.mem_read(at, 4)))[0]

    def f(self, at):
        return struct.unpack("<f", bytes(self.emu.mem_read(at, 4)))[0]

    def text(self, at):
        out = bytes(self.emu.mem_read(at, 128))
        return out[:out.index(0)].decode()

    def name(self, text):
        """A C string of ours for the game's code to read."""
        if text not in self.names:
            at = NAMES + 0x80 * len(self.names)
            self.emu.mem_write(at, text.encode() + b"\0")
            self.names[text] = at
        return self.names[text]

    def code(self, emu, address, size, _):
        if address in self.stops:
            self.stopped = address
            emu.emu_stop()
        elif address in self.stubs:
            esp = emu.reg_read(UC_X86_REG_ESP)
            cleanup, event = self.stubs[address](esp)
            if self.events is not None and event is not None:
                self.events.append(event)
            emu.reg_write(UC_X86_REG_ESP, esp + 4 + cleanup)
            emu.reg_write(UC_X86_REG_EIP, self.r(esp))

    def write(self, emu, access, address, size, value, _):
        if self.events is not None:
            self.events.append(("write", address - COMPILER, size, value & ((1 << (8 * size)) - 1)))

    def run(self, start, stops, regs=None, stack=(), frame=None):
        emu = self.emu
        emu.mem_write(STACK, bytes(STACK_SIZE))
        sp = STACK + STACK_SIZE // 2
        if frame is not None:
            emu.mem_write(sp, frame)
        emu.mem_write(sp, struct.pack("<%dI" % len(stack), *stack) if stack else b"")
        for reg, value in SAVED.items():
            emu.reg_write(reg, value)
        for reg in (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX):
            emu.reg_write(reg, 0x0A0A0A0A)
        for reg, value in (regs or {}).items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, sp)
        self.stops, self.stopped = set(stops), None
        emu.emu_start(start, 0, count=2_000_000)
        assert self.stopped in self.stops, hex(emu.reg_read(UC_X86_REG_EIP))
        return self.stopped, sp


# -- effect compiles ----------------------------------------------------------------------------------------

def compile_stubs(m):
    def begin_pass(esp):
        emu = m.emu
        return 0x1C, ("begin_pass", m.text(emu.reg_read(UC_X86_REG_EAX)), m.r(esp + 4) - COMPILER, m.r(esp + 8),
                      m.text(m.r(esp + 12)), bytes(emu.mem_read(esp + 16, 16)))

    def set_texture(esp):
        rest = tuple(m.r(esp + 16 + 4 * i) for i in range(5))
        return 0x20, ("set_texture", m.r(esp + 4) - COMPILER, m.text(m.r(esp + 8)), m.text(m.r(esp + 12)), rest)

    def regs(*which):
        return tuple(m.emu.reg_read(r) for r in which)

    m.stubs = {
        BEGIN_PASS: begin_pass,
        SET_TEXTURE: set_texture,
        END_PASS: lambda esp: (4, ("end_pass", m.r(esp + 4) - COMPILER)),
        END_TECHNIQUE: lambda esp: (4, ("end_technique", m.r(esp + 4) - COMPILER)),
        SET_STENCIL: lambda esp: (0x10, ("set_stencil", tuple(m.r(esp + 4 + 4 * i) for i in range(4)),
                                         m.emu.reg_read(UC_X86_REG_ESI) - COMPILER)),
        SET_ALPHA_BLEND: lambda esp: (0x0C, ("set_alpha_blend", tuple(m.r(esp + 4 + 4 * i) for i in range(3)),
                                             regs(UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI))),
        RELEASE_PASSES: lambda esp: (0, ("release_passes", regs(UC_X86_REG_EAX)[0] - COMPILER,
                                              regs(UC_X86_REG_EDI)[0] - COMPILER)),
    }


def techniques(events):
    """The events split at each end_technique (its own the last of each)."""
    out, current = [], []
    for event in events:
        current.append(event)
        if event[0] == "end_technique":
            out.append(current)
            current = []
    assert not current, ("events after the last technique", current)
    return out


def run_compile(m, start, caching):
    emu = m.emu
    emu.mem_write(COMPILER, bytes(COMPILER_SIZE))
    emu.mem_write(COMPILER + 0x61F4C, bytes([caching]))
    m.w(COMPILER + 0x61EB0, COMPILER + 0x61000)
    m.w(COMPILER + 0x61EB4, COMPILER + 0x61000)
    m.events = []
    stopped, sp = m.run(start, {RET}, regs={UC_X86_REG_ECX: 0x2261118}, stack=(RET, COMPILER, CFG, 0))
    events, m.events = m.events, None
    assert emu.reg_read(UC_X86_REG_ESP) == sp + 4 + 0xC, "ret 0Ch"
    for reg, value in SAVED.items():
        assert emu.reg_read(reg) == value, ("a callee-saved register", reg)
    return techniques(events)


def check_compiles(stock, patched):
    for m in (stock, patched):
        compile_stubs(m)
    for caching in (0, 1):
        mode = "caching" if caching else "building"
        native = run_compile(stock, 0x1CE100, caching)
        pss = [next(e[4] for e in t if e[0] == "begin_pass") for t in native]
        assert pss == ["wet_surface", "wet_surface_write_normal", "wet_surface_copy"], pss
        for _ in range(2):                                 # the same compiler again: the same again
            ours = run_compile(patched, 0x1CE100, caching)
            assert len(ours) == 6, len(ours)
            assert ours[:3] == native, "the original's three first"
            assert ours[4:] == native[1:], "write_normal and copy again, unchanged"
            twin = [("begin_pass", e[1], e[2], e[3], "wet_surface_v021d", e[5]) if e[0] == "begin_pass" else e
                    for e in native[0]]
            assert ours[3] == twin, "technique 3: technique 0 with 0.21's pixel shader"
        textures = [e[3] for e in native[0] if e[0] == "set_texture"]
        print(f"  ok effect_wet_surface::compile ({mode}): techniques 0..2 the original's, 3..5 the three again,"
              f" 3 with ps wet_surface_v021d ({len(native[0])} calls and writes, {len(textures)} textures"
              f"{', its stencil' if not caching else ''}), the same at a second compile")

        native = run_compile(stock, 0x1D3B90, caching)
        assert [next(e[4] for e in t if e[0] == "begin_pass") for t in native] == ["rain", "rain_sprite_drops"]
        for _ in range(2):
            ours = run_compile(patched, 0x1D3B90, caching)
            assert len(ours) == 3 and ours[:2] == native, len(ours)
            twin = [("set_texture", e[1], e[2], "ported/v021d/engine/rain_drop", e[4])
                    if e[0] == "set_texture" and e[2] == "t_rain_drop" else e for e in native[1]]
            assert twin != native[1]
            assert ours[2] == twin, "technique 2: technique 1 with 0.21's drop texture"
        print(f"  ok effect_rain::compile ({mode}): techniques 0..1 the original's, 2 technique 1's twin with"
              " t_rain_drop ported/v021d/engine/rain_drop, the same at a second compile")
    for m in (stock, patched):
        m.stubs = {}


# -- the map and the stages ---------------------------------------------------------------------------------

def set_project(m, project, scene, config):
    emu = m.emu
    emu.mem_write(project, bytes(0x200))
    m.w(project + 0x17C, PROJECT_CFG)
    m.w(PROJECT_CFG + 0x108, PROJECT_ROOT)
    emu.mem_write(PROJECT_ROOT, relocated_config(config, PROJECT_ROOT))
    m.w(project + 0x180, scene)
    for at in (0x184, 0x188, 0x1B4, 0x1B8):
        m.w(project + at, project + 0x1F0)
    m.w(project + 0x1E8, 0)
    m.w(project + 0x1EC, 0)


def insert(m, project):
    m.run(0x285750, {0x285793}, regs={UC_X86_REG_ECX: project}, stack=(RET, HEAP + 0x3F0000, HEAP + 0x3F1000))


def remove(m, project):
    m.run(0x2855B0, {RET}, regs={UC_X86_REG_ESI: project}, stack=(RET,))


def destroy(m, project):
    m.run(0x285330, {0x28533A}, regs={UC_X86_REG_ECX: project}, stack=(RET,))


def stages(m, drawn, quality, scales):
    """What each stage site does, the frame drawing `drawn`."""
    m.w(PRE_RAIN + 4, CONTEXT)
    m.w(RAIN + 4, CONTEXT)
    m.w(PRE_RAIN + 0x18, WET_EFFECT)
    m.w(RAIN + 0x34, DROPS_EFFECT)
    m.w(CONTEXT + 0x416C, drawn)
    m.w(CONTEXT + 0x4170, VIEW)
    m.w(OPTIONS_SLOT, OPTIONS)
    m.w(OPTIONS + 0xE8, quality)
    m.emu.mem_write(0xA71A35, b"\x01")                     # r_rain_render_wet_surface_effect
    u, v = scales
    m.emu.mem_write(VIEW + 0x2E8, struct.pack("<2f", u, v))
    gate, _ = m.run(0x17CB16, {0x17CB28, 0x17CC39}, regs={UC_X86_REG_ESI: PRE_RAIN})
    m.run(0x17CB90, {0x17CB95}, regs={UC_X86_REG_ESI: PRE_RAIN})
    wet = (m.emu.reg_read(UC_X86_REG_EAX), m.emu.reg_read(UC_X86_REG_ECX))
    frame = bytearray(0x100)
    struct.pack_into("<2f", frame, 0x40, mul32(u, 0.05), mul32(v, 0.4))     # as 0x1ADDA0 / 0x1ADDEF leave them
    _, sp = m.run(0x1ADF7D, {0x1ADF95}, regs={UC_X86_REG_EBP: RAIN}, frame=bytes(frame))
    parameters = bytes(m.emu.mem_read(sp + 0x74, 8))
    m.run(0x1ADFA4, {0x1ADFAC}, regs={UC_X86_REG_EBP: RAIN})
    drops = (m.emu.reg_read(UC_X86_REG_EAX), m.emu.reg_read(UC_X86_REG_ECX))
    return gate, wet, parameters, drops


def f32(x):
    return struct.unpack("<f", struct.pack("<f", x))[0]


def mul32(a, b):
    """A float32 product (mulss)."""
    return f32(f32(a) * f32(b))


class V021d:
    """0.21d's stage_rain::execute, its parameters1.zw (0x123382..0x12343E)."""

    def __init__(self, data):
        self.m = Machine(data, imports=False)
        self.factors = struct.unpack("<2f", bytes(self.m.emu.mem_read(0x7DD148, 4)) + bytes(self.m.emu.mem_read(0x836EDC, 4)))

    def parameters(self, u, v):
        m = self.m
        frame = bytearray(0x100)
        struct.pack_into("<f", frame, 0x48, u)
        struct.pack_into("<2f", frame, 0x30, *self.factors)
        _, sp = m.run(0x123382, {0x12343E}, frame=bytes(frame), regs={UC_X86_REG_XMM0: struct.unpack("<I", struct.pack("<f", v))[0]})
        return bytes(m.emu.mem_read(sp + 0x48, 8))


SCALES = [(0.77, 1.0), (1.0, 1.0), (0.5, 0.25), (1.3, 0.9), (0.0, 0.0), (2.0, 3.5), (0.123, 4.56), (10.0, 0.01)]


def check_map(stock, patched, v021d):
    plain = {s: struct.pack("<2f", mul32(s[0], 0.05), mul32(s[1], 0.4)) for s in SCALES}
    native_wet, native_drops = (WET_EFFECT, 0), (DROPS_EFFECT, 1)

    def expect_native(m, label):
        for drawn in (RENDER_SCENE, LOBBY_SCENE):
            for quality in (0, 1, 3):
                for scales in SCALES:
                    gate, wet, parameters, drops = stages(m, drawn, quality, scales)
                    assert gate == (0x17CB28 if quality else 0x17CC39), (label, hex(gate))
                    assert wet == native_wet and drops == native_drops, (label, wet, drops)
                    assert parameters == plain[scales], (label, scales)

    for m in (stock, patched):
        set_project(m, PROJECT + 0x10000, LOBBY_SCENE - 0x800, {})
        m.w(LOBBY_SCENE - 0x800 + 4, LOBBY_SCENE)
        m.w(GAME_SCENE + 4, RENDER_SCENE)
    expect_native(patched, "nothing loaded")

    # the lobby's project and a project marked false: nothing
    insert(patched, PROJECT + 0x10000)
    set_project(patched, PROJECT + 0x20000, GAME_SCENE, {"srp_renderer_v021d": False})
    insert(patched, PROJECT + 0x20000)
    expect_native(patched, "unmarked projects")
    print("  ok before a marked project, with unmarked ones (no key, the key false): every site the original's")

    halves = differs = 0
    for m in (stock, patched):
        set_project(m, PROJECT, GAME_SCENE, {"srp_renderer_v021d": True, "other": 1})
        insert(m, PROJECT)
    expect_native(stock, "the original, marked")
    for quality in (0, 1, 3):
        for scales in SCALES:
            gate, wet, parameters, drops = stages(patched, RENDER_SCENE, quality, scales)
            assert gate == 0x17CB28, "no shading-quality gate"
            assert wet == (WET_EFFECT, 3) and drops == (DROPS_EFFECT, 2), (wet, drops)
            if v021d is not None:
                assert parameters == v021d.parameters(*scales), (scales, parameters.hex(), v021d.parameters(*scales).hex())
            differs += parameters != plain[scales]
            z, w = struct.unpack("<2f", parameters)
            assert abs(z - mul32(scales[0], 0.05)) <= 0.001 * max(1, scales[0]) and abs(w - mul32(scales[1], 0.4)) <= 0.004 * max(1, scales[1])
            halves += 1
            # the lobby's scene, drawn while the match's project is loaded
            assert stages(patched, LOBBY_SCENE, quality, scales) == stages(stock, LOBBY_SCENE, quality, scales)
    assert differs >= len(SCALES), "the half floats are not the plain products"
    print(f"  ok the marked project's scene: the wet pass at any shading quality, techniques 3 and 2, parameters1.zw"
          f" {'0.21d' + chr(39) + 's own (its exe run), ' if v021d else ''}{halves} cases; the lobby's scene the original's")

    remove(patched, PROJECT + 0x10000)
    gate, wet, _, drops = stages(patched, RENDER_SCENE, 0, SCALES[0])
    assert (gate, wet, drops) == (0x17CB28, (WET_EFFECT, 3), (DROPS_EFFECT, 2)), "another project's removal keeps it"
    remove(patched, PROJECT)
    expect_native(patched, "removed")
    insert(patched, PROJECT)
    assert stages(patched, RENDER_SCENE, 0, SCALES[0])[1] == (WET_EFFECT, 3)
    destroy(patched, PROJECT)
    expect_native(patched, "destroyed")
    print("  ok another project's removal keeps the map; its own removal, or its destruction, ends it")


def god_rays(m, drawn, mode):
    """Where the god-rays pass goes from its entry: its body (the entry's span run) or its caller."""
    m.w(ATMOSPHERE + 4, CONTEXT)
    m.w(ATMOSPHERE + 0xAC, mode)
    m.w(CONTEXT + 0x416C, drawn)
    at, sp = m.run(0x1A5520, {0x1A5528, RET}, stack=(RET, ATMOSPHERE))
    esp = m.emu.reg_read(UC_X86_REG_ESP)
    if at == RET:
        assert esp == sp + 8, "ret 4: the return address and the stage popped"
        assert all(m.emu.reg_read(reg) == value for reg, value in SAVED.items()), "the registers kept"
        return "returned"
    assert esp == sp - 0x18 and m.emu.reg_read(UC_X86_REG_EBX) == ATMOSPHERE, "the entry's span run"
    return "drawn"


def check_god_rays(stock, patched):
    def expect_drawn(m, label):
        for drawn in (RENDER_SCENE, LOBBY_SCENE):
            for mode in (0, 1):
                assert god_rays(m, drawn, mode) == "drawn", (label, hex(drawn), mode)

    expect_drawn(stock, "the original")
    expect_drawn(patched, "nothing marked")
    set_project(patched, PROJECT, GAME_SCENE, {"srp_renderer_v021d": True})
    insert(patched, PROJECT)
    assert god_rays(patched, RENDER_SCENE, 0) == "returned", "the sky's stage on the marked scene"
    assert god_rays(patched, RENDER_SCENE, 1) == "drawn", "the geometry's stage"
    assert god_rays(patched, LOBBY_SCENE, 0) == "drawn", "the lobby's scene"
    remove(patched, PROJECT)
    expect_drawn(patched, "removed")
    print("  ok the god rays: on the marked project's scene the sky stage's pass returns at once (ret 4), the"
          " geometry's runs; the lobby's scene, unmarked or removed, both run, as the original")


def check_v021d_factors(v021d):
    assert struct.pack("<2f", *v021d.factors) == struct.pack("<2f", 0.05, 0.4), v021d.factors
    print("  ok 0.21d's factors are the float32 0.05 and 0.4 (0x7DD148, 0x836EDC)")


# -- the ported materials -----------------------------------------------------------------------------------

def material_pass(m, vs, vertex_input, config):
    emu = m.emu
    emu.mem_write(COMPILER, bytes(COMPILER_SIZE))
    emu.mem_write(COMPILER + 0x61F4C, b"\x01")
    emu.mem_write(SHADER_CONFIG, bytes(range(0x10, 0x20)))
    emu.mem_write(MATERIAL_OPTIONS, struct.pack("<BxxxiIi", vertex_input, -1, 0, -1))
    emu.mem_write(CFG, relocated_config(config, CFG))
    m.run(0x1CD0B0, {0x1CD1BC}, stack=(RET, m.name(vs), m.name("gbuffer_pass"), COMPILER, SHADER_CONFIG, CFG,
                                       MATERIAL_OPTIONS))
    return m.text(emu.reg_read(UC_X86_REG_EAX)), bytes(emu.mem_read(SHADER_CONFIG, 16))


def check_grass(stock, patched):
    marked = {"srp_grass_v021d": {"value": True, "is_model_parameter": False}, "wind_motion": {"value": 1}}
    cases = 0
    for vs in GRASS + ("vertex_base_skinned", "vertex_base_v021d", "z_only"):
        for vertex_input in (0x0B, 0x02, 0x08, 0x00):
            for config in (marked, {"wind_motion": {"value": 1}}, {}):
                native = material_pass(stock, vs, vertex_input, config)
                ours = material_pass(patched, vs, vertex_input, config)
                assert native[0] == vs
                twin = vs in GRASS and vertex_input == 0x0B and config is marked
                assert ours == ((vs + "_v021d") if twin else vs, native[1]), (vs, vertex_input, ours)
                cases += 1
    print(f"  ok compile_begin's begin_pass: a marked grassmesh pass of the 7 families takes <family>_v021d, every"
          f" other ({cases} cases) the original's; the configuration as the original's")


def water(m, config):
    seen = []

    def set_texture(esp):
        seen.append((m.text(m.r(esp + 8)), m.text(m.r(esp + 12))))
        return 0x20, None

    m.stubs = {BEGIN_PASS: lambda esp: (0x1C, None), RELEASE_PASSES: lambda esp: (0, None), SET_TEXTURE: set_texture}
    emu = m.emu
    emu.mem_write(COMPILER, bytes(COMPILER_SIZE))
    emu.mem_write(COMPILER + 0x61F4C, b"\x01")
    emu.mem_write(MATERIAL_OPTIONS, struct.pack("<BxxxiIi", 0x02, -1, 0, -1))
    emu.mem_write(CFG, relocated_config(config, CFG))
    m.run(0x1E5FE0, {0x1E6058}, stack=(RET, COMPILER, CFG, MATERIAL_OPTIONS))
    m.stubs = {}
    return seen[-1]


def check_water(stock, patched):
    marked = {"srp_water_v021d": {"value": True, "is_model_parameter": False}}
    for config in (marked, {"use_rain": {"value": 1}}, {}):
        native, ours = water(stock, config), water(patched, config)
        assert native == ("t_normal_map", "engine/water_waves2"), native
        assert ours == ("t_normal_map", "ported/v021d/engine/water_waves2" if config is marked else "engine/water_waves2")
    print("  ok the forward water's t_normal_map: ported/v021d/engine/water_waves2 for a marked material, the"
          " original's otherwise")


def main():
    if len(sys.argv) not in (2, 3):
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    stock, patched = Machine(original), Machine(bytes(built))
    v021d = V021d(open(sys.argv[2], "rb").read()) if len(sys.argv) == 3 else None
    print("patch 130 (v021d_render), original and patched exe" + (", and 0.21d's" if v021d else ""))
    if v021d:
        check_v021d_factors(v021d)
    check_compiles(stock, patched)
    check_map(stock, patched, v021d)
    check_god_rays(stock, patched)
    check_grass(stock, patched)
    check_water(stock, patched)


if __name__ == "__main__":
    main()
