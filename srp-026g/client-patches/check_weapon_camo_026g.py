#!/usr/bin/env python3
"""Run patch 44, a weapon's camouflage (weapon_camo_026g.asm), in the default
build on made-up weapon configs and models.

    python check_weapon_camo_026g.py <original survarium.exe>

Requires Python `unicorn`. Builds the default exe with patch_026g.py and
loads both exes' sections as they are (the exe is never rebased).

What runs as machine code: the instruction at 0x24B0D2 of
weapon_cook::on_weapon_subresources_ready to the next one (0x24B0D7), of the
original and of the build (its call into `.srpw`), with the game's
allocation name (0x420789), camouflage_decal_cook_parameters' constructor
(0x119760, with the vector constructor and the buffer strings' own),
buffer_string's assign (0x127C0), intrusive_ptr's release (0x11540), and,
for each bake asked for, the callback as the resources manager calls it:
boost::function's invoker (0x127700) of the manager 0x9ED148 and the
build's bake_done. Stubs: the config's key check (0xF6C00) and lookup
(0xF6B10) over a made-up config (a lookup of a key the table does not have
fails the run: the game's lookup assumes the key), get_all_surfaces
(0x159700), the allocator (0xE4F90), buffer_string's appendf (0xE45B0, as
printf with %s and %d, cut at the buffer's capacity), type_to_int (0x119590),
query_resources (0xE5B60) and deallocate (0x1539D0); each records what it
was given and takes its arguments off as the function does.

Each case must give, in the build: every register, the flags' carry and
zero bits, the stack pointer and what the stack holds as the original
leaves them at 0x24B0D7; with a non-empty `data/camouflage` text, an
`object/model` text and a model with a render instance, one query a
surface (at most 16), each of one request {"", 6Ch} with a variant of
concrete_type_helper<P*> (the P, a pointer to itself, the type),
the game's allocator and no parent, its function bind's manager | 1 with
bake_done and the P; the P from the game's allocator with the allocation
name the editor gives, its camouflage names (`<camo>`, `<camo>_nmap`,
`<camo>_smap`, no fresnel), the identity in its camouflage matrix, the
render instance (one reference more each), the surface's index, the scale
(`data/camo_scale`, else 4.0), 1 at +79Ch, 0 at +7A0h and the name
`srpw1_<camo>_<model>_<index>` with '/' and '\\' made '_'; after each
callback the reference given back and the P deallocated from the game's
allocator. Without one of them: no query, no allocation and no lookup of a
key a table does not have. The original asks for nothing.
"""
import os
import re
import struct
import sys
import tempfile

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_EFLAGS

import patch_026g as base

REGISTERS = {"eax": UC_X86_REG_EAX, "ecx": UC_X86_REG_ECX, "edx": UC_X86_REG_EDX, "ebx": UC_X86_REG_EBX,
             "esp": UC_X86_REG_ESP, "ebp": UC_X86_REG_EBP, "esi": UC_X86_REG_ESI, "edi": UC_X86_REG_EDI}
SITE, AFTER = 0x24B0D2, 0x24B0D7
INVOKER = 0x127700
BIND_MANAGER, VARIANT_HELPER, EMPTY = 0x9ED148, 0x9EBE74, 0x9DB708
GAME_ALLOCATOR, ALLOCATION_NAME = 0x1F1D9C4, 0xAC1228
TYPE_ID = 0x7E57
P_SIZE = 0x8B8
MAX_SURFACES = 16                       # weapon_camo_026g.asm: the surfaces baked at most
STUBS = {0xF6C00: ("key_exists", 0), 0xF6B10: ("child", 0), 0x159700: ("get_all_surfaces", 8),
         0xE4F90: ("allocate", 4), 0xE45B0: ("appendf", 0), 0x119590: ("type_id", 0),
         0xE5B60: ("query_resources", 0), 0x1539D0: ("deallocate", 0), 0xE0210: ("destroyed", 0)}
STOP_AT, HEAP, STACK = 0x30000000, 0x31000000, 0x32000000
ALLOCATOR, RESULT = 0x34000000, 0x35000000
T_BOOL, T_INT, T_FLOAT, T_TABLE, T_STRING = 0, 1, 2, 3, 5
CAMO = "character/human/base_camo/multicam_512"
MODEL = "weapons/uzi"


