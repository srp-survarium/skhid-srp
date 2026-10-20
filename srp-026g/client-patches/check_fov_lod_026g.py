#!/usr/bin/env python3
"""Execute patch 40 (a model's LOD and screen factor at a wider field of
view are the original's) and compare it with the original.

    python check_fov_lod_026g.py <original survarium.exe>

Requires Python `unicorn`. Builds the default exe of patch_026g.py and runs,
in the original and in the build:
- the camera's projection (0x242900, fov_projection in the build) at
  several settings and zooms: the k it leaves in the state page is
  max(1, tan(A/2) / tan(65 z/2)), exactly 1.0 at 65 and less and through a
  full scope;
- static_render_model_instance::select_lod (0x16EEC0) on a made-up model
  of screen-size LOD (the Kamaz trucks' params, 0.1 / 0.05 / 0.025) seen
  through each projection: the distance at which it goes to LOD 3 (drawn
  no more) is the original's at 65 at every setting, and without k (the
  page zeroed) it is the original's own short one;
- render_surface_instance::calculate_screen_factor (0x158D80) on a made-up
  surface: the same factor at every setting;
- the two sites are a call and nops of the original's length, their
  relocations are disabled and nothing else of the relocation table moved.
"""
import math
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDI, UC_X86_REG_EIP,
                               UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

STACK, WORK, RETURN = 0x20000000, 0x20100000, 0x20200000  # above the image
CAMERA, VIEW, MATRIX = WORK, WORK + 0x100, WORK + 0x200
EYE, MODEL, ORIGINAL, LODS, SURFACE, MATERIAL = (WORK + 0x300, WORK + 0x400, WORK + 0x800,
                                                 WORK + 0xA00, WORK + 0xB00, WORK + 0xC00)
SELECT_LOD, SCREEN_FACTOR = 0x16EEC0, 0x158D80
TRUCK = ((-1.25, 0.0, -3.75), (1.25, 3.5, 3.75))     # a Kamaz-sized box, metres
TRUCK_PARAMS = (0.1, 0.05, 0.025)                    # its lod_switching param0..2


def image(data):
    """Map the PE sections at the preferred base (the exe is never rebased);
    `.srpd` has no bytes in the file and is zero."""
    emu = Uc(UC_ARCH_X86, UC_MODE_32)
    for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
        start = base.IMAGE_BASE + rva
        emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
        emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
    for region in (STACK, WORK, RETURN):
        emu.mem_map(region, 0x10000, UC_PROT_ALL)
    return emu


def call(emu, va, args, registers):
    """Run `va` with stdcall stack arguments to its return; esp checked."""
    esp = STACK + 0x8000
    emu.mem_write(esp, struct.pack(f"<{1 + len(args)}I", RETURN, *args))
    emu.reg_write(UC_X86_REG_ESP, esp)
    for name, value in registers.items():
        emu.reg_write(name, value)
    emu.emu_start(va, RETURN, count=200000)
    if emu.reg_read(UC_X86_REG_EIP) != RETURN or emu.reg_read(UC_X86_REG_ESP) != esp + 4 + 4 * len(args):
        raise AssertionError(f"{va:#x} did not return as it should")
    return emu.reg_read(UC_X86_REG_EAX)


def projection(emu, setting, zoom, width=1920.0, height=1080.0, near=0.1, far=1000.0):
    """The camera's matrix at MATRIX (row vectors: the engine's)."""
    emu.mem_write(base.FOV_VALUE, struct.pack("<f", setting))
    emu.mem_write(CAMERA + 0x88, struct.pack("<fff", near, far, zoom))
    emu.mem_write(VIEW, struct.pack("<ff", width, height))
    call(emu, base.FOV_PROJECTION_VA, [MATRIX, VIEW], {UC_X86_REG_ECX: CAMERA})
    return struct.unpack("<16f", emu.mem_read(MATRIX, 64))


def k_of(emu, k_va):
    return struct.unpack("<f", emu.mem_read(k_va, 4))[0]


def select_lod(emu, distance):
    """select_lod of a static model with the truck's box `distance` m ahead
    of an eye at the origin, through MATRIX (the view is the identity)."""
    identity = struct.pack("<16f", *[float(i % 5 == 0) for i in range(16)])
    emu.mem_write(MODEL + 0x290, struct.pack("<I", ORIGINAL))
    emu.mem_write(MODEL + 0x14C, identity[:48] + struct.pack("<4f", 0.0, 0.0, distance, 1.0))
    emu.mem_write(ORIGINAL + 0x128, struct.pack("<I", LODS))
    emu.mem_write(ORIGINAL + 0x108, struct.pack("<6f", *TRUCK[0], *TRUCK[1]))
    # Three LODs with surfaces, type 1 (screen size), its own params.
    emu.mem_write(LODS, bytes([1, 1, 1, 0]) + bytes(12) + bytes([1, 0, 0, 0])
                  + struct.pack("<3f", *TRUCK_PARAMS) + bytes([0]) + bytes(3))
    emu.mem_write(EYE, struct.pack("<3f", 0.0, 0.0, 0.0))
    return call(emu, SELECT_LOD, [MODEL, MATRIX], {UC_X86_REG_EAX: EYE}) & 0xFF


def gone_at(emu):
    """The least distance (0.25 m steps) at which the truck draws nothing."""
    near, far = 1.0, 2000.0
    if select_lod(emu, far) != 3:
        raise AssertionError("the truck never goes to LOD 3")
    while far - near > 0.25:
        middle = (near + far) / 2
        near, far = (near, middle) if select_lod(emu, middle) == 3 else (middle, far)
    return far


