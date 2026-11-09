#!/usr/bin/env python3
"""Run patch 98 (a world up to 0xF000 bytes is hashed, the history item's six
world sections grow on the heap; ../docs/notes/client_patch.md section 3.66)
in the default build, the original beside it.

    python check_world_sections_026g.py <original survarium.exe>

Requires Python `unicorn`. The game's own code, from its entry to its `ret`,
on one game_state_history_item (0xC0BCC bytes) of each exe:

- game_state_history_item::construct (0x324510): in the build each section's
  mutable_buffer has the multithreaded allocator (0xA8001C) and a block of
  the inline capacity from it (call_realloc 0xEB590, a stub that records
  it); in the original the inline bytes, no allocator;
- the sections filled through each one's own buffer_writer (`w` 0x7AC10),
  the players' states through their buffers, as game_world_core::serialize
  leaves them: a 10v10 world of 33.3 KB within the original's inline
  buffers (rev47's crash: the original's hash() asserts, the build's
  hashes it), and 300 bullets in flight (20.7 KB of bullets) with twenty
  players (the original asserts on the bullets; the build grows them);
- game_state_history_item::hash (0x325150): its CRC-32 is zlib's of the body
  the frame's layout gives (mask, per player u16 length, state, the animated
  byte 0, six u16-prefixed sections): every byte of the world is the
  original's layout, only the room differs;
- game_state_history_item::deserialize (0x324D80) of that world into a new
  item (the 0xE8 load's own reader), and hash() of it again: the same CRC;
- game_state_history_item::destroy (0x324910): every block freed with the
  allocator (call_free 0xEB5B0, a stub), none twice, none left;
- a world over 0xF000 bytes: the build's hash() asserts as the original's
  did past 0x8000 (the limit that stays; patch 87 refuses such a world);
- a packed 0xE8 of a world over 0x8000 bytes (the server's LZMA2, liblzma,
  dictionary 0x8000) through the 0xE8 handler (process_dynamic_match_info
  0x27BFF0) from its entry, the game's own codec (0x357EB0, Lzma2Decode
  0x3EE810; its allocator a stub) and, in the build, patch 87's check, to the
  load (game_world_core::deserialize 0x2D0B70, a stub that records its
  reader): the build loads the whole world; the original's decode stops at
  its 0x8000 bytes and it loads the world cut short.

An assert is mutable_buffer::reallocate (0x7AB70) reached with no allocator
(its "mutable buffer overflow"): the run stops there.
"""
import argparse
import lzma
import struct
import sys
import zlib

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import patch_026g as base

CONSTRUCT, DESTROY, HASH, DESERIALIZE = 0x324510, 0x324910, 0x325150, 0x324D80
WRITE, REALLOCATE = 0x7AC10, 0x7AB70
CALL_REALLOC, CALL_FREE = 0xEB590, 0xEB5B0
MT_ALLOCATOR = 0xA8001C
ITEM_SIZE, PLAYER_SIZE, PLAYERS = 0xC0BCC, 0x9054, 20
ITEM_TIME, ITEM_CRC = 0xC0BB0, 0xC0BB4
# Each section: its item's offset, its inline capacity (the mutable_buffer
# right after the inline bytes, the writer 0x10 after that), in body order.
SECTIONS = ((0xB4690, 0x2400), (0xB6AB8, 0xC00), (0xBBF30, 0x2000), (0xB7F08, 0x4000), (0xB76E0, 0x800),
            (0xBFF80, 0xC00))
NAMES = ("bullets", "statistics", "objects", "physics", "rules", "effects")
ITEMS, DATA, HEAP, STACK, STOP = 0x40000000, 0x41000000, 0x42000000, 0x43000000, 0x44000000
STACK_ROOM, HEAP_ROOM = 0x100000, 0x800000
BULLET = 69                         # bytes a bullet takes: 0x2400 held about 133


