#!/usr/bin/env python3
"""The tick's sites in the 0.26g exe (patch 79's phase 4, netcode_port_069.md
section 6.1): every byte the game's 50 ms tick is in, and what it is on a
grid of 20, 25 or 50 ms. rust/netcode stamps them when a match's static
info gives its tick (restamp.rs; the exe is built at 50, so a 50 ms match
changes nothing).

    python tick_sites_026g.py --write            # rust/netcode/src/sites.rs
    python tick_sites_026g.py --check [<exe>]    # the file current; with the
                                                 # original exe, every site

Each row: the byte's VA, its kind, the instruction's VA and text (the
original's). The kinds, at a tick T (ms):

    Magic    a division by the tick: `mov r,imm32` 51EB851Fh (50, 25) or
             0CCCCCCCDh (20), with its Shift (`shr r,4`: 4 for 50 and 20,
             3 for 25); exact for every u32
    Tick8    an imm8 of 50 (`imul r,r,32h`, `add r,32h`): T
    Back8    a disp8 of -50 (`lea r,[..-32h]`): -T
    Tick32   an imm32 of 50 (the grid origin, `mov r,32h` before a `div`): T
    Hash8    the hash interval (patch 3's 50 at 0x27CC53): the largest
             multiple of T up to 50 (50, 50, 40)
    Fraction the interpolated view's 1/T (`fmul [0A2DB34h]`, 0.02): the
             disp32 to a float 1/T (the stock one at 50)
    Physics  the physics' fixed step (`fld [0A2DBB4h]`, 1/60: three
             substeps of a 50 ms tick): the disp32 to a float T/3000, three
             substeps of T (the stock one at 50)

--check with the exe proves every row against the original (its bytes, its
instruction), and that the table has every division by 50 of the exe's
code (each `mov r,51EB851Fh` with its `shr r,4`: 46) and every other site
of 6.1 (48 imm8, 7 imm32, the disp32 and the two physics loads).
"""
import argparse
from pathlib import Path
import struct
import sys

HERE = Path(__file__).resolve().parent
RUST = HERE / "rust" / "netcode" / "src" / "sites.rs"

