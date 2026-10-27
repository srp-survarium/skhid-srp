"""The equivalence spec of patch 54, the artefacts (equiv_026g.py;
artefacts_026g.asm, the arena's block artefacts.code, before the Rust port;
rust/patches/src/artefacts.rs after). Eight sites:

- loader 0x33AE13 (`mov dword [esi+48h],13h`; ESI the config, EBX its
  data): the three config numbers (the config lookup 0xF6B10 a stub on a
  made-up config) and the nodes written; on at 0x33AE1A (the asm saves
  XMM0; live every register, XMM0).
- link 0x311BC0 (EAX the Spring) and unlink 0x311DA0 ([esp+4]): the two
  nodes into or out of the holder's lists (the holder's get_player,
  vtable+0Ch, and the game's unlink 0x2D5D00 stubs; the lists real
  memory); on at 0x311BCA (live EAX, EBX, ESI, EDI, EBP: ECX and EDX are
  written there first) and 0x311DA5 (live EBX, ESI, EDI, EBP).
- lifebone 0x3122AA (EDI the Lifebone, [esp+14h] the time): pain, body,
  head through find_pool and increase_health (stubs); on at 0x3122B0 (live
  ECX, EBX, ESI, EDI, EBP).
- action 0x313300 (ECX the artefact; pressed, the time): a key-up the
  original's (on at 0x31330A, live ESI, EBX, EDI, EBP); a press fires
  (enable_active, vtable+80h, and the effect 0x313350 stubs; an Onyx's
  vtable+80h is 0x3128C0, the same stub), `ret 8` (every register).
- stagger 0x2DBD89 (ESI the player, XMM0 and [esp+14h] the sum; XMM1 0 as
  0x2DBD86 left it): apply_modifier (a stub setting XMM0's low dword), on at
  0x2DBD91 (live XMM0, XMM1, XMM2, ESI, EBX, EDI, EBP).
- onyx_hit 0x312C03 (ECX the Onyx): on at 0x312C0A (live ECX, EBX, ESI,
  EDI, EBP: EAX is loaded there, EDX never read again) or `ret 0Ch` (live
  EBX, ESI, EDI, EBP).
- onyx_hint, from 0x25F9DD (EBX the Onyx's client part; the asm's jump
  there, the port's lohook at 0x25F9E8 after the game's `push 0BB8h`): the
  run ends at 0x25F9F8, after the hint's show_parametrized_message (a stub
  where the game calls it, its time compared) or past it.
"""
import random
import struct

import equiv_026g as E

CONFIG, DATA, NODES = 0x20000000, 0x20000400, 0x20000800       # the config's data and its made-up nodes
SPRING, LIFEBONE, INVENTORY, HOLDER, HVT = 0x20100000, 0x20110000, 0x20200000, 0x20300000, 0x20300100
PLAYER, PROFILE, DM, POOLS = 0x20400000, 0x20500000, 0x20600000, 0x20700000
ARTEFACTS = 0x20800000                  # 0x2000 an artefact
VT_A, VT_B, VT_ONYX = 0x20900000, 0x20900200, 0x20900400
ONYX_CLIENT = 0xF20
STUBS = 0x30000000
GET_DM, GET_PLAYER, ENABLE = STUBS, STUBS + 0x10, STUBS + 0x20
CHILD, UNLINK, FIND, HEAL, EFFECT, GETTER, ONYX_ENABLE, SHOW = (0xF6B10, 0x2D5D00, 0x2E58B0, 0x32A420, 0x313350,
                                                                0x2D5AE0, 0x3128C0, 0x2375C0)
POOL_NAMES = ("pain", "body", "head", "arm")


def clobber(machine):
    machine.set_reg("ecx", machine.rng.getrandbits(32))
    machine.set_reg("edx", machine.rng.getrandbits(32))


def child(machine, args, rng):
    """The config lookup: the case's node of (table, key), else a fresh one."""
    key = args[1]
    machine.set_reg("eax", machine.case_config.get((args[0], key), NODES + 0x100 + 0x20 * (len(key) % 8)))
    clobber(machine)


def get_player(machine, args, rng):
    machine.set_reg("eax", PLAYER)
    clobber(machine)


