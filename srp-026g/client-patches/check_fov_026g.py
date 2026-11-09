#!/usr/bin/env python3
"""Execute the original and patch 22's camera projections and compare them.

    python check_fov_026g.py <original survarium.exe>

Requires Python `unicorn`. The real matrix builder (0xEEE30, including its
tangent call) runs on both images. For each setting and zoom z, the default
build of patch_026g.py (patch revision 11) must equal the original with its
65-degree constant replaced so that z times it is
65 z + (uro_fov - 65) * max(0, (z - 0.25) / 0.75) degrees, for several
viewports: uro_fov unaimed, the original's own view through a 4x scope
(z = 0.25) whatever the setting. Also checks the option name (the original's), the uro_fov
lower bound and the relocations.
"""
import math
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_ECX, UC_X86_REG_EIP, UC_X86_REG_ESP

import patch_026g as base

CONSTANT = 0xA2D9EC
CONSTANT_RELOC = 0x24293A
STACK, THIS, RETURN = 0x20000000, 0x20100000, 0x20200000  # above the image
VIEW, OUT = THIS + 0x100, THIS + 0x200


def image(data):
    """Map the PE sections at the preferred base (the exe is never rebased)."""
    emu = Uc(UC_ARCH_X86, UC_MODE_32)
    headers = base.Headers(data)
    for _, rva, vsize, raw, rsize, _ in headers.sections(data):
        start = base.IMAGE_BASE + rva
        size = (max(vsize, rsize) + 0xFFF) & ~0xFFF
        emu.mem_map(start, size, UC_PROT_ALL)
        emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
    emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
    emu.mem_map(THIS, 0x1000, UC_PROT_ALL)
    emu.mem_map(RETURN, 0x1000, UC_PROT_ALL)
    return emu


def projection(emu, zoom, width, height, near=0.1, far=1000.0):
    emu.mem_write(THIS + 0x88, struct.pack("<fff", near, far, zoom))
    emu.mem_write(VIEW, struct.pack("<ff", width, height))
    emu.mem_write(OUT, bytes(64))
    esp = STACK + 0x8000
    emu.mem_write(esp, struct.pack("<III", RETURN, OUT, VIEW))
    emu.reg_write(UC_X86_REG_ESP, esp)
    emu.reg_write(UC_X86_REG_ECX, THIS)
    emu.emu_start(base.FOV_PROJECTION_VA, RETURN, count=100000)
    if emu.reg_read(UC_X86_REG_EIP) != RETURN or emu.reg_read(UC_X86_REG_ESP) != esp + 12:
        raise AssertionError("projection did not return with ret 8")
    return struct.unpack("<16f", emu.mem_read(OUT, 64))


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
    late_join = True
    patched, _ = base.build(original, "section", code, None, late_join)
    stock, new, untouched = image(original), image(patched), image(original)
    checks = 0
    radians = lambda degrees: degrees * math.pi / 180
    # Unaimed, iron sights of 1.1 and 1.2, a scope's slide, a full scope.
    zooms = (1.0, 1 / 1.1, 1 / 1.2, 1 + (0.25 - 1) * 0.5, 1 + (0.25 - 1) * 0.9, 0.25)
    for setting in (50.0, 65.0, 74.5, 90.0, 100.0):
        new.mem_write(base.FOV_VALUE, struct.pack("<f", setting))
        for zoom in zooms:
            zoom = struct.unpack("<f", struct.pack("<f", zoom))[0]
            degrees = 65 * zoom + (setting - 65) * max(0.0, (zoom - 0.25) / 0.75)
            stock.mem_write(CONSTANT, struct.pack("<f", radians(degrees) / zoom))
            for width, height in ((1920.0, 1080.0), (1280.0, 1024.0), (800.0, 600.0)):
                expected = projection(stock, zoom, width, height)
                actual = projection(new, zoom, width, height)
                # float constants and x87 against the original's SSE: a few ulp.
                if any(abs(a - b) > 2e-6 * max(1.0, abs(b)) for a, b in zip(actual, expected)):
                    raise AssertionError(f"{setting} deg, zoom {zoom}, {width}x{height}: {actual} != {expected}")
                checks += 1
        # Through a full scope every setting sees the original's view.
        for a, b in zip(projection(new, 0.25, 1920.0, 1080.0), projection(untouched, 0.25, 1920.0, 1080.0)):
            assert abs(a - b) <= 2e-6 * max(1.0, abs(b)), (setting, a, b)
        checks += 1
    # Untouched defaults: uro_fov 65.0 reproduces the stock camera at every zoom.
    default = image(patched)
    assert struct.unpack("<f", default.mem_read(base.FOV_VALUE, 4))[0] == 65.0
    for zoom in zooms:
        for a, b in zip(projection(default, zoom, 1920.0, 1080.0), projection(untouched, zoom, 1920.0, 1080.0)):
            assert abs(a - b) <= 2e-6 * max(1.0, abs(b)), (zoom, a, b)

    # option 7 keeps the original's dead "fov" (the Gameplay slider, patch 83, sets uro_fov)
    assert bytes(default.mem_read(0xA281D0, 8)) == b"uro_fov\0"
    option_name = (0x262930, b"\xb8" + struct.pack("<I", 0xA0659C))
    assert bytes(default.mem_read(option_name[0], 5)) == bytes(stock.mem_read(option_name[0], 5)) == option_name[1]
    least = struct.unpack("<I", base.FOV_MINIMUM[1][7:11])[0]
    assert bytes(default.mem_read(base.FOV_MINIMUM[0] + 2, 8)) == struct.pack("<If", least, base.FOV_MIN)
    # The option's name keeps its relocation, the least's is disabled; the
    # projection's original body, never reached, keeps the constant's.
    before, after = relocations(original), relocations(patched)
    option = option_name[0] + 1
    assert before.get(option) == after.get(option) == 3
    assert before.get(base.FOV_MINIMUM_RELOC) == 3 and after.get(base.FOV_MINIMUM_RELOC) == 0
    assert before.get(CONSTANT_RELOC) == after.get(CONSTANT_RELOC) == 3
    print(f"patch 22: {checks} projections match (a full scope as the original at every setting); "
          "option name, minimum and relocations checked")


if __name__ == "__main__":
    main()
