#!/usr/bin/env python3
"""Exercise every generated hook-entry kind against the original exe
(docs/notes/rust_patches.md). For each kind a real install is the fixture:
the generated NASM entry (its alignment, trampoline, register and FPU
save/restore, calling convention and cleanup) is assembled, wrapped around
the fixture's real game function, and run in Unicorn beside the exe; the
entry must leave the same observable state -- the result, the callee-saved
registers (EBX, ESI, EDI, EBP) and the stack pointer, and the memory the
function touched -- as calling the game's function directly. The shim's
typed-argument path is the one check_suicide_key_026g.py proves for a `call`
hook against the exe; here the entry template is what is under test, with a
stub shim that forwards to the game's own function.

    python check_hooks_026g.py <original survarium.exe>

Requires Python `unicorn`, NASM (the dev shell) and the entry macros
(rust_hook_026g.inc).
"""
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, UC_X86_REG_ESP,
                               UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EFLAGS, UC_X86_REG_EIP)

import patch_026g as base
import hooks_026g

HERE = Path(__file__).resolve().parent
TEST_VA = 0xB000000                 # where the assembled entry + stub is mapped
STACK = 0x40000000
GP = {"eax": UC_X86_REG_EAX, "ecx": UC_X86_REG_ECX, "edx": UC_X86_REG_EDX, "ebx": UC_X86_REG_EBX,
      "ebp": UC_X86_REG_EBP, "esi": UC_X86_REG_ESI, "edi": UC_X86_REG_EDI}
SAVED = ("ebx", "esi", "edi", "ebp")
SENTINELS = {"ebx": 0x1B1B1B1B, "esi": 0x51515151, "edi": 0xD1D1D1D1, "ebp": 0xB9B9B9B9}

passed = failed = 0
RESULTS = []            # (ok, text) of every assertion, for selftest_026g.py
QUIET = False


def check(ok, what):
    global passed, failed
    RESULTS.append((bool(ok), what))
    if not QUIET:
        print(("  ok  " if ok else "  FAIL") + "  " + what)
    passed += ok
    failed += not ok


def run_checks(original):
    """Every kind's assertions as (ok, text), for the self-test (quiet: the
    caller prints)."""
    global ORIGINAL, QUIET
    ORIGINAL = original
    QUIET = True
    RESULTS.clear()
    for fn in (test_pointer, test_callback, test_vtable, test_splice, test_code, test_lohook, test_entry):
        fn()
    return list(RESULTS)


def assemble(entry_src, stub_src):
    """The entry (NASM, the macros of rust_hook_026g.inc, `extern stub`) and
    the stub (`global stub`) assembled as ELF32 and linked at TEST_VA (the
    entry's object first): the linked `.text` bytes and {symbol: VA}."""
    with tempfile.TemporaryDirectory() as folder:
        f = lambda n: str(Path(folder) / n)
        Path(f("entry.asm")).write_text('bits 32\n%include "rust_hook_026g.inc"\nsection .text\n' + entry_src)
        Path(f("stub.asm")).write_text("bits 32\nsection .text\n" + stub_src)
        for name in ("entry", "stub"):
            subprocess.run(["nasm", "-f", "elf32", "-I", str(HERE) + "/", f(name + ".asm"), "-o", f(name + ".o")],
                           check=True)
        subprocess.run(["ld", "-m", "elf_i386", "-Ttext", hex(TEST_VA), "-e", "entry", "--no-warn-rwx-segments",
                        "-o", f("t.elf"), f("entry.o"), f("stub.o")], check=True, capture_output=True)
        subprocess.run(["objcopy", "-O", "binary", "-j", ".text", f("t.elf"), f("t.bin")], check=True)
        code = Path(f("t.bin")).read_bytes()
        labels = {}
        for line in subprocess.run(["nm", f("t.elf")], capture_output=True, text=True).stdout.splitlines():
            parts = line.split()
            if len(parts) == 3:
                labels[parts[2]] = int(parts[0], 16)
        return code, labels


