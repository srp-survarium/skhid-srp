#!/usr/bin/env python3
"""Run the original's and the build's delayed_callbacks_queue::add after its
drain (0x2454C1) on made-up queues: a weapon's full sound queue drops the
new sound (the replay viewer, client_patch.md section 3.24).

    python check_sound_queue_026g.py <original survarium.exe>

Requires Python `unicorn` (2.1.4). From 0x2454C1 (ESI the queue: begin,
end, capacity end) the run stops where add goes on: 0x2454C6 with EAX the
begin and ECX the end (the original's two loads), or 0x24569A, the game's
own exit for a sound past its chain's cap (the functor called with flag 0:
not played; the queue untouched). The original goes on for every queue
(and push_back asserts at the capacity, 0x245B32); the build goes on below
the capacity and drops the sound at it; EBX, ESI, EDI, EBP and ESP are kept.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EIP, UC_X86_REG_ESP, \
    UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP

import patch_026g as base

SITE, ON, DROP = 0x2454C1, 0x2454C6, 0x24569A
QUEUE, STACK = 0x20000000, 0x21008000
KEPT = (UC_X86_REG_EBX, UC_X86_REG_EDI, UC_X86_REG_EBP)


def run(data, begin, end, capacity):
    emu = Uc(UC_ARCH_X86, UC_MODE_32)
    for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
        start = base.IMAGE_BASE + rva
        emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
        emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
    emu.mem_map(QUEUE, 0x1000, UC_PROT_ALL)
    emu.mem_map(0x21000000, 0x10000, UC_PROT_ALL)
    emu.mem_write(QUEUE, struct.pack("<3I", begin, end, capacity))
    emu.reg_write(UC_X86_REG_ESP, STACK)
    emu.reg_write(UC_X86_REG_ESI, QUEUE)
    for register in KEPT:
        emu.reg_write(register, 0x12345678)
    stops = []
    for stop in (ON, DROP):
        emu.hook_add(1 << 2, lambda uc, address, size, user: (stops.append(address), uc.emu_stop()),
                     begin=stop, end=stop)
    emu.emu_start(SITE, 0, count=100000)
    assert emu.reg_read(UC_X86_REG_ESP) == STACK and emu.reg_read(UC_X86_REG_ESI) == QUEUE
    assert all(emu.reg_read(register) == 0x12345678 for register in KEPT)
    assert struct.unpack("<3I", emu.mem_read(QUEUE, 12)) == (begin, end, capacity), "the queue untouched"
    where = stops[0]
    if where == ON:
        assert (emu.reg_read(UC_X86_REG_EAX), emu.reg_read(UC_X86_REG_ECX)) == (begin, end), "begin and end loaded"
    return where


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    cases = 0
    item = 0x38
    for count in range(0, 65):
        begin = 0x30000000
        end, capacity = begin + item * count, begin + item * 64
        assert run(original, begin, end, capacity) == ON
        assert run(built, begin, end, capacity) == (DROP if count == 64 else ON), count
        cases += 1
    for end, capacity in ((0x30000000 + 0x38 * 65, 0x30000000 + 0x38 * 64), (0xFFFFFFF0, 0x10), (0x10, 0xFFFFFFF0)):
        assert run(built, 0x30000000, end, capacity) == (DROP if end >= capacity else ON)
        cases += 1
    print(f"the sound queue: {cases} queues as expected (the original goes on with every one; the build drops "
          "the new sound at the capacity and past it, unsigned)")


if __name__ == "__main__":
    main()
