#!/usr/bin/env python3
"""Patch 109 (the artefact tree, docs/notes/artefact-skills.md) in Unicorn on the built exe.

Resistance: body_part_parameters::hit_by_type's armour slice (0x32A083 .. 0x32A0B0, the
game's own clamp and multiply) for every hit type, Resistance 0..5 and refused payloads, on
two kits. Onyx II: the landing (0x2D8220) with and without the skill, an Onyx active, passive,
none, or another artefact active. Larkspur II: the real detoured action entry (0x313300) of a
Larkspur, a press, a repeated press and a key-up, and another kind's activation. Spring II: the
game's Spring loader and its passive and active links and unlinks (0x311AD0, 0x311BC0, 0x311DA0,
0x311D10) through patch 54, the reload modifier (13) by the game's getter, one and two Springs,
the skill taken away between two links. Resonance and Cascade, one aura over a constructed match
(its world core and roster; the engine world's game, its game world and match for the getters):
the shared Springs through the game's getters (0x2D5AE0, the weighted 0x2D5BC0) on every Spring
list and one other, beside the recipient's own Springs linked by the game; the shared Onyx through
hit_by_type's protectors' path (0x32A020 .. 0x32A058), each compared with the game's own Onyx
protect (0x312BF0) run on that Onyx; the shared Lifebone at body_part::regenerate's join
(0x32A53F, with patch 94's handler after it) compared with the Lifebone's own regeneration
callback (0x312590) on its record, and the whole regenerate (0x32A4B0) healing to full; the
team, life, roster, reach (8 m passives, 5 m actives), state and cap (4 of a kind, own first)
rules, a lobby (no match) and a kit that is not the player's current one. The cap of 4 copies of
a kind with own copies (check_own_cap): five own Springs through the getters, five own Onyx
through the game's protect, five own Lifebones through its regeneration callback act as four;
three own with Resonance's shared make four. The same cap for actives (check_active_cap): patch
54's press (the real action 0x313300, each kind's own enable_active) of five ready Springs, Onyx
or Lifebones fires four (getters and Onyx times exactly four's), the fifth ready; Cascade's
recipient takes up to 4 with its own first; a carrier shares only its copies that count. A
constructed player, not a played match.
Usage: python check_artefact_skills_026g.py ORIGINAL_EXE
"""
import struct
import sys
from unicorn.x86_const import *  # noqa: F403
import patch_026g as base
from unicorn import UC_HOOK_CODE
from check_artefacts_026g import PLAYER, STACK, STOP, HOLDER_VT, DM, ENABLE_ACTIVE, KITS, SPRING, ONYX, Game, \
    PASSIVE_LINK, PASSIVE_UNLINK, ACTIVE_LINK, ACTIVE_UNLINK, GETTER, ONYX_HIT
from check_medical_skills_026g import MedicalGame, bits, close

ARMOUR_AT = 0x32A083   # after `movss xmm0,[eax+8]` (the part's armour for the type), the hook
ARMOUR_END = 0x32A0B0  # after `movss [ebx],xmm1` (the damage written back)
RANKS = 40             # artefact_power: the packed ranks


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def hit(g, kind, armour, piercing, damage, pool):
    cell = g.block(16)
    g.word(cell, bits(damage))
    g.emu.reg_write(UC_X86_REG_ESP, STACK - 0x200)
    for reg, value in [(UC_X86_REG_XMM0, armour), (UC_X86_REG_XMM1, piercing), (UC_X86_REG_XMM3, damage)]:
        g.emu.reg_write(reg, bits(value))
    for reg, value in [(UC_X86_REG_EDI, kind), (UC_X86_REG_EBP, pool), (UC_X86_REG_EBX, cell)]:
        g.emu.reg_write(reg, value)
    g.emu.emu_start(ARMOUR_AT, ARMOUR_END, count=200_000)
    assert g.emu.reg_read(UC_X86_REG_EIP) == ARMOUR_END
    return struct.unpack('<f', g.emu.mem_read(cell, 4))[0]


def check_resistance(data, kit):
    outcomes = []
    for payload, rank in [(0, 0), (1, 1), (2, 2), (3, 3), (4, 4), (4 + 8 + 256, 4), (4096 + 4, 4),
                          (5, 0), (6, 0), (7, 0), (2.5, 0), (-1, 0), (16384, 0), (16384 + 4, 0), (8192 + 4, 4), (512 + 1, 1),
                          (7 * 512 + 1, 0), (8 + 7 * 512 + 4, 0), (8 + 6 * 512 + 4, 4)]:
        g = MedicalGame(data)
        g.emu.mem_write(PLAYER + 0x115, bytes([kit]))
        g.ranks(payload, RANKS)
        for name in ['body', 'head', 'left_leg']:
            pool = g.pools[PLAYER][name]
            for kind in range(8):
                for armour, piercing in [(0, 0), (0.3, 0), (0.3, 0.1), (0.95, 0), (1.0, 0), (0.1, 0.5)]:
                    have = hit(g, kind, armour, piercing, 100.0, pool)
                    a = f32(f32(armour + 0.06 * rank) if kind >= 4 and rank else armour)
                    want = 100.0 * (1 - min(max(f32(a - piercing), 0), 1))
                    close(have, want)
                    outcomes.append((payload, name, kind, armour, piercing, round(have, 4)))
    print(f'Resistance kit {kit}: anomalous hits (4..7) A + 0.06 a rank 0..4 before piercing and the clamp, '
          'other types and refused payloads (5, 6, 7, 2.5, -1, 16384, a draw of 7) unchanged, a draw without Keeper, Artifact Sight (bit 12) and the late mark (bit 13) accepted: PASS')
    return outcomes


LANDING = 0x2D8220      # base_player::on_physical_controller_landing (thiscall: the speed), ret 4
LANDING_BODY = 0x2D8226 # its second instruction: reached only when the landing goes on
ONYX_ACTIVE = 0x3128C0  # Onyx's enable_active (vtable +80h)
LIFEBONE_ACTIVE = 0x312250
ACTION = 0x313300       # artefact_base::action, patch 54's detoured entry (thiscall: pressed, time)
ONYX_II, LARKSPUR_II = 64, 128


def inventory(g):
    """The player's inventory: its six artefact containers (+168h the first, +16Ch the count) are
    slots 13..18 of its 23, the holder (+174h) the player's inventory_holder (+108h)."""
    at = g.block(0x200)
    g.word(PLAYER + 0x10C, at)
    g.word(PLAYER + 0x108, HOLDER_VT)
    g.word(at + 0x174, PLAYER + 0x108)
    g.word(at + 0x168, at + 0x10C + 13 * 4)
    g.emu.mem_write(at + 0x16C, bytes([6]))
    return at


