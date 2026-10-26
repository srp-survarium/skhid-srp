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
                               UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EFLAGS)

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
    for fn in (test_pointer, test_callback, test_vtable):
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


ORIGINAL = b""


def main():
    global ORIGINAL
    ORIGINAL = open(sys.argv[1], "rb").read()
    print("hook entry kinds against the original exe")
    for fn in (test_pointer, test_callback, test_vtable):
        fn()
    print(f"\n{passed} ok, {failed} failed")
    raise SystemExit(1 if failed else 0)


if __name__ == "__main__":
    main()