class Machine:
    def __init__(self, data, blob=b"", blob_va=TEST_VA):
        self.uc = uc = Uc(UC_ARCH_X86, UC_MODE_32)
        headers = base.Headers(data)
        size = struct.unpack_from("<I", data, headers.size_of_image_at)[0]
        uc.mem_map(base.IMAGE_BASE, (size + 0xFFF) & ~0xFFF, UC_PROT_ALL)
        uc.mem_write(base.IMAGE_BASE, bytes(data[:headers.size_of_headers]))
        for _, rva, vsize, raw, rsize, _ in headers.sections(data):
            uc.mem_write(base.IMAGE_BASE + rva, bytes(data[raw:raw + rsize]))
        if blob:
            uc.mem_map(blob_va & ~0xFFF, ((len(blob) + (blob_va & 0xFFF)) + 0xFFF) & ~0xFFF | 0x1000, UC_PROT_ALL)
            uc.mem_write(blob_va, blob)
        uc.mem_map(STACK, 0x100000, UC_PROT_ALL)

    def run(self, va, ecx=0, args=(), n_pop_by_callee=None):
        uc = self.uc
        for name, reg in GP.items():
            uc.reg_write(reg, SENTINELS.get(name, 0))
        uc.reg_write(UC_X86_REG_EFLAGS, 0x202)
        esp = STACK + 0x80000
        for a in reversed(args):
            esp -= 4
            uc.mem_write(esp, struct.pack("<I", a & 0xFFFFFFFF))
        esp -= 4
        uc.mem_write(esp, struct.pack("<I", 0xDEAD0000))
        uc.reg_write(UC_X86_REG_ESP, esp)
        entry_esp = esp
        uc.reg_write(UC_X86_REG_ECX, ecx & 0xFFFFFFFF)
        uc.emu_start(va, 0xDEAD0000, count=500000)
        after = uc.reg_read(UC_X86_REG_ESP)
        return {"eax": uc.reg_read(UC_X86_REG_EAX), "edx": uc.reg_read(UC_X86_REG_EDX),
                **{r: uc.reg_read(GP[r]) for r in SAVED},
                "popped": after - (entry_esp + 4)}      # bytes cleaned beyond the return address


SCRATCH = 0xB800000                 # the recording stub writes here


def convention_entry(name, convention, nargs):
    """A convention entry (pointer, vtable slot, callback) of `nargs` dwords
    is transparent: it hands the shim ECX (thiscall) and the arguments in
    order, passes the shim's result back, keeps EBX/ESI/EDI/EBP, and cleans
    the game's arguments (thiscall) or none (cdecl). The stub shim stands for
    the real one, recording what it got and returning a sentinel."""
    macro = {"thiscall": "RUST_THISCALL", "cdecl": "RUST_CDECL"}[convention]
    thiscall = convention == "thiscall"
    total = nargs + (1 if thiscall else 0)         # the shim's cdecl arguments
    # record each cdecl argument [ebp+8+4i] at SCRATCH+4i, return 0xCAFED00D
    shim = ["global stub", "stub:", "    push ebp", "    mov ebp, esp", "    push esi",
            f"    mov esi, {SCRATCH:#x}"]
    for i in range(total):
        shim += [f"    mov eax, [ebp + {8 + 4 * i}]", f"    mov [esi + {4 * i}], eax"]
    shim += ["    pop esi", "    mov eax, 0xCAFED00D", "    leave", "    ret"]
    blob, labels = assemble(f"{macro} entry, stub, {nargs}\n", "\n".join(shim) + "\n")
    uc = Machine(bytes(ORIGINAL), blob, TEST_VA)
    uc.uc.mem_map(SCRATCH & ~0xFFF, 0x1000, UC_PROT_ALL)
    ecx = 0xEC000000 if thiscall else SENTINELS["ebx"]  # unused for cdecl (ecx is volatile)
    stack = tuple(0xA0000000 + i for i in range(nargs))
    out = uc.run(labels["entry"], ecx=ecx, args=stack)
    seen = struct.unpack_from(f"<{total}I", uc.uc.mem_read(SCRATCH, 4 * total))
    want = ((ecx,) + stack) if thiscall else stack
    ok = (seen == want and out["eax"] == 0xCAFED00D and all(out[r] == SENTINELS[r] for r in SAVED)
          and out["popped"] == (4 * nargs if thiscall else 0))
    check(ok, f"{name}: {macro} of {nargs} dwords hands the shim "
          f"{'ecx + ' if thiscall else ''}the arguments in order, returns its result, keeps the callee-saved "
          f"registers and cleans {out['popped']} bytes")