def get_dm(machine, args, rng):
    machine.set_reg("eax", DM - 0x40)       # &the damage model (it holds DM)
    clobber(machine)


def find(machine, args, rng):
    name = machine.text(machine.read32(machine.reg("ecx")), 16).decode("latin-1")
    machine.set_reg("eax", machine.case_pools.get(name, 0))
    clobber(machine)


def getter(machine, args, rng):
    """apply_modifier: XMM0's low dword some function of B (the upper bits
    left)."""
    b = struct.unpack("<f", struct.pack("<I", args[2]))[0]
    value = struct.unpack("<I", struct.pack("<f", b * machine.case_factor))[0]
    machine.set_reg("xmm0", machine.uc.reg_read(__import__("unicorn").x86_const.UC_X86_REG_XMM0) & ~0xFFFFFFFF | value)
    clobber(machine)


mem12 = lambda machine: machine.read32(machine.reg("esp") + 4)
EXTERNALS = {
    CHILD: E.External("cfg_child", ["eax", "edi:str"], pops=0, model=child),
    GET_PLAYER: E.External("get_player", ["ecx"], pops=0, model=get_player),
    GET_DM: E.External("damage_model_ptr", ["ecx"], pops=0, model=get_dm),
    UNLINK: E.External("modifier_unlink", ["esi", "eax"], pops=0, model=lambda m, a, r: clobber(m)),
    FIND: E.External("find_pool", ["eax", lambda m: m.text(m.read32(m.reg("ecx")), 16)], pops=0, model=find),
    HEAL: E.External("increase_health", ["esi", "arg0", "arg1"], pops=8, model=lambda m, a, r: clobber(m)),
    ENABLE: E.External("enable_active", ["ecx", "arg0"], pops=4, model=lambda m, a, r: clobber(m)),
    ONYX_ENABLE: E.External("enable_active", ["ecx", "arg0"], pops=4, model=lambda m, a, r: clobber(m)),
    EFFECT: E.External("effect", ["arg0", "arg1"], pops=8, model=lambda m, a, r: clobber(m)),
    GETTER: E.External("apply_modifier", ["eax", "esi", "arg0", "arg1"], pops=8, model=getter),
    SHOW: E.External("show_parametrized_message", ["ecx", "arg0", "arg1", "arg2", "arg3"], pops=16,
                     model=lambda m, a, r: clobber(m), from_game=True),
}
ALL = ("eax", "ecx", "edx", "ebx", "esi", "edi", "ebp")
LOADER = E.Hook("loader", 0x33AE13, ALL + ("xmm0",))
LINK = E.Hook("link", 0x311BC0, ("eax", "ebx", "esi", "edi", "ebp"))
UNLINK_HOOK = E.Hook("unlink", 0x311DA0, ("ebx", "esi", "edi", "ebp"))
LIFE = E.Hook("lifebone", 0x3122AA, ("ecx", "ebx", "esi", "edi", "ebp"))


class Live(dict):
    def __init__(self, at, there, other):
        super().__init__()
        self.at, self.there, self.other = at, there, other

    def get(self, at, default=None):
        return self.there if at == self.at else self.other


ACTION = E.Hook("action", 0x313300, ALL)
ACTION.live_at = Live(0x31330A, ("esi", "ebx", "edi", "ebp"), ALL)
STAGGER = E.Hook("stagger", 0x2DBD89, ("xmm0", "xmm1", "xmm2", "esi", "ebx", "edi", "ebp"))
ONYX = E.Hook("onyx_hit", 0x312C03, ("ecx", "ebx", "esi", "edi", "ebp"))
ONYX.live_at = Live(0x312C0A, ("ecx", "ebx", "esi", "edi", "ebp"), ("ebx", "esi", "edi", "ebp"))
HINT = E.Hook("onyx_hint", 0x25F9DD, ("ebx", "ebp"), until=0x25F9F8)
HOOKS = [LOADER, LINK, UNLINK_HOOK, LIFE, ACTION, STAGGER, ONYX, HINT]


def old(P):
    return "ARTEFACTS_HOOKS" in dir(P)