def artefact(g, inv, slot, enable_active, state):
    vt = g.block(0x100)
    g.word(vt + 0x80, enable_active)
    item = g.block(0x500)
    for off, value in [(0, vt), (0x250, inv), (0x24C, 1), (0x280, state), (0x48C, 3000)]:
        g.word(item + off, value)
    g.word(inv + 0x10C + slot * 4, item)
    return item


def check_onyx(data, kit):
    outcomes = []
    for payload in [0, ONYX_II, ONYX_II + 4 + 32 + 128, LARKSPUR_II, ONYX_II + 0.5]:
        for carried in [[], [(ONYX_ACTIVE, 3)], [(ONYX_ACTIVE, 4)], [(ENABLE_ACTIVE, 4)],
                        [(ENABLE_ACTIVE, 4), (ONYX_ACTIVE, 3), (ONYX_ACTIVE, 4)]]:
            g = MedicalGame(data)
            g.emu.mem_write(PLAYER + 0x115, bytes([kit]))
            g.ranks(payload, RANKS)
            inv = inventory(g)
            for slot, (kind, state) in enumerate(carried, 13):
                artefact(g, inv, slot, kind, state)
            # Harmless and fatal speeds the landing compares with: 100 m/s, a landing at 1 m/s
            # does nothing in the game either, so only the hook decides whether it goes on.
            g.word(PLAYER + 0x30C, bits(100.0))
            g.word(PLAYER + 0x310, bits(100.0))
            reached = []
            hook = g.emu.hook_add(UC_HOOK_CODE, lambda *a: reached.append(1), begin=LANDING_BODY, end=LANDING_BODY)
            esp = STACK - 0x300
            g.word(esp, STOP)
            g.word(esp + 4, bits(1.0))
            g.emu.reg_write(UC_X86_REG_ESP, esp)
            g.emu.reg_write(UC_X86_REG_ECX, PLAYER)
            g.emu.emu_start(LANDING, STOP, count=500_000)
            g.emu.hook_del(hook)
            assert g.emu.reg_read(UC_X86_REG_EIP) == STOP
            assert g.emu.reg_read(UC_X86_REG_ESP) == esp + 8, "ret 4 either way"
            protected = payload in (ONYX_II, ONYX_II + 4 + 32 + 128) and (ONYX_ACTIVE, 4) in carried
            assert bool(reached) != protected, (payload, carried)
            outcomes.append((payload, len(carried), protected))
    print(f"Onyx II kit {kit}: the landing skipped only with the skill and an active Onyx (passive Onyx, "
          "another kind active, no skill, a refused payload: the game's landing): PASS")
    return outcomes


def check_larkspur(data, kit):
    outcomes = []
    for payload, kind in [(0, LIFEBONE_ACTIVE), (LARKSPUR_II, LIFEBONE_ACTIVE), (LARKSPUR_II + 4, LIFEBONE_ACTIVE),
                          (LARKSPUR_II, ENABLE_ACTIVE), (LARKSPUR_II + 0.5, LIFEBONE_ACTIVE)]:
        g = MedicalGame(data)
        g.emu.mem_write(PLAYER + 0x115, bytes([kit]))
        g.ranks(payload, RANKS)
        g.word(DM, g.read(PLAYER + 0x2E0))
        inv = inventory(g)
        item = artefact(g, inv, 13, kind, 3)
        stamina = PLAYER + 0x6C0
        g.word(stamina, bits(100.0))
        g.word(stamina + 0x6C, bits(10.0))
        g.call(ACTION, (1, 1234), ecx=item)
        assert g.read(item + 0x280) == 4, "activated"
        filled = payload in (LARKSPUR_II, LARKSPUR_II + 4) and kind == LIFEBONE_ACTIVE
        close(g.float(stamina + 0x6C), 100.0 if filled else 10.0)
        # Active now: a repeated press and a key-up do not fill it again.
        g.word(stamina + 0x6C, bits(10.0))
        for pressed in [1, 0]:
            g.call(ACTION, (pressed, 2234), ecx=item)
            close(g.float(stamina + 0x6C), 10.0)
        outcomes.append((payload, kind, filled))
    print(f"Larkspur II kit {kit}: a Larkspur's activation fills the stamina with the skill only; "
          "a repeated press, a key-up, another kind and a refused payload do nothing: PASS")
    return outcomes


SPRING_II = 32
RELOAD = 13


def check_spring(data):
    outcomes = []
    for payload, skill in [(0, False), (SPRING_II, True), (SPRING_II + 4 + 8, True), (SPRING_II + 0.5, False)]:
        g = Game(data)
        # The holder as the game's base_player embeds it (+108h), its getter the made-up one.
        g.word(PLAYER + 0x108, HOLDER_VT)
        g.word(g.inventory() + 0x174, PLAYER + 0x108)
        g.word(KITS + 0xF4 + RANKS * 4, bits(payload))
        k = 1 if skill else 0
        a, b = g.spring(SPRING), g.spring(SPRING)
        close(g.modifier(RELOAD, 1.0), 1.0)
        g.call(PASSIVE_LINK, eax=a)
        close(g.modifier(RELOAD, 1.0), 1 + 0.05 * k)
        g.call(ACTIVE_LINK, eax=a)
        close(g.modifier(RELOAD, 1.0), 1 + 0.15 * k)
        g.call(PASSIVE_LINK, eax=b)
        g.call(ACTIVE_LINK, eax=b)
        close(g.modifier(RELOAD, 1.0), 1 + 0.30 * k)
        g.call(ACTIVE_UNLINK, (b,))
        close(g.modifier(RELOAD, 1.0), 1 + 0.20 * k)
        g.call(ACTIVE_UNLINK, (a,))
        close(g.modifier(RELOAD, 1.0), 1 + 0.10 * k)
        g.call(PASSIVE_UNLINK, (b,))
        g.call(PASSIVE_UNLINK, (a,))
        close(g.modifier(RELOAD, 1.0), 1.0)
        assert g.count(RELOAD) == 0, "every reload node out"
        # Linked again (a world load, a transfer) after the skill is gone: the values follow.
        g.word(KITS + 0xF4 + RANKS * 4, bits(0))
        g.call(PASSIVE_LINK, eax=a)
        g.call(ACTIVE_LINK, eax=a)
        close(g.modifier(RELOAD, 1.0), 1.0)
        g.call(ACTIVE_UNLINK, (a,))
        g.call(PASSIVE_UNLINK, (a,))
        assert g.count(RELOAD) == 0
        outcomes.append((payload, skill))
    print("Spring II: each Spring's reload speed +5% passive, +15% while active (two Springs +10%/+30%), "
          "linked and unlinked with the game's own, values from the holder's skill at each link; "
          "no skill or a refused payload: none: PASS")
    return outcomes


