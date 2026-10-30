#!/usr/bin/env python3
"""Real native match-table/parser checks with the linked Rust pickup hooks.

No network socket or game client is started. The original message parser,
info getters, fixed-vector append and message-data constructor execute.
The reused Inputs fixture stubs logging/timeline endpoints only; transport
calls themselves are not replaced. Gameplay dispatch is tested separately.
"""
from pathlib import Path
import struct
import sys

from check_pickup_hooks import HookedInputs, base
from check_pickup_inputs import Inputs
from check_artefacts_026g import STACK, STOP
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_ESI,
                              UC_X86_REG_ESP, UC_X86_REG_EIP, UC_X86_REG_EDI)


def make_table(g):
    table = g.block(0x10)
    g.emu.reg_write(UC_X86_REG_ESI, table)
    g.emu.reg_write(UC_X86_REG_ESP, STACK - 0x100)
    g.emu.emu_start(0x2792CC, 0x2792E6, count=100_000)
    assert g.emu.reg_read(UC_X86_REG_EIP) == 0x2792E6
    assert g.read(table) == 0xA08414
    return table


def info(g, message, received):
    output = g.block(8)
    g.emu.mem_write(output, b'\xA5' * 8)
    g.call(0x279B10 if received else 0x279A60, eax=output, ecx=message)
    assert g.emu.reg_read(UC_X86_REG_EIP) == STOP
    assert g.emu.reg_read(UC_X86_REG_ESP) == STACK - 0x100
    assert g.emu.reg_read(UC_X86_REG_EAX) == output
    assert bytes(g.emu.mem_read(output + 7, 1)) == b'\xA5'
    return bytes(g.emu.mem_read(output, 7))


def length(g, table, message, size=64, cursor_offset=0):
    payload, ok = g.block(128), g.block(4)
    # Both ED/EE masks have two set bits with the one-byte roster width.
    g.emu.mem_write(payload, b'\x03' * 128)
    g.emu.mem_write(table + 8, b'\x01')
    g.call(0x279670, (message, payload, payload + cursor_offset, size, 1, ok), ecx=table)
    assert g.emu.reg_read(UC_X86_REG_EIP) == STOP
    assert g.emu.reg_read(UC_X86_REG_ESP) == STACK - 0x100
    return g.emu.reg_read(UC_X86_REG_EAX) & 0xFFFF, bytes(g.emu.mem_read(ok, 1))[0]


def parse(g, table, wire, single=True):
    payload = g.block(len(wire) + 16)
    g.emu.mem_write(payload, wire + b'\xA5' * 16)
    reader, vector, storage = g.block(12), g.block(12), g.block(0x1C * 4)
    for at, value in [(reader, payload), (reader + 4, payload), (reader + 8, len(wire)),
                      (vector, storage), (vector + 4, storage), (vector + 8, storage + 0x1C * 4)]:
        g.word(at, value)
    messages = []
    while g.read(reader + 4) < payload + len(wire):
        before = g.read(vector + 4)
        g.call(0x3FA240, (vector, int(single)), eax=table, ecx=reader)
        assert g.emu.reg_read(UC_X86_REG_EIP) == STOP
        assert g.emu.reg_read(UC_X86_REG_ESP) == STACK - 0x108  # caller-clean two arguments
        assert g.read(reader + 4) <= payload + len(wire)
        if not (g.emu.reg_read(UC_X86_REG_EAX) & 0xFF):
            assert g.read(vector + 4) == before
            return False, messages
        assert g.read(vector + 4) == before + 0x1C
        start, cursor, size = struct.unpack('<III', bytes(g.emu.mem_read(before, 12)))
        assert start == cursor and payload <= start <= payload + len(wire)
        assert start + size <= payload + len(wire)
        messages.append(bytes(g.emu.mem_read(start, size)))
        if single:
            break
    return True, messages


