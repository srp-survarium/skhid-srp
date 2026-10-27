"""The equivalence spec of the replay viewer's full sound queue (README 24 to
26, its 25; equiv_026g.py): 20 bytes of machine code patch_026g.py wrote
into int3 padding at 0x835A20 before the Rust port, the Rust patch
replay_viewer's lohook weapon_sounds.added after.

The hook: delayed_callbacks_queue::add after its drain, 0x2454C1 `mov
eax,[esi] / mov ecx,[esi+4]` (ESI the queue: begin, end, capacity end). The
old code: ECX the end, against the capacity end; at or past it 0x24569A,
else EAX the begin and 0x2454C6. The port: Continue (the two loads
replayed, then 0x2454C6), or ECX the end and the exit 0x24569A. Live at
0x2454C6: EAX, ECX, EBX, ESI, EDI, EBP (EDX: `xor edx,edx` there). At
0x24569A: ECX, EBX, ESI, EDI, EBP; EAX is not (0x24569A loads EDI, then
the functor's call path loads EAX at 0x2456C0 before any read), nor EDX,
nor the flags (0x2456A1 `cmp` sets them).
"""
import random

import equiv_026g as E

QUEUE = 0x20000000
ADDED = E.Hook("added", 0x2454C1, ("eax", "ecx", "ebx", "esi", "edi", "ebp"),
               live_at={0x24569A: ("ecx", "ebx", "esi", "edi", "ebp")})
HOOKS = [ADDED]
EXTERNALS = {}
CAVE = (0x835A20, 0x835A34)


def ranges(P, linked):
    if "SOUND_QUEUE_CAVE" in dir(P):
        return [CAVE]
    blocks = [b for b in linked.blocks.values() if b.name == "patches.code"]
    return [(b.va, b.va + b.size) for b in blocks]


def entries(P, linked):
    if "SOUND_QUEUE_CAVE" in dir(P):
        return [CAVE[0]]
    return [linked.symbols["replay_viewer_on_sound_added"]]


def case(rng, what, begin, end, capacity):
    regs = {r: rng.getrandbits(32) for r in ("eax", "ecx", "edx", "ebx", "edi", "ebp")}
    regs["esi"] = QUEUE
    regs["eflags"] = rng.choice([0x202, 0x246, 0x283])
    memory = {QUEUE: begin.to_bytes(4, "little") + end.to_bytes(4, "little") + capacity.to_bytes(4, "little")}
    return ADDED, E.Case(what, regs, [rng.getrandbits(32) for _ in range(16)], memory)


def cases(seed, count):
    rng = random.Random(seed)
    begin = 0x30000000
    for n in range(0, 66):
        yield case(rng, f"{n} items of 64", begin, begin + 0x38 * n, begin + 0x38 * 64)
    for end, capacity in ((0xFFFFFFF0, 0x10), (0x10, 0xFFFFFFF0), (0, 0), (0xFFFFFFFF, 0xFFFFFFFF)):
        yield case(rng, f"end {end:#x} capacity {capacity:#x}", begin, end, capacity)
    for _ in range(max(0, count - 70)):
        yield case(rng, "random queue", rng.getrandbits(32), rng.getrandbits(32), rng.getrandbits(32))