RESONANCE, CASCADE = 16, 256
ENGINE_WORLD = 0x1F1D310    # the engine world's global: +2D0h its game, +18h its game world, +4D4Ch its match
WEIGHTED = 0x2D5BC0         # apply_modifiers (the kit's modifiers, &{begin, end} of {index, weight}, B), ret 0Ch
SPRING_ACTIVE, LIFEBONE_ACTIVE_, ONYX_ACTIVE_ = 0x3118D0, 0x312250, 0x3128C0
SPRING_LISTS = [4, 49, 3, 19, 42, 13]
# A Spring's nodes as the release's loader and patch 54 make them: {index: value} of its passive
# and active (artefacts.tsv); Spring II's reload (13) +0.05 / +0.10, 0 without the skill.
SPRING_PASSIVE = {4: 0.05, 49: -0.05}
SPRING_ACTIVE_NODES = {4: 0.1, 3: 5.0, 19: -5.0, 49: -0.1, 42: -1.0}


class Match:
    """A match around the medical fixture's player (place 0, team 0, at the origin): the engine
    world's chain (its game, the game's world, the world's match) to its world core, the getters'
    way to their kit's player; players at places with their own inventories and holders,
    artefacts in their containers."""

    def __init__(self, data, in_match=True):
        self.g = g = MedicalGame(data)
        if in_match:
            engine, game = g.block(0x300), g.block(0x5400)
            g.word(ENGINE_WORLD, engine)
            g.word(engine + 0x2D0, game)
            g.word(game + 0x18 + 0x4D4C, g.core - 0x1F8)
        self.inventories = {}
        self.inventory(PLAYER)

    def inventory(self, player):
        """The player's inventory: six containers (slots 13..18), its holder the player's own
        (+108h, a vtable whose +0Ch gives the player, as base_player's inventory_holder)."""
        g = self.g
        if player in self.inventories:
            return self.inventories[player]
        if player == PLAYER:
            g.word(PLAYER + 0x108, HOLDER_VT)
        else:
            stub, vt = g.block(8), g.block(0x10)
            g.emu.mem_write(stub, b'\xB8' + struct.pack('<I', player) + b'\xC3')
            g.word(vt + 0xC, stub)
            g.word(player + 0x108, vt)
        at = g.block(0x200)
        g.word(player + 0x10C, at)
        g.word(at + 0x174, player + 0x108)
        g.word(at + 0x168, at + 0x10C + 13 * 4)
        g.emu.mem_write(at + 0x16C, bytes([6]))
        self.inventories[player] = at
        return at

    def player(self, place, team, position, skills=0, alive=True):
        g = self.g
        at = g.player(place, team, position)
        g.word(g.read(at + 0x6B8) + 0xF4 + RANKS * 4, bits(skills))
        g.emu.mem_write(at + 0x33C, bytes([int(alive)]))
        self.inventory(at)
        return at

    def artefact(self, player, enable_active, state, size=0x1000):
        """An artefact of the kind `enable_active` in the player's next container."""
        g = self.g
        inv = self.inventory(player)
        slot = next(s for s in range(13, 19) if g.read(inv + 0x10C + s * 4) == 0)
        vt = g.block(0x100)
        g.word(vt + 0x80, enable_active)
        item = g.block(size)
        for off, value in [(0, vt), (0x250, inv), (0x24C, 1), (0x280, state)]:
            g.word(item + off, value)
        g.word(inv + 0x10C + slot * 4, item)
        return item

    def spring(self, player, state, link=True):
        """A Spring by the game's loader in the player's container; its passive linked (and its
        active in state 4) by the game, as a carried one is."""
        g = self.g
        item = g.spring(SPRING)
        inv = self.inventory(player)
        slot = next(s for s in range(13, 19) if g.read(inv + 0x10C + s * 4) == 0)
        vt = g.block(0x100)
        g.word(vt + 0x80, SPRING_ACTIVE)
        for off, value in [(0, vt), (0x250, inv), (0x24C, 1), (0x280, state)]:
            g.word(item + off, value)
        g.word(inv + 0x10C + slot * 4, item)
        if link:
            g.call(PASSIVE_LINK, eax=item)
            if state == 4:
                g.call(ACTIVE_LINK, eax=item)
        return item

    def getter(self, index, container=KITS + 0xF4):
        g = self.g
        g.call(GETTER, (bits(1.0), bits(1.0)), eax=container, esi=index)
        return struct.unpack('<f', (g.emu.reg_read(UC_X86_REG_XMM0) & 0xffffffff).to_bytes(4, 'little'))[0]

    def weighted(self, pairs, container=KITS + 0xF4):
        g = self.g
        vec, table = g.block(8), g.block(8 * len(pairs))
        for i, (index, weight) in enumerate(pairs):
            g.word(table + 8 * i, index)
            g.word(table + 8 * i + 4, bits(weight))
        g.word(vec, table)
        g.word(vec + 4, table + 8 * len(pairs))
        g.call(WEIGHTED, (container, vec, bits(1.0)))
        return struct.unpack('<f', (g.emu.reg_read(UC_X86_REG_XMM0) & 0xffffffff).to_bytes(4, 'little'))[0]


def spring_sum(index, passives, actives):
    """The nodes of list `index` of `passives` passive and `actives` active Springs (no Spring II)."""
    return SPRING_PASSIVE.get(index, 0.0) * passives + SPRING_ACTIVE_NODES.get(index, 0.0) * actives