def ranges(P, linked):
    names = ("artefacts.code", "engine.code") if old(P) else ("patches.code", "engine.code")
    return [(b.va, b.va + b.size) for b in linked.blocks.values() if b.name in names]


def entries(P, linked):
    if old(P):
        return [linked.symbols[s] for _, _, s, _ in P.ARTEFACTS_HOOKS]
    return [linked.symbols[f"artefacts_on_{n}"] for n in ("spring_loader", "spring_link", "spring_unlink", "lifebone",
                                                          "action", "stagger", "onyx_hit", "onyx_hint")]


def install():
    if getattr(E.Machine, "_artefacts", False):
        return
    run = E.Machine.run

    def wrapped(self, hook, case, seed, old, **more):
        self.case_config, self.case_pools, self.case_factor = case.config, case.pools, case.factor
        return run(self, hook, case, seed, old, **more)
    E.Machine.run, E.Machine._artefacts = wrapped, True


install()


class Case(E.Case):
    def __init__(self, what, regs, stack, memory, config=None, pools=None, factor=1.0):
        super().__init__(what, regs, stack, memory)
        self.config, self.pools, self.factor = dict(config or {}), dict(pools or {}), factor


def regs(rng, **fixed):
    out = {r: rng.getrandbits(32) for r in ALL}
    out["eflags"] = rng.choice([0x202, 0x246, 0x283])
    out.update(fixed)
    return out


def dword(value):
    return struct.pack("<I", value & 0xFFFFFFFF)


def f32(value):
    return struct.pack("<f", value)


def stack(rng, *first):
    return list(first) + [rng.getrandbits(32) for _ in range(16 - len(first))]


def loader_case(rng, what, kinds):
    """`kinds`: per number (passive aim, active aim, active stopping) 2 (a
    float) or 0 (an int)."""
    memory = {CONFIG: bytes(rng.getrandbits(8) for _ in range(0x98))}
    config = {}
    passive, active = NODES, NODES + 0x40
    config[(DATA, "passive")] = passive
    config[(DATA, "active")] = active
    for i, (section, key) in enumerate(((passive, "aim_time_mod"), (active, "aim_time_mod"),
                                        (active, "stopping_power_mod"))):
        node = NODES + 0x200 + 0x40 * i
        config[(section, key)] = node
        value = rng.choice([struct.unpack("<I", f32(rng.uniform(-1, 1)))[0], rng.getrandbits(32), rng.randrange(-5, 6)])
        memory[node] = dword(value) + bytes(0x10) + struct.pack("<H", kinds[i])
    r = regs(rng, esi=CONFIG, ebx=DATA)
    r["xmm0"] = rng.getrandbits(128)
    return LOADER, Case(what, r, stack(rng), memory, config=config)


def holder_memory(rng, kit=1, list_states=None):
    """The inventory, its holder (the player's +108h interface) and the
    player's profile with the kit's lists."""
    memory = {INVENTORY + 0x174: dword(HOLDER), HOLDER: dword(HVT), HVT + 8: dword(GET_DM), HVT + 0xC: dword(GET_PLAYER),
              PLAYER + 0x6B8: dword(PROFILE), PLAYER + 0x115: bytes([kit])}
    lists = PROFILE + 0x704 * kit + 0x204
    for index, (count, head, tail, sorted_) in (list_states or {}).items():
        memory[lists + 0x14 * index] = struct.pack("<IIII", count, head, tail, rng.getrandbits(32)) + bytes([sorted_])
    return memory, lists


def link_case(rng, what, unlink=False, tails=None, values=(0.5, -1.0)):
    """The Spring's two nodes (their indexes 49 and 42) into lists with
    `tails` {index: (count, tail node's value or None)}."""
    lists_state = {}
    memory = {SPRING + 0x250: dword(INVENTORY)}
    extra, lists = holder_memory(rng, kit=rng.randrange(3))
    memory.update(extra)
    for index, (count, tail_value) in (tails or {}).items():
        tail = 0
        if tail_value is not None:
            tail = NODES + 0x300 + 0x20 * index
            memory[tail] = dword(rng.getrandbits(32)) + dword(0) + dword(index) + f32(tail_value)
        memory[lists + 0x14 * index] = struct.pack("<IIII", count, tail if tail else 0, tail, rng.getrandbits(32)) + \
            bytes([rng.getrandbits(1)])
    for node, index, value in ((SPRING + 0x3B8, 0x31, values[0]), (SPRING + 0x3C8, 0x2A, values[1])):
        memory[node] = dword(rng.getrandbits(32)) + dword(rng.getrandbits(32)) + dword(index) + f32(value)
    if unlink:
        return UNLINK_HOOK, Case(what, regs(rng), stack(rng, rng.getrandbits(32), SPRING), memory)
    return LINK, Case(what, regs(rng, eax=SPRING), stack(rng), memory)


