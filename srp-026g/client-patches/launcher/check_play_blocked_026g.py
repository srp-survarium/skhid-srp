#!/usr/bin/env python3
"""Execute the patched launcher's PLAY after CreateProcessW under Unicorn.

Usage: python check_play_blocked_026g.py ORIGINAL_survarium_launcher.exe
Requires unicorn 2.1.4. Starts at the hook (0x1721C) with CreateProcessW's
result in eax and stubs GetLastError, MessageBoxW and CloseHandle through
the import table. A started game closes both handles and goes on to quit; error
4551 shows the box in every launcher language (another setting: English)
and returns without quitting; any other error reaches the original assertion.
Both at the preferred base and rebased.
"""
from pathlib import Path
import struct
import sys
from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
from unicorn.x86_const import *
import patch_launcher_026g as L

STACK, OBJECT, STUBS = 0x30008000, 0x31000000, 0x32000000
FRAME = STACK + 0x400                      # ebp of on_play_button_clicked
IMPORTS = {0x3B218: 'GetLastError', 0x3B388: 'MessageBoxW', 0x3B098: 'CloseHandle'}
ARGUMENTS = {'GetLastError': 0, 'MessageBoxW': 4, 'CloseHandle': 1}
QUIT, ASSERTION, RETURN = 0x172D3, 0x17235, 0x172DC
THREAD, PROCESS = 0x1111, 0x2222


class Native:
    def __init__(self, data, delta):
        self.delta = delta
        self.u = u = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vs, raw, rs, _ in L.P.Headers(data).sections(data):
            address = L.IMAGE_BASE + delta + rva
            u.mem_map(address, (max(vs, rs) + 4095) & ~4095)
            u.mem_write(address, bytes(data[raw:raw + min(rs, vs or rs)]))
        for address in L.relocated(data):
            self.word(address + delta, self.word(address + delta) + delta)
        for address, size in [(STACK - 0x8000, 0x10000), (OBJECT, 0x10000), (STUBS, 0x1000)]:
            u.mem_map(address, size)
        self.stubs = {}
        for n, (iat, name) in enumerate(IMPORTS.items()):
            self.word(iat + delta, STUBS + n * 16)
            self.stubs[STUBS + n * 16] = name
        u.hook_add(UC_HOOK_CODE, self.hook)

    def word(self, address, value=None):
        if value is None:
            return struct.unpack('<I', self.u.mem_read(address, 4))[0]
        self.u.mem_write(address, struct.pack('<I', value & 0xffffffff))

    def wide(self, address):
        units = bytearray()
        for i in range(1024):
            pair = self.u.mem_read(address + 2 * i, 2)
            if pair == b'\0\0':
                return units.decode('utf-16le')
            units.extend(pair)
        raise AssertionError('unterminated string')

    def hook(self, u, address, size, _):
        if address - self.delta in (QUIT, ASSERTION, RETURN):
            u.emu_stop()
            return
        name = self.stubs.get(address)
        if name is None:
            return
        sp = u.reg_read(UC_X86_REG_ESP)
        args = [self.word(sp + 4 + 4 * n) for n in range(ARGUMENTS[name])]
        self.calls.append((name, args))
        result = {'GetLastError': self.error, 'MessageBoxW': 1, 'CloseHandle': 1}[name]
        u.reg_write(UC_X86_REG_EAX, result)
        u.reg_write(UC_X86_REG_ECX, 0xBAD0BAD0)       # volatile, as Windows leaves it
        u.reg_write(UC_X86_REG_EDX, 0xBAD0BAD0)
        u.reg_write(UC_X86_REG_ESP, sp + 4 + 4 * len(args))
        u.reg_write(UC_X86_REG_EIP, self.word(sp))

    def play(self, started, error, language):
        u = self.u
        self.error, self.calls = error, []
        self.word(OBJECT + 0x1A64, language)
        self.word(FRAME - 0x20, THREAD)
        self.word(FRAME - 0x24, PROCESS)
        for register, value in ((UC_X86_REG_EAX, int(started)), (UC_X86_REG_EBX, 0),
                                (UC_X86_REG_ESI, OBJECT), (UC_X86_REG_EDI, 0x5555),
                                (UC_X86_REG_EBP, FRAME), (UC_X86_REG_ESP, STACK)):
            u.reg_write(register, value)
        u.emu_start(L.PLAY_STARTED[0] + self.delta, STUBS + 0x800, count=1000)
        assert u.reg_read(UC_X86_REG_ESP) == STACK, 'the stack is not as it was'
        assert (u.reg_read(UC_X86_REG_EBX), u.reg_read(UC_X86_REG_EBP)) == (0, FRAME)
        stop = u.reg_read(UC_X86_REG_EIP) - self.delta
        boxes = [(args[0], self.wide(args[1]), self.wide(args[2]), args[3])
                 for name, args in self.calls if name == 'MessageBoxW']
        closed = [args[0] for name, args in self.calls if name == 'CloseHandle']
        return stop, boxes, closed


def main():
    original = Path(sys.argv[1]).read_bytes()
    code = Path(L.HERE, 'launcher_values_026g.bin').read_bytes()
    patched, _ = L.build(original, code)
    texts = L.blocked_texts(L.DEFAULT_GAME_EXE)
    for delta in (0, 0x100000):
        native = Native(patched, delta)
        for language in range(5):
            assert native.play(True, 0, language) == (QUIT, [], [THREAD, PROCESS])
            assert native.play(False, 4551, language) == (
                RETURN, [(0, texts[language], 'Skhid', 0x12030)], []), language
            for error in (2, 5, 193, 225, 740, 1260):
                assert native.play(False, error, language) == (ASSERTION, [], []), error
        for language in (5, 0xFFFFFFFF):
            assert native.play(False, 4551, language)[1][0][1] == texts[0]
        print(f'base+{delta:#x}: a started game quits, 4551 shows the box in 5 languages '
              'and keeps the launcher, other errors reach the assertion')


if __name__ == '__main__':
    main()