def check_resonance_springs(data):
    outcomes = []
    # (what, the recipient's own Springs' states, carriers (place, team, position, skills, alive,
    # their Springs' states), the recipient alive, in a match, what it shares: (passives,
    # actives), Spring II's (passives, actives) of those).
    near = (3.0, 0.0, 0.0)
    cases = [
        ('one passive, Resonance at 3 m', [], [(1, 0, near, RESONANCE, True, [3])], True, True, (1, 0)),
        ('the reach: 8 m passives, 5 m actives, team, life', [],
         [(1, 0, near, RESONANCE, True, [3]), (2, 0, (0.0, 4.0, 0.0), CASCADE, True, [4]),
          (3, 1, (1.0, 0.0, 0.0), RESONANCE + CASCADE, True, [4]), (4, 0, (8.5, 0.0, 0.0), RESONANCE + CASCADE, True, [4]),
          (5, 0, (1.0, 0.0, 0.0), RESONANCE + CASCADE, False, [4]), (6, 0, (0.0, 0.0, 6.0), RESONANCE + CASCADE, True, [4]),
          (7, 0, (0.0, 7.99, 0.0), RESONANCE, True, [3, 4]), (8, 0, (0.0, 0.0, -4.99), CASCADE, True, [3])],
         True, True, (4, 1)),
        ('no skill, nothing', [], [(1, 0, near, 0, True, [3, 4])], True, True, (0, 0)),
        ('a cooling or collected Spring', [], [(1, 0, near, RESONANCE + CASCADE, True, [1, 5])], True, True, (0, 0)),
        ('two own and three near: two shared', [3, 3], [(1, 0, near, RESONANCE, True, [3, 3, 3])], True, True, (2, 0)),
        ('three own, two near active: one', [3, 3, 4], [(1, 0, near, RESONANCE + CASCADE, True, [4, 4])], True, True, (1, 1)),
        ('four own: none', [3, 3, 3, 3], [(1, 0, near, RESONANCE + CASCADE, True, [4])], True, True, (0, 0)),
        ('two carriers, place order', [], [(1, 0, near, RESONANCE, True, [3, 3, 3]), (2, 0, near, RESONANCE, True, [3, 3])],
         True, True, (4, 0)),
        ('a dead recipient', [], [(1, 0, near, RESONANCE + CASCADE, True, [4])], False, True, (0, 0)),
        ('the lobby: no match', [], [(1, 0, near, RESONANCE + CASCADE, True, [4])], True, False, (0, 0)),
    ]
    for what, own, carriers, alive, in_match, (passives, actives) in cases:
        m = Match(data, in_match)
        g = m.g
        g.emu.mem_write(PLAYER + 0x33C, bytes([int(alive)]))
        for state in own:
            m.spring(PLAYER, state)
        for place, team, position, skills, living, states in carriers:
            at = m.player(place, team, position, skills, living)
            for state in states:
                m.spring(at, state)
        own_p = len(own)
        own_a = sum(1 for s in own if s == 4)
        for index in SPRING_LISTS + [20, 2]:
            have = m.getter(index)
            want = max(0.0, 1 + spring_sum(index, own_p, own_a) + spring_sum(index, passives, actives))
            close(have, want)
            outcomes.append((what, index, round(have, 4)))
        # The weighted getter (movement_speed's {4, 1}, {20, 1}).
        close(m.weighted([(4, 1.0), (20, 1.0)]), max(0.0, 1 + spring_sum(4, own_p + passives, own_a + actives)))
        # Another kit of the same profile is no current kit of a player: nothing shared.
        close(m.getter(4, KITS + 0xF4 + 2 * 0x704), 1.0)
    # Spring II's reload nodes: the carrier's skill, linked by the game, shared as the others.
    m = Match(data)
    at = m.player(1, 0, near, RESONANCE + CASCADE + SPRING_II)
    m.spring(at, 4)
    close(m.getter(13), 1 + 0.05 + 0.10)
    m = Match(data)
    at = m.player(1, 0, near, RESONANCE + CASCADE)
    m.spring(at, 4)
    close(m.getter(13), 1.0)
    print("Resonance and Cascade, Spring: the game's getters (one index, the weighted sum) add a teammate's "
          "Springs' passive nodes within 8 m with Resonance and active nodes within 5 m with Cascade on every "
          "Spring list (Spring II's reload too), none for an enemy, a dead or distant carrier, a cooling or "
          "collected Spring, a dead recipient, the lobby or another kit; at most 4 of a kind with the own ones "
          "(two own and three near: two shared): PASS")
    return outcomes


TICK_SITE = 0x2DAAD3        # base_player::tick after the held object's tick (battery.carrier_tick), EBX the player
TICK_ANIMATIONS = 0x2DAADB  # its `call tick_animations` (0x2D8EE0): select_animations runs there only with +33Eh


QUERY = 0xE5B60             # query_resources (cdecl), which Forest Protection's glow (patch 111) calls at its first tick


def tick(m, player, time):
    """The player's tick at the site, its time (+324h) `time` ms, up to its tick_animations call;
    whether the animations are to be selected again (+33Eh) then, cleared for the next. The other
    hooks of the site run too (the battery's; the glow's, its resource query answered nothing)."""
    g = m.g
    if not getattr(m, 'query_stubbed', False):
        def no_query(emu, address, size, _):
            esp = emu.reg_read(UC_X86_REG_ESP)
            emu.reg_write(UC_X86_REG_EIP, g.read(esp))
            emu.reg_write(UC_X86_REG_ESP, esp + 4)
        g.emu.hook_add(UC_HOOK_CODE, no_query, begin=QUERY, end=QUERY)
        m.query_stubbed = True
    g.word(player + 0x324, time)
    g.emu.mem_write(player + 0x33E, b'\0')
    g.emu.reg_write(UC_X86_REG_ESP, STACK - 0x100)
    g.emu.reg_write(UC_X86_REG_EBX, player)
    g.emu.reg_write(UC_X86_REG_EDI, 0)
    g.emu.emu_start(TICK_SITE, TICK_ANIMATIONS, count=2_000_000)
    assert g.emu.reg_read(UC_X86_REG_EIP) == TICK_ANIMATIONS
    return g.emu.mem_read(player + 0x33E, 1)[0] == 1


def check_resonance_movement(data):
    """B runs with one input (+33Eh is set by nothing else); A, a teammate with Resonance and a
    passive Spring, comes into reach, leaves, comes back and dies. movement_speed's sum (the
    weighted getter, {4, 1} {20, 1}) is what select_animations would take."""
    m = Match(data)
    g = m.g
    far, near = (20.0, 0.0, 0.0), (3.0, 0.0, 0.0)
    a = m.player(1, 0, far, RESONANCE)
    m.spring(a, 3)
    speed = lambda: m.weighted([(4, 1.0), (20, 1.0)])
    at = lambda position: g.emu.mem_write(a + 0x43C, struct.pack('<fff', *position))
    steps = []
    def step(time, want, speed_want):
        steps.append((time, tick(m, PLAYER, time)))
        assert steps[-1][1] == want, (time, steps)
        close(speed(), speed_want)
    base, shared = 1.0, 1.0 + SPRING_PASSIVE[4]
    step(1000, False, base)            # out of reach: nothing shared, nothing to select
    step(1050, False, base)
    at(near)
    step(1100, True, shared)           # A in reach: selected again, the new speed
    step(1150, False, shared)          # no change
    step(1200, False, shared)
    at(far)
    step(1250, True, base)             # A left
    at(near)
    step(1300, True, shared)
    g.emu.mem_write(a + 0x33C, b'\0')
    step(1350, True, base)             # A died
    step(1400, False, base)
    # A re-simulation of 1100..1150 (A alive and near again): each tick as at first.
    g.emu.mem_write(a + 0x33C, b'\1')
    step(1100, True, shared)
    step(1150, False, shared)
    # An enemy's Spring and a Spring without the skill: nothing shared, nothing selected.
    m = Match(data)
    e = m.player(1, 1, (3.0, 0.0, 0.0), RESONANCE)
    m.spring(e, 3)
    n = m.player(2, 0, (3.0, 0.0, 0.0), 0)
    m.spring(n, 3)
    assert not tick(m, PLAYER, 1000) and not tick(m, PLAYER, 1050)
    # The lobby (no match): nothing.
    m = Match(data, in_match=False)
    a = m.player(1, 0, (3.0, 0.0, 0.0), RESONANCE)
    m.spring(a, 3)
    assert not tick(m, PLAYER, 1000)
    print("Resonance, Spring movement: at base_player::tick before tick_animations a running player's "
          "animations are selected again (+33Eh) at the tick its shared movement changes (a teammate's "
          "Spring in reach, out of it, its carrier dead), the getter movement_speed reads then giving the "
          "new speed; not while it stays, nor for an enemy's or a skill-less Spring, nor in the lobby; a "
          "re-simulated tick answers as at first: PASS")
    return steps


