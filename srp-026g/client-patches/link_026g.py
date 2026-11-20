#!/usr/bin/env python3
"""The arena's linker: `.srpx` built from its patches' C and NASM sources
(client_patch.md section 0.3, README "The arena").

    python link_026g.py --write
        links patch_026g.py's ARENA from its sources and writes
        arena_026g.lock.json: the linked arena's SHA-256, its blocks and
        symbols, and what built it
    python link_026g.py --check
        says whether the lock is what the sources give here (the self-test
        and every build compare the same)
    python link_026g.py --sizes
        each block's bytes, pages and languages, each Rust patch's bytes
        (its module's symbols), and the arena against its limit

Each block of ARENA names its sources. Every source becomes a relocatable
ELF32 object, once: NASM (`-f elf32`) for `.asm`, gcc for `.c` (`-m32 -c`,
CFLAGS below, no Nix hardening). GNU ld links all of them together at
ARENA_VA with a linker script made from ARENA: an output section per block,
in ARENA's order, each at the next multiple of its alignment, holding of each
of its sources, in the order given:

    code   their .text, then their .rodata
    data   their .data
    state  their .bss (zeros: in the file as zeros, as every state was)

A block's VA and size are what the link gives it; nothing is reserved or
pinned. A patch reaches another's code by name (the engine library's
`engine_...`, a C function from NASM): ordinary ELF symbols. The game's
functions and data are absolute addresses in the sources (the exe is never
rebased). patch_026g.py takes each hook's target from the link by its
symbol.

Refused, loudly: a missing tool; NASM other than 2.16 (NASM 3 assembles
`jmp near` to a label as a two-byte short jump); a section with bytes no
block takes; a relocation other than R_386_32 and R_386_PC32 (no GOT, PLT or
16-bit address: the code is not position-independent); a symbol no source
defines or one defined twice (ld); a block with nothing in it; an arena past
its limit. CC, LD and NASM (or NASM_DIR, the folder of nasm) name other tools
than `cc`, `ld` and `nasm`; `nix develop ./srp-026g` has them.

The lock: the arena is built from source by every build (the self-test's,
the release's), and the bytes must be the lock's: a change of a source is
followed by `--write` (and the lock's diff shows what moved), and an
unexpected change (another compiler, another NASM) stops the build.
"""
import collections
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve().parent
LOCK = HERE / "arena_026g.lock.json"
KINDS = ("code", "data", "state")
# What gcc gets for a C source (Research's flags since patch 53).
CFLAGS = ["-m32", "-Os", "-std=c11", "-ffreestanding", "-fno-pic", "-fno-builtin", "-fno-stack-protector",
          "-msse2", "-mfpmath=sse", "-mincoming-stack-boundary=2", "-ffp-contract=off", "-fno-strict-aliasing",
          "-fno-asynchronous-unwind-tables", "-fno-unwind-tables", "-Wall", "-Wextra", "-Werror"]
NASM_VERSION = "2.16"
# The input sections each kind of block takes (ld's patterns, and how
# check_object sorts an object's sections).
TAKES = {"code": ((".text", ".text.*"), (".rodata", ".rodata.*")), "data": ((".data", ".data.*"),),
         "state": ((".bss", ".bss.*", "COMMON"),)}
# Sections of an object that hold nothing the game runs: dropped.
DROPPED = (".comment", ".note", ".note.*", ".eh_frame", ".eh_frame.*", ".llvm_addrsig")
RELOCATIONS = {0: "R_386_NONE", 1: "R_386_32", 2: "R_386_PC32", 3: "R_386_GOT32", 4: "R_386_PLT32",
               9: "R_386_GOTOFF", 10: "R_386_GOTPC", 20: "R_386_16", 21: "R_386_PC16", 22: "R_386_8",
               23: "R_386_PC8", 43: "R_386_GOT32X"}
SUPPORTED = (1, 2)

