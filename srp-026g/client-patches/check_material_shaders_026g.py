#!/usr/bin/env python3
"""Patch 221 (material_shaders): a material draws with the shaders, textures and constants its
own config names (../docs/notes/client_patch.md section 3.116).

    python check_material_shaders_026g.py <original survarium.exe>

Runs the game's own code in the original exe and in the patched one (Unicorn), with patch
130's harness (check_rain_v021d_026g.py: the made-up compiler, the relocated config trees,
the engine's begin_pass and set_texture stubbed and recorded):

- effect_material_base::compile_begin (0x1CD0B0) whole, for PS families with a material
  whose srp_pixel_shader replaces that family, one that replaces another, a malformed key and
  none: begin_pass gets the material's family exactly when `from` is the pass's own, else the
  original's; the shader configuration as the original's; set_constant<float4> (0x1A8900) is
  called with each well-formed srp_constants entry (its name and four values, in order, after
  the pass began) and never for a material without the key; compile_begin returns as the
  original (ret 18h, the callee-saved registers);
- effect_fstage_simpe_water_materials::compile (0x1E5FE0) from its start to past its
  t_normal_map set_texture (0x1E6058), technique 0: the forward water's PS forward_water
  becomes the material's family, its t_normal_map the material's srp_textures entry for that
  slot; a material without the keys (or srp_water_v021d's, patch 130) binds as before.
"""
import importlib.util
from pathlib import Path
import struct
import sys

from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_ESP

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('check_rain_v021d_026g', HERE / 'check_rain_v021d_026g.py')
rig = importlib.util.module_from_spec(spec)
sys.path.insert(0, str(HERE))
spec.loader.exec_module(rig)
base = rig.base

SET_CONSTANT_FLOAT4 = 0x1A8900
OURS = "forward_water_069d"
NORMALS = "v069d_cologne/tiled/water/water_nmap"
CONSTANTS = [{"name": "water_069d_layer0", "value": [0.25, 0.25, 0.002, -0.002]},
             {"name": "water_069d_medium", "value": [0.75, 20.0, 1.0, 0.0]}]


def arr(items):
    """An indexed table (a config array: content_edit writes a dict of 0..n-1 as one)."""
    return {i: v for i, v in enumerate(items)}


def constants(items):
    return arr([{"name": c["name"], "value": arr(c["value"])} for c in items])


def marked(source="forward_water", target=OURS):
    return {"srp_pixel_shader": {"from": source, "to": target},
            "srp_constants": constants(CONSTANTS),
            "srp_textures": arr([{"slot": "t_normal_map", "texture_path": NORMALS}]),
            "wave_power": {"value": 0.05, "is_model_parameter": True}}


def recording_stubs(m, seen):
    def begin_pass(esp):
        seen.append(("begin_pass", m.text(m.r(esp + 12))))
        return 0x1C, None

    def set_texture(esp):
        seen.append(("set_texture", m.text(m.r(esp + 8)), m.text(m.r(esp + 12))))
        return 0x20, None

    def set_constant(esp):
        value = struct.unpack("<4f", bytes(m.emu.mem_read(m.emu.reg_read(UC_X86_REG_ECX), 16)))
        seen.append(("set_constant", m.text(m.r(esp + 8)), tuple(round(v, 6) for v in value)))
        return 8, None

    m.stubs = {rig.BEGIN_PASS: begin_pass, rig.SET_TEXTURE: set_texture, rig.RELEASE_PASSES: lambda esp: (0, None),
               SET_CONSTANT_FLOAT4: set_constant, rig.SET_ALPHA_BLEND: lambda esp: (0x0C, None)}


def compile_begin(m, ps, config):
    """compile_begin (0x1CD0B0) whole with `config`: what it called, the configuration it left."""
    emu = m.emu
    seen = []
    recording_stubs(m, seen)
    emu.mem_write(rig.COMPILER, bytes(rig.COMPILER_SIZE))
    m.w(rig.COMPILER + 0x61EB0, rig.COMPILER + 0x61000)
    m.w(rig.COMPILER + 0x61EB4, rig.COMPILER + 0x61000)
    emu.mem_write(rig.SHADER_CONFIG, bytes(range(0x10, 0x20)))
    emu.mem_write(rig.MATERIAL_OPTIONS, struct.pack("<BxxxiIi", 0x01, -1, 0, -1))
    emu.mem_write(rig.CFG, rig.relocated_config(config, rig.CFG))
    _, sp = m.run(0x1CD0B0, {rig.RET}, stack=(rig.RET, m.name("vertex_base"), m.name(ps), rig.COMPILER,
                                              rig.SHADER_CONFIG, rig.CFG, rig.MATERIAL_OPTIONS))
    assert emu.reg_read(UC_X86_REG_ESP) == sp + 4 + 0x18, "ret 18h"
    for reg, value in rig.SAVED.items():
        assert emu.reg_read(reg) == value, ("a callee-saved register", reg)
    m.stubs = {}
    return seen, bytes(emu.mem_read(rig.SHADER_CONFIG, 16))