class Overflow(Exception):
    """mutable_buffer::reallocate with no allocator: the game's assert."""


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((ITEMS, 0x400000), (DATA, 0x100000), (HEAP, HEAP_ROOM), (STACK, STACK_ROOM),
                         (STOP, 0x1000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        self.heap, self.blocks, self.freed, self.reallocs = HEAP, {}, [], []
        emu.hook_add(UC_HOOK_CODE, self.stub, begin=CALL_REALLOC, end=CALL_REALLOC)
        emu.hook_add(UC_HOOK_CODE, self.stub, begin=CALL_FREE, end=CALL_FREE)
        emu.hook_add(UC_HOOK_CODE, self.reallocate, begin=REALLOCATE, end=REALLOCATE)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def reallocate(self, emu, address, size, _):
        if self.read(emu.reg_read(UC_X86_REG_ESI)) == 0:
            self.overflow = (emu.reg_read(UC_X86_REG_ESI), emu.reg_read(UC_X86_REG_EDI))
            emu.emu_stop()

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 * i) for i in range(1, 7)]
        assert emu.reg_read(UC_X86_REG_ECX) == MT_ALLOCATOR, hex(emu.reg_read(UC_X86_REG_ECX))
        if address == CALL_REALLOC:          # (block, size, ...), ret 18h
            old, want = args[0], args[1]
            block = self.heap
            self.heap += (want + 0xF) & ~0xF
            assert self.heap <= HEAP + HEAP_ROOM, "the stub heap is full"
            if old:
                assert old in self.blocks, f"realloc of {old:#x}, no block"
                kept = self.blocks.pop(old)
                emu.mem_write(block, bytes(emu.mem_read(old, min(kept, want))))
            self.blocks[block] = want
            self.reallocs.append((old, want))
            result, pops = block, 0x18
        else:                                # (block, ...), ret 10h
            if args[0]:
                assert args[0] in self.blocks, f"free of {args[0]:#x}, no block (twice?)"
                self.freed.append(self.blocks.pop(args[0]))
            result, pops = 0, 0x10
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def call(self, at, regs=None, args=()):
        """Runs the function at `at` to its return; False on an overflow."""
        self.overflow = None
        sp = STACK + STACK_ROOM - 0x100
        for arg in reversed(args):
            sp -= 4
            self.word(sp, arg)
        sp -= 4
        self.word(sp, STOP)
        for reg in (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, UC_X86_REG_ESI,
                    UC_X86_REG_EDI, UC_X86_REG_EBP):
            self.emu.reg_write(reg, 0)
        for reg, value in (regs or {}).items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.emu_start(at, STOP, count=50_000_000)
        if self.overflow:
            return False
        assert self.emu.reg_read(UC_X86_REG_EIP) == STOP, hex(self.emu.reg_read(UC_X86_REG_EIP))
        return True

    def construct(self, item):
        self.emu.mem_write(item, bytes(ITEM_SIZE))
        assert self.call(CONSTRUCT, {UC_X86_REG_ESI: item}) and self.emu.reg_read(UC_X86_REG_EAX) == item

    def buffer(self, at):
        return [self.read(at + 4 * i) for i in range(4)]

    def fill(self, item, states, sections):
        """The players' states into their buffers, each section through its
        own writer in pieces of BULLET bytes; False on an overflow."""
        for slot, state in enumerate(states):
            player = item + slot * PLAYER_SIZE
            self.emu.mem_write(self.read(player + 0x1004), state)
            self.word(player + 0x100C, len(state))
        for (at, inline), section in zip(SECTIONS, sections):
            writer = item + at + inline + 0x10
            for start in range(0, len(section), BULLET):
                piece = section[start:start + BULLET]
                self.emu.mem_write(DATA, piece)
                if not self.call(WRITE, {UC_X86_REG_EDI: len(piece)}, (writer, DATA)):
                    return False
        return True

    def hash(self, item, mask):
        masks, node = DATA + 0xF000, DATA + 0xF100
        for at, value in ((masks + 8, node), (node + 4, 0), (node + 8, mask), (node + 0xC, 0)):
            self.word(at, value)
        self.word(item + ITEM_TIME, 1000)
        self.emu.mem_write(item + 0xC0BB8, b"\0")          # not hashed yet
        if not self.call(HASH, {UC_X86_REG_EAX: item}, (masks,)):
            return None
        return self.read(item + ITEM_CRC)

    def deserialize(self, item, state):
        reader = DATA + 0xF200
        self.emu.mem_write(DATA + 0x10000, state)
        self.word(reader, DATA + 0x10000)
        self.word(reader + 4, DATA + 0x10000)
        self.word(reader + 8, len(state))
        return self.call(DESERIALIZE, {}, (item, reader))


