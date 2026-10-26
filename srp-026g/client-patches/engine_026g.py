#!/usr/bin/env python3
"""The engine-function library: one plain cdecl function per engine function
our patches call, whatever registers, stack slots and cleanup the game's
compiler (LTO) gave the real one.

    python engine_026g.py --write
        writes engine_026g.h (the C declarations), engine_026g.inc (their
        externs for NASM patches), engine_026g.rs (the Rust patches' `raw`
        layer: these declarations and fields_026g.tsv's fields) and
        engine_026g.asm (the thunks and the direct functions' addresses, the
        arena's block engine.code: link_026g.py links it)
    python engine_026g.py --check
        says whether those four files are what engine_026g.tsv and the
        re-kit rows give here (the self-test runs the same)

The library's functions are engine_026g.tsv's rows (in any order: each is a
global symbol of its own, which the link resolves for every caller):

    symbol     the C name (engine_...)
    va         the real function's VA (0.26g never rebases)
    arguments  per C parameter, in order, LOCATION:word: where the real
               function takes it (EAX ECX EDX EBX ESI EDI EBP, or ESP+n, n =
               4, 8, ...: the stack slot above the return address) and a word
               of the re-kit's role for that location; "-" for none
    prototype  the C declaration, `ret name(type param, ...)`
    call       how every caller calls it: "thunk" (the default: a cdecl
               thunk, below), "stdcall", "cdecl" or "thiscall" (directly, in
               that convention, which the re-kit's rows must be: the symbol
               is the real VA, no thunk), "import" (va is the exe's IAT slot
               of a kernel32 function, stdcall; the symbol is the slot;
               arguments "-", no re-kit row)

fields_026g.tsv is the game's objects' fields Rust reads and writes (kind,
field, offset, type, about): `raw::<kind>::<field>(this)`, the field's
address, for the `vostok` layer (rust_patches.md).

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

Each function is a thunk, the global symbol `engine_...`, that keeps EBX,
ESI, EDI and EBP, puts the cdecl arguments where the row says, calls the real
VA, pops what the real function leaves on the stack and returns the result in
EAX (EDX:EAX, ST0). C includes engine_026g.h and calls `engine_...(...)`;
NASM includes engine_026g.inc and does `push ... / call engine_... / add esp,
4n`; the link puts each call's address in. check_engine_026g.py proves each
thunk against the rows and the original exe (the self-test runs it).
"""
import argparse
import csv
from pathlib import Path
import re
import sys
import tempfile

import link_026g as linker

HERE = Path(__file__).resolve().parent
REKIT = HERE / ".." / ".." / "tools" / "re-kit" / "v026g"
SPEC = HERE / "engine_026g.tsv"
FIELDS = HERE / "fields_026g.tsv"
HEADER, INCLUDE, ASM, RUST = (HERE / f"engine_026g{suffix}" for suffix in (".h", ".inc", ".asm", ".rs"))
CALLS = ("thunk", "stdcall", "cdecl", "thiscall", "import")
REGISTERS = ("EAX", "ECX", "EDX", "EBX", "ESI", "EDI", "EBP")
SAVED = ("ebp", "ebx", "esi", "edi")             # pushed in this order; cdecl's callee-saved
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


def parse_vostok(text, name, params):
    """The row's optional `vostok` column, the typed signature a Rust hook of
    this function takes: "arg: Type, .. -> Result" (Result left out for void).
    Returns {"args": [(arg, Type)], "result": Result or None}, or None when the
    column is empty (the function has no Rust typing yet). One arg per C
    parameter, each `arg: Type` with an identifier and a Vostok type name."""
    text = (text or "").strip()
    if not text:
        return None
    body, _, result = text.partition("->")
    args = [a.strip() for a in body.split(",") if a.strip()]
    if len(args) != len(params):
        raise Refused(f"{name}: the vostok signature has {len(args)} arguments, the prototype {len(params)}")
    out = []
    for a in args:
        label, _, vtype = a.partition(":")
        if not re.fullmatch(r"[a-z_][a-z0-9_]*", label.strip()) or not re.fullmatch(r"[A-Za-z][\w<>]*", vtype.strip()):
            raise Refused(f"{name}: a vostok argument is `name: Type` (got {a!r})")
        out.append((label.strip(), vtype.strip()))
    return {"args": out, "result": result.strip() or None}


