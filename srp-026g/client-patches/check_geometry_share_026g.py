#!/usr/bin/env python3
"""Patch 164 (the Rust patch geometry_share, rust/patches/src/geometry_share.rs):
a pooled vertex or index buffer with the same bytes as a live one takes that
one's range of the pool (client_patch.md section 3.88).

    python check_geometry_share_026g.py <original survarium.exe>

Requires Python `unicorn`. The original exe and the build, each with its
sections and arena as they are, run from three places of the game's own code
over a made-up pool: one chunk of 4 MiB, its taken ranges' vector in the
chunk's own room (the game's layout: fields_026g.tsv hw_buffer_pool_chunk).
Stubbed: GetCurrentThreadId (one thread) and the chunk's D3D11 buffer's
AddRef (the stop after the constructor's site).

- untyped_buffer::untyped_buffer from its pooled path (0x3D1178, the buffer's
  range taken) to the buffer's AddRef: a range of its own, kept by the build;
- hw_buffer_pool::allocate (0x3D43E0) from its call: the build answers bytes a
  range already holds with that range (1, the stack as the function leaves
  it, the chunk's vector as it was); other bytes, a buffer under 8 KiB, the
  original: the game's own allocate, which finds no room in the chunk this
  check gives it (one taken range over all of it);
- hw_buffer_pool::deallocate (0x3D4510) from its call: the build keeps a range
  while another buffer holds it (returns at once, the vector as it was), the
  last one's goes through the game's code, which takes it out of the vector.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_ESI, \
    UC_X86_REG_ESP

import patch_026g as base

POOLED, ADDREF_CALL, ALLOCATE, DEALLOCATE = 0x3D1178, 0x3D1189, 0x3D43E0, 0x3D4510
THREAD_ID = 0x870118
STACK, OBJECTS, STUBS, RETURN = 0x20000000, 0x22000000, 0x23000000, 0x25000000
POOL, CHUNK, BUFFER, FRAME, DATA, OUT, D3D, D3D_VT = (OBJECTS + 0x10000 * i for i in range(8))
CHUNK_BYTES, CHUNK_SIZE, ROOM = 0x400000, 0xC028, 0xC


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((STACK, 0x40000), (OBJECTS, 0x80000), (STUBS, 0x1000), (RETURN, 0x1000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        emu.mem_write(STUBS, bytes.fromhex("b801000000c3"))      # GetCurrentThreadId: 1
        self.word(THREAD_ID, STUBS)
        # the pool: one chunk; the chunk: its buffer, its pool, its bytes
        self.word(POOL, POOL + 0xC)
        self.word(POOL + 4, POOL + 0x10)
        self.word(POOL + 0xC, CHUNK)
        self.word(CHUNK + 0xC00C, D3D)
        self.word(CHUNK + 0xC010, POOL)
        self.word(CHUNK + 0xC020, CHUNK_BYTES)
        self.word(D3D, D3D_VT)
        self.ranges([])

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def ranges(self, ranges):
        """The chunk's taken ranges, its vector in its own room."""
        begin = CHUNK + ROOM
        for i, (first, last) in enumerate(ranges):
            self.emu.mem_write(begin + 12 * i, struct.pack("<3I", CHUNK, first, last))
        self.word(CHUNK, begin)
        self.word(CHUNK + 4, begin + 12 * len(ranges))
        self.word(CHUNK + 8, begin + 0xC000)

    def taken(self):
        begin, end = self.read(CHUNK), self.read(CHUNK + 4)
        return [struct.unpack("<3I", self.emu.mem_read(at, 12))[1:] for at in range(begin, end, 12)]

    def run(self, start, until, esp):
        self.emu.ctl_remove_cache(until - 0x40, until + 0x10)
        self.emu.reg_write(UC_X86_REG_ESP, esp)
        self.emu.emu_start(start, until, count=2_000_000)
        assert self.emu.reg_read(UC_X86_REG_EIP) == until, hex(self.emu.reg_read(UC_X86_REG_EIP))

    def call(self, function, args, eax=0):
        sp = STACK + 0x30000 - 4 * (len(args) + 1)
        self.emu.mem_write(sp, struct.pack(f"<{len(args) + 1}I", RETURN, *args))
        self.emu.reg_write(UC_X86_REG_EAX, eax)
        self.run(function, RETURN, sp)
        assert self.emu.reg_read(UC_X86_REG_ESP) == sp + 4 + 4 * len(args), "the function's own cleanup"
        return self.emu.reg_read(UC_X86_REG_EAX)

    def pooled(self, first, data):
        """A buffer made with `data`, given the range at `first` (the constructor's pooled path)."""
        self.emu.mem_write(DATA, data)
        self.emu.mem_write(BUFFER, struct.pack("<8I", 0, CHUNK, first, first + len(data) - 1, 0, len(data), 48, 0))
        self.word(FRAME + 0x10, DATA)
        self.emu.reg_write(UC_X86_REG_EDI, BUFFER)
        self.emu.reg_write(UC_X86_REG_ESI, BUFFER + 4)
        self.emu.reg_write(UC_X86_REG_EBP, FRAME)
        self.run(POOLED, ADDREF_CALL, STACK + 0x30000)
        ranges = self.taken() + [(first, first + len(data) - 1)]
        self.ranges(ranges)

    def allocate(self, data):
        """allocate(pool, out, data, bytes, stride): AL, the range written."""
        self.emu.mem_write(DATA, data)
        self.emu.mem_write(OUT, bytes(12))
        al = self.call(ALLOCATE, [POOL, OUT, DATA, len(data), 48]) & 0xFF
        return al, struct.unpack("<3I", self.emu.mem_read(OUT, 12))

    def deallocate(self, first, last):
        self.emu.mem_write(OUT, struct.pack("<3I", CHUNK, first, last))
        self.call(DEALLOCATE, [], eax=OUT)


