#!/usr/bin/env python3
"""Patch 170 (v069d_render): the passes a ported 0.69d map's programs need bind what they read
(../docs/notes/client_patch.md section 3.91).

    python check_v069d_render_026g.py <original survarium.exe>

Runs the game's own code in the original exe and in the patched one (Unicorn): effect_atmospheric_scattering::
compile (0x1D21F0) and effect_gather_luminance::compile (0x1D4C90) whole, caching and building, on a made-up
compiler, the engine's begin_pass, set_texture, end_pass, end_technique, set_stencil, set_alpha_blend and
pass-vector release stubbed and recorded with every write to the compiler (check_rain_v021d_026g.py's harness).
The patched compiles make the original's calls and writes, in order, and three more set_texture calls, each
with the arguments of the pass's own set_texture before it:

- the atmosphere pass (the sky), after t_rayleigh_scattering: t_scattering_v069d
  ported/v069d/engine/atmosphere_scattering;
- the atmospheric_scattering_on_geometry_temporary pass (the geometry fog), after t_rayleigh_scattering:
  t_sun_shadow_and_scattering $user$sun_shadow_and_scattering, then t_scattering_v069d;
- the gather_log_luminance pass, after its begin_pass: t_frame_luminance $user$frame_luminance, with the
  tone pass's arguments (0x1B5380..0x1B5395: 0, -1, 0, 1.0, 0).

They return as the original (ESP, EBX, ESI, EDI, EBP), and a second compile with the same compiler makes the
same again.
"""
import struct
import sys

import check_rain_v021d_026g as rig
import patch_026g as base

ATMOSPHERE, GATHER_LUMINANCE = 0x1D21F0, 0x1D4C90
TABLE = ("t_scattering_v069d", "ported/v069d/engine/atmosphere_scattering")
SHADOW = ("t_sun_shadow_and_scattering", "$user$sun_shadow_and_scattering")
ADAPTED = ("t_frame_luminance", "$user$frame_luminance")
ONE = struct.unpack("<I", struct.pack("<f", 1.0))[0]
TONE_ARGUMENTS = (0, 0xFFFFFFFF, 0, ONE, 0)


def flat(techniques):
    return [event for technique in techniques for event in technique]


def expected(native, extra):
    """The original's events with `extra[(pass, after)]` set_texture calls inserted after the pass's own
    set_texture of `after` (or its begin_pass when `after` is None)."""
    out, pass_name, done = [], None, set()
    for event in native:
        out.append(event)
        if event[0] == "begin_pass":
            pass_name = event[4]
        key = (pass_name, event[2] if event[0] == "set_texture" else None) if event[0] in ("set_texture", "begin_pass") else None
        if key in extra and key not in done:
            done.add(key)
            # a set_texture event's compiler is its [1], a begin_pass event's its [2]
            compiler, arguments = (event[1], event[4]) if event[0] == "set_texture" else (event[2], TONE_ARGUMENTS)
            out.extend(("set_texture", compiler, slot, texture, arguments) for slot, texture in extra[key])
    assert done == set(extra), ("passes or textures not found", set(extra) - done)
    return out


def check(stock, patched, start, extra, what):
    for caching in (0, 1):
        mode = "caching" if caching else "building"
        native = flat(rig.run_compile(stock, start, caching))
        want = expected(native, extra)
        for _ in range(2):
            ours = flat(rig.run_compile(patched, start, caching))
            if ours != want:
                at = next((i for i, (a, b) in enumerate(zip(ours, want)) if a != b), min(len(ours), len(want)))
                sys.exit(f"{what} ({mode}): differs at event {at} of {len(want)}: ours {ours[max(at - 2, 0):at + 3]}"
                         f" want {want[max(at - 2, 0):at + 3]}")
        passes = [e[4] for e in native if e[0] == "begin_pass"]
        added = sum(len(v) for v in extra.values())
        print(f"  ok {what} ({mode}): the original's {len(native)} calls and writes over passes {', '.join(passes)},"
              f" and the {added} binding(s), the same at a second compile")


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    stock, patched = rig.Machine(original), rig.Machine(bytes(built))
    for m in (stock, patched):
        rig.compile_stubs(m)
    print("patch 170 (v069d_render), original and patched exe")
    check(stock, patched, ATMOSPHERE, {
        ("atmosphere", "t_rayleigh_scattering"): [TABLE],
        ("atmospheric_scattering_on_geometry_temporary", "t_rayleigh_scattering"): [SHADOW, TABLE],
    }, "effect_atmospheric_scattering::compile")
    check(stock, patched, GATHER_LUMINANCE, {("gather_log_luminance", None): [ADAPTED]},
          "effect_gather_luminance::compile")


if __name__ == "__main__":
    main()
