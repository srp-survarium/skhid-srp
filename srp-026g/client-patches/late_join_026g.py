#!/usr/bin/env python3
"""The late-join code of the 0.26g client (patch revision 5): builds what
patch_026g.py puts into the exe, late_join_026g.section.bin.

    python late_join_026g.py --write-bin [--nasm /path/to/nasm]
        assembles late_join_026g.asm and compiles the three C files into
        late_join_026g.section.bin, and writes late_join_026g.section.json
        beside it: where the C functions are that the assembly calls, and
        what built them
    python late_join_026g.py --check-bin [--nasm /path/to/nasm]
        builds the same in memory and says whether the two files are what
        the sources give here
    python late_join_026g.py --write-prefix [--nasm /path/to/nasm]
        assembles late_join_026g.asm again into the .bin's first 4096
        bytes and keeps its C code as it is (the .json's symbols): for a
        change of the assembly alone, while the ports to Rust take its
        entries out (another compiler's C would be other bytes)
    python late_join_026g.py ORIGINAL OUTPUT [--server=host:port] [--test-session N]
        a client from the sources as they are now, not from the .bin: for
        work on the code. --test-session is for tests only: such an exe
        passes the login by and connects to -server as that session

Requires NASM, a C compiler for x86 and GNU ld; CC and LD name others than
`cc` and `ld`. The C code another compiler makes is other bytes that do the
same: the checked-in .bin is the one the players' exe has, and the .json
names what made it. Sources and existing outputs are never changed, except
the two files by --write-bin.

The layout is patch_026g.py's (LATE_JOIN_*): the code at the second page of
`.srp`, 4096 bytes of assembly whose entries are at fixed offsets and the C
code after them; the state on the page after the code.
"""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import struct
import subprocess
import tempfile

import patch_026g as base

ASM_SIZE = base.LATE_JOIN_ASM_SIZE
# What the exe says at its sign-in: patch_026g.py's.
PATCH_REVISION = base.REVISION
HERE = Path(__file__).resolve().parent
BIN = HERE / "late_join_026g.section.bin"
SYMBOLS = HERE / "late_join_026g.section.json"
C_SOURCES = ("late_join_validate_026g", "late_join_runtime_026g", "late_join_install_026g")
# The C functions the assembly calls, by the name of its -D.
ENTRIES = {
    "LJ_COMPLETE": "lj_complete",
    "LJ_PREPARE": "lj_prepare",
    "LJ_ACTIVATE": "lj_activate",
    "LJ_DEACTIVATE": "lj_deactivate",
    "LJ_DESTROYED": "lj_player_destroyed",
    "LJ_INSTALL": "lj_native_install",
    "LJ_ANIMATIONS": "lj_native_animations",
}


def c_payload(directory):
    """Link position-independent validation and asynchronous ownership code."""
    binary = directory / "runtime.bin"
    link_map = directory / "runtime.map"
    script = directory / "validate.ld"
    script.write_text("""
SECTIONS {
  . = 0;
  _GLOBAL_OFFSET_TABLE_ = .;
  .text : { *(.text.entry) *(.text*) }
  .unexpected : { *(.rodata*) *(.data*) *(.bss*) *(.got*) }
  /DISCARD/ : { *(.comment) *(.note*) *(.eh_frame*) *(.llvm_addrsig) }
  ASSERT(SIZEOF(.unexpected) == 0, "validator must have no static storage")
}
""")
    objects = []
    for name in C_SOURCES:
        source = HERE / (name + ".c")
        obj = directory / (name + ".o")
        subprocess.run([os.environ.get("CC", "cc"), "-m32", "-Os", "-std=c11",
                        "-ffreestanding", "-fPIC", "-fvisibility=hidden", "-fno-builtin",
                        "-fno-stack-protector", "-mno-sse", "-mno-sse2", "-mno-mmx",
                        "-fno-asynchronous-unwind-tables", "-fno-unwind-tables",
                        "-Wall", "-Wextra", "-Werror", "-c", str(source), "-o", str(obj)], check=True)
        objects.append(str(obj))
    subprocess.run([os.environ.get("LD", "ld"), "-m", "elf_i386", "-T", str(script),
                    "--oformat=binary", "-Map", str(link_map), *objects, "-o", str(binary)], check=True)
    symbols = {name: int(address, 16) for address, name in re.findall(
        r"^\s+(0x[0-9a-f]+)\s+(lj_\w+|late_join_validate_profile)\s*$",
        link_map.read_text(), re.MULTILINE)}
    if symbols.get("late_join_validate_profile") != 0 or any(name not in symbols for name in ENTRIES.values()):
        raise ValueError("missing C payload entry points")
    return binary.read_bytes(), symbols


def nasm_defines(symbols, size):
    """The -D of late_join_026g.asm for C code of `size` bytes whose
    functions are at `symbols`."""
    c_code = base.LATE_JOIN_VA + ASM_SIZE
    state = (c_code + size + 0xFFF) // 0x1000 * 0x1000
    defines = [f"-DCAVE={base.LATE_JOIN_VA}", f"-DC_CODE={c_code}"]
    defines += [f"-D{define}={c_code + symbols[name]}" for define, name in ENTRIES.items()]
    return defines + [f"-DSTATE_VA={state}", f"-DUPLOAD_READY={base.SECTION_VA}"]