# A block of the arena (patch_026g.py's ARENA): name "<patch>.<what>"; kind
# "code", "data" or "state"; align a power of two from 16 to 0x1000; its
# sources, .c or .asm (relative to this folder), a Rust crate's Cargo.toml,
# or a RustEntry.
ArenaBlock = collections.namedtuple("ArenaBlock", "name kind align sources")
# A way from the game into Rust that the link writes (rust_patches.md): a
# block's source, one of rust_hook_026g.inc's macros, never NASM by hand.
# `symbol` its global (what a hook names), `function` the Rust function it
# calls, `macro` and `argument`:
#   "RUST_HOOK"      a VA: RUST_HOOK function, then `jmp argument` (a hook on
#                    the game's `call argument`: the replaced call goes on as
#                    the game made it);
#   "RUST_THISCALL"  the dwords the game pushes (a vtable slot, a delegate);
#   "RUST_CDECL"     the dwords the game pushes.
RustEntry = collections.namedtuple("RustEntry", "symbol function macro argument")
# Where the link put a block.
Block = collections.namedtuple("Block", "name kind va size")
Linked = collections.namedtuple("Linked", "va size image blocks symbols every built_with sha256")
# blocks: {name: Block}; symbols: {global name: VA}; every: [(name, VA, block,
# global)] of every defined symbol (locals too); image: the arena's bytes from
# `va` (code blocks' gaps int3, the rest zeros).


class LinkError(Exception):
    """The arena cannot be linked; the text says why."""


def matches(name, patterns):
    return any(name == p or (p.endswith(".*") and name.startswith(p[:-1])) for p in patterns)


# ---- the tools -------------------------------------------------------------

def toolchain(**override):
    """{cc, ld, nasm}: each a runnable path, NASM 2.16; else LinkError."""
    nasm = os.path.join(os.environ["NASM_DIR"], "nasm") if os.environ.get("NASM_DIR") else "nasm"
    tools = {"cc": os.environ.get("CC", "cc"), "ld": os.environ.get("LD", "ld"), "nasm": os.environ.get("NASM", nasm)}
    tools.update(override)
    what = {"cc": "a C compiler for x86 (gcc: -m32)", "ld": "GNU ld", "nasm": f"NASM {NASM_VERSION}"}
    for key, tool in tools.items():
        if not shutil.which(tool):
            raise LinkError(f"the arena (`.srpx`) is built from its sources: {what[key]} is needed, and "
                            f"{tool!r} is not there ({key.upper()} names another; `nix develop ./srp-026g` "
                            "has gcc, GNU ld and NASM 2.16)")
    # A Rust crate's source needs rustc (nightly, with rust-src); found when
    # there, asked for by compile_source when a block names a crate.
    for key in ("rustc",):
        tool = override.get(key, os.environ.get(key.upper(), key))
        if shutil.which(tool):
            tools[key] = tool
    version = first_line([tools["nasm"], "-v"])
    if not re.match(rf"NASM version {re.escape(NASM_VERSION)}\b", version):
        raise LinkError(f"{tools['nasm']}: {version!r}; the arena needs NASM {NASM_VERSION} (NASM 3 assembles "
                        "`jmp near` to a label as a two-byte short jump, which moves every entry after it)")
    return tools


def first_line(command):
    try:
        return subprocess.run(command, capture_output=True, text=True, check=True).stdout.splitlines()[0].strip()
    except (OSError, subprocess.CalledProcessError, IndexError):
        return "unknown"


def versions(tools, rust=False):
    out = {"cc": first_line([tools["cc"], "--version"]), "ld": first_line([tools["ld"], "--version"]),
           "nasm": first_line([tools["nasm"], "-v"])}
    if rust:
        out["rustc"] = first_line([tools["rustc"], "--version"])
    return out


def run(command, what, cwd=None):
    env = {**os.environ, "NIX_HARDENING_ENABLE": ""}      # Nix's wrappers: no flags of their own
    done = subprocess.run(command, capture_output=True, text=True, cwd=cwd, env=env)
    if done.returncode:
        raise LinkError(f"{what} failed:\n{(done.stdout + done.stderr).strip()}")


# ---- ELF32 ---------------------------------------------------------------

def elf(data, what):
    """{"sections": [dict], "symbols": [dict]} of an ELF32 little-endian file."""
    if data[:6] != b"\x7fELF\x01\x01":
        raise LinkError(f"{what}: not a 32-bit little-endian ELF file")
    shoff, = struct.unpack_from("<I", data, 0x20)
    shentsize, shnum, shstrndx = struct.unpack_from("<HHH", data, 0x2E)
    sections = []
    for i in range(shnum):
        name, kind, flags, addr, offset, size, link, info, align, entsize = struct.unpack_from(
            "<10I", data, shoff + i * shentsize)
        sections.append({"index": i, "name_at": name, "type": kind, "flags": flags, "addr": addr, "offset": offset,
                         "size": size, "link": link, "info": info, "align": align, "entsize": entsize})

    def string(table, at):
        start = sections[table]["offset"] + at
        return data[start:data.index(b"\0", start)].decode()
    for s in sections:
        s["name"] = string(shstrndx, s["name_at"])
        s["bytes"] = data[s["offset"]:s["offset"] + s["size"]] if s["type"] != 8 else b""   # SHT_NOBITS
    symbols = []
    for s in sections:
        if s["type"] == 2:                                                               # SHT_SYMTAB
            for at in range(0, s["size"], 16):
                name, value, size, info, other, shndx = struct.unpack_from("<IIIBBH", s["bytes"], at)
                symbols.append({"name": string(s["link"], name), "value": value, "size": size,
                                "bind": info >> 4, "type": info & 0xF, "shndx": shndx})
    return {"sections": sections, "symbols": symbols}


