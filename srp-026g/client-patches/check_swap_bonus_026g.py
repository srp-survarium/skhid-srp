#!/usr/bin/env python3
"""Run patch 41 (a battery's pick-up takes the weapon swap bonus) beside the
weapon draw's own code, in the original and the build.

    python check_swap_bonus_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, on
made-up objects: a player (kit byte +115h, profile +6B8h) whose profile has
three kits of 68 modifiers, each with a list of item nodes; a weapon show
state and a battery take state of that player. For each kit, modifier 22
(change_weapon_speed: 0, skill 104's +0.075 and +0.15, with and without an
item node, a negative one and one clamped at 0) and time scale:
- the weapon: weapon_core_show_state/_hide_state's 0x30DF10 runs from its
  entry to its call of the getter's result (0x30DF85): xmm0, the hands'
  time scale the draw and holster play at;
- the battery: the take expression runs from 0x338B3B to 0x338B61, the
  scale stored for 0x344F60, in the original and in the build;
the build's battery scale is the weapon's, bit for bit, the original's the
state's own. The hook keeps eax, ecx is free, esi, edi, ebx, ebp and esp
are kept, xmm0 is 0 again and the x87 stack has one value more, as after
the original's fld. A take state without a user keeps its scale. The
getter's lazy sort of a kit's list (its first read) runs too. The build
without the late join keeps the original's six bytes.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX,
                               UC_X86_REG_EDI, UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI,
                               UC_X86_REG_ESP, UC_X86_REG_FPSW, UC_X86_REG_XMM0)

import patch_026g as base

STACK, WORK, RETURN = 0x20000000, 0x20100000, 0x20200000  # above the image
PLAYER, PROFILE, CORE, WEAPON_CORE = WORK, WORK + 0x1000, WORK + 0x6000, WORK + 0x6100
TAKE, SHOW, NODES = WORK + 0x7000, WORK + 0x7400, WORK + 0x8000
HOOK, AFTER_FLD, STORED = 0x338B3B, 0x338B41, 0x338B61   # the take expression
WEAPON_M, WEAPON_DONE = 0x30DF10, 0x30DF85               # the show/hide expression's helper
INDEX, KIT_SIZE, BLOCK, LISTS = 22, 0x704, 0xF4, 0x110


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def image(data):
    emu = Uc(UC_ARCH_X86, UC_MODE_32)
    for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
        start = base.IMAGE_BASE + rva
        emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
        emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
    emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
    emu.mem_map(WORK, 0x10000, UC_PROT_ALL)
    emu.mem_map(RETURN, 0x1000, UC_PROT_ALL)
    return emu


def world(emu, kit, kit_value, nodes, sorted_list=True, user=True):
    """The player of `kit`, kit modifier 22 = kit_value, the item nodes'
    values on its list (in order), the take state and the weapon's."""
    emu.mem_write(WORK, bytes(0x10000))
    emu.mem_write(PLAYER + 0x115, bytes([kit]))
    emu.mem_write(PLAYER + 0x6B8, struct.pack("<I", PROFILE))
    for k in range(3):
        block = PROFILE + k * KIT_SIZE + BLOCK
        # Every other kit has another value: only the player's kit counts.
        emu.mem_write(block + INDEX * 4, struct.pack("<f", kit_value if k == kit else 0.5 + k))
        emu.mem_write(block + LISTS + INDEX * 20 + 0x10, bytes([1]))
    block = PROFILE + kit * KIT_SIZE + BLOCK
    head = block + LISTS + INDEX * 20
    previous = 0
    for n, value in enumerate(nodes):
        node = NODES + n * 16
        emu.mem_write(node, struct.pack("<IIIf", previous, 0, INDEX, value))
        if previous:
            emu.mem_write(previous + 4, struct.pack("<I", node))
        previous = node
    emu.mem_write(head, struct.pack("<III", len(nodes), NODES if nodes else 0, previous))
    emu.mem_write(head + 0x10, bytes([1 if sorted_list else 0]))
    emu.mem_write(CORE + 8, struct.pack("<I", PLAYER if user else 0))
    emu.mem_write(TAKE + 0x1C, struct.pack("<I", CORE))
    emu.mem_write(TAKE + 0xC8, struct.pack("<I", TAKE + 0xD0))
    emu.mem_write(WEAPON_CORE + 8, struct.pack("<I", PLAYER))
    emu.mem_write(SHOW + 0x120, struct.pack("<I", WEAPON_CORE))
    emu.mem_write(SHOW + 0x130, struct.pack("<I", SHOW + 0x13C))


def weapon(emu, scale):
    """The draw's time scale: 0x30DF10 (edi = the show state) to the
    getter's return."""
    emu.mem_write(SHOW + 0x150, struct.pack("<f", scale))
    esp = STACK + 0x8000
    emu.mem_write(esp, struct.pack("<4I", RETURN, WORK + 0xF000, 1, 0))
    emu.reg_write(UC_X86_REG_ESP, esp)
    emu.reg_write(UC_X86_REG_EDI, SHOW)
    emu.emu_start(WEAPON_M, WEAPON_DONE, count=100000)
    assert emu.reg_read(UC_X86_REG_EIP) == WEAPON_DONE
    return emu.reg_read(UC_X86_REG_XMM0) & 0xFFFFFFFF


