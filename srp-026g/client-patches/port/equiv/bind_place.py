"""The equivalence spec of patch 14's bind of a place, with patch 11's ids
(equiv_026g.py; bind_slot_reserved at 0xB00 of the late join's code until
the Rust port, late_join.rs's bind_place after).

The hook: game_world_core's call of bind_place 0x321820 at 0x2D3880 (EAX the
core, the place pushed, bind_place pops it). The place is bound with a
counter of its own lent to the core's callback ids (+18FB0h, from
(place + 1) << 16, the core's own given back), the active objects' end
(+18B50h) at the place's six of the twenty places' 120 (then back where it
was, at least 120), and the game-object list (+18914h, +18918h) empty for
the bind, then the bound nodes in front of what it held. bind_place is
stubbed: it takes ids from the counter, puts weapons at the active
objects' end and appends nodes to the list, as many as the case's
generator says. Leaves past the call, its argument popped.

Live after the run: EBX, ESI, EDI, EBP (EAX, ECX and EDX are the call's).
"""
import random

import equiv_026g as E

CORE = 0x20000000
SLOT = 0x20100000
COUNTER = 0x20110000
OBJECTS = 0x20200000                 # the active objects' vector: 255 room
NODES = 0x20300000                   # the list's nodes, 0x10 each
BOUND = 0x20400000                   # the nodes bind_place makes
BIND = 0x321820


def bind(machine, args, rng):
    """bind_place: a few ids from the lent counter, a few weapons at the
    active objects' end, a few nodes appended to the list."""
    core, slot = args
    counter = machine.read32(core + 0x18FB0)
    machine.write32(counter, machine.read32(counter) + rng.randrange(0, 40))
    end = machine.read32(core + 0x18B50)
    for _ in range(rng.randrange(0, 7)):
        machine.write32(end, rng.getrandbits(32))
        end += 4
    machine.write32(core + 0x18B50, end)
    for i in range(rng.choice([0, 0, 1, 2, 5])):
        node = BOUND + i * 0x10
        last = machine.read32(core + 0x18918)
        machine.write32(node + 4, last)
        machine.write32(node + 8, 0)
        if last:
            machine.write32(last + 8, node)
        else:
            machine.write32(core + 0x18914, node)
        machine.write32(core + 0x18918, node)
    machine.write32(slot + 0x28, core)


EXTERNALS = {BIND: E.External("bind_place", ["eax", "arg0"], pops=4, model=bind)}
LIVE = ("ebx", "esi", "edi", "ebp")
HOOKS = [E.Hook("bind", 0x2D3880, LIVE)]


def ranges(P, linked):
    """bind_slot and bind_slot_reserved in the late join's code, or (the
    port) the patches crate's block and the engine library's thunks."""
    if hasattr(P, "LATE_JOIN_READY_OFFSET"):
        return [(P.LATE_JOIN_VA, P.LATE_JOIN_VA + 0x10), (P.LATE_JOIN_VA + 0xB00, P.LATE_JOIN_VA + 0xC00)]
    block = linked.blocks["patches.code"]
    return [(block.va, block.va + block.size)]


def entries(P, linked):
    if hasattr(P, "LATE_JOIN_READY_OFFSET"):
        return [P.LATE_JOIN_VA, P.LATE_JOIN_VA + 0xB00]
    return [linked.symbols["late_join_on_bind_place"]]


def dword(value):
    return (value & 0xFFFFFFFF).to_bytes(4, "little")


def cases(seed, count):
    rng = random.Random(seed)
    hook = HOOKS[0]
    for i in range(count):
        place = rng.choice([0, 1, 2, 8, 19, rng.randrange(20)])
        memory = {SLOT + 0x2C: bytes([place]), COUNTER: dword(rng.choice([0, 73, 4096, rng.getrandbits(32)])),
                  # the pages the stub writes (a model's writes map no page)
                  OBJECTS: rng.randbytes(0x1000), BOUND: bytes(0x100)}
        filled = rng.choice([0, 1, 119, 120, 121, 130, 255, rng.randrange(256)])
        memory[CORE + 0x18B4C] = dword(OBJECTS) + dword(OBJECTS + 4 * filled)
        memory[OBJECTS] = memory[OBJECTS][:4 * filled] + bytes(0x1000 - 4 * filled)
        memory[CORE + 0x18FB0] = dword(COUNTER)
        held = rng.choice([0, 0, 1, 2, 4])
        nodes = [NODES + n * 0x10 for n in range(held)]
        for n, node in enumerate(nodes):
            memory[node + 4] = dword(nodes[n - 1] if n else 0) + dword(nodes[n + 1] if n + 1 < held else 0)
        memory[CORE + 0x18914] = dword(nodes[0] if nodes else 0) + dword(nodes[-1] if nodes else 0)
        regs = {r: rng.getrandbits(32) for r in ("ecx", "edx", "ebx", "esi", "edi")}
        regs.update(eax=CORE, ebp=CORE, eflags=rng.choice([0x202, 0x246, 0x283]))
        stack = [SLOT] + [rng.getrandbits(32) for _ in range(15)]
        yield hook, E.Case(f"place {place}, {held} nodes held, {filled} objects", regs, stack, memory)
