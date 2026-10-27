"""The equivalence spec of patch 20, a weapon's state that a respawn kept
(equiv_026g.py; kit_state_hook at 0xC60 of the late join's code before the
Rust port, rust/patches/src/kit_state.rs after).

The hook: the respawn rule's spawn 0x316DA0 at 0x316E57, `mov edx,[esi]` /
`mov edx,[edx+1Ch]` (ESI the new kit's player; the spawn's [esp+10h] the
kit before, [esp+18h] the new one). For each of the 23 slots of the
player's inventory, an item whose vtable's +70h is weapon_core's bind
0x2E30F0 gets -1 at weapon+5A4h and, when the two kit bytes differ, the
constructor's values of KIT_STATE_FIELDS; then the two instructions. No
game function is called.

Live after the run (0x316E5C `sub esp,8`, the initialize's arguments and
its `call edx`): every register. The asm keeps all of them but EDX, which
the two instructions load; the lohook keeps all and replays them. (The x87
stack's fldz across the site is not compared here: neither touches the
FPU, and the lohook entry restores its whole state.) The asm writes the
bytes +5B1h/+5B2h and +5E5h/+5E6h as one word each, the port as two bytes:
the gate compares the final bytes.

Inputs: as port/equiv/weapon_random.py's, the stack's two kit bytes equal,
different, and at random (their dwords' other bytes random).
"""
import importlib.util
from pathlib import Path
import random

import equiv_026g as E

# The same spawn's player, inventory and weapons as weapon_random's spec.
_shared = importlib.util.spec_from_file_location("equiv_weapon_random_shared",
                                                 Path(__file__).with_name("weapon_random.py"))
W = importlib.util.module_from_spec(_shared)
_shared.loader.exec_module(W)
ARMS, WEAPON_ITEM_VTABLES, PLAIN_ITEM_VTABLE, kinds, regs, world = \
    W.ARMS, W.WEAPON_ITEM_VTABLES, W.PLAIN_ITEM_VTABLE, W.kinds, W.regs, W.world

LIVE = ("eax", "ecx", "edx", "ebx", "ebp", "esi", "edi")
HOOKS = [E.Hook("a spawn", 0x316E57, LIVE)]
EXTERNALS = {}


def ranges(P, linked):
    """Where the patch's code is in a build: kit_state_hook in the late
    join's code, or (the port) the patches crate's block."""
    if hasattr(P, "KIT_STATE_HOOK"):
        return [(P.LATE_JOIN_VA + 0xC60, P.LATE_JOIN_VA + 0xD00)]
    block = linked.blocks["patches.code"]
    return [(block.va, block.va + block.size)]


def entries(P, linked):
    if hasattr(P, "KIT_STATE_HOOK"):
        return [P.LATE_JOIN_VA + 0xC60]
    return [linked.symbols["kit_state_on_spawn"]]


def stack(rng, before, new):
    words = [rng.getrandbits(32) for _ in range(16)]
    words[4] = words[4] & ~0xFF | before
    words[6] = words[6] & ~0xFF | new
    return words


def used(rng, memory):
    """Every weapon's kept fields as a last life leaves them (random)."""
    for slot in range(23):
        item = ARMS + slot * 0x1000 + 0x10
        if item in memory:
            memory[item - 0x10 + 0x5A4] = E.made_up(rng, 0x6C)
    return memory


def cases(seed, count):
    rng = random.Random(seed)
    hook = HOOKS[0]
    kit = lambda: rng.choice([0, 1, 2, rng.randrange(256)])
    fixed = [
        ("no inventory", lambda: world(rng, [None] * 23, inventory=False)),
        ("an empty inventory", lambda: world(rng, [None] * 23)),
        ("every slot a weapon", lambda: world(rng, [rng.choice(WEAPON_ITEM_VTABLES) for _ in range(23)])),
        ("weapons in slots 7 and 10 (the checks' kits)",
         lambda: world(rng, [WEAPON_ITEM_VTABLES[i == 10] if i in (7, 10) else
                             (PLAIN_ITEM_VTABLE if i == 8 else None) for i in range(23)])),
        ("the last slot a weapon", lambda: world(rng, [None] * 22 + ["weapon"])),
        ("items one off the bind", lambda: world(rng, ["near"] * 23)),
    ]
    for what, make in fixed:
        for same in (True, False):
            for _ in range(5):
                before = kit()
                new = before if same else (before + rng.randrange(1, 256)) & 0xFF
                yield hook, E.Case(f"{what}, {'the same kit' if same else 'another kit'}", regs(rng),
                                   stack(rng, before, new), used(rng, make()))
    for _ in range(max(0, count - 10 * len(fixed))):
        before = kit()
        new = before if rng.random() < 0.4 else kit()
        yield hook, E.Case("random", regs(rng), stack(rng, before, new),
                           used(rng, world(rng, kinds(rng, rng.random()))))
