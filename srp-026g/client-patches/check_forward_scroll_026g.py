#!/usr/bin/env python3
"""Run patch 28, UV scrolling for the transparent effects (the Rust patch
forward_scroll, rust/patches/src/forward_scroll.rs), in the default build on
made-up materials.

    python check_forward_scroll_026g.py <original survarium.exe> [--server=host:port]

Requires Python `unicorn`. The exe's sections are loaded as they are (the
exe is never rebased, and the arena's code holds absolute addresses; until
the Rust port, the asm in `.srp` was also run rebased).

What runs as machine code: effect_fstage_default_materials::compile
(0x1D8F40) from its first instruction to its `ret 0Ch`, of the original exe
and of the build, with both hooks and the code they jump to; and
renderer_context's constructor from scene_time's binding (0x1498A6) to the
instruction after the third hook (0x149919). Stubs: the config's key check
(0xF6C00) and lookup (0xF6B10) over a made-up material (a lookup of a key
the material does not have fails the run: the game's lookup assumes the
key), compile_begin, set_texture, set_constant<float>, <float2>, <float4>,
the colour conversion 0x1BFCB0, color_write_enable, end_pass,
end_technique, the shared string 0xE0B50 and its destroy 0xE0AE0, and
register_constant_binding 0x3BCCC0; each records what it was given, takes
its arguments off as the function does and leaves eax, ecx and edx made up.

Each material must give, in the build: the original's calls, in order and
with the same values, but for the configuration given to compile_begin
(bit 6 of byte 0Ch set when use_uv_scrolling is true and use_sequence is
not) and, when that bit is set, set_constant<float4> of
uv_scrolling_parameters0 = (0, 0, tu, tv) and uv_scrolling_parameters1 =
(eu, ev, 0, 0) right after the uv distortion's constants (a value the
material does not have is 0, an integer is made a float); every register at
the instructions after the two hooks and at compile's return as in the
original; no lookup of a missing key. The constructor must bind
scene_slomo with the game's own calls for scene_time: the shared string of
the name, held while register_constant_binding is given {&value, 4, name,
0, 4} and the resource manager, then let go (destroyed at a count of 0),
the value being a 1.0 in the arena (`.srpx`), the stack dword-aligned at
the call (the arena aligns its own; the asm kept the game's), every register after the hook as the original's. (Until
the Rust port, --rev12= compared the build with revision 12's exe byte by
byte.) Says nothing about what the GPU does.
"""
import hashlib
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import patch_026g as base

REGISTERS = {"eax": UC_X86_REG_EAX, "ecx": UC_X86_REG_ECX, "edx": UC_X86_REG_EDX, "ebx": UC_X86_REG_EBX,
             "esp": UC_X86_REG_ESP, "ebp": UC_X86_REG_EBP, "esi": UC_X86_REG_ESI, "edi": UC_X86_REG_EDI}
COMPILE = 0x1D8F40
SWITCH_AFTER, CONSTANTS_AFTER, SLOMO_AFTER = 0x1D90E3, 0x1D94D2, 0x149919
SLOMO_SITE = 0x14990F                   # the third site (sites_026g.tsv renderer_context.scene_time_bound)
SCENE_TIME_BINDING = 0x1498A6
KEY_EXISTS, CHILD = 0xF6C00, 0xF6B10
RESOURCE_MANAGER = 0x226047C
# (name, what the function takes off the stack)
STUBS = {KEY_EXISTS: ("key_exists", 0), CHILD: ("child", 0), 0x1CD0B0: ("compile_begin", 0x18),
         0x3D98B0: ("set_texture", 0x20), 0x1C0960: ("float", 8), 0x1D9740: ("float2", 8), 0x1A8900: ("float4", 8),
         0x1BFCB0: ("colour", 0), 0x3D9D80: ("color_write_enable", 4), 0x3DA450: ("end_pass", 4),
         0x3DA6B0: ("end_technique", 4), 0xE0B50: ("shared_string", 4), 0xE0AE0: ("destroy", 4),
         0x3BCCC0: ("register_constant_binding", 4)}
