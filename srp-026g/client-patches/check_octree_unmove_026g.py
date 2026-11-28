#!/usr/bin/env python3
"""Run patch 222 (octree_unmove_026g.asm, a code hook at
loose_oct_tree::unmove_all) in the default build and the original exe, on
the same trees, and require the same bytes.

    python check_octree_unmove_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are and
unmove_all (0x396C0; ECX the tree, a plain `ret`) runs whole in both, from
its entry to a sentinel return, on made-up loose octrees (the tree: +4 the
root, +10h its center, +1Ch its half size; a node: eight child pointers,
+24h its first object; an object: +1Ch the next, +2Ch the moved flag) laid
out in a shuffled order: no root, a root alone, bushy and sparse trees, a
chain of nodes 60 deep, every flag 0, some, all, and flags of any byte.
Every byte of the nodes and objects, EBX, ESI, EDI, EBP and how far ESP
moved compared. The instructions each run took are printed.
"""
import os
import random
import struct
import sys

from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_ECX, UC_X86_REG_EBX, UC_X86_REG_ESP, UC_X86_REG_EBP, \
    UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

if len(sys.argv) > 1:
    os.environ.setdefault("SRP_026G_EXE", sys.argv[1])
import patch_026g as base

UNMOVE_ALL = 0x396C0
TREE, ARENA, STACK, SENTINEL = 0x20000000, 0x20100000, 0x21000000, 0x22000000
ARENA_SIZE = 0x400000
SLOT = 0x40
GAME = {UC_X86_REG_EBX: 0x11111111, UC_X86_REG_EBP: 0x22222222, UC_X86_REG_ESI: 0x33333333,
        UC_X86_REG_EDI: 0x44444444}
KEPT = (UC_X86_REG_EBX, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI)
LIMIT = 2000000


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(TREE, 0x1000, UC_PROT_ALL)
        emu.mem_map(ARENA, ARENA_SIZE, UC_PROT_ALL)
        for at in (STACK, SENTINEL):
            emu.mem_map(at, 0x100000, UC_PROT_ALL)
        self.steps = 0
        emu.hook_add(UC_HOOK_CODE, self._step)

    def _step(self, *_):
        self.steps += 1

    def case(self, tree, arena):
        emu = self.emu
        emu.mem_write(TREE, tree)
        emu.mem_write(ARENA, arena)
        sp = STACK + 0x80000 - 4
        emu.mem_write(sp, struct.pack("<I", SENTINEL))
        for reg, value in {**GAME, UC_X86_REG_ECX: TREE}.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, sp)
        try:
            emu.emu_start(UNMOVE_ALL, SENTINEL, count=LIMIT)
        except UcError as error:
            return f"fault: {error}"
        if emu.reg_read(UC_X86_REG_EIP) != SENTINEL:
            return "no end"
        return (tuple(emu.reg_read(r) for r in KEPT), emu.reg_read(UC_X86_REG_ESP) - sp,
                bytes(emu.mem_read(ARENA, len(arena))))


def make_tree(rng, nodes, objects, chain=False, flags="some"):
    """The tree's 0x20 bytes and the arena's: nodes and objects in shuffled slots."""
    slots = list(range(nodes + objects))
    rng.shuffle(slots)
    at = lambda k: ARENA + SLOT * slots[k]
    arena = bytearray(rng.getrandbits(8) for _ in range(SLOT * (nodes + objects)))
    put = lambda a, v: struct.pack_into("<I", arena, a - ARENA, v)
    for n in range(nodes):
        for s in range(8):
            put(at(n) + 4 * s, 0)
        put(at(n) + 0x24, 0)
    free = {n: list(range(8)) for n in range(nodes)}
    for n in range(1, nodes):
        p = n - 1 if chain else rng.choice([q for q in range(n) if free[q]])
        s = free[p].pop(rng.randrange(len(free[p])))
        put(at(p) + 4 * s, at(n))
    for o in range(objects):
        a, n = at(nodes + o), rng.randrange(nodes)
        put(a + 0x1C, struct.unpack_from("<I", arena, at(n) + 0x24 - ARENA)[0])
        put(at(n) + 0x24, a)
        flag = {"none": 0, "all": 1, "some": int(rng.random() < 0.05), "any": rng.getrandbits(8)}[flags]
        arena[a + 0x2C - ARENA] = flag
    tree = struct.pack("<4I3fI", 0, at(0) if nodes else 0, 0, 0, *(float(rng.uniform(-500, 500)) for _ in range(3)),
                       struct.unpack("<I", struct.pack("<f", rng.choice([512.0, 4096.0, 1.0])))[0])
    return tree.ljust(0x20, b"\0"), bytes(arena)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    stock, patched = Run(original), Run(bytes(built))
    rng = random.Random(222)
    failed = 0

    def check(what, count, make):
        nonlocal failed
        bad, steps = [], [0, 0]
        for _ in range(count):
            tree, arena = make()
            stock.steps = patched.steps = 0
            a, b = stock.case(tree, arena), patched.case(tree, arena)
            steps[0] += stock.steps
            steps[1] += patched.steps
            if a != b or isinstance(a, str):
                bad.append((a if isinstance(a, str) else a[:2], b if isinstance(b, str) else b[:2]))
        failed += bool(bad)
        print(f"  {'ok' if not bad else 'FAIL'}  {what}: {count} trees, {len(bad)} differ; "
              f"instructions {steps[0]} original, {steps[1]} patched")
        for args in bad[:5]:
            print(f"        {args}")

    check("no root", 3, lambda: (bytes(0x20), b"\0" * SLOT))
    check("a root alone, with and without objects", 20,
          lambda: make_tree(rng, 1, rng.choice([0, 1, 5]), flags=rng.choice(["none", "all", "any"])))
    check("trees of 2 to 300 nodes, every flag kind", 300,
          lambda: make_tree(rng, rng.randrange(2, 300), rng.randrange(0, 900),
                            flags=rng.choice(["none", "some", "all", "any"])))
    check("a chain of nodes 60 deep", 10, lambda: make_tree(rng, 60, 200, chain=True, flags="any"))
    check("4000 nodes, 16000 objects", 2, lambda: make_tree(rng, 4000, 16000, flags="some"))

    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
