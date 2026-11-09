"""The equivalence spec of patch 19, a first world's inputs and kit choices
in time order (equiv_026g.py; first_load_input at 0xF00 and first_load_kit
at 0xF50 of the late join's code until the Rust port,
rust/patches/src/first_world.rs after).

The hooks: game_world_core::deserialize's first-world block, its call of
0x2D4AB0 at 0x2D1050 (the current player's input) and of 0x2D4B50 at
0x2D1055 (the world's next kit), EDI -> {core, the world's time, place}.
Each finds the place's list entry of that time, from the newest end, or
takes a node of the list's pool (allocate_node 0x1FCB60, stubbed: a fresh
node from the case's pool, as the game's gives one unlinked) and links it
(insert_before 0x2D4850, the game's own code) before the oldest newer entry
(at the newest end with none), then writes the value. Leaves by the return
(the asm) or the jump (the lohook) to the instruction after the call.

Live after the run: EBX, ESI, EDI, EBP (the asm keeps them; EAX, ECX and
EDX are a call's to clobber; the lohook keeps all).

Inputs: places 0..19 whose slot's current kit holds a player (input words
at random) and whose next kit is any byte; lists of 0 to 8 entries in time
order, the world's time before, between, equal to and after theirs, at
the u32 edges; seeded random states.
"""
import random

import equiv_026g as E

CORE = 0x20000000
SLOTS = 0x20100000                   # the places vector: 48 bytes a place
INPUTS = 0x20200000                  # the places' input lists: 480h a place
KITS = 0x20300000                    # their kit choice lists
PLAYERS = 0x20400000                 # 0x1000 a player
NODES = 0x20500000                   # the lists' entries, 0x40 each
POOL = 0x20600000                    # the nodes allocate_node gives
ENTRY = 0x20700000                   # deserialize's record (in its frame in the game)

ALLOCATE, INSERT = 0x1FCB60, 0x2D4850


def allocate(machine, args, rng):
    """allocate_node: the next node of the case's pool, unlinked."""
    node = machine.read32(POOL) or POOL + 0x100
    machine.write32(POOL, node + 0x40)
    machine.uc.mem_write(node, bytes(0x40))
    machine.set_reg("eax", node)


EXTERNALS = {ALLOCATE: E.External("allocate_node", ["eax"], model=allocate),
             INSERT: E.External("insert_before", ["eax", "edx", "arg0"], real=True)}
LIVE = ("ebx", "esi", "edi", "ebp")
HOOKS = [E.Hook("input", 0x2D1050, LIVE), E.Hook("kit", 0x2D1055, LIVE)]


def ranges(P, linked):
    """Where the patch's code is in a build: first_load_input, first_load_kit
    and entry_at_time in the late join's code, or (the port) the patches
    crate's block and the engine library's thunks."""
    if hasattr(P, "FIRST_LOAD_INPUT_HOOK"):
        return [(P.LATE_JOIN_VA + 0xF00, P.LATE_JOIN_VA + 0x1000)]
    block = linked.blocks["patches.code"]
    return [(block.va, block.va + block.size)]


def entries(P, linked):
    if hasattr(P, "FIRST_LOAD_INPUT_HOOK"):
        return [P.LATE_JOIN_VA + 0xF00, P.LATE_JOIN_VA + 0xF50]
    return [linked.symbols["first_world_on_input"], linked.symbols["first_world_on_kit"]]


def dword(value):
    return (value & 0xFFFFFFFF).to_bytes(4, "little")


def place_memory(rng, place, times, time):
    """The core, the place's slot and player, its two lists holding entries
    at `times` (oldest first), the record at ENTRY."""
    memory = {CORE + 0x18458: dword(SLOTS), CORE + 0x2FCC: dword(INPUTS), CORE + 0x89D8: dword(KITS),
              POOL: dword(0)}
    slot = SLOTS + place * 48
    kit = rng.choice([0, 1, 2])
    player = PLAYERS + place * 0x1000
    players = [player if k == kit else rng.choice([0, PLAYERS + 0xF000]) for k in range(3)]
    memory[slot] = b"".join(dword(p) for p in players) + bytes([kit, rng.choice([0, 1, 2, rng.randrange(256)])])
    memory[player + 0x32C] = dword(rng.getrandbits(32)) + dword(rng.getrandbits(32))
    for lists in (INPUTS, KITS):
        head = lists + place * 0x480
        nodes = [NODES + (lists - INPUTS) // 0x100 * 0 + (place * 16 + i) * 0x40 + (0x10000 if lists == KITS else 0)
                 for i in range(len(times))]
        for i, (node, at) in enumerate(zip(nodes, times)):
            newer = nodes[i + 1] if i + 1 < len(nodes) else 0
            older = nodes[i - 1] if i else 0
            memory[node] = dword(newer) + dword(older) + dword(rng.getrandbits(32)) + dword(rng.getrandbits(32)) \
                + bytes(16) + dword(at)
        memory[head] = dword(len(nodes)) + dword(nodes[0] if nodes else 0) + dword(nodes[-1] if nodes else 0)
    memory[ENTRY] = dword(CORE) + dword(time) + bytes([place])
    return memory


def regs(rng):
    out = {r: rng.getrandbits(32) for r in ("eax", "ecx", "edx", "ebx", "esi", "ebp")}
    out.update(edi=ENTRY, eflags=rng.choice([0x202, 0x246, 0x283]))
    return out


def times_of(rng):
    count = rng.randint(0, 8)
    base = rng.choice([0, 50, 1000, 0x7FFFFF00, 0xFFFFFF00, rng.getrandbits(32)])
    out, at = [], base
    for _ in range(count):
        at = (at + rng.choice([1, 50, 50, 100])) & 0xFFFFFFFF
        out.append(at)
    return sorted(out)


def world_time(rng, times):
    picks = [0, 0xFFFFFFFF, rng.getrandbits(32)]
    if times:
        picks += [times[0] - 1 & 0xFFFFFFFF, times[-1] + 1 & 0xFFFFFFFF, rng.choice(times),
                  rng.choice(times) + 25 & 0xFFFFFFFF]
    return rng.choice(picks)


def cases(seed, count):
    rng = random.Random(seed)
    for i in range(count):
        hook = HOOKS[i % 2]
        place = rng.choice([0, 1, 2, 7, 19, rng.randrange(20)])
        times = times_of(rng)
        time = world_time(rng, times)
        # the caller's frame: values that name no code of ours (the gate takes
        # a jump to the game with our address at [esp] for a call)
        stack = [rng.getrandbits(27) for _ in range(16)]
        yield hook, E.Case(f"place {place}, {len(times)} entries", regs(rng), stack,
                           place_memory(rng, place, times, time))
