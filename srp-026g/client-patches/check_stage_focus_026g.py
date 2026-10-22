#!/usr/bin/env python3
"""Run the original's and patch 52's Stage focus getter (Scaleform's
flash.display.Stage::focusGet, 0x771FA0) with Unicorn (2.1.4).

    python check_stage_focus_026g.py <original survarium.exe>

The exe is built as patch_026g.py builds it (with the late join), without
the late join and over on_hash_mismatch; the original is run the same way.
The getter runs from its entry to its `ret 4` on a made-up movie: the
stage (ecx) whose +14h, +40h, +1A8h and +8 lead to the MovieImpl, its focus
group index (+3F54h, 0) and the focus group's weak pointer (+3B64h). Stubs:
the weak pointer's lock (0x514370, the focused object or none), the
release (0x505C90, counted), the focused object's interface call (vtable +4
of the interface at byte [obj+41h]: its AvmInteractiveObj + 1Ch, or none)
and the result's assignment (0x790E40, what it was given written down).
The AvmInteractiveObj's raw pointer (+8) and collectible pointer (+4) lead
to the ActionScript object, whose traits (+14h) give the type (+3Ch).

- a focused InteractiveObject (types 12h, 17h, 20h): both give it, by the
  raw pointer or by the collectible one with its bit 0 set;
- a focused object of another type (5, 16h): both give null;
- no focused object: both give null;
- a focused object whose ActionScript object is gone (both pointers 0, or
  the collectible one 1): the original reads 14h (the access violation at
  0x77200B of 2026-10-22), the build gives null;
- an interface call that gives none: the original reads 8, the build null.
The release is called as often in both, and the stack is as the getter
leaves it.
"""
import struct
import sys

from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EIP, UC_X86_REG_ESP

import patch_026g as base

GETTER, LOCK, RELEASE, ASSIGN = 0x771FA0, 0x514370, 0x505C90, 0x790E40
STAGE, MOVIE, FOCUSED, AVM, AS3, TRAITS = 0x41000000, 0x41010000, 0x41020000, 0x41030000, 0x41040000, 0x41050000
SCRATCH, STACK = 0x41060000, 0x41108000
INTERFACE_CALL = SCRATCH + 0x100              # the focused object's vtable +4
RESULT, DONE = SCRATCH + 0x200, SCRATCH + 0x300


class Movie:
    def __init__(self, data, focused, interface, raw, collectible, kind):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        headers = base.Headers(data)
        for _, rva, vsize, raw_at, rsize, _ in headers.sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw_at:raw_at + min(rsize, vsize or rsize)]))
        for at in (STAGE, MOVIE, FOCUSED, AVM, AS3, TRAITS, SCRATCH):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        emu.mem_map(STACK - 0x8000, 0x10000, UC_PROT_ALL)
        self.word(STAGE + 0x14, STAGE + 0x100)
        self.word(STAGE + 0x140, STAGE + 0x200)
        self.word(STAGE + 0x200 + 0x1A8, STAGE + 0x300)
        self.word(STAGE + 0x308, MOVIE)
        emu.mem_write(MOVIE + 0x3F54, b"\0")
        self.focused = FOCUSED if focused else 0
        self.word(FOCUSED, SCRATCH)                    # the interface at +0 (byte [+41h] 0): its vtable
        emu.mem_write(FOCUSED + 0x41, b"\0")
        self.word(SCRATCH + 4, INTERFACE_CALL)
        self.interface = AVM + 0x1C if interface else 0
        self.word(AVM + 8, raw)
        self.word(AVM + 4, collectible)
        self.word(AS3 + 0x14, TRAITS)
        self.word(TRAITS + 0x3C, kind)
        self.word(RESULT, 0)                           # the result's SPtr, empty
        self.assigned, self.releases = [], 0
        emu.hook_add(UC_HOOK_CODE, self.stub)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def returns(self, pop, value=None):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        if value is not None:
            self.emu.reg_write(UC_X86_REG_EAX, value)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pop)

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        if address == LOCK:                            # thiscall: ecx the weak pointer; (out), ret 4
            assert emu.reg_read(UC_X86_REG_ECX) == MOVIE + 0x3B64
            self.word(self.read(sp + 4), self.focused)
            self.returns(4, self.read(sp + 4))
        elif address == RELEASE:                       # thiscall: ecx the object
            assert emu.reg_read(UC_X86_REG_ECX) == FOCUSED
            self.releases += 1
            self.returns(0)
        elif address == INTERFACE_CALL:                # thiscall: ecx the interface
            assert emu.reg_read(UC_X86_REG_ECX) == FOCUSED
            self.returns(0, self.interface)
        elif address == ASSIGN:                        # thiscall: ecx the result; (object), ret 4
            assert emu.reg_read(UC_X86_REG_ECX) == RESULT
            self.assigned.append(self.read(sp + 4))
            self.returns(4)

    def run(self):
        emu = self.emu
        sp = STACK - 8
        self.word(sp, DONE)
        self.word(sp + 4, RESULT)
        emu.reg_write(UC_X86_REG_ESP, sp)
        emu.reg_write(UC_X86_REG_ECX, STAGE)
        try:
            emu.emu_start(GETTER, DONE, count=10000)
        except UcError:
            return ("fault", emu.reg_read(UC_X86_REG_EIP))
        assert emu.reg_read(UC_X86_REG_ESP) == STACK, "the stack is not what the getter leaves"
        return ("result", self.assigned, self.releases, self.read(RESULT))


