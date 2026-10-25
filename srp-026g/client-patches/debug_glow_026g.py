#!/usr/bin/env python3
"""A DEBUG exe for the artefacts' first-person glow: never a release.

    python debug_glow_026g.py <survarium_srp.exe> <out.exe>

<survarium_srp.exe> is a build of patch_026g.py (e.g. --revision=32
--server=127.0.0.1:2234). The output is it with its last section (`.srpx`,
the arena) grown by the debug code (debug_glow_026g.asm, assembled here with
NASM 2.16; the section table has no free entry for one more), and ten jumps
into it, each over the instructions the section does again before it goes on.
Every hooked site's bytes are checked first. The section logs, through the
game's own logger into Survarium_<name>.log, lines starting "srp-glow:" (see
the asm). patch_026g.py, the arena and its lock know nothing of this: a
release never has it.
"""
import os
import struct
import subprocess
import sys
import tempfile

import patch_026g as base

# site, the original bytes there, the entry's offset in .dbg (5 bytes apart)
HOOKS = (
    (0x313435, "8db858030000", 0),            # EFFECT: before game_effect_player::add
    (0x313640, "8b490883b98002000004", 5),    # artefact_base::effect_calculator
    (0x2AC400, "558bec83e4f883ec3c", 10),     # particle_game_effect_presenter's handler
    (0x2AC290, "568d9090f20000", 15),         # its play
    (0x022860, "83ec4c5355", 20),             # scene_renderer::stop_particle_system
    (0x29A320, "53558b6c240c", 25),           # particle_game_effect_emitter: an effect given back
    (0x0229D0, "558bec83e4f883ec44", 30),     # scene_renderer::remove_particle_system_instance
    (0x35A8B0, "8b44240433c9", 35),          # particle_world::stop (render side)
    (0x35B110, "518b819c010000", 40),        # particle_world::remove_particle_system_instance
    (0x35D120, "558bec83e4f8", 45),          # particle_system_instance_impl::reset
)


def nasm():
    for d in os.environ.get("PATH", "").split(os.pathsep):
        p = os.path.join(d, "nasm")
        if os.path.exists(p):
            return p
    sys.exit("nasm is needed (NASM 2.16)")


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    data = bytearray(open(sys.argv[1], "rb").read())
    data, va, grow = extend_last(data)
    here = os.path.dirname(os.path.abspath(__file__))
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "dbg.bin")
        subprocess.run([nasm(), "-f", "bin", f"-DBASE={va:#x}", "-o", out,
                        os.path.join(here, "debug_glow_026g.asm")], check=True)
        code = open(out, "rb").read()
    for site, expected, entry in HOOKS:
        at = file_offset(data, site)
        old = bytes.fromhex(expected)
        if data[at:at + len(old)] != old:
            sys.exit(f"{site:#x}: {data[at:at + len(old)].hex()}, not {expected}: another patch is there")
        jump = b"\xE9" + struct.pack("<i", va + entry - (site + 5))
        data[at:at + len(old)] = jump + b"\x90" * (len(old) - 5)
    data = grow(code)
    headers = base.Headers(data)
    struct.pack_into("<I", data, headers.checksum_at, base.pe_checksum(data, headers.checksum_at))
    open(sys.argv[2], "wb").write(data)
    print(f"{sys.argv[2]}: .dbg at {va:#x}, {len(code)} bytes, {len(HOOKS)} hooks")


def extend_last(data):
    """(data, the VA where the code goes, grow(code) -> the new file): the last
    section (RWX) grown by the code, 16-byte aligned after its contents, the
    certificate table moved behind."""
    headers = base.Headers(data)
    sections = headers.sections(data)
    name, rva, vsize, raw, rsize, chars = sections[-1]
    if chars & 0xE0000020 != 0xE0000020:
        sys.exit(f"{name}: not an executable, writable last section")
    align = lambda value, to: (value + to - 1) // to * to
    at = align(vsize, 16)
    security_offset, security_size = struct.unpack_from("<II", data, headers.security_at)
    if security_offset != raw + rsize or security_offset + security_size != len(data):
        sys.exit("unexpected layout after the last section's bytes")

    def grow(code):
        new_vsize = at + len(code)
        new_rsize = align(new_vsize, headers.file_alignment)
        body = bytes(data[raw:raw + min(rsize, at)]).ljust(at, b"\0") + code
        body = body.ljust(new_rsize, b"\xCC")
        out = bytearray(data[:raw]) + body + data[raw + rsize:]
        entry = headers.table + 40 * (len(sections) - 1)
        struct.pack_into("<II", out, entry + 8, new_vsize, rva)
        struct.pack_into("<II", out, entry + 16, new_rsize, raw)
        struct.pack_into("<I", out, headers.size_of_image_at, align(rva + new_vsize, headers.section_alignment))
        struct.pack_into("<I", out, headers.security_at, security_offset + new_rsize - rsize)
        return out

    return data, base.IMAGE_BASE + rva + at, grow


def file_offset(data, va):
    for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
        start = base.IMAGE_BASE + rva
        if start <= va < start + max(vsize, rsize):
            return va - start + raw
    sys.exit(f"{va:#x}: in no section")


if __name__ == "__main__":
    main()
