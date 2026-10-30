#!/usr/bin/env python3
"""Verify native history-record lifecycle boundaries needed by pickup state.

Executes the actual constructor, clear and destructor, including repeated
in-place reconstruction. No pickup hook or extension storage is installed.
Nonempty animation references and heap-overflow buffers remain separate cases.
"""
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "srp-026g/client-patches"))
from check_artefacts_026g import Game, STOP
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EIP, UC_X86_REG_ESP

SIZE = 0xC0BCC
SECTIONS = [(0xB4690, 0x2400), (0xB6AB8, 0xC00), (0xB76E0, 0x800),
            (0xB7F08, 0x4000), (0xBFF80, 0xC00)]


def run(data):
    g = Game(data)
    record = 0x23000010
    g.emu.mem_map(0x23000000, 0xC2000)
    guard = b"\xA5" * 16
    g.emu.mem_write(record - 16, guard)
    g.emu.mem_write(record + SIZE, guard)

    def call(address, args=(), **registers):
        g.call(address, args, **registers)
        assert g.emu.reg_read(UC_X86_REG_EIP) == STOP
        # All three boundaries clean their own arguments.
        assert g.emu.reg_read(UC_X86_REG_ESP) == 0x21007F00
        assert bytes(g.emu.mem_read(record - 16, 16)) == guard
        assert bytes(g.emu.mem_read(record + SIZE, 16)) == guard

    for generation in range(3):
        call(0x324510, esi=record)
        assert g.emu.reg_read(UC_X86_REG_EAX) == record
        assert g.read(record + 0xC0BB0) == 0xFFFFFFFF
        assert g.read(record + 0xC0BB4) == 0
        assert bytes(g.emu.mem_read(record + 0xC0BB8, 3)) == b"\0\0\0"
        for offset, capacity in SECTIONS:
            base = record + offset
            assert g.read(base + capacity + 4) == base
            assert g.read(base + capacity + 8) == capacity
            assert g.read(base + capacity + 0xC) == 0
            g.word(base + capacity + 0xC, 7)
        for place in range(20):
            base = record + place * 0x9054
            assert g.read(base + 0x2050) == 0  # shared animation analysis
            g.word(base + 0x100C, 3)
            g.word(base + 0x2034, 4)

        # Clearing precedes both live capture and wire decoding. It preserves
        # record identity/list links/time; it is not a constructor/destructor.
        g.word(record + 0xC0BB0, 50 * generation)
        g.word(record + 0xC0BA8, 0x1234)
        g.word(record + 0xC0BAC, 0x5678)
        g.word(record + 0xC0BB4, 0xABCDEF01)
        g.emu.mem_write(record + 0xC0BB8, b"\x01")
        call(0x324AD0, (record,))
        assert g.read(record + 0xC0BB0) == 50 * generation
        assert g.read(record + 0xC0BA8) == 0x1234
        assert g.read(record + 0xC0BAC) == 0x5678
        # Negative control: clear_buffers does NOT invalidate the cached
        # CRC. An extension replacing bytes must explicitly invalidate it;
        # otherwise hash returns this stale value without serializing.
        call(0x325150, (0,), eax=record)
        assert g.emu.reg_read(UC_X86_REG_EAX) == 0xABCDEF01
        for offset, capacity in SECTIONS:
            assert g.read(record + offset + capacity + 0xC) == 0
        for place in range(20):
            base = record + place * 0x9054
            assert g.read(base + 0x100C) == 0
            assert g.read(base + 0x2034) == 0
        call(0x324910, (record,))
        # The next iteration deliberately reuses this exact address, as
        # 3252A7/3252AC do when rebuilding an existing history record.
    print("native pickup history: constructor, clear, destructor and same-address rebuild passed")


if __name__ == "__main__":
    run(Path(sys.argv[1]).read_bytes())
