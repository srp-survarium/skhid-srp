#!/usr/bin/env python3
"""Run replay_network_client's "is this place mine" (its vtable 0x9FF344,
slot +54h) of the original and of the build on made-up clients: the
replay's camera follows its player into another kit (the replay viewer,
client_patch.md section 3.24).

    python check_replay_reattach_026g.py <original survarium.exe>

Requires Python `unicorn` (2.1.4). The slot is called as the client respawn
rule's kit change (0x2678E0) calls it: thiscall, ECX the replay client, the
place pushed, `ret 4`. The client's +4 is the player the camera is on
(none: 0), its place byte at +114h. The original's slot is the stub 0x54E10
(`xor al,al`): never mine. The build's says yes exactly for the camera's
place (is_player_current 0x242A40, the client's own); EBX, ESI, EDI, EBP and
ESP are kept, `ret 4` pops the place.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EIP, UC_X86_REG_ESP, \
    UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP

import patch_026g as base

SLOT = 0x9FF344 + 0x54
CLIENT, PLAYER, STACK, STOP = 0x20000000, 0x20001000, 0x21008000, 0x22000000
KEPT = (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)


def mine(data, player_place, place):
    emu = Uc(UC_ARCH_X86, UC_MODE_32)
    for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
        start = base.IMAGE_BASE + rva
        emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
        emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
    for at in (CLIENT, 0x21000000, STOP):
        emu.mem_map(at, 0x10000, UC_PROT_ALL)
    emu.mem_write(CLIENT + 4, struct.pack("<I", 0 if player_place is None else PLAYER))
    if player_place is not None:
        emu.mem_write(PLAYER + 0x114, bytes([player_place]))
    emu.mem_write(STACK, struct.pack("<II", STOP, place))
    emu.reg_write(UC_X86_REG_ESP, STACK)
    emu.reg_write(UC_X86_REG_ECX, CLIENT)
    emu.reg_write(UC_X86_REG_EAX, 0xA5A5A5A5)
    for register in KEPT:
        emu.reg_write(register, 0x12345678)
    target = struct.unpack("<I", emu.mem_read(SLOT, 4))[0]
    emu.emu_start(target, STOP, count=100000)
    assert emu.reg_read(UC_X86_REG_EIP) == STOP and emu.reg_read(UC_X86_REG_ESP) == STACK + 8, "`ret 4`"
    assert all(emu.reg_read(register) == 0x12345678 for register in KEPT), "the callee-saved registers"
    return emu.reg_read(UC_X86_REG_EAX) & 0xFF


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    cases = 0
    for player_place, place in ((3, 3), (3, 4), (0, 0), (None, 0), (None, 7), (0xFF, 0xFF), (5, 0x105), (5, 0x205)):
        assert mine(original, player_place, place) == 0, (player_place, place)
        expected = int(player_place is not None and player_place == place & 0xFF)
        assert mine(built, player_place, place) == expected, (player_place, place)
        cases += 1
    print(f"the replay's camera place: {cases} clients as expected (the original's stub says no to all; the build "
          "says yes for the camera player's place alone, the low byte compared)")


if __name__ == "__main__":
    main()
