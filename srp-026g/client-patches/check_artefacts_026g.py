#!/usr/bin/env python3
"""Run the original's and patch 54's artefact code on made-up artefacts,
configs, players and inventories.

    python check_artefacts_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, and
the game's own code runs: a Spring's loader 0x33ACF0 with the config lookup
0xF6B10 on a binary config written here (the keys and numbers of the
release's artefact configs), the link and unlink of a Spring's passive and
active nodes (0x311AD0, 0x311D10, 0x311BC0, 0x311DA0) into a made-up
player's modifier lists, the modifier getter 0x2D5AE0 on those lists,
Lifebone's enable_active 0x312250 with the pool lookup 0x2E58B0 and
increase_health 0x32A420, artefact_base::action 0x313300 over a
made-up inventory's artefact slots, and Onyx's enable_active 0x3128C0,
countdown 0x3127E0, end test 0x312A50 and hit callback 0x312BF0 on 1, 2
and 3 Onyx, and the end of Onyx's client update from 0x25F9DD (the
activation hint's set_parameterized_message 0x2375C0, a stub here). Stubs: the holder's two getters (the
inventory's +174h, slots 2 and 3), a pool's two notifications from
increase_health (0x32B920, 0x32A9C0), and the artefacts' enable_active in
the action cases (they count presses). Says nothing about the HUD, the
effects or a real match.

The original must give the original's answers (a Spring's passive node 2
max_carried_weight, no aim node; Lifebone heals no pool; one press fires one
artefact; n active Onyx reduce a hit n times, each 3 s), the build: aim
time from the config in the passive and the active, the active's linked and
unlinked with the others, pain, body and head full after Lifebone's active,
no slow from hits while a Spring is active, a press fires every ready
artefact of its class, and n Onyx fired together are each active 3 s +
(n - 1) s, reduce a hit once (x0.25) and show the activation hint once a
press, for that time.
"""
import json
import math
import os
import struct
import sys
import zlib

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, \
    UC_X86_REG_EDI, UC_X86_REG_ESP, UC_X86_REG_EBX, UC_X86_REG_EBP, UC_X86_REG_EIP, UC_X86_REG_XMM0

import patch_026g as base

SPRING_LOADER = 0x33ACF0                 # (esi = config at +340h; data), `ret 4`
PASSIVE_LINK, PASSIVE_UNLINK = 0x311AD0, 0x311D10   # eax = the Spring; stdcall (the Spring)
ACTIVE_LINK, ACTIVE_UNLINK = 0x311BC0, 0x311DA0
GETTER = 0x2D5AE0                        # eax = kit block +0F4h, esi = index; (base, k), `ret 8`
LIFEBONE_ACTIVE = 0x312250               # thiscall (time), `ret 4`
ACTION = 0x313300                        # thiscall (pressed, time), `ret 8`
NOTIFY, THRESHOLDS = 0x32B920, 0x32A9C0  # increase_health's notifications: stubs
STAGGER = 0x2DBCD0                       # eax = the player; (time), `ret 4`
INVENTORY_ACTION = 0x2FCAA0              # edi = the inventory; (slot, pressed, time), `ret 0Ch`
SPRING_ENABLE, SPRING_DISABLE = 0x3118D0, 0x311970   # thiscall: (time) `ret 4`; () `ret`
MOVEMENT, STAMINA_REGEN, MAX_WEIGHT, STAMINA_SPEND, AIM_TIME, STOPPING = 4, 3, 5, 19, 49, 42
AIM_NODE, STOP_NODE = 0x3B8, 0x3C8
ONYX_ENABLE = 0x3128C0                   # Onyx's enable_active, thiscall (time), `ret 4`
ONYX_TICK = 0x3127E0                     # its active's countdown, thiscall (elapsed ms, time), `ret 8`
ONYX_ENDED = 0x312A50                    # active_effect_ended, thiscall, al
ONYX_HIT = 0x312BF0                      # its hit callback, thiscall (hit, &damage, &piercing), `ret 0Ch`
ONYX_POPUP = 0x25F9DD                    # its client update's pop-up, state 4 seen (ebx = the Onyx +F20h)
MESSAGE = 0x2375C0                       # set_parameterized_message (ecx = the text; hud, size, shift, ms), `ret 10h`
LOCALIZE = 0x1FEF90                      # its text's last step: eax = the text, ecx = the output; (game), `ret 4`
from localization_026g import CATALOG, LANGUAGES, read_translations
HINTS = [read_translations(CATALOG / f'{language}.tsv')['st_onyx_activation_hint']
         for language, *_ in LANGUAGES]
