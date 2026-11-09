#!/usr/bin/env python3
"""Run native memory-budget selection and managed allocation in Unicorn 2.1.4.

Usage: python check_memory_defaults_026g.py <original survarium.exe>

Command-line readers are stubs supplying absent/invalid/explicit values; native
branches, units conversion and allocator instructions execute from the PE.
Patch 124's helper and render arenas run from the key's read to their
registration with the native is_set_as_number<u32> (0x141B0); only the key's
reader 0xDDF20 is a stub.
The fragmentation fixture matches the report's total/used/requested byte counts,
but its block layout is synthetic: the submitted dump does not contain the heap.
It demonstrates why aggregate free bytes do not imply a successful allocation;
it is not a reproduction of the player's sequence of map loads.
"""
import struct
import sys
from pathlib import Path

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
from unicorn.x86_const import *
import patch_026g as patch

STACK, STATE, ARENA, STOP = 0x40008000, 0x41000000, 0x50000000, 0x71000000
MIB = 1024 * 1024


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

    def run(self, begin, end):
        self.uc.emu_start(begin, end, count=100000)
        assert self.uc.reg_read(UC_X86_REG_EIP) == end


def budgets(data, resource_value, mt_value, resource_valid=True, mt_valid=True):
    m = Machine(data)
    uc = m.uc
    m.word(0xA832A4, 1 if resource_value is None else 2)

    def visit(uc, address, _size, _context):
        if address == 0xDDF20:
            assert uc.reg_read(UC_X86_REG_ESI) == 0xA83080
            uc.mem_write(uc.reg_read(UC_X86_REG_EDI), struct.pack('<f', resource_value))
            m.returned(int(resource_valid))
        elif address == 0x424B86:
            # CRT float-to-u64 conversion; not changed by this patch.
            m.returned(int(resource_value))
        elif address == 0x141B0:
            assert uc.reg_read(UC_X86_REG_ESI) == 0xA80150
            if mt_value is not None and mt_valid:
                m.word(m.read(uc.reg_read(UC_X86_REG_ESP) + 4), mt_value)
            m.returned(int(mt_value is not None and mt_valid), 4)
        elif address == 0xE6690:
            # Preserve the native optional platform extra; ordinary player case.
            m.returned(0)

    uc.hook_add(UC_HOOK_CODE, visit)
    m.run(0xE9956, 0xE9999)
    resource_bytes = uc.reg_read(UC_X86_REG_EAX) | uc.reg_read(UC_X86_REG_EDX) << 32
    uc.reg_write(UC_X86_REG_ESP, STACK)
    m.run(0xE0FF0, 0xE1029)
    mt_bytes = uc.reg_read(UC_X86_REG_ESI)
    return resource_bytes, mt_bytes


def arena_size(data, key, begin, end, size_at, value, valid=True):
    """Patch 124: one fixed arena's size as registered (the u64 at size_at
    when register_allocator 0xE21C0 is called at `end`). The native
    is_set_as_number<u32> (0x141B0, its float-to-int) runs; only the key's
    reader 0xDDF20 is a stub: absent, a value that is no number, or one."""
    m = Machine(data)
    uc = m.uc

    def visit(uc, address, _size, _context):
        if address == 0xDDF20:
            assert uc.reg_read(UC_X86_REG_ESI) == key
            given = value is not None and valid
            if given:
                uc.mem_write(uc.reg_read(UC_X86_REG_EDI), struct.pack('<f', value))
            m.returned(int(given))

    uc.hook_add(UC_HOOK_CODE, visit)
    m.run(begin, end)
    sp = uc.reg_read(UC_X86_REG_ESP)
    return m.read(sp + size_at) | m.read(sp + size_at + 4) << 32


def helper_arena(data, value, valid=True):
    # memory::preinitialize: rh_allocator (0xA80380) to the "resources
    # helper allocator" registration (its size at [esp+18h]).
    return arena_size(data, 0xA80380, 0xE108F, 0xE10D7, 0x18, value, valid)


def render_arena(data, value, valid=True):
    # engine_world's constructor: render_allocator (0xA7C088) to the
    # "render" registration (its size at [esp+78h]).
    return arena_size(data, 0xA7C088, 0x13A81, 0x13AF4, 0x78, value, valid)


