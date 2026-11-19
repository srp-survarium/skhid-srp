#!/usr/bin/env python3
"""Patch 162 (v011w_render): the ported 0.11 weathers render their atmosphere the 0.11 way
(srp-026g/docs/notes/v011-weather.md, README "The 0.11 weathers' fog").

    python check_v011w_render_026g.py <original survarium.exe>

Runs the game's own code in the original exe and in the patched one (Unicorn), with
stage_atmosphere::execute's engine calls stubbed and recorded (res_effect::apply_pass,
backend::set_ps_constant, renderer_context::get_rt, stage_atmosphere::fill_surfaces,
environment_sun_direction):

- the geometry stage's fog (0x1A50E4 to apply_pass at 0x1A50FA): technique 5, as the original,
  before any project, with a project without the key, on the lobby's scene, and after the marked
  project's removal or destruction; 3 on the marked project's own scene;
- after its apply_pass (0x1A50FF to past the s_eye_ray_corner set at 0x1A5111): the original's one
  constant; on the marked scene the inscatter host first, (the project's distance, 1, 0, 0);
- the god rays' entry (0x1A5520): the original's body at once; in the geometry stage after a
  multiplicative pass, first the additive pass: apply_pass with technique 4, then
  to_sun_direction_parameter (minus the environment's sun direction, .w 1 as 0.11e's),
  inscatter_parameters and s_eye_ray_corner (the context's +53A8h), get_rt(37h) and fill_surfaces
  with that target, no secondary and the flag 1 (as execute's 0x1A5111..0x1A512E); once: the next
  god rays without a new multiplicative pass draw nothing more; the sky stage's never; the body
  then runs as the original's.
- effect_atmospheric_scattering::compile (0x1D21F0), building and caching, twice each: the original's six
  techniques, then the six again, 6 and 7 with the pixel shaders make_mie_rayleigh_texture_v011w and
  atmosphere_v011w (all else as 0 and 1), 8..11 as 2..5;
- the sky stage's tables (0x1A4300's decision, run to its setne at 0x1A430F; 0x1A441D's apply, to its
  call at 0x1A4422) and sky (0x1A459D, to apply_pass at 0x1A45AA): the original's everywhere before a
  marked project and on the lobby's scene; on the marked scene technique 6 for the tables and 7 for the
  sky, and the tables remade (the decision's CL 1 though the ids are equal) whenever a frame's look is
  not the one they were made with, and not otherwise.
"""
import struct
import sys

from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_ECX, UC_X86_REG_EDI, UC_X86_REG_ESI

import patch_026g as base
import check_rain_v021d_026g as rig
from check_rain_v021d_026g import (Machine, set_project, insert, remove, destroy, compile_stubs, run_compile, HEAP, RET,
                                   PROJECT, GAME_SCENE, RENDER_SCENE, LOBBY_SCENE, CONTEXT, VIEW, ATMOSPHERE)

EFFECT = HEAP + 0x380000
HOSTS = {"to_sun": HEAP + 0x390000, "inscatter": HEAP + 0x390100, "eye_ray": HEAP + 0x390200}
BACKEND, BACKEND_SLOT = HEAP + 0x3A0000, 0x2260490
TARGET = HEAP + 0x3B0000
APPLY_PASS, SET_PS_CONSTANT, GET_RT, FILL, SUN = 0x3D7230, 0x132A90, 0x11C1D0, 0x1A3690, 0x34E020
SUN_DIRECTION = (0.25, -0.5, 0.8)
EYE_RAY = (0.7, 0.4, 1.0)