CASES = {
    # label: focused, interface call gives one, raw, collectible, traits type
    "interactive 12h, raw": (True, True, AS3, 0, 0x12),
    "interactive 17h, collectible": (True, True, 0, AS3, 0x17),
    "interactive 20h, collectible bit 0": (True, True, 0, AS3 + 1, 0x20),
    "type 5": (True, True, AS3, 0, 5),
    "type 16h": (True, True, AS3, 0, 0x16),
    "none focused": (False, True, AS3, 0, 0x12),
    "no ActionScript object": (True, True, 0, 0, 0x12),
    "collectible 1": (True, True, 0, 1, 0x12),
    "no interface": (True, False, AS3, 0, 0x12),
}


def run(data):
    return {label: Movie(data, *case).run() for label, case in CASES.items()}


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    patched, _ = base.build(original, "section", code, None, late_join)
    before, _ = base.build(original, "section", code, None, None)
    mismatch, _ = base.build(original, "hash-mismatch", open(base.default_bin("hash-mismatch"), "rb").read())
    va, old, new = base.STAGE_FOCUS
    at = base.file_offset(va)
    assert original[at:at + len(old)] == old
    assert all(image[at:at + len(new)] == new for image in (patched, before, mismatch))
    stock, built = run(original), run(patched)
    assert run(before) == built and run(mismatch) == built, "the other builds do not read the focus as this one"

    for label in ("interactive 12h, raw", "interactive 17h, collectible", "interactive 20h, collectible bit 0"):
        # given the object; the lock's reference released once (the copy's at 0x771FD9, the end's at 0x772027)
        assert stock[label] == built[label] == ("result", [AS3], 2, 0), (label, stock[label], built[label])
    for label in ("type 5", "type 16h"):
        assert stock[label] == built[label] == ("result", [], 2, 0), (label, stock[label], built[label])
    assert stock["none focused"] == built["none focused"] == ("result", [], 0, 0), stock["none focused"]
    # The crash of 2026-10-22: 0x77200B reads [0 + 14h]; an interface call that gives none, [0 + 8].
    assert stock["no ActionScript object"] == ("fault", 0x77200B), stock["no ActionScript object"]
    assert stock["collectible 1"] == ("fault", 0x77200B), stock["collectible 1"]
    assert stock["no interface"] == ("fault", 0x771FF8), stock["no interface"]
    for label in ("no ActionScript object", "collectible 1", "no interface"):
        assert built[label] == ("result", [], 2, 0), (label, built[label])

    print("patch 52: a focused InteractiveObject is given by both, by its raw or its collectible pointer; "
          "another type or no focus gives null in both; a focused object without its ActionScript object, or "
          "whose interface gives none, faulted in the original (0x77200B, 0x771FF8) and gives null in the "
          "build; every full build the same; the releases and the stack as the original's")


if __name__ == "__main__":
    main()
