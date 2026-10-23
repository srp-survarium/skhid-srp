#!/usr/bin/env python3
"""The engine-function library: one plain cdecl function per engine function
our patches call, whatever registers, stack slots and cleanup the game's
compiler (LTO) gave the real one.

    python engine_026g.py --write [--nasm /path/to/nasm]
        writes engine_026g.h (the C declarations), engine_026g.inc (their
        addresses for NASM patches), engine_026g.asm (the thunks) and
        engine_026g.section.bin (engine_026g.asm assembled for ENGINE_VA)
    python engine_026g.py --check [--nasm /path/to/nasm]
        says whether those four files are what engine_026g.tsv and the
        re-kit rows give here (the self-test runs the same)

The library's functions are engine_026g.tsv's rows, in order (append only: a
row's index is its entry's place, pinned in patch_026g.py's ENGINE_PINNED;
a row out of that order is refused):

    symbol     the C name (engine_...)
    va         the real function's VA (0.26g never rebases)
    arguments  per C parameter, in order, LOCATION:word: where the real
               function takes it (EAX ECX EDX EBX ESI EDI EBP, or ESP+n, n =
               4, 8, ...: the stack slot above the return address) and a word
               of the re-kit's role for that location; "-" for none
    prototype  the C declaration, `ret name(type param, ...)`

Where the real function takes its arguments and what it pops comes from the
re-kit (tools/re-kit/v026g/abi_inputs.tsv and abi.tsv, every row of the VA):
a row's arguments must name exactly the locations those rows have, the stack
slots 4, 8, ... without a gap, and abi.tsv must give the cleanup (one value,
at most the slots' bytes). Each argument's word must be in a re-kit role of
its location, and no two arguments' words may each fit the other's location:
two parameters given each other's locations are refused (a swap the thunk
would follow faithfully, which no entry check can see). A result is read
from abi.tsv's `result` (EAX, AL widened to EAX, EDX:EAX for a 64-bit
return, XMM0 moved to ST0 for a float).
Anything else (an XMM or byte-register argument, varargs, a range) is
refused: add the row to the re-kit, or call that function by hand.

Each function is a `jmp near` at ENGINE_VA + 5 x its index (the entries never
move), to a thunk that keeps EBX, ESI, EDI and EBP, puts the cdecl arguments
where the row says, calls the real VA, pops what the real function leaves on
the stack and returns the result in EAX (EDX:EAX, ST0). C includes
engine_026g.h and calls `engine_...(...)` (the header gives each its address);
NASM includes engine_026g.inc and does `push ... / call engine_... / add esp,
4n`. check_engine_026g.py proves each thunk against the rows and the
original exe (the self-test runs it).
"""
import argparse
import csv
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

import patch_026g as base

HERE = Path(__file__).resolve().parent
REKIT = HERE / ".." / ".." / "tools" / "re-kit" / "v026g"
SPEC = HERE / "engine_026g.tsv"
HEADER, INCLUDE, ASM, BIN = (HERE / f"engine_026g{suffix}" for suffix in (".h", ".inc", ".asm", ".section.bin"))
REGISTERS = ("EAX", "ECX", "EDX", "EBX", "ESI", "EDI", "EBP")
SAVED = ("ebp", "ebx", "esi", "edi")             # pushed in this order; cdecl's callee-saved
ENTRY_SIZE = base.ENGINE_ENTRY
# "entry ESP+4", "stack+8", "ESP+0x10", "entry ESP+0Ch low byte", "low byte of entry ESP+8"
SLOT = re.compile(r"(?:low byte of )?(?:entry ESP|ESP|stack)\+(0x[0-9A-Fa-f]+|[0-9][0-9A-Fa-f]*h|[0-9]+)(?:\s.*)?")


class Refused(Exception):
    """A row the library cannot make a thunk of."""


def read_tsv(path):
    with open(path, newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f, delimiter="\t", quoting=csv.QUOTE_NONE))


def number(text):
    text = text.strip()
    if text.lower().endswith("h"):
        return int(text[:-1], 16)
    return int(text, 0)


def location(text):
    """A re-kit location as the library names it: a register ("EAX") or a
    stack slot ("ESP+4"); a byte register is its register, a slot's
    qualifier ("low byte", "float32") its slot."""
    text = text.strip()
    if text.upper() in REGISTERS:
        return text.upper()
    if text.upper() in ("AL", "CL", "DL", "BL"):
        return "E" + text[0].upper() + "X"
    match = SLOT.fullmatch(text)
    if match and ".." not in text:
        offset = number(match.group(1))
        if offset >= 4 and offset % 4 == 0:
            return f"ESP+{offset}"
    raise Refused(f"location {text!r}")


