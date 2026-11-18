#!/usr/bin/env python3
"""Run the grass sort's __linear_insert in the original and patch 146's exe.

    python check_grass_sort_026g.py <original survarium.exe>

Requires Python `unicorn`. grass_world::process_culling's std::sort (0x1610B0)
places each of its first sixteen patches with __linear_insert (0x161E50): the
comparator 0x1603B0 tests the patch against the first element, and when it is
not below it the unguarded walk 0x162060 moves it down until it is not below
its neighbour. For a pair of patches whose squared distances differ only in the
order of the adds, the original's test and walk disagree: the walk passes the
first element and reads the null word before the array (the rev58 crash, read
of 0x50 at 0x162085). The patched exe must sort the pair without reading it.
"""
import random
import struct
import sys

from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EIP, UC_X86_REG_ESP

import patch_026g as base

LINEAR_INSERT, READ_AT = 0x161E50, 0x162085
STACK, HEAP, RETURN = 0x20000000, 0x20100000, 0x20400000   # above the image
ARRAY = HEAP + 0x100                                        # word before it: 0
FIRST, VALUE = HEAP + 0x1000, HEAP + 0x6000                 # two patches (0x40C4 bytes)
KEY, CENTER = 0x50, 0x4084


def f32(x):
    return struct.unpack("<f", struct.pack("<f", x))[0]


def walk_order(x, y, z):
    """Every inlined copy and the comparator's right side: dz2 + dy2 + dx2."""
    return f32(f32(f32(z * z) + f32(y * y)) + f32(x * x))


def near_tie():
    """A patch whose two orders of adds differ: the pair (x, y, z), (y, x, z)."""
    rng = random.Random(146)
    while True:
        x, y, z = (f32(rng.uniform(-100.0, 100.0)) for _ in range(3))
        # the original's left side, dz2 + dx2 + dy2, is the swapped patch's walk order
        if walk_order(x, y, z) < walk_order(y, x, z):
            return (x, y, z), (y, x, z)


def image(data):
    emu = Uc(UC_ARCH_X86, UC_MODE_32)
    headers = base.Headers(data)
    for _, rva, vsize, raw, rsize, _ in headers.sections(data):
        start = base.IMAGE_BASE + rva
        emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
        emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
    emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
    emu.mem_map(HEAP, 0x10000, UC_PROT_ALL)
    emu.mem_map(RETURN, 0x1000, UC_PROT_ALL)
    return emu


def insert(emu, value, first):
    """__linear_insert(first, value) with the value's hole right after first."""
    emu.mem_write(HEAP, bytes(0x10000))
    for patch, center in ((FIRST, first), (VALUE, value)):
        emu.mem_write(patch + KEY, struct.pack("<I", 7))
        emu.mem_write(patch + CENTER, struct.pack("<3f", *center))
    emu.mem_write(ARRAY, struct.pack("<II", FIRST, VALUE))
    esp = STACK + 0x8000
    # cdecl: first, value, the camera (by value, at the origin); last in eax
    emu.mem_write(esp, struct.pack("<III3f", RETURN, ARRAY, VALUE, 0.0, 0.0, 0.0))
    emu.reg_write(UC_X86_REG_ESP, esp)
    emu.reg_write(UC_X86_REG_EAX, ARRAY + 4)
    try:
        emu.emu_start(LINEAR_INSERT, RETURN, count=100000)
    except UcError:
        return ("fault", emu.reg_read(UC_X86_REG_EIP))
    if emu.reg_read(UC_X86_REG_EIP) != RETURN or emu.reg_read(UC_X86_REG_ESP) != esp + 4:
        raise AssertionError("__linear_insert did not return")
    return ("sorted", struct.unpack("<II", emu.mem_read(ARRAY, 8)))


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    patched, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    value, first = near_tie()
    stock = insert(image(original), value, first)
    if stock != ("fault", READ_AT):
        raise AssertionError(f"the original should read before the array at {READ_AT:#x}: {stock}")
    new = insert(image(patched), value, first)
    if new != ("sorted", (VALUE, FIRST)):
        raise AssertionError(f"the patched exe should put the nearer patch first: {new}")
    # In order already, both leave the pair as it was.
    for data in (original, patched):
        if insert(image(data), first, value) != ("sorted", (FIRST, VALUE)):
            raise AssertionError("a pair in order was moved")
    va, old, replacement = base.GRASS_SORT_ORDER
    at = base.file_offset(va)
    assert original[at:at + len(old)] == old and patched[at:at + len(old)] == replacement
    print(f"patch 146: the near tie {value} / {first}: the original reads before the array at "
          f"{READ_AT:#x}, the patched exe sorts it; a pair in order stays")


if __name__ == "__main__":
    main()