# (the byte's VA, kind, the instruction's VA, its text in the original)
SITES = [
    (0x1edefd, "Magic", 0x1edefc, "mov eax, 0x51eb851f"),
    (0x1edf06, "Shift", 0x1edf04, "shr edx, 4"),
    (0x1edf0a, "Tick8", 0x1edf08, "imul edx, edx, 0x32"),
    (0x1ee199, "Magic", 0x1ee198, "mov eax, 0x51eb851f"),
    (0x1ee1a1, "Shift", 0x1ee19f, "shr edx, 4"),
    (0x1ee1a4, "Tick8", 0x1ee1a2, "imul edx, edx, 0x32"),
    (0x1ee269, "Magic", 0x1ee268, "mov eax, 0x51eb851f"),
    (0x1ee271, "Shift", 0x1ee26f, "shr edx, 4"),
    (0x1ee27a, "Tick8", 0x1ee278, "imul edx, edx, 0x32"),
    (0x1f7a58, "Magic", 0x1f7a57, "mov eax, 0x51eb851f"),
    (0x1f7a60, "Shift", 0x1f7a5e, "shr edx, 4"),
    (0x1f7a65, "Tick8", 0x1f7a63, "imul ecx, ecx, 0x32"),
    (0x1f7abc, "Tick8", 0x1f7aba, "add eax, 0x32"),
    (0x1f7ad8, "Tick8", 0x1f7ad6, "add ecx, 0x32"),
    (0x1fad55, "Tick8", 0x1fad53, "imul eax, eax, 0x32"),
    (0x1fad6b, "Tick8", 0x1fad69, "add ecx, 0x32"),
    (0x1fad80, "Magic", 0x1fad7f, "mov eax, 0x51eb851f"),
    (0x1fad88, "Shift", 0x1fad86, "shr edx, 4"),
    (0x1fad8b, "Tick8", 0x1fad89, "imul edx, edx, 0x32"),
    (0x1fafdc, "Magic", 0x1fafdb, "mov eax, 0x51eb851f"),
    (0x1fafe4, "Shift", 0x1fafe2, "shr edx, 4"),
    (0x1fafe8, "Magic", 0x1fafe7, "mov eax, 0x51eb851f"),
    (0x1faff0, "Shift", 0x1fafee, "shr edx, 4"),
    (0x1faff5, "Tick8", 0x1faff3, "imul ecx, ecx, 0x32"),
    (0x1fb006, "Back8", 0x1fb003, "lea ecx, [edi + ecx - 0x32]"),
    (0x1fb2d6, "Tick32", 0x1fb2d5, "mov ecx, 0x32"),
    (0x1fb3b4, "Tick32", 0x1fb3b3, "mov esi, 0x32"),
    (0x1fb934, "Tick8", 0x1fb932, "imul eax, eax, 0x32"),
    (0x1fb948, "Tick8", 0x1fb946, "add ecx, 0x32"),
    (0x1fb95d, "Magic", 0x1fb95c, "mov eax, 0x51eb851f"),
    (0x1fb965, "Shift", 0x1fb963, "shr edx, 4"),
    (0x1fb968, "Tick8", 0x1fb966, "imul edx, edx, 0x32"),
    (0x1fbbb9, "Magic", 0x1fbbb8, "mov eax, 0x51eb851f"),
    (0x1fbbc1, "Shift", 0x1fbbbf, "shr edx, 4"),
    (0x1fbbc5, "Magic", 0x1fbbc4, "mov eax, 0x51eb851f"),
    (0x1fbbcd, "Shift", 0x1fbbcb, "shr edx, 4"),
    (0x1fbbd2, "Tick8", 0x1fbbd0, "imul ecx, ecx, 0x32"),
    (0x1fbbe3, "Back8", 0x1fbbe0, "lea ecx, [esi + ecx - 0x32]"),
    (0x1fbe3e, "Tick32", 0x1fbe3d, "mov esi, 0x32"),
    (0x1fdcca, "Magic", 0x1fdcc9, "mov eax, 0x51eb851f"),
    (0x1fdcd4, "Shift", 0x1fdcd2, "shr eax, 4"),
    (0x1fdcd7, "Tick8", 0x1fdcd5, "imul eax, eax, 0x32"),
    (0x1fdcde, "Magic", 0x1fdcdd, "mov eax, 0x51eb851f"),
    (0x1fdce8, "Shift", 0x1fdce6, "shr eax, 4"),
    (0x1fdceb, "Tick8", 0x1fdce9, "imul eax, eax, 0x32"),
    (0x1fdcfa, "Magic", 0x1fdcf9, "mov eax, 0x51eb851f"),
    (0x1fdd04, "Shift", 0x1fdd02, "shr eax, 4"),
    (0x1fdd08, "Tick8", 0x1fdd06, "imul eax, eax, 0x32"),
    (0x1fdd0f, "Magic", 0x1fdd0e, "mov eax, 0x51eb851f"),
    (0x1fdd19, "Shift", 0x1fdd17, "shr eax, 4"),
    (0x1fdd1d, "Tick8", 0x1fdd1b, "imul eax, eax, 0x32"),
    (0x218c4e, "Magic", 0x218c4d, "mov eax, 0x51eb851f"),
    (0x218c56, "Shift", 0x218c54, "shr edx, 4"),
    (0x218c5a, "Tick8", 0x218c58, "imul edx, edx, 0x32"),
    (0x27c39b, "Magic", 0x27c39a, "mov eax, 0x51eb851f"),
    (0x27c3a9, "Shift", 0x27c3a7, "shr edx, 4"),
    (0x27c3cc, "Tick8", 0x27c3ca, "imul eax, eax, 0x32"),
    (0x27c621, "Magic", 0x27c620, "mov eax, 0x51eb851f"),
    (0x27c644, "Shift", 0x27c642, "shr ebx, 4"),
    (0x27c65c, "Tick8", 0x27c65a, "imul eax, eax, 0x32"),
    (0x27c6c6, "Magic", 0x27c6c5, "mov eax, 0x51eb851f"),
    (0x27c6e9, "Shift", 0x27c6e7, "shr ebx, 4"),
    (0x27c6fd, "Tick8", 0x27c6fb, "imul eax, eax, 0x32"),
    (0x27c819, "Magic", 0x27c818, "mov eax, 0x51eb851f"),
    (0x27c837, "Shift", 0x27c835, "shr ebx, 4"),
    (0x27c853, "Magic", 0x27c852, "mov eax, 0x51eb851f"),
    (0x27c866, "Tick8", 0x27c864, "imul esi, esi, 0x32"),
    (0x27c876, "Shift", 0x27c874, "shr ebx, 4"),
    (0x27c890, "Tick8", 0x27c88e, "imul eax, eax, 0x32"),
    (0x27c911, "Magic", 0x27c910, "mov eax, 0x51eb851f"),
    (0x27c931, "Shift", 0x27c92f, "shr ebx, 4"),
    (0x27c947, "Tick8", 0x27c945, "imul eax, eax, 0x32"),
    (0x27ca99, "Magic", 0x27ca98, "mov eax, 0x51eb851f"),
    (0x27cab7, "Shift", 0x27cab5, "shr ebx, 4"),
    (0x27cad3, "Magic", 0x27cad2, "mov eax, 0x51eb851f"),
    (0x27cae6, "Tick8", 0x27cae4, "imul esi, esi, 0x32"),
    (0x27caf6, "Shift", 0x27caf4, "shr ebx, 4"),
    (0x27cb10, "Tick8", 0x27cb0e, "imul eax, eax, 0x32"),
    (0x27cb84, "Magic", 0x27cb83, "mov eax, 0x51eb851f"),
    (0x27cbb1, "Shift", 0x27cbaf, "shr ebx, 4"),
    (0x27cbc8, "Tick8", 0x27cbc6, "imul eax, eax, 0x32"),
    (0x27cc22, "Magic", 0x27cc21, "mov eax, 0x51eb851f"),
    (0x27cc2b, "Shift", 0x27cc29, "shr edx, 4"),
    (0x27cc30, "Tick8", 0x27cc2e, "imul edi, edi, 0x32"),
    (0x27d076, "Magic", 0x27d075, "mov eax, 0x51eb851f"),
    (0x27d080, "Shift", 0x27d07e, "shr eax, 4"),
    (0x27d083, "Tick8", 0x27d081, "imul eax, eax, 0x32"),
    (0x27eaaf, "Tick32", 0x27eaae, "mov edi, 0x32"),
    (0x27ec4f, "Tick32", 0x27ec4e, "mov edi, 0x32"),
    (0x2cc008, "Magic", 0x2cc007, "mov eax, 0x51eb851f"),
    (0x2cc010, "Shift", 0x2cc00e, "shr edx, 4"),
    (0x2d00d3, "Tick32", 0x2d00cd, "mov dword ptr [ebx + 0x18fbc], 0x32"),
    (0x2d0766, "Tick32", 0x2d0765, "mov eax, 0x32"),
    (0x2d1304, "Magic", 0x2d1303, "mov eax, 0x51eb851f"),
    (0x2d130c, "Shift", 0x2d130a, "shr edx, 4"),
    (0x2d1311, "Tick8", 0x2d130f, "imul eax, eax, 0x32"),
    (0x2d15f7, "Magic", 0x2d15f6, "mov eax, 0x51eb851f"),
    (0x2d1601, "Shift", 0x2d15ff, "shr edi, 4"),
    (0x2d1605, "Tick8", 0x2d1603, "imul edi, edi, 0x32"),
    (0x2d1a27, "Magic", 0x2d1a26, "mov eax, 0x51eb851f"),
    (0x2d1a31, "Shift", 0x2d1a2f, "shr edi, 4"),
    (0x2d1a35, "Tick8", 0x2d1a33, "imul edi, edi, 0x32"),
    (0x2d1e22, "Magic", 0x2d1e21, "mov eax, 0x51eb851f"),
    (0x2d1e2a, "Shift", 0x2d1e28, "shr edx, 4"),
    (0x2d1e2d, "Tick8", 0x2d1e2b, "imul edx, edx, 0x32"),
    (0x2d2273, "Magic", 0x2d2272, "mov eax, 0x51eb851f"),
    (0x2d2280, "Shift", 0x2d227e, "shr esi, 4"),
    (0x2d2284, "Tick8", 0x2d2282, "imul esi, esi, 0x32"),
    (0x2d4eef, "Magic", 0x2d4eee, "mov eax, 0x51eb851f"),
    (0x2d4ef9, "Shift", 0x2d4ef7, "shr esi, 4"),
    (0x2d4efd, "Tick8", 0x2d4efb, "imul esi, esi, 0x32"),
    (0x2d502f, "Magic", 0x2d502e, "mov eax, 0x51eb851f"),
    (0x2d5039, "Shift", 0x2d5037, "shr esi, 4"),
    (0x2d503d, "Tick8", 0x2d503b, "imul esi, esi, 0x32"),
    (0x2f696e, "Magic", 0x2f696d, "mov eax, 0x51eb851f"),
    (0x2f698a, "Shift", 0x2f6988, "shr ebx, 4"),
    (0x2f699c, "Tick8", 0x2f699a, "imul eax, eax, 0x32"),
    (0x30a154, "Magic", 0x30a153, "mov eax, 0x51eb851f"),
    (0x30a164, "Shift", 0x30a162, "shr ebx, 4"),
    (0x30a169, "Tick8", 0x30a167, "imul ebx, ebx, 0x32"),
    (0x3169df, "Tick8", 0x3169d8, "add dword ptr [ecx + edx*4 + 0x138], 0x32"),
    (0x316b8c, "Magic", 0x316b8b, "mov eax, 0x51eb851f"),
    (0x316b98, "Shift", 0x316b96, "shr edx, 4"),
    (0x316b9c, "Tick8", 0x316b9a, "imul edx, edx, 0x32"),
    (0x318417, "Magic", 0x318416, "mov eax, 0x51eb851f"),
    (0x318438, "Shift", 0x318436, "shr ebx, 4"),
    (0x31844a, "Tick8", 0x318448, "imul eax, eax, 0x32"),
    (0x3184b8, "Magic", 0x3184b7, "mov eax, 0x51eb851f"),
    (0x3184d2, "Shift", 0x3184d0, "shr edx, 4"),
    (0x318542, "Magic", 0x318541, "mov eax, 0x51eb851f"),
    (0x318565, "Shift", 0x318563, "shr ebx, 4"),
    (0x318574, "Tick8", 0x318572, "imul eax, eax, 0x32"),
    (0x3185a7, "Magic", 0x3185a6, "mov eax, 0x51eb851f"),
    (0x3185c1, "Shift", 0x3185bf, "shr edx, 4"),
    (0x3185f1, "Magic", 0x3185f0, "mov eax, 0x51eb851f"),
    (0x318610, "Shift", 0x31860e, "shr edx, 4"),
    (0x318624, "Magic", 0x318623, "mov eax, 0x51eb851f"),
    (0x31862c, "Shift", 0x31862a, "shr edx, 4"),
    (0x31862f, "Tick8", 0x31862d, "imul edx, edx, 0x32"),
    (0x318638, "Magic", 0x318637, "mov eax, 0x51eb851f"),
    (0x318651, "Shift", 0x31864f, "shr edx, 4"),
    (0x318692, "Magic", 0x318691, "mov eax, 0x51eb851f"),
    (0x3186b5, "Shift", 0x3186b3, "shr ebx, 4"),
    (0x3186cb, "Tick8", 0x3186c9, "imul ecx, ecx, 0x32"),
    (0x31bd92, "Magic", 0x31bd91, "mov eax, 0x51eb851f"),
    (0x31bd9a, "Shift", 0x31bd98, "shr edx, 4"),
    (0x31bd9d, "Tick8", 0x31bd9b, "imul edx, edx, 0x32"),
    (0x27cc53, "Hash8", 0x27cc4f, "mov dword ptr [esp + 0x18], 0x64"),
    (0x1f7b0c, "Fraction", 0x1f7b0a, "fmul dword ptr [0xa2db34]"),
    (0x075612, "Physics", 0x075610, "fld dword ptr [0xa2dbb4]"),
    (0x07562e, "Physics", 0x07562c, "fld dword ptr [0xa2dbb4]"),
]
SITES.sort()

