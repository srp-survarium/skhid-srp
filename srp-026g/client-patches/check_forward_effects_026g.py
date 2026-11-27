#!/usr/bin/env python3
"""Run patch 169, the forward effects as 0.44a5 draws them (rust/patches
forward_effects.rs), in the default build on made-up materials.

    python check_forward_effects_026g.py <original survarium.exe>

Requires Python `unicorn`. Runs as machine code, of the original exe and of
the build: effect_fstage_default_materials::compile (0x1D8F40, forward_default)
and effect_offscreen_particle_lighting_materials::compile (0x1DB170,
particle_lighting), each from its first instruction to its `ret 0Ch`, with
the build's three hooks and the Rust they enter. The material is a binary
config as the loader makes it (a table node per key, its `value` child), so
the patch reads it as it reads a game's; the stubs are patch 28's check's
(the config's key check and lookup over the made-up material, compile_begin,
the constant and texture setters, end_pass, ...) and set_alpha_blend; pow
runs as it is.

Each material must give, in the build, the original's calls in order and
with the same values but for:
- the configuration compile_begin is given: byte 0Dh bit 6 (CONFIG_USE_OLTA)
  set when use_ttransparency and use_ttransparency_dxt1 are on, bit 7
  (CONFIG_USE_DISPLACEMENT) when use_uv_tiling is on and use_sequence is not,
  bit 0 (CONFIG_USE_FUZZINESS) when apply_distortion_to_emissive_only is on
  (forward_default); bit 6 alone for particle_lighting; every other byte and
  bit the original's;
- forward_default's constant_tile_uv: with the tiling bit, the patch's
  set_constant<float4> of (emissive_map_tiling_u_factor, _v,
  transparency_map_tiling_u_factor, _v), 1 for each one missing, right
  before solid_color_specular's, so before the game's own block sends its
  float2 (which the build still calls);
and compile must return with `ret 0Ch` and its callee-saved registers. The
materials without these keys (every material players have) give exactly the
original's calls.

The first value stands: the game's own set_constant<float4> (0x1A8900) and
set_constant<float2> (0x1D9740) run as they are on a made-up compiler, with
its binding search (0x3C3DD0) as it is and stubs for the name's shared string,
the value pool and push_back: a name already bound adds no binding (the
block's float2 after the patch's float4), a new name does. Says nothing about
what the GPU does.

probe_diffuse_mult, as 0.44a5's particle pass sets it: stage_lights' constructor
from 0x19015D (after its last host) to the next instruction, the game's
shared string, holder assign, register_constant_host and destroy stubbed (a
name's host made once), then stage_lights::accumulate_particle_lighting from
0x19834B (where its two ways meet) over its 9 bytes, set_ps_constant stubbed:
the build registers one host named probe_diffuse_mult with the backend at
[2260490h] and lets its holder go (the string destroyed at a count of 0);
each pass sets it to the probe's diffuse multiplier (+328h) times the scene
view's probes diffuse intensity (+290h), 0 without a probe, nothing before
the constructor ran; the registers after both spans the original's.
"""
import hashlib
import struct
import sys

from unicorn.x86_const import UC_X86_REG_ESP, UC_X86_REG_EIP, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_ECX, UC_X86_REG_EAX, UC_X86_REG_EDX, \
    UC_X86_REG_EDI, UC_X86_REG_EBP

import check_forward_scroll_026g as scroll
import patch_026g as base

FORWARD, PARTICLE_LIGHTING = 0x1D8F40, 0x1DB170
scroll.STUBS[0x3E2EA0] = ("set_alpha_blend", 12)
OLTA, DISPLACEMENT, FUZZINESS = 0x40, 0x80, 0x01