def stubs(m):
    def constant(esp):
        host = m.emu.reg_read(UC_X86_REG_EAX)
        name = next((n for n, at in HOSTS.items() if at == host), hex(host))
        data = struct.unpack("<4f", bytes(m.emu.mem_read(m.r(esp + 4), 16)))
        return 4, ("constant", name, m.emu.reg_read(UC_X86_REG_EDI) == BACKEND, tuple(round(x, 5) for x in data))

    def get_rt(esp):
        m.w(m.emu.reg_read(UC_X86_REG_EAX), TARGET)
        return 4, ("get_rt", m.r(esp + 4) == CONTEXT, m.emu.reg_read(UC_X86_REG_ECX))

    def sun(esp):
        environment, out = m.emu.reg_read(UC_X86_REG_EAX), m.emu.reg_read(UC_X86_REG_ESI)
        m.emu.mem_write(out, struct.pack("<3f", *SUN_DIRECTION))
        m.emu.reg_write(UC_X86_REG_EAX, out)
        return 0, ("sun", environment == VIEW + 0x118)

    m.stubs = {
        APPLY_PASS: lambda esp: (0, ("apply_pass", m.emu.reg_read(UC_X86_REG_EAX) == EFFECT, m.r(EFFECT + 0x5620))),
        SET_PS_CONSTANT: constant,
        GET_RT: get_rt,
        FILL: lambda esp: (16, ("fill", m.r(esp + 4) == ATMOSPHERE, m.r(esp + 8) == TARGET, m.r(esp + 12), m.r(esp + 16))),
        SUN: sun,
    }


def stage(m, drawn, mode):
    m.w(ATMOSPHERE + 4, CONTEXT)
    m.w(ATMOSPHERE + 0x50, EFFECT)
    m.w(ATMOSPHERE + 0x6C, HOSTS["to_sun"])
    m.w(ATMOSPHERE + 0xA4, HOSTS["inscatter"])
    m.w(ATMOSPHERE + 0xA8, HOSTS["eye_ray"])
    m.w(ATMOSPHERE + 0xAC, mode)
    m.w(CONTEXT + 0x416C, drawn)
    m.w(CONTEXT + 0x4170, VIEW)
    m.emu.mem_write(CONTEXT + 0x53A8, struct.pack("<3f", *EYE_RAY))
    m.w(BACKEND_SLOT, BACKEND)


def fog(m, drawn):
    """The geometry fog's technique, then the constants set after its apply_pass."""
    stage(m, drawn, 1)
    m.w(EFFECT + 0x5620, 0)
    m.run(0x1A50E4, {0x1A50FA}, regs={UC_X86_REG_EBP: ATMOSPHERE})
    technique = m.r(EFFECT + 0x5620)
    m.events = []
    m.run(0x1A50FF, {0x1A5111}, regs={UC_X86_REG_EBP: ATMOSPHERE, UC_X86_REG_ESI: CONTEXT + 0x53A8})
    events, m.events = m.events, None
    return technique, events


def god_rays(m, drawn, mode):
    stage(m, drawn, mode)
    m.events = []
    where = rig.god_rays(m, drawn, mode)
    events, m.events = m.events, None
    return where, events


ORIGINAL_CONSTANTS = [("constant", "eye_ray", True, (0.7, 0.4, 1.0, 0.0))]

# A weather's profile as the edit list v011w-renderer-1.json writes it (the evening's tone values).
FILMIC = {"shoulder_strength": 0.05, "linear_strength": 7, "linear_angle": 0.3, "toe_strength": 6.2,
          "toe_numerator": 2, "toe_denominator": 0.02, "linear_white": 10000, "dark_desaturation_limit": 4.5}
PACKED = ((0.05, 7.0, 0.3, 6.2), (2.0, 0.02, 10000.0, 4.5))


def profile(distance):
    return {"inscatter_distance": distance, **FILMIC}


def add_pass(distance):
    to_sun = tuple(round(-x, 5) for x in SUN_DIRECTION) + (1.0,)
    return [("apply_pass", True, 4), ("sun", True), ("constant", "to_sun", True, to_sun),
            ("constant", "inscatter", True, (distance, 1.0, 0.0, 0.0)), ("constant", "eye_ray", True, EYE_RAY + (0.0,)),
            ("get_rt", True, 0x37), ("fill", True, True, 0, 1)]


def native(m, label):
    for drawn in (RENDER_SCENE, LOBBY_SCENE):
        assert fog(m, drawn) == (5, ORIGINAL_CONSTANTS), (label, hex(drawn), fog(m, drawn))
        for mode in (0, 1):
            assert god_rays(m, drawn, mode) == ("drawn", []), (label, hex(drawn), mode)


