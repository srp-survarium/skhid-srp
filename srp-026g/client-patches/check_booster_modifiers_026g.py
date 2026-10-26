#!/usr/bin/env python3
"""Run the original's booster and modifier code on made-up boosters, kits and
players, for the four booster buffs (respawn, energy costs, aiming speed,
collection time).

    python check_booster_modifiers_026g.py <original survarium.exe>

Requires Python `unicorn` and `patch_026g.py` next to this file. The exe's
sections are loaded as they are, and the game's own code runs:

- the item config reader's loop over the 68 `parameters/*_modifier` names
  (0x2C83E0 up to 0x2C8474, with the config lookups 0xF6B10 / 0xF6C00) on a
  binary config written here: every name of the table 0x928448 lands at its
  index, a misspelt name lands nowhere;
- the booster constructor's node loop (0x33F690 after the item base
  constructor: from 0x33F6A7, the allocator 0xE4F90 a stub) and its spawn
  link (slot 14, 0x33F960 -> 0x33F990; the holder's player getter a stub):
  one node for every nonzero float of the 68, linked into the list of its
  index in the player's kit;
- the getter 0x2D5AE0 on those lists, through its readers: the jump cost
  0x329710, the can-jump test 0x2D6420 and the sprint drain 0x329A70
  (indexes 19 and 67), the artefact container's loader slice 0x31C48F..0x31C4CB
  and use_execute 0x31C780 (index 7), the aim start / end slices of
  0x2E1B50 / 0x2E1BF0 (index 49), and the respawn rule's on_event 0x316AA0
  whole (index 60; its spawn-point test 0x317700 a stub that says yes) with
  fill_match_hud_state 0x316C20 for the HUD's seconds.

Says nothing about the HUD's drawing, the movies or a real match.
"""
import hashlib
import math
import struct
import sys
import zlib

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, \
    UC_X86_REG_EDI, UC_X86_REG_ESP, UC_X86_REG_EBX, UC_X86_REG_EBP, UC_X86_REG_EIP, UC_X86_REG_XMM0

import patch_026g as base

NAME_TABLE = 0x928448                    # 68 pointers to `<name>_modifier`
CONFIG_READER, READER_LOOP_END = 0x2C83E0, 0x2C8474   # (out, config); stop after the 68 names
BOOSTER_NODES, BOOSTER_NODES_END = 0x33F6A7, 0x33F823  # edi = the booster, esi = 1, ebp = 0
ALLOCATOR_OBJECT, ALLOCATE = 0x420789, 0xE4F90  # stubs: the allocator; eax = size, (allocator) `ret 4`
BOOSTER_LINK = 0x33F960                  # thiscall, slot 14 of the booster vtable 0xA1CA34
GETTER = 0x2D5AE0                        # eax = kit block +0F4h, esi = index; (B, k), `ret 8`
JUMP_SPEND, CAN_JUMP, SPRINT = 0x329710, 0x2D6420, 0x329A70   # edi = player_stamina
CONTAINER_LOAD, CONTAINER_LOAD_END = 0x31C48F, 0x31C4CB  # ebx = config, ebp = container
USE_EXECUTE = 0x31C780                   # thiscall (user data, now, required object), `ret 0Ch`
USE_NEEDED, USE_DONE, USE_WAIT = 0x31C802, 0x31C806, 0x31C865
AIM_START, AIM_START_END = 0x2E1B66, 0x2E1BE0  # edi = the weapon; after 0x31D970
AIM_END, AIM_END_END = 0x2E1C07, 0x2E1C7C      # edi = the weapon; after 0x38CA20
RESPAWN_EVENT, SPAWN_TEST = 0x316AA0, 0x317700  # thiscall (1, event, time), `ret 0Ch`; (out) `ret 4`
HUD_STATE = 0x316C20                     # thiscall (hud state, now), `ret 8`

ARTEFACT_SEARCH, STAMINA_SPEND, AIM_TIME, RESPAWN, STAMINA_SKILL = 7, 19, 49, 60, 67
ADDITIVE = [5, 10, 14, 15, 16, 17, 25, 28, 29, 31, 32, 37, 43, 44, 45, 46, 47, 48, 50, 53, 62, 64]

MEMORY, STACK, STOP, STUB = 0x20000000, 0x21008000, 0x22000000, 0x22000100
HOLDER, HOLDER_VT, PLAYER, KITS, CORE = (MEMORY + 0x10000 * i for i in range(1, 6))
KIT = 1                                  # the player's kit (+115h): exercises the x 704h
GET_PLAYER = STUB


