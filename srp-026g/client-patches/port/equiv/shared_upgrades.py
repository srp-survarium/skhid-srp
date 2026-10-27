"""The equivalence spec of patch 30, one upgrade on several items
(equiv_026g.py; shared_upgrades_026g.asm at 0x700 of `.srp` before the Rust
port, rust/patches/src/shared_upgrades.rs after).

- drop_sent, 0x29AAD5 `push edi / push ecx / call 0x2708C0` (EAX the lobby
  client, ECX the kit, EDI the upgrade, [ebp+8] -> +8 -> +8 the item): both
  send `46 02 <kit> <upgrade> <item>` (new_packet(client+8, 46h), resize to
  +13, the bytes, enqueue(client+8, packet)) and go on at 0x29AADC (the
  asm's `ret` lands on the nops at 0x29AADA: alias). Live: EBX, ESI, EDI,
  EBP; EAX is not (0x29AADE `mov al,1`: the delegate's callers read AL,
  check_shared_upgrades_026g.py asserts it), nor ECX, EDX (the handler's
  epilogue, `ret 4`).
- detach_answer, 0x275756 (EAX the read position, ESI the reader, EBP the
  client): <upgrade> <item> read, the reader +8, process_detach_upgrade(the
  client, upgrade) with the item for find_holder: the asm a third stack
  argument (it pops it after the `ret 8`), the port its GameCell HOST while
  the call runs. The stub's third input is that item: [esp+0Ch] in the old
  build, HOST in the new. Then 0x275765 with EAX the call's. Live: EAX,
  EBX, ESI, EDI, EBP (ECX and EDX: the case's `ret 4` follows).
- detach_first, 0x275589 (EBX the item, EDI the upgrade, EBP the client):
  the same call with EBX's item; 0x275590 (the asm's `ret` on the nops at
  0x27558E: alias).
- find_holder, 0x27534D `call 0x277CF0` (cdecl: [esp] the items, [esp+4]
  their end, [esp+8] &the id): the named item -- the asm's [esp+58h] at its
  entry (the site's ESP+54h), the port's HOST -- 0: the original's search
  (the asm tail-jumps to 0x277CF0, the port replays the call); else the
  item of that id or the end, EAX it, ECX the end, EDX the id; 0x275352.
  Live: EAX, ECX, EDX, EBX, ESI, EDI, EBP.
"""
import random
import struct

import equiv_026g as E

CLIENT = 0x30000000
PACKET, BUFFER, DATA = 0x21200000, 0x21201000, 0x21202000
ARGS, VALUES = 0x20005000, 0x20005100
FRAME = 0x20006000              # the drop handler's EBP
MESSAGE, READER = 0x21300100, 0x21300000
ITEMS = 0x21100000
NEW_PACKET, RESIZE, ENQUEUE, DETACH, FIND = 0x37A0E0, 0x7AB30, 0x379640, 0x275220, 0x277CF0
SCRATCH_HOST = 0x20F00000       # the old build's stand-in for HOST (it keeps the item on the stack)


def new_packet(machine, args, rng):
    machine.set_reg("eax", PACKET)


def resize(machine, args, rng):
    machine.write32(args[0] + 0xC, args[1])


def clobbers(machine, args, rng):
    """A cdecl callee leaves ECX and EDX its own (0x277CF0 computes in both)."""
    machine.set_reg("ecx", rng.getrandbits(32))
    machine.set_reg("edx", rng.getrandbits(32))


def host(machine):
    """The item process_detach_upgrade's find_holder will see."""
    if machine.old:
        return machine.read32(machine.reg("esp") + 12)
    return machine.read32(machine.places["host"][0])


EXTERNALS = {
    NEW_PACKET: E.External("new_packet", ["eax", "arg0"], pops=4, model=new_packet),
    RESIZE: E.External("resize", ["eax", "arg0"], pops=4, model=resize),
    ENQUEUE: E.External("enqueue", ["arg0", "arg1"], pops=8),
    DETACH: E.External("process_detach_upgrade", ["arg0", "arg1", host], pops=8),
    FIND: E.External("find_item_holding_upgrade", ["arg0", "arg1", "arg2"], pops=0, model=clobbers),
}
DROP = E.Hook("drop_sent", 0x29AAD5, ("ebx", "esi", "edi", "ebp"), aliases={0x29AADA: 0x29AADC})
ANSWER = E.Hook("detach_answer", 0x275756, ("eax", "ebx", "esi", "edi", "ebp"))
FIRST = E.Hook("detach_first", 0x275589, ("eax", "ebx", "esi", "edi", "ebp"), aliases={0x27558E: 0x275590})
HOLDER = E.Hook("find_holder", 0x27534D, ("eax", "ecx", "edx", "ebx", "esi", "edi", "ebp"), tails=(FIND,))
HOOKS = [DROP, ANSWER, FIRST, HOLDER]