def lifebone_case(rng, what, pools):
    """`pools` {name: (max, health)}."""
    memory = {LIFEBONE + 0x250: dword(INVENTORY), DM - 0x40: dword(DM)}
    extra, _ = holder_memory(rng)
    memory.update(extra)
    found = {}
    for i, (name, (top, health)) in enumerate(pools.items()):
        pool = POOLS + 0x100 * i
        memory[pool + 0x9C] = f32(top) + f32(health)
        found[name] = pool
    return LIFE, Case(what, regs(rng, edi=LIFEBONE), stack(rng, *[rng.getrandbits(32) for _ in range(5)], 77777), memory,
                      pools=found)


def artefact_memory(memory, at, vt, kind=1, state=3, cooldown=0, activated=0, time_left=3000, inventory=INVENTORY):
    memory[at] = dword(vt)
    memory[at + 0x24C] = dword(kind)
    memory[at + 0x250] = dword(inventory)
    memory[at + 0x280] = dword(state)
    memory[at + 0x28C] = dword(cooldown)
    memory[at + 0x2A0] = dword(activated)
    memory[at + 0xF14] = dword(time_left)


def vtables(memory):
    for vt, enable in ((VT_A, ENABLE), (VT_B, ENABLE), (VT_ONYX, ONYX_ENABLE)):
        memory[vt + 0x80] = dword(enable)


def action_case(rng, what, pressed_byte, pressed, slots, inventory=True):
    """`pressed` (vt, kind, state, cooldown); `slots` six of the same or
    None ("self" the pressed one)."""
    memory = {}
    vtables(memory)
    at = ARTEFACTS
    artefact_memory(memory, at, *pressed[:1], kind=pressed[1], state=pressed[2], cooldown=pressed[3],
                    inventory=INVENTORY if inventory else 0)
    items = []
    for i, slot in enumerate(slots):
        if slot is None:
            items.append(0)
        elif slot == "self":
            items.append(at)
        else:
            a = ARTEFACTS + 0x2000 * (i + 1)
            artefact_memory(memory, a, slot[0], kind=slot[1], state=slot[2], cooldown=slot[3])
            items.append(a)
    memory[INVENTORY + 0x10C + 13 * 4] = b"".join(dword(v) for v in items)
    return ACTION, Case(what, regs(rng, ecx=at), stack(rng, rng.getrandbits(32), pressed_byte, 123456), memory)


def stagger_case(rng, what, factor):
    memory, _ = holder_memory(rng, kit=rng.randrange(3))
    memory[PLAYER + 0x31C] = f32(rng.uniform(0, 3))
    r = regs(rng, esi=PLAYER)
    r["xmm0"] = rng.getrandbits(96) << 32 | struct.unpack("<I", f32(rng.uniform(0, 5)))[0]
    r["xmm1"] = 0
    r["xmm2"] = rng.getrandbits(128)
    return STAGGER, Case(what, r, stack(rng), memory, factor=factor)


def onyx_case(rng, what, state, slots, inventory=True):
    """The Onyx at ARTEFACTS (`state`); `slots` six (vt, state) or None or
    "self"."""
    memory = {}
    vtables(memory)
    artefact_memory(memory, ARTEFACTS, VT_ONYX, state=state, inventory=INVENTORY if inventory else 0)
    items = []
    for i, slot in enumerate(slots):
        if slot is None:
            items.append(0)
        elif slot == "self":
            items.append(ARTEFACTS)
        else:
            a = ARTEFACTS + 0x2000 * (i + 1)
            artefact_memory(memory, a, slot[0], state=slot[1])
            items.append(a)
    memory[INVENTORY + 0x10C + 13 * 4] = b"".join(dword(v) for v in items)
    return ONYX, Case(what, regs(rng, ecx=ARTEFACTS), stack(rng, rng.getrandbits(32)), memory)


