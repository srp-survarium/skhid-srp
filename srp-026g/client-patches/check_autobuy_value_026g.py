#!/usr/bin/env python3
"""Exercise the native autobuy callback with Boolean and numeric GFx values.

Usage: python check_autobuy_value_026g.py <0.26g exe>
The game callback executes unchanged; only the network-client getter is
stubbed. Stop at the request writer and inspect the byte it will send.
"""
import struct
import sys
from pathlib import Path

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32
from unicorn.x86_const import UC_X86_REG_ECX, UC_X86_REG_EIP, UC_X86_REG_ESP
import patch_026g as base


def check(image):
    emu = Uc(UC_ARCH_X86, UC_MODE_32)
    for _, rva, vsize, raw, rsize, _ in base.Headers(image).sections(image):
        start = base.IMAGE_BASE + rva
        emu.mem_map(start, (max(vsize, rsize) + 4095) & ~4095)
        emu.mem_write(start, bytes(image[raw:raw + min(rsize, vsize or rsize)]))
    heap = 0x20000000
    emu.mem_map(heap, 0x20000)
    menu, game, holder, vtable, getter, params, value, stack = (
        heap + offset for offset in (0, 0x1000, 0x7000, 0x8000, 0x9000, 0xa000, 0xb000, 0x18000))

    def word(address, number):
        emu.mem_write(address, struct.pack('<I', number))

    word(menu + 0x9c, game)
    word(game + 0x53ac, holder)
    word(holder, vtable)
    word(vtable + 0x3c, getter)
    emu.mem_write(getter, b'\xc3')
    word(params + 8, value)
    # GFx::Value is {interface, type, union, ...}. Boolean=2, Number=5.
    # Repeat off/on and include nonzero union padding: only BValue is valid.
    cases = [(2, bytes([on]) + b'\xa5' * 7, on) for on in (1, 0, 1, 0, 1)]
    cases += [(5, struct.pack('<d', n), 0) for n in (0.0, 1.0)]
    for kind, payload, expected in cases:
        word(stack, 0)
        word(stack + 4, params)
        emu.mem_write(value, struct.pack('<II', 0, kind) + payload + b'\0' * 8)
        emu.reg_write(UC_X86_REG_ECX, menu)
        emu.reg_write(UC_X86_REG_ESP, stack)
        emu.emu_start(0x288cd0, 0x273000, count=100)
        assert emu.reg_read(UC_X86_REG_EIP) == 0x273000
        sp = emu.reg_read(UC_X86_REG_ESP)
        assert sp == stack - 8
        actual = emu.mem_read(sp + 4, 1)[0]
        assert actual == expected, (kind, payload.hex(), actual, expected)
    print('Autobuy callback: Boolean off/on preserved; numeric 1 reproduces off (7 cases)')


if __name__ == '__main__':
    check(Path(sys.argv[1]).read_bytes())