def f32(value):
    return struct.unpack("<I", struct.pack("<f", value))[0]


def config_bytes(tree, at):
    """A binary config of `tree` (dicts, ints, floats) loaded at `at`, as
    check_artefacts_026g.py writes it: 24-byte values {data, name, crc, type,
    count}, a table's children sorted by the CRC of their names."""
    values, names = [], bytearray()

    def name(text):
        offset = len(names)
        names.extend(text.encode() + b"\0")
        return offset

    def put(key, value, slot):
        crc = zlib.crc32(key.encode()) if key else 0
        if isinstance(value, dict):
            children = sorted(value.items(), key=lambda kv: zlib.crc32(kv[0].encode()))
            first = len(values)
            values.extend([None] * len(children))
            for i, (k, v) in enumerate(children):
                put(k, v, first + i)
            values[slot] = ("table", first, key, crc, 3, len(children))
        elif isinstance(value, float):
            values[slot] = ("raw", f32(value), key, crc, 2, 0)
        else:
            values[slot] = ("raw", value & 0xFFFFFFFF, key, crc, 1, 0)

    values.append(None)
    put("", tree, 0)
    names_at = at + 24 * len(values)
    out = bytearray()
    for kind, data, key, crc, type_, count in values:
        if kind == "table":
            data = at + 24 * data
        out += struct.pack("<QQIHH", data, names_at + name(key), crc, type_, count)
    return bytes(out + names)