def result_of(text):
    match = re.match(r"\s*(EDX:EAX|EAX|AL|XMM0)\b", text or "")
    return match.group(1) if match else None


def rows_of(va, abi=None, inputs=None):
    """{"locations": set, "cleanups": set, "results": set, "names": set} of
    every re-kit row of `va`."""
    abi = read_tsv(REKIT / "abi.tsv") if abi is None else abi
    inputs = read_tsv(REKIT / "abi_inputs.tsv") if inputs is None else inputs
    out = {"locations": set(), "cleanups": set(), "results": set(), "names": set(), "roles": {}}
    for row in abi:
        if int(row["va"], 16) == va:
            out["names"].add(row["name"])
            for cleanup in re.split(r"[ ,;/]+", row["ret_cleanup"].strip()):
                if cleanup and not cleanup.isdigit():
                    raise Refused(f"{va:#x}: ret_cleanup {row['ret_cleanup']!r} is no number of bytes")
                if cleanup:
                    out["cleanups"].add(int(cleanup))
            if result_of(row["result"]):
                out["results"].add(result_of(row["result"]))
    for row in inputs:
        if int(row["va"], 16) == va:
            out["names"].add(row["name"])
            out["locations"].add(location(row["location"]))
            out["roles"].setdefault(location(row["location"]), []).append(row["role"])
    out["names"].discard("")
    return out


def parse_prototype(text):
    """(return type, name, [(type, param)]) of `ret name(type param, ...)`."""
    match = re.fullmatch(r"\s*(.+?)\s*\b([A-Za-z_]\w*)\s*\((.*)\)\s*", text)
    if not match:
        raise Refused(f"prototype {text!r}")
    ret, name, params = match.group(1), match.group(2), match.group(3).strip()
    out = []
    if params and params != "void":
        for param in params.split(","):
            pm = re.fullmatch(r"\s*(.+?)\s*\b([A-Za-z_]\w*)\s*", param)
            if not pm:
                raise Refused(f"parameter {param!r}")
            kind = pm.group(1)
            if "*" not in kind and re.search(r"\b(double|long long|struct|union)\b", kind):
                raise Refused(f"parameter {param.strip()!r}: not four bytes")
            out.append((kind, pm.group(2)))
    return ret, name, out


def return_kind(ret):
    """How the C side takes the result: None (void), "float", "u64", "word"."""
    ret = ret.strip()
    if ret == "void":
        return None
    if "*" not in ret and ret == "float":
        return "float"
    if "*" not in ret and re.search(r"\blong long\b", ret):
        return "u64"
    if "*" not in ret and re.search(r"\b(double|struct|union)\b", ret):
        raise Refused(f"return type {ret!r}")
    return "word"


