"""The equivalence spec of patch 67, the kill feed's icons (equiv_026g.py;
kill_feed_026g.asm in the arena's block kill_feed.code before the Rust port,
rust/patches/src/kill_feed.rs after).

The hooks:
- the event, generate_killed_event's 0x2DA3D1 `mov [eax+4],bl / mov
  edx,[esi+120h]` (EAX the node, BL the cause icon, ESI the victim, EBP the
  bullet or 0, [esp+14h] the melee flag): the asm was called there (INLINE,
  four nops after) and returned to 0x2DA3D6; the port is a lohook whose
  Continue replays the two and jumps to 0x2DA3DA. Both runs end at
  0x2DA3DA (`call 0x2D0A60`, the queueing). Live there: EAX, EDX (the
  queueing's inputs), EBX, ESI, EDI, EBP. ECX is dead: the asm left it
  clobbered, the port keeps it; 0x2D0A60 writes ECX before it reads it on
  both its ways (0x2D0A69 `mov ecx,[edx+12750h]`, 0x2D0A7B `mov
  ecx,[edx+12744h]`), and after the call 0x2DA3E5 loads it again;
- the row, on_player_killed's 0x23814F `mov edx,[esp+30h] / mov
  ecx,[esp+2Ch]` (after extra_icon): the asm called there (three nops
  after) and returned to 0x238154; the port replays the two to 0x238157.
  Both runs end at 0x238157. Live there: EBX, ESI, EDI, EBP, ECX and EDX
  (the two moves). EAX is dead: the asm left SetMember's answer there, the
  port keeps it; 0x238157..0x238178 (`and edx,8Fh` .. `mov eax,[ecx]`)
  write it before any read.

The game functions: the part look-up 0x2E58B0 (EAX the parts, ECX the
address of a name's pointer; compared by the name), stubbed (a part of the
name or none, from the run's generator) or the game's own on made-up part
lists (check_kill_feed_026g.py's layout); SetMember through the
description's interface (thiscall: data, name, &value, is a display object;
`ret 10h`; compared by the name and the value's type and data), at the
game's own vtable 0x8CE2A8 (slot +14h 0x677000, stubbed) or at a made-up
one.
"""
import random
import struct

import equiv_026g as E

NODE, VICTIM, MODEL, BULLET = 0x20000000, 0x20001000, 0x20002000, 0x20003000
PARTS = {b"pain": 0x20004000, b"body": 0x20004100, b"head": 0x20004200}
TEXTS = 0x20005000                      # the real look-up's names
IFACE, VTABLE, STUB = 0x20006000, 0x20006100, 0x20006200
GAME_VTABLE, SET_MEMBER = 0x8CE2A8, 0x677000
FIND_PART = 0x2E58B0


def bits(value):
    return struct.unpack("<I", struct.pack("<f", value))[0]


HEALTHS = [0, 0, bits(0.5), bits(1.0), bits(1e-7), 0x80000000, bits(-1.0), 0x7FC00000, 0xFFC00000, 1]


def name_of(machine):
    """The look-up's name: the text its ECX points to the pointer of."""
    return machine.text(machine.read32(machine.reg("ecx")))


def member_name(machine):
    return machine.text(machine.read32(machine.reg("esp") + 8))


def value_word(offset):
    return lambda machine: machine.read32(machine.read32(machine.reg("esp") + 12) + offset)


def answer(machine, args, rng):
    """A part of the name, or none."""
    machine.set_reg("eax", 0 if rng.random() < 0.15 else PARTS[args[1]])


LOOKUP = E.External("find_body_part", ["eax", name_of], model=answer)
REAL_LOOKUP = E.External("find_body_part", ["eax", name_of], real=True)
SET = ["ecx", "arg0", member_name, "arg2", value_word(4), value_word(8), "arg3"]
EXTERNALS = {FIND_PART: LOOKUP, STUB: E.External("SetMember (made up)", SET, pops=16),
             SET_MEMBER: E.External("SetMember (the game's slot)", SET, pops=16)}
EVENT_LIVE = ("eax", "edx", "ebx", "esi", "edi", "ebp")
EVENT = E.Hook("the event, the look-up stubbed", 0x2DA3D1, EVENT_LIVE, until=0x2DA3DA)
EVENT_REAL = E.Hook("the event, the game's look-up", 0x2DA3D1, EVENT_LIVE, {FIND_PART: REAL_LOOKUP},
                    until=0x2DA3DA)
ROW = E.Hook("the row", 0x23814F, ("ebx", "esi", "edi", "ebp", "ecx", "edx"), until=0x238157)
HOOKS = [EVENT, EVENT_REAL, ROW]


def ranges(P, linked):
    """Where the patch's code is in a build: kill_feed_026g.asm's block, or
    (the port) the patches crate's block."""
    for name in ("kill_feed.code", "patches.code"):
        if name in linked.blocks:
            block = linked.blocks[name]
            return [(block.va, block.va + block.size)]


def entries(P, linked):
    if "kill_feed.code" in linked.blocks:
        return [linked.symbols["kill_feed_event"], linked.symbols["kill_feed_row"]]
    return [linked.symbols["kill_feed_on_cause_icon"], linked.symbols["kill_feed_on_mastery_icon"]]


def dword(value):
    return (value & 0xFFFFFFFF).to_bytes(4, "little")


def regs(rng, **fixed):
    out = {r: rng.getrandbits(32) for r in E.REG_NAMES}
    out["eflags"] = rng.choice([0x202, 0x246, 0x283, 0xA97])
    out.update(fixed)
    return out


