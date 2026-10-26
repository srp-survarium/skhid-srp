"""The equivalence spec of patch 38's battery body (equiv_026g.py; spot_026g.asm
at 0x7C0 of `.srp` before the Rust port, rust/patches/src/spot.rs after).

The hook: base_player::apply_new_input's hit with no player behind it,
0x2D8A0F `mov ecx,[esi]; mov ecx,[ecx+0Ch]` (ESI the hit's object pointer,
EDI the spotter). The asm reads the owner; a victory_item_core body owner
(vtable 0xA13EEC) has its battery's spottable interface at owner-4 asked
through that interface's vtable (slot 1 can_be_spotted_by, slot 0
set_spotted, thiscall, `ret 4`, the spotter pushed) and leaves to 0x2D888F
(spotted) or 0x2D8A8F (not); any other owner goes on at 0x2D8A14 with ECX
the owner.

Live after the run:
- the other owner's way (0x2D8A14): EBX, ESI, EDI, EBP and ECX (the owner,
  which 0x2D8A14 reads);
- a battery's ways: EBX, EDI, EBP. ESI, EAX, ECX and EDX are dead at both
  exits (0x2D888F: `xor esi,esi`, then EAX, ECX, EDX loaded before read;
  0x2D8A8F: EBX and EAX loaded from the frame, the loop's 0x2D8820 loads
  ECX, ESI and EDX before reading them; its end jumps to 0x2D888F), as
  spot_026g.asm's own comment says ("esi is free on both ways out").

Inputs: the battery's interface with the game's own vtable 0xA13EE0 (the
real can_be_spotted_by 0x301CA0, `mov al,1`; set_spotted stubbed: the
statistics and the team mask are its own business, it is called with the
same arguments or not at all), with a made-up vtable whose two slots are
stubs answering yes or no (al random), owners of other vtables (the game's
player spottable 0xA10B64, random ones), and seeded random states.
"""
import random

import equiv_026g as E

HIT = 0x20000000                  # the hit's object pointer (ESI)
OBJECT = 0x20001000               # the collision object
ITEM = 0x20002000                 # a victory_item_core: +54h the spottable, +58h the body owner
OWNER = ITEM + 0x58
SPOTTABLE = ITEM + 0x54
STUB_VTABLE = 0x20003000          # a made-up spottable vtable: slot 0, slot 1
SET_STUB, CAN_STUB = 0x20004000, 0x20004010
BODY_OWNER_VTABLE, ITEM_SPOTTABLE_VTABLE = 0xA13EEC, 0xA13EE0
CAN_BE_SPOTTED, SET_SPOTTED = 0x301CA0, 0x301C40


def answer(machine, args, rng):
    machine.set_reg("eax", rng.getrandbits(32) & ~0xFF | rng.choice([0, 1, 1, 0x80]))


EXTERNALS = {
    CAN_BE_SPOTTED: E.External("can_be_spotted_by", ["ecx", "arg0"], pops=4, real=True),
    SET_SPOTTED: E.External("set_spotted", ["ecx", "arg0"], pops=4),
    CAN_STUB: E.External("slot 1 (a made-up can_be_spotted_by)", ["ecx", "arg0"], pops=4, model=answer),
    SET_STUB: E.External("slot 0 (a made-up set_spotted)", ["ecx", "arg0"], pops=4),
}
OTHER = E.Hook("another owner", 0x2D8A0F, ("ebx", "esi", "edi", "ebp", "ecx"))
BATTERY = E.Hook("a battery's body", 0x2D8A0F, ("ebx", "edi", "ebp"))
HOOKS = [OTHER, BATTERY]


def ranges(P, linked):
    """Where the patch's code is in a build: spot_026g.asm in `.srp`, or
    (the port) the patches crate's block."""
    if "spot_bin" in dir(P):
        return [(P.SECTION_VA + 0x7C0, P.SECTION_VA + 0x800)]
    block = linked.blocks["patches.code"]
    return [(block.va, block.va + block.size)]


def entries(P, linked):
    if "spot_bin" in dir(P):
        return [P.SECTION_VA + 0x7C0]
    return [linked.symbols["spot_on_hit_owner"]]


def dword(value):
    return value.to_bytes(4, "little")


def regs(rng):
    return {"esi": HIT, "edi": rng.getrandbits(32), "ebx": rng.getrandbits(32), "ebp": rng.getrandbits(32),
            "eax": rng.getrandbits(32), "ecx": rng.getrandbits(32), "edx": rng.getrandbits(32),
            "eflags": rng.choice([0x202, 0x246, 0x283])}


def world(owner, owner_vtable, spottable_vtable=None):
    memory = {HIT: dword(OBJECT), OBJECT + 0xC: dword(owner), owner: dword(owner_vtable)}
    if spottable_vtable is not None:
        memory[SPOTTABLE] = dword(spottable_vtable)
        memory[STUB_VTABLE] = dword(SET_STUB) + dword(CAN_STUB)
    return memory


def cases(seed, count):
    rng = random.Random(seed)
    stack = lambda: [rng.getrandbits(32) for _ in range(16)]
    fixed = [
        (BATTERY, "the game's battery: can_be_spotted_by yes", world(OWNER, BODY_OWNER_VTABLE, ITEM_SPOTTABLE_VTABLE)),
        (BATTERY, "a battery with made-up slots", world(OWNER, BODY_OWNER_VTABLE, STUB_VTABLE)),
        (OTHER, "the game's player spottable as the owner", world(OWNER, 0xA10B64)),
        (OTHER, "another owner (0xA13EE0)", world(OWNER, ITEM_SPOTTABLE_VTABLE)),
        (OTHER, "an owner one past the body's vtable", world(OWNER, BODY_OWNER_VTABLE + 4)),
    ]
    for hook, what, memory in fixed:
        for _ in range(10):
            yield hook, E.Case(what, regs(rng), stack(), memory)
    for i in range(max(0, count - 50)):
        if i % 2:
            vtable = rng.choice([ITEM_SPOTTABLE_VTABLE, STUB_VTABLE])
            owner = OWNER if rng.random() < 0.8 else 0x20005000 + 4 * rng.randrange(64)
            memory = world(owner, BODY_OWNER_VTABLE, vtable)
            if owner != OWNER:
                memory[owner - 4] = dword(vtable)
            yield BATTERY, E.Case("random battery", regs(rng), stack(), memory)
        else:
            vtable = rng.choice([rng.getrandbits(32), 0xA10B64, BODY_OWNER_VTABLE ^ (1 << rng.randrange(32))])
            yield OTHER, E.Case("random owner", regs(rng), stack(), world(OWNER, vtable))
