#!/usr/bin/env python3
"""Execute native launcher install-status selection and wcscat_s under Unicorn.

Usage: python check_install_status_026g.py ORIGINAL_survarium_launcher.exe
Requires unicorn 2.1.4. Reproduces the 2026-10-18 launcher dump's stale
install-info id and stack-local string id, including the native ERANGE
failure. Checks the guarded selector at both preferred and rebased addresses
using every shipped language's actual RT_STRING resources.
"""
from pathlib import Path
import struct
import sys
from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
from unicorn.x86_const import *
import patch_launcher_026g as L

STACK, OBJECT, STOP, ERRNO = 0x30008000, 0x31000000, 0x32000000, 0x32000100


def resources(data):
    image = L.Image(bytearray(data))
    _, rva, _, raw, _, _ = next(s for s in L.P.Headers(data).sections(data) if s[0].rstrip(b'\0') == b'.rsrc')
    result = {}
    def walk(at, path):
        named, ids = struct.unpack_from('<HH', data, raw + at + 12)
        for i in range(named + ids):
            name, child = struct.unpack_from('<II', data, raw + at + 16 + i * 8)
            if child & 0x80000000:
                walk(child & 0x7fffffff, path + [name])
            elif path[0] == 6:
                address, size = struct.unpack_from('<II', data, raw + child)
                pos = image.offset(L.IMAGE_BASE + address, size)
                for j in range(16):
                    n = struct.unpack_from('<H', data, pos)[0]; pos += 2
                    text = data[pos:pos + n * 2].decode('utf-16le'); pos += n * 2
                    if n: result.setdefault(name, {})[(path[1] - 1) * 16 + j] = text
    walk(0, [])
    return result


class Native:
    def __init__(self, data, delta=0):
        self.delta = delta
        self.u = u = Uc(UC_ARCH_X86, UC_MODE_32)
        headers = L.P.Headers(data)
        for _, rva, vs, raw, rs, _ in headers.sections(data):
            address = L.IMAGE_BASE + delta + rva
            u.mem_map(address, (max(vs, rs) + 4095) & ~4095)
            u.mem_write(address, bytes(data[raw:raw + min(rs, vs or rs)]))
        for address in L.relocated(data):
            self.word(address + delta, self.word(address + delta) + delta)
        for address, size in [(STACK - 0x8000, 0x10000), (OBJECT, 0x10000), (STOP, 0x1000)]:
            u.mem_map(address, size)
        self.strings = {}
        self.selected = []
        self.invalid = False
        self.end = 0
        u.hook_add(UC_HOOK_CODE, self.hook)

    def word(self, address, value=None):
        if value is None: return struct.unpack('<I', self.u.mem_read(address, 4))[0]
        self.u.mem_write(address, struct.pack('<I', value & 0xffffffff))

    def wide(self, address, value=None):
        if value is not None:
            self.u.mem_write(address, value.encode('utf-16le') + b'\0\0'); return
        units = bytearray()
        for i in range(2048):
            pair = self.u.mem_read(address + 2 * i, 2)
            if pair == b'\0\0': return units.decode('utf-16le')
            units.extend(pair)
        raise AssertionError('unterminated string')

    def ret(self, value=0):
        sp = self.u.reg_read(UC_X86_REG_ESP)
        self.u.reg_write(UC_X86_REG_EAX, value)
        self.u.reg_write(UC_X86_REG_ESP, sp + 4)
        self.u.reg_write(UC_X86_REG_EIP, self.word(sp))

    def hook(self, u, address, size, _):
        at = address - self.delta
        if address == self.end:
            u.emu_stop(); return
        sp = u.reg_read(UC_X86_REG_ESP)
        if at == 0x137a8:  # LoadString wrapper; use the executable's own resources.
            sid, dst, count = [self.word(sp + n) for n in (4, 8, 12)]
            assert sid in self.strings, f'out-of-table resource id {sid:#x}'
            self.selected.append(sid)
            text = self.strings[sid][:count - 1]
            self.wide(dst, text)
            self.ret(bool(text))
        elif at == 0x17afa:  # Only the one-%s install label; UI/CRT setup is irrelevant here.
            dst, fmt, name = [self.word(sp + n) for n in (4, 8, 12)]
            text = self.wide(fmt)
            self.wide(dst, text.replace('%s', self.wide(name)))
            self.ret()
        elif at == 0x29250:  # _errno, used by the real wcscat_s on overflow.
            self.ret(ERRNO)
        elif at == 0x27d04:  # Native invalid-parameter path from the reported dump.
            self.invalid = True
            u.emu_stop()

    def select(self, msg, strings, stale_id, filename='resources.db.patch_20'):
        u = self.u
        self.strings, self.selected = strings, []
        u.mem_write(STACK - 0x100, bytes(0x1000))
        self.wide(OBJECT + 0x4fe4, filename)
        self.word(OBJECT + 0x51ec, msg)
        # The unguarded msg=2 lookup reads ESP+0x10: an unrelated local.
        self.word(STACK + 0x10, stale_id)
        u.reg_write(UC_X86_REG_ESI, OBJECT)
        u.reg_write(UC_X86_REG_EBX, 0)
        u.reg_write(UC_X86_REG_ESP, STACK)
        self.end = 0x16d4b + self.delta
        u.emu_start(0x16cdd + self.delta, STOP, count=10000)
        assert u.reg_read(UC_X86_REG_EIP) == self.end
        assert u.reg_read(UC_X86_REG_ESP) == STACK - 12
        return self.wide(STACK + 0x30)

    def append(self, text, suffix):
        self.wide(OBJECT, text); self.wide(OBJECT + 0x1000, suffix)
        for i, value in enumerate((STOP, OBJECT, 128, OBJECT + 0x1000)):
            self.word(STACK + i * 4, value)
        self.u.reg_write(UC_X86_REG_ESP, STACK)
        self.invalid = False
        self.end = STOP
        self.u.emu_start(0x25dc9 + self.delta, STOP, count=10000)
        return not self.invalid


def main():
    original = Path(sys.argv[1]).read_bytes()
    code = L.code()
    patched, _ = L.build(original, code)
    texts = resources(original)
    russian = texts[0x419]
    warning_id = next(i for i, t in russian.items() if t.startswith('Характеристики центрального'))
    for delta in (0, 0x100000):
        old, new = Native(original, delta), Native(patched, delta)
        label = old.select(2, russian, warning_id)
        assert old.selected == [warning_id]
        assert not old.append(label + ', ', 'осталось 0 секунд')
        assert old.word(ERRNO) == 34  # ERANGE, exactly the dump's ESI.
        count = 0
        for lang, strings in texts.items():
            for msg in list(range(256)) + [0xffffffff, 0x10000]:
                label = new.select(msg, strings, warning_id)
                expected = 0xb3 + msg - 6 if 6 <= msg <= 9 else 0xa9
                assert new.selected == [expected], (lang, msg, new.selected)
                assert new.append(label + ', ', '0 seconds left'), (lang, msg, label)
                count += 1
            # An empty filename already takes the generic native path.
            assert new.select(7, strings, warning_id, filename='') == strings[0xa9]
        print(f'base+{delta:#x}: original reproduces ERANGE; {count} patched status/append cases pass')


if __name__ == '__main__':
    main()