def function(row, abi=None, inputs=None):
    """A spec row checked against the re-kit: {symbol, va, args: [location],
    params, ret, cleanup, result, name, call, vostok}."""
    va = number(row["va"])
    ret, name, params = parse_prototype(row["prototype"])
    vostok = parse_vostok(row.get("vostok"), name, params)
    call = (row.get("call") or "thunk").strip()
    if name != row["symbol"] or not re.fullmatch(r"engine_[a-z0-9_]+", name):
        raise Refused(f"{row['symbol']}: the prototype names {name}; a symbol is engine_<what>")
    if call not in CALLS:
        raise Refused(f"{name}: call {call!r}, one of {', '.join(CALLS)}")
    if call == "import":
        if row["arguments"].strip() != "-":
            raise Refused(f"{name}: an import's arguments are \"-\" (stdcall, its prototype's)")
        return {"symbol": name, "va": va, "args": [f"ESP+{4 * i + 4}" for i in range(len(params))], "params": params,
                "ret": ret, "cleanup": 4 * len(params), "result": None if return_kind(ret) is None else "EAX",
                "name": "kernel32 (the IAT slot)", "prototype": row["prototype"].strip(), "call": call,
                "vostok": vostok}
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
    if call != "thunk":
        stack = [f"ESP+{4 * i + 4}" for i in range(len(args) - (call == "thiscall"))]
        if args != (["ECX"] if call == "thiscall" else []) + stack \
                or cleanup != (0 if call == "cdecl" else 4 * len(stack)) or result not in (None, "EAX", "AL"):
            raise Refused(f"{name}: not {call} ({describe_args(params, args)}; pops {cleanup}; {result})")
        if result == "AL" and not re.fullmatch(r"(unsigned )?char", ret.strip()):
            raise Refused(f"{name}: called directly, its result is AL only: the prototype returns unsigned char")
    return {"symbol": name, "va": va, "args": args, "params": params, "ret": ret, "cleanup": cleanup,
            "result": result, "name": sorted(rows["names"])[0] if rows["names"] else "",
            "prototype": row["prototype"].strip(), "call": call, "vostok": vostok}


def describe_args(params, args):
    return ", ".join(f"{p[1]} {a}" for p, a in zip(params, args)) or "no arguments"


def every_function(spec=None, abi=None, inputs=None):
    """Every row of the spec, checked."""
    spec = read_tsv(SPEC) if spec is None else spec
    abi = read_tsv(REKIT / "abi.tsv") if abi is None else abi
    inputs = read_tsv(REKIT / "abi_inputs.tsv") if inputs is None else inputs
    out = [function(row, abi, inputs) for row in spec]
    names = [f["symbol"] for f in out]
    if len(set(names)) != len(names):
        raise Refused("a symbol twice")
    return out


def functions(spec=None, abi=None, inputs=None):
    """The rows called through a thunk (what check_engine_026g.py proves)."""
    return [f for f in every_function(spec, abi, inputs) if f["call"] == "thunk"]


FIELD_TYPES = {"u8": 1, "u16": 2, "u32": 4, "i32": 4, "f32": 4}


def fields(path=None):
    """fields_026g.tsv's rows: [{kind, module, field, offset, type, about}]."""
    out, seen = [], set()
    for row in read_tsv(FIELDS if path is None else path):
        kind, field, kind_type = row["kind"].strip(), row["field"].strip(), row["type"].strip()
        module = kind.rpartition("::")[2]
        offset = number(row["offset"])
        if not re.fullmatch(r"[a-z][a-z0-9_]*", module) or not re.fullmatch(r"[a-z][a-z0-9_]*", field) \
                or kind_type not in FIELD_TYPES or not 0 <= offset < 0x10000 or offset % FIELD_TYPES[kind_type] \
                or (module, field) in seen:
            raise Refused(f"fields_026g.tsv: {row}: a kind (a::b), a field (snake case, once), an offset "
                          f"under 0x10000 aligned to its type, a type of {', '.join(FIELD_TYPES)}")
        seen.add((module, field))
        out.append({"kind": kind, "module": module, "field": field, "offset": offset, "type": kind_type,
                    "about": row["about"].strip()})
    return out