# The release's Onyx (artefacts.tsv): passive x0.95, active x0.25 for 3 s,
# absorb_total 5; its loader's fields at +C7Ch (+26Ch.. of 0x33B780).
ONYX = {0xEE8: 7, 0xEF0: 0.95, 0xEF8: 0.95, 0xF00: 0.25, 0xF08: 0.25, 0xF0C: 5.0, 0xF10: 3000}

MEMORY, STACK, STOP, STUB = 0x20000000, 0x21008000, 0x22000000, 0x22000100
HINT, GAME = 0x22000800, 0x22001000      # an activation hint's text, the game (its hud at +1FCh)
HOLDER, HOLDER_VT, PLAYER, KITS, CONFIG, POOLS, DM, NAMES = (MEMORY + 0x10000 * i for i in range(1, 9))
GET_PLAYER, GET_DM, ENABLE_ACTIVE, RET4, RET8 = (STUB + 0x10 * i for i in range(5))

# The release's numbers (srp-026g/scripts/content-edit/artefacts.tsv).
SPRING = {"passive": {"movement_speed_mod": 0.05, "max_carried_weight_mod": 0.0, "aim_time_mod": -0.05},
          "active": {"movement_speed_mod": 0.1, "stamina_regen_mod": 5, "stamina_spend_mod": -5,
                     "duration": 10, "aim_time_mod": -0.1, "stopping_power_mod": -1.0}}
RETAIL = {"passive": {"movement_speed_mod": 0.02, "max_carried_weight_mod": 0},
          "active": {"movement_speed_mod": 0.1, "stamina_regen_mod": 5, "stamina_spend_mod": -5, "duration": 10}}