def test_pointer():
    # patch 11's SLOT: the replay client's "is this place mine" (vtable
    # 0x9FF344+54h, model 0x54E10), thiscall with one dword, AL, `ret 4`.
    convention_entry("pointer (replay is_place_mine's slot)", "thiscall", 1)
    # and a real leaf run through the entry: 0x54E10 is `xor al,al / ret 4`,
    # so the entry wrapping it returns AL 0, as calling it directly does.
    shim = ["global stub", "stub:", "    mov ecx, [esp+4]", "    push dword [esp+8]",
            "    mov eax, 0x54e10", "    call eax", "    ret"]
    blob, labels = assemble("RUST_THISCALL entry, stub, 1\n", "\n".join(shim) + "\n")
    direct = Machine(bytes(ORIGINAL)).run(0x54E10, ecx=0x200000, args=(7,))
    hooked = Machine(bytes(ORIGINAL), blob, TEST_VA).run(labels["entry"], ecx=0x200000, args=(7,))
    check((hooked["eax"] & 0xFF) == (direct["eax"] & 0xFF) == 0 and all(hooked[r] == SENTINELS[r] for r in SAVED)
          and hooked["popped"] == 4,
          "pointer (is_place_mine run through the entry): AL 0 as the direct call, callee-saved kept, cleaned 4")


def test_callback():
    # patch 44's bake_done model (cdecl, two dwords); the game cleans them.
    convention_entry("callback (bake_done, cdecl 2)", "cdecl", 2)


def test_vtable():
    # a Research rule vtable slot the game calls with its arguments on the
    # stack; the entry is the slot's. One and two dwords.
    convention_entry("vtable slot (1 dword)", "thiscall", 1)
    convention_entry("vtable slot (2 dwords)", "thiscall", 2)


def test_splice():
    # a splice over a cdecl function with a 5-byte branch-free prologue:
    # the start jumps to our entry; the handler calls the Original (the
    # trampoline, the relocated prologue then a jump to the rest), so the
    # whole runs as the unhooked function. The stub shim stands for the real
    # one (it calls the trampoline). orig: `mov eax,0x11110000` (the prologue)
    # then `add eax,[esp+4]; ret` (the rest).
    entry_src = ("extern orig_rest\n"
                 "RUST_CDECL entry, stub, 1\n"
                 "global entry_tramp\nentry_tramp:\n"
                 "    db 0xb8, 0x00, 0x00, 0x11, 0x11\n"      # mov eax, 0x11110000 (relocated prologue)
                 "    jmp orig_rest\n")
    stub_src = ("extern entry_tramp\nglobal stub\nstub:\n"
                "    push dword [esp+4]\n    call entry_tramp\n    add esp, 4\n    ret\n"
                "global orig\norig:\n    mov eax, 0x11110000\n"
                "global orig_rest\norig_rest:\n    add eax, [esp+4]\n    ret\n")
    blob, labels = assemble(entry_src, stub_src)
    m = Machine(bytes(ORIGINAL), blob, TEST_VA)
    through = m.run(labels["entry"], args=(0x42,))
    direct = Machine(bytes(ORIGINAL), blob, TEST_VA).run(labels["orig"], args=(0x42,))
    ok = (through["eax"] == direct["eax"] == 0x11110042 and all(through[r] == SENTINELS[r] for r in SAVED)
          and through["popped"] == 0)
    check(ok, f"splice: the entry and trampoline run the function whole (eax {through['eax']:#x}), callee-saved "
          "kept, caller cleans (cdecl)")
    # the generator relocates a real branch-free prologue (network_client's
    # destructor 0x219E60: push ebp / mov ebp,esp / and esp,-8 / sub esp,0x28),
    # read from the exe by the site's span; a branchy span is refused.
    import hooks_026g
    patch = {"name": "fix", "patch": 0}
    row = {"site": "dtor.start", "handler": "h"}
    fns = [{"symbol": "engine_dtor", "va": 0x219E60, "args": [], "params": [], "ret": "unsigned int",
            "cleanup": 0, "result": "EAX", "name": "dtor", "prototype": "", "call": "cdecl",
            "vostok": {"args": [], "result": "u32"}}]
    sites = {"dtor.start": {"site": "dtor.start", "va": 0x219E60, "kind": "splice", "callee": "engine_dtor",
                            "span": 9, "exits": [], "what": "the destructor's start"}}
    good = lambda va, n: bytes.fromhex("558bec83e4f883ec28")[:n]
    branchy = lambda va, n: bytes.fromhex("eb0090909090")[:n]
    try:
        hooks_026g.splice_hook(patch, row, sites, fns, good)
        check(True, "splice: a real branch-free prologue (0x219E60's) is read from the exe and relocated")
    except hooks_026g.Bad as e:
        check(False, f"splice prologue rejected: {e}")
    try:
        hooks_026g.splice_hook(patch, row, sites, fns, branchy)
        check(False, "splice must refuse a prologue with a branch")
    except hooks_026g.Bad:
        check(True, "splice refuses a prologue with a relative branch (cannot relocate)")


