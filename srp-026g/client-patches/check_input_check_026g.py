#!/usr/bin/env python3
"""Run patch 99's input check (the Rust patch `input_check`) in the default
build beside the original, on made-up player inputs.

    python check_input_check_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, and
game_world_core::change_input<player_input> (0x1FAB50) is entered as its
callers enter it (stdcall, `ret 24h`: the core, the input's rotation and
action mask, its time, its player, then the from-network flag, the precise
flag and the caller's two last arguments). A run stops at 0x1FAB56, past the
hooked span (the function goes on to keep the input), or back at its return
address.

- An input whose rotation a client's mouse makes (each half a number under
  1024): the build reaches 0x1FAB56 with every register as the original has
  it there, the stack as the original's, nothing logged.
- Any other (a NaN, an infinity, 1024 or more, as the yaw or the pitch): the
  original reaches 0x1FAB56 (it keeps the input); the build returns at once
  to the caller with EAX 6 (the event action both of the function's own
  refusals return), its 24h bytes of arguments popped, EBX/ESI/EDI/EBP as
  they were, the core and the arguments not written, and one line in
  srp_patches.log naming the place, the time and the rotation.

The verdicts are the match server's (`PlayerInput::rotation_is_sane`,
crates/survarium/src/v026g/match_msgs.rs), restated here as RULE.
"""
import os
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE, UC_HOOK_MEM_WRITE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI,
                               UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP)

if len(sys.argv) > 1:
    os.environ.setdefault("SRP_026G_EXE", sys.argv[1])    # a span hook's bytes (patch_026g.RUST_HOOKS)
import patch_026g as base

START, PAST_SPAN, SPAN = 0x1FAB50, 0x1FAB56, bytes.fromhex("558bec83e4f8")
REJECTED, POPS = 6, 0x24
THREAD_SLOT = 0x870118
CREATE_FILE_SLOT, WRITE_FILE_SLOT, CLOSE_SLOT = 0x870048, 0x870108, 0x870040   # the log's kernel32
CORE, STUBS, STACK = 0x30000000, 0x31000000, 0x32000000
DONE = STUBS + 0xF00                    # the caller's return address
SP = STACK + 0x8000
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP)
STEADY = dict(zip(REGS, (0x0A0A0A0A, 0x0B0B0B0B, 0x0C0C0C0C, 0x0D0D0D0D, 0x05050505, 0x0E0E0E0E, 0x0019FF00)))
KEPT = (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)


def RULE(x, y):
    """PlayerInput::rotation_is_sane: each half's exponent under that of 1024."""
    return all(h & 0x7C00 < 25 << 10 for h in (x, y))


SANE = [(0, 0), (0x3C00, 0xBC00), (0x2E66, 0x1A3D), (0x63FF, 0xE3FF), (0x0001, 0x8000), (0x8000, 0x0000)]
WRONG = [0x7C00, 0xFC00, 0x7E00, 0x7C01, 0xFE00, 0xFFFF, 0x6400, 0xE400, 0x7BFF]
INSANE = [(w, 0x3C00) for w in WRONG] + [(0x3C00, w) for w in WRONG] + [(0x7C00, 0x7E00)]

failed = 0


def check(ok, what):
    global failed
    failed += not ok
    print(f"  {'ok' if ok else 'FAIL'}  {what}")