def assemble_prefix(nasm, symbols, size, test_session=None):
    """The 4096 bytes of late_join_026g.asm."""
    with tempfile.TemporaryDirectory(prefix="srp-late-join-") as temporary:
        binary = Path(temporary) / "late-join.bin"
        test = [] if test_session is None else [f"-DTEST_SESSION={test_session}"]
        subprocess.run([nasm, "-f", "bin", *test, *nasm_defines(symbols, size),
                        str(HERE / "late_join_026g.asm"), "-o", str(binary)], check=True)
        prefix = binary.read_bytes()
    if len(prefix) != ASM_SIZE:
        raise ValueError("C payload must follow the NASM prefix")
    return prefix


def assemble(nasm="nasm", test_session=None):
    """(the late-join code, where its C functions are in the C part)"""
    if test_session is not None and not 1 <= test_session <= 0xffffffff:
        raise ValueError("test session must be a nonzero u32")
    with tempfile.TemporaryDirectory(prefix="srp-late-join-") as temporary:
        code, symbols = c_payload(Path(temporary))
    code_symbols = {name: symbols[name] for name in ENTRIES.values()}
    return assemble_prefix(nasm, code_symbols, len(code), test_session) + code, code_symbols


def build(original, nasm="nasm", test_session=None, server=None):
    """The client with the late-join code as the sources give it now."""
    code, _ = assemble(nasm, test_session)
    upload = Path(base.default_bin("section")).read_bytes()
    data, _ = base.build(original, "section", upload, server, code)
    if test_session is not None:
        # Direct connections have no lobby profile whose statistics can reset.
        base.patch(data, 0x27a1d1, bytes.fromhex("33c0"), bytes.fromhex("eb05"))
        address = 0x1ec0e1
        base.patch(data, address, bytes.fromhex("e8fa150000"),
                   b"\xe8" + struct.pack("<i", base.LATE_JOIN_VA + 2560 - address - 5))
        if base.neutralize_relocs(data, [(0x27a1d1, 0x27a1d3), (address, address + 5)]):
            raise ValueError("unexpected relocation in the test-session hooks")
        headers = base.Headers(data)
        struct.pack_into("<I", data, headers.checksum_at, base.pe_checksum(data, headers.checksum_at))
    return data


def tool_versions(nasm):
    def first_line(command):
        try:
            return subprocess.run(command, capture_output=True, text=True, check=True).stdout.splitlines()[0].strip()
        except (OSError, subprocess.CalledProcessError, IndexError):
            return "unknown"
    return {"cc": first_line([os.environ.get("CC", "cc"), "--version"]),
            "ld": first_line([os.environ.get("LD", "ld"), "--version"]),
            "nasm": first_line([nasm, "-v"])}


def described(code, symbols, nasm):
    return {"size": len(code), "sha256": hashlib.sha256(code).hexdigest(),
            "symbols": symbols, "built_with": tool_versions(nasm)}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("original", type=Path, nargs="?")
    parser.add_argument("output", type=Path, nargs="?")
    parser.add_argument("--nasm", default="nasm")
    parser.add_argument("--write-bin", action="store_true")
    parser.add_argument("--check-bin", action="store_true")
    parser.add_argument("--write-prefix", action="store_true")
    parser.add_argument("--server", help="an embedded default login server, as patch_026g.py --server")
    parser.add_argument("--test-session", type=lambda value: int(value, 0),
                        help="test only: bypass login and connect directly to -server with this session")
    args = parser.parse_args()
    if args.write_prefix:
        kept = json.loads(SYMBOLS.read_text())
        c_code = BIN.read_bytes()[ASM_SIZE:]
        code = assemble_prefix(args.nasm, kept["symbols"], len(c_code)) + c_code
        BIN.write_bytes(code)
        kept.update(size=len(code), sha256=hashlib.sha256(code).hexdigest())
        SYMBOLS.write_text(json.dumps(kept, indent=2) + "\n", newline="\n")
        print(f"{BIN.name}: {len(code)} bytes (the assembly again, the C kept), SHA-256 {kept['sha256']}")
        return
    if args.write_bin or args.check_bin:
        if args.original or args.test_session is not None:
            parser.error("--write-bin and --check-bin take no exe and no test session")
        code, symbols = assemble(args.nasm)
        if args.write_bin:
            BIN.write_bytes(code)
            SYMBOLS.write_text(json.dumps(described(code, symbols, args.nasm), indent=2) + "\n", newline="\n")
            print(f"{BIN.name}: {len(code)} bytes, SHA-256 {hashlib.sha256(code).hexdigest()}")
            return
        kept = json.loads(SYMBOLS.read_text())
        same = BIN.read_bytes() == code and kept["symbols"] == symbols and kept["size"] == len(code)
        print(f"{BIN.name}: " + ("current" if same else
                                 f"NOT what the sources give here ({hashlib.sha256(code).hexdigest()}; "
                                 f"built with {tool_versions(args.nasm)}, kept one with {kept['built_with']})"))
        raise SystemExit(0 if same else 1)
    if not args.original or not args.output:
        parser.error("name the original exe and the output, or --write-bin / --check-bin")
    if args.output.exists() or args.output.resolve() == args.original.resolve():
        parser.error("choose a new output file; existing executables are never overwritten")
    data = build(args.original.read_bytes(), args.nasm, args.test_session,
                 base.parse_server(args.server) if args.server else None)
    with args.output.open("xb") as stream:
        stream.write(data)
    print(f"Late-join client from the sources: {args.output}; SHA-256 {hashlib.sha256(data).hexdigest()}")


if __name__ == "__main__":
    main()