def battery(emu, scale, eax=1):
    """The take expression from the hook's site: the registers after the
    six bytes, then the scale it stores for 0x344F60 (as bits)."""
    emu.mem_write(TAKE + 0xE8, struct.pack("<f", scale))
    esp = STACK + 0x8000
    registers = {UC_X86_REG_EAX: eax, UC_X86_REG_ECX: TAKE, UC_X86_REG_ESI: TAKE, UC_X86_REG_EDI: 0x13579BDF,
                 UC_X86_REG_EBX: 0x2468ACE0, UC_X86_REG_EBP: STACK + 0x9000, UC_X86_REG_ESP: esp,
                 UC_X86_REG_EDX: 0x55AA55AA}
    for register, value in registers.items():
        emu.reg_write(register, value)
    emu.reg_write(UC_X86_REG_XMM0, 0)
    top = (emu.reg_read(UC_X86_REG_FPSW) >> 11) & 7
    emu.emu_start(HOOK, AFTER_FLD, count=100000)
    assert emu.reg_read(UC_X86_REG_EIP) == AFTER_FLD
    for register in (UC_X86_REG_EAX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBX, UC_X86_REG_EBP,
                     UC_X86_REG_ESP):
        assert emu.reg_read(register) == registers[register], register
    assert emu.reg_read(UC_X86_REG_XMM0) == 0
    assert (emu.reg_read(UC_X86_REG_FPSW) >> 11) & 7 == (top - 1) & 7
    emu.emu_start(AFTER_FLD, STORED, count=100)
    assert emu.reg_read(UC_X86_REG_EIP) == STORED
    assert emu.reg_read(UC_X86_REG_ESP) == esp - 8
    return struct.unpack("<I", emu.mem_read(esp - 8, 4))[0]


def bits(value):
    return struct.unpack("<I", struct.pack("<f", value))[0]


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    patched, _ = base.build(original, "section", code, None, late_join)
    without, _ = base.build(original, "section", code)
    checks = 0

    for va, old, offset, _ in base.SWAP_BONUS_HOOKS:
        at = base.file_offset(va)
        site = patched[at:at + len(old)]
        assert original[at:at + len(old)] == without[at:at + len(old)] == old, hex(va)
        assert site[0] == 0xE8 and site[5:] == b"\x90" and len(site) == len(old), hex(va)
        assert va + 5 + struct.unpack_from("<i", site, 1)[0] == base.LATE_JOIN_VA + offset, hex(va)
        checks += 1

    stock, new = image(original), image(patched)
    # Retail: 0 or skill 104 at one and two levels; an item node; a node
    # alone; below 0; clamped at 0 (max(0, 1 + S)).
    cases = [(0.0, []), (0.075, []), (0.15, []), (0.15, [0.05]), (0.0, [0.075, 0.075]), (-0.3, []),
             (-1.5, [])]
    # Take scales: (end - start) / 30 / take_time of a few lengths.
    scales = [f32(x) for x in (1.0, 16 / 30 / 0.5, 16 / 30 / 0.6, 0.37, 2.5)]
    rows = []
    for kit in range(3):
        for kit_value, nodes in cases:
            for scale in scales:
                world(new, kit, kit_value, nodes)
                drawn = weapon(new, scale)
                world(new, kit, kit_value, nodes)
                taken = battery(new, scale)
                world(stock, kit, kit_value, nodes)
                assert weapon(stock, scale) == drawn, (kit, kit_value, nodes, scale)
                world(stock, kit, kit_value, nodes)
                stock_taken = battery(stock, scale)
                s = f32(kit_value) + sum(f32(v) for v in nodes)
                expected = scale * max(0.0, 1 + s)
                assert taken == drawn, (kit, kit_value, nodes, scale, hex(taken), hex(drawn))
                assert abs(struct.unpack("<f", struct.pack("<I", taken))[0] - expected) <= 1e-6 * max(1, expected)
                assert stock_taken == bits(scale), (kit, kit_value, nodes, scale)
                checks += 1
            if kit == 0 and not nodes:
                rows.append(f"{kit_value:+g}: x{max(0.0, 1 + kit_value):g}")
    # eax of the expression (`sete al`) either way.
    world(new, 1, 0.15, [])
    drawn = weapon(new, scales[1])
    for eax in (0, 1):
        world(new, 1, 0.15, [])
        assert battery(new, scales[1], eax) == drawn
        checks += 1
    # The getter's first read of a list sorts it (0x2D59C0): both ways alike.
    for nodes in ([], [0.05], [0.05, -0.02]):
        world(new, 2, 0.075, nodes, sorted_list=False)
        drawn = weapon(new, scales[1])
        world(new, 2, 0.075, nodes, sorted_list=False)
        assert battery(new, scales[1]) == drawn, nodes
        checks += 1
    # A take state whose item has no user keeps its scale.
    for scale in scales:
        world(new, 0, 0.15, [], user=False)
        assert battery(new, scale) == bits(scale)
        world(stock, 0, 0.15, [], user=False)
        assert battery(stock, scale) == bits(scale)
        checks += 1
    print(f"patch 41: {checks} checks; the battery's take scale is the weapon draw's, bit for bit "
          f"(kit modifier 22 " + ", ".join(rows) + "), the original's its own")


if __name__ == "__main__":
    main()
