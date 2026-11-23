#!/usr/bin/env python3
"""Execute the replay reader and native Windows open, with OS effects stubbed.

The path-normalization service is stubbed; the actual reader options, native
file-open assertion guard, result branch and generated Rust entries execute.
"""
import struct
import sys
from pathlib import Path

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
from unicorn.x86_const import *
import patch_026g as patch
STACK, STATE, STOP = 0x40008000, 0x41000000, 0x71000000


class Machine:
    def __init__(self, data):
        self.uc = uc = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in patch.Headers(data).sections(data):
            size = max(vsize, rsize)
            if not size:
                continue
            address = patch.IMAGE_BASE + rva
            uc.mem_map(address, (size + 4095) & ~4095)
            uc.mem_write(address, bytes(data[raw:raw + rsize]))
        for at in (STACK & ~65535, STATE, STOP):
            uc.mem_map(at, 65536)
        uc.reg_write(UC_X86_REG_ESP, STACK)
        uc.reg_write(UC_X86_REG_EBX, 0)

    def word(self, at, value):
        self.uc.mem_write(at, struct.pack('<I', value))

    def read(self, at):
        return struct.unpack('<I', self.uc.mem_read(at, 4))[0]

    def returned(self, value, pop=0):
        uc = self.uc
        sp = uc.reg_read(UC_X86_REG_ESP)
        uc.reg_write(UC_X86_REG_EAX, value)
        uc.reg_write(UC_X86_REG_EDX, 0)
        uc.reg_write(UC_X86_REG_EIP, self.read(sp))
        uc.reg_write(UC_X86_REG_ESP, sp + 4 + pop)


def run(data, path, *, opened=False, normalized=True):
    m = Machine(data)
    uc = m.uc
    wrapper, fs, table, name = STATE, STATE + 0x100, STATE + 0x200, STATE + 0x1000
    for at, value in [(wrapper, fs), (fs, table), (fs + 4, 0xffffffff),
                      (table + 4, 0x40000), (0x870048, STOP + 0x100),
                      (0xaeff30, STOP + 0x200), (0x87020c, STOP + 0x300)]:
        m.word(at, value)
    uc.mem_write(name, path + b'\0')
    uc.reg_write(UC_X86_REG_ESI, name)
    uc.reg_write(UC_X86_REG_EDI, wrapper)
    uc.reg_write(UC_X86_REG_EBX, 0)
    uc.reg_write(UC_X86_REG_XMM3, 0x123456789abcdef)
    events = []

    def cstr(at):
        result = bytearray()
        while byte := bytes(uc.mem_read(at + len(result), 1))[0]:
            result.append(byte)
        return bytes(result)

    def visit(uc, address, _size, _context):
        sp = uc.reg_read(UC_X86_REG_ESP)
        if address == 0x40f10:
            # Engine path normalization, source native path -> destination.
            events.append(('normalize', m.read(sp + 8)))
            source = cstr(m.read(m.read(sp + 4)))
            dest = uc.reg_read(UC_X86_REG_ESI)
            uc.mem_write(m.read(dest), source + b'\0')
            m.word(dest + 4, m.read(dest) + len(source))
            m.returned(int(normalized))
        elif address == 0x40590:
            # Special cached device-name comparison: ordinary replay file.
            m.returned(0)
        elif address == STOP + 0x100:
            assert cstr(m.read(sp + 4)) == path
            assert [m.read(sp + n) for n in (8, 12, 16, 20, 28)] == [0x80000000, 3, 0, 3, 0]
            events.append(('open',))
            m.returned(0x1234 if opened else 0xffffffff, 28)
        elif address == STOP + 0x200:
            assert m.read(sp + 4) == 0
            text = cstr(m.read(sp + 8))
            assert path in text
            assert b"Couldn't open replay" in text
            assert b'replace the example path' in text
            assert text.endswith(b'This replay window will now close.')
            assert cstr(m.read(sp + 12)) == b"Skhid - Couldn't open replay"
            events.append(('message',))
            m.returned(1, 16)
        elif address == STOP + 0x300:
            assert m.read(sp + 4) == 1
            events.append(('exit',))
            uc.emu_stop()
        elif address == 0x113450:
            events.append(('assert',))
            uc.emu_stop()

    uc.hook_add(UC_HOOK_CODE, visit)
    uc.emu_start(0x1f01f9, 0x1f02b0, count=200000)
    if events[-1] not in [('exit',), ('assert',)]:
        assert uc.reg_read(UC_X86_REG_EIP) == 0x1f02b0
        assert uc.reg_read(UC_X86_REG_ESP) == STACK + 0x154
        assert uc.reg_read(UC_X86_REG_EAX) == 0x1234
        assert m.read(STACK + 0x10) == 0x1234
        assert uc.reg_read(UC_X86_REG_ESI) == name
        assert uc.reg_read(UC_X86_REG_XMM3) == 0x123456789abcdef
    return events


def main():
    stock = Path(sys.argv[1]).read_bytes()
    built, _ = patch.build(stock, 'section', Path(patch.default_bin('section')).read_bytes(),
                           late_join=Path(patch.late_join_bin()).read_bytes())
    for path in [rb'Z:\full\path\to\x.sur', rb'C:\My Games\match.sur',
                 rb'\\server\share\match.sur', b'C:\\' + b'x' * 252 + b'.sur',
                 'C:\\повтор.sur'.encode('cp1251')]:
        assert run(stock, path)[-1] == ('assert',)
        assert run(built, path) == [('normalize', 0), ('open',), ('message',), ('exit',)]
        assert run(built, path, normalized=False) == [('normalize', 0), ('message',), ('exit',)]
        assert run(stock, path, opened=True) == [('normalize', 1), ('open',)]
        assert run(built, path, opened=True) == [('normalize', 0), ('open',)]
    at = patch.file_offset(0x40000)
    assert built[at:at+0x190] == stock[at:at+0x190], 'global file open must stay unchanged'
    print('PASS: actual replay/native open, assertion replaced by one dialog and exit, '
          'success/normalization failure, spaces/UNC/Cyrillic/259-byte paths, stack/SSE')


if __name__ == '__main__':
    main()
