#!/usr/bin/env python3
"""Patch 117 (controller_position): a character controller's step never
leaves its position non-finite (../docs/notes/client_patch.md section 3.71).

    python check_controller_position_026g.py <original survarium.exe>

bullet_character_controller::player_step's epilogue (0x7C7D7: pop edi / pop
esi / pop ebx / mov esp,ebp / pop ebp / ret 4) run in the original exe and
in the patched one, from the site to the return, on a made-up controller
(EDI; +0B0h the position the step left, +50h m_last_pos, where it began) and
a made-up player_step frame:

- a finite position (the live bot's last finite one, zeros, signed zeros,
  the largest floats, denormals, random ones): the patched exe leaves the
  controller, the stack and every register as the original does, byte for
  byte;
- a NaN or an infinity in any of the three floats (x87's indefinite
  0FFC00000h, the 7FC00000h of the live world, a signalling NaN, +-inf):
  the patched exe puts the position back to m_last_pos's bits, and only
  those 12 bytes differ from the original's run; the original keeps the NaN
  (it fails the case);
- a non-finite position whose m_last_pos is not finite either: left as the
  original leaves it.
"""
import random
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

SITE = 0x7C7D7
THREAD_SLOT = 0x870118                    # GetCurrentThreadId's import slot
CONTROLLER_SIZE = 0x2C790
POSITION, LAST_POS = 0xB0, 0x50

DATA = 0x30000000
CONTROLLER = DATA
THREAD = DATA + 0x30000
RETURN = DATA + 0x31000                   # player_step's caller: where the run stops
DATA_SIZE = 0x40000
STACK = 0x21000000
SP = STACK + 0x8000                       # the site's ESP: the saved edi, esi, ebx
FRAME = SP + 0x80                         # player_step's EBP: saved ebp, return, dt
SAVED = {"edi": 0x5EDE5EDE, "esi": 0x5E515E51, "ebx": 0x5EB05EB0, "ebp": 0x0019FF00}
GAME = {UC_X86_REG_EAX: 0x0A0A0A0A, UC_X86_REG_ECX: 0x0C0C0C0C, UC_X86_REG_EDX: 0x0D0D0D0D,
        UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_ESI: 0x05050505}
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP, UC_X86_REG_ESP)

# The live bot's last finite position, Bullet's frame (match 03439962, 539 200 ms).
LIVE = (0xC14B4B6C, 0x41AD8513, 0x41FBA450)
NON_FINITE = (0xFFC00000, 0x7FC00000, 0x7F800001, 0xFFBFFFFF, 0x7F800000, 0xFF800000)


def finite(bits):
    return all(b & 0x7F800000 != 0x7F800000 for b in bits)


class Epilogue:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(DATA, DATA_SIZE, UC_PROT_ALL)
        emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
        emu.hook_add(UC_HOOK_CODE, self.stop)

    def stop(self, emu, address, size, _):
        if address == RETURN:
            self.stopped = True
            emu.emu_stop()

    def run(self, position, last_pos, fill):
        """The epilogue on a controller of `fill`'s bytes with these
        positions: the controller's bytes, the stack's and the registers."""
        emu = self.emu
        emu.mem_write(DATA, bytes(DATA_SIZE))
        emu.mem_write(CONTROLLER, fill)
        emu.mem_write(CONTROLLER + POSITION, struct.pack("<3I", *position))
        emu.mem_write(CONTROLLER + LAST_POS, struct.pack("<3I", *last_pos))
        emu.mem_write(THREAD, bytes.fromhex("b801000000c3"))     # GetCurrentThreadId: mov eax,1 / ret
        emu.mem_write(THREAD_SLOT, struct.pack("<I", THREAD))
        emu.mem_write(STACK, bytes(0x10000))
        emu.mem_write(SP, struct.pack("<3I", SAVED["edi"], SAVED["esi"], SAVED["ebx"]))
        emu.mem_write(FRAME, struct.pack("<3I", SAVED["ebp"], RETURN, 0x3D4CCCCD))  # dt 0.05
        for reg, value in GAME.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_EDI, CONTROLLER)
        emu.reg_write(UC_X86_REG_EBP, FRAME)
        emu.reg_write(UC_X86_REG_ESP, SP)
        self.stopped = False
        emu.emu_start(SITE, 0, count=200000)
        assert self.stopped, hex(emu.reg_read(UC_X86_REG_EIP))
        regs = {reg: emu.reg_read(reg) for reg in REGS}
        assert regs[UC_X86_REG_EDI] == SAVED["edi"] and regs[UC_X86_REG_ESI] == SAVED["esi"]
        assert regs[UC_X86_REG_EBX] == SAVED["ebx"] and regs[UC_X86_REG_EBP] == SAVED["ebp"]
        assert regs[UC_X86_REG_ESP] == FRAME + 12, "ret 4"
        controller = bytes(emu.mem_read(CONTROLLER, CONTROLLER_SIZE))
        stack = bytes(emu.mem_read(FRAME, 12))
        return controller, stack, regs


