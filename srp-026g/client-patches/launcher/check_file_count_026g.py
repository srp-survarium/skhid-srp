#!/usr/bin/env python3
"""Runs the file count of a download in the patched launcher (patch 17,
launcher_values_026g.asm "Patch 17") under Unicorn, through the sites the
patch changed, at its own base and rebased (check_relay_box_026g's stubs).

Usage: python check_file_count_026g.py ORIGINAL_survarium_launcher.exe

Checks:
- message 0B (on_updater_message's dispatch, 0x15BAC, to the handlers' end
  0x16093, the stack and the launcher's registers as they were): the file
  and how many are kept (launcher_values' FILE_COUNT); 0 of 0, a file past
  the last and file 0 keep none;
- the downloading line's "... left" (refresh_status's load_string call,
  0x16ADF to 0x16AE4): load_string's own text, and after it " (5/69)" while
  a count is kept, 1 of 1 to 65535 of 65535; nothing added with no count, or
  to a text that leaves no room in the buffer; the stack and the registers
  the caller keeps as they were.
"""
from pathlib import Path
import sys
from unicorn.x86_const import UC_X86_REG_ESI
import check_language_026g as C
import check_relay_box_026g as R
import check_torrent_box_026g as T
import patch_launcher_026g as L

HERE = Path(__file__).resolve().parent
LAUNCHER = T.LAUNCHER
FILE_COUNT = 0x4A810
LEFT_TEXT = 0x8D
# (the file, of how many) as the updater says them -> what is kept.
MESSAGES = [((5, 69), (5, 69)), ((1, 1), (1, 1)), ((69, 69), (69, 69)), ((65535, 65535), (65535, 65535)),
            ((0, 0), (0, 0)), ((70, 69), (0, 0)), ((0, 69), (0, 0)), ((1, 0), (0, 0))]


class Count(R.Relay):
    text = '%s left'

    def load_string(self, number, buffer, size, *_):
        self.log.append(('load', number, buffer, size))
        self.u.mem_write(buffer, self.text.encode('utf-16-le') + b'\0\0')
        return 0x77


def message(one, file, of):
    """Message 0B, from on_updater_message's dispatch to its end."""
    payload = T.FRAME + 0x200
    one.u.mem_write(payload, file.to_bytes(2, 'little') + of.to_bytes(2, 'little'))
    sp = C.STACK - 0x100
    one.u.mem_write(sp, bytes(0x40))
    one.u.mem_write(sp + 0xF, bytes([0x0B]))
    one.word(sp + 0x18, payload)
    one.reg(T.UC_X86_REG_ESP, sp)
    kept = ((T.UC_X86_REG_EBX, LAUNCHER), (T.UC_X86_REG_ESI, 0x1234), (T.UC_X86_REG_EDI, 0x5678),
            (T.UC_X86_REG_EBP, 0x9ABC))
    for register, value in kept:
        one.reg(register, value)
    one.u.emu_start(0x15BAC + one.delta, 0x16093 + one.delta, count=500000)
    assert one.reg(T.UC_X86_REG_EIP) == 0x16093 + one.delta
    assert one.reg(T.UC_X86_REG_ESP) == sp
    assert [one.reg(register) for register, _ in kept] == [value for _, value in kept]


def kept(one):
    count = one.u.mem_read(FILE_COUNT + one.delta, 4)
    return int.from_bytes(count[:2], 'little'), int.from_bytes(count[2:], 'little')


def left(one, text='%s left'):
    """The "... left" site -> the buffer's text."""
    buffer = T.FRAME + 0x400
    one.text = text
    one.log.clear()
    one.u.mem_write(buffer, b'\xEE' * 0x100)
    registers = {'ebx': 0x1111, 'edi': 0x2222, 'ebp': 0x3333, 'esi': LAUNCHER}
    assert one.go(0x16ADF, 0x16AE4, args=(LEFT_TEXT, buffer, 0x80), **registers)
    assert [one.reg(r) for r in (T.UC_X86_REG_EBX, T.UC_X86_REG_EDI, T.UC_X86_REG_EBP, UC_X86_REG_ESI)] \
        == [0x1111, 0x2222, 0x3333, LAUNCHER]
    assert one.reg(T.UC_X86_REG_EAX) == 0x77
    assert one.log == [('load', LEFT_TEXT, buffer, 0x80)], one.log
    return one.wide(buffer)


def check(data, delta):
    count = 0
    one = Count(data, delta)
    assert left(one) == '%s left'                   # before any message
    count += 1
    for (file, of), (kept_file, kept_of) in MESSAGES:
        message(one, file, of)
        assert kept(one) == (kept_file, kept_of), (file, of, kept(one))
        assert left(one) == ('%s left' + (f' ({kept_file}/{kept_of})' if kept_of else '')), left(one)
        count += 1
    # The buffer's 128 characters: the count only where " (65535/65535)" and its zero fit.
    message(one, 65535, 65535)
    for length, added in ((111, True), (112, False), (127, False)):
        text = 'x' * length
        assert left(one, text) == text + (' (65535/65535)' if added else ''), length
        count += 1
    return count


def main():
    original = Path(sys.argv[1]).read_bytes()
    code = (HERE / 'launcher_values_026g.bin').read_bytes()
    patched, _ = L.build(original, code)
    C.use_documents(C.DOCUMENTS_FOLDERS[0])
    for delta in (0, 0x100000):
        print(f'base+{delta:#x}: {check(bytes(patched), delta)} file count cases pass')
    print('ALL FILE COUNT CASES PASSED')


if __name__ == '__main__':
    main()
