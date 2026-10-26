"""The equivalence spec of patch 23, a battery under the floor
(equiv_026g.py; pickup_below_026g.asm at 0x800 of `.srp` before the Rust
port, rust/patches/src/pickup_below.rs after).

base_player's use scan 0x2D84C0, its frame grown by 8 (both builds), ESP
the same at its three sites: the groups at [esp+0Ch], the `crossed` byte at
[esp+40h].
- reach, 0x2D84E9 `fld dword [0xA710D8]`: the asm (call + nop) clears the
  byte, pushes 2 x the distance (fld, fadd st0,st0) and returns onto the nop
  0x2D84EE (alias: the port's 0x2D84EF). Live: every register, the flags
  (neither touches them) and ST0 (its value and TOP).
- solid, 0x2D861C `and al,0Ah / cmp al,0Ah / jz`: AL &= 0Ah; both bits:
  crossed, the groups' word & FFF5h; then 0x2D8622 (the next hit). Live:
  every register; the flags are not (0x2D8622 `mov eax,[esp+18h]`, then
  `add` sets them).
- candidate, 0x2D85BE `mov eax,[ebp+8] / mov edx,[esi] / mov edx,[edx+30h]`
  (ESI the usable object, EDI the scan's copy of its position, [ebp+8] the
  player): within the distance of the eye (+47Ch) and nothing crossed, the
  three instructions and 0x2D85C6; else a battery (its vtable's +30h
  0x300710) whose get_transform (vtable+44h) centre is below the feet (+43Ch)
  and within 1 m across: EDI's position := the feet, and the same way on;
  else the groups & FF7Fh and 0x2D85FC. Live at 0x2D85C6: EAX, EDX, EBX,
  ESI, EDI, EBP; ECX is not (0x2D85CC `mov ecx,esi` before its use), nor
  the flags or the XMM registers (the call at 0x2D85CE). At 0x2D85FC: EBX,
  ESI, EDI, EBP; EAX, ECX, EDX are not (0x2D85FC `mov ecx,[esp+38h] / mov
  edx,[ecx] / mov eax,[edx+18h]`).
"""
import random
import struct

import equiv_026g as E

PLAYER = 0x20000000
OBJECT = 0x20010000              # the usable object; its vtable at +0x800
VTABLE = OBJECT + 0x800
POSITION = 0x20020000            # the scan's copy of the object's position (EDI)
GET_TRANSFORM = 0x31000000       # a stub at the vtable's +44h
OTHER_CAN_USE = 0x31000100
DISTANCE = 0xA710D8
ESP = E.STACK_TOP - 0x8000
FEET = (12.0, 10.0, -7.0)
EYE = (12.0, 11.7, -7.0)


def floats(*values):
    return struct.pack(f"<{len(values)}f", *values)


def transform(machine, args, rng):
    """get_transform(out): the case's centre at +30h..+38h of a matrix."""
    out = machine.read32(machine.reg("esp") + 4)
    centre = machine.case_centre
    machine.uc.mem_write(out, floats(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, *centre, 1))
    machine.set_reg("eax", out)


EXTERNALS = {GET_TRANSFORM: E.External("get_transform", ["ecx"], pops=4, model=transform)}
ALL = ("eax", "ecx", "edx", "ebx", "esi", "edi", "ebp")
REACH = E.Hook("reach", 0x2D84E9, ALL + ("eflags", "st0"), aliases={0x2D84EE: 0x2D84EF})
SOLID = E.Hook("solid", 0x2D861C, ALL)
CANDIDATE = E.Hook("candidate", 0x2D85BE, ("eax", "edx", "ebx", "esi", "edi", "ebp"),
                   live_at={0x2D85FC: ("ebx", "esi", "edi", "ebp")})
HOOKS = [REACH, SOLID, CANDIDATE]


def ranges(P, linked):
    if "pickup_bin" in dir(P):
        return [(P.SECTION_VA + 0x800, P.SECTION_VA + 0x800 + len(open(P.pickup_bin(), "rb").read()))]
    blocks = [b for b in linked.blocks.values() if b.name == "patches.code"
              or b.kind == "code" and b.va <= linked.symbols.get("memset", 0) < b.va + b.size]
    return [(b.va, b.va + b.size) for b in blocks]


def entries(P, linked):
    if "pickup_bin" in dir(P):
        return [P.SECTION_VA + 0x800, P.SECTION_VA + 0x820, P.SECTION_VA + 0x840]
    return [linked.symbols[f"pickup_below_on_{name}"] for name in ("reach", "solid", "candidate")]


def frame(rng, groups=None, crossed=None):
    stack = [rng.getrandbits(32) for _ in range(24)]
    if groups is not None:
        stack[3] = groups
    if crossed is not None:
        stack[0x10] = (stack[0x10] & ~0xFF) | crossed
    return stack