def ranges(P, linked):
    if "shared_upgrades_bin" in dir(P):
        return [(P.SECTION_VA + 0x700, P.SECTION_VA + 0x700 + len(open(P.shared_upgrades_bin(), "rb").read()))]
    blocks = [b for b in linked.blocks.values() if b.name == "patches.code"
              or b.kind == "code" and b.va <= linked.symbols.get("memset", 0) < b.va + b.size]
    return [(b.va, b.va + b.size) for b in blocks]


def entries(P, linked):
    if "shared_upgrades_bin" in dir(P):
        return [P.SECTION_VA + 0x700 + at for at in (0, 0x60, 0x80, 0x90)]
    return [linked.symbols[f"shared_upgrades_on_{n}"] for n in ("drop_sent", "detach_answer", "detach_first",
                                                                "find_holder")]


def places(P, linked):
    if "shared_upgrades_bin" in dir(P):
        return {"host": (SCRATCH_HOST, 4)}
    va = next(va for name, va, _, _ in linked.every if "shared_upgrades" in name and "HOST" in name)
    return {"host": (va, 4)}


def dword(value):
    return (value & 0xFFFFFFFF).to_bytes(4, "little")


def regs(rng, **fixed):
    out = {r: rng.getrandbits(32) for r in ("eax", "ecx", "edx", "ebx", "esi", "edi", "ebp")}
    out["eflags"] = rng.choice([0x202, 0x246, 0x283])
    out.update(fixed)
    return out


def stack(rng):
    return [rng.getrandbits(32) for _ in range(32)]


def drop_case(rng, what, kit, upgrade, item, size):
    memory = {FRAME + 8: dword(ARGS), ARGS + 8: dword(VALUES), VALUES + 8: dword(item),
              PACKET + 0x2BC: dword(BUFFER), BUFFER + 4: dword(DATA), BUFFER + 0xC: dword(size),
              DATA: bytes(0x100), "host+0x0": dword(0)}
    return DROP, E.Case(what, regs(rng, eax=CLIENT, ecx=kit, edi=upgrade, ebp=FRAME), stack(rng), memory)


def answer_case(rng, what, upgrade, item):
    memory = {MESSAGE: dword(upgrade) + dword(item) + E.made_up(rng, 8), READER + 4: dword(MESSAGE),
              "host+0x0": dword(0)}
    return ANSWER, E.Case(what, regs(rng, eax=MESSAGE, esi=READER, ebp=CLIENT), stack(rng), memory)


def first_case(rng, what, upgrade, item):
    return FIRST, E.Case(what, regs(rng, ebx=item, edi=upgrade, ebp=CLIENT), stack(rng), {"host+0x0": dword(0)})


def holder_case(rng, what, ids, host_id, end_at=None):
    """The items (B4h each) with these ids; the named id; the end."""
    memory = {}
    for i, item in enumerate(ids):
        memory[ITEMS + 0xB4 * i] = dword(item) + E.made_up(rng, 0xB0)
    end = ITEMS + 0xB4 * (len(ids) if end_at is None else end_at)
    s = stack(rng)
    s[0], s[1], s[2] = ITEMS, end, rng.getrandbits(32)
    s[0x54 // 4] = host_id
    memory["host+0x0"] = dword(host_id)
    return HOLDER, E.Case(what, regs(rng), s, memory)


def cases(seed, count):
    rng = random.Random(seed)
    for kit, upgrade, item, size in ((0x51, 9, 1, 0), (0, 0, 0, 3), (0xFFFFFFFF, 0x7, 2, 0x40)):
        for _ in range(3):
            yield drop_case(rng, f"drop kit {kit:#x}", kit, upgrade, item, size)
    for upgrade, item in ((9, 1), (9, 0), (7, 2), (0, 0)):
        for _ in range(3):
            yield answer_case(rng, f"answer {upgrade} off {item}", upgrade, item)
            yield first_case(rng, f"first {upgrade} off {item}", upgrade, item)
    for ids, host_id, end_at in (([1, 2, 3], 2, None), ([1, 2, 3], 0, None), ([1, 2, 3], 5, None), ([], 2, None),
                                 ([2, 2], 2, None), ([1, 2, 3], 3, 2)):
        for _ in range(3):
            yield holder_case(rng, f"holders {ids}, {host_id}", ids, host_id, end_at)
    for i in range(max(0, count - 80)):
        kind = i % 4
        if kind == 0:
            yield drop_case(rng, "random drop", rng.getrandbits(32), rng.getrandbits(32), rng.getrandbits(32),
                            rng.randrange(0x80))
        elif kind == 1:
            yield answer_case(rng, "random answer", rng.getrandbits(32), rng.choice([0, rng.randrange(8)]))
        elif kind == 2:
            yield first_case(rng, "random first", rng.getrandbits(32), rng.choice([0, rng.randrange(8)]))
        else:
            ids = [rng.randrange(6) for _ in range(rng.randrange(6))]
            yield holder_case(rng, "random holders", ids, rng.choice([0, rng.randrange(7)]))
