#!/usr/bin/env python3
"""Run patch 95 (rust/patches/src/animation_id_log.rs, its lohook
`animations.id_missing`: an animation pair the registry has no id for is
logged; ../docs/notes/animation-ids.md) in the default build, the original
beside it.

    python check_animation_id_log_026g.py <original survarium.exe>

Requires Python `unicorn`. The registry's lookup (0x2D6590: EAX the pair,
the registry pushed, ret 4) runs from its entry to its return on a made-up
registry, with kernel32's CreateFileA, WriteFile and CloseHandle (the log's
import slots) stubs that record the lines, GetCurrentThreadId one thread:

- a pair the registry has: both builds return its id, nothing is logged;
- a pair it has not: both builds return 0FFFFh with EBX, ESI, EDI, EBP and
  ESP as they were; the patch logs the clip's name, the registry's size and
  the resolver's return address and the six stack words above its
  arguments once: asked again,
  nothing more; a pair of two resources logs both names;
- a resource without a name entry is logged by its address alone;
- after eight distinct pairs, a ninth is not logged.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import patch_026g as base

LOOKUP = 0x2D6590
CREATE_FILE, WRITE_FILE, CLOSE_HANDLE = 0x870048, 0x870108, 0x870040   # the import slots
THREAD_SLOT = 0x870118                     # GetCurrentThreadId (the lock's owner)
REGISTRY, DATA, STACK, STUBS = 0x40000000, 0x41000000, 0x42000000, 0x43000000
MAP, HEADER = REGISTRY + 0xE030, REGISTRY + 0xE034
IDS = DATA + 0x8000                        # the ids' vector
RESOURCES = DATA + 0x1000                  # resource i at RESOURCES + 0x100 x i
ENTRIES, NAMES = DATA + 0x4000, DATA + 0x5000
KEY = DATA + 0x7000
RETURN = 0x2D7C2B                          # the resolver's, after its call (never run)
FRAME = STACK + 0x8000                     # the resolver's EBP
CALLER = 0x3980AB
ABOVE = tuple(0x51510000 + i for i in range(6))     # EBP+14h..28h
KEPT = (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)
NAME = b"resources/animations/single/weapons/ak/player/stand/3rd_view/reload_tactic"


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((REGISTRY, 0x11000), (DATA, 0x10000), (STACK, 0x10000), (STUBS, 0x1000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        for slot, stub in ((CREATE_FILE, STUBS), (WRITE_FILE, STUBS + 0x10), (CLOSE_HANDLE, STUBS + 0x20)):
            self.word(slot, stub)
        emu.mem_write(STUBS + 0x30, bytes.fromhex("b801000000c3"))   # mov eax,1 / ret
        self.word(THREAD_SLOT, STUBS + 0x30)
        for address in (STUBS, STUBS + 0x10, STUBS + 0x20):
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=address, end=address)
        emu.hook_add(UC_HOOK_CODE, self.stop, begin=RETURN, end=RETURN)
        self.registry()

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def registry(self):
        """One registered pair (resource 0 twice, id 0) in the map, its
        header's root, leftmost and rightmost; numbered, one id."""
        for i in range(12):
            res = RESOURCES + 0x100 * i
            if i != 11:                    # resource 11 has no name entry
                self.word(res + 0xB0, ENTRIES + 0x10 * i)
                self.word(ENTRIES + 0x10 * i + 4, res)
                self.word(ENTRIES + 0x10 * i + 8, NAMES + 0x100 * i)
                self.emu.mem_write(NAMES + 0x100 * i, NAME + b"_%d\0" % i)
        entry = DATA + 0x9000
        node = entry + 12                  # the entry's key, id, then its node
        self.word(entry, RESOURCES)
        self.word(entry + 4, RESOURCES)
        self.word(entry + 8, 0)
        self.word(node, HEADER)            # parent, left, right
        self.word(node + 4, 0)
        self.word(node + 8, 0)
        self.word(HEADER, node)
        self.word(HEADER + 4, node)
        self.word(HEADER + 8, node)
        self.word(IDS, entry)
        self.word(REGISTRY + 0xE044, IDS)
        self.word(REGISTRY + 0xE048, IDS + 4)
        self.emu.mem_write(REGISTRY + 0x10050, b"\x01")

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 * i) for i in range(1, 6)]
        if address == STUBS:               # CreateFileA: 7 arguments, a handle
            pops, result = 28, 0x1234
        elif address == STUBS + 0x10:      # WriteFile(file, bytes, size, &written, 0)
            self.lines.append(bytes(emu.mem_read(args[1], args[2])).decode())
            self.word(args[3], args[2])
            pops, result = 20, 1
        else:                              # CloseHandle
            pops, result = 4, 1
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def stop(self, emu, address, size, _):
        self.stopped = True
        emu.emu_stop()

    def lookup(self, first, second):
        """The lookup of (resource `first`, resource `second`) as the
        resolver makes it: the pair its local, EBP its frame."""
        self.lines, self.stopped = [], False
        self.word(KEY, RESOURCES + 0x100 * first)
        self.word(KEY + 4, RESOURCES + 0x100 * second)
        self.word(FRAME, 0x0F0F0F0F)       # the resolver's saved EBP: whatever its caller had
        self.word(FRAME + 4, CALLER)
        for i, word in enumerate(ABOVE):
            self.word(FRAME + 0x14 + 4 * i, word)
        sp = FRAME - 0x20
        self.word(sp, RETURN)
        self.word(sp + 4, REGISTRY)
        regs = {UC_X86_REG_EAX: KEY, UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_ESI: 0x05050505,
                UC_X86_REG_EDI: 0x0E0E0E0E, UC_X86_REG_EBP: FRAME, UC_X86_REG_ECX: 0, UC_X86_REG_EDX: 0}
        for reg, value in regs.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.emu_start(LOOKUP, 0, count=2_000_000)
        assert self.stopped, "the lookup did not return"
        kept = {reg: self.emu.reg_read(reg) for reg in KEPT}
        assert kept == {reg: regs[reg] for reg in KEPT}, kept
        assert self.emu.reg_read(UC_X86_REG_ESP) == sp + 8, "ret 4"
        return self.emu.reg_read(UC_X86_REG_EAX) & 0xFFFF, self.lines