def config_bytes(tree, at):
    """A binary config of `tree` (dicts, ints, floats) as the game holds it
    loaded at `at`: 24-byte values {data, name, crc, type, count}, a named
    table's children sorted by the CRC of their names, offsets made
    addresses."""
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
            values[slot] = ("raw", struct.unpack("<I", struct.pack("<f", value))[0], key, crc, 2, 0)
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
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(MEMORY, 0x100000, UC_PROT_ALL)
        emu.mem_map(0x21000000, 0x10000, UC_PROT_ALL)
        emu.mem_map(STOP, 0x10000, UC_PROT_ALL)
        # The holder's getters: slot 3 the player, slot 2 the damage model's owner.
        emu.mem_write(GET_PLAYER, b"\xB8" + struct.pack("<I", PLAYER) + b"\xC3")
        emu.mem_write(GET_DM, b"\xB8" + struct.pack("<I", DM) + b"\xC3")
        emu.mem_write(ENABLE_ACTIVE, b"\xC2\x04\x00")
        emu.mem_write(RET4, b"\xC2\x04\x00")
        emu.mem_write(RET8, b"\xC2\x08\x00")
        self.word(HOLDER, HOLDER_VT)
        self.word(HOLDER_VT + 0x8, GET_DM)
        self.word(HOLDER_VT + 0xC, GET_PLAYER)
        self.word(PLAYER + 0x6B8, KITS)        # kit 0 (+115h is 0)
        self.word(DM, DM + 0x100)              # the owner: its pools at +108h
        self.next = MEMORY + 0xA0000
        self.pressed, self.notified = [], 0
        emu.hook_add(UC_HOOK_CODE, self.on_code, begin=STUB, end=STUB + 0x100)
        emu.hook_add(UC_HOOK_CODE, self.on_notify, begin=NOTIFY, end=NOTIFY)
        emu.hook_add(UC_HOOK_CODE, self.on_notify, begin=THRESHOLDS, end=THRESHOLDS)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def float(self, at):
        return struct.unpack("<f", self.emu.mem_read(at, 4))[0]

    def block(self, size):
        at, self.next = self.next, self.next + (size + 0xF) // 0x10 * 0x10
        self.emu.mem_write(at, bytes(size))
        return at

    def on_code(self, emu, address, size, _):
        if address == ENABLE_ACTIVE:
            self.pressed.append(emu.reg_read(UC_X86_REG_ECX))

    def on_notify(self, emu, address, size, _):
        # 0x32B920 (`ret 4`, eax = the pool's subscribers) and 0x32A9C0
        # (ecx = the pool): counted and returned from.
        self.notified += 1
        esp = emu.reg_read(UC_X86_REG_ESP)
        ret = self.read(esp)
        emu.reg_write(UC_X86_REG_ESP, esp + (8 if address == NOTIFY else 4))
        emu.reg_write(UC_X86_REG_EIP, ret)

    def call(self, va, args=(), **regs):
        esp = STACK - 0x100
        for value in reversed(args):
            esp -= 4
            self.word(esp, value)
        esp -= 4
        self.word(esp, STOP)
        self.emu.reg_write(UC_X86_REG_ESP, esp)
        for name, value in regs.items():
            self.emu.reg_write(REGISTER[name], value)
        self.emu.emu_start(va, STOP, count=2_000_000)

    # A Spring with the config `tree`, its holder the made-up player.
    def spring(self, tree):
        obj = self.block(0x400)
        config = self.block(0x400)
        self.emu.mem_write(config, config_bytes({"data": tree}, config))
        data = self.read(config) + 0               # the root's first child: `data`
        self.word(obj + 0x250, self.inventory())
        self.call(SPRING_LOADER, (data,), esi=obj + 0x340)
        return obj

    def inventory(self):
        if not hasattr(self, "_inventory"):
            self._inventory = self.block(0x200)
            self.word(self._inventory + 0x174, HOLDER)
        return self._inventory

    def node(self, at):
        return self.read(at + 8), round(self.float(at + 0xC), 6)

    def listed(self, index):
        """The values of the nodes in the player's list of `index`."""
        head = self.read(KITS + 0x204 + index * 0x14 + 4)
        values = []
        while head:
            values.append(round(self.float(head + 0xC), 6))
            head = self.read(head + 4)
        return values

    def count(self, index):
        return self.read(KITS + 0x204 + index * 0x14)

    def modifier(self, index, value):
        """The getter's M_index(value) for the player's kit 0."""
        self.call(GETTER, (struct.unpack("<I", struct.pack("<f", value))[0],
                           struct.unpack("<I", struct.pack("<f", 1.0))[0]), eax=KITS + 0xF4, esi=index)
        return round(struct.unpack("<f", self.emu.reg_read(UC_X86_REG_XMM0).to_bytes(16, "little")[:4])[0], 5)


REGISTER = {"eax": UC_X86_REG_EAX, "ecx": UC_X86_REG_ECX, "edx": UC_X86_REG_EDX, "esi": UC_X86_REG_ESI,
            "edi": UC_X86_REG_EDI, "ebx": UC_X86_REG_EBX}


