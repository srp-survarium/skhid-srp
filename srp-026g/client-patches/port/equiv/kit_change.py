"""The equivalence spec of patch 31, a kit chosen before the start is played
at once (equiv_026g.py; kit_change_026g.asm at 0xEC0 of `.srp` before the
Rust port, rust/patches/src/kit_change.rs after).

- tick, 0x316820: the respawn rule's tick at its start (ECX the rule,
  [esp+8] the tick's time). Before play, each place to respawn: the
  player's damage model's holder (vtable+8), the player out of the world
  (vtable+24h, 1), the rule's spawn (vtable+5Ch: the place, a 24h-byte
  point compared by its bytes, the time). Then the prologue, on at 0x316826
  (live ECX, EDX, EBX, ESI, EDI, EBP; EAX is written there first,
  `mov eax,18h` at 0x31682B).
- view, 0x26794D: `push 1` / `call 1F8200h` (ESI the network client, EAX
  the game world). In play the player camera (the asm's own call, the
  port's replay); before, switch_to_warmup_camera (EDX the world; the asm's
  tail jump, the port's call). On at 0x267954 (the asm's `ret` lands on
  the nops at 0x267952: alias); live EBX, ESI, EDI, EBP (0x267954 `mov
  eax,[esp+10h]`; the calls leave ECX and EDX theirs).
- alive, 0x23B7DA: show_stamina (AL, EDX), then the kits shown or hidden by
  the camera's state (show_profiles_selector: EAX the ui, the flag's low
  byte), on at 0x23B7FC (live EBX, ESI, EDI, EBP: `lea eax`, `or edx,-1`,
  `mov ecx,eax` there).
- update, 0x23B7FC: a live local player's kits by the state, then `lea
  eax,[ebp+0D0h]`, on at 0x23B802 (the asm's `ret` on the nop at 0x23B801:
  alias); live EAX, EBX, ESI, EDI, EBP.

Every pointer the cases give is set (the asm reads through them unchecked,
the port takes a null one as nothing to do).
"""
import random
import struct

import equiv_026g as E

RULE, RVT, CORE, PLACES, STATS = 0x20000000, 0x20000100, 0x20100000, 0x20200000, 0x20300000
PLAYERS = 0x20400000                    # 0x2000 a player
PVT, HVT = 0x20380000, 0x20380100
CLIENT, WORLD, GAME, UI, LOCAL = 0x20500000, 0x20600000, 0x20700000, 0x20800000, 0x20900000
STUBS = 0x30000000
REMOVE, SPAWN, HOLDER = STUBS, STUBS + 0x10, STUBS + 0x20
PLAYER_CAMERA, WARMUP_CAMERA, STAMINA, SELECTOR = 0x1F8200, 0x1F8190, 0x23A1A0, 0x2383B0
OLD = 0xA425EC0


def clobber(machine):
    machine.set_reg("ecx", machine.rng.getrandbits(32))
    machine.set_reg("edx", machine.rng.getrandbits(32))


def holder(machine, args, rng):
    """The damage model's holder: &the damage model (the player's +2E0h)."""
    machine.set_reg("eax", args[0] - 0x108 + 0x2E0)
    clobber(machine)


low_byte = lambda machine: machine.read32(machine.reg("esp") + 4) & 0xFF
EXTERNALS = {
    REMOVE: E.External("remove_from_world", ["ecx", "arg0"], pops=4, model=lambda m, a, r: clobber(m)),
    SPAWN: E.External("spawn", ["ecx", "arg0", "arg1:mem:36", "arg2"], pops=12, model=lambda m, a, r: clobber(m)),
    HOLDER: E.External("damage_model_ptr", ["ecx"], pops=0, model=holder),
    PLAYER_CAMERA: E.External("switch_to_player_camera", ["eax", "arg0"], pops=4, model=lambda m, a, r: clobber(m)),
    WARMUP_CAMERA: E.External("switch_to_warmup_camera", ["edx"], pops=0, model=lambda m, a, r: clobber(m)),
    STAMINA: E.External("show_stamina", ["eax", "edx"], pops=0, model=lambda m, a, r: clobber(m)),
    SELECTOR: E.External("show_profiles_selector", ["eax", low_byte], pops=4, model=lambda m, a, r: clobber(m)),
}
TICK = E.Hook("tick", 0x316820, ("ecx", "edx", "ebx", "esi", "edi", "ebp"))
VIEW = E.Hook("view", 0x26794D, ("ebx", "esi", "edi", "ebp"), aliases={0x267952: 0x267954}, tails=(WARMUP_CAMERA,))
ALIVE = E.Hook("alive", 0x23B7DA, ("ebx", "esi", "edi", "ebp"))
UPDATE = E.Hook("update", 0x23B7FC, ("eax", "ebx", "esi", "edi", "ebp"), aliases={0x23B801: 0x23B802})
HOOKS = [TICK, VIEW, ALIVE, UPDATE]


