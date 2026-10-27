#!/usr/bin/env python3
"""Run the original's and patch 33's predicate of the archive reads
(hdd_manager_sorting_predicate 0x10BCC0) and the game's own std::sort of
hdd_manager::grab_sorted_queries (introsort 0x10BF70, final insertion sort
0x10C060) on made-up read queries.

    python check_read_order_026g.py <original survarium.exe>

Requires Python `unicorn` (2.1.4). Each image is loaded as it is (the exe is
never rebased: since the Rust port the patch is arena code with absolute
addresses, so the rebased load this check once made of the asm is gone).
The
predicate runs with the game's own get_physical_path (0x4BC10), strcmp_s
(0x3B5A0) and get_file_offs (0x47160): a made-up archive node has the flags
4 | 2000h (in an archive, its offset beside it), its mount's path at +28h.
What must hold:
- on random sets of queries over two and three archives, files that are in
  none and null iterators, the build's predicate is a strict weak ordering
  (irreflexive, transitive, incomparability transitive), and the game's
  sort with it reads only the array and leaves it sorted;
- the original's is not one on such sets (a cycle is found), and its sort
  reads outside the array on some of them, the 0 before it among them (the
  round 4d crash), already with reads of two archives alone;
- on sets of one archive, or of files in none, in which queries of
  different priorities share their parent, the two predicates answer alike
  for every pair, so the sort orders such a batch as the original.
"""
import random
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE, UcError
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EIP, \
    UC_X86_REG_ESP, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP

import patch_026g as base

PREDICATE, INTROSORT, FINAL_SORT = 0x10BCC0, 0x10BF70, 0x10C060
QUERIES, NODES, MOUNTS, ARRAY = 0x30000000, 0x31000000, 0x32000000, 0x33000000
STACK, STOP = 0x340E0000, 0x35000000
QUERY_SIZE, NODE_SIZE = 0x300, 0x80
CALLEE_SAVED = {UC_X86_REG_EBX: 0x11111111, UC_X86_REG_EBP: 0x22222222, UC_X86_REG_ESI: 0x33333333,
                UC_X86_REG_EDI: 0x44444444}
ARCHIVES = (b"E:\\game\\resources.db", b"E:\\game\\resources.db.patch_12", b"E:\\game\\resources.db.patch_14")