scroll.STUBS[0x1D9800] = ("pool2", 12)
scroll.STUBS[0x1AA970] = ("pool4", 20)
scroll.STUBS[0x3C1110] = ("push_back", 0)
SETTERS = {0x1D9740, 0x1A8900}
scroll.STUBS[0x11170] = ("assign", 0)
scroll.STUBS[0x3C8BC0] = ("register_constant_host", 8)
scroll.STUBS[0x132A90] = ("set_ps_constant", 4)
CONSTRUCTED, CONSTRUCTED_AFTER = 0x19015D, 0x190162
PARTICLE_PROBE, PARTICLE_PROBE_AFTER = 0x19834B, 0x198354
BACKEND_GLOBAL, BACKEND, HOST = 0x2260490, 0x37000000, 0x37100000
THREAD_SLOT = 0x870118                     # GetCurrentThreadId's import (the lock's owner)
BINDINGS, BINDING_SIZE = 0x61C1C, 0x14


class Run(scroll.Run):
    real_setters = False
    probe_mode = False

    def stub(self, emu, address, size, user):
        name, taken = self.stubs[address]
        if self.probe_mode:
            return self.probe_stub(emu, address, name, taken)
        if self.real_setters:
            return self.setter_stub(emu, address, name, taken)
        if name != "set_alpha_blend":
            return super().stub(emu, address, size, user)
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.calls.append((self.read(sp) - self.delta, (name, self.read(sp + 4)), sp))
        self.ret(sp, taken)

    def probe_stub(self, emu, address, name, taken):
        """The registration's and the setter's stand-ins: a string a text,
        a host a name (made once), every call kept."""
        sp = emu.reg_read(UC_X86_REG_ESP)
        caller = self.read(sp) - self.delta
        reg = emu.reg_read
        if name == "shared_string":
            text = self.text(self.read(sp + 4))
            string = self.alloc(struct.pack("<I", 0) + b"\0" * 12)
            self.strings[string] = text
            self.probe_calls.append((caller, ("create", text)))
            return self.ret(sp, taken, string)
        if name == "assign":
            holder, string = reg(UC_X86_REG_EDI), reg(UC_X86_REG_EAX)
            self.word(holder, string)
            self.word(string, self.read(string) + 1)
            self.probe_calls.append((caller, ("assign", self.strings.get(string))))
            return self.ret(sp, taken, keep=(UC_X86_REG_EAX,))
        if name == "register_constant_host":
            backend, holder, aux = reg(UC_X86_REG_EAX), self.read(sp + 4), self.read(sp + 8)
            text = self.strings.get(self.read(holder))
            self.word(self.read(holder), self.read(self.read(holder)) + 1)   # the host keeps the name
            self.probe_calls.append((caller, ("register", backend, text, aux)))
            return self.ret(sp, taken, HOST)
        if name == "destroy":
            string = self.read(sp + 4)
            self.probe_calls.append((caller, ("destroy", self.strings.get(string), self.read(string))))
            return self.ret(sp, taken)
        if name == "set_ps_constant":
            host, backend = reg(UC_X86_REG_EAX), reg(UC_X86_REG_EDI)
            self.probe_calls.append((caller, ("set_ps_constant", host, backend, self.floats(self.read(sp + 4), 4))))
            return self.ret(sp, taken)
        return self.fail(f"stub {name} in the probe's run")

    def span(self, start, end, regs):
        """The game's code from `start` to `end` with these registers; the registers after."""
        self.probe_mode, self.probe_calls = True, []
        self.strings = getattr(self, "strings", {})
        emu = self.emu
        emu.reg_write(UC_X86_REG_ESP, scroll.STACK + 0x70000)
        for reg, value in regs.items():
            emu.reg_write(reg, value)
        emu.emu_start(start + self.delta, end + self.delta, count=200000)
        if self.error:
            raise scroll.Failed(self.error)
        if emu.reg_read(UC_X86_REG_EIP) != end + self.delta:
            raise scroll.Failed(f"stopped at {emu.reg_read(UC_X86_REG_EIP):#x}")
        return {n: emu.reg_read(r) for n, r in scroll.REGISTERS.items()}

    def setter_stub(self, emu, address, name, taken):
        """The game's setters run; their helpers are made up: one shared
        string a name, the pool's index 0, push_back keeping the name."""
        sp = emu.reg_read(UC_X86_REG_ESP)
        if address - self.delta in SETTERS:
            return
        if name == "shared_string":
            text = self.text(self.read(sp + 4))
            if text not in self.interned:
                self.interned[text] = self.alloc(struct.pack("<I", 1) + b"\0" * 12)
            return self.ret(sp, taken, self.interned[text])
        if name in ("pool2", "pool4"):
            return self.ret(sp, taken, 0)
        if name == "destroy":
            return self.ret(sp, taken)
        if name == "push_back":
            binding = emu.reg_read(UC_X86_REG_EDI)
            self.pushed.append(self.read(binding + 8))
            return self.ret(sp, taken)
        return self.fail(f"stub {name} in the setters' run")

    def bound(self, names):
        """The compiler's binding vector holding `names`."""
        self.interned = getattr(self, "interned", {})
        records = b""
        for text in names:
            if text not in self.interned:
                self.interned[text] = self.alloc(struct.pack("<I", 1) + b"\0" * 12)
            records += struct.pack("<IIIII", 0, 8, self.interned[text], 0, 0)
        start = self.alloc(records + b"\0" * BINDING_SIZE)
        self.word(scroll.COMPILER + BINDINGS, start)
        self.word(scroll.COMPILER + BINDINGS + 4, start + len(records))
        self.emu.mem_write(scroll.COMPILER + 0x61F4C, b"\0")

    def set_constant(self, entry, name, value):
        """The game's set_constant<float2|float4>(&value, compiler, name); the names push_back was given."""
        self.real_setters, self.pushed = True, []
        emu = self.emu
        at = self.alloc(struct.pack(f"<{len(value)}f", *value))
        sp = scroll.STACK + 0x60000
        for word in (self.alloc(name.encode() + b"\0"), scroll.COMPILER, scroll.STOP_AT):
            sp -= 4
            self.word(sp, word)
        emu.reg_write(UC_X86_REG_ESP, sp)
        emu.reg_write(UC_X86_REG_ECX, at)
        emu.emu_start(entry + self.delta, scroll.STOP_AT, count=100000)
        if self.error:
            raise scroll.Failed(self.error)
        if emu.reg_read(UC_X86_REG_EIP) != scroll.STOP_AT or emu.reg_read(UC_X86_REG_ESP) != sp + 4 + 8:
            raise scroll.Failed(f"set_constant {entry:#x} did not return with `ret 8`")
        names = {v: k for k, v in self.interned.items()}
        return [names.get(p, hex(p)) for p in self.pushed]

    def node(self, kind, data=0, count=0):
        record = bytearray(0x18)
        struct.pack_into("<I", record, 0, data & 0xFFFFFFFF)
        struct.pack_into("<HH", record, 0x14, kind, count)
        return self.alloc(bytes(record))

    def material(self, keys):
        """A binary config as the loader makes it: the material a table, each
        key a table whose one child is its `value`."""
        values = {}
        for key, value in keys.items():
            if isinstance(value, bool):
                leaf = self.node(0, int(value))
            elif isinstance(value, int):
                leaf = self.node(1, value)
            elif isinstance(value, float):
                leaf = self.node(2, struct.unpack("<I", struct.pack("<f", value))[0])
            elif isinstance(value, str):
                leaf = self.node(5, self.alloc(value.encode() + b"\0"), len(value) + 1)
            else:
                leaf = self.node(4, self.alloc(struct.pack(f"<{len(value)}f", *value)), len(value))
            values[key] = self.node(3, leaf, 1)
            self.nodes[values[key]] = {"value": leaf}
            self.nodes[leaf] = {}
        first = next(iter(values.values()), 0)
        root = self.node(3, first, len(values))
        self.nodes[root] = values
        return root

    def compile_at(self, entry, material):
        emu = self.emu
        pass_ = self.alloc(struct.pack("<III", 0, 0, 0xFFFFFFFF))
        sp = scroll.STACK + 0x80000
        for value in (pass_, material, scroll.COMPILER, scroll.STOP_AT):
            sp -= 4
            self.word(sp, value)
        kept = {UC_X86_REG_EBX: 0x0BB0BB00, UC_X86_REG_ESI: 0x05105100, UC_X86_REG_EDI: 0x0D10D100,
                UC_X86_REG_EBP: 0x0BE0BE00}
        for reg, value in kept.items():
            emu.reg_write(reg, value)
        for reg, junk in scroll.JUNK.items():
            emu.reg_write(reg, junk)
        emu.reg_write(UC_X86_REG_ESP, sp)
        emu.emu_start(entry + self.delta, scroll.STOP_AT, count=2000000)
        if self.error:
            raise scroll.Failed(self.error)
        if emu.reg_read(UC_X86_REG_EIP) != scroll.STOP_AT:
            raise scroll.Failed(f"stopped at {emu.reg_read(UC_X86_REG_EIP):#x}")
        if emu.reg_read(UC_X86_REG_ESP) != sp + 4 + 12 or any(emu.reg_read(r) != v for r, v in kept.items()):
            raise scroll.Failed("compile did not return with `ret 0Ch` and its callee-saved registers")
        return [(caller, record) for caller, record, _ in self.calls]


