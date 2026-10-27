"""The equivalence spec of patch 33, the archive reads' order (equiv_026g.py;
read_order_026g.asm over hdd_manager_sorting_predicate's body before the
Rust port, rust/patches/src/read_order.rs, a lohook at its start, after).

The hook: the predicate's entry 0x10BCC0 (EAX the first query_result,
[esp+4] the second, the return address on top): both answer EAX 0 or 1
and `ret 4` to the return address (a made-up R). Live there: EAX, EBX, ESI,
EDI, EBP and ESP; ECX and EDX are not: the original predicate writes both
on every path (0x10BCCE `mov ecx,[ebx+0A4h]`, 0x10BCDE `mov edx,[esi+..]`),
so none of its fifteen callers in the sort's helpers relies on them.

The game functions: get_physical_path (0x4BC10, EDI the path's
buffer_string, [esp+4] the node; cdecl) stubbed: the path's text pointer
written at the buffer's +0, a text of the case's for that node;
strcmp_s (0x3B5A0) the game's own; get_file_offs (0x47160, ECX the node;
EDX:EAX) stubbed from the case's offsets.
"""
import random
import struct

import equiv_026g as E

QUERIES = 0x20000000             # query_result, 0x400 each
NODES = 0x20100000               # vfs nodes, 0x40 each
TEXTS = 0x20200000               # the paths' texts, 0x40 each
RETURN = 0x10BE60                # the predicate's return address: an address of the game past its body
GET_PATH, STRCMP, GET_OFFS = 0x4BC10, 0x3B5A0, 0x47160
PATHS = [b"resources.db", b"resources.db.patch_3", b"resources.db.patch_10", b"", b"resources.db"]


def get_path(machine, args, rng):
    node = args[0]
    text = machine.case_paths.get(node)
    machine.write32(machine.reg("edi"), 0 if text is None else text)


def get_offs(machine, args, rng):
    offset = machine.case_offsets.get(args[0], rng.getrandbits(64))
    machine.set_reg("eax", offset & 0xFFFFFFFF)
    machine.set_reg("edx", offset >> 32)


EXTERNALS = {
    GET_PATH: E.External("get_physical_path", ["arg0"], pops=0, model=get_path),
    STRCMP: E.External("strcmp_s", ["eax:str", "ecx:str"], pops=0, real=True),
    GET_OFFS: E.External("get_file_offs", ["ecx"], pops=0, model=get_offs),
}
PREDICATE = E.Hook("the predicate", 0x10BCC0, ("eax", "ebx", "esi", "edi", "ebp"))
HOOKS = [PREDICATE]


def ranges(P, linked):
    if "read_order_bin" in dir(P):
        return [(0x10BCC0, 0x10BCC0 + 0x1A0)]
    blocks = [b for b in linked.blocks.values() if b.name == "patches.code"
              or b.kind == "code" and b.va <= linked.symbols.get("memset", 0) < b.va + b.size]
    return [(b.va, b.va + b.size) for b in blocks]


def entries(P, linked):
    if "read_order_bin" in dir(P):
        return [0x10BCC0]
    return [linked.symbols["read_order_on_predicate"]]


class Case(E.Case):
    def __init__(self, what, regs, stack, memory, paths, offsets):
        super().__init__(what, regs, stack, memory)
        self.paths, self.offsets = paths, offsets


def install():
    if getattr(E.Machine, "_read_order", False):
        return
    run = E.Machine.run

    def wrapped(self, hook, case, seed, old, **more):
        self.case_paths, self.case_offsets = case.paths, case.offsets
        return run(self, hook, case, seed, old, **more)
    E.Machine.run, E.Machine._read_order = wrapped, True


install()


def dword(value):
    return (value & 0xFFFFFFFF).to_bytes(4, "little")