def check(stock, patched):
    for m in (stock, patched):
        stubs(m)
    native(stock, "the original")
    native(patched, "before any project")
    patched.w(GAME_SCENE + 4, RENDER_SCENE)               # the game scene's render scene (project.render_scene)
    set_project(patched, PROJECT, GAME_SCENE, {"srp_renderer_v021d": False})
    insert(patched, PROJECT)
    native(patched, "a project without the key")
    remove(patched, PROJECT)
    for distance, gone in ((200, remove), (300.0, destroy)):
        set_project(patched, PROJECT, GAME_SCENE, {"srp_renderer_v011w": profile(distance)})
        insert(patched, PROJECT)
        assert fog(patched, LOBBY_SCENE) == (5, ORIGINAL_CONSTANTS), "the lobby's scene"
        assert god_rays(patched, LOBBY_SCENE, 1) == ("drawn", []), "the lobby's god rays"
        technique, events = fog(patched, RENDER_SCENE)
        assert technique == 3, technique
        assert events == [("constant", "inscatter", True, (float(distance), 1.0, 0.0, 0.0))] + ORIGINAL_CONSTANTS, events
        assert god_rays(patched, RENDER_SCENE, 0) == ("drawn", []), "the sky stage draws no fog"
        assert god_rays(patched, RENDER_SCENE, 1) == ("drawn", add_pass(float(distance))), god_rays
        assert god_rays(patched, RENDER_SCENE, 1) == ("drawn", []), "the additive pass once"
        gone(patched, PROJECT)
        native(patched, "after the project's " + gone.__name__)
    print("  ok the geometry fog: technique 5 and the original's constants before a marked project, without the"
          " key, on the lobby's scene and after its removal or destruction; on the marked scene technique 3, its"
          " inscatter_parameters (200 and 300 m), then once, at the geometry stage's god rays, technique 4 with"
          " to_sun_direction_parameter, inscatter_parameters, s_eye_ray_corner, target 37h and fill_surfaces;"
          " the god rays' body after it as the original's")


# -- the scattering tables and the sky ----------------------------------------------------------------------

TARGETS_ID = 0x5150
FLAGS = HEAP + 0x3C0000                     # execute's [ebp+8]: its +18h and +19h the remake flags (0)


def check_compile(stock, patched):
    for m in (stock, patched):
        compile_stubs(m)
    for caching in (0, 1):
        native = run_compile(stock, 0x1D21F0, caching)
        pss = [next(e[4] for e in t if e[0] == "begin_pass") for t in native]
        assert pss[:2] == ["make_mie_rayleigh_texture", "atmosphere"] and len(native) == 6, pss
        for _ in range(2):
            ours = run_compile(patched, 0x1D21F0, caching)
            assert len(ours) == 12 and ours[:6] == native, len(ours)
            for twin, original, ps in ((6, 0, "make_mie_rayleigh_texture_v011w"), (7, 1, "atmosphere_v011w")):
                want = [("begin_pass", e[1], e[2], e[3], ps, e[5]) if e[0] == "begin_pass" else e for e in native[original]]
                assert ours[twin] == want, (twin, ours[twin][:2], want[:2])
            assert ours[8:] == native[2:], "2..5 again, unchanged"
        print(f"  ok effect_atmospheric_scattering::compile ({'caching' if caching else 'building'}): techniques 0..5"
              " the original's, 6..11 the six again, 6 with ps make_mie_rayleigh_texture_v011w and 7 with"
              " atmosphere_v011w, the same at a second compile")
    for m in (stock, patched):
        m.stubs = {}


def sky_stage(m, drawn):
    m.w(ATMOSPHERE + 4, CONTEXT)
    m.w(ATMOSPHERE + 0x50, EFFECT)
    m.w(ATMOSPHERE + 0xAC, 0)
    m.w(CONTEXT + 0x416C, drawn)
    m.w(CONTEXT + 0x4170, VIEW)
    m.emu.mem_write(FLAGS, bytes(0x40))


def remake(m, drawn):
    """The tables' decision with the ids equal: 1 when it remakes them."""
    sky_stage(m, drawn)
    m.w(VIEW + 0x1DE38, TARGETS_ID)
    m.run(0x1A4300, {0x1A430F}, regs={UC_X86_REG_EBP: ATMOSPHERE, UC_X86_REG_ESI: VIEW, UC_X86_REG_ECX: TARGETS_ID,
                                      UC_X86_REG_EAX: FLAGS})
    return m.emu.reg_read(UC_X86_REG_ECX) & 0xFF