def test_code():
    # a code hook (the escape hatch): a hand-written cave the site jumps to,
    # which does its work and jumps back. Here it adds a constant and returns,
    # as the site's `add eax,0x11111111 / ret` would.
    cave = ("bits 32\nsection .text\nglobal cave\ncave:\n    add eax, 0x11111111\n    jmp done\n"
            "global site\nsite:\n    jmp cave\nglobal done\ndone:\n    ret\n"
            "global orig_site\norig_site:\n    add eax, 0x11111111\n    ret\n")
    with tempfile.TemporaryDirectory() as folder:
        f = lambda n: str(Path(folder) / n)
        Path(f("c.asm")).write_text(cave)
        subprocess.run(["nasm", "-f", "elf32", f("c.asm"), "-o", f("c.o")], check=True)
        subprocess.run(["ld", "-m", "elf_i386", "-Ttext", hex(TEST_VA), "-e", "cave", "--no-warn-rwx-segments",
                        "-o", f("c.elf"), f("c.o")], check=True, capture_output=True)
        subprocess.run(["objcopy", "-O", "binary", "-j", ".text", f("c.elf"), f("c.bin")], check=True)
        blob = Path(f("c.bin")).read_bytes()
        labels = {}
        for line in subprocess.run(["nm", f("c.elf")], capture_output=True, text=True).stdout.splitlines():
            p = line.split()
            if len(p) == 3:
                labels[p[2]] = int(p[0], 16)
    m = Machine(bytes(ORIGINAL), blob, TEST_VA)
    m.uc.reg_write(UC_X86_REG_EAX, 0)
    through = m.run(labels["site"])
    direct = Machine(bytes(ORIGINAL), blob, TEST_VA)
    direct.uc.reg_write(UC_X86_REG_EAX, 0)
    base_run = direct.run(labels["orig_site"])
    check(through["eax"] == base_run["eax"] and all(through[r] == SENTINELS[r] for r in SAVED),
          f"code: the cave the site jumps to runs and returns as the replaced bytes would (eax {through['eax']:#x})")
    # the generator installs a jump to the cave and links its .asm source;
    # the site gives the VA and the span (read from the exe), the patch the
    # author's source/symbol.
    import hooks_026g
    toml = [{"name": "fix", "patch": 0, "hook": [{"site": "x.site", "source": "x.asm", "symbol": "cave"}]}]
    sites = {"x.site": {"site": "x.site", "va": 0x100000, "kind": "code", "callee": "-", "span": 5,
                        "exits": [], "what": "a cave"}}
    exe = lambda va, n: b"\x90" * n
    hs = hooks_026g.hooks(toml, [], sites, exe)
    ok = (hs[0]["install"] == ("code", 0xE9)
          and "x.asm" in hooks_026g.arena_sources(toml, [], sites, exe))
    check(ok, "code: the generator installs a jump to the cave and links its .asm source")


LOHOOK_PAST = 0xB900000         # the lohook span's reconvergence (site + len), mapped


