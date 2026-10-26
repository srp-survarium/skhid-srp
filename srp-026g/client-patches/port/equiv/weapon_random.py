"""The equivalence spec of patch 18, the weapons' random state at a spawn
(equiv_026g.py; weapon_random_hook at 0xE80 of the late join's code before
the Rust port, rust/patches/src/weapon_random.rs after).

The hook: the respawn rule's spawn 0x316DA0 at 0x316EA0, `mov edx,[esi]` /
`mov eax,[edx+20h]` (ESI the new kit's player, EBP the spawn's time). For
each of the 23 slots of the player's inventory (+10Ch, its +10Ch..), an
item whose vtable's +70h is weapon_core's bind 0x2E30F0 gets the state of
its place (+114h), kit (+115h), slot and the time at item+5ACh; then the two
instructions. No game function is called.

Live after the run (0x316EA5 `push 1 / mov ecx,esi / call eax`, then the
spawn's own end): every register. The asm keeps all of them but EDX and
EAX, which the two instructions load (pushad/popad, then them); the lohook
keeps all and replays them.

Inputs: players with no inventory, and inventories whose slots hold
nothing, weapons (the game's item vtables of weapon_core 0xA1114C and of the
client's weapon 0xA0422C, and a made-up one whose +70h is the bind), other
items (the game's ammunition item vtable, a made-up vtable, one whose +70h
is one off the bind), place and kit bytes and times at random and at their
edges; seeded random states.
"""
import random

import equiv_026g as E

PLAYER = 0x20000000
INVENTORY = 0x20001000
ARMS = 0x20010000                     # 23 objects of 0x1000 each
FAKE_VTABLES = 0x20030000
BIND = 0x2E30F0
WEAPON_ITEM_VTABLES = (0xA1114C, 0xA0422C)       # weapon_core's item, the client's weapon's item
PLAIN_ITEM_VTABLE = 0xA1395C                     # an ammunition item's (check_late_join_026g.py's)

LIVE = ("eax", "ecx", "edx", "ebx", "ebp", "esi", "edi")
HOOKS = [E.Hook("a spawn", 0x316EA0, LIVE)]
EXTERNALS = {}


def ranges(P, linked):
    """Where the patch's code is in a build: weapon_random_hook in the late
    join's code, or (the port) the patches crate's block."""
    if hasattr(P, "WEAPON_RANDOM_HOOK"):
        return [(P.LATE_JOIN_VA + 0xE80, P.LATE_JOIN_VA + 0xF00)]
    block = linked.blocks["patches.code"]
    return [(block.va, block.va + block.size)]


def entries(P, linked):
    if hasattr(P, "WEAPON_RANDOM_HOOK"):
        return [P.LATE_JOIN_VA + 0xE80]
    return [linked.symbols["weapon_random_on_spawn"]]


def dword(value):
    return (value & 0xFFFFFFFF).to_bytes(4, "little")


def regs(rng, player=PLAYER):
    out = {r: rng.getrandbits(32) for r in ("eax", "ecx", "edx", "ebx", "edi")}
    time = rng.choice([0, 50, 21750, 148800, 729200, 0xFFFFFFCE, 0xFFFFFFFF, rng.getrandbits(32)])
    out.update(esi=player, ebp=time, eflags=rng.choice([0x202, 0x246, 0x283]))
    return out


def world(rng, kinds, place=None, kit=None, inventory=True):
    """A player with an inventory whose slots are `kinds` (23 of: None, a
    weapon vtable, another vtable)."""
    place = rng.choice([0, 1, 8, 19, rng.randrange(256)]) if place is None else place
    kit = rng.choice([0, 1, 2, rng.randrange(256)]) if kit is None else kit
    memory = {PLAYER + 0x10C: dword(INVENTORY if inventory else 0), PLAYER + 0x114: bytes([place, kit])}
    fakes = {"weapon": FAKE_VTABLES, "other": FAKE_VTABLES + 0x100, "near": FAKE_VTABLES + 0x200}
    memory[FAKE_VTABLES + 0x70] = dword(BIND)
    memory[FAKE_VTABLES + 0x170] = dword(rng.getrandbits(32))
    memory[FAKE_VTABLES + 0x270] = dword(BIND + 1)
    slots = b""
    for slot, kind in enumerate(kinds):
        if kind is None:
            slots += dword(0)
            continue
        item = ARMS + slot * 0x1000 + 0x10
        vtable = fakes.get(kind, kind)
        memory[item] = dword(vtable)
        memory[item + 0x5AC] = dword(rng.getrandbits(32))
        slots += dword(item)
    memory[INVENTORY + 0x10C] = slots
    return memory


def kinds(rng, weapons=0.4):
    choices = [*WEAPON_ITEM_VTABLES, "weapon"]
    others = [PLAIN_ITEM_VTABLE, "other", "near", None]
    return [rng.choice(choices) if rng.random() < weapons else rng.choice(others) for _ in range(23)]


def cases(seed, count):
    rng = random.Random(seed)
    stack = lambda: [rng.getrandbits(32) for _ in range(16)]
    hook = HOOKS[0]
    fixed = [
        ("no inventory", lambda: world(rng, [None] * 23, inventory=False)),
        ("an empty inventory", lambda: world(rng, [None] * 23)),
        ("every slot a weapon", lambda: world(rng, [rng.choice(WEAPON_ITEM_VTABLES) for _ in range(23)])),
        ("weapons in slots 7 and 10 (the checks' kits)",
         lambda: world(rng, [WEAPON_ITEM_VTABLES[i == 10] if i in (7, 10) else
                             (PLAIN_ITEM_VTABLE if i == 8 else None) for i in range(23)])),
        ("the last slot a weapon, place 255, kit 255",
         lambda: world(rng, [None] * 22 + ["weapon"], place=255, kit=255)),
        ("items one off the bind", lambda: world(rng, ["near"] * 23)),
    ]
    for what, make in fixed:
        for _ in range(10):
            yield hook, E.Case(what, regs(rng), stack(), make())
    for _ in range(max(0, count - 10 * len(fixed))):
        yield hook, E.Case("random", regs(rng), stack(), world(rng, kinds(rng, rng.random())))