def function(row, abi=None, inputs=None):
    """A spec row checked against the re-kit: {symbol, va, args: [location],
    params, ret, cleanup, result, name}."""
    va = number(row["va"])
    ret, name, params = parse_prototype(row["prototype"])
    if name != row["symbol"] or not re.fullmatch(r"engine_[a-z0-9_]+", name):
        raise Refused(f"{row['symbol']}: the prototype names {name}; a symbol is engine_<what>")
    tokens = [] if row["arguments"].strip() == "-" else [a.partition(":") for a in row["arguments"].split()]
    if any(not word for _, _, word in tokens):
        raise Refused(f"{name}: each argument is LOCATION:word (a word of the re-kit's role there)")
    args, words = [location(a) for a, _, _ in tokens], [word for _, _, word in tokens]
    if len(args) != len(params) or len(set(args)) != len(args):
        raise Refused(f"{name}: {len(params)} parameters, {len(args)} locations (each once)")
    rows = rows_of(va, abi, inputs)
    if not rows["names"] and not rows["cleanups"]:
        raise Refused(f"{name}: no re-kit row of {va:#x}")
    if set(args) != rows["locations"]:
        raise Refused(f"{name}: the arguments {sorted(args)} are not the re-kit's {sorted(rows['locations'])} "
                      f"for {va:#x}")
    fits = lambda word, at: any(re.search(rf"(?<![\w]){re.escape(word)}(?![\w])", role, re.I)
                                for role in rows["roles"].get(at, ()))
    for i, (param, at, word) in enumerate(zip(params, args, words)):
        if not fits(word, at):
            raise Refused(f"{name}: {param[1]} is in {at}, but no re-kit role of {at} for {va:#x} says {word!r} "
                          f"({' | '.join(rows['roles'].get(at, ()))})")
        for j in range(i + 1, len(args)):
            if fits(word, args[j]) and fits(words[j], at):
                raise Refused(f"{name}: {word!r} and {words[j]!r} fit both {at} and {args[j]}: words that tell "
                              "the two apart")
    slots = sorted(int(a[4:]) for a in args if a.startswith("ESP+"))
    if slots != list(range(4, 4 * len(slots) + 4, 4)):
        raise Refused(f"{name}: the stack slots {slots} have a gap")
    if len(rows["cleanups"]) != 1:
        raise Refused(f"{name}: abi.tsv gives {sorted(rows['cleanups']) or 'no'} cleanup for {va:#x}; one is needed")
    cleanup = rows["cleanups"].pop()
    if cleanup % 4 or cleanup > 4 * len(slots):
        raise Refused(f"{name}: {va:#x} pops {cleanup} bytes; the rows have {4 * len(slots)} bytes of stack")
    kind = return_kind(ret)
    result = None
    if kind is not None:
        if len(rows["results"]) != 1:
            raise Refused(f"{name}: returns {ret}, abi.tsv gives {sorted(rows['results']) or 'no'} result")
        result = rows["results"].pop()
        fits = {"word": ("EAX", "AL"), "u64": ("EDX:EAX",), "float": ("XMM0",)}[kind]
        if result not in fits:
            raise Refused(f"{name}: returns {ret}, but {va:#x}'s result is {result}")
    return {"symbol": name, "va": va, "args": args, "params": params, "ret": ret, "cleanup": cleanup,
            "result": result, "name": sorted(rows["names"])[0] if rows["names"] else "",
            "prototype": row["prototype"].strip()}


def functions(spec=None, abi=None, inputs=None):
    spec = read_tsv(SPEC) if spec is None else spec
    abi = read_tsv(REKIT / "abi.tsv") if abi is None else abi
    inputs = read_tsv(REKIT / "abi_inputs.tsv") if inputs is None else inputs
    out = [function(row, abi, inputs) for row in spec]
    names = [f["symbol"] for f in out]
    if len(set(names)) != len(names):
        raise Refused("a symbol twice")
    # The entries' places are pinned: code built against the header calls
    # them by address, so a row moved or dropped would call another function.
    pinned = {name: entry_va(i) for i, name in enumerate(names)}
    if pinned != base.ENGINE_PINNED or names != list(base.ENGINE_PINNED):
        moved = [n for n in names if base.ENGINE_PINNED.get(n) != pinned[n]]
        gone = [n for n in base.ENGINE_PINNED if n not in pinned]
        raise Refused("the rows are not at the entries patch_026g.py's ENGINE_PINNED gives them (append only; a "
                      "new row adds its entry there): " + ", ".join(
                          [f"{n} at {pinned[n]:#x}" for n in moved] + [f"{n} gone" for n in gone]))
    return out


def entry_va(index, va=None):
    return (base.ENGINE_VA if va is None else va) + ENTRY_SIZE * index


def describe(f):
    where = ", ".join(f"{p[1]} {a}" for p, a in zip(f["params"], f["args"])) or "no arguments"
    return (f"{f['va']:#x} {f['name'] or '(unnamed)'}: {where}; pops {f['cleanup']}"
            + (f"; {f['result']}" if f["result"] else ""))


GENERATED = "generated by engine_026g.py from engine_026g.tsv and tools/re-kit/v026g/abi*.tsv: do not edit"


def thunk(f):
    """The thunk's NASM lines."""
    lines = [f"{f['symbol']}:", f"    ; {describe(f)}"]
    lines += [f"    push {r}" for r in SAVED]
    frame = 4 * len(SAVED) + 4                         # the first cdecl argument, from esp
    stack = sorted(((int(a[4:]), i) for i, a in enumerate(f["args"]) if a.startswith("ESP+")), reverse=True)
    pushed = 0
    for slot, i in stack:
        lines.append(f"    push dword [esp + {frame + 4 * i + pushed}]   ; {f['params'][i][1]} -> {slot}")
        pushed += 4
    for i, a in enumerate(f["args"]):
        if not a.startswith("ESP+"):
            lines.append(f"    mov {a.lower()}, [esp + {frame + 4 * i + pushed}]   ; {f['params'][i][1]}")
    lines.append(f"    call {f['va']:#x}")
    left = pushed - f["cleanup"]
    if left:
        lines.append(f"    add esp, {left}                  ; the caller's part of the stack")
    if f["result"] == "AL":
        lines.append("    movzx eax, al")
    elif f["result"] == "XMM0":
        lines += ["    sub esp, 4", "    movss [esp], xmm0", "    fld dword [esp]", "    add esp, 4"]
    lines += [f"    pop {r}" for r in reversed(SAVED)]
    lines.append("    ret")
    return lines