def full(run):
    """The whole chunk taken: the game's allocate finds no room."""
    run.ranges([(0, CHUNK_BYTES - 1)])


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    built = bytes(built)
    mesh = bytes((i * 7 + 3) & 0xFF for i in range(48 * 400))       # 19 200 bytes, a part's vertices
    other = bytes((i * 11 + 5) & 0xFF for i in range(48 * 400))
    small = bytes((i * 13) & 0xFF for i in range(48 * 100))           # 4 800 bytes, under 8 KiB

    for name, data in (("original", original), ("build", built)):
        r = Run(data)
        r.pooled(0, mesh)
        r.pooled(0x10000, small)
        r.pooled(0x20000, other)
        before = r.taken()
        if name == "build":
            assert r.allocate(mesh) == (1, (CHUNK, 0, len(mesh) - 1)), "the same bytes: the range they are in"
            assert r.allocate(mesh) == (1, (CHUNK, 0, len(mesh) - 1))
            assert r.taken() == before, "the pool not asked"
            assert r.allocate(other)[1] == (CHUNK, 0x20000, 0x20000 + len(other) - 1)
            # bytes no range holds, and a small buffer: the game's allocate, a full chunk
            saved = r.taken()
            full(r)
            assert r.allocate(mesh[:-48] + b"\1" * 48)[0] == 0
            assert r.allocate(small)[0] == 0
            r.ranges(saved)
            # three buffers hold the mesh's range, two the other's
            r.deallocate(0, len(mesh) - 1)
            r.deallocate(0, len(mesh) - 1)
            assert r.taken() == before, "kept while another buffer holds it"
            r.deallocate(0, len(mesh) - 1)
            assert (0, len(mesh) - 1) not in r.taken(), "the last holder's: the game takes it back"
            r.deallocate(0x10000, 0x10000 + len(small) - 1)
            assert (0x10000, 0x10000 + len(small) - 1) not in r.taken(), "a small one: the game's own"
            # the range given back is no longer given
            full(r)
            assert r.allocate(mesh)[0] == 0
            print("ok build: the same bytes get the live range (1, the chunk untouched), other or small bytes "
                  "the game's allocate; a range stays while another buffer holds it, the last one's goes")
        else:
            full(r)
            assert r.allocate(mesh)[0] == 0, "the original asks its chunks: none has room"
            r.ranges(before)
            r.deallocate(0, len(mesh) - 1)
            assert (0, len(mesh) - 1) not in r.taken()
            print("ok original: allocate asks the chunks for the same bytes again; deallocate takes the range out")
    print("PASS: geometry_share (patch 164)")


if __name__ == "__main__":
    main()