def check_object(source, path, kinds_of_source):
    """Every section of the object `path` with bytes is one a block naming
    `source` takes, and every relocation is R_386_32 or R_386_PC32; else
    LinkError. kinds_of_source: the kinds of the blocks that name it."""
    parsed = elf(path.read_bytes(), source)
    for s in parsed["sections"]:
        if s["type"] == 4:                                                               # SHT_RELA
            raise LinkError(f"{source}: RELA relocations ({s['name']}): not an i386 object")
        if s["type"] == 9:                                                               # SHT_REL
            target = parsed["sections"][s["info"]]
            if not target["flags"] & 2 or matches(target["name"], DROPPED):
                continue
            for at in range(0, s["size"], 8):
                offset, info = struct.unpack_from("<II", s["bytes"], at)
                if info & 0xFF not in SUPPORTED:
                    name = RELOCATIONS.get(info & 0xFF, f"type {info & 0xFF}")
                    raise LinkError(f"{source}: a {name} relocation at {target['name']}+{offset:#x}: the arena "
                                    "links R_386_32 and R_386_PC32 only (code built without -fno-pic, or a "
                                    "16- or 8-bit address)")
        if not s["flags"] & 2 or not s["size"] or matches(s["name"], DROPPED):          # SHF_ALLOC, bytes
            continue
        kind = next((k for k, groups in TAKES.items() if any(matches(s["name"], g) for g in groups)), None)
        if kind is None or kind not in kinds_of_source:
            raise LinkError(f"{source}: its section {s['name']} ({s['size']} bytes) goes in "
                            + (f"a {kind} block, and no {kind} block of ARENA names {source}" if kind else
                               "no block (a code block takes .text and .rodata, a data block .data, a state "
                               "block .bss)"))
    if any(sym["shndx"] == 0xFFF2 for sym in parsed["symbols"]) and "state" not in kinds_of_source:  # SHN_COMMON
        raise LinkError(f"{source}: a common symbol, and no state block of ARENA names {source}")


# ---- the link ------------------------------------------------------------