TAIL = [f"srp: animation_id:   registry of 1 ids; asked from 0x{CALLER:08x}\r\n",
        "srp: animation_id:   stack above:" + "".join(f" 0x{w:08x}" for w in ABOVE) + "\r\n"]


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    stock, ours = Run(original), Run(bytes(built))
    failed = 0

    def case(what, ok, detail=""):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {what}" + (f"\n      {detail}" if not ok and detail else ""))

    def name(i):
        return (NAME + b"_%d" % i).decode()

    for run, label in ((stock, "original"), (ours, "patched")):
        case(f"{label}: a registered pair: its id, nothing logged", run.lookup(0, 0) == (0, []), run.lookup(0, 0))
    case("original: a missing pair: 0FFFFh, nothing logged", stock.lookup(1, 1) == (0xFFFF, []))
    result, lines = ours.lookup(1, 1)
    want = [f"srp: animation_id: no id for 0x{RESOURCES + 0x100:08x} {name(1)}\r\n",
            *TAIL]
    case("patched: a missing pair: 0FFFFh, its name, the registry and the callers logged",
         result == 0xFFFF and lines == want, (hex(result), lines))
    case("patched: the same pair again: nothing more", ours.lookup(1, 1) == (0xFFFF, []))
    result, lines = ours.lookup(2, 3)
    want = [f"srp: animation_id: no id for 0x{RESOURCES + 0x200:08x} {name(2)}\r\n",
            f"srp: animation_id:   paired with 0x{RESOURCES + 0x300:08x} {name(3)}\r\n",
            *TAIL]
    case("patched: a pair of two resources: both names", result == 0xFFFF and lines == want, (hex(result), lines))
    result, lines = ours.lookup(11, 11)
    case("patched: a resource without a name entry: its address",
         result == 0xFFFF and lines[:1] == [f"srp: animation_id: no id for 0x{RESOURCES + 0xB00:08x} (no name)\r\n"],
         (hex(result), lines))
    for i in range(4, 9):                  # pairs 6, 7 and 8 of the run
        ours.lookup(i, i)
    result, lines = ours.lookup(9, 9)
    case("patched: a ninth distinct pair: not logged", (result, lines) == (0xFFFF, []), (hex(result), lines))
    print("all cases passed" if not failed else f"{failed} FAILED")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
