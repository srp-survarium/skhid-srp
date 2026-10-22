#!/usr/bin/env python3
"""Run the original's and patches 50 and 51's code with Unicorn (2.1.4):
what a relayed input's weapon bits and a relayed kit choice do.

    python check_relay_guards_026g.py <original survarium.exe>

The exe is built as patch_026g.py builds it (with the late join), without
the late join and over on_hash_mismatch; the original is run the same way.

Patch 50, the player's mask (0x2D8BB0) from 0x2D8CD7 to the slot's test
(0x2D8D28, ebp the slot) or the way of no slot (0x2D8D7D): every `which`
(0, 1 for the item in hands in slot 7 or 10, 0FFh for another item), an
item in hands or none, bits 25 and 26 in all four ways and the slot bits 12
and 13 left in ebp (17h, 7, 10). The original and the build agree on all
of them but one: bit 26 without bit 25 from a hand whose item is in neither
weapon slot, where the original goes on with slot 47 of 23 (the dword at
0x9281E8, read past the 188h-byte inventory) and the build takes no slot,
as the original does when that dword is 0.

Patch 51, the place's tick 0x321790 whole, from its entry to its `ret 0Ch`
on a made-up place (+28h its core, +2Ch its index into the core's kit
choice lists, +0Dh the next kit, +0..+8 the players of kits 0 to 2): a
choice of 0, 1, 2, 3 and 0FFh, with no player for the kit, a player with
nothing in slot 7 or 10 (inventory +128h, +134h), slot 7 alone, slot 10
alone, both, and a player with no inventory. The original takes every
choice under 3 that has a player; the build only one whose player has
something in slot 7 or 10, else the old kit stays, as for a kit with no
player. The stack is as the function leaves it in every case.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_ECX, UC_X86_REG_EIP,
                               UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

MASK_START, MASK_SLOT, MASK_NONE = 0x2D8CD7, 0x2D8D28, 0x2D8D7D
PICK = 0x321790
BIT_25, BIT_26 = 0x2000000, 0x4000000
NO_SLOT = 0x17
STACK, HEAP, RETURN = 0x41108000, 0x42000000, 0x41200000
PLACE, CORE, LIST, NODE = HEAP, HEAP + 0x100, HEAP + 0x10000, HEAP + 0x200
PLAYERS = (HEAP + 0x1000, HEAP + 0x2000, HEAP + 0x3000)
INVENTORY = 0x10C                             # the player's
SLOT_7, SLOT_10 = 0x128, 0x134                # in the inventory
CHOICE_TIME, TICK_TIME = 100, 200


def emulator(data):
    emu = Uc(UC_ARCH_X86, UC_MODE_32)
    headers = base.Headers(data)
    for _, rva, vsize, raw, rsize, _ in headers.sections(data):
        start = base.IMAGE_BASE + rva
        emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
        emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
    for at, size in ((STACK - 0x8000, 0x10000), (HEAP, 0x20000), (RETURN, 0x1000)):
        emu.mem_map(at, size, UC_PROT_ALL)
    return emu


def word(emu, at, value):
    emu.mem_write(at, struct.pack("<I", value))


def mask(data, which, item, actions, slot):
    """Where the mask goes and with what: ("slot", n) at 0x2D8D28 or ("none",)."""
    emu = emulator(data)
    stops = []

    def stop(emu, address, size, _):
        if address in (MASK_SLOT, MASK_NONE):
            stops.append(("slot", emu.reg_read(UC_X86_REG_EBP)) if address == MASK_SLOT else ("none",))
            emu.emu_stop()

    emu.hook_add(UC_HOOK_CODE, stop)
    emu.reg_write(UC_X86_REG_ESP, STACK)
    emu.mem_write(STACK + 0x17, bytes([which]))
    word(emu, STACK + 0x18, HEAP if item else 0)
    emu.reg_write(UC_X86_REG_ECX, actions)
    emu.reg_write(UC_X86_REG_EBP, slot)
    emu.reg_write(UC_X86_REG_ESI, HEAP + 0x8000)
    emu.emu_start(MASK_START, 0, count=100)
    assert len(stops) == 1, f"stopped at {emu.reg_read(UC_X86_REG_EIP):#x}"
    assert emu.reg_read(UC_X86_REG_ESP) == STACK
    return stops[0]


def pick(data, choice, player, old=1):
    """The place's next kit after the tick. player: None (no player for the
    kit), "no inventory", or the set of its filled slots."""
    emu = emulator(data)
    word(emu, PLACE + 0x28, CORE)
    emu.mem_write(PLACE + 0x2C, b"\0")                  # its lists: the first
    emu.mem_write(PLACE + 0xD, bytes([old]))
    word(emu, CORE + 0x89D8, LIST)
    word(emu, LIST + 8, NODE)                           # the newest choice
    word(emu, NODE + 4, 0)
    emu.mem_write(NODE + 8, bytes([choice]))
    word(emu, NODE + 0x20, CHOICE_TIME)
    for kit, at in enumerate(PLAYERS):
        filled = player if kit == choice % 3 else {7, 10}
        if kit == choice % 3 and player is None:
            word(emu, PLACE + 4 * kit, 0)
            continue
        word(emu, PLACE + 4 * kit, at)
        if filled == "no inventory":
            word(emu, at + INVENTORY, 0)
            continue
        word(emu, at + INVENTORY, at + 0x800)
        word(emu, at + 0x800 + SLOT_7, HEAP + 0x9000 if 7 in filled else 0)
        word(emu, at + 0x800 + SLOT_10, HEAP + 0x9100 if 10 in filled else 0)
    sp = STACK - 0x10
    for i, value in enumerate((RETURN, 0, TICK_TIME, 0)):
        word(emu, sp + 4 * i, value)
    emu.reg_write(UC_X86_REG_ESP, sp)
    emu.reg_write(UC_X86_REG_ECX, PLACE)
    emu.reg_write(UC_X86_REG_EAX, 0)
    emu.emu_start(PICK, RETURN, count=200)
    assert emu.reg_read(UC_X86_REG_EIP) == RETURN, f"stopped at {emu.reg_read(UC_X86_REG_EIP):#x}"
    assert emu.reg_read(UC_X86_REG_ESP) == sp + 4 + 0xC, "not `ret 0Ch`"
    return emu.mem_read(PLACE + 0xD, 1)[0]


MASK_CASES = [(which, item, actions, slot)
              for which in (0, 1, 0xFF) for item in (True, False)
              for actions in (0, BIT_25, BIT_26, BIT_25 | BIT_26) for slot in (NO_SLOT, 7, 10)]
PLAYER_CASES = (None, frozenset(), frozenset({7}), frozenset({10}), frozenset({7, 10}), "no inventory")
PICK_CASES = [(choice, player) for choice in (0, 1, 2, 3, 0xFF) for player in PLAYER_CASES]


def run(data):
    return ({case: mask(data, *case) for case in MASK_CASES},
            {case: pick(data, *case) for case in PICK_CASES})


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    patched, _ = base.build(original, "section", code, None, late_join)
    before, _ = base.build(original, "section", code, None, None)
    mismatch, _ = base.build(original, "hash-mismatch", open(base.default_bin("hash-mismatch"), "rb").read())
    stock, built = run(original), run(patched)
    assert run(before) == built and run(mismatch) == built, "the other builds do not do as this one"
    stock_mask, stock_pick = stock
    built_mask, built_pick = built

    # Patch 50.
    stray = struct.unpack("<I", emulator(original).mem_read(0x9281E8, 4))[0]
    assert stray == 0x2F, hex(stray)
    for case in MASK_CASES:
        which, item, actions, slot = case
        if which == 0xFF and item and actions & BIT_26 and not actions & BIT_25:
            assert stock_mask[case] == ("slot", 0x2F), (case, stock_mask[case])
            assert built_mask[case] == ("none",), (case, built_mask[case])
        else:
            assert stock_mask[case] == built_mask[case], (case, stock_mask[case], built_mask[case])
    # What the two bits do from each slot, both builds.
    for which, want25, want26 in ((0, 10, 10), (1, 7, 7), (0xFF, 7, None)):
        for actions, want in ((BIT_25, want25), (BIT_26, want26)):
            got = built_mask[(which, True, actions, NO_SLOT)]
            assert got == (("slot", want) if want else ("none",)), (which, actions, got)
    assert all(built_mask[(w, False, a, s)] == (("slot", s) if s != NO_SLOT else ("none",))
               for w in (0, 1, 0xFF) for a in (BIT_25, BIT_26) for s in (NO_SLOT, 7, 10)), \
        "with no item in hands the two bits do nothing"

    # Patch 51.
    old = 1
    for case in PICK_CASES:
        choice, player = case
        has_player = choice < 3 and player is not None
        armed = has_player and player != "no inventory" and bool(player)
        assert stock_pick[case] == (choice if has_player else old), (case, stock_pick[case])
        assert built_pick[case] == (choice if armed else old), (case, built_pick[case])

    print("patch 50: bits 25 and 26 take the slots the original takes, but bit 26 from a hand whose item is in "
          "neither weapon slot, where the original goes on with slot 47 (0x9281E8 = 2Fh, past the inventory) "
          "and the build takes no slot; patch 51: the place's tick takes a kit choice under 3 only when the "
          "kit's player has something in slot 7 or 10 (the original: any player), else the old kit stays; "
          "every full build the same; the stack as the original leaves it")


if __name__ == "__main__":
    main()