def dispatch(data, linked):
    i = HookedInputs(data, linked)
    g = i.g
    wrapper = g.block(0x19400)
    g.emu.mem_write(wrapper + 0x1F8, bytes(g.emu.mem_read(i.core, 0x19000)))
    i.core = wrapper + 0x1F8
    g.word(wrapper + 0x191B8, 1000)
    client = 0x25000000
    g.emu.mem_map(client, 0x200000)
    g.word(client + 0x1F1C40, wrapper)
    g.word(client + 0x1F1C64, 2)
    i.lifecycle(True, i.core)
    recorded = []

    def recorder(emu, at, _size, _user):
        if at != 0x279480:
            return
        esp = emu.reg_read(UC_X86_REG_ESP)
        reader = emu.reg_read(UC_X86_REG_EDI)
        assert g.read(reader) == g.read(reader + 4)  # native tail rewound it
        recorded.append((g.read(esp + 4), bytes(emu.mem_read(g.read(reader), g.read(reader + 8)))))
        emu.reg_write(UC_X86_REG_EIP, g.read(esp))
        emu.reg_write(UC_X86_REG_ESP, esp + 4)  # cdecl, caller cleans ID

    g.emu.hook_add(UC_HOOK_CODE, recorder)
    mode = g.block(4)
    g.word(mode, 1)
    g.word(0xA83948, mode)
    g.word(0xA8394C, 1)

    def receive(message, payload):
        at, reader = g.block(len(payload)), g.block(12)
        g.emu.mem_write(at, payload)
        for offset, value in [(0, at), (4, at), (8, len(payload))]:
            g.word(reader + offset, value)
        try:
            g.call(0x27A300, (message, reader), ecx=client)
        except Exception as error:
            raise AssertionError(f'dispatch {message:#x} stopped at '
                                 f'{g.emu.reg_read(UC_X86_REG_EIP):#x}') from error
        assert g.emu.reg_read(UC_X86_REG_EIP) == STOP
        assert g.emu.reg_read(UC_X86_REG_ESP) == STACK - 0x100
        assert g.read(reader + 4) == at + len(payload)
        assert recorded[-1] == (message, payload)

    relay = bytes([0, 18, 23]) + struct.pack('<II', 0, 4) + bytes([1]) + struct.pack('<I', 700)
    receive(0xF7, relay)
    assert i.inputs(0) == [(923, 0, 4)]
    dirty = list(i.dirty)
    receive(0xF7, relay)
    assert i.inputs(0) == [(923, 0, 4)] and i.dirty == dirty
    # Populated journal now runs through the real move/discard hooks too.
    assert i.move(923, 1000) == 0
    assert i.inputs(0) == [(1000, 0, 4)]
    assert i.discard(1000) == 0
    assert i.inputs(0) == []
    g.word(client + 0x1F1C64, 1)
    receive(0xF7, relay)
    assert i.inputs(0) == []
    g.word(client + 0x1F1C64, 2)
    # Existing EB goes through the original dispatcher/decoder unchanged.
    ordinary = bytes([19, 0]) + struct.pack('<II', 0, 8)
    receive(0xEB, ordinary)
    assert i.inputs(0) == [(950, 0, 8)]
    # Original ED/EE decoder loops through the same completion for each place.
    options = 0x26000000
    g.emu.mem_map(options, 0x58000)
    g.emu.mem_write(options + 0x577B5, b'\x02')
    vtable = g.block(0x38)
    getter = 0x2200F100
    g.emu.mem_write(getter, b'\xB8' + struct.pack('<I', options) + b'\xC3')
    g.word(client, vtable)
    g.word(vtable + 0x34, getter)
    # Duplicate inputs also execute the native statistics path. Supply its
    # disabled statistics object, keeping the completion function itself real.
    stats = 0x27000000
    g.emu.mem_map(stats, 0x200000)
    enabled, provider, stats_vtable = g.block(4), g.block(4), g.block(0x30)
    g.word(stats + 0x1E3CCC, enabled)
    g.word(enabled, 0)
    stats_getter = 0x2200F200
    g.emu.mem_write(stats_getter, b'\xB8' + struct.pack('<I', stats) + b'\xC3')
    g.word(client + 0x1F1C48, provider)
    g.word(provider, stats_vtable)
    g.word(stats_vtable + 0x2C, stats_getter)
    batch = bytes([22, 3]) + struct.pack('<IIHHHH', 16, 32, 0, 0, 0, 0)
    receive(0xED, batch)
    assert i.inputs(0) == [(950, 0, 8), (1100, 0, 16)]
    assert i.inputs(1) == [(1100, 0, 32)]
    precise_batch = bytes([23, 23, 3]) + struct.pack('<IIHHHH', 64, 128, 0, 0, 0, 0)
    receive(0xEE, precise_batch)
    assert i.inputs(0)[-1] == (1173, 0, 64)
    assert i.inputs(1)[-1] == (1173, 0, 128)
    before, dirty = [i.inputs(place) for place in range(2)], list(i.dirty)
    receive(0xED, batch)
    receive(0xEE, precise_batch)
    assert [i.inputs(place) for place in range(2)] == before and i.dirty == dirty
    # A full native input pool can evict the source while moving its original
    # portion. Exercise the linked begin/eviction/end hooks with an active
    # target journal, including exact source-address reuse.
    # Native original/corrected splitting belongs to the local player's list.
    g.emu.mem_write(i.core + 0x18FDC, b'\x02')
    receive(0xF7, bytes([2, 16, 0]) + struct.pack('<II', 0, 4)
            + bytes([1]) + struct.pack('<I', 700))
    assert i.submit(800, place=2, corrected=True, actions=2) == (0, 800)
    source = g.read(i.nodes + 2 * 0x480 + 4)
    for n in range(26):
        time = 850 + n * 50
        assert i.submit(time, place=2) == (0, time)
    assert i.move(800, 2150, place=2) == 0
    assert i.inputs(2)[0][0] == 850 and i.inputs(2)[-1] == (2150, 0, 4), i.inputs(2)
    assert g.read(i.nodes + 2 * 0x480 + 8) == source
    # Further ordinary and targeted placement exercise both registry-owned
    # and checked-out journal eviction paths.
    receive(0xEB, bytes([44, 2]) + struct.pack('<II', 0, 8))
    receive(0xF7, bytes([2, 45, 0]) + struct.pack('<II', 0, 16)
            + bytes([1]) + struct.pack('<I', 701))
    assert len(i.inputs(2)) == 27
    assert i.inputs(2)[-2:] == [(2200, 0, 8), (2250, 0, 16)]
    # Completion must release the move transaction after native destination
    # refusal and retiming too, so a later receive/teardown remains valid.
    g.emu.mem_write(i.core + 0x18FDC, b'\x03')
    receive(0xF7, bytes([3, 16, 0]) + struct.pack('<II', 0, 4)
            + bytes([1]) + struct.pack('<I', 700))
    g.word(i.core + 0x18FD4, 600)
    assert i.move(800, 550, place=3) == 6
    assert i.inputs(3) == []
    g.word(i.core + 0x18FD4, 0)
    receive(0xF7, bytes([3, 17, 0]) + struct.pack('<II', 0, 8)
            + bytes([1]) + struct.pack('<I', 701))
    assert i.inputs(3) == [(850, 0, 8)]
    assert i.move(850, 423, place=3) == 2
    assert i.inputs(3) == [(1023, 0, 8)]
    receive(0xF7, bytes([4, 46, 255]) + struct.pack('<II', 0x3C003C00, 32)
            + bytes([1]) + struct.pack('<I', 702))
    receive(0xF7, bytes([5, 47, 23]) + struct.pack('<II', 0x40003C00, 64)
            + bytes([1]) + struct.pack('<I', 703))
    sending(i)
    i.lifecycle(False, i.core)
    assert not i.allocations
    print('native pickup dispatch: real Rust receive/placement, duplicate, populated '
          'move/discard, full-pool eviction/source reuse, outside-match consumption, '
          'ordinary EB/ED/EE and recorder-tail routing passed')