def tables_technique(m, drawn):
    sky_stage(m, drawn)
    m.run(0x1A441D, {0x1A4422}, regs={UC_X86_REG_EBP: ATMOSPHERE})
    assert m.emu.reg_read(UC_X86_REG_EAX) == EFFECT, "EAX the stage's effect"
    return m.emu.reg_read(UC_X86_REG_ECX)


def sky_technique(m, drawn):
    sky_stage(m, drawn)
    m.w(EFFECT + 0x5620, 0)
    m.run(0x1A459D, {0x1A45AA}, regs={UC_X86_REG_EBP: ATMOSPHERE})
    assert m.emu.reg_read(UC_X86_REG_EAX) == EFFECT
    return m.r(EFFECT + 0x5620)


def check_sky(stock, patched):
    for m, label in ((stock, "the original"), (patched, "before any project")):
        for drawn in (RENDER_SCENE, LOBBY_SCENE):
            assert remake(m, drawn) == 0 and tables_technique(m, drawn) == 0 and sky_technique(m, drawn) == 1, label
    patched.w(GAME_SCENE + 4, RENDER_SCENE)
    set_project(patched, PROJECT, GAME_SCENE, {"srp_renderer_v011w": profile(200)})
    insert(patched, PROJECT)
    assert tables_technique(patched, LOBBY_SCENE) == 0, "the lobby's tables"
    assert sky_technique(patched, LOBBY_SCENE) == 1, "the lobby's sky"
    assert remake(patched, LOBBY_SCENE) == 0, "the lobby's tables kept"
    assert remake(patched, RENDER_SCENE) == 1, "the marked scene remakes the lobby's tables"
    assert tables_technique(patched, RENDER_SCENE) == 6, "0.11e's tables"
    assert remake(patched, RENDER_SCENE) == 0, "0.11e's tables kept"
    assert sky_technique(patched, RENDER_SCENE) == 7, "0.11e's sky"
    assert remake(patched, LOBBY_SCENE) == 1, "the lobby remakes the 0.11 tables"
    assert tables_technique(patched, LOBBY_SCENE) == 0
    remove(patched, PROJECT)
    assert remake(patched, RENDER_SCENE) == 0 and tables_technique(patched, RENDER_SCENE) == 0
    assert sky_technique(patched, RENDER_SCENE) == 1, "removed"
    print("  ok the sky stage: technique 0 for the tables and 1 for the sky, as the original, before a marked project,"
          " on the lobby's scene and after the project's removal; on the marked scene 6 and 7; the tables remade"
          " when a frame's look differs from theirs (both ways), kept otherwise")


# -- the post ----------------------------------------------------------------------------------------------

POST_STAGE = HEAP + 0x3D0000
BLENDS = {0: HEAP + 0x3D1000, 2: HEAP + 0x3D2000, 3: HEAP + 0x3D3000}
FILMIC_HOSTS = {"filmic_0": HEAP + 0x3D4000, "filmic_1": HEAP + 0x3D4100}
POST_COMPILES = {"<0,0>": 0x1B51D0, "<1,0>": 0x1B5440, "<1,1>": 0x1B56B0}


def check_post_compiles(stock, patched):
    for m in (stock, patched):
        compile_stubs(m)
    for name, start in POST_COMPILES.items():
        for caching in (0, 1):
            native = run_compile(stock, start, caching)
            assert len(native) == 1, len(native)
            assert next(e[4] for e in native[0] if e[0] == "begin_pass") == "complex_post_process_blend"
            for _ in range(2):
                ours = run_compile(patched, start, caching)
                assert len(ours) == 2 and ours[0] == native[0], (name, len(ours))
                twin = [("begin_pass", e[1], e[2], e[3], "complex_post_process_blend_v011w_026g", e[5]) if e[0] == "begin_pass"
                        else e for e in native[0]]
                assert ours[1] == twin, (name, "the twin: the same pass with 0.11e's post")
        print(f"  ok effect_complex_post_process_blend{name}::compile (building, caching): its technique, then the"
              " twin with ps complex_post_process_blend_v011w_026g (every texture and state the same), twice")
    for m in (stock, patched):
        m.stubs = {}