class Game:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at in (CORE, STUBS, STACK):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        self.word(THREAD_SLOT, STUBS + 0x80)
        emu.mem_write(STUBS + 0x80, b"\xB8\x01\x00\x00\x00\xC3")
        # the log: CreateFileA (a handle, stdcall 7), WriteFile (the bytes kept, stdcall 5), CloseHandle (1)
        self.logged = b""
        for slot, stub, code in ((CREATE_FILE_SLOT, STUBS + 0x90, b"\xB8\x07\x00\x00\x00\xC2\x1C\x00"),
                                 (WRITE_FILE_SLOT, STUBS + 0xA0, b"\xB8\x01\x00\x00\x00\xC2\x14\x00"),
                                 (CLOSE_SLOT, STUBS + 0xB0, b"\xB8\x01\x00\x00\x00\xC2\x04\x00")):
            self.word(slot, stub)
            emu.mem_write(stub, code)
        emu.hook_add(UC_HOOK_CODE, self.write_file, begin=STUBS + 0xA0, end=STUBS + 0xA0)
        self.writes = []
        emu.hook_add(UC_HOOK_MEM_WRITE, self.wrote)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at, size=4):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0] if size == 4 else bytes(self.emu.mem_read(at, size))

    def write_file(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.logged += self.read(self.read(sp + 8), self.read(sp + 12))

    def wrote(self, emu, access, address, size, value, _):
        self.writes.append(address)

    def change_input(self, x, y, actions=0x81, time=12350, player=3):
        """Enters change_input as the relay (0x27C321) does; (where it stopped, the registers, the
        stack's arguments after, the core after, the line logged)."""
        emu = self.emu
        self.logged, self.writes = b"", []
        emu.mem_write(CORE, bytes(0x100))
        args = [CORE, x | y << 16, actions, time, player, 1, 0, 0x2C, STACK + 0x9000]
        emu.mem_write(SP, struct.pack("<10I", DONE, *args))
        for reg, value in STEADY.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, SP)
        stops = (PAST_SPAN, DONE)
        hook = emu.hook_add(UC_HOOK_CODE, lambda e, a, *_: e.emu_stop() if a in stops else None)
        emu.emu_start(START, DONE, count=200000)
        emu.hook_del(hook)
        regs = {r: emu.reg_read(r) for r in REGS + (UC_X86_REG_ESP, UC_X86_REG_EIP)}
        return regs, self.read(SP + 4, 36), self.read(CORE, 0x100), self.logged


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    built = bytes(built)
    linked = base.arena()
    block = linked.blocks["patches.code"]
    installs = {h[0]: h for h in base.rust_hooks(original)}
    va, old, symbol, opcode = installs[START]
    at = base.file_offset(va)
    new = base.hook_bytes(linked.symbols[symbol], va, old, 0, opcode)
    check(opcode == 0xE9 and old == SPAN and original[at:at + 6] == SPAN and built[at:at + len(new)] == new
          and block.va <= linked.symbols[symbol] < block.va + block.size,
          f"{va:#x}: {old.hex()} in the original, `jmp {symbol}` (patches.code) here")
    ret = base.file_offset(0x1FAC4E)
    check(original[ret:ret + 3] == b"\xC2\x24\x00", "the refused exit 0x1FAC4E is the function's `ret 24h`")

    stock, ours = Game(original), Game(built)
    for x, y in SANE:
        assert RULE(x, y)
        s_regs, s_args, s_core, _ = stock.change_input(x, y)
        o_regs, o_args, o_core, logged = ours.change_input(x, y)
        check(s_regs[UC_X86_REG_EIP] == o_regs[UC_X86_REG_EIP] == PAST_SPAN and s_regs == o_regs
              and s_args == o_args and s_core == o_core and logged == b"",
              f"rotation {x:04x} {y:04x}: taken, at 0x1FAB56 as the original, every register the same, nothing logged")
    for x, y in INSANE:
        assert not RULE(x, y)
        s_regs, _, _, _ = stock.change_input(x, y)
        o_regs, o_args, o_core, logged = ours.change_input(x, y)
        line = (f"srp: input_check: place 3's input at 12350 ms has a rotation no client makes "
                f"(0x{x | y << 16:08x}), not taken\r\n").encode()
        args = struct.pack("<9I", CORE, x | y << 16, 0x81, 12350, 3, 1, 0, 0x2C, STACK + 0x9000)
        check(s_regs[UC_X86_REG_EIP] == PAST_SPAN and o_regs[UC_X86_REG_EIP] == DONE
              and o_regs[UC_X86_REG_EAX] == REJECTED and o_regs[UC_X86_REG_ESP] == SP + 4 + POPS
              and all(o_regs[r] == STEADY[r] for r in KEPT) and o_args == args and o_core == bytes(0x100)
              and not any(CORE <= w < CORE + 0x10000 for w in ours.writes) and logged == line,
              f"rotation {x:04x} {y:04x}: the original keeps it; refused, eax 6, ret 24h, registers kept, logged")
    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
