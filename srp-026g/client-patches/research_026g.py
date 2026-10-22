#!/usr/bin/env python3
"""Research's code (exe patch 53): builds research_026g.section.bin, the code
patch_026g.py puts at the start of the section `.srpx`.

    python research_026g.py --write-bin [--nasm /path/to/nasm]
        assembles research_026g.asm and compiles research_rule_026g.c and
        research_native_026g.c into research_026g.section.bin, and writes
        research_026g.section.json beside it: where the C functions are
        that the assembly calls, and what built them
    python research_026g.py --check-bin [--nasm /path/to/nasm]
        builds the same in memory and says whether the two files are what
        the sources give here

Requires NASM, a C compiler for x86 (gcc: `-m32`, SSE2 scalar floats) and GNU
ld; CC and LD name others than `cc` and `ld`. Another compiler gives other
bytes that do the same: the checked-in .bin is the one the players' exe has,
and the .json names what made it. Nix's compiler wrapper adds hardening flags
(stack protector, fortify, PIC, ...) that change the code; the compiler runs
with NIX_HARDENING_ENABLE empty, so a nix-shell gives the same bytes.

The layout (patch_026g.py's RESEARCH_*): at RESEARCH_VA the 256 bytes of
research_026g.asm (three `jmp near` entries at 0, 5 and 0Ah, its stubs), the
C code and its constant data after them, linked for where they run (the exe is
never rebased; the vtables hold the C functions' addresses); the state the C
code keeps (the cook, the hud's cache) on the section's last page,
RESEARCH_STATE_OFFSET, zeroed.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

import patch_026g as base

HERE = Path(__file__).resolve().parent
BIN = HERE / "research_026g.section.bin"
SYMBOLS = HERE / "research_026g.section.json"
C_SOURCES = ("research_rule_026g", "research_native_026g")
ASM_SIZE = base.RESEARCH_ASM_SIZE
# The C functions the assembly calls, by the name of its -D.
ENTRIES = {"RN_REGISTER": "rn_register", "RN_HUD": "rn_hud"}
CFLAGS = ["-m32", "-Os", "-std=c11", "-ffreestanding", "-fno-pic", "-fno-builtin", "-fno-stack-protector",
          "-msse2", "-mfpmath=sse", "-mincoming-stack-boundary=2", "-ffp-contract=off", "-fno-strict-aliasing",
          "-fno-asynchronous-unwind-tables", "-fno-unwind-tables", "-Wall", "-Wextra", "-Werror"]


# Nix's cc wrapper: no hardening flags of its own (see the module's text).
COMPILER_ENV = {**os.environ, "NIX_HARDENING_ENABLE": ""}


def c_payload(directory):
    """The C code linked at RESEARCH_VA + ASM_SIZE: (bytes, {symbol: VA})."""
    c_va = base.RESEARCH_VA + ASM_SIZE
    binary, link_map, script = directory / "research.bin", directory / "research.map", directory / "research.ld"
    script.write_text(f"""
SECTIONS {{
  . = {c_va:#x};
  .text : {{ *(.text*) *(.rodata*) }}
  .unexpected : {{ *(.data*) *(.bss*) *(.got*) *(COMMON) }}
  /DISCARD/ : {{ *(.comment) *(.note*) *(.eh_frame*) *(.llvm_addrsig) }}
  ASSERT(SIZEOF(.unexpected) == 0, "the C code keeps its state on the state page, not in data")
}}
""")
    objects = []
    for name in C_SOURCES:
        obj = directory / (name + ".o")
        subprocess.run([os.environ.get("CC", "cc"), *CFLAGS, f"-DRN_STATE_VA={base.RESEARCH_STATE_VA:#x}",
                        "-c", str(HERE / (name + ".c")), "-o", str(obj)], check=True, env=COMPILER_ENV)
        objects.append(str(obj))
    subprocess.run([os.environ.get("LD", "ld"), "-m", "elf_i386", "-T", str(script), "--oformat=binary",
                    "-Map", str(link_map), *objects, "-o", str(binary)], check=True)
    symbols = {name: int(address, 16) for address, name in re.findall(
        r"^\s+(0x[0-9a-f]+)\s+(rn_\w+|rr_\w+)\s*$", link_map.read_text(), re.MULTILINE)}
    if any(name not in symbols for name in ENTRIES.values()):
        raise ValueError("missing C entry points")
    return binary.read_bytes(), symbols


def assemble(nasm="nasm"):
    """(the section's code, the C functions' VAs)"""
    with tempfile.TemporaryDirectory(prefix="srp-research-") as temporary:
        code, symbols = c_payload(Path(temporary))
        prefix_bin = Path(temporary) / "prefix.bin"
        defines = [f"-DCAVE={base.RESEARCH_VA:#x}"] + [f"-D{define}={symbols[name]:#x}"
                                                        for define, name in ENTRIES.items()]
        subprocess.run([nasm, "-f", "bin", *defines, str(HERE / "research_026g.asm"), "-o", str(prefix_bin)],
                       check=True)
        prefix = prefix_bin.read_bytes()
    if len(prefix) != ASM_SIZE:
        raise ValueError(f"research_026g.asm is {len(prefix)} bytes, not {ASM_SIZE}")
    out = prefix + code
    if len(out) > base.RESEARCH_STATE_OFFSET:
        raise ValueError(f"the code is {len(out):#x} bytes; the state page starts at {base.RESEARCH_STATE_OFFSET:#x}")
    kept = {name: symbols[name] for name in sorted(symbols) if name.startswith(("rn_", "rr_"))}
    return out, kept


def tool_versions(nasm):
    def first_line(command):
        try:
            return subprocess.run(command, capture_output=True, text=True, check=True).stdout.splitlines()[0].strip()
        except (OSError, subprocess.CalledProcessError, IndexError):
            return "unknown"
    return {"cc": first_line([os.environ.get("CC", "cc"), "--version"]),
            "ld": first_line([os.environ.get("LD", "ld"), "--version"]),
            "nasm": first_line([nasm, "-v"])}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--nasm", default="nasm")
    parser.add_argument("--write-bin", action="store_true")
    parser.add_argument("--check-bin", action="store_true")
    args = parser.parse_args()
    if args.write_bin == args.check_bin:
        parser.error("--write-bin or --check-bin")
    code, symbols = assemble(args.nasm)
    described = {"va": f"{base.RESEARCH_VA:#x}", "size": len(code), "sha256": hashlib.sha256(code).hexdigest(),
                 "symbols": {name: f"{va:#x}" for name, va in symbols.items()},
                 "built_with": tool_versions(args.nasm)}
    if args.write_bin:
        BIN.write_bytes(code)
        SYMBOLS.write_text(json.dumps(described, indent=2) + "\n", newline="\n")
        print(f"{BIN.name}: {len(code)} bytes, SHA-256 {described['sha256']}")
        return
    kept = json.loads(SYMBOLS.read_text())
    same = BIN.read_bytes() == code and kept["symbols"] == described["symbols"]
    print(f"{BIN.name}: " + ("current" if same else
                             f"NOT what the sources give here ({described['sha256']}; built with "
                             f"{described['built_with']}, kept one with {kept['built_with']})"))
    raise SystemExit(0 if same else 1)


if __name__ == "__main__":
    main()