def event_case(rng, what, bullet, cause, melee, segments, reflections, model, healths, real=False, order=None):
    memory = {NODE: E.made_up(rng, 0x30),
              VICTIM + 0x2E0: dword(MODEL if model else 0), VICTIM + 0x120: dword(rng.getrandbits(32)),
              BULLET + 0x84: dword(segments), BULLET + 0x88: bytes([reflections])}
    for name, at in PARTS.items():
        memory[at + 0xA0] = dword(healths.get(name, rng.choice(HEALTHS)))
    if real:
        previous = 0
        names = order or [b"infection", b"body", b"head", b"pain", b"left_leg"]
        for i, name in reversed(list(enumerate(names))):     # [+0] next, [+80h] the name, [+0A0h] health
            node = PARTS.get(name, 0x20007000 + 0x100 * i)
            memory[TEXTS + 0x20 * i] = name + b"\0"
            memory[node] = dword(previous)
            memory[node + 0x80] = dword(TEXTS + 0x20 * i)
            previous = node
        memory[MODEL + 0x108 + 8] = dword(previous)
    stack = [rng.getrandbits(32) for _ in range(24)]
    stack[5] = stack[5] & ~0xFF | melee
    r = regs(rng, eax=NODE, esi=VICTIM, ebp=BULLET if bullet else 0)
    r["ebx"] = r["ebx"] & ~0xFF | cause
    return (EVENT_REAL if real else EVENT), E.Case(what, r, stack, memory)


def row_case(rng, what, flags, kind, game_vtable):
    stack = [rng.getrandbits(32) for _ in range(0x44)]
    stack[0x2C // 4] = IFACE
    stack[0x30 // 4] = kind
    stack[0x104 // 4] = rng.getrandbits(32) & ~0xFF00 | flags << 8
    memory = {IFACE: dword(GAME_VTABLE if game_vtable else VTABLE), VTABLE + 0x14: dword(STUB)}
    return ROW, E.Case(what, regs(rng), stack, memory)


def cases(seed, count):
    rng = random.Random(seed)
    empty, full = 0, bits(1.0)
    # check_kill_feed_026g.py's cases: (healths, segments, reflections, cause, extra)
    checks = [({b"body": empty, b"pain": bits(0.3)}, 0, 0, 0, {}),
              ({b"pain": empty, b"body": bits(0.6), b"head": bits(0.4)}, 0, 0, 0, {}),
              ({b"pain": empty, b"body": empty}, 0, 0, 0, {}),
              ({b"pain": empty, b"head": empty, b"body": bits(0.7)}, 0, 0, 0, {}),
              ({b"pain": empty, b"head": empty}, 0, 0, 1, {}),
              ({b"pain": empty, b"head": bits(0.1)}, 0, 0, 1, {}),
              ({b"pain": bits(1e-7), b"body": empty}, 0, 0, 0, {}),
              ({b"body": empty}, 1, 0, 0, {}), ({b"body": empty}, 1, 1, 0, {}), ({b"body": empty}, 3, 1, 0, {}),
              ({b"pain": empty, b"body": bits(0.5)}, 2, 2, 0, {}), ({b"body": empty}, 1, 1, 3, {}),
              ({b"body": empty}, 0, 0, 0, {"melee": 1}), ({b"body": empty}, 2, 1, 0, {"melee": 1}),
              ({b"pain": empty, b"body": bits(0.4)}, 1, 0, 0, {"melee": 1}),
              ({b"body": empty}, 1, 0, 3, {}), ({b"head": empty}, 1, 0, 1, {}),
              ({b"pain": empty, b"body": bits(0.2)}, 2, 0, 0, {}),
              ({b"pain": empty}, 5, 0, 0, {"bullet": False}), ({b"pain": empty}, 1, 0, 0, {"model": False}),
              ({b"pain": empty}, 0, 0, 0, {"order": [b"pain", b"head", b"body"]}),
              ({b"pain": empty}, 0, 0, 0, {"order": [b"pain", b"body"]})]
    for healths, segments, reflections, cause, extra in checks:
        healths = {name: healths.get(name, full) for name in PARTS}
        for real in (False, True):
            yield event_case(rng, "a check case", extra.get("bullet", True), cause, extra.get("melee", 0), segments,
                             reflections, extra.get("model", True), healths, real, extra.get("order"))
    for flags in range(8):
        for kind in (0x0A, 0x4A, 0x8A, 0x08, 0x0B, 0x8B):
            yield row_case(rng, f"row flags {flags} type {kind:#x}", flags, kind, rng.random() < 0.5)
    for i in range(max(0, count - 92)):
        if i % 3 == 2:
            yield row_case(rng, "random row", rng.getrandbits(8),
                           rng.choice([0x0A, 0x4A, 0x8A, rng.getrandbits(32)]), rng.random() < 0.5)
        else:
            healths = {name: rng.choice(HEALTHS + [rng.getrandbits(32)]) for name in PARTS}
            yield event_case(rng, "random event", rng.random() < 0.8, rng.choice([0, 0, 1, 3, rng.getrandbits(8)]),
                             rng.choice([0, 0, 0, 1, rng.getrandbits(8)]), rng.choice([0, 0, 1, 2, rng.getrandbits(32)]),
                             rng.choice([0, 0, 1, rng.getrandbits(8)]), rng.random() < 0.9, healths, i % 3 == 1,
                             rng.choice([None, [b"pain", b"head", b"body"], [b"pain", b"body"], [b"head"]]))
