#!/usr/bin/env python3
"""Patch 162 (v011w_render): the ported 0.11 weathers fog their geometry the 0.11 way
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
"""
import struct
import sys

from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_ECX, UC_X86_REG_EDI, UC_X86_REG_ESI

import patch_026g as base
import check_rain_v021d_026g as rig
from check_rain_v021d_026g import (Machine, set_project, insert, remove, destroy, HEAP, RET, PROJECT, GAME_SCENE,
                                   RENDER_SCENE, LOBBY_SCENE, CONTEXT, VIEW, ATMOSPHERE)

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
        set_project(patched, PROJECT, GAME_SCENE, {"srp_renderer_v011w": distance})
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


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    print("patch 162 (v011w_render), original and patched exe")
    check(Machine(original), Machine(bytes(built)))


if __name__ == "__main__":
    main()