def sending(i):
    """Execute the native history sender and linked Rust D7 replacement.

    Pool checkout supplies blank packet storage; original construction,
    metadata initialization, payload writers and match-client wrappers run.
    Statistics and asynchronous queue submission are recording endpoints.
    No socket or transport worker is started.
    """
    g = i.g
    transport = 0x28000000
    g.emu.mem_map(transport, 0x250000)
    g.word(transport, 0xA08864)
    low = transport + 0x578B0
    g.word(low + 0xA8, make_table(g))
    disconnected = g.block(4)
    g.word(low + 0x1E55C0, disconnected)
    packets = []

    def endpoints(emu, at, _size, _user):
        if at not in (0x279D80, 0x351660, 0x379640):
            return
        esp = emu.reg_read(UC_X86_REG_ESP)
        cleanup = 0
        if at == 0x279D80:
            emu.reg_write(UC_X86_REG_EAX, g.block(0x2C0))
        elif at == 0x351660:
            cleanup = 4
        else:
            assert g.read(esp + 4) == low
            packet = g.read(esp + 8)
            begin, size = g.read(packet + 0x29C), g.read(packet + 0x2A4)
            assert 4 <= size <= 20
            packets.append(bytes(emu.mem_read(begin + 3, size - 3)))
            assert bytes(emu.mem_read(transport + 4, 1)) == b'\x01'
            cleanup = 8
        emu.reg_write(UC_X86_REG_EIP, g.read(esp))
        emu.reg_write(UC_X86_REG_ESP, esp + 4 + cleanup)

    handle = g.emu.hook_add(UC_HOOK_CODE, endpoints)
    slot, event, sent, context = g.block(4), g.block(0x1C0), g.block(4), g.block(20)
    g.word(slot, transport)
    for offset, value in [(0, slot), (4, i.core), (8, event), (12, sent), (16, 20)]:
        g.word(context + offset, value)

    def emit(place, time):
        g.word(event + 0x19C, 0)  # recipient20's already-sent input mask
        g.word(event + 0x1AC, 1 << place)
        g.word(event + 0x1BC, time)
        g.word(sent, 0)
        before = len(packets)
        try:
            g.call(0x27E9F0, (context,))
        except Exception as error:
            raise AssertionError(f'input sender stopped at {g.emu.reg_read(UC_X86_REG_EIP):#x}') from error
        assert g.emu.reg_read(UC_X86_REG_EIP) == STOP
        assert g.emu.reg_read(UC_X86_REG_ESP) == STACK - 0x100
        assert g.read(event + 0x19C) == 1 << place
        assert g.read(sent) == 1 and len(packets) == before + 1
        return packets[-1]

    assert emit(0, 950) == b'\xDC\x13' + struct.pack('<II', 0, 8)
    node = g.read(i.nodes + 2 * 0x480 + 8)
    g.emu.mem_write(node + 0x27, b'\x01')
    expected = b'\xD7' + bytes([45, 0]) + struct.pack('<II', 0, 0x80000010) + bytes([1]) + struct.pack('<I', 701)
    assert emit(2, 2250) == expected
    assert emit(2, 2250) == expected  # target remains attached, no consumption
    # A corrected contribution changes the combined native mask, but the
    # outgoing request must keep the selected original component.
    g.emu.mem_write(i.core + 0x18FDC, b'\x02')
    assert i.submit(2250, place=2, corrected=True, actions=32) == (0, 2250)
    assert g.read(node + 0xC) == 48
    assert emit(2, 2250) == expected
    # Corrected targets are received state, never a fresh pickup request.
    assert emit(3, 1023)[0] == 0xDD
    for place, time, tick, ms, rotation, actions, spawned in [
        (4, 2300, 46, 255, 0x3C003C00, 32, 702),
        (5, 2373, 47, 23, 0x40003C00, 64, 703),
    ]:
        assert emit(place, time) == (b'\xD7' + bytes([tick, ms])
            + struct.pack('<II', rotation, actions) + bytes([1]) + struct.pack('<I', spawned))
    before = len(packets)
    g.word(sent, 0)
    g.call(0x27E9F0, (context,))
    assert len(packets) == before and g.read(sent) == 0
    g.emu.hook_del(handle)
    print('native pickup sending: DC passthrough, D7 exact payload/high action bit/precise zero, '
          'repeat serialization, corrected-target exclusion and native loop accounting passed')