def entry_text(entry):
    """The NASM of a RustEntry; LinkError when it is none."""
    symbol, function, macro, argument = entry
    if not all(re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", n) for n in (symbol, function)) or type(argument) is not int:
        raise LinkError(f"{entry}: no entry (a symbol and a function, identifiers; an integer argument)")
    head = 'bits 32\nsection .text\n%include "rust_hook_026g.inc"\n'
    if macro == "RUST_HOOK" and 0 < argument < 1 << 32:
        return head + f"global {symbol}\n{symbol}:\n    RUST_HOOK {function}\n    jmp {argument:#x}\n"
    if macro in ("RUST_THISCALL", "RUST_CDECL") and 0 <= argument <= 16:
        return head + f"    {macro} {symbol}, {function}, {argument}\n"
    raise LinkError(f"{entry}: the macro RUST_HOOK (and a VA), RUST_THISCALL or RUST_CDECL (and 0 to 16 dwords)")


def source_name(source):
    """A source as messages name it."""
    return f"the entry {source.symbol} ({source.macro})" if isinstance(source, RustEntry) else str(source)


def check_blocks(blocks):
    names = set()
    for block in blocks:
        name, kind, align, sources = block
        if not re.fullmatch(r"[a-z0-9_-]+\.[a-z0-9_-]+", name) or name in names:
            raise LinkError(f"{name!r} is no block name of its own (<patch>.<what>)")
        names.add(name)
        if kind not in KINDS or type(align) is not int or align < 16 or align & (align - 1) or align > 0x1000:
            raise LinkError(f"block {name}: a kind of {', '.join(KINDS)} and an alignment a power of two from "
                            "16 to 0x1000")
        if not sources or len(set(sources)) != len(sources) \
                or any(not (isinstance(s, RustEntry) or str(s).endswith((".c", ".asm"))
                            or Path(s).name == "Cargo.toml") for s in sources):
            raise LinkError(f"block {name}: its sources, each once, .c or .asm files, a Rust crate's Cargo.toml "
                            "or a RustEntry")
        for s in sources:
            if isinstance(s, RustEntry):
                entry_text(s)
    for kind in KINDS:
        seen = [s for b in blocks if b.kind == kind for s in b.sources]
        twice = sorted({s for s in seen if seen.count(s) > 1})
        if twice:
            raise LinkError(f"{', '.join(map(source_name, twice))}: named by two {kind} blocks")


def rodata_symbol(block):
    """The symbol at the start of a code block's .rodata (its .text's end;
    the block's end when it has no .rodata, then not in Linked.symbols): what
    a check disassembles of the block ends there."""
    return "__rodata_" + block.replace(".", "_")


def script_text(blocks, objects, va, limit):
    """The linker script: one output section per block, at ARENA's order."""
    lines = ["SECTIONS", "{", f"  . = {va:#x};"]
    for block in blocks:
        rules = []
        for at, group in enumerate(TAKES[block.kind]):
            if block.kind == "code" and at == 1:
                rules.append(f"    {rodata_symbol(block.name)} = .;")       # where .text ends
            rules += [f'    "{objects[s]}"({" ".join(group)})' for s in block.sources]
        noload = " (NOLOAD)" if block.kind == "state" else ""
        lines += [f"  .{block.name} ALIGN({block.align:#x}){noload} :", "  {"] + rules + ["  }"]
    lines += ["  __arena_end = .;",
              f'  ASSERT(__arena_end <= {va + limit:#x}, "the arena is past its limit (ARENA_LIMIT)")',
              f"  /DISCARD/ : {{ *({' '.join(DROPPED)}) }}", "}"]
    return "\n".join(lines) + "\n"


_objects = {}


def crate_files(manifest):
    """Every file a Rust crate's build reads: its manifest and sources, and
    those of the crates it names by path, in a fixed order."""
    out, todo, seen = [], [manifest.resolve()], set()
    while todo:
        cargo = todo.pop()
        if cargo in seen:
            continue
        seen.add(cargo)
        out.append(cargo)
        out += sorted(p for p in (cargo.parent / "src").rglob("*.rs"))
        for dep in re.findall(r'path\s*=\s*"([^"]+)"', cargo.read_text()):
            target = (cargo.parent / dep).resolve()
            if (target / "Cargo.toml").is_file():
                todo.append(target / "Cargo.toml")
    for included in re.findall(r'include!\("([^"]+)"\)', "".join(
            p.read_text() for p in out if p.suffix == ".rs")):
        for p in out:
            candidate = (p.parent / included).resolve()
            if p.suffix == ".rs" and candidate.is_file() and candidate not in out:
                out.append(candidate)
    return out


def source_path(source):
    return Path(source) if os.path.isabs(source) else HERE / source


def source_digest(source):
    """The SHA-256 of a source as the build reads it (a crate: all its
    files; an entry: its text and the include); None when it is missing."""
    if isinstance(source, RustEntry):
        include = (HERE / "rust_hook_026g.inc").read_bytes()
        return hashlib.sha256(entry_text(source).encode() + b"\0" + include).hexdigest()
    path = source_path(source)
    if not path.is_file():
        return None
    if path.name == "Cargo.toml":
        digest = hashlib.sha256()
        for p in crate_files(path):
            digest.update(str(p.relative_to(HERE.parent)).encode() + b"\0" + p.read_bytes())
        return digest.hexdigest()
    return hashlib.sha256(path.read_bytes()).hexdigest()


# What rustc gets for a Rust crate (`Cargo.toml` source), and for the `core`
# it is built against: the game's i686, no position-independent code, size
# first, aborts, one codegen unit, no debug information, the paths in the
# bytes made relative (the lock does not depend on the folder). No cargo:
# `-Zbuild-std` resolves the whole standard library's workspace (proc_macro's
# crates.io dependencies among them), which needs the network or a warm cache;
# rustc alone builds `core` from the toolchain's rust-src, offline.
RUST_TARGET = "i686-unknown-linux-gnu"
RUST_EDITION = "2024"
RUSTFLAGS = ["--target", RUST_TARGET, "-C", "relocation-model=static", "-C", "target-cpu=pentium4",
             "-C", "opt-level=s", "-C", "panic=abort", "-C", "codegen-units=1", "-C", "debuginfo=0",
             "-C", "force-frame-pointers=no", "-C", "embed-bitcode=yes"]
# rustc links every `no_std` crate against a crate named compiler_builtins.
# The arena takes none of its code (a call to an intrinsic is an undefined
# symbol, which the link refuses), so an empty one stands in for it.
COMPILER_BUILTINS_STUB = ("#![no_std]\n#![feature(compiler_builtins)]\n#![compiler_builtins]\n"
                          "//! The arena's stand-in: no intrinsics (link_026g.py).\n")


def ar_members(data, what):
    """[(name, bytes)] of a System V `ar` archive."""
    if data[:8] != b"!<arch>\n":
        raise LinkError(f"{what}: not an ar archive")
    out, at, names = [], 8, b""
    while at + 60 <= len(data):
        header = data[at:at + 60]
        name, size = header[:16].decode().strip(), int(header[48:58].decode().strip())
        body = data[at + 60:at + 60 + size]
        if name == "//":
            names = body
        elif name.startswith("/") and name[1:].isdigit():
            start = int(name[1:])
            out.append((names[start:names.index(b"/\n", start)].decode(), body))
        elif name not in ("/", "/SYM64/"):
            out.append((name.rstrip("/"), body))
        at += 60 + size + (size & 1)
    return out


def rustc(tools, args, what, env_extra=None):
    env = {k: v for k, v in os.environ.items() if k not in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS")}
    env.update(env_extra or {})
    done = subprocess.run([tools["rustc"], *args], capture_output=True, text=True, env=env)
    if done.returncode:
        raise LinkError(f"rustc {what} failed:\n{(done.stdout + done.stderr).strip()}")


def rust_sysroot(tools):
    """A sysroot of our own for i686 (built once a toolchain, in
    srp-026g/target-arena): core from the toolchain's rust-src with RUSTFLAGS,
    and the empty compiler_builtins."""
    version = first_line([tools["rustc"], "--version"])
    toolchain_root = first_line([tools["rustc"], "--print", "sysroot"])
    key = hashlib.sha256(repr((version, toolchain_root, RUSTFLAGS, COMPILER_BUILTINS_STUB)).encode()).hexdigest()[:16]
    root = HERE.parent / "target-arena" / f"sysroot-{key}"
    lib = root / "lib" / "rustlib" / RUST_TARGET / "lib"
    if (lib / "libcompiler_builtins.rlib").is_file():
        return root
    core = Path(toolchain_root) / "lib" / "rustlib" / "src" / "rust" / "library" / "core" / "src" / "lib.rs"
    if not core.is_file():
        raise LinkError(f"{core}: no rust-src in the toolchain (the dev shell's has it)")
    building = root.with_name(root.name + ".building")
    shutil.rmtree(building, ignore_errors=True)
    blib = building / "lib" / "rustlib" / RUST_TARGET / "lib"
    blib.mkdir(parents=True)
    remap = [f"--remap-path-prefix={toolchain_root}=rustc"]
    rustc(tools, ["--edition", RUST_EDITION, "--crate-name", "core", "--crate-type", "rlib", *RUSTFLAGS, *remap,
                  "-Zforce-unstable-if-unmarked", str(core), "-o", str(blib / "libcore.rlib")], "core")
    stub = building / "compiler_builtins.rs"
    stub.write_text(COMPILER_BUILTINS_STUB)
    rustc(tools, ["--edition", RUST_EDITION, "--crate-name", "compiler_builtins", "--crate-type", "rlib", *RUSTFLAGS,
                  "--sysroot", str(building), str(stub), "-o", str(blib / "libcompiler_builtins.rlib")],
          "compiler_builtins")
    shutil.rmtree(root, ignore_errors=True)
    building.rename(root)
    return root


def crate_of(manifest):
    """(the crate's name, its root source, [the manifests it names by path])."""
    text = manifest.read_text()
    name = re.search(r'^name\s*=\s*"([^"]+)"', text, re.M).group(1).replace("-", "_")
    lib = re.search(r'^\[lib\][^\[]*?^path\s*=\s*"([^"]+)"', text, re.M | re.S)
    deps = [(manifest.parent / d).resolve() / "Cargo.toml" for d in re.findall(r'path\s*=\s*"([^"]+)"', text)
            if (manifest.parent / d / "Cargo.toml").is_file()]
    return name, manifest.parent / (lib.group(1) if lib else "src/lib.rs"), deps


def build_crate(manifest, tools, out):
    """The crate's own object of the staticlib `manifest` describes, built by
    rustc against rust_sysroot(): the crates it names by path as rlibs, then
    it with fat LTO (one object; the archive's core and compiler_builtins
    members are left out: a symbol the crate would need from them is an
    undefined symbol, which the link refuses)."""
    sysroot = rust_sysroot(tools)
    toolchain_root = first_line([tools["rustc"], "--print", "sysroot"])
    remap = [f"--remap-path-prefix={HERE.parent}=srp-026g", f"--remap-path-prefix={toolchain_root}=rustc"]
    with tempfile.TemporaryDirectory(prefix="srp-rust-") as folder:
        built, externs = set(), []

        def rlib(path):
            name, root, deps = crate_of(path)
            for dep in deps:
                rlib(dep)
            if name in built:
                return
            built.add(name)
            target = Path(folder) / f"lib{name}.rlib"
            rustc(tools, ["--edition", RUST_EDITION, "--crate-name", name, "--crate-type", "rlib", *RUSTFLAGS, *remap,
                          "--sysroot", str(sysroot), "-L", folder, *externs, str(root), "-o", str(target)], name)
            externs.extend(["--extern", f"{name}={target}"])

        name, root, deps = crate_of(manifest)
        for dep in deps:
            rlib(dep)
        archive = Path(folder) / f"lib{name}.a"
        rustc(tools, ["--edition", RUST_EDITION, "--crate-name", name, "--crate-type", "staticlib", *RUSTFLAGS, *remap,
                      "-C", "lto=fat", "--sysroot", str(sysroot), "-L", folder, *externs, str(root),
                      "-o", str(archive)], name)
        own = [body for member, body in ar_members(archive.read_bytes(), archive.name)
               if member.endswith(".o") and f".{name}." in member]
        if len(own) != 1:
            raise LinkError(f"{archive.name}: {len(own)} objects of the crate's own; one is needed (fat LTO, one "
                            "codegen unit)")
        out.write_bytes(own[0])


def compile_source(source, tools, folder, name):
    """The object of `source` in `folder` (the same source and tools: built
    once per process)."""
    digest = source_digest(source)
    if digest is None:
        raise LinkError(f"{source}: no such source")
    path = None if isinstance(source, RustEntry) else source_path(source)
    key = (str(path or source), digest, tuple(sorted(tools.items())))
    if key not in _objects:
        out = Path(folder) / ("obj-" + name)
        if path is None:
            text = Path(folder) / (name + ".asm")
            text.write_text(entry_text(source))
            run([tools["nasm"], "-f", "elf32", "-I", str(HERE) + os.sep, str(text), "-o", str(out)],
                f"nasm {source_name(source)}")
        elif path.name == "Cargo.toml":
            if "rustc" not in tools:
                raise LinkError(f"{source}: a Rust crate, and no rustc (`nix develop ./srp-026g` has the "
                                "nightly with rust-src it needs)")
            build_crate(path, tools, out)
        elif path.suffix == ".asm":
            run([tools["nasm"], "-f", "elf32", "-I", str(path.parent) + os.sep, str(path), "-o", str(out)],
                f"nasm {source}")
        else:
            run([tools["cc"], *CFLAGS, "-I", str(path.parent), "-c", str(path), "-o", str(out)], f"cc {source}")
        _objects[key] = out.read_bytes()
    target = Path(folder) / name
    target.write_bytes(_objects[key])
    return target


_links = {}


def link(blocks, va, limit, tools=None):
    """The arena of `blocks` linked at `va`: a Linked; LinkError when it
    cannot be (the text says why). Linked once per process for the same
    blocks, sources and tools."""
    blocks = tuple(blocks)
    check_blocks(blocks)
    tools = toolchain() if tools is None else tools
    sources = list(dict.fromkeys(s for b in blocks for s in b.sources))
    key = (blocks, va, limit, tuple(sorted(tools.items())), tuple(source_digest(s) for s in sources))
    if key in _links:
        return _links[key]
    with tempfile.TemporaryDirectory(prefix="srp-arena-") as folder:
        objects = {}
        for i, source in enumerate(sources):
            stem = f"entry_{source.symbol}" if isinstance(source, RustEntry) else Path(source).stem
            name = f"{i:02d}_{stem}.o"
            path = compile_source(source, tools, folder, name)
            check_object(source_name(source), path, {b.kind for b in blocks if source in b.sources})
            objects[source] = name
        script = Path(folder) / "arena.ld"
        script.write_text(script_text(blocks, objects, va, limit))
        run([tools["ld"], "-m", "elf_i386", "-T", "arena.ld", "-e", "0", "--orphan-handling=discard",
             "-z", "noexecstack", "--no-warn-rwx-segments", "-o", "arena.elf", *objects.values()], "ld", cwd=folder)
        out = elf((Path(folder) / "arena.elf").read_bytes(), "the linked arena")
    rust = any(not isinstance(s, RustEntry) and Path(s).name == "Cargo.toml" for s in sources)
    linked = layout(blocks, out, va, limit, versions(tools, rust))
    _links[key] = linked
    return linked


def layout(blocks, out, va, limit, built_with):
    """The Linked of the linked ELF `out`: each block's section where the
    link put it, checked (in ARENA's order, aligned, none overlapping, inside
    the arena), and the arena's bytes."""
    by_name = {s["name"]: s for s in out["sections"]}
    placed, at = {}, va
    for block in blocks:
        s = by_name.get("." + block.name)
        if s is None or not s["size"]:
            raise LinkError(f"block {block.name}: nothing of its sources ({', '.join(map(source_name, block.sources))}) "
                            f"is in it")
        if s["addr"] < at or s["addr"] % block.align:
            raise LinkError(f"block {block.name} at {s['addr']:#x}: not after the block before it ({at:#x}) or "
                            f"not aligned to {block.align:#x}")
        placed[block.name] = (Block(block.name, block.kind, s["addr"], s["size"]), s["bytes"])
        at = s["addr"] + s["size"]
    if at - va > limit:
        raise LinkError(f"the arena is {at - va:#x} bytes, past its limit {limit:#x} (ARENA_LIMIT)")
    stray = [s["name"] for s in out["sections"] if s["flags"] & 2 and s["size"]
             and s["name"][1:] not in placed]
    if stray:
        raise LinkError(f"the link has sections of no block: {', '.join(stray)}")
    image = bytearray(at - va)
    order = [placed[b.name] for b in blocks]
    for (block, body), following in zip(order, order[1:] + [None]):
        start = block.va - va
        end = (following[0].va - va) if following else len(image)
        if block.kind == "code":
            image[start:end] = body + b"\xCC" * (end - start - len(body))
        elif block.kind == "data":
            image[start:start + len(body)] = body
    every, symbols = [], {}
    for sym in out["symbols"]:
        if sym["shndx"] == 0 or sym["type"] in (3, 4) or not sym["name"] or sym["shndx"] >= 0xFF00:
            continue                                                   # undefined, section, file, absolute
        home = next((b.name for b, _ in order if b.va <= sym["value"] < b.va + b.size), None)
        if home is None:
            continue                                                   # __arena_end and the like
        every.append((sym["name"], sym["value"], home, sym["bind"] == 1))
        if sym["bind"] == 1:
            symbols[sym["name"]] = sym["value"]
    image = bytes(image)
    return Linked(va, len(image), image, {b.name: b for b, _ in order}, dict(sorted(symbols.items())),
                  sorted(every), built_with, hashlib.sha256(image).hexdigest())


def block_bytes(linked, name):
    block = linked.blocks[name]
    return linked.image[block.va - linked.va:block.va - linked.va + block.size]


# ---- the lock ------------------------------------------------------------

def lock_of(linked):
    return {"generated": "by link_026g.py --write from patch_026g.py's ARENA and its sources: do not edit",
            "va": f"{linked.va:#x}", "size": linked.size, "sha256": linked.sha256,
            "blocks": {b.name: {"kind": b.kind, "va": f"{b.va:#x}", "size": b.size} for b in linked.blocks.values()},
            "symbols": {name: f"{value:#x}" for name, value in linked.symbols.items()},
            "built_with": linked.built_with}


def lock_differs(linked, kept):
    """None when `kept` (a lock) is the linked arena's; else why not."""
    if kept is None:
        return "there is no lock"
    mine = lock_of(linked)
    if all(kept.get(k) == mine[k] for k in ("va", "size", "sha256", "blocks", "symbols")):
        return None
    blocks = sorted({n for n in set(mine["blocks"]) | set(kept.get("blocks", {}))
                     if mine["blocks"].get(n) != kept.get("blocks", {}).get(n)})
    return (f"the linked arena is {mine['sha256']} ({mine['size']:#x} bytes), the lock's {kept.get('sha256')}"
            + (f"; blocks that differ: {', '.join(blocks)}" if blocks else "")
            + f"; built here with {mine['built_with']}, the lock with {kept.get('built_with')}")


def read_lock(path=LOCK):
    try:
        return json.loads(Path(path).read_text())
    except FileNotFoundError:
        return None


def rust_path(name):
    """The path of a Rust symbol (legacy mangling, `_ZN` .. `E`, the hash
    left out): ["patches", "suicide_key", "hook"]; [name] when unmangled."""
    if not name.startswith("_ZN"):
        return [name]
    parts, at = [], 3
    while at < len(name) and name[at].isdigit():
        digits = re.match(r"\d+", name[at:]).group()
        at += len(digits)
        parts.append(name[at:at + int(digits)])
        at += int(digits)
    return [p for p in parts if not re.fullmatch(r"h[0-9a-f]{16}", p)] or [name]


def patch_sizes(linked, block, patches):
    """{owner: bytes} of a Rust block: each symbol's bytes up to the next
    symbol (or the block's end), its owner the patch whose module it is in
    (`patches::<patch>::..`) or whose name it starts with (the hooks'
    `#[unsafe(no_mangle)]` functions and the entries: `<patch>`, `<patch>_..`),
    else the crate it is from (patch_core, core: shared)."""
    placed = linked.blocks[block]
    starts = sorted({va for _, va, home, _ in linked.every if home == block} | {placed.va})
    owner_at = {}
    for name, va, home, _ in sorted(linked.every, key=lambda e: (e[1], not e[3], e[0])):
        if home != block or va in owner_at:
            continue
        path = rust_path(name)
        owns = lambda p: name == p or name.startswith(p + "_") or name.startswith("hook_" + p + "_")
        if len(path) > 2 and path[0] == "patches" and path[1] in patches:
            owner_at[va] = path[1]                         # patches::<patch>::..
        elif len(path) == 1 and any(owns(p) for p in patches):
            owner_at[va] = max((p for p in patches if owns(p)), key=len)   # the entry, the shim
        else:
            owner_at[va] = path[0] if len(path) > 1 else "unnamed"         # vostok, core, memcpy/memset
    sizes = collections.Counter()
    for va, end in zip(starts, starts[1:] + [placed.va + placed.size]):
        sizes[owner_at.get(va, "unnamed")] += end - va
    return dict(sizes)


def sizes_text(blocks, linked, limit, patches=()):
    """Each block's bytes, the pages it takes and its languages; each
    patch's bytes in a block of Rust (`patches`: their module names); and
    the arena against its limit (the budget of every patch)."""
    def language(source):
        if isinstance(source, RustEntry):
            return "entries (generated)"
        return "Rust" if Path(source).name == "Cargo.toml" else "C" if str(source).endswith(".c") else "NASM"
    lines = [f"  {'block':<20} {'kind':<5} {'bytes':>7} {'pages':>5}  languages"]
    for block in blocks:
        placed = linked.blocks[block.name]
        pages = (placed.size + 0xFFF) // 0x1000
        languages = ", ".join(dict.fromkeys(language(s) for s in block.sources))
        lines.append(f"  {block.name:<20} {block.kind:<5} {placed.size:>7} {pages:>5}  {languages}")
        if block.kind == "code" and any(language(s) == "Rust" for s in block.sources):
            for owner, size in sorted(patch_sizes(linked, block.name, patches).items(), key=lambda o: -o[1]):
                lines.append(f"      {owner:<24} {size:>7}")
    lines.append(f"the arena: {linked.size} bytes ({linked.size:#x}) of its limit {limit:#x}, "
                 f"{100 * linked.size / limit:.1f}%")
    return "\n".join(lines)


def main():
    import argparse
    import patch_026g as base
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--sizes", action="store_true")
    args = parser.parse_args()
    if args.write + args.check + args.sizes != 1:
        parser.error("--write, --check or --sizes")
    try:
        linked = link(base.ARENA, base.ARENA_VA, base.ARENA_LIMIT)
    except LinkError as error:
        sys.exit(f"link_026g: {error}")
    if args.sizes:
        print(sizes_text(base.ARENA, linked, base.ARENA_LIMIT, base.RUST_PATCHES))
        return
    for block in linked.blocks.values():
        print(f"  {block.name:<16} {block.kind:<5} {block.va:#x} {block.size:#7x} bytes")
    if args.write:
        LOCK.write_text(json.dumps(lock_of(linked), indent=2) + "\n", newline="\n")
        print(f"{LOCK.name}: {linked.size:#x} bytes at {linked.va:#x}, SHA-256 {linked.sha256}")
        return
    why = lock_differs(linked, read_lock())
    print(f"{LOCK.name}: " + ("current" if why is None else f"NOT what the sources give here: {why}"))
    raise SystemExit(0 if why is None else 1)


if __name__ == "__main__":
    main()
