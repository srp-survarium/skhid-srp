#!/usr/bin/env python3
"""Prove independent pickup templates with the original profile constructor/copy.

The profile contains self-relative buffer_vectors; memcpy would leave pointers
into the donor. Execute the native routines, including full attachment/upgrade
buffers, then overwrite the donor. No allocator or copy routine is mocked.
"""
from pathlib import Path
import struct
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "srp-026g/client-patches"))
from check_artefacts_026g import Game, STOP
from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_READ
from unicorn.x86_const import UC_X86_REG_EIP

SIZE, BODY_SIZE, ITEMS, STRIDE, COUNT = 0x4634, 0x45FC, 0x1568, 0xB4, 69


def run(data):
    g = Game(data)
    source, target = g.block(SIZE + 16), g.block(SIZE + 16)
    guard = b"\xA5" * 16
    for address, size, constructor in [(source, BODY_SIZE, 0x2F7C50), (target, SIZE, 0x2F73F0)]:
        g.emu.mem_write(address + size, guard)
        g.call(constructor, ecx=address)
        assert g.emu.reg_read(UC_X86_REG_EIP) == STOP

    # A nonempty maximum-capacity pair of inline buffers in every item.
    expected = []
    for index in range(COUNT):
        item = source + ITEMS + index * STRIDE
        g.word(item, index + 1000)
        g.emu.mem_write(item + 4, struct.pack("<H", index + 200))
        g.word(item + 8, 500 + index)
        first = struct.pack("<16I", *[index * 100 + n for n in range(16)])
        second = struct.pack("<18I", *[index * 1000 + n for n in range(18)])
        g.emu.mem_write(item + 0x20, first)
        g.emu.mem_write(item + 0x6C, second)
        g.word(item + 0x18, item + 0x60)
        g.word(item + 0x64, item + 0xB4)
        expected.append((first, second))
    g.emu.mem_write(source + 0x1564, bytes([COUNT]))
    g.emu.mem_write(source + 0x57, b"\x03")
    for kit in range(3):
        for slot in range(23):
            g.emu.mem_write(source + 0x98 + kit * 0x704 + slot * 4,
                           struct.pack("<HH", (kit * 23 + slot) % COUNT, 100 + slot))

    # Record every called instruction: the complete copy must need no heap
    # allocation or hidden retain/release. The called graph below is also
    # bounded by disassembly; all item payloads are scalar bytes.
    visited = set()
    overreads = []
    g.emu.hook_add(UC_HOOK_CODE, lambda _uc, address, _size, _user: visited.add(address))
    def check_source(_uc, _access, address, size, _value, _user):
        if source <= address < source + SIZE and address + size > source + BODY_SIZE:
            overreads.append(address)
    g.emu.hook_add(UC_HOOK_MEM_READ, check_source)
    # Late join supplies only the 45FCh body. The full assignment is a
    # negative control: it reads beyond that source allocation.
    g.call(0x284D10, esi=target, edi=source)
    assert overreads
    g.call(0x2F73F0, ecx=target)
    visited.clear()
    overreads.clear()
    g.call(0x284DC0, (target, source))
    assert g.emu.reg_read(UC_X86_REG_EIP) == STOP
    assert not overreads
    ranges = [(0x284D10, 0x284F1F), (0x270390, 0x2703FB), (0x14AF20, 0x14AF54)]
    assert all(any(lo <= address < hi for lo, hi in ranges) for address in visited)
    visited.clear()
    g.call(0x26F3B0, ecx=source)
    assert g.emu.reg_read(UC_X86_REG_EIP) == STOP
    assert all(0x26F3B0 <= address < 0x26F3D6 for address in visited)
    g.emu.mem_write(source, b"\xCC" * BODY_SIZE)
    for index, (first, second) in enumerate(expected):
        item = target + ITEMS + index * STRIDE
        assert g.read(item) == index + 1000
        assert g.read(item + 8) == index + 500
        assert [g.read(item + offset) for offset in [0x14, 0x18, 0x1C]] == [
            item + 0x20, item + 0x60, item + 0x60]
        assert [g.read(item + offset) for offset in [0x60, 0x64, 0x68]] == [
            item + 0x6C, item + 0xB4, item + 0xB4]
        assert bytes(g.emu.mem_read(item + 0x20, len(first))) == first
        assert bytes(g.emu.mem_read(item + 0x6C, len(second))) == second
    for kit in range(3):
        for slot in range(23):
            assert bytes(g.emu.mem_read(target + 0x98 + kit * 0x704 + slot * 4, 4)) == \
                struct.pack("<HH", (kit * 23 + slot) % COUNT, 100 + slot)
    assert bytes(g.emu.mem_read(target + SIZE, 16)) == guard
    assert bytes(g.emu.mem_read(source + BODY_SIZE, 16)) == guard
    visited.clear()
    g.call(0x26F3B0, ecx=target)
    assert g.emu.reg_read(UC_X86_REG_EIP) == STOP
    assert all(0x26F3B0 <= address < 0x26F3D6 for address in visited)
    for index in range(COUNT):
        item = target + ITEMS + index * STRIDE
        assert g.read(item + 0x18) == item + 0x20
        assert g.read(item + 0x64) == item + 0x6C
    assert bytes(g.emu.mem_read(target + SIZE, 16)) == guard
    assert not overreads
    print("native pickup profile: all 69 full attachment/upgrade buffers are independent, "
          "all three kits preserved, 45FCh source boundary, bounded allocation-free copy, "
          "donor teardown and guards passed; full-assignment overread negative control detected")


if __name__ == "__main__":
    run(Path(sys.argv[1]).read_bytes())