def old(P):
    return "kit_change_bin" in dir(P)


def ranges(P, linked):
    if old(P):
        return [(OLD, OLD + len(open(P.kit_change_bin(), "rb").read()))]
    blocks = [b for b in linked.blocks.values() if b.name in ("patches.code", "engine.code")]
    return [(b.va, b.va + b.size) for b in blocks]


def entries(P, linked):
    if old(P):
        return [OLD + at for at in (0, 0xE1, 0xF9, 0x108)]
    return [linked.symbols[f"kit_change_on_{n}"] for n in ("tick", "kit_view", "alive", "update")]


def regs(rng, **fixed):
    out = {r: rng.getrandbits(32) for r in ("eax", "ecx", "edx", "ebx", "esi", "edi", "ebp")}
    out["eflags"] = rng.choice([0x202, 0x246, 0x283])
    out.update(fixed)
    return out


def dword(value):
    return struct.pack("<I", value & 0xFFFFFFFF)


def tick_case(rng, what, state, places, time, stats=True):
    """`places`: per place (kit, next_kit, [player or None per kit], dirty
    record) with a player (alive, in world, invulnerable until)."""
    memory = {RULE: dword(RVT), RVT + 0x5C: dword(SPAWN), RULE + 0x134: dword(CORE),
              CORE + 0x18FE4: dword(state), CORE + 0x18458: dword(PLACES),
              CORE + 0x1845C: dword(PLACES + 0x30 * len(places)), CORE + 0x18FA8: dword(STATS if stats else 0),
              PVT + 0x24: dword(REMOVE), HVT + 8: dword(HOLDER)}
    n = 0
    for i, (kit, next_kit, players, dirty) in enumerate(places):
        at = PLACES + 0x30 * i
        pointers = []
        for player in players:
            if player is None:
                pointers.append(0)
                continue
            alive, in_world, until = player
            p = PLAYERS + 0x2000 * n
            n += 1
            memory[p] = dword(PVT)
            memory[p + 0x108] = dword(HVT)
            memory[p + 0x33C] = bytes([alive, rng.getrandbits(8), rng.getrandbits(8), in_world])
            memory[p + 0x2E0] = dword(p + 0x1000)
            memory[p + 0x1000 + 0x32C] = dword(until)
            memory[p + 0x43C] = struct.pack("<3f", *(rng.uniform(-500, 500) for _ in range(3)))
            memory[p + 0x490] = struct.pack("<f", rng.uniform(-3.2, 3.2))
            pointers.append(p)
        memory[at] = b"".join(dword(p) for p in pointers) + bytes([kit, next_kit]) + bytes(rng.getrandbits(8)
                                                                                          for _ in range(0x22))
        record = bytearray(0x1F8)
        if dirty:
            word = rng.randrange(0x198 // 4)
            struct.pack_into("<I", record, 4 * word, rng.getrandbits(32) | 1)
        for w in range(0x198 // 4, 0x1F8 // 4):        # past the clean part: anything
            struct.pack_into("<I", record, 4 * w, rng.getrandbits(32))
        memory[STATS + 8 + 0x1F8 * i] = bytes(record)
    stack = [rng.getrandbits(32), rng.getrandbits(32), time, rng.getrandbits(32)] + [rng.getrandbits(32) for _ in range(12)]
    return TICK, E.Case(what, regs(rng, ecx=RULE), stack, memory)


def ui_memory(state):
    return {UI + 0x18: dword(WORLD), WORLD + 0x9C: dword(GAME), GAME + 0x53AC: dword(CLIENT),
            CLIENT + 0x1F1C60: dword(state)}


def view_case(rng, what, state):
    return VIEW, E.Case(what, regs(rng, esi=CLIENT, eax=WORLD), [rng.getrandbits(32) for _ in range(16)],
                        {CLIENT + 0x1F1C60: dword(state)})


def alive_case(rng, what, state):
    return ALIVE, E.Case(what, regs(rng, esi=UI, ebp=LOCAL, eax=rng.getrandbits(24) << 8 | 1, edx=UI),
                         [rng.getrandbits(32) for _ in range(16)], ui_memory(state))


def update_case(rng, what, state, alive):
    memory = ui_memory(state)
    memory[LOCAL + 0x33C] = bytes([alive])
    return UPDATE, E.Case(what, regs(rng, esi=UI, ebp=LOCAL), [rng.getrandbits(32) for _ in range(16)], memory)


def player(alive=1, in_world=1, until=0xFFFFFFFF):
    return alive, in_world, until


def cases(seed, count):
    rng = random.Random(seed)
    out = []
    t = 50000
    fixed = [
        ("a switch", 1, [(0, 1, [player(), player(), None], False)]),
        ("a switch in the countdown", 2, [(1, 2, [player(), player(), player()], False)]),
        ("in play", 3, [(0, 1, [player(), player(), None], False)]),
        ("state 0", 0, [(0, 1, [player(), player(), None], False)]),
        ("state 4", 4, [(0, 1, [player(), player(), None], False)]),
        ("the same kit", 1, [(1, 1, [player(), player(), None], False)]),
        ("next kit 3", 1, [(0, 3, [player(), player(), player()], False)]),
        ("next kit 255", 1, [(0, 255, [player(), player(), player()], False)]),
        ("no player for the next kit", 1, [(0, 2, [player(), player(), None], False)]),
        ("dead", 1, [(0, 1, [player(alive=0), player(), None], False)]),
        ("not in the world", 1, [(0, 1, [player(in_world=0), player(), None], False)]),
        ("statistics", 1, [(0, 1, [player(), player(), None], True)]),
        ("protected", 1, [(0, 1, [player(until=t + 1500), player(), None], False)]),
        ("protected to the time", 1, [(0, 1, [player(until=t), player(), None], False)]),
        ("protected past", 1, [(0, 1, [player(until=t - 1), player(), None], False)]),
        ("protected a tick", 1, [(0, 1, [player(until=t + 1), player(), None], False)]),
        ("twenty places", 1, [(rng.randrange(3), rng.randrange(3), [player(), player(), player()], rng.random() < 0.3)
                              for _ in range(20)]),
        ("no places", 1, []),
    ]
    for what, state, places in fixed:
        for _ in range(3):
            out.append(tick_case(rng, what, state, places, t))
    for state in (0, 1, 2, 3, 4, 0x103):
        for _ in range(3):
            out.append(view_case(rng, f"view, state {state}", state))
            out.append(alive_case(rng, f"alive, state {state}", state))
            for alive in (0, 1, 0x80):
                out.append(update_case(rng, f"update, state {state}, alive {alive}", state, alive))
    yield from out
    for i in range(max(0, count - len(out))):
        kind = i % 4
        if kind == 0:
            n = rng.randrange(0, 21)
            time = rng.getrandbits(32)
            places = []
            for _ in range(n):
                players = [rng.choice([None, player(alive=rng.choice([0, 1, 1]), in_world=rng.choice([0, 1, 1]),
                                                     until=rng.choice([0xFFFFFFFF, time, time + rng.randrange(1, 9000),
                                                                       time - rng.randrange(1, 9000),
                                                                       rng.getrandbits(32)]))])
                           for _ in range(3)]
                kit = rng.randrange(3)
                if players[kit] is None:
                    players[kit] = player()
                places.append((kit, rng.choice([0, 1, 2, rng.getrandbits(8)]), players, rng.random() < 0.3))
            yield tick_case(rng, "random tick", rng.choice([0, 1, 2, 2, 3]), places, time)
        elif kind == 1:
            yield view_case(rng, "random view", rng.choice([0, 1, 2, 3, rng.getrandbits(32)]))
        elif kind == 2:
            yield alive_case(rng, "random alive", rng.choice([1, 2, 3, rng.getrandbits(32)]))
        else:
            yield update_case(rng, "random update", rng.choice([1, 2, 3, rng.getrandbits(32)]), rng.choice([0, 1]))
