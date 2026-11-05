#!/usr/bin/env python3
"""The alignment probe of the Unicorn checks: every aligned-SSE memory
operand our code executes must be 16-byte aligned.

Unicorn does not fault on a misaligned `movaps` (or any legacy-SSE packed
operation with a 16-byte memory operand); a real CPU raises #GP. rustc's
i686 code keeps 16-byte locals with movaps and assumes the stack is aligned
to 16 at every call, which only the framework's entries make
(rust_hook_026g.inc, hooks_026g.py): a way into Rust that skips them, or a
NASM constant at an unaligned address, runs clean under Unicorn and crashes
the game. Found in #893's review: patch 48 called the Rust preview apply
through a dword with the game's 4-aligned stack.

Two ways to use it:

- in a check, before its first `Uc` is made:

      import align_probe_026g as align
      align.install()
      ...
      failed += align.report(report)     # report(ok, text), the check's own

  every `Uc` made afterwards runs a code hook over our code (the sections
  srp adds, `.srp` up to the arena's end), and `report` says each
  misaligned operand (its VA, instruction, address, count);

- around any check, unchanged:

      python align_probe_026g.py check_<patch>_026g.py <original survarium.exe> [its other arguments]

  runs the check with the probe installed and fails (exit 1) when the
  check fails or anything misaligned ran.

Requires Python `unicorn` and `capstone` (the decoder).
"""
import os
import runpy
import sys

import capstone
import unicorn
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, \
    UC_X86_REG_EDI, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_ESP

import patch_026g as base

# Our code: from `.srp` (the first section srp adds) to the arena's end.
LOW, HIGH = base.SECTION_VA, base.ARENA_VA + base.ARENA_LIMIT
# Legacy SSE instructions with a 16-byte memory operand that need it aligned
# (Intel SDM: "the operand must be aligned on a 16-byte boundary or a #GP");
# the unaligned forms (movups, movupd, movdqu, lddqu) and the scalar ones
# (an 8- or 4-byte operand) are not.
UNALIGNED = {"movups", "movupd", "movdqu", "lddqu", "vmovups", "vmovupd", "vmovdqu"}
REGISTERS = {capstone.x86.X86_REG_EAX: UC_X86_REG_EAX, capstone.x86.X86_REG_EBX: UC_X86_REG_EBX,
             capstone.x86.X86_REG_ECX: UC_X86_REG_ECX, capstone.x86.X86_REG_EDX: UC_X86_REG_EDX,
             capstone.x86.X86_REG_ESI: UC_X86_REG_ESI, capstone.x86.X86_REG_EDI: UC_X86_REG_EDI,
             capstone.x86.X86_REG_EBP: UC_X86_REG_EBP, capstone.x86.X86_REG_ESP: UC_X86_REG_ESP}

_decoder = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
_decoder.detail = True
_operands = {}          # VA -> (text, [memory operands]) of each aligned-SSE instruction; None for others
FOUND = {}              # (VA, instruction) -> (the first address it used, count)
_installed = []


def _aligned_operands(insn):
    """The 16-byte memory operands of `insn` that must be aligned."""
    if insn.mnemonic in UNALIGNED or insn.mnemonic.startswith("v"):
        return []
    groups = set(insn.groups)
    if not groups & {capstone.x86.X86_GRP_SSE1, capstone.x86.X86_GRP_SSE2, capstone.x86.X86_GRP_SSE3,
                     capstone.x86.X86_GRP_SSSE3, capstone.x86.X86_GRP_SSE41, capstone.x86.X86_GRP_SSE42}:
        return []
    return [op.mem for op in insn.operands if op.type == capstone.x86.X86_OP_MEM and op.size == 16]


def _hook(uc, address, size, _):
    if address not in _operands:
        code = bytes(uc.mem_read(address, size))
        insn = next(_decoder.disasm(code, address), None)
        mems = _aligned_operands(insn) if insn is not None else []
        _operands[address] = (f"{insn.mnemonic} {insn.op_str}", mems) if mems else None
    found = _operands[address]
    if found is None:
        return
    text, mems = found
    for mem in mems:
        at = mem.disp
        if mem.base:
            at += uc.reg_read(REGISTERS[mem.base])
        if mem.index:
            at += uc.reg_read(REGISTERS[mem.index]) * mem.scale
        at &= 0xFFFFFFFF
        if at % 16:
            key = (address, text)
            first, count = FOUND.get(key, (at, 0))
            FOUND[key] = (first, count + 1)


def install():
    """Every `unicorn.Uc` made from now on probes our code (idempotent)."""
    if _installed:
        return
    original = unicorn.Uc.__init__

    def init(self, *args, **kwargs):
        original(self, *args, **kwargs)
        self.hook_add(unicorn.UC_HOOK_CODE, _hook, None, LOW, HIGH - 1)

    unicorn.Uc.__init__ = init
    _installed.append(original)


def misaligned():
    """[(VA, instruction, the first address, count)] of what ran misaligned."""
    return [(va, text, first, count) for (va, text), (first, count) in sorted(FOUND.items())]


def report(say):
    """One line through `say(ok, text)`: the misaligned operands, or none;
    1 when there were any (to add to a check's failures), else 0."""
    bad = misaligned()
    say(not bad, "every aligned-SSE memory operand our code ran was 16-byte aligned "
        f"({len([v for v in _operands.values() if v])} such instructions seen in {LOW:#x}..{HIGH:#x})"
        + "".join(f"\n          {va:#x} {text}: {first:#x} (x{count})" for va, text, first, count in bad))
    return 1 if bad else 0


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    check = os.path.abspath(sys.argv[1])
    install()
    sys.argv = [check] + sys.argv[2:]
    sys.path.insert(0, os.path.dirname(check))
    code = 0
    try:
        runpy.run_path(check, run_name="__main__")
    except SystemExit as stop:
        code = stop.code if isinstance(stop.code, int) else (0 if stop.code is None else 1)
    say = lambda ok, text: print(f"  {'ok' if ok else 'FAIL'}  {text}")
    bad = report(say)
    sys.exit(1 if code or bad else 0)


if __name__ == "__main__":
    main()