def fragmented_allocation(data, size):
    """Same held blocks in both arenas; extra capacity stays free at the end."""
    total, used, requested = 295436288, 173033088, 22369685
    m = Machine(data)
    uc = m.uc
    # Eight live blocks alternating with eight holes, all aligned to 128 bytes.
    parts = []
    for amount in (used, total - used):
        chunk = amount // 8 // 128 * 128
        parts.append([chunk] * 7 + [amount - chunk * 7])
    nodes, at = [], ARENA
    for live, free in zip(*parts):
        nodes.extend([(at, live, False), (at + live, free, True)])
        at += live + free
    last, length, free = nodes[-1]
    nodes[-1] = (last, length + size - total, free)
    # Map headers only: the allocator never reads or writes resource payloads.
    pages = set()
    for at, length, free in nodes:
        for addr in (at, at + ((requested + 0x34 + 127) & ~127)):
            page = addr & ~4095
            if page not in pages:
                uc.mem_map(page, 8192)
                pages.update((page, page + 4096))
    frees = [at for at, _, free in nodes if free]
    for i, (at, length, free) in enumerate(nodes):
        m.word(at + 4, nodes[i + 1][0] if i + 1 < len(nodes) else 0)
        m.word(at + 8, nodes[i - 1][0] if i else 0)
        m.word(at + 0x28, length)
        uc.mem_write(at + 0x30, bytes([int(free)]))
        if free:
            idx = frees.index(at)
            m.word(at + 0xC, frees[idx + 1] if idx + 1 < len(frees) else 0)
    m.word(STATE, frees[0])
    m.word(STATE + 0x10, 128)
    m.word(STATE + 0x14, size - used)
    m.word(STACK, STOP)
    m.word(STACK + 4, STATE)
    uc.reg_write(UC_X86_REG_ECX, requested)
    m.run(0x10A240, STOP)
    return uc.reg_read(UC_X86_REG_EAX)


def single_region_fallback(data):
    """Native fallback can shrink requested pools below the resource floor.

    Begin after virtual-address enumeration with one available region. Execute
    the native ratio, integer conversion, alignment, and allocation branches;
    stub only VirtualAlloc success. This is a synthetic address-space layout,
    not one recovered from the submitted dump.
    """
    m = Machine(data)
    uc = m.uc
    frame, region = STACK + 0x200, STATE + 0x100
    small, large = STATE + 0x200, STATE + 0x220
    for address, value in (
        (small, 256 * MIB), (large, 512 * MIB),
        (region, 443154432), (region + 8, 0x80000000),
        (frame + 8, region + 16), (frame - 0x24, region),
        (frame - 8, 768 * MIB), (frame - 0x68, 65536),
        (0x87015C, STOP + 0x100),
    ):
        m.word(address, value)
    for register, value in (
        (UC_X86_REG_EBP, frame), (UC_X86_REG_ESP, STACK),
        (UC_X86_REG_ESI, small), (UC_X86_REG_EDI, large),
        (UC_X86_REG_EBX, region), (UC_X86_REG_EAX, region + 16),
    ):
        uc.reg_write(register, value)
    allocations = []

    def visit(uc, address, _size, _context):
        if address == STOP + 0x100:
            sp = uc.reg_read(UC_X86_REG_ESP)
            base, size = m.read(sp + 4), m.read(sp + 8)
            assert (m.read(sp + 12), m.read(sp + 16)) == (0x3000, 4)
            allocations.append((base, size))
            m.returned(base, 16)

    uc.hook_add(UC_HOOK_CODE, visit)
    m.run(0xE8891, 0xE8BF3)
    assert allocations == [(0x80000000, 147718144), (0x88CE0000, 295436288)]
    return m.read(small), m.read(large)


def main():
    original = Path(sys.argv[1]).read_bytes()
    built, _ = patch.build(original, 'section', Path(patch.default_bin('section')).read_bytes(),
                           late_join=Path(patch.late_join_bin()).read_bytes())
    assert budgets(original, None, None) == (336 * MIB, 376 * MIB)
    assert budgets(built, None, None) == (768 * MIB, 768 * MIB)
    assert budgets(built, 99, 99, False, False) == (768 * MIB, 768 * MIB)
    for value in (256, 512, 768, 1024):
        assert budgets(built, value, value) == budgets(original, value, value) == (value * MIB, value * MIB)
    assert budgets(built, None, 512) == (768 * MIB, 512 * MIB)
    assert budgets(built, 512, None) == (512 * MIB, 768 * MIB)
    assert fragmented_allocation(original, 295436288) == 0
    assert fragmented_allocation(built, 512 * MIB) != 0
    assert single_region_fallback(original) == single_region_fallback(built) == (147718144, 295436288)
    # Patch 124: the defaults change; an option gives the size it gave.
    assert helper_arena(original, None) == helper_arena(original, 99, False) == 8 * MIB
    assert helper_arena(built, None) == helper_arena(built, 99, False) == 32 * MIB
    for value in (4, 8, 16, 32, 64):
        assert helper_arena(built, value) == helper_arena(original, value) == value * MIB
    assert render_arena(original, None) == render_arena(original, 99, False) == 224 * MIB
    assert render_arena(built, None) == render_arena(built, 99, False) == 320 * MIB
    for value, size in ((100, 224), (224, 224), (256, 256), (320, 320), (512, 512)):
        assert render_arena(built, value) == render_arena(original, value) == size * MIB
    print('PASS: native defaults, independent/explicit/invalid option paths, fragmentation fixture, '
          'address-space fallback limitation, helper/render arena defaults and options')


if __name__ == '__main__':
    main()