def asm_text(fs):
    lines = [f"; The engine-function library ({GENERATED}).",
             "; A `jmp near` per function at CAVE + 5 x its row, then the thunks: cdecl in,",
             "; the engine's registers and stack out (engine_026g.py).",
             "bits 32", "org CAVE", "", "entries:"]
    lines += [f"    jmp near {f['symbol']:<40} ; +{ENTRY_SIZE * i:#x}" for i, f in enumerate(fs)]
    for f in fs:
        lines += ["", "    align 16, db 0xCC"] + thunk(f)
    return "\n".join(lines) + "\n"


def header_text(fs, va=None):
    tags = sorted({t for f in fs for t in re.findall(r"\bstruct\s+(\w+)", f["prototype"])})
    lines = [f"/* The engine-function library ({GENERATED}).",
             " *",
             " * Ordinary cdecl functions: each a thunk in `.srpx` (block engine.code) that",
             " * calls the game's function with its arguments where the game's compiler put",
             " * them (re-kit abi_inputs.tsv) and pops what it pops (abi.tsv). Each",
             " * declaration's comment is the real function's VA, name and shape. */",
             "#ifndef ENGINE_026G_H", "#define ENGINE_026G_H", ""]
    lines += [f"struct {t};" for t in tags] + ([""] if tags else [])
    for i, f in enumerate(fs):
        lines += [f"/* {describe(f)} */", f"{f['prototype']};",
                  f'__asm__(".set {f["symbol"]}, {entry_va(i, va):#x}");', ""]
    lines.append("#endif")
    return "\n".join(lines) + "\n"


def include_text(fs, va=None):
    lines = [f"; The engine-function library's entries ({GENERATED}).",
             "; cdecl: push the arguments right to left, call, add esp, 4 x their count;",
             "; EBX, ESI, EDI and EBP are kept, EAX, ECX and EDX are not."]
    for i, f in enumerate(fs):
        lines += [f"; {f['prototype']}", f"; {describe(f)}", f"%define {f['symbol']} {entry_va(i, va):#x}"]
    return "\n".join(lines) + "\n"


def assemble(text, va=None, nasm="nasm"):
    with tempfile.TemporaryDirectory(prefix="srp-engine-") as temporary:
        source, out = Path(temporary) / "engine.asm", Path(temporary) / "engine.bin"
        source.write_text(text)
        subprocess.run([nasm, "-f", "bin", f"-DCAVE={(base.ENGINE_VA if va is None else va):#x}", str(source),
                        "-o", str(out)], check=True)
        return out.read_bytes()


def outputs(nasm="nasm", spec=None, abi=None, inputs=None, va=None):
    """{path: bytes} of the four files."""
    fs = functions(spec, abi, inputs)
    text = asm_text(fs)
    code = assemble(text, va, nasm)
    if len(code) > base.ARENA_BLOCKS["engine.code"].size:
        raise Refused(f"the library is {len(code):#x} bytes; its block keeps {base.ARENA_BLOCKS['engine.code'].size:#x}")
    return {HEADER: header_text(fs, va).encode(), INCLUDE: include_text(fs, va).encode(), ASM: text.encode(),
            BIN: code}


def stale(nasm="nasm"):
    """The files that are not what the rows give ([] when all are current)."""
    return [path.name for path, content in outputs(nasm).items()
            if not path.exists() or path.read_bytes() != content]


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--nasm", default=os.path.join(os.environ.get("NASM_DIR", ""), "nasm")
                        if os.environ.get("NASM_DIR") else "nasm")
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if args.write == args.check:
        parser.error("--write or --check")
    try:
        files = outputs(args.nasm)
    except Refused as error:
        sys.exit(f"engine_026g.tsv: {error}")
    if args.write:
        for path, content in files.items():
            path.write_bytes(content)
        print(f"{BIN.name}: {len(files[BIN])} bytes, {len(functions())} functions at {base.ENGINE_VA:#x}")
        return
    bad = [path.name for path, content in files.items() if not path.exists() or path.read_bytes() != content]
    print("engine_026g: " + ("current" if not bad else "NOT what the rows give: " + ", ".join(bad)))
    raise SystemExit(1 if bad else 0)


if __name__ == "__main__":
    main()
