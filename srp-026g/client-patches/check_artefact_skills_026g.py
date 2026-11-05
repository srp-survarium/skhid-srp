#!/usr/bin/env python3
"""Patch 109 (the artefact tree, docs/notes/artefact-skills.md) in Unicorn on the built exe.

Resistance: body_part_parameters::hit_by_type's armour slice (0x32A083 .. 0x32A0B0, the
game's own clamp and multiply) for every hit type, Resistance 0..5 and refused payloads, on
two kits. Onyx II: the landing (0x2D8220) with and without the skill, an Onyx active, passive,
none, or another artefact active. Larkspur II: the real detoured action entry (0x313300) of a
Larkspur, a press, a repeated press and a key-up, and another kind's activation. Spring II: the
game's Spring loader and its passive and active links and unlinks (0x311AD0, 0x311BC0, 0x311DA0,
0x311D10) through patch 54, the reload modifier (13) by the game's getter, one and two Springs,
the skill taken away between two links. A constructed player, not a played match.
Usage: python check_artefact_skills_026g.py ORIGINAL_EXE
"""
import struct
import sys
from unicorn.x86_const import *  # noqa: F403
import patch_026g as base
from unicorn import UC_HOOK_CODE
from check_artefacts_026g import PLAYER, STACK, STOP, HOLDER_VT, DM, ENABLE_ACTIVE, KITS, SPRING, Game, \
    PASSIVE_LINK, PASSIVE_UNLINK, ACTIVE_LINK, ACTIVE_UNLINK
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
    for payload, rank in [(0, 0), (1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (5 + 8 + 256, 5),
                          (6, 0), (7, 0), (2.5, 0), (-1, 0), (4096, 0), (512 + 1, 0), (8 + 7 * 512 + 5, 0), (8 + 6 * 512 + 5, 5)]:
        g = MedicalGame(data)
        g.emu.mem_write(PLAYER + 0x115, bytes([kit]))
        g.ranks(payload, RANKS)
        for name in ['body', 'head', 'left_leg']:
            pool = g.pools[PLAYER][name]
            for kind in range(8):
                for armour, piercing in [(0, 0), (0.3, 0), (0.3, 0.1), (0.95, 0), (1.0, 0), (0.1, 0.5)]:
                    have = hit(g, kind, armour, piercing, 100.0, pool)
                    a = f32(f32(armour + 0.05 * rank) if kind >= 4 and rank else armour)
                    want = 100.0 * (1 - min(max(f32(a - piercing), 0), 1))
                    close(have, want)
                    outcomes.append((payload, name, kind, armour, piercing, round(have, 4)))
    print(f'Resistance kit {kit}: anomalous hits (4..7) A + 0.05 a rank 0..5 before piercing and the clamp, '
          'other types and refused payloads (6, 7, 2.5, -1, 4096, a draw without Keeper or of 7) unchanged: PASS')
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
    for payload in [0, ONYX_II, ONYX_II + 5 + 32 + 128, LARKSPUR_II, ONYX_II + 0.5]:
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
            protected = payload in (ONYX_II, ONYX_II + 5 + 32 + 128) and (ONYX_ACTIVE, 4) in carried
            assert bool(reached) != protected, (payload, carried)
            outcomes.append((payload, len(carried), protected))
    print(f"Onyx II kit {kit}: the landing skipped only with the skill and an active Onyx (passive Onyx, "
          "another kind active, no skill, a refused payload: the game's landing): PASS")
    return outcomes


def check_larkspur(data, kit):
    outcomes = []
    for payload, kind in [(0, LIFEBONE_ACTIVE), (LARKSPUR_II, LIFEBONE_ACTIVE), (LARKSPUR_II + 5, LIFEBONE_ACTIVE),
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
        filled = payload in (LARKSPUR_II, LARKSPUR_II + 5) and kind == LIFEBONE_ACTIVE
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
    for payload, skill in [(0, False), (SPRING_II, True), (SPRING_II + 5 + 8, True), (SPRING_II + 0.5, False)]:
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


def main():
    original = open(sys.argv[1], 'rb').read()
    patched, _ = base.build(original, 'section', open(base.default_bin('section'), 'rb').read(), None,
                            open(base.late_join_bin(), 'rb').read())
    assert check_resistance(patched, 0) == check_resistance(patched, 2)
    assert check_onyx(patched, 0) == check_onyx(patched, 2)
    assert check_larkspur(patched, 0) == check_larkspur(patched, 2)
    check_spring(patched)
    print('Artefact skills: identical outcomes on two kits: PASS')


if __name__ == '__main__':
    main()