class Failed(Exception):
    pass


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        headers = base.Headers(data)
        self.sections = []
        for name, rva, vsize, raw, rsize, _ in headers.sections(data):
            start = base.IMAGE_BASE + rva
            size = (max(vsize, rsize) + 0xFFF) & ~0xFFF
            emu.mem_map(start, size, UC_PROT_ALL)
            if rsize:
                emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
            self.sections.append((name, start, size))
        for at, size in ((STOP_AT, 0x1000), (HEAP, 0x400000), (STACK, 0x100000), (ALLOCATOR, 0x1000),
                         (RESULT, 0x1000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        self.word(GAME_ALLOCATOR, ALLOCATOR)
        for at in STUBS:
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=at, end=at)
        emu.hook_add(UC_HOOK_CODE, lambda e, a, s, u: e.emu_stop(), begin=STOP_AT, end=STOP_AT)
        emu.hook_add(UC_HOOK_CODE, lambda e, a, s, u: e.emu_stop(), begin=AFTER, end=AFTER)
        self.heap = HEAP
        self.nodes = {}                  # table node -> {key: child record}
        self.calls, self.error = [], None
        self.surfaces = 0

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def text(self, at):
        out = bytearray()
        while (byte := self.emu.mem_read(at + len(out), 1)[0]) != 0:
            out.append(byte)
        return out.decode("ascii")

    def alloc(self, raw, fill=None):
        at = self.heap
        self.emu.mem_write(at, raw)
        self.heap += (len(raw) + 15) & ~15
        return at

    def record(self, value):
        """A config value of 18h bytes, as the loaded config holds it."""
        out = bytearray(0x18)
        if isinstance(value, dict):
            node = self.alloc(bytes(out))
            self.nodes[node] = {key: self.record(child) for key, child in value.items()}
            struct.pack_into("<HH", out, 0x14, T_TABLE, len(value))
            self.emu.mem_write(node, bytes(out))
            return node
        if isinstance(value, bool):
            struct.pack_into("<I", out, 0, int(value))
            struct.pack_into("<HH", out, 0x14, T_BOOL, 4)
        elif isinstance(value, int):
            struct.pack_into("<i", out, 0, value)
            struct.pack_into("<HH", out, 0x14, T_INT, 4)
        elif isinstance(value, float):
            struct.pack_into("<f", out, 0, value)
            struct.pack_into("<HH", out, 0x14, T_FLOAT, 4)
        else:
            struct.pack_into("<I", out, 0, self.alloc(value.encode() + b"\0"))
            struct.pack_into("<HH", out, 0x14, T_STRING, len(value) + 1)
        return self.alloc(bytes(out))

    def ret(self, sp, taken, result=None):
        if result is not None:
            self.emu.reg_write(UC_X86_REG_EAX, result & 0xFFFFFFFF)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + taken)

    def fail(self, text):
        self.error = text
        self.emu.emu_stop()

    def string(self, at):
        begin, end, cap = struct.unpack("<3I", self.emu.mem_read(at, 12))
        return begin, end, cap

    def stub(self, emu, address, size, _):
        name, taken = STUBS[address]
        sp = emu.reg_read(UC_X86_REG_ESP)
        caller = self.read(sp)
        reg = emu.reg_read
        arg = lambda i: self.read(sp + 4 + 4 * i)
        if name == "key_exists":
            node, key = reg(UC_X86_REG_EAX), self.text(reg(UC_X86_REG_ESI))
            if node not in self.nodes:
                return self.fail(f"key check on {node:#x}, no table, from {caller:#x}")
            self.calls.append((name, key))
            return self.ret(sp, 0, (reg(UC_X86_REG_EAX) & 0xFFFFFF00) | (key in self.nodes[node]))
        if name == "child":
            node, key = reg(UC_X86_REG_EAX), self.text(reg(UC_X86_REG_EDI))
            if node not in self.nodes or key not in self.nodes[node]:
                return self.fail(f"lookup of {key!r}, which the table does not have, from {caller:#x}")
            self.calls.append((name, key))
            return self.ret(sp, 0, self.nodes[node][key])
        if name == "get_all_surfaces":
            model, vector = arg(0), arg(1)
            begin, end, cap = self.string(vector)
            for i in range(self.surfaces):
                if end + 4 > cap:
                    return self.fail("get_all_surfaces: the vector is full")
                self.word(end, 0x5F000000 + i)
                end += 4
            self.word(vector + 4, end)
            self.calls.append((name, model, (cap - begin) // 4))
            return self.ret(sp, taken)
        if name == "allocate":
            size, allocator, where = reg(UC_X86_REG_EAX), reg(UC_X86_REG_ESI), arg(0)
            block = self.alloc(b"\xCD" * size)
            self.calls.append((name, size, allocator, where, block))
            return self.ret(sp, taken, block)
        if name == "appendf":
            target, fmt = reg(UC_X86_REG_EAX), self.text(arg(0))
            values, k = [], 1
            for spec in re.findall(r"%[sd]", fmt):
                values.append(self.text(arg(k)) if spec == "%s" else struct.unpack("<i", struct.pack("<I", arg(k)))[0])
                k += 1
            text = (fmt.replace("%s", "{}").replace("%d", "{}")).format(*values).encode()
            begin, end, cap = self.string(target)
            text = text[:cap - begin - 1]       # vsnprintf into cap - begin bytes
            self.emu.mem_write(end, text + b"\0")
            self.word(target + 4, end + len(text))
            self.calls.append((name, fmt, tuple(values)))
            return self.ret(sp, taken, target)
        if name == "type_id":
            return self.ret(sp, taken, TYPE_ID)
        if name == "query_resources":
            requests, count = reg(UC_X86_REG_EAX), reg(UC_X86_REG_ECX)
            function, allocator, user_data, parent, flag = (arg(i) for i in range(5))
            reqs = [struct.unpack("<II", self.emu.mem_read(requests + 8 * i, 8)) for i in range(count)]
            variant = self.read(user_data)
            words = struct.unpack("<12I", self.emu.mem_read(variant, 0x30))
            fobj = struct.unpack("<5I", self.emu.mem_read(function, 20))
            p = words[2]
            self.calls.append((name, reqs, variant, words, fobj, allocator, parent, flag, p, self.parameters(p)))
            return self.ret(sp, taken)
        if name == "deallocate":
            memory, allocator, slot = reg(UC_X86_REG_EAX), reg(UC_X86_REG_ECX), arg(0)
            self.calls.append((name, memory, allocator, self.read(slot)))
            self.word(slot, 0)
            return self.ret(sp, taken)
        return self.fail(f"{name} from {caller:#x}")

    def parameters(self, p):
        names = {}
        for field, at in (("diffuse", 0x520), ("normal", 0x5AC), ("fresnel", 0x638), ("smoothness", 0x6C4),
                          ("name", 0x7A4)):
            begin, end, cap = self.string(p + at)
            names[field] = bytes(self.emu.mem_read(begin, end - begin)).decode()
        matrix = struct.unpack("<16f", self.emu.mem_read(p + 0x4E0, 64))
        model, surface = self.read(p + 0x790), self.read(p + 0x794)
        scale = struct.unpack("<f", self.emu.mem_read(p + 0x798, 4))[0]
        one, editor = self.read(p + 0x79C), self.emu.mem_read(p + 0x7A0, 1)[0]
        return names, matrix, model, surface, scale, one, editor

    def site(self, config, model, regs):
        """0x24B0D2 to 0x24B0D7 with the original's [esp+20h] = model and
        [esp+3Ch] = config."""
        emu = self.emu
        sp = STACK + 0x80000
        frame = bytearray(struct.pack("<I", 0x11111111) * 0x20)
        struct.pack_into("<I", frame, 0x20, model)
        struct.pack_into("<I", frame, 0x3C, config)
        emu.mem_write(sp - 0x100, b"\xEE" * 0x100)
        emu.mem_write(sp, bytes(frame))
        for name, reg in REGISTERS.items():
            emu.reg_write(reg, regs[name])
        emu.reg_write(UC_X86_REG_ESP, sp)
        emu.reg_write(UC_X86_REG_EFLAGS, 0x203)                      # carry set, zero clear
        emu.emu_start(SITE, STOP_AT, count=2_000_000)
        if self.error:
            raise Failed(self.error)
        if emu.reg_read(UC_X86_REG_EIP) != AFTER:
            raise Failed(f"stopped at {emu.reg_read(UC_X86_REG_EIP):#x}, not {AFTER:#x}")
        out = {name: emu.reg_read(reg) for name, reg in REGISTERS.items()}
        out["flags"] = emu.reg_read(UC_X86_REG_EFLAGS) & 0x41
        out["stack"] = bytes(emu.mem_read(out["esp"], 0x44))
        return out

    def callback(self, fobj):
        """The resources manager's call of the function when the query ends:
        the manager's invoker with the function's buffer and the result."""
        emu = self.emu
        vtable = fobj[0] & ~1
        invoker = self.read(vtable + 4)
        buffer = self.alloc(struct.pack("<3I", *fobj[2:5]))
        sp = STACK + 0x40000
        emu.mem_write(sp, struct.pack("<3I", STOP_AT, buffer, RESULT))
        emu.reg_write(UC_X86_REG_ESP, sp)
        emu.emu_start(invoker, STOP_AT, count=1_000_000)
        if self.error:
            raise Failed(self.error)
        if emu.reg_read(UC_X86_REG_ESP) != sp + 4:
            raise Failed("the callback did not return as cdecl")
        return invoker


def make_render(run, references=5):
    render = run.alloc(b"\0" * 0x100)
    run.word(render + 0xD0, references)
    model = run.alloc(b"\0" * 0x200)
    run.word(model + 0x108, render)
    return model, render


def config(run, root):
    resource = run.alloc(b"\0" * 0x200)
    run.word(resource + 0x108, run.record(root) if root is not None else 0)
    return resource


def weapon(data=None, model=MODEL):
    root = {"parameters": {"weight": 3.5, "camo_available": True}}
    if model is not None:
        root["object"] = {"model": model, "skeleton": model}
    if data is not None:
        root["data"] = data
    return root


REGS = {"eax": 0xA0A0A0A0, "ecx": 0xC0C0C0C0, "edx": 0xD0D0D0D0, "ebx": 0xB0B0B0B0, "esp": 0,
        "ebp": 0xBEBEBEBE, "esi": 0x51515151, "edi": 0x7A7A7A7A}


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    build, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(),
                          late_join=late_join)
    failures = 0

    def say(ok, text):
        nonlocal failures
        failures += not ok
        print(("ok    " if ok else "FAIL  ") + text)

    plain = (
        ("no data table", weapon(None), 3, True),
        ("a data table without camouflage", weapon({"camo_scale": 2.0}), 3, True),
        ("an empty camouflage", weapon({"camouflage": ""}), 3, True),
        ("a camouflage that is no text", weapon({"camouflage": 7}), 3, True),
        ("no object table", weapon({"camouflage": CAMO}, model=None), 3, True),
        ("no model", {"data": {"camouflage": CAMO}, "object": {"skeleton": MODEL}}, 3, True),
        ("no config root", None, 3, True),
        ("no model instance", weapon({"camouflage": CAMO}), 3, False),
        ("no surfaces", weapon({"camouflage": CAMO}), 0, True),
    )
    baked = (
        ("three surfaces", weapon({"camouflage": CAMO}), 3, 4.0, MODEL, CAMO),
        ("camo_scale 2.5", weapon({"camouflage": CAMO, "camo_scale": 2.5}), 1, 2.5, MODEL, CAMO),
        ("an integer camo_scale is not taken", weapon({"camouflage": CAMO, "camo_scale": 3}), 1, 4.0, MODEL, CAMO),
        ("twelve surfaces: twelve", weapon({"camouflage": CAMO}), 12, 4.0, MODEL, CAMO),
        ("twenty surfaces: sixteen", weapon({"camouflage": CAMO}), 20, 4.0, MODEL, CAMO),
        ("a model path with \\", weapon({"camouflage": "camo\\x"}, model="weapons\\ak_74n/premium"), 2, 4.0,
         "weapons\\ak_74n/premium", "camo\\x"),
    )
    for title, root, surfaces, with_render in plain:
        results = []
        for exe in (original, build):
            run = Run(exe)
            run.surfaces = surfaces
            model, render = make_render(run)
            if not with_render:
                run.word(model + 0x108, 0)
            regs = dict(REGS)
            out = run.site(config(run, root), model, regs)
            results.append((out, run))
        (o, ro), (b, rb) = results
        queries = [c for c in rb.calls if c[0] in ("query_resources", "allocate")]
        say(o == b and not queries and not ro.calls,
            f"{title}: no query, the registers and the stack as the original's"
            + ("" if o == b else f" (original {o}, build {b})"))
    for title, root, surfaces, scale, model_path, camo in baked:
        o_run = Run(original)
        o_run.surfaces = surfaces
        o_model, _ = make_render(o_run)
        o = o_run.site(config(o_run, root), o_model, dict(REGS))
        run = Run(build)
        run.surfaces = surfaces
        model, render = make_render(run, references=5)
        b = run.site(config(run, root), model, dict(REGS))
        say(o == b, f"{title}: the registers, flags and stack at 0x24B0D7 as the original's")
        say(not o_run.calls, f"{title}: the original asks for nothing")
        queries = [c for c in run.calls if c[0] == "query_resources"]
        allocations = [c for c in run.calls if c[0] == "allocate"]
        want = min(surfaces, MAX_SURFACES)
        say(len(queries) == want and len(allocations) == want,
            f"{title}: {want} queries and allocations (got {len(queries)}, {len(allocations)})")
        say(run.read(render + 0xD0) == 5 + want, f"{title}: the render instance holds {want} references more")
        safe = lambda text: text.replace("/", "_").replace("\\", "_")
        for i, (q, a) in enumerate(zip(queries, allocations)):
            _, reqs, variant, words, fobj, allocator, parent, flag, p, params = q
            names, matrix, p_model, surface, p_scale, one, editor = params
            identity = tuple(float(r == c) for r in range(4) for c in range(4))
            say(a[1] == P_SIZE and a[2] == ALLOCATOR and a[3] == ALLOCATION_NAME and a[4] == p,
                f"{title} #{i}: P of 8B8h bytes from the game's allocator, the editor's allocation name")
            say(reqs == [(EMPTY, 0x6C)], f"{title} #{i}: one request {{\"\", 6Ch}}")
            say(words[0] == VARIANT_HELPER and words[2] == p and words[10] == variant and words[11] == TYPE_ID,
                f"{title} #{i}: the variant: helper, P, itself, the type")
            say(allocator == ALLOCATOR and parent == 0 and flag == 1, f"{title} #{i}: the game's allocator, no parent, 1")
            bake_done = fobj[2]
            say(fobj[0] == BIND_MANAGER | 1 and fobj[1] == 0 and fobj[3] == 0 and fobj[4] == p
                and run.section_name(bake_done) == ".srpw",
                f"{title} #{i}: the function: bind's manager | 1, bake_done in .srpw, 0, P")
            say(names == {"diffuse": camo, "normal": camo + "_nmap", "fresnel": "", "smoothness": camo + "_smap",
                          "name": f"srpw1_{safe(camo)}_{safe(model_path)}_{i}"},
                f"{title} #{i}: the names {names}")
            say(matrix == identity, f"{title} #{i}: the camouflage's matrix is the identity")
            say(p_model == render and surface == i and abs(p_scale - scale) < 1e-6 and one == 1 and editor == 0,
                f"{title} #{i}: render instance, surface {i}, scale {scale}, 1, 0")
        for i, q in enumerate(queries):
            before = len(run.calls)
            invoker = run.callback(q[4])
            done = [c for c in run.calls[before:] if c[0] == "deallocate"]
            say(invoker == INVOKER and done == [("deallocate", q[8], ALLOCATOR, q[8])],
                f"{title} #{i}: the callback deallocates P from the game's allocator")
        say(run.read(render + 0xD0) == 5, f"{title}: every reference given back")
    print("all checks passed" if not failures else f"{failures} FAILED")
    sys.exit(1 if failures else 0)


def section_name(self, at):
    return next((name.rstrip(b"\0").decode() for name, start, size in self.sections if start <= at < start + size),
                None)


Run.section_name = section_name

if __name__ == "__main__":
    main()