class Image:
    def __init__(self, data, delta):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        self.delta = delta
        headers = base.Headers(data)
        for _, rva, vsize, raw, rsize, _ in headers.sections(data):
            start = base.IMAGE_BASE + rva + delta
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        if delta:
            reloc_rva, reloc_size = struct.unpack_from("<II", data, headers.reloc_at)
            table = next(raw + reloc_rva - rva for _, rva, vsize, raw, rsize, _ in headers.sections(data)
                         if rva <= reloc_rva < rva + vsize)
            at = table
            while at < table + reloc_size:
                page, block = struct.unpack_from("<II", data, at)
                if block < 8:
                    break
                for i in range((block - 8) // 2):
                    entry = struct.unpack_from("<H", data, at + 8 + 2 * i)[0]
                    if entry >> 12 == 3:
                        where = base.IMAGE_BASE + page + (entry & 0xFFF) + delta
                        self.word(where, self.read(where) + delta)
                at += block
        for at in (QUERIES, NODES, MOUNTS, ARRAY - 0x10000, STACK - 0x80000, STOP):
            emu.mem_map(at, 0x100000 if at != ARRAY - 0x10000 else 0x20000, UC_PROT_ALL)
        for i, path in enumerate(ARCHIVES):
            emu.mem_write(MOUNTS + 0x1000 * i + 0x100, path + b"\0")
            self.word(MOUNTS + 0x1000 * i + 0x28, MOUNTS + 0x1000 * i + 0x100)
        self.operands = []
        emu.hook_add(UC_HOOK_CODE, self.at_predicate, begin=PREDICATE + delta, end=PREDICATE + delta)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def at_predicate(self, emu, address, size, _):
        self.operands.append((emu.reg_read(UC_X86_REG_EAX), self.read(emu.reg_read(UC_X86_REG_ESP) + 4)))

    def load(self, queries):
        """queries: [(kind, archive, offset, parent, priority)]; kind 'null',
        'archive' or 'loose'; parent an index into the queries or None.
        Returns their addresses."""
        emu = self.emu
        emu.mem_write(QUERIES, bytes(QUERY_SIZE * len(queries)))
        addresses = [QUERIES + QUERY_SIZE * i for i in range(len(queries))]
        for i, (kind, archive, offset, parent, priority) in enumerate(queries):
            q, node = addresses[i], NODES + NODE_SIZE * i + 0x10
            emu.mem_write(NODES + NODE_SIZE * i, bytes(NODE_SIZE))
            if kind == "null":
                self.word(q + 0xA4, 0)
            else:
                self.word(q + 0xA4, node)
                emu.mem_write(node + 0x30, struct.pack("<H", 0x2004 if kind == "archive" else 0))
                self.word(node, MOUNTS + 0x1000 * archive)
                emu.mem_write(node - 8, struct.pack("<Q", offset))
            self.word(q + 0xA8, 0)
            self.word(q + 0x158, 0 if parent is None else addresses[parent])
            self.word(q + 0x2B8, priority)
        return addresses

    def less(self, a, b):
        emu = self.emu
        self.word(STACK, STOP)
        self.word(STACK + 4, b)
        emu.reg_write(UC_X86_REG_ESP, STACK)
        emu.reg_write(UC_X86_REG_EAX, a)
        for register, value in CALLEE_SAVED.items():
            emu.reg_write(register, value)
        emu.emu_start(PREDICATE + self.delta, STOP, count=100000)
        assert emu.reg_read(UC_X86_REG_EIP) == STOP and emu.reg_read(UC_X86_REG_ESP) == STACK + 8
        assert all(emu.reg_read(r) == v for r, v in CALLEE_SAVED.items()), "callee-saved registers"
        return emu.reg_read(UC_X86_REG_EAX) & 0xFF

    def sort(self, addresses):
        """grab_sorted_queries' two calls on an array of the addresses, a 0
        before it as in the round 4d dump. Returns (sorted, operands outside
        the set or None on a fault)."""
        emu = self.emu
        n = len(addresses)
        begin, end = ARRAY, ARRAY + 4 * n
        self.word(begin - 4, 0)
        for i, a in enumerate(addresses):
            self.word(begin + 4 * i, a)
        depth, k = 0, n
        while k > 1:
            k >>= 1
            depth += 1
        self.operands = []
        try:
            sp = STACK - 0x40
            for i, v in enumerate((STOP, begin, end, 0, 2 * depth, 0)):
                self.word(sp + 4 * i, v)
            emu.reg_write(UC_X86_REG_ESP, sp)
            emu.emu_start(INTROSORT + self.delta, STOP, count=20000000)
            for i, v in enumerate((STOP, 0)):
                self.word(sp + 4 * i, v)
            emu.reg_write(UC_X86_REG_ESP, sp)
            emu.reg_write(UC_X86_REG_EAX, begin)
            emu.reg_write(UC_X86_REG_EDI, end)
            emu.emu_start(FINAL_SORT + self.delta, STOP, count=20000000)
        except UcError:
            return None, [x for pair in self.operands for x in pair if x not in addresses] or ["fault"]
        result = [self.read(begin + 4 * i) for i in range(n)]
        stray = [x for pair in self.operands for x in pair if x not in addresses]
        return result, stray


LEFT_ITS_ARRAY = [
    ('archive', 0, 550258688, None, 1),
    ('archive', 1, 297259008, None, 1),
    ('archive', 1, 963239936, None, 1),
    ('archive', 1, 972845056, None, 1),
    ('archive', 0, 631334912, None, 1),
    ('archive', 1, 411621376, None, 1),
    ('archive', 0, 1067868160, None, 1),
    ('archive', 0, 493301760, None, 1),
    ('archive', 1, 530718720, None, 1),
    ('archive', 1, 1016692736, None, 1),
    ('archive', 0, 160837632, None, 1),
    ('archive', 0, 4329472, None, 1),
    ('archive', 0, 133847040, None, 1),
    ('archive', 0, 92968960, None, 1),
    ('archive', 0, 949977088, None, 1),
    ('archive', 1, 929050624, None, 1),
    ('archive', 0, 817158144, None, 1),
    ('archive', 0, 466604032, None, 1),
    ('archive', 1, 397035520, None, 1),
    ('archive', 0, 814702592, None, 1),
    ('archive', 0, 22831104, None, 1),
    ('archive', 0, 543850496, None, 1),
    ('archive', 0, 404490240, None, 1),
    ('archive', 0, 507047936, None, 1),
    ('archive', 0, 684462080, None, 1),
    ('archive', 0, 972791808, None, 1),
]


def random_set(rng, n, archives, kinds=("null", "archive", "archive", "archive", "loose"), parents=True):
    queries = []
    for i in range(n):
        kind = rng.choice(kinds)
        parent = rng.randrange(i) if parents and i and rng.random() < 0.4 else None
        queries.append((kind, rng.randrange(archives), rng.randrange(0, 1 << 34, 0x800), parent,
                        rng.choice((0, 1, 2, 5))))
    return queries


def strict_weak(m):
    n = len(m)
    for a in range(n):
        if m[a][a]:
            return f"{a} < {a}"
        for b in range(n):
            if m[a][b] and m[b][a]:
                return f"{a} < {b} < {a}"
            for c in range(n):
                if m[a][b] and m[b][c] and not m[a][c]:
                    return f"{a} < {b} < {c}, not {a} < {c}"
                ia, ib = not m[a][b] and not m[b][a], not m[b][c] and not m[c][b]
                if ia and ib and (m[a][c] or m[c][a]):
                    return f"{a} ~ {b} ~ {c}, not {a} ~ {c}"
    return None


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    patched, _ = base.build(original, "section", code, None, late_join)
    rng = random.Random(20261017)
    checks = 0

    def say(text):
        nonlocal checks
        checks += 1
        print(f"  ok    {text}")

    for delta in (0,):                  # the exe is never rebased (the Rust arena is absolute)
        rebased = " (rebased)" if delta else ""
        old, new = Image(original, delta), Image(patched, delta)
        cycles = 0
        for trial in range(120):
            queries = random_set(rng, rng.randrange(3, 13), rng.choice((2, 3)))
            matrices = []
            for image in (old, new):
                addresses = image.load(queries)
                matrices.append([[image.less(a, b) for b in addresses] for a in addresses])
            fault = strict_weak(matrices[1])
            assert fault is None, (trial, fault, queries)
            cycles += strict_weak(matrices[0]) is not None
        assert cycles, "the original never failed on a mixed set"
        say(f"mixed sets{rebased}: the build's predicate is a strict weak ordering on all 120 random sets over two "
            f"and three archives, files in none and null iterators; the original's is not on {cycles} of them")

        # The game's sort on larger batches (past 16, where its loops run unguarded).
        faults = 0
        for trial in range(400):
            n = rng.randrange(17, 59)
            if trial % 2:
                queries = random_set(rng, n, rng.choice((2, 3)))
            else:              # reads of two archives, no parents, one priority (as a lobby's sound batch)
                queries = [("archive", rng.randrange(2), rng.randrange(0, 1 << 30, 0x800), None, 1) for _ in range(n)]
            addresses = new.load(queries)
            result, stray = new.sort(addresses)
            assert result is not None and not stray, (trial, stray)
            assert sorted(result) == sorted(addresses)
            assert all(not new.less(result[i + 1], result[i]) for i in range(len(result) - 1)), trial
            old.load(queries)
            result, stray = old.sort(addresses)
            faults += result is None or bool(stray)
        assert faults, "the original's sort never left its array on the random batches"
        say(f"the game's sort{rebased} with the build's predicate, 400 random batches of 17 to 58 queries (half of them "
            f"reads of two archives alone): it compares only the array's queries and leaves them sorted; the "
            f"original's left its array on {faults} of them")
        # A batch found by a random search (seed 6, the 27th): 26 reads of two
        # archives, no parents, one priority. The original's sort walks off
        # the front of its array and calls the predicate with the 0 before
        # it, as in the round 4d dump.
        old.load(LEFT_ITS_ARRAY)
        result, stray = old.sort(old.load(LEFT_ITS_ARRAY))
        assert result is None or 0 in stray, (result, stray)
        addresses = new.load(LEFT_ITS_ARRAY)
        result, stray = new.sort(addresses)
        assert result is not None and not stray and sorted(result) == sorted(addresses)
        assert all(not new.less(result[i + 1], result[i]) for i in range(len(result) - 1))
        say(f"26 reads of two archives, no parents, one priority{rebased}: the original's sort reads the 0 before its array as a query "
            "(the round 4d crash); the build's sorts it inside the array")

        # One archive (or files in none) where different priorities share a parent: alike, pair by pair.
        same = 0
        for kinds in (("archive",), ("loose",), ("archive", "null"), ("loose", "null")):
            for trial in range(40):
                # Either one priority (any parents) or no parents (any priorities).
                one_priority = trial % 2 == 0
                queries = random_set(rng, rng.randrange(2, 12), 1, kinds=kinds, parents=one_priority)
                if one_priority:
                    queries = [(k, a, o, p, 1) for k, a, o, p, _ in queries]
                pairs = []
                for image in (old, new):
                    addresses = image.load(queries)
                    pairs.append([image.less(a, b) for a in addresses for b in addresses])
                assert pairs[0] == pairs[1], (kinds, queries)
                same += 1
        say(f"one archive, or files in none{rebased}: on {same} sets whose queries of different priorities share "
            "their parent, the build answers as the original for every pair")

        # Two reads of one archive: by priority, then in file order, as the original.
        for priorities, offsets, expected in (((1, 1), (0x800, 0x1000), 1), ((1, 1), (0x1000, 0x800), 0),
                                              ((2, 1), (0x1000, 0x800), 1), ((1, 2), (0x800, 0x1000), 0)):
            for image in (old, new):
                addresses = image.load([("archive", 0, offsets[i], None, priorities[i]) for i in range(2)])
                assert image.less(*addresses) == expected, (priorities, offsets)
        say(f"two reads of one archive{rebased}: the higher priority first, then the lower file offset, in both")

    print(f"patch 33: {checks} checks passed")


if __name__ == "__main__":
    main()