def post_stage(m, drawn):
    m.w(POST_STAGE + 4, CONTEXT)
    for index, effect in BLENDS.items():
        m.w(POST_STAGE + 0xA4 + 4 * index, effect)
    m.w(POST_STAGE + 0xA8, 0)
    m.w(POST_STAGE + 0x344, FILMIC_HOSTS["filmic_0"])
    m.w(POST_STAGE + 0x348, FILMIC_HOSTS["filmic_1"])
    m.w(CONTEXT + 0x416C, drawn)
    m.w(BACKEND_SLOT, BACKEND)


def blend_apply(m, drawn, index):
    post_stage(m, drawn)
    m.run(0x1B2A35, {0x1B2A3E}, regs={UC_X86_REG_EBP: POST_STAGE, UC_X86_REG_EAX: index})
    assert m.emu.reg_read(UC_X86_REG_EAX) == BLENDS[index], "EAX the blend of the index"
    return m.emu.reg_read(UC_X86_REG_ECX)


def blend_constants(m, drawn):
    post_stage(m, drawn)
    def constant(esp):
        host = m.emu.reg_read(UC_X86_REG_EAX)
        name = next((n for n, at in FILMIC_HOSTS.items() if at == host), hex(host))
        data = struct.unpack("<4f", bytes(m.emu.mem_read(m.r(esp + 4), 16)))
        return 4, (name, m.emu.reg_read(UC_X86_REG_EDI) == BACKEND, tuple(round(x, 5) for x in data))
    m.stubs = {SET_PS_CONSTANT: constant}
    m.events = []
    m.emu.mem_write(POST_STAGE + 0x314, struct.pack("<I", 0x31431431))
    m.run(0x1B2C13, {0x1B2C19}, regs={UC_X86_REG_EBP: POST_STAGE})
    assert m.emu.reg_read(UC_X86_REG_EAX) == 0x31431431, "the replayed mov eax,[ebp+314h]"
    events, m.events, m.stubs = m.events, None, {}
    return events


def check_post(stock, patched):
    for m, label in ((stock, "the original"), (patched, "before any project")):
        for drawn in (RENDER_SCENE, LOBBY_SCENE):
            assert all(blend_apply(m, drawn, i) == 0 for i in BLENDS), label
            assert blend_constants(m, drawn) == [], label
    patched.w(GAME_SCENE + 4, RENDER_SCENE)
    set_project(patched, PROJECT, GAME_SCENE, {"srp_renderer_v011w": profile(200)})
    insert(patched, PROJECT)
    assert all(blend_apply(patched, LOBBY_SCENE, i) == 0 for i in BLENDS), "the lobby's post"
    assert blend_constants(patched, LOBBY_SCENE) == [], "the lobby's constants"
    assert all(blend_apply(patched, RENDER_SCENE, i) == 1 for i in BLENDS), "0.11e's post on the marked scene"
    rounded = tuple(tuple(round(x, 5) for x in p) for p in PACKED)
    assert blend_constants(patched, RENDER_SCENE) == [("filmic_0", True, rounded[0]), ("filmic_1", True, rounded[1])]
    remove(patched, PROJECT)
    assert all(blend_apply(patched, RENDER_SCENE, i) == 0 for i in BLENDS) and blend_constants(patched, RENDER_SCENE) == []
    set_project(patched, PROJECT, GAME_SCENE, {"srp_renderer_v011w": {"inscatter_distance": 300}})
    insert(patched, PROJECT)
    assert all(blend_apply(patched, RENDER_SCENE, i) == 0 for i in BLENDS), "a profile without the curve: 0.26g's post"
    assert blend_constants(patched, RENDER_SCENE) == []
    remove(patched, PROJECT)
    print("  ok the post: technique 0 of the blend of each index (0, 2, 3) and no constant of ours before a marked"
          " project, on the lobby's scene, after the project's removal and for a profile without the curve; on"
          " the marked scene technique 1 and filmic_tonemap_packed_parameters_0/1 the profile's, packed as 0.11e")


# -- the skylight ------------------------------------------------------------------------------------------