class Game:
    def __init__(self, data):
        self.data = data
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(MEMORY, 0x100000, UC_PROT_ALL)
        emu.mem_map(0x21000000, 0x10000, UC_PROT_ALL)
        emu.mem_map(STOP, 0x10000, UC_PROT_ALL)
        emu.mem_write(GET_PLAYER, b"\xB8" + struct.pack("<I", PLAYER) + b"\xC3")
        self.word(HOLDER, HOLDER_VT)
        self.word(HOLDER_VT + 0xC, GET_PLAYER)
        self.word(PLAYER + 0x6B8, KITS)
        emu.mem_write(PLAYER + 0x115, bytes([KIT]))
        for index in range(68):          # the kit's lists: empty, sorted
            emu.mem_write(self.list(index) + 0x10, b"\x01")
        self.next = MEMORY + 0xA0000
        self.seen = {}
        emu.hook_add(UC_HOOK_CODE, self.on_stub, begin=ALLOCATOR_OBJECT, end=ALLOCATOR_OBJECT)
        emu.hook_add(UC_HOOK_CODE, self.on_stub, begin=ALLOCATE, end=ALLOCATE)
        emu.hook_add(UC_HOOK_CODE, self.on_stub, begin=SPAWN_TEST, end=SPAWN_TEST)
        for at in (USE_NEEDED, USE_DONE, USE_WAIT):
            emu.hook_add(UC_HOOK_CODE, self.on_seen, begin=at, end=at)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def put_float(self, at, value):
        self.word(at, f32(value))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def float(self, at):
        return struct.unpack("<f", self.emu.mem_read(at, 4))[0]

    def block(self, size):
        at, self.next = self.next, self.next + (size + 0xF) // 0x10 * 0x10
        self.emu.mem_write(at, bytes(size))
        return at

    @staticmethod
    def kit():
        return KITS + KIT * 0x704 + 0xF4        # the getter's eax

    def list(self, index):
        return self.kit() + 0x110 + index * 0x14  # {count, head, tail, -, sorted}

    def on_stub(self, emu, address, size, _):
        esp = emu.reg_read(UC_X86_REG_ESP)
        ret = self.read(esp)
        if address == ALLOCATOR_OBJECT:
            emu.reg_write(UC_X86_REG_EAX, 0)
            pop = 4
        elif address == ALLOCATE:
            emu.reg_write(UC_X86_REG_EAX, self.block(emu.reg_read(UC_X86_REG_EAX)))
            pop = 8
        else:                                   # SPAWN_TEST: *out = 1 (a point is free)
            emu.mem_write(self.read(esp + 4), b"\x01")
            emu.reg_write(UC_X86_REG_EAX, 1)
            pop = 8
        emu.reg_write(UC_X86_REG_ESP, esp + pop)
        emu.reg_write(UC_X86_REG_EIP, ret)

    def on_seen(self, emu, address, size, _):
        self.seen[address] = emu.reg_read(UC_X86_REG_EAX), emu.reg_read(UC_X86_REG_EBP)
        if address != USE_NEEDED:
            emu.emu_stop()

    def run(self, va, until=STOP, args=(), **regs):
        esp = STACK - 0x100
        for value in reversed(args):
            esp -= 4
            self.word(esp, value)
        esp -= 4
        self.word(esp, STOP)
        self.emu.reg_write(UC_X86_REG_ESP, esp)
        for name, value in regs.items():
            self.emu.reg_write(REGISTER[name], value)
        self.emu.emu_start(va, until, count=2_000_000)

    def xmm0(self):
        return struct.unpack("<f", self.emu.reg_read(UC_X86_REG_XMM0).to_bytes(16, "little")[:4])[0]

    # --- the item config reader and the booster

    def read_parameters(self, parameters):
        """The 68 floats 0x2C83E0 reads from `parameters`."""
        config, out = self.block(0x4000), self.block(0x200)
        self.emu.mem_write(config, config_bytes({"parameters": parameters, "ui_desc": {}}, config))
        self.run(CONFIG_READER, READER_LOOP_END, (out, config))
        return [self.float(out + 4 * i) for i in range(68)]

    def booster(self, floats):
        """A booster whose 68 floats (+108h) are `floats`, made and linked as
        at a spawn: its node count and nodes."""
        obj = self.block(0x300)
        for i, value in enumerate(floats):
            self.put_float(obj + 0x108 + 4 * i, value)
        inventory = self.block(0x200)
        self.word(inventory + 0x174, HOLDER)
        self.word(obj + 0x250, inventory)
        self.run(BOOSTER_NODES, BOOSTER_NODES_END, edi=obj, esi=1, ebp=0)
        self.run(BOOSTER_LINK, ecx=obj)
        nodes = self.read(obj + 0x268)
        return [(self.read(nodes + 16 * i + 8), self.float(nodes + 16 * i + 12))
                for i in range(self.read(obj + 0x26C))]

    def listed(self, index):
        head, values = self.read(self.list(index) + 4), []
        while head:
            values.append(self.float(head + 0xC))
            head = self.read(head + 4)
        return values

    def append_node(self, index, value):
        """Another source's node (an artefact's, an armour's) at the list's tail."""
        node, lst = self.block(16), self.list(index)
        tail = self.read(lst + 8)
        self.word(node, tail)
        self.word(node + 8, index)
        self.put_float(node + 0xC, value)
        if tail:
            self.word(tail + 4, node)
        else:
            self.word(lst + 4, node)
        self.word(lst + 8, node)
        self.word(lst, self.read(lst) + 1)

    def getter(self, index, value, k=1.0):
        self.run(GETTER, args=(f32(value), f32(k)), eax=self.kit(), esi=index)
        return self.xmm0()

    # --- index 19 and 67: stamina

    def stamina(self, current, jump=0.0, sprint=0.0, last=0):
        s = self.block(0x100)
        self.put_float(s, 100.0)                 # +0 max_value
        self.put_float(s + 4, sprint)            # +4 sprint_spending_speed (per s)
        self.put_float(s + 0xC, jump)            # +0Ch amount_to_jump
        self.word(s + 0x48, self.kit())          # the kit's modifiers
        self.put_float(s + 0x6C, current)
        self.word(s + 0x70, last)
        return s

    # --- index 7: the artefact container

    def container(self, seconds):
        c, config = self.block(0x100), self.block(0x400)
        self.emu.mem_write(config, config_bytes({"artefacts_search_time_sec": seconds}, config))
        self.run(CONTAINER_LOAD, CONTAINER_LOAD_END, ebx=config, ebp=c)
        return c

    def use(self, container, elapsed):
        """use_execute at `elapsed` ms after the use began: (needed ms, done)."""
        user = self.block(0x20)
        self.word(user, PLAYER)
        self.word(PLAYER + 0x330, 1 << 21)
        self.word(user + 8, 100_000)
        self.seen = {}
        self.run(USE_EXECUTE, args=(user, 100_000 + elapsed, container), ecx=container)
        return self.seen[USE_NEEDED][0], USE_DONE in self.seen

    # --- index 49: aim

    def aim(self, aim_time, start):
        w = self.block(0x700)
        self.word(w + 8, PLAYER)
        self.put_float(w + 0x5F8, aim_time)
        self.put_float(w + 0x604, 0.0 if start else 1.0)
        self.run(*((AIM_START, AIM_START_END) if start else (AIM_END, AIM_END_END)), edi=w)
        return self.float(w + 0x608)

    # --- index 60: the respawn rule

    def respawn(self, base_ms, death, now, place=3):
        rule, core, records, event, hud = (self.block(n) for n in (0x200, 0x19000, 0x400, 0x10, 0x10))
        self.word(rule + 0x114, base_ms)
        self.put_float(rule + 0x118, 1.0)        # the base constructor's 1.0 (0x31672D)
        self.word(rule + 0x110, self.block(0x100))
        self.word(rule + 0x134, core)
        self.word(core + 0x18458, records)
        self.word(core + 0x18FBC, now)
        self.word(records + place * 0x30, PLAYER)  # record +0Ch = 0: its first player
        self.word(KITS + 0x45EC, 0)
        self.emu.mem_write(event + 1, bytes([place]))
        self.run(RESPAWN_EVENT, args=(1, event, death), ecx=rule)
        at = self.read(rule + 0x138 + 4 * place)
        self.emu.mem_write(hud, bytes([place]))
        self.run(HUD_STATE, args=(hud, now), ecx=rule)
        return at, self.read(hud + 8)