def describe(f):
    if f["call"] == "import":
        return f"the IAT slot {f['va']:#x} (kernel32, stdcall)"
    where = describe_args(f["params"], f["args"])
    return (f"{f['va']:#x} {f['name'] or '(unnamed)'}: {where}; pops {f['cleanup']}"
            + (f"; {f['result']}" if f["result"] else "")
            + ("" if f["call"] == "thunk" else f"; {f['call']}, called directly"))


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
    """`fs`: every row; a thunk for each called through one, the address of
    each other (a direct function's VA, an import's IAT slot)."""
    thunks = [f for f in fs if f["call"] == "thunk"]
    lines = [f"; The engine-function library ({GENERATED}).",
             "; A thunk per function, each a global symbol: cdecl in, the engine's",
             "; registers and stack out (engine_026g.py). The arena's block engine.code.",
             "; A function called directly (or an import) is its address alone.",
             "bits 32", "section .text"]
    lines += [f"global {f['symbol']}" for f in fs]
    for f in fs:
        if f["call"] != "thunk":
            lines.append(f"{f['symbol']} equ {f['va']:#x}   ; {describe(f)}")
    for i, f in enumerate(thunks):
        lines += [""] + (["    align 16, db 0xCC"] if i else []) + thunk(f)
    return "\n".join(lines) + "\n"


def header_text(fs):
    tags = sorted({t for f in fs for t in re.findall(r"\bstruct\s+(\w+)", f["prototype"])})
    lines = [f"/* The engine-function library ({GENERATED}).",
             " *",
             " * Ordinary cdecl functions: each a thunk in `.srpx` (block engine.code, linked",
             " * by link_026g.py: a call names the function, the link gives its address) that",
             " * calls the game's function with its arguments where the game's compiler put",
             " * them (re-kit abi_inputs.tsv) and pops what it pops (abi.tsv). Each",
             " * declaration's comment is the real function's VA, name and shape. */",
             "#ifndef ENGINE_026G_H", "#define ENGINE_026G_H", ""]
    lines += [f"struct {t};" for t in tags] + ([""] if tags else [])
    for f in fs:
        declaration = f["prototype"]
        if f["call"] in ("stdcall", "cdecl", "thiscall"):
            declaration = declaration.replace(f["symbol"] + "(", f"__attribute__(({f['call']})) {f['symbol']}(", 1)
        elif f["call"] == "import":
            ret, _, params = parse_prototype(f["prototype"])
            declaration = (f"extern {ret} (__attribute__((stdcall)) *const {f['symbol']})("
                           + (", ".join(f"{k} {n}" for k, n in params) or "void") + ")")
        lines += [f"/* {describe(f)} */", f"{declaration};", ""]
    lines.append("#endif")
    return "\n".join(lines) + "\n"


def include_text(fs):
    lines = [f"; The engine-function library's functions ({GENERATED}).",
             "; cdecl: push the arguments right to left, call, add esp, 4 x their count;",
             "; EBX, ESI, EDI and EBP are kept, EAX, ECX and EDX are not. The link gives",
             "; each its address."]
    for f in fs:
        how = {"thunk": [], "import": ["; `call [symbol]`: stdcall, it pops its arguments"]}.get(
            f["call"], [f"; {f['call']}: push (and ECX for thiscall) as that convention says"])
        lines += [f"; {f['prototype']}", f"; {describe(f)}"] + how + [f"extern {f['symbol']}"]
    return "\n".join(lines) + "\n"


RUST_BASES = {"void": "c_void", "char": "u8", "unsigned char": "u8", "unsigned int": "u32", "float": "f32",
              "unsigned long long": "u64"}
# C parameter names that are Rust keywords: written as raw identifiers.
RUST_KEYWORDS = {"as", "async", "await", "box", "break", "const", "continue", "crate", "dyn", "else", "enum",
                 "extern", "false", "fn", "for", "gen", "if", "impl", "in", "let", "loop", "match", "mod", "move",
                 "mut", "pub", "ref", "return", "static", "struct", "trait", "true", "type", "unsafe", "use",
                 "where", "while", "yield"}


def rust_type(kind):
    """The Rust type of a C type of the library's prototypes: the base
    (a struct is c_void) with a `*const` or `*mut` for each `*`, by the
    constness of what it points to."""
    tokens = re.findall(r"\*|[A-Za-z_]\w*", kind)
    stars = [i for i, t in enumerate(tokens) if t == "*"]
    base_tokens = tokens[:stars[0]] if stars else tokens
    const = "const" in base_tokens
    words = [t for t in base_tokens if t != "const"]
    base = "c_void" if words[:1] == ["struct"] else RUST_BASES.get(" ".join(words))
    if base is None or (base == "c_void" and not stars):
        raise Refused(f"type {kind!r}: no Rust type for it")
    out = base
    for i in stars:
        out = ("*const " if const else "*mut ") + out
        const = i + 1 < len(tokens) and tokens[i + 1] == "const"
    return out