def run(data, patched):
    checks = 0

    def expect(ok, what):
        nonlocal checks
        if not ok:
            sys.exit(f"{'the build' if patched else 'the original'}: {what}")
        checks += 1

    # The loader on the release's config.
    g = Game(data)
    obj = g.spring(SPRING)
    nodes = [g.node(obj + at) for at in (0x340, 0x350, 0x360, 0x370, 0x380)]
    want = [(MOVEMENT, 0.05), (AIM_TIME, -0.05) if patched else (MAX_WEIGHT, 0.0),
            (MOVEMENT, 0.1), (STAMINA_REGEN, 5.0), (STAMINA_SPEND, -5.0)]
    expect(nodes == want, f"the loader's nodes {nodes}, not {want}")
    expect(g.read(obj + 0x390) == 10000, "the active's duration 10 s")
    expect(g.node(obj + AIM_NODE) == ((AIM_TIME, -0.1) if patched else (0, 0.0)),
           f"the active's aim node {g.node(obj + AIM_NODE)}")
    expect(g.node(obj + STOP_NODE) == ((STOPPING, -1.0) if patched else (0, 0.0)),
           f"the active's stopping power node {g.node(obj + STOP_NODE)}")
    # The retail config in the original: its numbers.
    if not patched:
        r = Game(data)
        old = r.spring(RETAIL)
        nodes = [r.node(old + at) for at in (0x340, 0x350, 0x360, 0x370, 0x380)]
        expect(nodes == [(MOVEMENT, 0.02), (MAX_WEIGHT, 0.0), (MOVEMENT, 0.1), (STAMINA_REGEN, 5.0),
                         (STAMINA_SPEND, -5.0)], f"the retail config's nodes {nodes}")

    # Passive, then active, then the active's end, then the passive's end.
    g.call(PASSIVE_LINK, eax=obj)
    expect(g.listed(MOVEMENT) == [0.05], "the passive's speed node linked")
    expect(g.listed(AIM_TIME) == ([-0.05] if patched else []), "the passive's aim node linked")
    expect(g.modifier(AIM_TIME, 1.0) == (0.95 if patched else 1.0), "aim time x0.95 with the passive")
    expect(g.modifier(MOVEMENT, 1.0) == 1.05, "speed x1.05 with the passive")
    g.call(ACTIVE_LINK, eax=obj)
    expect(sorted(g.listed(MOVEMENT)) == [0.05, 0.1], "the active's speed node linked too")
    expect(sorted(g.listed(AIM_TIME)) == ([-0.1, -0.05] if patched else []), "the active's aim node linked")
    expect(g.count(AIM_TIME) == (2 if patched else 0), "two aim nodes counted")
    expect(g.modifier(AIM_TIME, 1.0) == (0.85 if patched else 1.0), "aim time x0.85 while active")
    expect(g.modifier(MOVEMENT, 1.0) == 1.15, "speed x1.15 while active")
    expect(g.listed(STAMINA_REGEN) == [5.0] and g.listed(STAMINA_SPEND) == [-5.0], "the stamina nodes")
    expect(g.listed(STOPPING) == ([-1.0] if patched else []), "the active's stopping power node linked")
    g.call(ACTIVE_UNLINK, (obj,))
    expect(g.listed(STOPPING) == [] and g.count(STOPPING) == 0, "the stopping power node out at its end")
    expect(g.listed(AIM_TIME) == ([-0.05] if patched else []) and g.count(AIM_TIME) == (1 if patched else 0),
           "the active's aim node out at its end")
    expect(g.listed(MOVEMENT) == [0.05] and g.listed(STAMINA_REGEN) == [], "the active's other nodes out")
    expect(g.modifier(AIM_TIME, 1.0) == (0.95 if patched else 1.0), "aim time x0.95 after the active")
    g.call(PASSIVE_UNLINK, (obj,))
    expect(g.listed(AIM_TIME) == [] and g.listed(MOVEMENT) == [] and g.count(AIM_TIME) == 0, "all out")
    # Two Springs, both active: their nodes add up.
    g = Game(data)
    one, two = g.spring(SPRING), g.spring(SPRING)
    for s in (one, two):
        g.call(PASSIVE_LINK, eax=s)
        g.call(ACTIVE_LINK, eax=s)
    expect(g.modifier(AIM_TIME, 1.0) == (0.7 if patched else 1.0), "two active Springs: aim time x0.7")
    expect(g.modifier(MOVEMENT, 1.0) == 1.3, "two active Springs: speed x1.3")
    g.call(ACTIVE_UNLINK, (one,))
    expect(g.modifier(AIM_TIME, 1.0) == (0.8 if patched else 1.0) and g.modifier(MOVEMENT, 1.0) == 1.2,
           "one of two actives ends: both passives and the other active")

    # The player's stagger: the recent hits' stopping power (within 0.5 s),
    # at most 0.5, as a movement_speed node of the player's (+2F4h); none
    # while a Spring is active.
    def stagger(hits, spring):
        g = Game(data)
        s = g.spring(SPRING)
        g.call(PASSIVE_LINK, eax=s)
        if spring:
            g.call(ACTIVE_LINK, eax=s)
        recent = g.block(8 * len(hits) + 8)
        for i, (age, power) in enumerate(hits):
            g.emu.mem_write(recent + 8 * i, struct.pack("<If", 10000 - age, power))
        g.word(PLAYER + 0x2E8, recent)
        g.word(PLAYER + 0x2EC, recent + 8 * len(hits))
        g.emu.mem_write(PLAYER + 0x31C, struct.pack("<ff", 0.5, 0.5))   # max_stagger, stagger_duration
        g.word(PLAYER + 0x2FC, MOVEMENT)
        g.call(STAGGER, (10000,), eax=PLAYER)
        return round(g.float(PLAYER + 0x300), 5)

    expect(stagger([(100, 0.15), (200, 0.3)], False) == -0.45, "two hits: -0.45 speed")
    expect(stagger([(100, 0.6), (200, 0.3)], False) == -0.5, "at most -0.5")
    expect(stagger([(100, 0.15), (700, 0.3)], False) == -0.15, "a hit older than 0.5 s is out")
    expect(stagger([(100, 0.15), (200, 0.3)], True) == (0.0 if patched else -0.45),
           "an active Spring: no slow" if patched else "an active Spring: the slow as ever")
    expect(stagger([(100, 0.6), (200, 0.3)], True) == (0.0 if patched else -0.5), "an active Spring, big hits")

    # Lifebone's active: pain, body and head full, the limbs left to their
    # remove_affects; a pool that is full or not there is left alone.
    cases = (
        # {pool: (Hmax, H before)} -> H after in the build
        ({"pain": (1.0, 0.3), "body": (1.0, 0.3), "head": (0.4, 0.1), "left_leg": (0.5, 0.1)},
         {"pain": 1.0, "body": 1.0, "head": 0.4, "left_leg": 0.1}),
        ({"pain": (1.0, 1.0), "body": (1.0, 0.7), "head": (0.4, 0.35)},
         {"pain": 1.0, "body": 1.0, "head": 0.4}),
        ({"pain": (1.0, 1.0), "body": (1.0, 1.0), "head": (0.4, 0.4)},
         {"pain": 1.0, "body": 1.0, "head": 0.4}),
        ({"body": (1.0, 0.2)}, {"body": 1.0}),
    )
    for before, after in cases:
        g = Game(data)
        life = g.block(0x500)
        g.word(life + 0x250, g.inventory())
        g.word(life + 0x48C, 3000)             # the active's duration, ms
        pools = {}
        for name, (hmax, h) in before.items():
            text = g.block(0x10)
            g.emu.mem_write(text, name.encode() + b"\0")
            pool = g.block(0x400)
            g.word(pool + 0x80, text)
            g.emu.mem_write(pool + 0x9C, struct.pack("<ff", hmax, h))
            if pools:
                g.word(list(pools.values())[-1], pool)
            pools[name] = pool
        g.word(DM + 0x100 + 0x108 + 8, list(pools.values())[0])   # the owner's pools' head
        g.call(LIFEBONE_ACTIVE, (77,), ecx=life)
        got = {name: round(g.float(pool + 0xA0), 5) for name, pool in pools.items()}
        want = after if patched else {name: h for name, (_, h) in before.items()}
        expect(got == want, f"the pools {got} after the active, not {want}")
        # increase_health: 0x32A9C0 after every heal, 0x32B920 when the pool is then full.
        healed = [name for name in before if want[name] != before[name][1]]
        full = [name for name in healed if want[name] == before[name][0]]
        expect(g.notified == len(healed) + len(full), f"{g.notified} notifications for {len(healed)} heals")
        expect(g.read(life + 0x494) == 3000, "the active's timer set as before")

    # One press over an inventory: two Springs, an Onyx, a Spring on its
    # cooldown, a Spring already active.
    g = Game(data)
    inventory = g.inventory()
    spring_vt, onyx_vt = g.block(0x100), g.block(0x100)
    for vt in (spring_vt, onyx_vt):
        g.word(vt + 0x2C, ACTION)
        g.word(vt + 0x80, ENABLE_ACTIVE)

    def artefact(slot, vt, state=3, cooldown=0):
        a = g.block(0x300)
        g.word(a, vt)
        g.word(a + 0x24C, 1)
        g.word(a + 0x250, inventory)
        g.word(a + 0x280, state)
        g.word(a + 0x28C, cooldown)
        g.word(inventory + 0x10C + 4 * slot, a)
        return a

    a, b = artefact(13, spring_vt), artefact(14, spring_vt)
    onyx = artefact(15, onyx_vt)
    cooling = artefact(16, spring_vt, cooldown=500)
    active = artefact(17, spring_vt, state=4)
    c = artefact(18, spring_vt)
    g.call(ACTION, (1, 1234), ecx=a)
    fired = [x for x in (a, b, onyx, cooling, active, c) if g.read(x + 0x280) == 4]
    want = [a, b, active, c] if patched else [a, active]
    expect(fired == want, "the press fires " + ("every ready Spring" if patched else "the pressed one"))
    expect(sorted(g.pressed) == sorted([a, b, c] if patched else [a]), "enable_active once for each fired")
    expect(all(g.read(x + 0x2A0) == 1234 for x in ([a, b, c] if patched else [a])), "their activation time")
    expect(g.read(onyx + 0x280) == 3 and g.read(cooling + 0x280) == 3, "the Onyx and the cooling Spring stay")
    # The Onyx pressed: it alone (no other Onyx).
    g.pressed = []
    g.call(ACTION, (1, 1300), ecx=onyx)
    expect(g.pressed == [onyx] and g.read(onyx + 0x280) == 4, "the Onyx alone")
    # A key-up, and a press of an active one: nothing.
    g.pressed = []
    g.call(ACTION, (0, 1400), ecx=cooling)
    g.call(ACTION, (1, 1400), ecx=a)
    expect(g.pressed == [], "a key-up or an active one: nothing")
    # A press of a cooling Spring's key, a ready one beside it: the ready one
    # fires in the build, nothing in the original.
    g.word(cooling + 0x28C, 500)
    ready = artefact(13, spring_vt)
    g.pressed = []
    g.call(ACTION, (1, 1500), ecx=cooling)
    expect(g.pressed == ([ready] if patched else []) and g.read(ready + 0x280) == (4 if patched else 3)
           and g.read(cooling + 0x280) == 3, "a cooling one's key fires the ready one of its kind")

    # Through the game's own callers, with what they pass: inventory::action
    # 0x2FCAA0 (edi = the inventory; slot, pressed, time) calls the pressed
    # item's vtable +2Ch (artefact_base::action, our entry) with (pressed,
    # time); its activation calls each Spring's own enable_active 0x3118D0
    # (thiscall, the time), which calls the active's link 0x311BC0 (ours
    # first) with eax = the Spring; the active's end, disable_active 0x311970
    # (thiscall), calls its unlink 0x311DA0 (ours first) with the Spring
    # pushed.
    g = Game(data)
    inventory = g.inventory()
    spring_vt = g.block(0x100)
    g.word(spring_vt + 0x2C, ACTION)
    g.word(spring_vt + 0x80, SPRING_ENABLE)
    one, two = g.spring(SPRING), g.spring(SPRING)
    for slot, s in ((13, one), (14, two)):
        g.word(s, spring_vt)
        g.word(s + 0x24C, 1)
        g.word(s + 0x280, 3)
        g.word(s + 0xD0, 1)                    # a reference held, as by the inventory
        g.word(inventory + 0x10C + 4 * slot, s)
        g.call(PASSIVE_LINK, eax=s)
    g.call(INVENTORY_ACTION, (13, 1, 2000), edi=inventory)
    expect([g.read(s + 0x280) for s in (one, two)] == ([4, 4] if patched else [4, 3])
           and g.read(two + 0x2A0) == (2000 if patched else 0),
           "the slot's key through inventory::action 0x2FCAA0 (slot, pressed, time): "
           + ("both Springs active at its time" if patched else "the pressed Spring alone"))
    expect(g.modifier(AIM_TIME, 1.0) == (0.7 if patched else 1.0)
           and g.modifier(MOVEMENT, 1.0) == (1.3 if patched else 1.2)
           and g.listed(STOPPING) == ([-1.0, -1.0] if patched else []),
           "their active nodes linked by the game's enable_active 0x3118D0 (eax = the Spring at 0x311946)")
    g.call(SPRING_DISABLE, ecx=one)
    expect(g.modifier(AIM_TIME, 1.0) == (0.8 if patched else 1.0)
           and g.listed(STOPPING) == ([-1.0] if patched else []) and g.count(STOPPING) == (1 if patched else 0),
           "one active's end through disable_active 0x311970 (the Spring pushed at 0x311973): its nodes out")
    g.call(INVENTORY_ACTION, (13, 0, 2100), edi=inventory)
    expect(g.read(one + 0x280) == 4 and g.read(one + 0xD0) == 1 and g.read(two + 0xD0) == 1,
           "a key-up through inventory::action: nothing, the reference given back")

    # Onyx, the game's own enable_active 0x3128C0, countdown 0x3127E0, end
    # test 0x312A50 and hit callback 0x312BF0 on made-up Onyx in the slots
    # 13.. (`states`: (state, cooldown) each), the release's numbers. A hit
    # (damage 1, piercing 0.5) goes through every Onyx's callback in turn, as
    # the protected pool's list calls them.
    def onyx_game(states):
        g = Game(data)
        inventory = g.inventory()
        vt = g.block(0x100)
        g.word(vt + 0x2C, ACTION)
        g.word(vt + 0x80, ONYX_ENABLE)
        stones = []
        for slot, (state, cooldown) in enumerate(states, 13):
            o = g.block(0xF40)
            g.word(o, vt)
            g.word(o + 0x24C, 1)
            g.word(o + 0x250, inventory)
            g.word(o + 0x280, state)
            g.word(o + 0x28C, cooldown)
            for at, value in ONYX.items():
                g.emu.mem_write(o + at, struct.pack("<f" if isinstance(value, float) else "<I", value))
            g.word(inventory + 0x10C + 4 * slot, o)
            g.word(o + 0x2F0, HINT)
            stones.append(o)
        g.messages = []
        g.emu.hook_add(UC_HOOK_CODE, message, g, begin=MESSAGE, end=MESSAGE)
        return g, stones

    def message(emu, address, size, g):
        esp = emu.reg_read(UC_X86_REG_ESP)
        g.messages.append((emu.reg_read(UC_X86_REG_ECX),) + struct.unpack("<4I", emu.mem_read(esp + 4, 16)))
        emu.reg_write(UC_X86_REG_ESP, esp + 0x14)
        emu.reg_write(UC_X86_REG_EIP, g.read(esp))

    def popups(g, stones):
        """Onyx's client update from 0x25F9DD for each Onyx in turn (its state
        seen turn 4, the holder the local player): the messages it shows, as
        (text, size, shift, ms)."""
        g.messages = []
        frame = STACK - 0x200
        for o in stones:
            for i, value in enumerate((0, 0, 0, 0, STOP)):   # edi, esi, ebx popped; ebp; the return
                g.word(frame + 4 * i, value)
            g.emu.reg_write(UC_X86_REG_ESP, frame)
            g.emu.reg_write(UC_X86_REG_EBP, frame + 12)
            g.emu.reg_write(UC_X86_REG_EBX, o + 0xF20)
            g.emu.reg_write(UC_X86_REG_ESI, GAME)
            g.emu.emu_start(ONYX_POPUP, STOP, count=100_000)
            expect(g.read(o + 0xF30) == 4, "the client update keeps the state it saw")
        return [(text, size, shift, ms) for text, hud, size, shift, ms in g.messages if hud == GAME + 0x1FC]

    def hit(g, stones):
        h = g.block(0x40)
        g.word(h + 0x1C, ONYX[0xEE8])
        values = g.block(8)
        g.emu.mem_write(values, struct.pack("<ff", 1.0, 0.5))
        for o in stones:
            g.call(ONYX_HIT, (h, values, values + 4), ecx=o)
        return round(g.float(values), 5), round(g.float(values + 4), 5)

    for n in (1, 2, 3):
        g, stones = onyx_game([(3, 0)] * n)
        expect(hit(g, stones) == (round(0.95 ** n, 5), round(0.5 * 0.95 ** n, 5)),
               f"{n} Onyx carried: their passives x0.95 each, as before")
        g.call(ACTION, (1, 4000), ecx=stones[-1])
        time = 3000 + (n - 1) * 1000 if patched else 3000
        expect([g.read(o + 0x280) for o in stones] == ([4] * n if patched else [3] * (n - 1) + [4])
               and all(g.read(o + 0xF14) == time for o in stones if g.read(o + 0x280) == 4)
               and all(g.float(o + 0xF18) == 5.0 for o in stones if g.read(o + 0x280) == 4),
               f"{n} Onyx, one press: " + (f"all {n} active {time} ms" if patched else "the pressed one, 3000 ms"))
        if not patched:
            for o in stones[:-1]:                # the original: each its own press
                g.call(ACTION, (1, 4000), ecx=o)
        want = (0.25, 0.125) if patched else (round(0.25 ** n, 5), round(0.5 * 0.25 ** n, 5))
        expect(hit(g, stones) == want, f"{n} Onyx active: damage and piercing x{want[0]}"
               + (", once" if patched else ", once for each"))
        shown = popups(g, stones)
        want = [(HINT, 0x21, 0x14, time)] if patched else [(HINT, 0x21, 0x14, 3000)] * n
        expect(shown == want, f"{n} Onyx: the activation hint " + (f"once, {time} ms on screen" if patched
                                                                  else "once for each, 3000 ms"))
        for o in stones:
            g.call(ONYX_TICK, (3000, 7000), ecx=o)
        g.call(ONYX_ENDED, ecx=stones[0])
        ended = g.emu.reg_read(UC_X86_REG_EAX) & 0xFF
        expect(ended == (1 if n == 1 or not patched else 0)
               and g.read(stones[0] + 0xF14) == (time - 3000),
               f"{n} Onyx after 3 s: " + ("ended" if ended else f"{time - 3000} ms left"))
    # An Onyx cooling beside a ready one: the press fires the ready one alone,
    # 3 s; the cooling one's passive and the active one's share both count.
    g, (cool, ready_one) = onyx_game([(3, 500), (3, 0)])
    g.call(ACTION, (1, 4000), ecx=ready_one)
    expect(g.read(cool + 0x280) == 3 and g.read(ready_one + 0x280) == 4 and g.read(ready_one + 0xF14) == 3000,
           "a cooling Onyx beside: the ready one alone, 3 s")
    expect(hit(g, (cool, ready_one)) == (0.2375, 0.11875), "the cooling one's passive x0.95, the active x0.25")
    # Two Onyx active from two presses (the second once ready): one share.
    g, (a1, a2) = onyx_game([(3, 0), (3, 500)])
    g.call(ACTION, (1, 4000), ecx=a1)
    g.word(a2 + 0x28C, 0)
    g.call(ACTION, (1, 5000), ecx=a2)
    expect([g.read(o + 0x280) for o in (a1, a2)] == [4, 4] and g.read(a2 + 0xF14) == 3000,
           "two presses, two active Onyx of 3 s")
    expect(hit(g, (a2, a1)) == ((0.25, 0.125) if patched else (0.0625, 0.03125)),
           "two active Onyx from two presses: " + ("x0.25 once" if patched else "x0.25 each"))
    expect(popups(g, (a1, a2)) == [(HINT, 0x21, 0x14, 3000)] * 2, "two presses: the hint for each, 3000 ms")

    # The text set_parameterized_message shows: the game's localizer 0x1FEF90
    # ({USE} the bound key, any other {...} nothing), then the hud's %s, the
    # time on screen in seconds (HUD.as). Each of the language hints keeps its %s
    # and says the time of 1, 2 and 3 Onyx; {s} would have been lost.
    def localized(text):
        source, out = g.block(len(text.encode()) + 1), g.block(0x400)
        g.emu.mem_write(source, text.encode() + b"\0")
        g.call(LOCALIZE, (0,), eax=source, ecx=out)
        return bytes(g.emu.mem_read(out, 0x400)).split(b"\0")[0].decode()

    expect(localized("for {s} s.") == "for  s." and localized("{{x") == "{x",
           "the localizer drops {s} (the owner's first test: \"for  s.\")")
    expect(len(HINTS) == len(LANGUAGES) and all(localized(h) == h and h.count("%s") == 1 and "75" in h for h in HINTS),
           "all Onyx activation hints come through the localizer as they are, one %s each")
    for n in (1, 2, 3):
        ms = 3000 + (n - 1) * 1000 if patched else 3000
        shown = [h.replace("%s", str(math.ceil(ms / 1000))) for h in HINTS]
        expect(shown[0] == f"Onyx activated! Decreases the damage and armor-piercing of incoming bullets by 75% "
                          f"for {ms // 1000} s." and all(f" {ms // 1000} " in s for s in shown),
               f"{n} Onyx: the hud shows \"{shown[0]}\"")
    return checks


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    patched, _ = base.build(original, "section", code, None, late_join)
    for data, name in ((original, "the original"), (patched, "the build")):
        print(f"patch 54, {name}: {run(data, data is patched)} checks as expected")


if __name__ == "__main__":
    main()