def onyx(m, player, state, parts=('body', 'head')):
    """An Onyx with the release's numbers (absorb 5 from its enable_active), protecting `parts`."""
    g = m.g
    item = m.artefact(player, ONYX_ACTIVE_, state)
    for at, value in ONYX.items():
        g.emu.mem_write(item + at, struct.pack('<f' if isinstance(value, float) else '<I', value))
    g.word(item + 0xF18, bits(5.0))
    records = g.block(0x1C * len(parts))
    for i, name in enumerate(parts):
        text = g.block(16)
        g.emu.mem_write(text, name.encode() + b'\0')
        g.word(records + 0x1C * i, text)
    g.word(item + 0xC90, records)
    g.word(item + 0xC94, records + 0x1C * len(parts))
    return item


def native_protect(m, stone, state, kind, damage, piercing):
    """The game's own protect (0x312BF0, patch 54's detour first) of an Onyx like `stone` in
    `state`, alone (no inventory): what that Onyx does to a hit for its holder."""
    g = m.g
    ref = g.block(0x1000)
    g.emu.mem_write(ref, bytes(g.emu.mem_read(stone, 0x1000)))
    g.word(ref + 0x250, 0)
    g.word(ref + 0x280, state)
    hit, cells = g.block(0x40), g.block(8)
    g.word(hit + 0x1C, kind)
    g.word(cells, bits(damage))
    g.word(cells + 4, bits(piercing))
    g.call(ONYX_HIT, (hit, cells, cells + 4), ecx=ref)
    return g.float(cells), g.float(cells + 4)


PROTECTORS_AT, PROTECTORS_END = 0x32A020, 0x32A058   # the hook, after the part's protectors' loop