def rust_text(fs, fields_=()):
    """The `raw` layer: every function (a thunk's cdecl, or the function's
    own convention), the imports, the fields' addresses."""
    lines = [f"// The Rust patches' `raw` layer ({GENERATED}, and fields_026g.tsv).",
             "//",
             "// The game's functions as the link gives them: a thunk (cdecl, block",
             "// engine.code) or the function itself in its own convention; kernel32's",
             "// imports (their IAT slots); the address of each field. Unsafe, no logic:",
             "// only `vostok` calls these (rust_patches.md).",
             "",
             "#[allow(unused_imports)]",
             "use core::ffi::c_void;"]

    def signature(f):
        params = ", ".join(f"{'r#' if name in RUST_KEYWORDS else ''}{name}: {rust_type(kind)}"
                           for kind, name in f["params"])
        ret = "" if return_kind(f["ret"]) is None else f" -> {rust_type(f['ret'])}"
        return params, ret
    for abi in ("C", "stdcall", "cdecl", "thiscall"):
        group = [f for f in fs if {"thunk": "C"}.get(f["call"], f["call"]) == abi]
        if group:
            lines += ["", "#[allow(dead_code)]", f'unsafe extern "{abi}" {{']
            for f in group:
                params, ret = signature(f)
                lines += [f"    /// {describe(f)}", f"    pub fn {f['symbol']}({params}){ret};"]
            lines.append("}")
    imports = [f for f in fs if f["call"] == "import"]
    if imports:
        lines += ["", "#[allow(dead_code, non_upper_case_globals)]", 'unsafe extern "C" {']
        for f in imports:
            params, ret = signature(f)
            types = ", ".join(rust_type(kind) for kind, _ in f["params"])
            lines += [f"    /// {describe(f)}: {f['prototype']}",
                      f'    pub static {f["symbol"]}: unsafe extern "stdcall" fn({types}){ret};']
        lines.append("}")
    for module in dict.fromkeys(x["module"] for x in fields_):
        group = [x for x in fields_ if x["module"] == module]
        lines += ["", f"/// {group[0]['kind']}'s fields (fields_026g.tsv).", "#[allow(dead_code)]",
                  f"pub mod {module} {{", "    use core::ffi::c_void;"]
        for x in group:
            lines += [f"", f"    /// +{x['offset']:#x} ({x['type']}): {x['about']}", "    ///",
                      "    /// # Safety", f"    /// `this` is a {x['kind']}.", "    #[inline(always)]",
                      f"    pub unsafe fn {x['field']}(this: *mut c_void) -> *mut {x['type']} {{",
                      f"        unsafe {{ this.byte_add({x['offset']:#x}).cast() }}", "    }"]
        lines.append("}")
    return "\n".join(lines) + "\n"


def assemble(text, va=0x10000000, tools=None):
    """The library of `text` (an engine_026g.asm) linked alone at `va`:
    (va, its bytes, {symbol: VA})."""
    with tempfile.TemporaryDirectory(prefix="srp-engine-") as temporary:
        source = Path(temporary) / "engine_026g.asm"
        source.write_text(text)
        block = linker.ArenaBlock("engine.code", "code", 0x1000, (str(source),))
        linked = linker.link([block], va, 0x100000, tools)
    return linked.va, linker.block_bytes(linked, "engine.code"), linked.symbols


def outputs(spec=None, abi=None, inputs=None):
    """{path: bytes} of the four files."""
    fs = every_function(spec, abi, inputs)
    return {HEADER: header_text(fs).encode(), INCLUDE: include_text(fs).encode(), ASM: asm_text(fs).encode(),
            RUST: rust_text(fs, fields()).encode()}


def stale():
    """The files that are not what the rows give ([] when all are current)."""
    return [path.name for path, content in outputs().items()
            if not path.exists() or path.read_bytes() != content]


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if args.write == args.check:
        parser.error("--write or --check")
    try:
        files = outputs()
    except Refused as error:
        sys.exit(f"engine_026g.tsv: {error}")
    if args.write:
        for path, content in files.items():
            path.write_bytes(content)
        print(f"{ASM.name}: {len(functions())} functions (link_026g.py --write links them into the arena)")
        return
    bad = [path.name for path, content in files.items() if not path.exists() or path.read_bytes() != content]
    print("engine_026g: " + ("current" if not bad else "NOT what the rows give: " + ", ".join(bad)))
    raise SystemExit(1 if bad else 0)


if __name__ == "__main__":
    main()