def run(data):
    linked = base.arena()
    patched, stock = HookedInputs(data, linked).g, Inputs(data).g
    table, original = make_table(patched), make_table(stock)
    assert bytes(patched.emu.mem_read(table + 4, 5)) == bytes([0xE3, 0xF7, 0xD7, 0xE1, 0])
    assert bytes(stock.emu.mem_read(original + 4, 5)) == bytes([0xE3, 0xF6, 0xD8, 0xE1, 0])
    for received, ids in [(False, range(0xD8, 0xE2)), (True, range(0xE3, 0xF7))]:
        for message in ids:
            assert info(patched, message, received) == info(stock, message, received), hex(message)
            assert length(patched, table, message) == length(stock, original, message), hex(message)
    assert info(patched, 0xD7, False) == struct.pack('<IHB', 500, 50, 0x9F)
    assert info(patched, 0xF7, True) == struct.pack('<IHB', 250, 50, 0x9F)
    for message, count in [(0xD7, 15), (0xF7, 16)]:
        for size in range(count + 2):
            assert length(patched, table, message, size) == (count, int(size >= count))
        assert length(patched, table, message, count + 4, 4) == (count, 1)
        assert length(patched, table, message, count, -1) == (count, 0)
        assert length(patched, table, message, count, count + 1) == (count, 0)

    relay = bytes(range(16))
    assert parse(stock, original, b'\xF7' + relay) == (False, [])
    assert parse(patched, table, b'\xF7' + relay) == (True, [relay])
    for size in range(16):
        assert parse(patched, table, b'\xF7' + relay[:size]) == (False, [])
        assert parse(patched, table, b'\xF7' + relay[:size], single=False) == (False, [])
    assert parse(patched, table, b'\xF7' + relay + b'\0') == (False, [])
    ordinary = bytes(range(10))
    assert parse(patched, table, b'\xF7' + relay + b'\xEB' + ordinary,
                 single=False) == (True, [relay, ordinary])
    assert parse(patched, table, b'\xEB' + ordinary + b'\xF7' + relay,
                 single=False) == (True, [ordinary, relay])
    for message in (0xE2, 0xF8, 0xF9):
        assert parse(patched, table, bytes([message]) + relay) == (False, [])
    print('native pickup transport: original 30 IDs unchanged; D7/F7 metadata, '
          'bounds, real single/multi parsing and truncated/extra payload rejection passed')
    dispatch(data, linked)


if __name__ == '__main__':
    run(Path(sys.argv[1]).read_bytes())