HANDLER, LOAD, EPILOGUE, CLOSE = 0x27BFF0, 0x2D0B70, 0x27C1E0, 0x27A790
CODEC_VTABLE = 0x9FAFF4                  # the LZMA2 codec (network_client+4Ch)
IMPORTS = (0x870048, 0x870108, 0x870040)  # CreateFileA, WriteFile, CloseHandle (patch 87's log)
CLIENT, GAME, FAKE = 0x45000000, 0x45400000, 0x45500000


def unpack(run, packet):
    """The 0xE8 handler on `packet` (generation, flag, stream): the state
    the load got, or what stopped it."""
    emu = run.emu
    if not getattr(run, "handler_ready", False):
        for at, size in ((CLIENT, 0x200000), (GAME, 0x100000), (FAKE, 0x10000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        vtable, stubs = FAKE + 0x100, FAKE + 0x200
        run.word(FAKE, vtable)
        for slot, stub in ((0x10, stubs), (0x18, stubs + 0x10)):
            run.word(vtable + slot, stub)
        for i, slot in enumerate(IMPORTS):
            run.word(slot, stubs + 0x20 + 0x10 * i)
        run.unpack_stubs = {stubs: ("malloc", 20), stubs + 0x10: ("free", 16), stubs + 0x20: ("create", 28),
                            stubs + 0x30: ("write", 20), stubs + 0x40: ("close", 4), CLOSE: ("leave", 4)}

        def stub(emu, address, size, _):
            what, pops = run.unpack_stubs[address]
            sp = emu.reg_read(UC_X86_REG_ESP)
            arg = run.read(sp + 4)
            result = 1
            if what == "malloc":
                result = run.heap
                run.heap += (arg + 0xF) & ~0xF
            elif what == "write":
                run.word(run.read(sp + 16), run.read(sp + 12))
            elif what == "leave":
                run.left = True
            emu.reg_write(UC_X86_REG_EAX, result)
            emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
            emu.reg_write(UC_X86_REG_EIP, run.read(sp))

        def stop(emu, address, size, _):
            run.stopped = address
            if address == LOAD:
                reader = run.read(emu.reg_read(UC_X86_REG_ESP) + 8)
                begin, cursor, length = (run.read(reader + 4 * i) for i in range(3))
                run.loaded = bytes(emu.mem_read(begin, length))
            emu.emu_stop()

        for address in run.unpack_stubs:
            emu.hook_add(UC_HOOK_CODE, stub, begin=address, end=address)
        for address in (LOAD, EPILOGUE):
            emu.hook_add(UC_HOOK_CODE, stop, begin=address, end=address)
        run.handler_ready = True
    emu.mem_write(CLIENT, bytes(0x200000))
    run.word(CLIENT + 0x1F1C40, GAME)
    run.word(CLIENT + 0x4C, CODEC_VTABLE)
    run.word(CLIENT + 0x4C + 4, FAKE)
    emu.mem_write(CLIENT + 0x4C + 0x18, bytes([6]))      # the property byte of 0x8000 (0x357DD0)
    reader = DATA + 0xF300
    emu.mem_write(DATA + 0x20000, packet)
    for i, value in enumerate((DATA + 0x20000, DATA + 0x20000, len(packet))):
        run.word(reader + 4 * i, value)
    run.stopped, run.loaded, run.left = None, None, False
    sp = STACK + STACK_ROOM - 0x100
    run.word(sp, STOP)
    for reg, value in ((UC_X86_REG_EAX, reader), (UC_X86_REG_ECX, CLIENT), (UC_X86_REG_ESP, sp),
                       (UC_X86_REG_EBX, 0), (UC_X86_REG_ESI, 0), (UC_X86_REG_EDI, 0), (UC_X86_REG_EBP, 0)):
        emu.reg_write(reg, value)
    emu.emu_start(HANDLER, STOP, count=200_000_000)
    return run.loaded, run.stopped, run.left


def body(states, sections):
    out = struct.pack("<I", (1 << len(states)) - 1)
    for state in states:
        out += struct.pack("<H", len(state)) + state + b"\0"
    for section in sections:
        out += struct.pack("<H", len(section)) + section
    return out


def worlds():
    """(what, states, sections) of each world, the largest last."""
    state = lambda slot: bytes((slot * 7 + i) & 0xFF for i in range(0x3A8))
    pattern = lambda n, seed: bytes((seed + i * 13 + i // 251) & 0xFF for i in range(n))
    states = [state(slot) for slot in range(PLAYERS)]
    real = [pattern(n, k) for k, n in enumerate((0x230, 0x45C, 0x3EC, 0, 0x76B, 0x57))]
    # The bullets' inline buffer full (93 pellets and more) and more game
    # objects than seen: 0x818A bytes, over the original's 0x8000.
    rev47 = [pattern(0x2400, 1), real[1], pattern(0x800, 6)] + real[3:]
    many = [pattern(300 * BULLET, 2)] + real[1:]
    over = [pattern(0xF000 - 0x3A8 * 20 - 0x1000, 3), pattern(0x800, 4), pattern(0x2000, 5)] + real[3:]
    return (("a 10v10 world within the original's inline buffers", states, rev47),
            ("300 bullets in flight and twenty players", states, many),
            ("a world over 0xF000 bytes", states, over))


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("exe")
    args = parser.parse_args()
    original = open(args.exe, "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, True)
    stock, ours = Run(original), Run(bytes(built))
    failed = []

    def check(ok, what):
        print(f"  {'ok' if ok else 'FAIL'}  {what}")
        if not ok:
            failed.append(what)

    item, copy = ITEMS, ITEMS + 0x100000
    # The constructor.
    stock.construct(item)
    ours.construct(item)
    inline = [stock.buffer(item + at + cap) for at, cap in SECTIONS]
    check(all(found == [0, item + at, cap, 0] for found, (at, cap) in zip(inline, SECTIONS)),
          "the original's sections: {no allocator, the inline bytes, " +
          ", ".join(f"{cap:#x}" for _, cap in SECTIONS) + ", 0}")
    heap = [ours.buffer(item + at + cap) for at, cap in SECTIONS]
    check(all(alloc == MT_ALLOCATOR and data in ours.blocks and ours.blocks[data] == cap == capacity and size == 0
              for (alloc, data, capacity, size), (_, cap) in zip(heap, SECTIONS))
          and ours.reallocs == [(0, cap) for _, cap in SECTIONS],
          "the build's: the multithreaded allocator and a block of the same capacity each, from realloc(0, capacity)")
    players = [stock.buffer(item + slot * PLAYER_SIZE + 0x1000) for slot in range(PLAYERS)]
    check(players == [ours.buffer(item + slot * PLAYER_SIZE + 0x1000) for slot in range(PLAYERS)]
          and all(p[0] == 0 and p[2] == 0x1000 for p in players),
          "the players' state buffers the original's (inline 0x1000, no allocator) in both")
    rest = lambda run: bytes(run.emu.mem_read(item + 0xC0BA8, ITEM_SIZE - 0xC0BA8))
    check(rest(stock) == rest(ours), "the item's fields after the sections the original's")
    stock.destroy = ours.destroy = None
    for run in (stock, ours):
        assert run.call(DESTROY, {}, (item,))
    check(not ours.blocks and sorted(ours.freed) == sorted(cap for _, cap in SECTIONS) and not stock.freed,
          "destroy frees each block once with the allocator (the original frees nothing)")

    for what, states, sections in worlds():
        expected = body(states, sections)
        big = len(expected) > base.WORLD_BUFFER_SIZE
        print(f"{what}: {len(expected):#x} bytes, bullets {len(sections[0]):#x}")
        for run in (stock, ours):
            run.construct(item)
        filled_stock, filled_ours = stock.fill(item, states, sections), ours.fill(item, states, sections)
        within = all(len(s) <= cap for s, (_, cap) in zip(sections, SECTIONS))
        check(filled_stock == within, "the original holds the sections" if within else
              f"the original asserts filling them ({NAMES[[len(s) > c for s, (_, c) in zip(sections, SECTIONS)].index(True)]})")
        check(filled_ours, "the build holds them: " + ", ".join(
            f"{name} {ours.buffer(item + at + cap)[2]:#x}" for name, (at, cap) in zip(NAMES, SECTIONS)))
        mask = (1 << PLAYERS) - 1
        if filled_stock:
            crc = stock.hash(item, mask)
            check(crc is None and len(expected) > 0x8000, "the original's hash() asserts: over its 0x8000 bytes")
        crc = ours.hash(item, mask)
        if big:
            check(crc is None and ours.overflow[1] > base.WORLD_BUFFER_SIZE,
                  f"the build's hash() asserts past its {base.WORLD_BUFFER_SIZE:#x} bytes (the limit that stays)")
        else:
            check(crc == zlib.crc32(expected), f"the build's hash() is the CRC-32 of the body: {crc:#010x}")
            # The 0xE8 load of that world into a new item, and its hash.
            ours.construct(copy)
            loaded = ours.deserialize(copy, struct.pack("<I", 1000) + expected)
            check(loaded and ours.hash(copy, mask) == crc and all(
                bytes(ours.emu.mem_read(ours.buffer(copy + at + cap)[1], len(s))) == s
                for s, (at, cap) in zip(sections, SECTIONS)),
                "deserialize loads it into a new item (its sections grown), whose hash() is the same")
            ours.call(DESTROY, {}, (copy,))
        for run in (stock, ours):
            run.call(DESTROY, {}, (item,))
        check(not ours.blocks, "destroy frees every block, none left")
    # A packed 0xE8 of a world past the original's 0x8000 bytes: no players
    # (patch 87's check and the pickup's preparation have nothing to cook),
    # the bullets of some 600 pellets.
    world = struct.pack("<I", 1000) + body([], [bytes((i * 7 + i // 300) & 0xFF for i in range(0xA000)),
                                                 b"", b"", b"", b"", b""])
    stream = lzma.compress(world, format=lzma.FORMAT_RAW,
                           filters=[{"id": lzma.FILTER_LZMA2, "dict_size": 0x8000, "preset": 3}])
    packet = bytes([1, 1]) + stream
    print(f"a packed 0xE8 of a world of {len(world):#x} bytes ({len(stream)} bytes packed)")
    loaded, stopped, left = unpack(ours, packet)
    check(stopped == LOAD and loaded == world and not left,
          "the build unpacks it whole and loads it")
    loaded, stopped, left = unpack(stock, packet)
    check(stopped == LOAD and loaded is not None and len(loaded) == 0x8000 and loaded == world[:0x8000],
          f"the original's decode stops at its 0x8000 bytes: it loads {len(loaded or b''):#x} bytes of the world")
    print("all cases passed" if not failed else f"{len(failed)} FAILED")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