def run_lohook(h, stub_src, until, regs=None, flags=0x202):
    """Assemble a lohook entry (hooks_026g.lohook_entry) with a stub shim and
    run it from the entry to `until`, returning the machine state."""
    entry_src = "\n".join(hooks_026g.lohook_entry(h)) + "\n"
    blob, labels = assemble(entry_src, stub_src)
    m = Machine(bytes(ORIGINAL), blob, TEST_VA)
    m.uc.mem_map(LOHOOK_PAST & ~0xFFF, 0x1000, UC_PROT_ALL)
    uc = m.uc
    for name, reg in GP.items():
        uc.reg_write(reg, (regs or {}).get(name, SENTINELS.get(name, 0)))
    uc.reg_write(UC_X86_REG_EFLAGS, flags)
    esp = STACK + 0x80000
    uc.reg_write(UC_X86_REG_ESP, esp)
    uc.emu_start(labels[h["entry"]], until, count=200000)
    return {**{r: uc.reg_read(GP[r]) for r in GP}, "esp": uc.reg_read(UC_X86_REG_ESP),
            "eip": uc.reg_read(UC_X86_REG_EIP), "eflags": uc.reg_read(UC_X86_REG_EFLAGS)}


def test_lohook():
    span = bytes.fromhex("b834120000")                 # mov eax, 0x1234 (5 bytes, branch-free)
    site = LOHOOK_PAST - len(span)
    nop = b"\x90" * 5
    base = dict(patch="fix", patch_no=0, kind="lohook", site=site, handler="h",
                entry="fix_h", shim="hook_fix_h", what="x", exits=[], install=("lohook", 0xE9))
    noop_stub = "global hook_fix_h\nhook_fix_h:\n    xor eax, eax\n    ret\n"      # Continue, nothing changed
    # Continue replays the span: eax becomes 0x1234, the callee-saved kept
    out = run_lohook(dict(base, original=span), noop_stub, LOHOOK_PAST)
    check(out["eip"] == LOHOOK_PAST and out["eax"] == 0x1234 and all(out[r] == SENTINELS[r] for r in SAVED),
          f"lohook Continue: the span replays (eax {out['eax']:#x}) and reaches the span's end, callee-saved kept")
    # a handler's register write (ecx, offset 24 in Regs) is applied; a nop
    # span so the replay does not overwrite it
    set_ecx = ("global hook_fix_h\nhook_fix_h:\n    mov eax, [esp+4]\n    mov eax, [eax]\n"
               "    mov dword [eax+24], 0xc0ffee\n    xor eax, eax\n    ret\n")
    out = run_lohook(dict(base, original=nop), set_ecx, LOHOOK_PAST)
    check(out["eip"] == LOHOOK_PAST and out["ecx"] == 0xC0FFEE and all(out[r] == SENTINELS[r] for r in SAVED),
          f"lohook Continue: a Context register write is applied (ecx {out['ecx']:#x})")
    # Skip does not replay: eax stays as it was
    skip_stub = "global hook_fix_h\nhook_fix_h:\n    mov eax, 1\n    ret\n"
    out = run_lohook(dict(base, original=span), skip_stub, LOHOOK_PAST, regs={"eax": 0x999})
    check(out["eip"] == LOHOOK_PAST and out["eax"] == 0x999,
          f"lohook Skip: the span is not replayed (eax {out['eax']:#x} unchanged)")
    # Exit jumps to the named address
    exit_at = LOHOOK_PAST + 0x40
    exit_stub = "global hook_fix_h\nhook_fix_h:\n    mov eax, 2\n    ret\n"
    out = run_lohook(dict(base, original=span, exits=[("there", exit_at)]), exit_stub, exit_at)
    check(out["eip"] == exit_at and out["eax"] != 0x1234,
          f"lohook Exit: control jumps to the named exit {exit_at:#x} without replaying")
    # A no-op handler must preserve incoming arithmetic flags on every
    # dispatch path, including when the replayed span does not set them.
    for flow, stub, until, exits in (("Continue", noop_stub, LOHOOK_PAST, []),
                                    ("Skip", skip_stub, LOHOOK_PAST, []),
                                    ("Exit", exit_stub, exit_at, [("there", exit_at)])):
        for flags in (0x202, 0x247, 0xA93):
            out = run_lohook(dict(base, original=nop, exits=exits), stub, until, flags=flags)
            check(out["eflags"] == flags and out["esp"] == STACK + 0x80000,
                  f"lohook {flow}: preserves flags {flags:#x} and ESP")
    set_flags = ("global hook_fix_h\nhook_fix_h:\n    mov eax, [esp+4]\n    mov eax, [eax]\n"
                 "    or dword [eax+32], 1\n    and dword [eax+32], ~0x40\n    xor eax, eax\n    ret\n")
    out = run_lohook(dict(base, original=nop), set_flags, LOHOOK_PAST, flags=0x246)
    check(out["eflags"] == 0x207, "lohook: applies a Context flag edit (set CF, clear ZF)")


