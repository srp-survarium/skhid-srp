#!/usr/bin/env python3
"""Run patch 60 (the saved world's room as 0.28's: WORLD_SIZES and
world_buffers_026g.asm) in the default build and the same cases in the
original exe, with the game's own code.

    python check_world_buffers_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are (the
arena with them in the build). The game's malloc and free (0xF7650,
0xF76B0) are stubs over a heap of their own that records every block, and
the assert (0x113450) stops the run: a stream or buffer that would grow past
its room is an assert, as in the game. Everything else is the game's code.

- A world: game_state_history_item's constructor (0x324510) on a made-up
  item; its game objects' two streams (item+0BBF30h) must have 0x4000 bytes
  each from malloc in the build, 0x2000 in place in the original.
- Its streams filled through the game's buffer_writer::w (0x7AC10): 0x3000
  bytes of game objects in each of the two streams (more than the original's
  0x2000: an assert there), and the item's other four sections, so that the
  item's serialize (0x324BE0) gives a world of more than 0x8000 bytes; 0x4000
  more in one stream is an assert in the build too (the room is 0x4000, no
  growth).
- hash() (0x325150): the build's gives the world's CRC-32 (the zlib one);
  the original's asserts on a world over 0x8000.
- The upload: the build's own upload code (the section's first entry,
  cave_ready, as the ready hook runs it) sends the world as 0xE0 chunks
  through a made-up match client: the chunks parse as the server's
  upload.rs does and put together give the world, `ceil(n / 480)` of them.
- The receive: process_dynamic_match_info (0x27BFF0) takes a 0xE8 with the
  world and its time packed with LZMA2 (dictionary 0x8000, as the server's),
  unpacks it with the game's own lzma2_compressor (vtable 0x9FAFF4,
  Lzma2Decode 0x3EE810) into its buffer and hands it to the world's
  deserialize (0x2D0B70, a stub that records its reader): the whole state in
  the build, the first 0x8000 bytes in the original.
- The world read back: a second item's deserialize (0x324D80) from the same
  state, then its serialize: the same bytes.
- The worlds taken down: the destructor (0x324910) of each, a remake in place
  (destructor then constructor on the same item, as the rollback's 0x3252A7)
  and a world whose malloc fails (each stream as the game made it): every
  block given back once, nothing freed twice or unknown, nothing left.
"""
import lzma
import random
import struct
import sys
import zlib

from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import patch_026g as base

ITEM_CONSTRUCT, ITEM_DESTROY = 0x324510, 0x324910     # esi = item; stdcall(item), ret 4
ITEM_SERIALIZE, ITEM_DESERIALIZE = 0x324BE0, 0x324D80  # stdcall(item, writer, mask) ret 0Ch; (item, reader) ret 8
HASH = 0x325150                                         # eax = item, push the masks; ret 4
WRITE = 0x7AC10                                         # push src, push writer, edi = n; ret 8
MALLOC, FREE, ASSERT = 0xF7650, 0xF76B0, 0x113450
DYNAMIC_INFO, DESERIALIZE = 0x27BFF0, 0x2D0B70          # eax = the reader, ecx = the client; the world's
READY_BACK = 0x27AEA7                                   # the ready hook's way back
COMPRESSOR_VTABLE, DICTIONARY, PROPERTY = 0x9FAFF4, 0x8000, 6
ITEM_SIZE = 0xC0BD0
OBJECTS = 0xBBF30                                       # the game objects' state: two streams
SECOND = 0x2028
ITEM_TIME = 0xC0BB0
# Each section's writer (its mutable_buffer is the writer's +14h), the room the
# original gives it, and how much is written to it here.
SECTIONS = (
    (0xB4690 + 0x2410, 0x2400, 0x2400),
    (0xB6AB8 + 0xC10, 0xC00, 0xC00),
    (OBJECTS + 0x2010, 0x2000, 0x3000),                 # the game objects: over the original's room
    (0xB7F08 + 0x4010, 0x4000, 0x4000),
    (0xB76E0 + 0x810, 0x800, 0x800),
    (0xBFF80 + 0xC10, 0xC00, 0xC00),
    (OBJECTS + SECOND + 0x2010, 0x2000, 0x3000),        # the second stream: kept, never serialized
)