AMBIENT = HEAP + 0x3E0000
SKYLIGHT_EFFECT = HEAP + 0x3E1000
SKY_HOSTS = [HEAP + 0x3E2000 + 0x100 * k for k in range(8)]
SKY_EYE_HOST = HEAP + 0x3E2800
SET_VS_CONSTANT = 0x14AE40
SIDE = [0.509804, 0.529412, 0.564706]
SKY = {"skylight_light": [0.32, 0.39, 0.5], "skylight_lower": 0, "skylight_upper": 5, "skylight_power": 0.1,
       "environment_skycolor_x": SIDE, "environment_skycolor_minus_x": SIDE, "environment_skycolor_z": SIDE,
       "environment_skycolor_minus_z": SIDE, "environment_skycolor_up": [1, 1, 1],
       "environment_skycolor_down": [1, 1, 1], "sun_tint": [0.25, 0.5, 1.0]}


def check_skylight_compile(stock, patched):
    for m in (stock, patched):
        compile_stubs(m)
    for caching in (0, 1):
        native = run_compile(stock, 0x1CF370, caching)
        assert [next(e[4] for e in t if e[0] == "begin_pass") for t in native] == ["skylight", "skylight"]
        for _ in range(2):
            ours = run_compile(patched, 0x1CF370, caching)
            assert len(ours) == 4 and ours[:2] == native, len(ours)
            for twin, original in ((2, 0), (3, 1)):
                want = [("begin_pass", e[1], e[2], e[3], "skylight_v011w", e[5]) if e[0] == "begin_pass" else e
                        for e in native[original]]
                assert ours[twin] == want, twin
    for m in (stock, patched):
        m.stubs = {}
    print("  ok effect_skylight::compile (building, caching): techniques 0..1 the original's, 2..3 their twins with"
          " ps skylight_v011w (every texture and state the same), twice")


def ambient_stage(m, drawn):
    m.w(AMBIENT + 4, CONTEXT)
    m.w(AMBIENT + 0x58, SKYLIGHT_EFFECT)
    m.w(AMBIENT + 0xF0, SKY_EYE_HOST)
    for k, host in enumerate(SKY_HOSTS):
        m.w(AMBIENT + 0x114 + 4 * k, host)
    m.w(CONTEXT + 0x416C, drawn)
    m.w(CONTEXT + 0x4170, VIEW)
    m.emu.mem_write(CONTEXT + 0x53A8, struct.pack("<3f", *EYE_RAY))
    m.emu.mem_write(VIEW + 0x118 + 0x48, struct.pack("<f", 2.0))
    m.w(BACKEND_SLOT, BACKEND)


def skylight_gate(m, drawn):
    """Where execute goes from its skylight gate: "render" the call, "skip" past it."""
    ambient_stage(m, drawn)
    m.emu.mem_write(AMBIENT + 0xE4, b"\x00")             # as the constructor leaves it, for good
    at, _ = m.run(0x18275F, {0x182768, 0x18276F}, regs={UC_X86_REG_ESI: AMBIENT})
    return {0x182768: "render", 0x18276F: "skip"}[at]


def skylight_technique(m, drawn):
    ambient_stage(m, drawn)
    m.w(SKYLIGHT_EFFECT + 0x5620, 0)
    m.run(0x18245C, {0x182469}, regs={UC_X86_REG_ESI: AMBIENT})
    assert m.emu.reg_read(UC_X86_REG_EAX) == SKYLIGHT_EFFECT
    return m.r(SKYLIGHT_EFFECT + 0x5620)


def skylight_constants(m, drawn):
    """The constants set before the skylight's fill, and its default-depth flag (pushed 1 at 0x1824AD)."""
    ambient_stage(m, drawn)
    names = {host: f"skylight_parameters{k}" for k, host in enumerate(SKY_HOSTS)}
    names[SKY_EYE_HOST] = "s_eye_ray_corner"

    def ps(esp):
        data = struct.unpack("<4f", bytes(m.emu.mem_read(m.r(esp + 4), 16)))
        return 4, ("ps", names.get(m.emu.reg_read(UC_X86_REG_EAX)), m.emu.reg_read(UC_X86_REG_EDI) == BACKEND,
                   tuple(round(x, 5) for x in data))

    def vs(esp):
        data = struct.unpack("<4f", bytes(m.emu.mem_read(m.emu.reg_read(UC_X86_REG_EAX), 16)))
        return 4, ("vs", names.get(m.r(esp + 4)), m.emu.reg_read(UC_X86_REG_ESI) == BACKEND,
                   tuple(round(x, 5) for x in data))

    def sun(esp):
        out = m.emu.reg_read(UC_X86_REG_ESI)
        m.emu.mem_write(out, struct.pack("<3f", *SUN_DIRECTION))
        m.emu.reg_write(UC_X86_REG_EAX, out)
        return 0, None

    m.stubs = {SET_PS_CONSTANT: ps, SET_VS_CONSTANT: vs, SUN: sun}
    m.events = []
    _, sp = m.run(0x1824D3, {0x1824DC}, regs={UC_X86_REG_ESI: AMBIENT}, stack=(0, 0, 0, 0, 1))
    assert m.emu.reg_read(UC_X86_REG_ESI) == CONTEXT, "mov esi,[esi+4] replayed"
    events, m.events, m.stubs = m.events, None, {}
    return events, m.r(sp + 0x10)


