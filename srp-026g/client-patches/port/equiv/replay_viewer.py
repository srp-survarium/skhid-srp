"""The equivalence spec of the replay viewer (README "24 to 26";
equiv_026g.py): its 26, replay_seek_026g.asm at 0xA00 of `.srp` before the
Rust port, rust/patches/src/replay_viewer.rs after.

26, the seek: 0x1EE3A3 `lea eax,[edi-3E8h]` (ESI the game, EDI the target
time) on a user seek. The asm walks [game+4D64h]'s places (+18650h begin,
+18654h end, 30h each), each place's three kits' players (+0, +4, +8), a
player's inventory (+10Ch) and its 23 slots (+10Ch, 4 each); an item whose
vtable is 0xA0422C (a client weapon's) gets 0x245C50 (EAX the queue's
begin, item+9ECh; pushed &end, item+9F0h; caller cleans), then end = begin
and the fx history's write index (+818h) = its oldest (+81Ch); then the
lea and 0x1EE3A9. Every register is live there (the asm's popad): EAX the
lea's, the others as at the site; the flags are not (0x1EE3A9 `mov`, `mov`,
`pop`, `pop`, `add esp,18h`).

Inputs: worlds of 0 to 6 places, a kit a player or none, a player an
inventory or none, a slot an item or none, an item a client weapon's or
another vtable (0xA04174, random), its fields random; the destroy stub
moves a weapon's begin at times (the asm reads it again after the call);
null games and worlds are not inputs (the asm faults on them).
"""
import random

import equiv_026g as E

GAME, WORLD, PLACES = 0x20000000, 0x20010000, 0x20020000
HEAP = 0x20100000
CLIENT_WEAPON_ITEM = 0xA0422C
DESTROY = 0x245C50


def destroy(machine, args, rng):
    """0x245C50 answered: at times it writes the weapon's begin (+9ECh, just
    below the &end it was given), which the asm reads again."""
    if rng.random() < 0.3:
        machine.write32(args[1] - 4, rng.getrandbits(32))


EXTERNALS = {DESTROY: E.External("delayed_callbacks_queue destroy_range", ["eax", "arg0"], pops=0, model=destroy)}
SEEK = E.Hook("the seek", 0x1EE3A3, ("eax", "ecx", "edx", "ebx", "esi", "edi", "ebp"))
HOOKS = [SEEK]


def ranges(P, linked):
    """Where the patch's code is in a build: replay_seek_026g.asm in `.srp`,
    or (the port) the patches crate's block."""
    if "seek_clear_bin" in dir(P):
        return [(P.SECTION_VA + 0xA00, P.SECTION_VA + 0xA97)]
    block = linked.blocks["patches.code"]
    return [(block.va, block.va + block.size)]


def entries(P, linked):
    if "seek_clear_bin" in dir(P):
        return [P.SECTION_VA + 0xA00]
    return [linked.symbols["replay_viewer_on_seek"]]


def dword(value):
    return (value & 0xFFFFFFFF).to_bytes(4, "little")


def regs(rng):
    return {"esi": GAME, "edi": rng.getrandbits(32), "ebx": rng.getrandbits(32) & ~0xFF,
            "ebp": rng.getrandbits(32), "eax": rng.getrandbits(32), "ecx": rng.getrandbits(32),
            "edx": rng.getrandbits(32), "eflags": rng.choice([0x202, 0x246, 0x283])}


def world(rng, places, kit_odds=0.7, weapon_odds=0.5):
    memory = {GAME + 0x4D64: dword(WORLD), WORLD + 0x18650: dword(PLACES),
              WORLD + 0x18654: dword(PLACES + 0x30 * places)}
    heap = HEAP
    for p in range(places):
        for k in range(3):
            if rng.random() > kit_odds:
                memory[PLACES + 0x30 * p + 4 * k] = dword(0)
                continue
            player, heap = heap, heap + 0x200
            memory[PLACES + 0x30 * p + 4 * k] = dword(player)
            if rng.random() < 0.1:
                memory[player + 0x10C] = dword(0)
                continue
            inventory, heap = heap, heap + 0x200
            memory[player + 0x10C] = dword(inventory)
            for s in range(23):
                if rng.random() < 0.5:
                    memory[inventory + 0x10C + 4 * s] = dword(0)
                    continue
                item, heap = heap, heap + 0x1000
                memory[inventory + 0x10C + 4 * s] = dword(item)
                vtable = CLIENT_WEAPON_ITEM if rng.random() < weapon_odds else \
                    rng.choice([0xA04174, rng.getrandbits(32), CLIENT_WEAPON_ITEM ^ 4])
                memory[item] = dword(vtable)
    return memory


def cases(seed, count):
    rng = random.Random(seed)
    stack = lambda: [rng.getrandbits(32) for _ in range(16)]
    fixed = [(0, "no places"), (1, "one place"), (3, "three places"), (6, "six places")]
    for places, what in fixed:
        for _ in range(10):
            yield SEEK, E.Case(what, regs(rng), stack(), world(rng, places))
    yield SEEK, E.Case("every kit a player, every slot a weapon", regs(rng), stack(), world(rng, 2, 1.0, 1.0))
    for _ in range(max(0, count - 41)):
        yield SEEK, E.Case("random world", regs(rng), stack(), world(rng, rng.randint(0, 5)))