HEAP, STACK, OBJ, STUBS = 0x30000000, 0x31000000, 0x32000000, 0x35000000
STOP = STUBS                                            # every call returns here
NEW_PACKET, ENQUEUE, ALLOC, ALLOC_FREE = STUBS + 0x10, STUBS + 0x20, STUBS + 0x30, STUBS + 0x40
REGS = (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP)


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(HEAP, 0x1000000, UC_PROT_ALL)
        emu.mem_map(STACK, 0x200000, UC_PROT_ALL)
        emu.mem_map(OBJ, 0x2000000, UC_PROT_ALL)
        emu.mem_map(STUBS, 0x1000, UC_PROT_ALL)
        emu.mem_write(STUBS, b"\xC3" * 0x1000)
        self.next_block, self.blocks, self.freed, self.errors = HEAP, {}, [], []
        self.fail_malloc = False
        self.mallocs = []
        self.packets, self.chunks, self.read_back = [], [], None
        self.next_obj = OBJ
        for at in (MALLOC, FREE, ASSERT, DESERIALIZE, NEW_PACKET, ENQUEUE, ALLOC, ALLOC_FREE):
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=at, end=at)

    # memory
    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def bytes_at(self, at, size):
        return bytes(self.emu.mem_read(at, size))

    def obj(self, size, align=0x1000):
        at = (self.next_obj + align - 1) & ~(align - 1)
        self.next_obj = at + size
        self.emu.mem_write(at, bytes(size))
        return at

    def give(self, size):
        at = (self.next_block + 15) & ~15
        self.next_block = at + size
        self.blocks[at] = size
        return at

    # stubs
    def back(self, pops, value=None):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        if value is not None:
            self.emu.reg_write(UC_X86_REG_EAX, value)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        arg = lambda i: self.read(sp + 4 * i)
        if address == MALLOC:                       # cdecl(size)
            self.mallocs.append(arg(1))
            self.back(0, 0 if self.fail_malloc else self.give(arg(1)))
        elif address == FREE:                       # cdecl(block)
            block = arg(1)
            if block:
                if block in self.freed and block not in self.blocks:
                    self.errors.append(f"{block:#x} freed twice")
                elif block not in self.blocks:
                    self.errors.append(f"{block:#x} freed, never given")
                else:
                    del self.blocks[block]
                    self.freed.append(block)
            self.back(0)
        elif address == ASSERT:
            self.asserted = True
            emu.emu_stop()
        elif address == DESERIALIZE:                # stdcall(core, &reader, 1, 8): the reader's bytes
            reader = arg(2)
            self.read_back = self.bytes_at(self.read(reader), self.read(reader + 8))
            emu.emu_stop()
        elif address == NEW_PACKET:                 # thiscall(id), ret 4: a packet as the ctor 0x278D00 leaves it
            packet = self.obj(0x2C0, 16)
            emu.mem_write(packet + 0x98, b"\x60")
            for offset, value in ((0x298, 0), (0x29C, packet + 0x99), (0x2A0, 0x1FC), (0x2A4, 3), (0x2BC, packet + 0x298)):
                self.word(packet + offset, value)
            emu.mem_write(packet + 0x2B8, b"\xFF")
            self.packets.append((arg(1), packet))
            self.back(4, packet)
        elif address == ENQUEUE:                    # thiscall(packet), ret 4
            packet = arg(1)
            self.chunks.append((self.bytes_at(packet + 0x98, 1)[0], self.bytes_at(packet + 0x99 + 3, self.read(packet + 0x2A4) - 3)))
            self.back(4)
        elif address == ALLOC:                      # the decoder's allocator (0x357D70): thiscall(size, 4 more), ret 14h
            self.back(20, self.give(arg(1)))
        elif address == ALLOC_FREE:                 # (0x357DA0): thiscall(block, 3 more), ret 10h
            self.blocks.pop(arg(1), None)
            self.back(16)

    def call(self, va, args=(), regs=None, pops=None, count=50_000_000):
        """Calls va with the stack arguments and registers; returns
        (registers, what it popped): None when it asserted."""
        sp = STACK + 0x1F0000 - 4 * len(args) - 4
        for i, value in enumerate((STOP,) + tuple(args)):
            self.word(sp + 4 * i, value)
        for reg in REGS:
            self.emu.reg_write(reg, 0)
        for reg, value in (regs or {}).items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.asserted = False
        self.emu.emu_start(va, STOP, count=count)
        if self.asserted:
            return None
        if self.emu.reg_read(UC_X86_REG_EIP) != STOP:
            raise RuntimeError(f"{va:#x} did not return (eip {self.emu.reg_read(UC_X86_REG_EIP):#x})")
        return {reg: self.emu.reg_read(reg) for reg in REGS}, self.emu.reg_read(UC_X86_REG_ESP) - sp - 4

    # the game's objects
    def item(self):
        item = self.obj(ITEM_SIZE, 0x1000)
        out = self.call(ITEM_CONSTRUCT, regs={UC_X86_REG_ESI: item})
        assert out is not None and out[1] == 0 and out[0][UC_X86_REG_EAX] == item
        self.word(item + ITEM_TIME, 1000)
        return item

    def streams(self, item):
        """[(data, capacity, size)] of the game objects' two streams."""
        out = []
        for stream in (item + OBJECTS, item + OBJECTS + SECOND):
            out.append(tuple(self.read(stream + 0x2000 + offset) for offset in (4, 8, 12)))
        return out

    def write(self, writer, data):
        """buffer_writer::w of data; False when it asserted."""
        src = self.obj(len(data), 16)
        self.emu.mem_write(src, data)
        out = self.call(WRITE, (writer, src), {UC_X86_REG_EDI: len(data)})
        return out is not None and out[1] == 8

    def serialize(self, item):
        """The item's serialize, into a writer of 0x20000 bytes: its bytes."""
        buffer, mb, writer = self.obj(0x20000, 16), self.obj(16, 16), self.obj(0x18, 16)
        for offset, value in ((0, 0), (4, buffer), (8, 0x20000), (12, 0)):
            self.word(mb + offset, value)
        self.emu.mem_write(writer + 0x10, b"\xFF")
        self.word(writer + 0x14, mb)
        out = self.call(ITEM_SERIALIZE, (item, writer, 0))
        assert out is not None and out[1] == 12
        return self.bytes_at(buffer, self.read(mb + 12))

    def masks(self):
        masks, node = self.obj(16, 16), self.obj(16, 16)
        self.word(masks + 8, node)                  # one node: next 0, mask 0, time 0
        return masks

    def hash(self, item):
        out = self.call(HASH, (self.masks(),), {UC_X86_REG_EAX: item})
        return None if out is None else out[0][UC_X86_REG_EAX]

    def upload(self, item):
        """The build's upload code from its first entry (cave_ready), ebx = a
        made-up network client whose newest world is item."""
        nc, pvp, mc, pool, vtable = self.obj(0x200000), self.obj(0x20000), self.obj(0x60000), self.obj(64), self.obj(64)
        self.word(nc + 0x1F1C40, pvp)
        self.word(nc + 0x1F1C48, mc)
        self.word(pvp + 0x1F8 + 0x2FC0, item)
        self.word(pvp + 0x1F8 + 0x12280 + 8, self.read(self.masks() + 8))
        self.word(mc, vtable)
        self.word(vtable + 0x18, NEW_PACKET)
        self.word(vtable + 0x1C, ENQUEUE)
        self.word(mc + 0x578B0, pool)
        self.word(pool + 0x34, 256)
        self.chunks, self.packets = [], []
        sp = STACK + 0x1F0000
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.reg_write(UC_X86_REG_EBX, nc)
        self.asserted = False
        self.emu.emu_start(base.SECTION_VA, READY_BACK, count=50_000_000)
        ok = not self.asserted and self.emu.reg_read(UC_X86_REG_EIP) == READY_BACK \
            and self.emu.reg_read(UC_X86_REG_ESP) == sp and self.emu.reg_read(UC_X86_REG_EBX) == mc
        return ok, list(self.chunks), list(self.packets)

    def receive(self, state):
        """process_dynamic_match_info on a 0xE8 of state packed with LZMA2:
        the bytes the world's deserialize is handed."""
        nc, allocator, vtable = self.obj(0x200000), self.obj(64), self.obj(64)
        packed = lzma.compress(state, format=lzma.FORMAT_RAW,
                               filters=[{"id": lzma.FILTER_LZMA2, "dict_size": DICTIONARY}])
        message = bytes([5, 1]) + packed
        data, reader = self.obj(len(message), 16), self.obj(16, 16)
        self.emu.mem_write(data, message)
        for offset, value in ((0, data), (4, data), (8, len(message))):
            self.word(reader + offset, value)
        self.emu.mem_write(nc + 0x1F1C68, b"\x05")                 # the generation the client has
        self.word(nc + 0x1F1C40, self.obj(0x200))
        decoder = nc + 0x4C                                         # lzma2_compressor, as its ctor 0x357DD0 makes it
        self.word(decoder, COMPRESSOR_VTABLE)
        self.word(decoder + 4, allocator)
        self.word(decoder + 8, DICTIONARY)
        self.emu.mem_write(decoder + 0x18, bytes([PROPERTY]))
        self.word(allocator, vtable)
        self.word(vtable + 0x10, ALLOC)
        self.word(vtable + 0x18, ALLOC_FREE)
        self.read_back = None
        try:
            self.call(DYNAMIC_INFO, (), {UC_X86_REG_EAX: reader, UC_X86_REG_ECX: nc})
        except RuntimeError:
            pass
        return self.read_back, len(packed)

    def deserialize(self, item, state):
        data, reader = self.obj(len(state), 16), self.obj(16, 16)
        self.emu.mem_write(data, state)
        for offset, value in ((0, data), (4, data), (8, len(state))):
            self.word(reader + offset, value)
        out = self.call(ITEM_DESERIALIZE, (item, reader))
        return out is not None and out[1] == 8

    def destroy(self, item):
        out = self.call(ITEM_DESTROY, (item,))
        return out is not None and out[1] == 4


