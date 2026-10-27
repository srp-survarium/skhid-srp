"""The equivalence spec of patch 31's keys, a mouse button held from before
the start is not the player's until it is let go (equiv_026g.py;
warmup_keys_026g.asm at 0x9A8 of `.srp`, its state a dword at 0xE30 of the
late join's state page, before the Rust port; rust/patches/src/warmup_keys.rs
after).

The hook: the game world's on_mouse_key_action 0x25CF60 at its start (ECX
the game world's input handler, its cursor shown at +2E0h; [esp+4] the
input world, [esp+8] the key's code, [esp+0Ch] its kind). The original's:
the ten bytes (`push ebp` / `push esi` / `mov esi,ecx` / `mov ecx,[esp+0Ch]`
/ `mov eax,[ecx]`) and on at 0x25CF6A, the asm's own copy of them, the
port's replay. Taken: `ret 0Ch` with EAX the button + 1. Live at 0x25CF6A:
EAX, ECX, EBX, ESI, EDI, EBP (EDX is not: 0x25CF6A `mov edx,[eax+24h]`);
at the return: EAX, EBX, ESI, EDI, EBP. The marked buttons compare by place.
"""
import random
import struct

import equiv_026g as E

HANDLER, WORLD = 0x20000000, 0x21000000
FIRST = 0x151
OLD_STATE = 0xA428E30
EXTERNALS = {}
KEY = E.Hook("mouse key", 0x25CF60, ("eax", "ecx", "ebx", "esi", "edi", "ebp"),
             live_at={})


class Returns(dict):
    """The live registers at a return (any VA but 0x25CF6A): the caller's."""

    def get(self, at, default=None):
        return default if at == 0x25CF6A else ("eax", "ebx", "esi", "edi", "ebp")


KEY.live_at = Returns()
HOOKS = [KEY]


def ranges(P, linked):
    if "warmup_keys_bin" in dir(P):
        return [(P.WARMUP_KEYS_VA, P.WARMUP_KEYS_VA + len(open(P.warmup_keys_bin(), "rb").read()))]
    blocks = [b for b in linked.blocks.values() if b.name == "patches.code"]
    return [(b.va, b.va + b.size) for b in blocks]


def entries(P, linked):
    if "warmup_keys_bin" in dir(P):
        return [P.WARMUP_KEYS_VA]
    return [linked.symbols["warmup_keys_on_mouse_key"]]


def places(P, linked):
    if "warmup_keys_bin" in dir(P):
        return {"marked": (OLD_STATE, 4)}
    return {"marked": (next(va for name, va, _, _ in linked.every if "warmup_keys" in name and "MARKED" in name), 4)}


def case(rng, what, code, kind, shown, marked):
    regs = {r: rng.getrandbits(32) for r in ("eax", "edx", "ebx", "esi", "edi", "ebp")}
    regs["ecx"] = HANDLER
    regs["eflags"] = rng.choice([0x202, 0x246, 0x283])
    memory = {HANDLER + 0x2E0: bytes([shown]), WORLD: struct.pack("<I", rng.getrandbits(32)),
              "marked+0x0": struct.pack("<I", marked)}
    stack = [rng.getrandbits(32), WORLD, code & 0xFFFFFFFF, kind & 0xFFFFFFFF] + [rng.getrandbits(32) for _ in range(12)]
    return KEY, E.Case(what, regs, stack, memory)


def cases(seed, count):
    rng = random.Random(seed)
    out = []
    for button in range(8):
        for kind in (0, 1, 2, 3, 0x100, 0x101, 0x1FF):
            for shown in (0, 1, 0x80):
                for marked in (0, 1 << button, 0xFF, 0xFF & ~(1 << button), rng.getrandbits(32)):
                    out.append(case(rng, f"button {button} kind {kind:#x} shown {shown} marked {marked:#x}",
                                    FIRST + button, kind, shown, marked))
    for code in (0, 0x10, 0x150, 0x159, 0x15A, 0xFFFFFFFF, FIRST + 0x100000000 - 1, FIRST - 1 + 0x100000000):
        for kind in (0, 1, 2):
            out.append(case(rng, f"not a button {code:#x}", code, kind, rng.choice([0, 1]), rng.getrandbits(8)))
    for c in out[:max(count, len(out))]:
        yield c
    for _ in range(max(0, count - len(out))):
        yield case(rng, "random", rng.choice([FIRST + rng.randrange(8), rng.getrandbits(32), FIRST + rng.randrange(-4, 12)]),
                   rng.choice([0, 1, 2, rng.getrandbits(32)]), rng.choice([0, 1, rng.getrandbits(8)]),
                   rng.getrandbits(32))