def test_entry():
    # an entry (our own NASM's call into Rust, cdecl; patch 68's preview
    # apply): called from a stack aligned to 4 only, it calls the shim with
    # the stack aligned to 16, hands it a Context whose site ESP is the
    # caller's (the return address, then the arguments), and returns to the
    # caller with every register and the flags as they were.
    h = dict(patch="fix", patch_no=0, kind="entry", site=None, site_name="x.entry", handler="h",
             entry="fix_h", shim="hook_fix_h", what="x", exits=[], original=b"", install=("entry", None))
    seen = STACK + 0x100                                # the stub writes: esp at its call, its first argument
    stub = ("global hook_fix_h\nhook_fix_h:\n"
            f"    lea eax, [esp+4]\n    mov [{seen:#x}], eax\n"          # esp before the call's return address
            "    mov eax, [esp+4]\n    mov eax, [eax]\n"                 # the Frame's Regs
            "    mov eax, [eax+12]\n    add eax, 8\n"                    # Regs.esp + 8: the site's ESP
            f"    mov ecx, [eax+4]\n    mov [{seen + 4:#x}], ecx\n"      # its first argument
            "    xor eax, eax\n    ret\n")
    blob, labels = assemble("\n".join(hooks_026g.entry_template(h)) + "\n", stub)
    for skew in (0, 4, 8, 12):
        m = Machine(bytes(ORIGINAL), blob, TEST_VA)
        uc = m.uc
        for name, reg in GP.items():
            uc.reg_write(reg, SENTINELS.get(name, 0))
        uc.reg_write(UC_X86_REG_EFLAGS, 0x203)
        esp = STACK + 0x80000 - skew
        for a in (0x33, 0x22, 0x11):
            esp -= 4
            uc.mem_write(esp, struct.pack("<I", a))
        esp -= 4
        uc.mem_write(esp, struct.pack("<I", 0xDEAD0000))
        uc.reg_write(UC_X86_REG_ESP, esp)
        uc.emu_start(labels["fix_h"], 0xDEAD0000, count=200000)
        at_call, first = struct.unpack("<II", bytes(uc.mem_read(seen, 8)))
        out = {r: uc.reg_read(GP[r]) for r in GP}
        check(uc.reg_read(UC_X86_REG_EIP) == 0xDEAD0000 and uc.reg_read(UC_X86_REG_ESP) == esp + 4
              and at_call % 16 == 0 and first == 0x11 and uc.reg_read(UC_X86_REG_EFLAGS) & 0xFFF == 0x203
              and all(out[r] == SENTINELS.get(r, 0) for r in GP),
              f"entry from a stack {skew} past 16: the shim called aligned to 16 ({at_call:#x}), its Context's "
              f"site ESP the caller's (first argument {first:#x}), back at the caller with esp, the registers "
              "and the flags as they were")



ORIGINAL = b""


def main():
    global ORIGINAL
    ORIGINAL = open(sys.argv[1], "rb").read()
    print("hook entry kinds against the original exe")
    for fn in (test_pointer, test_callback, test_vtable, test_splice, test_code, test_lohook, test_entry):
        fn()
    print(f"\n{passed} ok, {failed} failed")
    raise SystemExit(1 if failed else 0)


if __name__ == "__main__":
    main()