def chunks_of(payloads):
    """The server's Chunk::parse and Reassembler (upload.rs): the body, or why not."""
    parts, total, time = {}, None, None
    for payload in payloads:
        t, offset, n = struct.unpack_from("<IHH", payload)
        if total not in (None, n) or time not in (None, t) or offset % 480 or offset >= n \
                or len(payload) != 8 + min(480, n - offset) or n > 0xFF00:
            return None, f"a chunk {t}, {offset:#x} of {n:#x} with {len(payload) - 8} bytes"
        total, time = n, t
        parts[offset] = payload[8:]
    body = b"".join(parts[offset] for offset in sorted(parts))
    return (body, time) if len(body) == total else (None, f"{len(body):#x} of {total:#x} bytes")


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    linked = base.arena()
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    stock, patched = Run(original), Run(bytes(built))
    print("the build's sites")
    for va, old, new in base.WORLD_SIZES:
        report(stock.bytes_at(va, len(old)) == old and patched.bytes_at(va, len(new)) == new,
               f"{va:#x}: {old.hex()} -> {new.hex()}")
    for va, old, symbol, opcode in base.WORLD_BUFFERS_HOOKS:
        target = linked.symbols[symbol]
        report(stock.bytes_at(va, len(old)) == old
               and patched.bytes_at(va, 5) == bytes([opcode]) + struct.pack("<i", target - va - 5),
               f"{va:#x}: `call {target:#x}` ({symbol})")

    print("a world: the item's constructor")
    a, b = stock.item(), patched.item()
    report(stock.streams(a) == [(a + OBJECTS, 0x2000, 0), (a + OBJECTS + SECOND, 0x2000, 0)] and not stock.mallocs,
           "the original: each stream 0x2000 bytes in place, no malloc")
    got = patched.streams(b)
    report(patched.mallocs == [0x4000, 0x4000] and [c for _, c, _ in got] == [0x4000, 0x4000]
           and all(patched.blocks.get(d) == 0x4000 for d, _, _ in got) and got[0][0] != got[1][0],
           f"the build: each stream 0x4000 bytes from malloc ({', '.join(f'{d:#x}' for d, _, _ in got)})")

    print("its sections filled through buffer_writer::w")
    # Half noise, half a pattern: LZMA2 packs some and stores some.
    payloads = [random.Random(i).randbytes(n // 2) + bytes((i * 31 + j * 7 + j // 251) & 0xFF for j in range(n - n // 2))
                for i, (_, _, n) in enumerate(SECTIONS)]
    stock_ok = [stock.write(a + w, p) for (w, _, _), p in zip(SECTIONS, payloads)]
    report(stock_ok == [n <= room for (_, room, n) in SECTIONS],
           "the original: an assert at each game objects' stream given 0x3000 bytes, the rest written")
    patched_ok = [patched.write(b + w, p) for (w, _, _), p in zip(SECTIONS, payloads)]
    got = patched.streams(b)
    report(all(patched_ok) and [s for _, _, s in got] == [0x3000, 0x3000]
           and patched.bytes_at(got[0][0], 0x3000) == payloads[2] and patched.bytes_at(got[1][0], 0x3000) == payloads[6]
           and patched.bytes_at(b + OBJECTS, 0x2000) == bytes(0x2000)
           and patched.bytes_at(b + OBJECTS + SECOND, 0x2000) == bytes(0x2000),
           "the build: 0x3000 bytes in each game objects' stream, in its block (the bytes in place untouched)")
    report(not patched.write(b + OBJECTS + 0x2010, bytes(0x1001)), "the build: 0x1001 more (0x4001 in all) is an assert")
    report(patched.streams(b)[0][2] == 0x3000, "and leaves the stream as it was")

    print("serialize and hash")
    body = patched.serialize(b)
    report(len(body) > 0x8000 and payloads[2] in body and payloads[6] not in body,
           f"the build's serialize: {len(body):#x} bytes, with the game objects (0x3000) and not the second stream")
    crc = patched.hash(b)
    report(crc == zlib.crc32(body), f"the build's hash(): {crc:#010x}, the CRC-32 of those bytes")
    stock_body = stock.serialize(a)
    report(len(stock_body) > 0x8000 and stock.hash(a) is None,
           f"the original's hash() of its world of {len(stock_body):#x} bytes asserts (0x8000)")

    print("the upload (cave_ready, the build's own code)")
    ok, chunks, packets = patched.upload(b)
    put, time = chunks_of([p for _, p in chunks])
    report(ok and put == body and time == 1000 and len(chunks) == -(-len(body) // 480)
           and all(flags == 0x20 for flags, _ in chunks) and all(i == 0xE0 for i, _ in packets),
           f"{len(chunks)} chunks of 0xE0, each alone (flags 20h), put together as the server does: the world")

    print("the receive (process_dynamic_match_info, the game's LZMA2)")
    state = struct.pack("<I", 1000) + body
    got, packed = patched.receive(state)
    report(got == state, f"the build: {len(state):#x} bytes packed to {packed:#x}, handed to the world whole")
    got, _ = stock.receive(state)
    report(got is not None and got == state[:0x8000], "the original: the first 0x8000 bytes only")

    print("the world read back")
    c = patched.item()
    report(patched.deserialize(c, state) and patched.serialize(c) == body
           and patched.streams(c)[0][1:] == (0x4000, 0x3000),
           "a second world's deserialize of that state, then its serialize: the same bytes")
    d = stock.item()
    report(not stock.deserialize(d, state), "the original's deserialize of it asserts")

    print("the worlds taken down")
    for item in (b, c):
        report(patched.destroy(item) and patched.streams(item) == [(item + OBJECTS, 0x2000, 0x3000), (item + OBJECTS + SECOND, 0x2000, 0x3000 if item == b else 0)],
               f"{item:#x}: its destructor; each stream its bytes in place again")
    remake = patched.item()
    report(patched.destroy(remake), "a remake in place: the destructor")
    out = patched.call(ITEM_CONSTRUCT, regs={UC_X86_REG_ESI: remake})
    second = [d for d, _, _ in patched.streams(remake)]
    report(out is not None and all(s in patched.blocks for s in second) and patched.destroy(remake),
           "then the constructor on the same item: new blocks, given back by its destructor")
    patched.fail_malloc = True
    e = patched.item()
    report(patched.streams(e) == [(e + OBJECTS, 0x2000, 0), (e + OBJECTS + SECOND, 0x2000, 0)] and patched.destroy(e),
           "a world whose malloc fails: each stream as the game made it (0x2000 in place), nothing freed")
    stream_blocks = [k for k, size in patched.blocks.items() if size == 0x4000]
    report(not patched.errors and not stream_blocks and len(patched.freed) == 8 and len(set(patched.freed)) == 8,
           f"every stream block given back once ({len(patched.freed)} frees), none twice or unknown, none left"
           + "".join(f"; {e}" for e in patched.errors))

    print("OK" if not failed else f"{failed} FAILED")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