def screen_factor(emu, distance):
    """calculate_screen_factor of a 2 m box `distance` m ahead, through
    the projection at MATRIX; the factor it writes at +58h."""
    emu.mem_write(SURFACE + 0x20, struct.pack("<6f", -1.0, -1.0, distance - 1, 1.0, 1.0, distance + 1))
    emu.mem_write(SURFACE + 0x38, struct.pack("<I", MATERIAL))
    emu.mem_write(MATERIAL + 0x48, bytes(4))
    emu.mem_write(SURFACE + 0x58, bytes(8))
    emu.mem_write(EYE, struct.pack("<3f", 0.0, 0.0, 0.0))
    call(emu, SCREEN_FACTOR, [MATRIX], {UC_X86_REG_ESI: SURFACE, UC_X86_REG_EDI: EYE})
    return struct.unpack("<f", emu.mem_read(SURFACE + 0x58, 4))[0]


def relocations(data):
    headers = base.Headers(data)
    rva, size = struct.unpack_from("<II", data, headers.reloc_at)
    offset = next(rva - start + raw for _, start, vsize, raw, rsize, _ in headers.sections(data)
                  if start <= rva < start + max(vsize, rsize))
    found, end = {}, offset + size
    while offset < end:
        page, block = struct.unpack_from("<II", data, offset)
        if block < 8:
            break
        for at in range(offset + 8, offset + block, 2):
            entry = struct.unpack_from("<H", data, at)[0]
            if entry:
                found.setdefault(base.IMAGE_BASE + page + (entry & 0xFFF), entry >> 12)
        offset += block
    return found


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    patched, _ = base.build(original, "section", code, None, late_join)
    without, _ = base.build(original, "section", code)
    k_va = base.late_join_state_va(late_join) + base.FOV_LOD_STATE_OFFSET
    checks = 0

    # The sites: a call of the original's length into the late join, and
    # the two relocations under them alone disabled.
    for va, old, offset, _ in base.FOV_LOD_HOOKS:
        at = base.file_offset(va)
        site = patched[at:at + len(old)]
        assert original[at:at + len(old)] == without[at:at + len(old)] == old, hex(va)
        assert site[0] == 0xE8 and set(site[5:]) == {0x90} and len(site) == len(old), hex(va)
        assert va + 5 + struct.unpack_from("<i", site, 1)[0] == base.LATE_JOIN_VA + offset, hex(va)
        checks += 1
    before, after = relocations(without), relocations(patched)
    for va in base.FOV_LOD_RELOCS:
        assert before.get(va) == 3 and after.get(va) == 0, hex(va)
    checks += 1

    # k after each projection.
    stock, new = image(original), image(patched)
    assert k_of(new, k_va) == 0.0
    sights = (1.0, 1 / 1.1, 1 / 1.2, 1 + (0.25 - 1) * 0.5, 0.25)
    shown = {}
    for setting in (50.0, 60.0, 65.0, 80.0, 90.0, 100.0):
        for zoom in sights:
            zoom = struct.unpack("<f", struct.pack("<f", zoom))[0]
            projection(new, setting, zoom)
            k = k_of(new, k_va)
            degrees = 65 * zoom + (setting - 65) * max(0.0, (zoom - 0.25) / 0.75)
            expected = max(1.0, math.tan(math.radians(degrees) / 2) / math.tan(math.radians(65 * zoom) / 2))
            if setting <= 65 or zoom == 0.25:
                assert k == 1.0, (setting, zoom, k)
            assert abs(k - expected) <= 2e-6 * expected, (setting, zoom, k, expected)
            if zoom == 1.0:
                shown[setting] = k
            checks += 1

    # select_lod: where the truck goes out. The original sees 65 degrees.
    projection(stock, 65.0, 1.0)
    original_gone = gone_at(stock)
    rows = []
    for setting in (65.0, 80.0, 100.0):
        projection(new, setting, 1.0)
        compensated = gone_at(new)
        k = k_of(new, k_va)
        new.mem_write(k_va, bytes(4))           # as before the first projection: the original's x0.5
        uncompensated = gone_at(new)
        new.mem_write(k_va, struct.pack("<f", k))
        if setting == 65.0:
            assert compensated == uncompensated == original_gone, (compensated, uncompensated, original_gone)
        assert abs(compensated - original_gone) <= 0.01 * original_gone, (setting, compensated, original_gone)
        ratio = math.tan(math.radians(32.5)) / math.tan(math.radians(setting / 2))
        # The box is 7.5 m deep: its near corners are nearer than its middle,
        # so the out-distance falls a little less than the ratio of tangents.
        assert original_gone * ratio <= uncompensated <= 1.04 * original_gone * ratio, (setting, uncompensated)
        rows.append(f"{setting:g}: {compensated:.1f} m (without k {uncompensated:.1f} m)")
        checks += 1

    # The screen factor of a 2 m box at 20, 60 and 150 m.
    for distance in (20.0, 60.0, 150.0):
        projection(stock, 65.0, 1.0)
        reference = screen_factor(stock, distance)
        for setting in (65.0, 80.0, 100.0):
            projection(new, setting, 1.0)
            factor = screen_factor(new, distance)
            if setting == 65.0:
                assert factor == reference, (distance, factor, reference)
            assert abs(factor - reference) <= 1e-4 * reference, (setting, distance, factor, reference)
            checks += 1

    print(f"patch 40: {checks} checks; k unaimed " + ", ".join(f"{s:g}: {k:.4f}" for s, k in shown.items())
          + f"; the truck goes out at {original_gone:.1f} m in the original at 65, in the build at "
          + "; ".join(rows))


if __name__ == "__main__":
    main()