# Each kind: its bytes, and their value in the original exe.
WIDTH = {"Magic": 4, "Shift": 1, "Tick8": 1, "Back8": 1, "Tick32": 4, "Hash8": 1, "Fraction": 4, "Physics": 4}
ORIGINAL = {"Magic": 0x51EB851F, "Shift": 4, "Tick8": 50, "Back8": 0xCE, "Tick32": 50, "Hash8": 100,
            "Fraction": 0xA2DB34, "Physics": 0xA2DBB4}


def rust():
    rows = "".join(f"    Site {{ at: {at:#08x}, kind: {kind} }}, // {va:#08x} {text}\n"
                   for at, kind, va, text in SITES)
    return (
        "//! Generated by tick_sites_026g.py --write from its table (netcode_port_069.md\n"
        "//! section 6.1): the tick's sites in the exe. Do not edit.\n"
        "\n"
        "use crate::restamp::Kind::*;\n"
        "use crate::restamp::Site;\n"
        "\n"
        f"pub const SITES: [Site; {len(SITES)}] = [\n{rows}];\n"
    ).encode()


def check_exe(data):
    """Every row against the original exe (its bytes); the table complete. Problems."""
    import capstone
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    count = struct.unpack_from("<H", data, pe + 6)[0]
    table = pe + 24 + struct.unpack_from("<H", data, pe + 20)[0]
    sections = [struct.unpack_from("<8sIIII", data, table + 40 * i) for i in range(count)]
    base = struct.unpack_from("<I", data, pe + 24 + 28)[0]          # ImageBase (0x10000)

    def offset(va):
        for name, vsize, rva, rsize, raw in sections:
            if rva <= va - base < rva + max(vsize, rsize):
                return raw + va - base - rva
        raise KeyError(hex(va))

    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
    problems = []
    for at, kind, va, text in SITES:
        ins = next(md.disasm(data[offset(va):offset(va) + 16], va, 1))
        width = WIDTH[kind]
        value = int.from_bytes(data[offset(at):offset(at) + width], "little")
        if f"{ins.mnemonic} {ins.op_str}" != text or not va < at <= va + ins.size - width \
                or value != ORIGINAL[kind]:
            problems.append(f"{at:#x} ({kind}): {ins.mnemonic} {ins.op_str}, {value:#x}")
    name, vsize, rva, rsize, raw = next(s for s in sections if s[0].rstrip(b"\0") == b".text")
    stamped = {at for at, _, _, _ in SITES}
    divisions = 0
    for i in range(raw, raw + rsize - 5):
        if data[i] & 0xF8 == 0xB8 and data[i + 1:i + 5] == b"\x1f\x85\xeb\x51":
            va = base + rva + i - raw
            after = list(md.disasm(data[i:i + 0x60], va, 20))
            shift = next((x for x in after if x.mnemonic == "shr" and x.op_str.split(", ")[-1].isdigit()), None)
            if shift is not None and shift.op_str.endswith(", 4"):
                divisions += 1
                if va + 1 not in stamped or shift.address + shift.size - 1 not in stamped:
                    problems.append(f"{va:#x}: a division by 50 the table does not have")
    kinds = {k: sum(1 for _, kind, _, _ in SITES if kind == k) for k in WIDTH}
    if divisions != 46 or kinds != {"Magic": 46, "Shift": 46, "Tick8": 46, "Back8": 2, "Tick32": 7, "Hash8": 1,
                                    "Fraction": 1, "Physics": 2}:
        problems.append(f"{divisions} divisions by 50 in the code, the table's kinds {kinds}")
    return problems


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("exe", nargs="?")
    args = parser.parse_args()
    if args.write == args.check:
        parser.error("--write or --check")
    if args.write:
        RUST.write_bytes(rust())
        print(f"{RUST.name}: {len(SITES)} sites")
        return
    bad = []
    if not RUST.exists() or RUST.read_bytes() != rust():
        bad.append(f"{RUST.name} is not what the table gives (--write)")
    if args.exe:
        bad += check_exe(open(args.exe, "rb").read())
    print("tick_sites_026g: " + ("current" + (f", {len(SITES)} sites as the original's" if args.exe else "")
                                 if not bad else "\n  ".join(["FAILED"] + bad)))
    raise SystemExit(1 if bad else 0)


if __name__ == "__main__":
    main()