def pair_case(rng, what, a, b):
    """a, b: dicts of a query (iterator, file, in_archive_iterator, in_archive_file, priority, parent, path,
    offset); a node index None for none."""
    memory, paths, offsets = {}, {}, {}
    for i, text in enumerate(PATHS):
        memory[TEXTS + 0x40 * i] = text + b"\0"
    queries = []
    for k, q in enumerate((a, b)):
        at = QUERIES + 0x400 * k if not q.get("same") else QUERIES
        queries.append(at)
        body = bytearray(E.made_up(rng, 0x2C0))
        nodes = {}
        for role in ("iterator", "file"):
            if q.get(role) is None:
                struct.pack_into("<I", body, 0xA4 if role == "iterator" else 0xA8, 0)
                continue
            node = NODES + 0x40 * (2 * k + (role == "file"))
            nodes[role] = node
            struct.pack_into("<I", body, 0xA4 if role == "iterator" else 0xA8, node)
            flags = rng.getrandbits(8) & ~4 | (4 if q.get(f"{role}_archive") else 0)
            memory[node] = E.made_up(rng, 0x30) + bytes([flags]) + E.made_up(rng, 0xF)
            paths[node] = TEXTS + 0x40 * q.get("path", 0) if rng.random() < 0.97 else None
            offsets[node] = q.get("offset", rng.getrandbits(64))
        struct.pack_into("<I", body, 0x158, q.get("parent", 0))
        struct.pack_into("<I", body, 0x2B8, q.get("priority", 0))
        if not q.get("same"):
            memory[at] = bytes(body)
    regs = {"eax": queries[0], "ecx": rng.getrandbits(32), "edx": rng.getrandbits(32), "ebx": rng.getrandbits(32),
            "esi": rng.getrandbits(32), "edi": rng.getrandbits(32), "ebp": rng.getrandbits(32),
            "eflags": rng.choice([0x202, 0x246, 0x283])}
    stack = [RETURN, queries[1]] + [rng.getrandbits(32) for _ in range(14)]
    return PREDICATE, Case(what, regs, stack, memory, paths, offsets)


def random_query(rng):
    return {"iterator": rng.choice([None, 1, 1, 1]), "file": rng.choice([None, 1]),
            "iterator_archive": rng.random() < 0.7, "file_archive": rng.random() < 0.7,
            "priority": rng.choice([0, 0, 1, 2, rng.getrandbits(32)]),
            "parent": rng.choice([0, 0, QUERIES + 0x800, rng.getrandbits(32)]),
            "path": rng.randrange(len(PATHS)), "offset": rng.choice([0, 0x1000, 0x2000, rng.getrandbits(64)])}


def cases(seed, count):
    rng = random.Random(seed)
    base = {"iterator": 1, "file": 1, "iterator_archive": True, "file_archive": True, "priority": 1, "parent": 0,
            "path": 0, "offset": 0x1000}
    variants = [("same archive, offsets", {}, {"offset": 0x2000}), ("equal", {}, {}),
                ("other archive", {}, {"path": 1}), ("priority", {"priority": 2}, {}), ("priority b", {}, {"priority": 3}),
                ("a null iterator", {"iterator": None}, {}), ("both null", {"iterator": None}, {"iterator": None}),
                ("b null", {}, {"iterator": None}), ("a loose", {"file_archive": False}, {}),
                ("both loose", {"file_archive": False, "parent": 5}, {"file_archive": False, "parent": 3}),
                ("loose, no parents", {"file_archive": False}, {"file_archive": False}),
                ("no file node", {"file": None}, {"file": None, "iterator_archive": False}),
                ("high offsets", {"offset": 1 << 40}, {"offset": (1 << 40) + 1}), ("empty path", {"path": 3}, {}),
                ("the same query", {}, {"same": True}),
                ("loose, the same parent", {"file_archive": False, "parent": 7}, {"file_archive": False, "parent": 7}),
                ("the same query, loose", {"file_archive": False}, {"same": True})]
    for what, a, b in variants:
        for _ in range(4):
            yield pair_case(rng, what, {**base, **a}, {**base, **b})
    for _ in range(max(0, count - 4 * len(variants))):
        yield pair_case(rng, "random pair", random_query(rng), random_query(rng))