STOP_AT, HEAP, STACK = 0x30000000, 0x31000000, 0x32000000
COMPILER, CONTEXT, MANAGER = 0x34000000, 0x35000000, 0x36000000
JUNK = {UC_X86_REG_EAX: 0xA5A50000, UC_X86_REG_ECX: 0xC5C50000, UC_X86_REG_EDX: 0xD5D50000}


class Failed(Exception):
    pass


class Run:
    def __init__(self, data, delta):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        self.delta = delta
        headers = base.Headers(data)
        self.sections = []
        for name, rva, vsize, raw, rsize, _ in headers.sections(data):
            start = base.IMAGE_BASE + rva + delta
            size = (max(vsize, rsize) + 0xFFF) & ~0xFFF
            emu.mem_map(start, size, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
            self.sections.append((name, start, size))
        if delta:
            reloc_rva, reloc_size = struct.unpack_from("<II", data, headers.reloc_at)
            table = next(raw + reloc_rva - rva for _, rva, vsize, raw, rsize, _ in headers.sections(data)
                         if rva <= reloc_rva < rva + vsize)
            at = table
            while at < table + reloc_size:
                page, block = struct.unpack_from("<II", data, at)
                if block < 8:
                    break
                for i in range((block - 8) // 2):
                    entry = struct.unpack_from("<H", data, at + 8 + 2 * i)[0]
                    if entry >> 12 == 3:
                        where = base.IMAGE_BASE + page + (entry & 0xFFF) + delta
                        self.word(where, self.read(where) + delta)
                at += block
        for at, size in ((STOP_AT, 0x1000), (HEAP, 0x400000), (STACK, 0x100000), (COMPILER, 0x80000),
                         (CONTEXT, 0x10000), (MANAGER, 0x1000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        self.stubs = {va + delta: entry for va, entry in STUBS.items()}
        for at in self.stubs:
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=at, end=at)
        emu.hook_add(UC_HOOK_CODE, lambda e, a, s, u: e.emu_stop(), begin=STOP_AT, end=STOP_AT)
        self.snapshots = {}
        for va in (SWITCH_AFTER, CONSTANTS_AFTER):
            emu.hook_add(UC_HOOK_CODE, self.snapshot, begin=va + delta, end=va + delta)
        self.heap = HEAP
        self.calls, self.lookups, self.error, self.shared = [], [], None, 0
        self.nodes = {}                  # node address -> {name: child node}

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def floats(self, at, count):
        return struct.unpack(f"<{count}f", self.emu.mem_read(at, 4 * count))

    def text(self, at):
        out = bytearray()
        while (byte := self.emu.mem_read(at + len(out), 1)[0]) != 0:
            out.append(byte)
        return out.decode("ascii")

    def alloc(self, raw):
        at = self.heap
        self.emu.mem_write(at, raw)
        self.heap += (len(raw) + 15) & ~15
        return at

    def section_of(self, at):
        return next((name.rstrip(b"\0").decode() for name, start, size in self.sections
                     if start <= at < start + size), None)

    # The made-up material: key -> bool, int, float, str (a name) or a tuple
    # of floats (a vector the value points to).
    def material(self, keys):
        node = self.alloc(b"\0" * 0x18)
        self.nodes[node] = {}
        for key, value in keys.items():
            record = bytearray(0x18)
            if isinstance(value, bool) or isinstance(value, int):
                struct.pack_into("<iH", record, 0, int(value), 0)
                struct.pack_into("<H", record, 0x14, 1)
            elif isinstance(value, float):
                struct.pack_into("<f", record, 0, value)
                struct.pack_into("<H", record, 0x14, 2)
            elif isinstance(value, str):
                struct.pack_into("<I", record, 0, self.alloc(value.encode() + b"\0"))
                struct.pack_into("<H", record, 0x14, 4)
            else:
                struct.pack_into("<I", record, 0, self.alloc(struct.pack(f"<{len(value)}f", *value)))
                struct.pack_into("<H", record, 0x14, 5)
            value_node = self.alloc(bytes(record))
            key_node = self.alloc(b"\0" * 0x18)
            self.nodes[key_node] = {"value": value_node}
            self.nodes[value_node] = {}
            self.nodes[node][key] = key_node
        return node

    def snapshot(self, emu, address, size, _):
        values = {name: emu.reg_read(reg) for name, reg in REGISTERS.items()}
        self.snapshots.setdefault(address - self.delta, []).append(values)

    def ret(self, sp, taken, result=None, keep=()):
        emu = self.emu
        for reg, junk in JUNK.items():
            if reg not in keep:
                emu.reg_write(reg, junk | (reg & 0xFFFF))
        if result is not None:
            emu.reg_write(UC_X86_REG_EAX, result & 0xFFFFFFFF)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + taken)

    def stub(self, emu, address, size, _):
        name, taken = self.stubs[address]
        sp = emu.reg_read(UC_X86_REG_ESP)
        caller = self.read(sp) - self.delta
        reg = lambda r: emu.reg_read(r)
        arg = lambda i: self.read(sp + 4 + 4 * i)
        if name == "key_exists":
            node, key = reg(UC_X86_REG_EAX), self.text(reg(UC_X86_REG_ESI))
            if node not in self.nodes:
                return self.fail(f"key check on a node that is none ({node:#x}) from {caller:#x}")
            self.lookups.append(("exists", key))
            return self.ret(sp, 0, (reg(UC_X86_REG_EAX) & 0xFFFFFF00) | (key in self.nodes[node]))
        if name == "child":
            node, key = reg(UC_X86_REG_EAX), self.text(reg(UC_X86_REG_EDI))
            if node not in self.nodes or key not in self.nodes[node]:
                return self.fail(f"lookup of {key!r}, which the node does not have, from {caller:#x}")
            self.lookups.append(("child", key))
            return self.ret(sp, 0, self.nodes[node][key])
        if name == "colour":                # edi = the colour, esi = the result: the result
            out = reg(UC_X86_REG_ESI)
            self.emu.mem_write(out, bytes(self.emu.mem_read(reg(UC_X86_REG_EDI), 12)))
            return self.ret(sp, 0, out)
        if name == "compile_begin":
            record = (name, self.text(arg(0)), self.text(arg(1)), arg(2), bytes(self.emu.mem_read(arg(3), 16)), arg(4))
        elif name == "set_texture":
            record = (name, arg(0), self.text(arg(1)), self.text(arg(2)) if arg(2) else None)
        elif name == "float":
            record = (name, arg(0), self.text(arg(1)), self.floats(reg(UC_X86_REG_EAX), 1))
        elif name in ("float2", "float4"):
            record = (name, arg(0), self.text(arg(1)), self.floats(reg(UC_X86_REG_ECX), 2 if name == "float2" else 4))
        elif name in ("color_write_enable", "end_pass", "end_technique"):
            record = (name, arg(0))
        elif name == "shared_string":
            text = self.text(arg(0))
            record = (name, text)
            self.calls.append((caller, record, sp))
            return self.ret(sp, taken, self.shared)
        elif name == "register_constant_binding":
            binding = reg(UC_X86_REG_EAX)
            words = struct.unpack("<5I", self.emu.mem_read(binding, 20))
            record = (name, arg(0), words, self.floats(words[0], 1), self.section_of(words[0]),
                      self.read(self.shared) if self.shared else None)
        elif name == "destroy":
            record = (name, arg(0), self.read(arg(0)))
        else:
            return self.fail(f"stub {name}")
        self.calls.append((caller, record, sp))
        result = COMPILER if name == "float" else None
        self.ret(sp, taken, result)

    def fail(self, text):
        self.error = text
        self.emu.emu_stop()

    def compile(self, material, compiler=COMPILER):
        """compile(compiler, material, pass) from its first instruction."""
        emu = self.emu
        pass_ = self.alloc(struct.pack("<III", 0, 0, 0xFFFFFFFF))
        sp = STACK + 0x80000
        for value in (pass_, material, compiler, STOP_AT):
            sp -= 4
            self.word(sp, value)
        kept = {UC_X86_REG_EBX: 0x0BB0BB00, UC_X86_REG_ESI: 0x05105100, UC_X86_REG_EDI: 0x0D10D100,
                UC_X86_REG_EBP: 0x0BE0BE00}
        for reg, value in kept.items():
            emu.reg_write(reg, value)
        for reg, junk in JUNK.items():
            emu.reg_write(reg, junk)
        emu.reg_write(UC_X86_REG_ESP, sp)
        emu.emu_start(COMPILE + self.delta, STOP_AT, count=500000)
        if self.error:
            raise Failed(self.error)
        if emu.reg_read(UC_X86_REG_EIP) != STOP_AT:
            raise Failed(f"stopped at {emu.reg_read(UC_X86_REG_EIP):#x}")
        if emu.reg_read(UC_X86_REG_ESP) != sp + 4 + 12 or any(emu.reg_read(r) != v for r, v in kept.items()):
            raise Failed("compile did not return with `ret 0Ch` and its callee-saved registers")
        return [(caller, record) for caller, record, _ in self.calls]

    def constructor(self, start, end, shared_count, has_shared=True):
        """renderer_context's constructor from `start` to `end`: ebx the
        context, edi 0 (as the constructor has them), the resource manager
        at its pointer; the shared string's count `shared_count`."""
        emu = self.emu
        self.word(RESOURCE_MANAGER + self.delta, MANAGER)
        self.shared = self.alloc(struct.pack("<I", shared_count) + b"\0" * 12) if has_shared else 0
        sp = STACK + 0x40000
        regs = {UC_X86_REG_EAX: 0x1111, UC_X86_REG_ECX: 0x2222, UC_X86_REG_EDX: 0x3333, UC_X86_REG_EBX: CONTEXT,
                UC_X86_REG_ESP: sp, UC_X86_REG_EBP: 0x0BE0BE00, UC_X86_REG_ESI: 4, UC_X86_REG_EDI: 0}
        for reg, value in regs.items():
            emu.reg_write(reg, value)
        self.word(CONTEXT + 0x4140, 0)
        emu.emu_start(start + self.delta, end + self.delta, count=10000)
        if self.error:
            raise Failed(self.error)
        if emu.reg_read(UC_X86_REG_EIP) != end + self.delta:
            raise Failed(f"stopped at {emu.reg_read(UC_X86_REG_EIP):#x}")
        return sp, {name: emu.reg_read(reg) for name, reg in REGISTERS.items()}, self.read(sp + 0x2C)


def base_material(**extra):
    keys = {"use_temissive": 1, "use_ttransparency": True, "texture_emissive": "fx/sand_e",
            "texture_transparency": "fx/sand_t", "constant_emissive_multiplier": 1.5,
            "constant_emissive": (0.5, 0.25, 0.125)}
    keys.update(extra)
    return {k: v for k, v in keys.items() if v is not None}


DISTORTION = {"use_uv_distortion": True, "uv_distortion_texture": "fx/noise", "uv_distortion_multiplier": 0.75,
              "distortion_1_moving_direction": (0.1, 0.2, 0.3), "distortion_2_moving_direction": (0.4, 0.5, 0.6),
              "uv_distortion_tile": 3}
SEQUENCE = {"use_sequence": True, "constant_sequence_play_speed": 0.5, "constant_sequence_start_frame_index": 8.0,
            "constant_sequence_array_height": 8, "constant_sequence_array_width": 8.0}
SCROLL_ALL = {"transparency_map_scroll_u_direction": 0.03, "transparency_map_scroll_v_direction": -0.125,
              "emissive_map_scroll_u_direction": 0.5, "emissive_map_scroll_v_direction": 2.25}
TU, TV, EU, EV = (f"{side}_map_scroll_{axis}_direction" for side, axis in
                  (("transparency", "u"), ("transparency", "v"), ("emissive", "u"), ("emissive", "v")))

CASES = [
    # name, material, whether the bit is set, (tu, tv, eu, ev) expected
    ("no use_uv_scrolling key", base_material(**SCROLL_ALL), False, None),
    ("use_uv_scrolling false", base_material(use_uv_scrolling=False, **SCROLL_ALL), False, None),
    ("use_uv_scrolling true, no values", base_material(use_uv_scrolling=True), True, (0, 0, 0, 0)),
    ("use_uv_scrolling true, four floats", base_material(use_uv_scrolling=True, **SCROLL_ALL), True,
     (0.03, -0.125, 0.5, 2.25)),
    ("use_uv_scrolling true, integers", base_material(use_uv_scrolling=True, **{TU: 1, TV: -2, EU: 3, EV: 0}), True,
     (1, -2, 3, 0)),
    ("use_uv_scrolling true, tu and ev only", base_material(use_uv_scrolling=True, **{TU: 0.25, EV: -0.5}), True,
     (0.25, 0, 0, -0.5)),
    ("use_uv_scrolling true and use_sequence", base_material(use_uv_scrolling=True, **SCROLL_ALL, **SEQUENCE),
     False, None),
    ("use_uv_scrolling true, use_sequence false", base_material(use_uv_scrolling=True, use_sequence=False,
                                                               **SCROLL_ALL), True, (0.03, -0.125, 0.5, 2.25)),
    ("use_uv_scrolling true with uv distortion, angle fade, soft edges",
     base_material(use_uv_scrolling=True, use_angle_depended_attenuation=True, attenuation_power=2.0,
                   use_soft_edges=True, **DISTORTION, **SCROLL_ALL), True, (0.03, -0.125, 0.5, 2.25)),
    ("use_uv_distortion, no scrolling", base_material(use_angle_depended_attenuation=True, **DISTORTION), False, None),
    ("constant emissive, no transparency texture, scrolling", base_material(
        use_temissive=0, use_ttransparency=False, texture_emissive=None, texture_transparency=None,
        constant_transparency=0.5, constant_tile_u=2.0, constant_tile_v=4, use_uv_scrolling=1, **SCROLL_ALL), True,
     (0.03, -0.125, 0.5, 2.25)),
]


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    options = dict(a[2:].split("=", 1) for a in sys.argv[1:] if a.startswith("--") and "=" in a)
    if len(args) != 1 or set(options) - {"server"}:
        sys.exit(__doc__)
    original = open(args[0], "rb").read()
    upload = open(base.default_bin("section"), "rb").read()
    server = options.get("server")
    data, _ = base.build(original, "section", upload, server, open(base.late_join_bin(), "rb").read())
    arena = base.arena()                # the code is the arena's (the Rust patch, the engine's thunks)
    ours = lambda caller: arena.va <= caller < arena.va + arena.size
    failed = 0

    def say(ok, text):
        nonlocal failed
        failed += not ok
        print(f"{'ok  ' if ok else 'FAIL'} {text}")

    for delta in (0,):
        rebased = " (rebased)" if delta else ""
        for name, keys, bit, expected in CASES:
            try:
                stock, built = Run(original, delta), Run(data, delta)
                before = stock.compile(stock.material(keys))
                after = built.compile(built.material(keys))
            except Failed as error:
                say(False, f"{name}{rebased}: {error}")
                continue
            # The build's calls from the exe are the original's, the
            # configuration's bit aside; the code's own come between the
            # last call before 0x1D94CB and the first after it.
            from_code = [(i, record) for i, (caller, record) in enumerate(after) if ours(caller)]
            from_exe = [record for caller, record in after if not ours(caller)]
            originals = []
            for _, record in before:
                if record[0] == "compile_begin" and bit:
                    cfg = bytearray(record[4])
                    cfg[0x0C] |= 0x40
                    record = record[:4] + (bytes(cfg),) + record[5:]
                originals.append(record)
            split = sum(1 for caller, _ in before if caller < 0x1D94CB)
            if expected is None:
                wanted = []
            else:
                tu, tv, eu, ev = (f32(v) for v in expected)
                wanted = [("float4", COMPILER, "uv_scrolling_parameters0", (0.0, 0.0, tu, tv)),
                          ("float4", COMPILER, "uv_scrolling_parameters1", (eu, ev, 0.0, 0.0))]
            cfg_after = next(r[4] for _, r in after if r[0] == "compile_begin")
            cfg_before = next(r[4] for _, r in before if r[0] == "compile_begin")
            ok = from_exe == originals and [r for _, r in from_code] == wanted \
                and [i for i, _ in from_code] == list(range(split, split + len(wanted)))
            # The lookups the build adds: whether the material has each name
            # it reads, and a lookup only of a name it has (and its value).
            extra = sorted(set(built.lookups) - set(stock.lookups))
            asked = {"use_uv_scrolling"} if not keys.get("use_sequence") else set()
            if bit:
                asked |= {TU, TV, EU, EV}
            ok &= set(extra) <= {("exists", k) for k in asked} | {("child", k) for k in asked & set(keys)}
            same = [(after_va, built.snapshots.get(after_va) == stock.snapshots.get(after_va))
                    for after_va in (SWITCH_AFTER, CONSTANTS_AFTER)]
            ok &= all(s for _, s in same) and all(stock.snapshots.get(v) for v, _ in same)
            shown = "; ".join(f"{r[2]} = ({', '.join(f'{x:g}' for x in r[3])})" for r in wanted) or "no constant"
            say(ok, f"{name}{rebased}: configuration byte 0Ch {cfg_before[0x0C]:#04x} -> {cfg_after[0x0C]:#04x}, "
                    f"{len(from_exe)} calls as the original's, {shown}; registers after both hooks as the original's"
                    + ("" if ok else f"\n     got {[r for _, r in from_code]} at {[i for i, _ in from_code]} (split {split}),"
                       f" extra lookups {extra}, same {same}, exe calls equal {from_exe == originals}"))

        # The binding of scene_slomo, against the game's own of scene_time.
        for count, has_shared, label in ((1, True, "a pooled name"), (0, True, "a name nobody else holds"),
                                         (0, False, "no name")):
            try:
                game = Run(original, delta)
                game_sp, _, _ = game.constructor(SCENE_TIME_BINDING, SLOMO_SITE, count, has_shared)
                stock = Run(original, delta)
                stock_sp, stock_regs, stock_slot = stock.constructor(SLOMO_SITE, SLOMO_AFTER, count, has_shared)
                built = Run(data, delta)
                built_sp, built_regs, built_slot = built.constructor(SLOMO_SITE, SLOMO_AFTER, count, has_shared)
            except Failed as error:
                say(False, f"scene_slomo, {label}{rebased}: {error}")
                continue
            shape = lambda run: [(r[0], r[1]) if r[0] != "register_constant_binding" else
                                 (r[0], r[1], r[2][1:], r[5]) for _, r, _ in run.calls]
            sps = lambda run, start_sp: [(start_sp - sp) % 16 for _, r, sp in run.calls if r[0] == "register_constant_binding"]
            ours = [r for _, r, _ in built.calls if r[0] == "register_constant_binding"]
            theirs = [r for _, r, _ in game.calls if r[0] == "register_constant_binding"]
            ok = [(n if n != "shared_string" else n) for n, *_ in shape(built)] == [n for n, *_ in shape(game)]
            ok &= [s[1] for s in shape(built) if s[0] == "shared_string"] == ["scene_slomo"]
            ok &= [s[1] for s in shape(game) if s[0] == "shared_string"] == ["scene_time"]
            ok &= [s[1:] for s in shape(built) if s[0] != "shared_string"] == \
                [s[1:] for s in shape(game) if s[0] != "shared_string"]
            ok &= len(ours) == 1 and ours[0][1] == MANAGER and ours[0][3] == (1.0,) and ours[0][4] == base.ARENA_NAME.rstrip(b"\0").decode() \
                and ours[0][2][1:] == (4, built.shared, 0, 4) and theirs[0][2][0] == CONTEXT + 0x4140
            ok &= (built.read(built.shared) if has_shared else 0) == (game.read(game.shared) if has_shared else 0)
            # The arena aligns its own stack (16 at every Rust call): at the call the
            # stack is dword-aligned, which is all MSVC's x86 code assumes (the
            # constructor aligns its own frame to 8; until the Rust port the
            # asm kept the game's alignment mod 16, which this compared).
            ok &= len(sps(built, built_sp)) == 1 and all(s % 4 == 0 for s in sps(built, built_sp) + sps(game, game_sp))
            ok &= built_regs == stock_regs and built_slot == stock_slot and not stock.calls
            say(ok, f"scene_slomo, {label}{rebased}: the game's calls for scene_time with the name scene_slomo, "
                    f"bound to {ours[0][3][0] if ours else None} in .srpx; registers and [esp+2Ch] after the hook "
                    "as the original's; the stack dword-aligned at the call"
                    + ("" if ok else f"\n     ours {shape(built)}\n     game {shape(game)}\n     "
                       f"{built_regs} / {stock_regs}\n     alignment {sps(built, built_sp)} / {sps(game, game_sp)}"))

    sha = hashlib.sha256(data).hexdigest()
    print(f"     the build{' with ' + server if server else ''}: {len(data)} bytes, sha256 {sha}")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