def regs(rng, **fixed):
    out = {r: rng.getrandbits(32) for r in ALL}
    out["eflags"] = rng.choice([0x202, 0x246, 0x283, 0xA93])
    out.update(fixed)
    return out


class Case(E.Case):
    def __init__(self, what, regs_, stack, memory, centre=(0.0, 0.0, 0.0)):
        super().__init__(what, regs_, stack, memory)
        self.centre = centre


def install():
    if getattr(E.Machine, "_pickup_below", False):
        return
    run = E.Machine.run

    def wrapped(self, hook, case, seed, old):
        self.case_centre = getattr(case, "centre", (0.0, 0.0, 0.0))
        return run(self, hook, case, seed, old)
    E.Machine.run, E.Machine._pickup_below = wrapped, True


install()


def candidate_case(rng, what, position, battery=True, crossed=0, centre=(0.0, 0.0, 0.0), feet=FEET, eye=EYE,
                   distance=2.0):
    memory = {PLAYER + 0x43C: floats(*feet), PLAYER + 0x47C: floats(*eye), POSITION: floats(*position),
              OBJECT: VTABLE.to_bytes(4, "little"),
              VTABLE + 0x30: (0x300710 if battery else OTHER_CAN_USE).to_bytes(4, "little"),
              VTABLE + 0x44: GET_TRANSFORM.to_bytes(4, "little"), DISTANCE: floats(distance)}
    ebp = 0x20030000
    memory[ebp + 8] = PLAYER.to_bytes(4, "little")
    return CANDIDATE, Case(what, regs(rng, esi=OBJECT, edi=POSITION, ebp=ebp), frame(rng, crossed=crossed), memory,
                           centre)


def cases(seed, count):
    rng = random.Random(seed)
    x, y, z = FEET
    nan, inf = float("nan"), float("inf")
    for distance in (2.0, 0.5, 1e30, nan, -3.0, 1.5e-45):
        for _ in range(4):
            yield REACH, E.Case(f"reach, distance {distance}", regs(rng), frame(rng, crossed=rng.getrandbits(8)),
                                {DISTANCE: floats(distance)})
    for al in (0x0A, 0x0B, 0x0E, 0x02, 0x08, 0x00, 0xFF, 0x80, 0x8A):
        for _ in range(3):
            yield SOLID, E.Case(f"solid, al {al:#x}", regs(rng, eax=rng.getrandbits(24) << 8 | al),
                                frame(rng, groups=rng.getrandbits(32), crossed=rng.getrandbits(8)), {})
    fixed = [("in view", (x + 1, y + 0.1, z), {}), ("out of reach", (x + 2.5, y + 0.4, z), {}),
             ("crossed, in view", (x + 1, y + 0.1, z), {"crossed": 1}),
             ("other, out of reach", (x + 3, y, z), {"battery": False}),
             ("other, crossed", (x, y, z), {"battery": False, "crossed": 1})]
    for across in (0.0, 0.6, 0.99, 1.0, 1.01, 1.5):
        fixed.append((f"under the floor, {across} across", (x + across, y - 0.2, z),
                      {"crossed": 1, "centre": (x + across, y - 0.3, z)}))
    fixed += [("centre above the feet", (x, y - 0.1, z), {"crossed": 1, "centre": (x, y + 0.05, z)}),
              ("centre at the feet", (x, y, z), {"crossed": 1, "centre": (x, y, z)}),
              ("a NaN centre", (x, y, z), {"crossed": 1, "centre": (nan, y - 0.3, z)}),
              ("a NaN depth", (x, y, z), {"crossed": 1, "centre": (x, nan, z)}),
              ("a NaN position", (nan, y, z), {}), ("an infinite eye", (x, y, z), {"eye": (inf, y, z)}),
              ("a NaN distance", (x + 1, y, z), {"distance": nan})]
    for what, position, kw in fixed:
        for _ in range(4):
            yield candidate_case(rng, what, position, **kw)
    r = lambda spread: rng.uniform(-spread, spread)
    for i in range(max(0, count - 200)):
        if i % 5 == 0:
            yield SOLID, E.Case("random solid", regs(rng), frame(rng, groups=rng.getrandbits(32),
                                                                 crossed=rng.getrandbits(8)), {})
        elif i % 5 == 1:
            yield REACH, E.Case("random reach", regs(rng), frame(rng), {DISTANCE: floats(rng.uniform(0, 5))})
        else:
            centre = (x + r(1.6), y + r(0.6), z + r(1.6))
            yield candidate_case(rng, "random candidate", (x + r(4), y + r(3), z + r(4)),
                                 battery=rng.random() < 0.7, crossed=rng.choice([0, 0, 1, 2]), centre=centre,
                                 distance=rng.choice([2.0, 2.0, rng.uniform(0.1, 4)]))