def protected(m, kind, part, damage, piercing):
    """hit_by_type from after apply_hit_modifiers through the part's protectors (none of its
    own here): EBP the recipient's pool, EDI the hit, EBX &damage, ESP+144h &piercing."""
    g = m.g
    pool = g.pools[PLAYER][part]
    g.word(pool + 0xC4, 0)
    hit, cells = g.block(0x40), g.block(8)
    g.word(hit + 0x1C, kind)
    g.word(cells, bits(damage))
    g.word(cells + 4, bits(piercing))
    stack = [0] * 0x60
    stack[0x144 // 4] = cells + 4
    g.step(PROTECTORS_AT, PROTECTORS_END - PROTECTORS_AT, stack=stack, ebp=pool, edi=hit, ebx=cells)
    return g.float(cells), g.float(cells + 4)


def check_resonance_onyx(data):
    outcomes = []
    near, kind = (3.0, 0.0, 0.0), 7
    # (what, own Onyx states, carriers (place, team, position, skills, alive, Onyx states),
    # the hit's type, the part, (shared passives applied, a shared active applied)).
    cases = [
        ('one passive, Resonance at 3 m', [], [(1, 0, near, RESONANCE, True, [3])], kind, 'body', (1, 0)),
        ('three passives near, one own: three', [3], [(1, 0, near, RESONANCE, True, [3, 3]), (2, 0, near, RESONANCE, True, [3, 3])],
         kind, 'body', (3, 0)),
        ('an active by Cascade at 4 m, twice: once', [], [(1, 0, (0.0, 4.0, 0.0), CASCADE, True, [4]), (2, 0, near, CASCADE, True, [4])],
         kind, 'head', (0, 1)),
        ('both skills: passive and active', [], [(1, 0, near, RESONANCE + CASCADE, True, [3, 4])], kind, 'body', (1, 1)),
        ('own active: no shared active', [4], [(1, 0, near, RESONANCE + CASCADE, True, [3, 4])], kind, 'body', (1, 0)),
        ('an active Onyx gives no passive', [], [(1, 0, near, RESONANCE, True, [4])], kind, 'body', (0, 0)),
        ('another hit type', [], [(1, 0, near, RESONANCE + CASCADE, True, [3, 4])], 0, 'body', (0, 0)),
        ('a part it does not protect', [], [(1, 0, near, RESONANCE + CASCADE, True, [3, 4])], kind, 'left_leg', (0, 0)),
        ('enemy, dead, too far', [], [(1, 1, near, RESONANCE + CASCADE, True, [3, 4]), (2, 0, near, RESONANCE + CASCADE, False, [3, 4]),
                                      (3, 0, (8.1, 0, 0), RESONANCE, True, [3]), (4, 0, (5.1, 0, 0), CASCADE, True, [4])],
         kind, 'body', (0, 0)),
        ('four own: none', [3, 3, 3, 3], [(1, 0, near, RESONANCE + CASCADE, True, [3, 4])], kind, 'body', (0, 0)),
    ]
    for what, own, carriers, hit_kind, part, (passives, actives) in cases:
        m = Match(data)
        for state in own:
            onyx(m, PLAYER, state)
        stone = None
        for place, team, position, skills, living, states in carriers:
            at = m.player(place, team, position, skills, living)
            for state in states:
                stone = onyx(m, at, state)
        for damage, piercing in [(100.0, 0.4), (4.0, 0.9), (0.0, 0.0)]:
            have = protected(m, hit_kind, part, damage, piercing)
            want = (damage, piercing)
            for _ in range(passives):
                want = native_protect(m, stone, 3, hit_kind, *want)
            for _ in range(actives):
                want = native_protect(m, stone, 4, hit_kind, *want)
            close(have[0], want[0])
            close(have[1], want[1])
            outcomes.append((what, damage, round(have[0], 4), round(have[1], 4)))
    print("Resonance and Cascade, Onyx: before a part's protectors, each teammate's passive Onyx within 8 m "
          "(Resonance) and one active Onyx within 5 m (Cascade, none while one of the own is active) do what "
          "the game's own protect does with that Onyx; another hit type, an unprotected part, an active Onyx's "
          "passive, an enemy, a dead or distant carrier, four own: nothing: PASS")
    return outcomes


REGENERATE_JOIN = 0x32A53F
LIFEBONE_CALLBACK = 0x312590   # a Lifebone record's regeneration modifier: (frame, record), ret 8


def lifebone(m, player, state, record):
    """A Lifebone whose passive has `record` {name: (rate +, x, delay s +, x, limit +, x)}."""
    g = m.g
    item = m.artefact(player, LIFEBONE_ACTIVE_, state, 0x500)
    first = g.block(0x34 * len(record))
    for i, (name, values) in enumerate(record.items()):
        text = g.block(16)
        g.emu.mem_write(text, name.encode() + b'\0')
        g.word(first + 0x34 * i, text)
        g.emu.mem_write(first + 0x34 * i + 0x1C, struct.pack('<6f', *values))
    g.word(item + 0x354, first)
    g.word(item + 0x358, first + 0x34 * len(record))
    return item


def check_resonance_lifebone(data):
    outcomes = []
    near = (3.0, 0.0, 0.0)
    release = {'body': (0.1, 1.0, 0.0, 1.0, 1.0, 1.0), 'head': (0.1, 1.0, 0.0, 1.0, 1.0, 1.0)}
    odd = {'body': (0.05, 1.5, -2.0, 0.75, 0.25, 0.8)}
    # (what, own Lifebone states, carriers, the record, the part, how many shared)
    cases = [
        ('one, Resonance at 3 m', [], [(1, 0, near, RESONANCE, True, [3])], release, 'body', 1),
        ('active still gives its passive; two', [], [(1, 0, near, RESONANCE, True, [4, 3])], release, 'head', 2),
        ('other numbers, twice', [], [(1, 0, near, RESONANCE, True, [3, 3])], odd, 'body', 2),
        ('three own: one', [3, 3, 3], [(1, 0, near, RESONANCE, True, [3, 3])], release, 'body', 1),
        ('a part without its record', [], [(1, 0, near, RESONANCE, True, [3])], release, 'left_leg', 0),
        ('Cascade shares no passive; enemy, dead, far', [],
         [(1, 0, near, CASCADE, True, [3]), (2, 1, near, RESONANCE, True, [3]), (3, 0, near, RESONANCE, False, [3]),
          (4, 0, (0, 8.1, 0), RESONANCE, True, [3])], release, 'body', 0),
    ]
    for what, own, carriers, record, part, shared in cases:
        m = Match(data)
        g = m.g
        for state in own:
            lifebone(m, PLAYER, state, record)
        for place, team, position, skills, living, states in carriers:
            at = m.player(place, team, position, skills, living)
            for state in states:
                lifebone(m, at, state, record)
        pool = g.pools[PLAYER][part]
        for rate, delay, limit in [(0.03, 8000, 0.5), (0.0, 1000, 0.2)]:
            stack = [0] * 12
            stack[4], stack[5], stack[6], stack[9] = bits(rate), delay, bits(limit), 12345
            g.step(REGENERATE_JOIN, 6, stack=stack, eax=delay, edi=pool)
            have = (g.float(STACK - 0x100 + 0x10), g.emu.reg_read(UC_X86_REG_EAX), g.float(STACK - 0x100 + 0x18))
            assert g.emu.reg_read(UC_X86_REG_EBP) == 12345
            # The game's own Lifebone callback on the same record, as many times.
            frame = g.block(12)
            g.word(frame, bits(rate))
            g.word(frame + 4, delay)
            g.word(frame + 8, bits(limit))
            entry = g.block(0x34)
            if part in record:
                g.emu.mem_write(entry + 0x1C, struct.pack('<6f', *record[part]))
            for _ in range(shared):
                g.call(LIFEBONE_CALLBACK, (frame, entry), ecx=0)
            want = (g.float(frame), g.read(frame + 4), g.float(frame + 8))
            close(have[0], want[0])
            assert have[1] == want[1], (what, have, want)
            close(have[2], want[2])
            outcomes.append((what, rate, have))
    # The whole regenerate: a body at 10 % regenerates to its 50 % alone, to full with one shared
    # Lifebone (its limit +1), never before its delay.
    for share, now, expected in [(False, 5000, 0.1), (False, 200000, 0.5), (True, 5000, 0.1), (True, 200000, 1.0)]:
        m = Match(data)
        g = m.g
        at = m.player(1, 0, near, RESONANCE if share else 0)
        lifebone(m, at, 3, release)
        body = g.pools[PLAYER]['body']
        g.word(body + 0xA0, bits(0.1))
        g.word(body + 0xA4, bits(0.03))
        g.word(body + 0xA8, bits(0.5))
        g.word(body + 0xB0, 0)
        g.call(0x32A4B0, (now, now), edi=body)
        close(g.float(body + 0xA0), expected)
    print("Resonance, Lifebone: at a part's regeneration tick each teammate's Lifebone within 8 m does to its "
          "rate, delay and limit what the game's own Lifebone callback does with its record for the part, "
          "with patch 94's handler after it; the whole regenerate heals to full with one; three own leave one; "
          "a part without a record, Cascade alone, an enemy, a dead or distant carrier: nothing: PASS")
    return outcomes


def check_own_cap(data):
    """At most 4 copies of a kind count for a player, own copies included (the owner, 2026-11-07):
    a fifth own copy (Artifact Container) gives no passive where the game applies it, and three
    own with Resonance's shared still make four."""
    outcomes = []
    near = (3.0, 0.0, 0.0)
    pairs = [(4, 1.0), (20, 1.0)]

    # Spring: the game's getters on the recipient's own Springs, linked by the game in container
    # order (the fifth container's last). (own states, carriers' Springs by Resonance, the
    # passives and actives that count.)
    def springs(own, carried=(), in_match=True):
        m = Match(data, in_match)
        for state in own:
            m.spring(PLAYER, state)
        if carried:
            at = m.player(1, 0, near, RESONANCE)
            for state in carried:
                m.spring(at, state)
        return [m.getter(index) for index in SPRING_LISTS + [20, 2]] + [m.weighted(pairs)]
    four = springs([3, 3, 3, 3])
    assert springs([3, 3, 3, 3, 3]) == four, "five own passive Springs give exactly four's"
    for what, own, carried, in_match, (passives, actives) in [
        ('five own', [3, 3, 3, 3, 3], (), True, (4, 0)),
        ('five own, the fifth active: its active only', [3, 3, 3, 3, 4], (), True, (4, 1)),
        ('five own, the first active', [4, 3, 3, 3, 3], (), True, (4, 1)),
        ('five own and a carrier: none shared', [3, 3, 3, 3, 3], [3, 3], True, (4, 0)),
        ('three own and three shared: four', [3, 3, 3], [3, 3, 3], True, (4, 0)),
        ('no match: the game sums them all', [3, 3, 3, 3, 3], (), False, (5, 0)),
    ]:
        have = springs(own, carried, in_match)
        for index, value in zip(SPRING_LISTS + [20, 2], have):
            close(value, max(0.0, 1 + spring_sum(index, passives, actives)))
        close(have[-1], max(0.0, 1 + spring_sum(4, passives, actives)))
        outcomes.append((what, [round(v, 4) for v in have]))
    assert springs([3, 3, 3], [3, 3, 3]) == springs([3, 3, 3], [3]), "three own take one shared"

    # Onyx: each own Onyx's protect (0x312BF0) as the part's protectors call it, in container
    # order, after the shared ones (0x32A020); compared with the game's protect on a copy.
    kind = 7
    for what, own, carried, (passives, active) in [
        ('five own passive', [3, 3, 3, 3, 3], (), (4, False)),
        ('five own, the first active: three passives and its active', [4, 3, 3, 3, 3], (), (3, True)),
        ('four own: all four', [3, 3, 3, 3], (), (4, False)),
        ('three own and two shared: four', [3, 3, 3], [3, 3], (4, False)),
        ('five own and a carrier: four', [3, 3, 3, 3, 3], [3, 3], (4, False)),
    ]:
        m = Match(data)
        g = m.g
        stones = [onyx(m, PLAYER, state) for state in own]
        if carried:
            at = m.player(1, 0, near, RESONANCE)
            for state in carried:
                onyx(m, at, state)
        for damage, piercing in [(100.0, 0.4), (4.0, 0.9)]:
            have = protected(m, kind, 'body', damage, piercing)
            hit, cells = g.block(0x40), g.block(8)
            g.word(hit + 0x1C, kind)
            g.word(cells, bits(have[0]))
            g.word(cells + 4, bits(have[1]))
            for stone in stones:
                g.call(ONYX_HIT, (hit, cells, cells + 4), ecx=stone)
            have = (g.float(cells), g.float(cells + 4))
            want = (damage, piercing)
            if active:
                want = native_protect(m, stones[0], 4, kind, *want)
            for _ in range(passives):
                want = native_protect(m, stones[-1], 3, kind, *want)
            close(have[0], want[0])
            close(have[1], want[1])
            outcomes.append((what, damage, round(have[0], 4)))

    # Lifebone: each own Lifebone's regeneration modifier (0x312590, ECX the Lifebone, as its
    # subscriber's invoker calls it) with its record for the part, then the shared ones at the
    # join (0x32A53F); compared with the game's callback on that record.
    release = {'body': (0.1, 1.0, 0.0, 1.0, 1.0, 1.0)}
    odd = {'body': (0.05, 1.5, -2.0, 0.75, 0.25, 0.8)}
    for what, own, carried, record, counted in [
        ('five own', [3, 3, 3, 3, 3], (), odd, 4),
        ('five own, one active', [3, 4, 3, 3, 3], (), odd, 4),
        ('four own', [3, 3, 3, 3], (), odd, 4),
        ('three own and two shared: four', [3, 3, 3], [3, 3], release, 4),
        ('five own and a carrier: four', [3, 3, 3, 3, 3], [3], release, 4),
    ]:
        m = Match(data)
        g = m.g
        bones = [lifebone(m, PLAYER, state, record) for state in own]
        if carried:
            at = m.player(1, 0, near, RESONANCE)
            for state in carried:
                lifebone(m, at, state, record)
        pool = g.pools[PLAYER]['body']
        rate, delay, limit = 0.03, 8000, 0.5
        frame = g.block(12)
        g.word(frame, bits(rate))
        g.word(frame + 4, delay)
        g.word(frame + 8, bits(limit))
        for bone in bones:
            g.call(LIFEBONE_CALLBACK, (frame, g.read(bone + 0x354)), ecx=bone)
        stack = [0] * 12
        stack[4], stack[5], stack[6], stack[9] = g.read(frame), g.read(frame + 4), g.read(frame + 8), 12345
        g.step(REGENERATE_JOIN, 6, stack=stack, eax=g.read(frame + 4), edi=pool)
        have = (g.float(STACK - 0x100 + 0x10), g.emu.reg_read(UC_X86_REG_EAX), g.float(STACK - 0x100 + 0x18))
        native = g.block(12)
        g.word(native, bits(rate))
        g.word(native + 4, delay)
        g.word(native + 8, bits(limit))
        entry = g.block(0x34)
        g.emu.mem_write(entry + 0x1C, struct.pack('<6f', *record['body']))
        for _ in range(counted):
            g.call(LIFEBONE_CALLBACK, (native, entry), ecx=0)
        want = (g.float(native), g.read(native + 4), g.float(native + 8))
        close(have[0], want[0])
        assert have[1] == want[1], (what, have, want)
        close(have[2], want[2])
        outcomes.append((what, have))
    print("At most 4 copies of a kind count, own copies included: five own Springs give the game's getters "
          "(one index, the weighted sum) exactly four's passive nodes (a fifth active keeps its active), five "
          "own Onyx protect a hit as four through the game's protect (the first active as its active), five own "
          "Lifebones regenerate as four through the game's callback; three own with Resonance take one shared "
          "(four in all), five own none; outside a match the getters sum them all: PASS")
    return outcomes


def check_active_cap(data):
    """At most 4 copies of a kind count when activated too (the owner, 2026-11-07): patch 54's
    press (the real detoured action 0x313300, each copy's own enable_active) fires at most the 4
    copies of the kind that count for the holder (the first 4 in their passive or active, in
    container order); a fifth is not fired and stays ready; Cascade's recipient takes shared
    actives up to 4 with its own counting first, and a carrier shares only its copies that count."""
    outcomes = []
    near = (3.0, 0.0, 0.0)

    def stack(m, make, states):
        """Copies in the player's containers made by `make(state)`, one class: the first's vtable."""
        copies = [make(state) for state in states]
        for copy in copies[1:]:
            m.g.word(copy, m.g.read(copies[0]))
        return copies

    def press(m, copies, at, time=4000):
        m.g.call(ACTION, (1, time), ecx=copies[at])
        return [m.g.read(c + 0x280) for c in copies]

    def springs_match(states):
        m = Match(data)
        return m, stack(m, lambda state: m.spring(PLAYER, state), states)

    def onyx_match(states):
        m = Match(data)
        return m, stack(m, lambda state: onyx(m, PLAYER, state), states)

    def lifebone_match(states):
        m = Match(data)
        m.g.word(DM, m.g.read(PLAYER + 0x2E0))
        record = {'body': (0.1, 1.0, 0.0, 1.0, 1.0, 1.0)}

        def make(state):
            bone = lifebone(m, PLAYER, state, record)
            m.g.word(bone + 0x48C, 3000)
            return bone
        return m, stack(m, make, states)

    def sums(m):
        return [m.getter(index) for index in SPRING_LISTS + [20, 2]] + [m.weighted([(4, 1.0), (20, 1.0)])]

    # Spring: five ready, pressed by the first's key and by the fifth's: four fire, the getters
    # give exactly four active Springs' nodes; the fifth stays ready (state 3, no cooldown, no
    # activation time).
    m4, four = springs_match([3] * 4)
    assert press(m4, four, 0) == [4] * 4
    want = sums(m4)
    for index, value in zip(SPRING_LISTS + [20, 2], want):
        close(value, max(0.0, 1 + spring_sum(index, 4, 4)))
    for at in (0, 4):
        m, five = springs_match([3] * 5)
        assert press(m, five, at) == [4, 4, 4, 4, 3], ('Spring', at)
        assert (m.g.read(five[4] + 0x28C), m.g.read(five[4] + 0x2A0)) == (0, 0), "the fifth stays ready"
        assert sums(m) == want, ('five Springs fire as four', at, sums(m), want)
        outcomes.append(('Spring', at, [round(v, 4) for v in want]))
    # One of the four counting on its cooldown: three fire; the fifth does not fill in.
    m, five = springs_match([3] * 5)
    m.g.word(five[1] + 0x28C, 500)
    assert press(m, five, 0) == [4, 3, 4, 4, 3], "a cooling one of the four: three fire, not the fifth"

    # Onyx: four fired together are each active 3 s + 3 s; five fire as four (6 s, not 7 s), the
    # fifth stays ready with its time untouched.
    m4, four = onyx_match([3] * 4)
    assert press(m4, four, 0) == [4] * 4
    want = [m4.g.read(o + 0xF14) for o in four]
    assert want == [6000] * 4, want
    for at in (0, 4):
        m, five = onyx_match([3] * 5)
        before = m.g.read(five[4] + 0xF14)
        assert press(m, five, at) == [4, 4, 4, 4, 3], ('Onyx', at)
        have = [m.g.read(o + 0xF14) for o in five]
        assert have[:4] == want and have[4] == before, ('five Onyx fire as four', at, have)
        assert (m.g.read(five[4] + 0x28C), m.g.read(five[4] + 0x2A0)) == (0, 0), "the fifth stays ready"
        outcomes.append(('Onyx', at, have))
    m, five = onyx_match([3] * 5)
    m.g.word(five[2] + 0x28C, 500)
    assert press(m, five, 0) == [4, 4, 3, 4, 3], "a cooling Onyx of the four: three fire"
    assert [m.g.read(o + 0xF14) for o in (five[0], five[1], five[3])] == [5000] * 3, "three: 5 s each"

    # Lifebone: five ready, four fire (each its active: the timer, state 4); the fifth stays ready.
    for at in (0, 4):
        m, five = lifebone_match([3] * 5)
        assert press(m, five, at) == [4, 4, 4, 4, 3], ('Lifebone', at)
        assert (m.g.read(five[4] + 0x28C), m.g.read(five[4] + 0x2A0)) == (0, 0), "the fifth stays ready"
        assert all(m.g.read(b + 0x2A0) == 4000 for b in five[:4])
        outcomes.append(('Lifebone', at))

    # Cascade: three own Springs active and a carrier's two active within 5 m: one shared active
    # (four in all, own first); four own active: none shared.
    for own, shared in [(3, 1), (4, 0), (2, 2)]:
        m, mine = springs_match([3] * own)
        assert press(m, mine, 0) == [4] * own
        at = m.player(1, 0, near, CASCADE)
        for state in (4, 4):
            m.spring(at, state)
        have = sums(m)
        for index, value in zip(SPRING_LISTS + [20, 2], have):
            close(value, max(0.0, 1 + spring_sum(index, own, own + shared)))
        outcomes.append(('Cascade', own, shared))

    # A carrier shares only its copies that count for it: five Springs, the fifth active (loaded
    # so), share no active by Cascade; five Onyx, the first active, share three passives (the
    # second to the fourth) by Resonance.
    m = Match(data)
    at = m.player(1, 0, near, CASCADE)
    for state in (3, 3, 3, 3, 4):
        m.spring(at, state)
    for value in sums(m):
        close(value, 1.0)
    m = Match(data)
    at = m.player(1, 0, near, RESONANCE)
    stones = [onyx(m, at, state) for state in (4, 3, 3, 3, 3)]
    for damage, piercing in [(100.0, 0.4), (4.0, 0.9)]:
        have = protected(m, 7, 'body', damage, piercing)
        want = (damage, piercing)
        for _ in range(3):
            want = native_protect(m, stones[1], 3, 7, *want)
        close(have[0], want[0])
        close(have[1], want[1])
        outcomes.append(('carrier', damage, round(have[0], 4)))
    print("At most 4 copies of a kind count when activated: a press of five ready Springs, Onyx or "
          "Lifebones (the first's key or the fifth's) fires the first four, the getters and the Onyx "
          "durations exactly four's (6 s), the fifth not fired and ready; a cooling one of the four leaves "
          "three fired, the fifth not filling in; Cascade: three own active take one shared active, four "
          "none, two two; a carrier shares only its copies that count (its fifth active Spring none, its "
          "five Onyx with the first active three passives): PASS")
    return outcomes


def main():
    original = open(sys.argv[1], 'rb').read()
    patched, _ = base.build(original, 'section', open(base.default_bin('section'), 'rb').read(), None,
                            open(base.late_join_bin(), 'rb').read())
    assert check_resistance(patched, 0) == check_resistance(patched, 2)
    assert check_onyx(patched, 0) == check_onyx(patched, 2)
    assert check_larkspur(patched, 0) == check_larkspur(patched, 2)
    check_spring(patched)
    print('Artefact skills: identical outcomes on two kits: PASS')
    check_resonance_springs(patched)
    check_resonance_movement(patched)
    check_resonance_onyx(patched)
    check_resonance_lifebone(patched)
    check_own_cap(patched)
    check_active_cap(patched)


if __name__ == '__main__':
    main()
