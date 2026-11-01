#!/usr/bin/env python3
"""The engine-function library's ABI check (engine_026g.py): every thunk of
the arena's block engine.code (link_026g.py) against the re-kit's rows and
the original exe.

    python check_engine_026g.py <original survarium.exe>

Requires Python `unicorn`, NASM's ndisasm (NASM_DIR or PATH) and the arena's
tools (link_026g.py: gcc -m32, GNU ld, NASM 2.16). The self-test
runs the same (test_engine). For every function of engine_026g.tsv:

- the cleanup: what the real function's `ret`s pop, read from the original exe
  (selftest's function_pops, the call-shape check's reader), is abi.tsv's;
- at the entry: the thunk, called as C calls it (cdecl, a distinct marker per
  argument), reaches the real VA once, with each marker in the register or
  stack slot the rows give it; there the real function's return is played
  with what the exe's `ret` pops, EBX, ESI, EDI and EBP clobbered and a
  result marker in EAX (EDX, XMM0): the thunk must come back to its caller
  with the stack balanced, the four registers kept and the result where C
  takes it (EAX, AL widened, EDX:EAX, ST0);
- every vtables.tsv row: the vtable holds that function at that slot;
- a virtual function (`call` virtual: `[[this]+slot]`): each of its
  vtables.tsv rows is the exe's (the vtable holds that function at that
  slot) and each function's `ret`s pop what abi.tsv says; the thunk, called
  with an object whose vtable is a made-up one, reaches that vtable's slot
  with `this` in ECX and the rest as above, and with an object of each
  listed vtable, the exe's own, reaches that vtable's function;
- for the functions that run on made-up data (the config's child and has,
  the field writers, the statistics' listener, the pool lookup, the
  allocation's name): the real function runs, and
  what it does is what the arguments in the rows' places make it do, so a
  row with two arguments swapped fails here though the thunk follows it.

What it cannot see: a re-kit row whose location is itself wrong for a
function it does not run for real (the thunk and the entry check both follow
the row). Research's functions on its pay path (malloc, the rule base's
constructor, the message, Invoke, register_cook) run for real in
check_research_026g.py, which reads each argument where the game does.

The negative cases (each must be rejected): spec rows with two parameters
in each other's places (engine_026g.py refuses them: the words of the
re-kit's roles do not fit), a cleanup that is no number; libraries made from
wrong re-kit rows (0xF6B10's table and key
exchanged, w<unsigned char> popping 16, value_exists' key in EDI); and the
built library against rows that say otherwise.
"""
import os
import struct
import sys

from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_XMM0

import patch_026g as base
import engine_026g as engine

HERE = os.path.dirname(os.path.abspath(__file__))
REG = {"EAX": UC_X86_REG_EAX, "ECX": UC_X86_REG_ECX, "EDX": UC_X86_REG_EDX, "EBX": UC_X86_REG_EBX,
       "ESI": UC_X86_REG_ESI, "EDI": UC_X86_REG_EDI, "EBP": UC_X86_REG_EBP}
KEPT = {"EBX": 0x0B0B0B0B, "ESI": 0x05150515, "EDI": 0x0D1D0D1D, "EBP": 0x0B9B0B9B}
HEAP, STACK, STUBS, CONFIG = 0x30000000, 0x21000000, 0x22000000, 0x50000000
STOP = STUBS + 0x10
RESULT, RESULT_HIGH = 0x7EADBE2A, 0x13579BDF
RESIZE, DUMP_ON = 0x7AB30, 0x1F1D32C          # a buffer's resize (under the writers); the hash mismatch dump


def marker(i):
    return 0x41000000 + 0x10101 * (i + 1)


def built():
    """The library as the build has it: (its VA, its bytes, {symbol: VA}),
    the arena's block engine.code."""
    linked = base.arena()
    return (linked.blocks["engine.code"].va, base.linker.block_bytes(linked, "engine.code"),
            {name: va for name, va in linked.symbols.items() if name.startswith("engine_")})