def hint_case(rng, what, slots, activated=5000, inventory=True):
    """The Onyx at ARTEFACTS (active, activated at `activated`); `slots` six
    (vt, state, activated) or None or "self"."""
    memory = {}
    vtables(memory)
    artefact_memory(memory, ARTEFACTS, VT_ONYX, state=4, activated=activated, time_left=rng.randrange(1, 9000),
                    inventory=INVENTORY if inventory else 0)
    memory[ARTEFACTS + 0x2F0] = dword(rng.getrandbits(32))
    items = []
    for i, slot in enumerate(slots):
        if slot is None:
            items.append(0)
        elif slot == "self":
            items.append(ARTEFACTS)
        else:
            a = ARTEFACTS + 0x2000 * (i + 1)
            artefact_memory(memory, a, slot[0], state=slot[1], activated=slot[2])
            items.append(a)
    memory[INVENTORY + 0x10C + 13 * 4] = b"".join(dword(v) for v in items)
    r = regs(rng, ebx=ARTEFACTS + ONYX_CLIENT, esi=0x21000000)
    return HINT, Case(what, r, stack(rng), memory)


def cases(seed, count):
    rng = random.Random(seed)
    out = []
    for kinds in ((2, 2, 2), (0, 0, 0), (2, 0, 2), (0, 2, 0)):
        for _ in range(4):
            out.append(loader_case(rng, f"loader {kinds}", kinds))
    for unlink in (False, True):
        for tails in ({}, {0x31: (0, None)}, {0x31: (2, 0.25), 0x2A: (1, -2.0)}, {0x31: (1, 0.75), 0x2A: (3, -0.5)},
                      {0x31: (1, 0.5)}, {0x2A: (5, -1.0)}):
            for _ in range(3):
                out.append(link_case(rng, f"{'unlink' if unlink else 'link'} {tails}", unlink, tails))
    for pools in ({"pain": (100.0, 40.0), "body": (100.0, 100.0), "head": (50.0, 60.0)}, {"body": (80.0, 0.0)}, {},
                  {"pain": (10.0, float("nan")), "head": (5.0, 4.0), "arm": (9.0, 1.0)}):
        for _ in range(3):
            out.append(lifebone_case(rng, f"lifebone {sorted(pools)}", pools))
    A, B, O = VT_A, VT_B, VT_ONYX
    for what, pressed_byte, pressed, slots, inv in (
            ("key-up", 0, (A, 1, 3, 0), ["self"] + [None] * 5, True),
            ("key-up, another byte", 0x100, (A, 1, 3, 0), ["self", (A, 1, 3, 0)] + [None] * 4, True),
            ("a press, one", 1, (A, 1, 3, 0), ["self"] + [None] * 5, True),
            ("a press, three of its class", 1, (A, 1, 3, 0), ["self", (A, 1, 3, 0), (B, 1, 3, 0), (A, 1, 3, 0), None,
                                                               (A, 0, 3, 0)], True),
            ("a press, the pressed cooling", 1, (A, 1, 3, 50), ["self", (A, 1, 3, 0), (A, 1, 4, 0), (A, 1, 3, 9), None,
                                                                None], True),
            ("a press, three Onyx", 1, (O, 1, 3, 0), ["self", (O, 1, 3, 0), (O, 1, 3, 0), None, None, None], True),
            ("a press, two Onyx, one cooling", 1, (O, 1, 3, 0), [(O, 1, 3, 7), "self", (O, 1, 3, 0), None, None, None],
             True),
            ("a press, an Onyx alone", 1, (O, 1, 3, 0), ["self"] + [None] * 5, True),
            ("a press, Onyx not ready", 1, (O, 1, 4, 0), ["self", (O, 1, 3, 0), (O, 1, 3, 0), None, None, None], True),
            ("a press, no inventory", 1, (O, 1, 3, 0), ["self"] + [None] * 5, False)):
        for _ in range(3):
            out.append(action_case(rng, what, pressed_byte, pressed, slots, inv))
    for factor in (1.0, 0.0, 0.5, -1.0):
        for _ in range(3):
            out.append(stagger_case(rng, f"stagger x{factor}", factor))
    for what, state, slots, inv in (
            ("passive", 3, ["self"] + [None] * 5, True),
            ("active, the first", 4, ["self", (VT_ONYX, 4), None, None, None, None], True),
            ("active, the second", 4, [(VT_ONYX, 4), "self", None, None, None, None], True),
            ("active, after a passive one", 4, [(VT_ONYX, 3), "self", None, None, None, None], True),
            ("active, after another class", 4, [(VT_A, 4), "self", None, None, None, None], True),
            ("active, in no slot", 4, [(VT_ONYX, 3), None, None, None, None, None], True),
            ("active, in no slot, one active", 4, [None, (VT_ONYX, 4), None, None, None, None], True),
            ("active, no inventory", 4, ["self"] + [None] * 5, False)):
        for _ in range(3):
            out.append(onyx_case(rng, f"onyx hit: {what}", state, slots, inv))
    for what, slots, inv in (
            ("alone", ["self"] + [None] * 5, True),
            ("the first of its press", ["self", (VT_ONYX, 4, 5000), None, None, None, None], True),
            ("the second of its press", [(VT_ONYX, 4, 5000), "self", None, None, None, None], True),
            ("after one of another press", [(VT_ONYX, 4, 4000), "self", None, None, None, None], True),
            ("after a passive one", [(VT_ONYX, 3, 5000), "self", None, None, None, None], True),
            ("after another class", [(VT_A, 4, 5000), "self", None, None, None, None], True),
            ("in no slot", [(VT_ONYX, 4, 4000), None, None, None, None, None], True),
            ("in no slot, one of its press", [(VT_ONYX, 4, 5000), None, None, None, None, None], True),
            ("no inventory", ["self"] + [None] * 5, False)):
        for _ in range(3):
            out.append(hint_case(rng, f"hint: {what}", slots, 5000, inv))
    yield from out
    pick = lambda: rng.choice([None, "self", (rng.choice([VT_A, VT_B, VT_ONYX]), rng.choice([0, 1, 1]),
                                              rng.choice([3, 3, 4]), rng.choice([0, 0, 5]))])
    for i in range(max(0, count - len(out))):
        kind = i % 6
        if kind == 0:
            slots = [pick() for _ in range(6)]
            yield action_case(rng, "random action", rng.choice([0, 1, 1, 0xFF]),
                              (rng.choice([VT_A, VT_ONYX]), rng.choice([0, 1]), rng.choice([3, 4]), rng.choice([0, 0, 3])),
                              slots, rng.random() < 0.9)
        elif kind == 1:
            slots = [rng.choice([None, "self", (rng.choice([VT_A, VT_ONYX]), rng.choice([3, 4]))]) for _ in range(6)]
            yield onyx_case(rng, "random onyx hit", rng.choice([3, 4, 4]), slots, rng.random() < 0.9)
        elif kind == 2:
            slots = [rng.choice([None, "self", (rng.choice([VT_A, VT_ONYX]), rng.choice([3, 4]),
                                                rng.choice([5000, 4000]))]) for _ in range(6)]
            yield hint_case(rng, "random hint", slots, 5000, rng.random() < 0.9)
        elif kind == 3:
            tails = {index: (rng.randrange(0, 5), rng.choice([None, rng.uniform(-3, 3)]))
                     for index in rng.sample([0x31, 0x2A], rng.randrange(0, 3))}
            yield link_case(rng, "random link", rng.random() < 0.5, tails, (rng.uniform(-2, 2), rng.uniform(-2, 2)))
        elif kind == 4:
            yield stagger_case(rng, "random stagger", rng.uniform(-2, 2))
        else:
            pools = {name: (rng.uniform(0, 100), rng.uniform(-10, 110)) for name in rng.sample(POOL_NAMES, rng.randrange(0, 5))}
            yield lifebone_case(rng, "random lifebone", pools)