def forward(**extra):
    keys = {"use_temissive": True, "use_ttransparency": True, "texture_emissive": "fx/sand_e",
            "texture_transparency": "fx/sand_t", "constant_emissive_multiplier": 1.5,
            "constant_emissive": (0.5, 0.25, 0.125, 1.0), "use_soft_edges": True}
    keys.update(extra)
    return {k: v for k, v in keys.items() if v is not None}


def lighting(**extra):
    keys = {"use_tdiffuse": False, "use_nmap": True, "use_ttransparency": True,
            "texture_normal": "fx/clouds_n", "texture_transparency": "fx/clouds_t", "constant_diffuse": (1.0, 1.0, 1.0, 1.0)}
    keys.update(extra)
    return {k: v for k, v in keys.items() if v is not None}


TILING = {"emissive_map_tiling_u_factor": 0.7, "emissive_map_tiling_v_factor": 2,
          "transparency_map_tiling_u_factor": 0.4, "transparency_map_tiling_v_factor": 0.5}
SEQUENCE = {"use_sequence": True, "constant_sequence_play_speed": 0.5, "constant_sequence_start_frame_index": 8.0,
            "constant_sequence_array_height": 8, "constant_sequence_array_width": 8.0}

# name, compile, material, the bits byte 0Dh gains, constant_tile_uv the patch sends (or None)
CASES = [
    ("forward, none of the keys (players' materials)", FORWARD, forward(), 0, None),
    ("forward, constant_tile_u/v of its own", FORWARD, forward(constant_tile_u=2.0, constant_tile_v=4), 0, None),
    ("forward, dxt1 off", FORWARD, forward(use_ttransparency_dxt1=False), 0, None),
    ("forward, dxt1 (sand_vortex)", FORWARD, forward(use_ttransparency_dxt1=True, use_uv_tiling=False, **TILING), OLTA, None),
    ("forward, dxt1 without a transparency map", FORWARD,
     forward(use_ttransparency=False, texture_transparency=None, constant_transparency=0.5, use_ttransparency_dxt1=True), 0, None),
    ("forward, dxt1 as an integer", FORWARD, forward(use_ttransparency_dxt1=1), OLTA, None),
    ("forward, tiling, four factors", FORWARD, forward(use_uv_tiling=True, **TILING), DISPLACEMENT, (0.7, 2, 0.4, 0.5)),
    ("forward, tiling, no factors", FORWARD, forward(use_uv_tiling=True), DISPLACEMENT, (1, 1, 1, 1)),
    ("forward, tiling, the transparency's v only (ground_wind's are u and v)", FORWARD,
     forward(use_uv_tiling=True, transparency_map_tiling_v_factor=0.4), DISPLACEMENT, (1, 1, 1, 0.4)),
    ("forward, tiling over constant_tile_u/v", FORWARD,
     forward(use_uv_tiling=True, constant_tile_u=2.0, constant_tile_v=4, **TILING), DISPLACEMENT, (0.7, 2, 0.4, 0.5)),
    ("forward, tiling and a sequence", FORWARD, forward(use_uv_tiling=True, **TILING, **SEQUENCE), 0, None),
    ("forward, tiling, use_sequence false", FORWARD, forward(use_uv_tiling=True, use_sequence=False, **TILING),
     DISPLACEMENT, (0.7, 2, 0.4, 0.5)),
    ("forward, distortion of the emissive map alone", FORWARD,
     forward(use_uv_distortion=True, uv_distortion_texture="fx/noise", uv_distortion_multiplier=0.75,
             distortion_1_moving_direction=(0.1, 0.2, 0.3), distortion_2_moving_direction=(0.4, 0.5, 0.6),
             uv_distortion_tile=3, apply_distortion_to_emissive_only=True), FUZZINESS, None),
    ("forward, all three (ground_wind's dxt1 and tiling, the emissive distortion)", FORWARD,
     forward(use_ttransparency_dxt1=True, use_uv_tiling=True, apply_distortion_to_emissive_only=True, **TILING),
     OLTA | DISPLACEMENT | FUZZINESS, (0.7, 2, 0.4, 0.5)),
    ("particle_lighting, none of the keys", PARTICLE_LIGHTING, lighting(), 0, None),
    ("particle_lighting, dxt1 (clouds_v1)", PARTICLE_LIGHTING, lighting(use_ttransparency_dxt1=True), OLTA, None),
    ("particle_lighting, dxt1 without a transparency map", PARTICLE_LIGHTING,
     lighting(use_ttransparency=False, texture_transparency=None, use_ttransparency_dxt1=True), 0, None),
    ("particle_lighting, tiling is forward's alone", PARTICLE_LIGHTING, lighting(use_uv_tiling=True, **TILING), 0, None),
]


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    upload = open(base.default_bin("section"), "rb").read()
    data, _ = base.build(original, "section", upload, None, open(base.late_join_bin(), "rb").read())
    failed = 0

    def say(ok, text):
        nonlocal failed
        failed += not ok
        print(f"{'ok  ' if ok else 'FAIL'} {text}")

    def ours(caller):
        return caller >= base.ARENA_VA

    for name, entry, keys, bits, tile in CASES:
        try:
            stock, built = Run(original, 0), Run(data, 0)
            before = stock.compile_at(entry, stock.material(keys))
            after = built.compile_at(entry, built.material(keys))
        except scroll.Failed as error:
            say(False, f"{name}: {error}")
            continue
        wanted = []
        ours_wanted = ("float4", scroll.COMPILER, "constant_tile_uv", tuple(f32(v) for v in tile)) if tile else None
        for _, record in before:
            if record[0] == "compile_begin":
                cfg = bytearray(record[4])
                cfg[0x0D] |= bits
                record = record[:4] + (bytes(cfg),) + record[5:]
            if ours_wanted and record[0] == "float4" and record[2] == "solid_color_specular":
                wanted.append(ours_wanted)
            wanted.append(record)
        got = [record for _, record in after]
        from_code = [record for caller, record in after if ours(caller)]
        ok = got == wanted and from_code == ([ours_wanted] if tile else [])
        cfg_before = next(r[4] for _, r in before if r[0] == "compile_begin")
        cfg_after = next(r[4] for _, r in after if r[0] == "compile_begin")
        constants = [r for r in got if r[0] in ("float2", "float4") and r[2] == "constant_tile_uv"]
        shown = ", then ".join(f"{r[0]} constant_tile_uv ({', '.join(f'{x:g}' for x in r[3])})" for r in constants) \
            or "no constant_tile_uv"
        say(ok, f"{name}: byte 0Dh {cfg_before[0x0D]:#04x} -> {cfg_after[0x0D]:#04x}, {shown}, "
                f"{len(got)} calls as the original's otherwise"
                + ("" if ok else f"\n     got  {got}\n     want {wanted}\n     ours {from_code}"))

    # The first value of a name stands: the game's setters on a made-up compiler.
    try:
        run = Run(original, 0)
        run.bound([])
        first = run.set_constant(0x1A8900, "constant_tile_uv", (0.7, 2.0, 0.4, 0.5))
        run.bound(["solid_color_specular", "constant_tile_uv"])
        again = run.set_constant(0x1D9740, "constant_tile_uv", (1.0, 1.0))
        other = run.set_constant(0x1D9740, "uv_distortion_tile", (3.0, 3.0))
        ok = first == ["constant_tile_uv"] and again == [] and other == ["uv_distortion_tile"]
        say(ok, f"set_constant: the float4 of a new name adds its binding {first}; the float2 of a bound name adds none "
                f"{again}; of another name its own {other}")
    except scroll.Failed as error:
        say(False, f"set_constant: {error}")

    # probe_diffuse_mult: the constructor's registration, then the particle pass's value.
    stage, context, view, probe, frame = 0x38000000, 0x38100000, 0x38200000, 0x38300000, 0x38400000
    def world(run, probe_there):
        run.emu.mem_map(0x38000000, 0x600000)
        run.word(BACKEND_GLOBAL, BACKEND)
        run.word(stage + 4, context)
        run.word(context + 0x4170, view)
        run.emu.mem_write(view + 0x290, struct.pack("<f", 0.8))
        run.emu.mem_write(probe + 0x328, struct.pack("<f", 1.5))
        run.word(frame - 0x24, probe if probe_there else 0)
        run.emu.mem_write(0x38500000, bytes.fromhex("b801000000c3"))   # GetCurrentThreadId: mov eax,1 / ret
        run.word(THREAD_SLOT, 0x38500000)
    ctor_regs = {UC_X86_REG_EBX: stage, UC_X86_REG_ESI: 0, UC_X86_REG_EDI: 0x0D10D100, UC_X86_REG_EBP: 0x0BE0BE00,
                 UC_X86_REG_EAX: 0x1234, UC_X86_REG_ECX: 0x2345, UC_X86_REG_EDX: 0x3456}
    pass_regs = {UC_X86_REG_ESI: stage, UC_X86_REG_EBX: BACKEND, UC_X86_REG_EBP: frame, UC_X86_REG_EDI: 0x0D10D100,
                 UC_X86_REG_EAX: 0x1234, UC_X86_REG_ECX: 0x2345, UC_X86_REG_EDX: 0x3456}
    for probe_there, wanted in ((True, f32(1.5) * f32(0.8)), (False, 0.0)):
        label = "a probe" if probe_there else "no probe"
        try:
            stock, built = Run(original, 0), Run(data, 0)
            for run in (stock, built):
                world(run, probe_there)
            ctor_stock, ctor_built = stock.span(CONSTRUCTED, CONSTRUCTED_AFTER, ctor_regs), built.span(CONSTRUCTED, CONSTRUCTED_AFTER, ctor_regs)
            registered = [c for _, c in built.probe_calls]
            pass_stock = stock.span(PARTICLE_PROBE, PARTICLE_PROBE_AFTER, pass_regs)
            pass_built = built.span(PARTICLE_PROBE, PARTICLE_PROBE_AFTER, pass_regs)
            sets = [c for _, c in built.probe_calls]
        except scroll.Failed as error:
            say(False, f"probe_diffuse_mult, {label}: {error}")
            continue
        ok = registered == [("create", "probe_diffuse_mult"), ("assign", "probe_diffuse_mult"),
                            ("register", BACKEND, "probe_diffuse_mult", 0)] \
            and sets == [("set_ps_constant", HOST, BACKEND, (f32(wanted), 0.0, 0.0, 0.0))] \
            and not stock.probe_calls and ctor_built == ctor_stock and pass_built == pass_stock
        say(ok, f"probe_diffuse_mult, {label}: registered once ({len(registered)} calls), set to "
                f"{sets[0][3][0] if sets else None:g}; the registers after both spans the original's"
                + ("" if ok else f"\n     registered {registered}\n     sets {sets}\n     {ctor_built} / {ctor_stock}\n     {pass_built} / {pass_stock}"))
    try:
        fresh = Run(data, 0)
        world(fresh, True)
        fresh.span(PARTICLE_PROBE, PARTICLE_PROBE_AFTER, pass_regs)
        say(fresh.probe_calls == [], f"probe_diffuse_mult before the constructor: no set ({fresh.probe_calls})")
    except scroll.Failed as error:
        say(False, f"probe_diffuse_mult before the constructor: {error}")

    sha = hashlib.sha256(data).hexdigest()
    print(f"     the build: {len(data)} bytes, sha256 {sha}")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