def position_of(controller):
    return struct.unpack_from("<3I", controller, POSITION)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    stock, patched = Epilogue(original), Epilogue(bytes(built))
    rng = random.Random(117)

    def both(position, last_pos):
        fill = bytes(rng.getrandbits(8) for _ in range(CONTROLLER_SIZE))
        return stock.run(position, last_pos, fill), patched.run(position, last_pos, fill)

    print("player_step's epilogue (0x7C7D7), original and patched exe")
    finite_cases = [LIVE, (0, 0, 0), (0x80000000, 0x80000000, 0), (0x7F7FFFFF, 0xFF7FFFFF, 1),
                    (0x00000001, 0x807FFFFF, 0x3F800000)]
    while len(finite_cases) < 200:
        bits = tuple(rng.getrandbits(32) for _ in range(3))
        if finite(bits):
            finite_cases.append(bits)
    for position in finite_cases:
        last_pos = tuple(rng.getrandbits(32) for _ in range(3))
        a, b = both(position, last_pos)
        assert a == b, ("a finite step differs", [hex(x) for x in position])
        assert position_of(b[0]) == position
    print(f"  ok {len(finite_cases)} finite positions (the live bot's, zeros, extremes, denormals, random):"
          " the controller, stack and registers byte for byte the original's")

    restored = 0
    for bad in NON_FINITE:
        for at in range(3):
            for base_position in (LIVE, (0, 0, 0)):
                position = list(base_position)
                position[at] = bad
                position = tuple(position)
                last_pos = LIVE
                a, b = both(position, last_pos)
                assert position_of(a[0]) == position, "the original keeps the non-finite position"
                assert not finite(position_of(a[0]))
                assert position_of(b[0]) == last_pos, ([hex(x) for x in position], [hex(x) for x in position_of(b[0])])
                assert a[0][:POSITION] == b[0][:POSITION] and a[0][POSITION + 12:] == b[0][POSITION + 12:], \
                    "only the position's 12 bytes differ"
                assert a[1:] == b[1:], "the stack and registers as the original's"
                restored += 1
        position = (bad, bad, bad)
        a, b = both(position, LIVE)
        assert position_of(b[0]) == LIVE and position_of(a[0]) == position
        restored += 1
    print(f"  ok {restored} positions with a NaN or an infinity (0FFC00000h, 7FC00000h, signalling, +-inf) in"
          " any float: back to m_last_pos, nothing else changed; the original keeps them (fails)")

    for bad in NON_FINITE:
        position = (LIVE[0], bad, LIVE[2])
        for last_bad in (0xFFC00000, 0x7F800000):
            a, b = both(position, (last_bad, 0, 0))
            assert a == b and position_of(b[0]) == position
    print("  ok a non-finite position with a non-finite m_last_pos: left as the original leaves it")


if __name__ == "__main__":
    main()