def expected_skylight():
    r = lambda v: tuple(round(x, 5) for x in v)
    plus_y = [1.0, 1.0, 1.0]
    p = [(0.32, 0.39, 0.5, 0.0), (0.0, 5.0, 0.1, 0.0), (*SIDE, SIDE[0]), (*SIDE, SIDE[1]), (1.0, 1.0, 1.0, SIDE[2]),
         (*SIDE, plus_y[0]), (*SUN_DIRECTION, plus_y[1]), (0.5, 1.0, 2.0, plus_y[2])]
    return ([("ps", f"skylight_parameters{k}", True, r(v)) for k, v in enumerate(p)]
            + [("vs", "skylight_parameters6", True, r(p[6])), ("ps", "s_eye_ray_corner", True, r(EYE_RAY + (0.0,)))])


def check_skylight(stock, patched):
    for m, label in ((stock, "the original"), (patched, "before any project")):
        for drawn in (RENDER_SCENE, LOBBY_SCENE):
            assert skylight_gate(m, drawn) == "skip", label
            assert skylight_technique(m, drawn) == 1 and skylight_constants(m, drawn) == ([], 1), label
    patched.w(GAME_SCENE + 4, RENDER_SCENE)
    set_project(patched, PROJECT, GAME_SCENE, {"srp_renderer_v011w": {"inscatter_distance": 200, **SKY}})
    insert(patched, PROJECT)
    assert skylight_gate(patched, LOBBY_SCENE) == "skip"
    assert skylight_technique(patched, LOBBY_SCENE) == 1 and skylight_constants(patched, LOBBY_SCENE) == ([], 1)
    assert skylight_gate(patched, RENDER_SCENE) == "render", "the marked scene draws the skylight"
    assert skylight_technique(patched, RENDER_SCENE) == 3, "0.11e's skylight"
    assert skylight_constants(patched, RENDER_SCENE) == (expected_skylight(), 0), \
        "0.11e's constants, the fill without the default depth-stencil"
    remove(patched, PROJECT)
    set_project(patched, PROJECT, GAME_SCENE, {"srp_renderer_v011w": {"inscatter_distance": 200}})
    insert(patched, PROJECT)
    assert skylight_gate(patched, RENDER_SCENE) == "skip", "a profile without the skylight: 0.26g's (none)"
    assert skylight_technique(patched, RENDER_SCENE) == 1 and skylight_constants(patched, RENDER_SCENE) == ([], 1), \
        "a profile without the skylight: 0.26g's"
    remove(patched, PROJECT)
    assert skylight_technique(patched, RENDER_SCENE) == 1
    print("  ok the skylight: 0.26g's gate skips it (its +E4h never set) and technique 1 with no constant of ours"
          " before a marked project, on the lobby's scene,"
          " for a profile without the skylight and after the removal; on the marked scene drawn, technique 3 and"
          " skylight_parameters0..7 packed as 0.11e (the sun's direction and its tinted intensity), 6 to the"
          " vertex shader too, s_eye_ray_corner, and its fill without the default depth-stencil")


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    print("patch 162 (v011w_render), original and patched exe")
    stock, patched = Machine(original), Machine(bytes(built))
    check(stock, patched)
    stock, patched = Machine(original), Machine(bytes(built))
    check_compile(stock, patched)
    check_sky(stock, patched)
    stock, patched = Machine(original), Machine(bytes(built))
    check_post_compiles(stock, patched)
    check_post(stock, patched)
    stock, patched = Machine(original), Machine(bytes(built))
    check_skylight_compile(stock, patched)
    check_skylight(stock, patched)


if __name__ == "__main__":
    main()