REGISTER = {"eax": UC_X86_REG_EAX, "ecx": UC_X86_REG_ECX, "edx": UC_X86_REG_EDX, "esi": UC_X86_REG_ESI,
            "edi": UC_X86_REG_EDI, "ebx": UC_X86_REG_EBX, "ebp": UC_X86_REG_EBP}


def run(data):
    checks = 0

    def expect(ok, what):
        nonlocal checks
        checks += 1
        if not ok:
            sys.exit(f"FAIL: {what}")

    def close(a, b, eps=1e-5):
        return abs(a - b) <= eps

    g = Game(data)
    names = [bytes(g.emu.mem_read(g.read(NAME_TABLE + 4 * i), 64)).split(b"\0")[0].decode() for i in range(68)]
    expect(names[ARTEFACT_SEARCH] == "artefact_search_time_modifier", "index 7's name")
    expect(names[STAMINA_SPEND] == "stamina_spending_speed_modifier", "index 19's name")
    expect(names[AIM_TIME] == "aim_time_modifier", "index 49's name")
    expect(names[RESPAWN] == "respawn_time_modifier", "index 60's name")
    expect(names[STAMINA_SKILL] == "stamina_spending_speed_skill_modifier", "index 67's name")
    expect([g.read(0x927E08 + 4 * i) for i in range(22)] == ADDITIVE, "the additive indexes 0x927E08")

    # The config reader: each name at its index, a misspelling nowhere.
    every = {n: -(i + 1) / 128 for i, n in enumerate(names)}
    expect(g.read_parameters(every) == [-(i + 1) / 128 for i in range(68)], "68 names read at their indexes")
    floats = g.read_parameters({"stamina_spending_modifier": -0.3, "stamina_spending_speed_modifer": -0.3,
                                "respawn_time": -0.375})
    expect(floats == [0.0] * 68, "a misspelt name is read nowhere")
    print("config reader: the 68 names of 0x928448 land at their indexes; misspellings are dropped")

    # Every one of the 68 nonzero: 68 nodes, each in its own index's list.
    nodes = Game(data).booster([-(i + 1) / 128 for i in range(68)])
    expect([n for n, _ in nodes] == list(range(68)), "a node for every index")
    gg = Game(data)
    gg.booster([-(i + 1) / 128 for i in range(68)])
    expect(all(gg.listed(i) == [-(i + 1) / 128] for i in range(68)), "each node in its index's kit list")
    print("booster: 68 nonzero floats -> 68 nodes, indexes 0..67, each linked into its kit list (no filter)")

    # The four planned boosters, each its own float array (other indexes 0).
    plan = {"radio (respawn_booster)": {RESPAWN: -0.375},
            "rations (regeneration_booster)": {STAMINA_SPEND: -0.30},
            "protein (weight_booster)": {AIM_TIME: -0.15},
            "artefact master (artefact_booster)": {ARTEFACT_SEARCH: -0.60}}
    for name, mods in plan.items():
        b = Game(data)
        floats = b.read_parameters({names[i]: v for i, v in mods.items()})
        made = b.booster(floats)
        expect([(i, round(v, 6)) for i, v in made] == [(i, v) for i, v in mods.items()], name + " nodes")
        for i, v in mods.items():
            expect(close(b.listed(i)[0], v), name + " listed")
        print(f"  {name}: nodes {[(i, round(v, 4)) for i, v in made]}")

    # Index 19 and 67 (energy costs).
    print("stamina (index 19 x index 67, each B x max(0, 1 + S)):")
    for s19, s67 in ((0.0, 0.0), (-0.30, 0.0), (-0.30, -0.10), (-1.5, 0.0)):
        e = Game(data)
        floats = [0.0] * 68
        floats[STAMINA_SPEND] = s19
        e.booster(floats)
        e.put_float(e.kit() + 4 * STAMINA_SKILL, s67)   # the kit's own float (a skill)
        factor = max(0.0, 1 + s19) * max(0.0, 1 + s67)
        st = e.stamina(50.0, jump=10.0)
        e.run(JUMP_SPEND, edi=st)
        jump_left = e.float(st + 0x6C)
        expect(close(jump_left, 50.0 - 10.0 * factor, 1e-4), f"jump cost {s19} {s67}")
        st = e.stamina(8.0, jump=10.0)
        e.run(CAN_JUMP, edi=st)
        can = e.emu.reg_read(UC_X86_REG_EAX) & 0xFF
        expect(can == (8.0 >= 10.0 * factor), f"can jump {s19} {s67}")
        st = e.stamina(50.0, sprint=10.0, last=1000)
        e.run(SPRINT, args=(2000,), edi=st)
        sprint_left = e.float(st + 0x6C)
        expect(close(sprint_left, 50.0 - 10.0 * factor, 1e-4), f"sprint cost {s19} {s67}")
        print(f"  S19 {s19:+.2f} S67 {s67:+.2f}: jump 10 -> {50 - jump_left:.4f}, can jump at 8: {bool(can)}, "
              f"sprint 10/s for 1000 ms -> {50 - sprint_left:.4f}")

    # Index 7 (collection time).
    print("artefact container (index 7): needed = ceil(round(sec x 1000) x max(0, 1 + S)) ms")
    for seconds in (3.0, 5.0, 10):
        for s7 in (0.0, -0.25, -0.60):
            c = Game(data)
            floats = [0.0] * 68
            floats[ARTEFACT_SEARCH] = s7
            c.booster(floats)
            box = c.container(seconds)
            base_ms = c.read(box + 0x58)
            expect(base_ms == round(seconds * 1000), "the container's ms")
            needed, done = c.use(box, 0)
            expect(not done, "not done at 0 ms")
            _, done_before = c.use(box, needed - 1)
            _, done_at = c.use(box, needed)
            expect(not done_before and done_at, "done exactly at the needed ms")
            expect(needed == math.ceil(base_ms * max(0.0, 1 + struct.unpack("<f", struct.pack("<f", s7))[0])),
                   "needed ms")
            print(f"  {seconds} s, S7 {s7:+.2f}: {needed} ms (not done at {needed - 1}, done at {needed})")

    # Index 49 (aim time), the booster's node plus another source's.
    print("aim (index 49): duration = aim_time x max(0, 1 + S)")
    for nodes49 in ((), (-0.15,), (-0.15, -0.05)):
        a = Game(data)
        floats = [0.0] * 68
        if nodes49:
            floats[AIM_TIME] = nodes49[0]
        a.booster(floats)
        for extra in nodes49[1:]:
            a.append_node(AIM_TIME, extra)
        start, end = a.aim(0.4, True), a.aim(0.4, False)
        expect(close(start, 0.4 * (1 + sum(nodes49))) and close(end, start), f"aim {nodes49}")
        print(f"  nodes {list(nodes49)}: aim_time 0.4 -> start {start:.5f}, end {end:.5f}")

    # Index 60 (respawn): the whole on_event, then the HUD's seconds.
    print("respawn (index 60): on_event 0x316AA0, death at `now` and 17 ms before; HUD at the death")
    for base_s in (8, 10):
        for s60 in (0.0, -0.2, -0.375, -0.6):
            r = Game(data)
            floats = [0.0] * 68
            floats[RESPAWN] = s60
            r.booster(floats)
            now = 600_000
            at, hud = r.respawn(base_s * 1000, now, now)
            at17, _ = r.respawn(base_s * 1000, now - 17, now)
            exact = base_s * 1000 * max(0.0, 1 + struct.unpack("<f", struct.pack("<f", s60))[0])
            expect(at - now == math.ceil(math.floor(exact + 0.5) / 50) * 50, "respawn delay on the grid")
            expect(hud == math.ceil((at - now) / 1000), "hud seconds")
            print(f"  {base_s} s, S60 {s60:+.3f}: delay {at - now} ms (death 17 ms earlier: "
                  f"{at17 - now} ms after now), HUD {hud} s")
    return checks


def main():
    data = open(sys.argv[1], "rb").read()
    if hashlib.sha256(data).hexdigest() != base.EXE_SHA256:
        sys.exit("not the original 0.26g survarium.exe")
    print(f"booster modifiers, the original: {run(data)} checks as expected")


if __name__ == "__main__":
    main()