def check_compile_begin(stock, patched):
    expected_constants = [("set_constant", c["name"], tuple(round(v, 6) for v in c["value"])) for c in CONSTANTS]
    cases = 0
    for ps in ("forward_water", "forward_water_local_reflections", "gbuffer_pass"):
        for config in (marked(), marked("gbuffer_pass", "gbuffer_pass_ours"), {"srp_pixel_shader": "forward_water_069d"},
                       {"srp_constants": constants([{"name": "half", "value": [1.0, 2.0]}, CONSTANTS[1]])},
                       {"wave_power": {"value": 0.05}}, {}):
            native, native_config = compile_begin(stock, ps, config)
            ours, ours_config = compile_begin(patched, ps, config)
            assert native[0] == ("begin_pass", ps), native
            assert native_config == ours_config, ps
            source = config.get("srp_pixel_shader")
            to = source["to"] if isinstance(source, dict) and source["from"] == ps else ps
            assert ours[0] == ("begin_pass", to), (ps, config, ours[0])
            sent = [e for e in ours[1:] if e[0] == "set_constant"]
            if "srp_constants" in config:
                wanted = [("set_constant", c["name"], tuple(round(v, 6) for v in c["value"].values()))
                          for c in config["srp_constants"].values() if len(c["value"]) == 4]
                assert sent == wanted, (ps, sent)
            else:
                assert sent == [], (ps, sent)
            assert [e for e in ours if e[0] != "set_constant"][1:] == native[1:], (ps, ours, native)
            cases += 1
    assert expected_constants
    print(f"  ok compile_begin: a pass begins with the material's srp_pixel_shader family exactly when `from` is its"
          f" own, its srp_constants sent after it began (a malformed entry skipped), else as the original ({cases} cases)")


def water(m, config):
    """effect_fstage_simpe_water_materials::compile from its start to past t_normal_map's set_texture."""
    emu = m.emu
    seen = []
    recording_stubs(m, seen)
    emu.mem_write(rig.COMPILER, bytes(rig.COMPILER_SIZE))
    emu.mem_write(rig.COMPILER + 0x61F4C, b"\x01")
    emu.mem_write(rig.MATERIAL_OPTIONS, struct.pack("<BxxxiIi", 0x01, -1, 0, -1))
    emu.mem_write(rig.CFG, rig.relocated_config(config, rig.CFG))
    m.run(0x1E5FE0, {0x1E6058}, stack=(rig.RET, rig.COMPILER, rig.CFG, rig.MATERIAL_OPTIONS))
    m.stubs = {}
    return [e for e in seen if e[0] != "set_constant"]


def check_water(stock, patched):
    for config in (marked(), {"srp_water_v021d": {"value": True, "is_model_parameter": False}}, {"use_rain": {"value": 1}}, {}):
        native, ours = water(stock, config), water(patched, config)
        assert native == [("begin_pass", "forward_water"), ("set_texture", "t_normal_map", "engine/water_waves2")], native
        if config.get("srp_pixel_shader"):
            assert ours == [("begin_pass", OURS), ("set_texture", "t_normal_map", NORMALS)], ours
        elif "srp_water_v021d" in config:
            assert ours == [("begin_pass", "forward_water"), ("set_texture", "t_normal_map", "ported/v021d/engine/water_waves2")], ours
        else:
            assert ours == native, ours
    print(f"  ok the forward water's technique 0: {OURS} and the material's t_normal_map for a material that names them,"
          " patch 130's 0.21 wave normals for srp_water_v021d, the original's otherwise")


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    stock, patched = rig.Machine(original), rig.Machine(bytes(built))
    print("patch 221 (material_shaders), original and patched exe")
    check_compile_begin(stock, patched)
    check_water(stock, patched)


if __name__ == "__main__":
    main()