class Run:
    """The original exe's sections and the library's code, in Unicorn."""

    def __init__(self, original, library):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(original).sections(original):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(original[raw:raw + min(rsize, vsize or rsize)]))
        va, code, _ = library
        self.lo, self.hi = va, va + len(code)
        size = (va + len(code) + 0xFFF) // 0x1000 * 0x1000 - va // 0x1000 * 0x1000
        emu.mem_map(va // 0x1000 * 0x1000, size, UC_PROT_ALL)
        emu.mem_write(va // 0x1000 * 0x1000, b"\xCC" * size)
        emu.mem_write(va, code)
        for at, size in ((HEAP, 0x100000), (STACK, 0x10000), (STUBS, 0x1000), (CONFIG, 0x10000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        self.heap = HEAP
        self.hooks = {}                               # VA: f(run): what runs there instead
        emu.hook_add(UC_HOOK_CODE, self.intercept)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at, size=4):
        return struct.unpack({1: "<B", 2: "<H", 4: "<I"}[size], self.emu.mem_read(at, size))[0]

    def allocate(self, size, content=b""):
        at = self.heap
        self.heap += (size + 0xF) & ~0xF
        self.emu.mem_write(at, content + bytes(size - len(content)))
        return at

    def text(self, value):
        return self.allocate(len(value) + 1, value.encode() + b"\0")

    def ret(self, cleanup, value=None):
        emu = self.emu
        sp = emu.reg_read(UC_X86_REG_ESP)
        if value is not None:
            emu.reg_write(UC_X86_REG_EAX, value)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + cleanup)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def intercept(self, emu, address, size, user):
        if address in self.hooks:
            self.hooks[address](self)

    def cdecl(self, entry, args, regs=None):
        """entry(args) as C calls it, back at STOP: (eax, esp after - esp
        before the call, the four kept registers as they came back)."""
        emu = self.emu
        sp = STACK + 0x8000 - 4 * len(args) - 4
        self.word(sp, STOP)
        for i, value in enumerate(args):
            self.word(sp + 4 + 4 * i, value)
        for name, value in KEPT.items():
            emu.reg_write(REG[name], value)
        for name, value in {"EAX": 0xEAEAEAEA, "ECX": 0xECECECEC, "EDX": 0xEDEDEDED, **(regs or {})}.items():
            emu.reg_write(REG[name], value)
        emu.reg_write(UC_X86_REG_ESP, sp)
        emu.emu_start(entry, STOP, count=2_000_000)
        return (emu.reg_read(UC_X86_REG_EAX), emu.reg_read(UC_X86_REG_ESP) - sp,
                {name: emu.reg_read(REG[name]) for name in KEPT})


def st0(run):
    """ST0 popped as a float (an `fstp dword` run after the thunk's return)."""
    at = STUBS + 0x300
    run.emu.mem_write(at, b"\xD9\x1D" + struct.pack("<I", STUBS + 0x400))
    run.emu.emu_start(at, at + 6)
    return struct.unpack("<f", run.emu.mem_read(STUBS + 0x400, 4))[0]


def exe_pops(original, va, cache={}):
    """What the real function's rets pop, from the original exe (the
    self-test's function_pops): the value, or None when they differ, none was
    found or a branch could not be followed."""
    if va not in cache:
        import selftest_026g as selftest
        image = cache.setdefault("image", selftest.Image(original))
        found, lost = selftest.function_pops(image, va)
        cache[va] = found.pop() if len(found) == 1 and not lost else None
    return cache[va]


def at_entry(run, f, entry, pops, target=None, this=None):
    """The thunk at `entry` called with markers, stopped at f's VA (or
    `target`: a virtual function's, `this` its object, the first
    argument), the return played with `pops`; [(ok, text)]."""
    emu, seen = run.emu, []
    target = f["va"] if target is None else target
    given = [this if this is not None and i == 0 else marker(i) for i in range(len(f["params"]))]

    def real(run):
        sp = emu.reg_read(UC_X86_REG_ESP)
        seen.append(({name: emu.reg_read(r) for name, r in REG.items()},
                     [run.read(sp + 4 * k) for k in range(len(f["args"]) + 1)]))
        for name in KEPT:
            emu.reg_write(REG[name], 0xBAD00000 + len(name))
        emu.reg_write(UC_X86_REG_EDX, RESULT_HIGH)
        emu.reg_write(UC_X86_REG_XMM0, struct.unpack("<I", struct.pack("<f", 1.5))[0] | 0xFFFF << 64)
        run.ret(pops, RESULT)
    run.hooks[target] = real
    out = []
    try:
        eax, moved, kept = run.cdecl(entry, given)
    except UcError as error:
        return [(False, f"{f['symbol']}: the thunk faulted ({error})")]
    finally:
        del run.hooks[target]
    if len(seen) != 1:
        return [(False, f"{f['symbol']}: reached {target:#x} {len(seen)} times, not once")]
    regs, stack = seen[0]
    where = []
    for i, location in enumerate(f["args"]):
        got = regs[location] if location in REG else stack[int(location[4:]) // 4]
        if got != given[i]:
            where.append(f"{f['params'][i][1]} not in {location} ({got:#x})")
    out.append((not where and run.lo <= stack[0] < run.hi,
                f"{f['symbol']}: at {target:#x} each argument where the rows put it "
                f"({', '.join(f['args']) or 'none'}), called from the library" + (": " + "; ".join(where) if where else "")))
    result = {None: True, "EAX": eax == RESULT, "AL": eax == RESULT & 0xFF,
              "EDX:EAX": eax == RESULT and run.emu.reg_read(UC_X86_REG_EDX) == RESULT_HIGH,
              "XMM0": f["result"] == "XMM0" and st0(run) == 1.5}[f["result"]]
    out.append((moved == 4 and kept == KEPT and result,
                f"{f['symbol']}: back with the stack balanced (esp +{moved}, the return address only), EBX ESI EDI "
                f"EBP kept, the result {f['result'] or '(none)'} where C takes it (eax {eax:#x})"))
    return out


def virtual(run, original, f, entry):
    """[(ok, text)] of a virtual function: its vtables.tsv rows against the
    exe, each body's cleanup, the thunk through a made-up vtable and through
    each listed one."""
    base_class, slot, found = f["virtual"]
    out = []
    for vtable, va, cls in found:
        held = run.read(vtable + slot)
        pops = exe_pops(original, va)
        out.append((held == va and pops == f["cleanup"],
                    f"{f['symbol']}: {cls}'s vtable {vtable:#x}+{slot:#x} holds {held:#x} in the original exe "
                    f"(vtables.tsv: {va:#x}), whose rets pop {pops} (abi.tsv: {f['cleanup']})"))
    table, this = run.allocate(0x400), run.allocate(0x10)
    stub = STUBS + 0x200
    run.word(this, table)
    run.word(table + slot, stub)
    out += at_entry(run, f, entry, f["cleanup"], target=stub, this=this)
    for vtable, va, cls in found:
        run.word(this, vtable)
        reached = at_entry(run, f, entry, f["cleanup"], target=va, this=this)
        out.append((all(ok for ok, _ in reached), f"{f['symbol']}: an object of {cls} (vtable {vtable:#x}) reaches "
                    f"{va:#x}" + "".join(f"; {text}" for ok, text in reached if not ok)))
    return out


# ---- the functions run for real ------------------------------------------

def config(run):
    import check_research_026g as research
    run.emu.mem_write(CONFIG, research.loaded(research.config_bytes(), CONFIG))
    return CONFIG


def name_of(run, value):
    out, at = bytearray(), run.read(value + 8)
    while at and len(out) < 64 and run.read(at + len(out), 1):
        out.append(run.read(at + len(out), 1))
    return out.decode(errors="replace")


def behaviour(run, entries):
    """[(symbol, ok, text)] of the functions run on made-up data."""
    out = []

    def call(symbol, *args):
        try:
            return run.cdecl(entries[symbol], list(args))
        except UcError as error:
            return (None, None, str(error))
    if "engine_cfg_child" in entries and "engine_cfg_has" in entries:
        root = config(run)
        mode, _, _ = call("engine_cfg_child", root, run.text("mode"))
        inner, _, _ = call("engine_cfg_child", mode, run.text("mode")) if mode else (None, 0, 0)
        out.append(("engine_cfg_child", mode is not None and name_of(run, mode) == "mode"
                    and inner is not None and run.read(inner) == 5,
                    "the config's child: root[\"mode\"] is the value named mode, its mode 5"))
        yes, _, _ = call("engine_cfg_has", root, run.text("mode"))
        no, _, _ = call("engine_cfg_has", root, run.text("nope"))
        out.append(("engine_cfg_has", yes == 1 and no == 0, f"the config's has: mode {yes}, nope {no}"))

    def resize(run):
        emu, sp = run.emu, run.emu.reg_read(UC_X86_REG_ESP)
        buffer, size = emu.reg_read(UC_X86_REG_EAX), run.read(sp + 4)
        if size > run.read(buffer + 8):
            fresh = run.allocate(size + 0x100)
            emu.mem_write(fresh, bytes(emu.mem_read(run.read(buffer + 4), run.read(buffer + 0xC))))
            run.word(buffer + 4, fresh)
            run.word(buffer + 8, size + 0x100)
        run.word(buffer + 0xC, size)
        run.ret(4, 0)
    run.hooks[RESIZE] = resize
    run.emu.mem_write(DUMP_ON, b"\0\0")
    for symbol, size, value in (("engine_write_u8", 1, 0xA5), ("engine_write_u16", 2, 0xBEEF),
                                ("engine_write_u32", 4, 0x12345678)):
        if symbol not in entries:
            continue
        writer, buffer = run.allocate(0x20), run.allocate(0x10)
        run.word(writer + 0x14, buffer)
        source = run.allocate(4, struct.pack("<I", value))
        _, moved, _ = call(symbol, source, writer, run.text("file.c"), 7, run.text("fn"), run.text("field"))
        size_now = run.read(buffer + 0xC)
        got = bytes(run.emu.mem_read(run.read(buffer + 4), size_now)) if size_now else b""
        out.append((symbol, moved == 4 and got == struct.pack("<I", value)[:size],
                    f"{symbol}: appends the value's {size} bytes to the writer's buffer ({got.hex() or 'nothing'})"))
    del run.hooks[RESIZE]
    if "engine_statistics_notify" in entries:
        listener, table, called = run.allocate(0x20), run.allocate(8), []
        run.word(listener, table)
        run.word(table + 4, STUBS + 0x100)

        def invoker(run):
            sp = run.emu.reg_read(UC_X86_REG_ESP)
            called.append(tuple(run.read(sp + 4 * k) for k in range(1, 5)))
            run.ret(0)
        run.hooks[STUBS + 0x100] = invoker
        _, moved, _ = call("engine_statistics_notify", listener, 3, 6, 100)
        del run.hooks[STUBS + 0x100]
        out.append(("engine_statistics_notify", moved == 4 and called == [(listener + 8, 3, 6, 100)],
                    f"the statistics' listener: its invoker gets (functor, place 3, event 6, amount 100) {called}"))
    if "engine_find_pool" in entries:
        pools, first, second = run.allocate(0x10), run.allocate(0x90), run.allocate(0x90)
        run.word(pools + 8, first)
        run.word(first, second)
        run.word(first + 0x80, run.text("health"))
        run.word(second + 0x80, run.text("stamina"))
        name = run.allocate(4)
        run.word(name, run.text("stamina"))
        got, moved, _ = call("engine_find_pool", pools, name)
        out.append(("engine_find_pool", moved == 4 and got == second,
                    f"the pool lookup: the second pool by its name ({got and hex(got)})"))
    if "engine_allocation_name" in entries:
        got, moved, _ = call("engine_allocation_name", 0xAC1220)
        out.append(("engine_allocation_name", moved == 4 and got == 0xAC1228, f"the allocation's name: {got and hex(got)}"))
    return out


def run_checks(original, library=None, spec=None, abi=None, inputs=None, behave=True, vtables=None):
    """[(ok, text)] of the library (its VA, bytes and symbols; the build's
    when None) against the rows (engine_026g.tsv, the re-kit's) and the
    original exe."""
    library = built() if library is None else library
    va, code, symbols = library
    fs = engine.functions(spec, abi, inputs, vtables)
    out = [(all(va <= symbols.get(f["symbol"], -1) < va + len(code) for f in fs),
            f"each of the library's {len(fs)} functions is a symbol of its code")]
    if not out[0][0]:
        return out
    run = Run(original, library)
    rows = engine.read_tsv(engine.REKIT / "vtables.tsv") if vtables is None else vtables
    wrong = [f"{row['class']} {row['vtable']}+{row['slot']}: {run.read(engine.number(row['vtable']) + engine.number(row['slot'])):#x}"
             for row in rows if run.read(engine.number(row["vtable"]) + engine.number(row["slot"])) != engine.number(row["va"])]
    out.append((not wrong, f"every vtables.tsv row ({len(rows)}) is the original exe's: the vtable holds the function "
                "at the slot" + "".join(f"; not {w}" for w in wrong)))
    for f in fs:
        if f["call"] == "virtual":
            out += virtual(run, original, f, symbols[f["symbol"]])
            continue
        pops = exe_pops(original, f["va"])
        out.append((pops == f["cleanup"], f"{f['symbol']}: {f['va']:#x}'s rets pop {pops} in the original exe; "
                    f"abi.tsv says {f['cleanup']}"))
        out += at_entry(run, f, symbols[f["symbol"]], f["cleanup"] if pops is None else pops)
    if behave:
        out += [(ok, text) for _, ok, text in behaviour(run, symbols)]
    return out


def negative(original, tools=None):
    """[(what, it was rejected)] of wrong spec rows (the generator must refuse
    them), wrong re-kit rows (the check must fail on the library made from
    them) and a wrong expectation (the check must fail on the built one)."""
    spec, abi, inputs, vtables = (engine.read_tsv(path) for path in (
        engine.SPEC, engine.REKIT / "abi.tsv", engine.REKIT / "abi_inputs.tsv", engine.REKIT / "vtables.tsv"))

    def library(spec=spec, abi=abi, inputs=inputs):
        return engine.assemble(engine.asm_text(engine.functions(spec, abi, inputs)), tools=tools)

    def failed(results):
        return any(not ok for ok, _ in results)

    def refused(spec=spec, abi=abi, inputs=inputs, vtables=vtables):
        try:
            engine.functions(spec, abi, inputs, vtables)
        except engine.Refused:
            return True
        return False

    def arguments(symbol, text):
        return [dict(row, arguments=text) if row["symbol"] == symbol else row for row in spec]

    def swapped(symbol, a, b):
        """The spec with the locations of two of `symbol`'s parameters
        exchanged, each keeping its word: two parameters given each other's
        registers or slots."""
        row = next(r for r in spec if r["symbol"] == symbol)
        tokens = [t.partition(":") for t in row["arguments"].split()]
        flip = {a: b, b: a}
        return arguments(symbol, " ".join(flip.get(at, at) + ":" + word for at, _, word in tokens))

    def located(va, moves):
        """abi_inputs with `va`'s rows moved to other locations ({old: new})."""
        return [dict(row, location=moves[row["location"]]) if int(row["va"], 16) == va and row["location"] in moves
                else row for row in inputs]
    out = []
    # A spec row whose parameters take each other's places: the generator
    # refuses it (the words of the re-kit's roles do not fit).
    for symbol, a, b in (("engine_cfg_child", "EAX", "EDI"), ("engine_malloc", "ESI", "EAX"),
                         ("engine_match_rule_base", "EAX", "ESP+4"), ("engine_show_message", "ESP+12", "ESP+16"),
                         ("engine_movie_invoke", "ESP+4", "ESP+8"), ("engine_free", "ECX", "EAX"),
                         ("engine_profile_assign", "ESI", "EDI")):
        out.append((f"{symbol} with its {a} and {b} parameters swapped in engine_026g.tsv", refused(swapped(symbol, a, b))))
    # A virtual row at another slot (the emitter's +1Ch, destroy: one stack
    # argument) and one implementation's cleanup otherwise: refused.
    out.append(("engine_emitter_emit at the emitter's slot +0x1c in engine_026g.tsv",
                refused([dict(row, va="survarium::pure_game_effect_emitter+0x1c") if row["symbol"] == "engine_emitter_emit"
                         else row for row in spec])))
    out.append(("a virtual row of a class with no vtables.tsv row (survarium::game_effect+0x20)",
                refused([dict(row, va="survarium::game_effect+0x20") if row["symbol"] == "engine_emitter_emit"
                         else row for row in spec])))
    out.append(("one emit (0x333010, reinforcement storm's) popping 8 in abi.tsv",
                refused(abi=[dict(row, ret_cleanup="8") if int(row["va"], 16) == 0x333010 else row for row in abi])))
    # A vtables.tsv row that is not the exe's: the hud's +20h said to be the
    # sound emitter's emit. The generator follows it; the exe does not.
    wrong_vtables = [dict(row, va="0x0029a580") if (row["vtable"], row["slot"]) == ("0x00a0bc7c", "0x20") else row
                     for row in vtables]
    out.append(("a vtables.tsv row saying the hud emitter's +0x20 is 0x29a580 (the sound emitter's)",
                failed(run_checks(original, None, spec, abi, inputs, behave=False, vtables=wrong_vtables))))
    out.append(("an abi.tsv cleanup that is no number of bytes (0x10)",
                refused(abi=[dict(row, ret_cleanup="0x10") if int(row["va"], 16) == 0x1F3DE0 else row
                             for row in abi])))
    # A wrong re-kit row the generator cannot see: 0xF6B10's table and key
    # rows in each other's registers, the spec following them. The thunk and
    # the entry check agree with the rows; the real lookup does not.
    wrong = located(0xF6B10, {"EAX": "EDI", "EDI": "EAX"})
    follows = arguments("engine_cfg_child", "EDI:table EAX:key")
    out.append(("a re-kit row with 0xF6B10's table and key registers exchanged (the spec following it)",
                failed(run_checks(original, library(follows, abi, wrong), follows, abi, wrong))))
    # w<unsigned char> popping 16: abi.tsv against the exe's `ret 14h`.
    popping = [dict(row, ret_cleanup="16") if int(row["va"], 16) == 0x1F3DE0 else row for row in abi]
    out.append(("w<unsigned char> (0x1F3DE0) popping 16 in abi.tsv",
                failed(run_checks(original, library(abi=popping), spec, popping, inputs, behave=False))))
    # value_exists' key in EDI: the row and the spec both say EDI.
    edi_inputs = located(0xF6C00, {"ESI": "EDI"})
    edi_spec = arguments("engine_cfg_has", "EAX:table EDI:key")
    out.append(("value_exists (0xF6C00) with its key in EDI in abi_inputs.tsv",
                failed(run_checks(original, library(edi_spec, abi, edi_inputs), edi_spec, abi, edi_inputs))))
    # The checked-in thunks against rows that say otherwise: the entry check
    # sees it.
    moved = located(0xE4F90, {"ESI": "EAX", "EAX": "ESI"})
    # Every row of malloc's VA follows the moved rows (engine_allocate_recipe is a typed one).
    follows_malloc = [dict(row, arguments="EAX:allocator ESI:size ESP+4:description")
                      if row["va"].lower() in ("0xe4f90", "0x0e4f90") else row for row in spec]
    out.append(("the built library against re-kit rows with malloc's allocator in EAX and size in ESI",
                failed(run_checks(original, None, follows_malloc, abi, moved, behave=False))))
    return out


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    bad = 0
    for ok, text in run_checks(original):
        print(("  ok    " if ok else "  FAIL  ") + text)
        bad += not ok
    print("negative: wrong rows and libraries the check must reject")
    for what, failed in negative(original):
        print(("  ok    " if failed else "  FAIL  ") + f"rejected: {what}")
        bad += not failed
    print(f"{bad} FAILED" if bad else "all cases passed")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
