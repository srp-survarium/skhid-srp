#!/usr/bin/env python3
"""Run patch 134 (the Rust patch `constant_upload`, constant_upload_026g.asm)
beside the original on made-up constant buffers.

    python check_constant_upload_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
original's and the default build's, and each runs the game's own
shader_constant_buffer code:

- set_memory (0x11B5A0), 4000 seeded cases: buffers of 16 to 4096 bytes
  and odd sizes, offsets inside and past the end, counts clamped or not,
  sources equal to the shadow, differing at one byte (first, inside, last)
  or everywhere; every register given made-up values (XMM0..7 too). Both
  must leave the same EAX, ECX, EDX, EBX, ESI, EDI, EBP, ESP, XMM0..7, the
  same buffer object and shadow (its whole block), and return to the caller.
  The instructions each runs are counted.
- the constructor (0x3DDE90) for sizes 16 to 4096, with a model of
  malloc_impl (0xE4F90) and of the device's CreateBuffer: the original asks
  `size` bytes, the build 2 x size + 16, all of them zeros; the object
  (0x68 bytes) the same in both but for its shadow's address.
- a run of frames: buffers made by each exe's constructor, draws that set
  constants (most of them the same as before, a world matrix and a bone
  palette new each draw, sometimes a byte of an unchanged one) and update
  every buffer (0x3DDFD0, as update_buffers 0x3D63F0 does), a byte now and
  then written into a shadow with m_changed set (as the 67 inlined copies of
  set_memory do), the device context a model: Map (WRITE_DISCARD) hands a
  new slice, Unmap makes it the buffer's contents on the GPU. After every update the GPU's contents of
  every buffer must be the original's and m_changed as the original leaves
  it; the build must map less.

Prints the instructions per set_memory by size and the Maps saved.
"""
import os
import random
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI,
                               UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP, UC_X86_REG_XMM0,
                               UC_X86_REG_XMM1, UC_X86_REG_XMM2, UC_X86_REG_XMM3, UC_X86_REG_XMM4,
                               UC_X86_REG_XMM5, UC_X86_REG_XMM6, UC_X86_REG_XMM7)

if len(sys.argv) > 1:
    os.environ.setdefault("SRP_026G_EXE", sys.argv[1])
import patch_026g as base

SET_MEMORY, CTOR, UPDATE = 0x11B5A0, 0x3DDE90, 0x3DDFD0
MALLOC_IMPL = 0xE4F90               # doug_lea_allocator::malloc_impl: ESI the allocator, EAX the bytes, ret 4
RENDER_DEVICE = 0x2260478           # the render device: +17Ch the ID3D11Device, +180h the context
SITES = {0x11B5A0: "constant_upload_set_memory", 0x3DDFA1: "constant_upload_alloc",
         0x3DDFAC: "constant_upload_zero", 0x3DDFD0: "constant_upload_update"}
DATA, SIZE, HARDWARE, CHANGED = 0x58, 0x5C, 0x60, 0x64
OBJECT_SIZE = 0x68

STUBS, OBJ, HEAP, GPU, STACK = 0x30000000, 0x30100000, 0x31000000, 0x34000000, 0x37000000
RETURN = STUBS                      # the made-up caller: a hlt the run stops at
CREATE, MAP, UNMAP = STUBS + 0x10, STUBS + 0x20, STUBS + 0x30
DEVICE, D3D_DEVICE, CONTEXT, VTABLE = OBJ, OBJ + 0x400, OBJ + 0x800, OBJ + 0xC00
NAME = OBJ + 0x1000                 # the buffer's name and its {begin, end}
BUFFERS = OBJ + 0x2000              # the made-up buffer objects of set_memory's cases
SOURCE = OBJ + 0x8000               # the constants' sources
GPRS = (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP, UC_X86_REG_ESP)
GPR_NAMES = ("eax", "ecx", "edx", "ebx", "esi", "edi", "ebp", "esp")
XMMS = (UC_X86_REG_XMM0, UC_X86_REG_XMM1, UC_X86_REG_XMM2, UC_X86_REG_XMM3, UC_X86_REG_XMM4, UC_X86_REG_XMM5,
        UC_X86_REG_XMM6, UC_X86_REG_XMM7)


class Machine:
    """One exe loaded, with the models of the allocator and the device."""

    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((STUBS, 0x1000), (OBJ, 0x10000), (HEAP, 0x1000000), (GPU, 0x1000000), (STACK, 0x10000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        emu.mem_write(RETURN, b"\xf4")
        emu.mem_write(CREATE, b"\xc2\x10\x00")      # CreateBuffer(device, desc, data, out): ret 10h
        emu.mem_write(MAP, b"\xc2\x18\x00")         # Map(context, resource, sub, type, flags, out): ret 18h
        emu.mem_write(UNMAP, b"\xc2\x0c\x00")       # Unmap(context, resource, sub): ret 0Ch
        self.word(RENDER_DEVICE, DEVICE)
        self.word(DEVICE + 0x17C, D3D_DEVICE)
        self.word(DEVICE + 0x180, CONTEXT)
        self.word(D3D_DEVICE, VTABLE)
        self.word(CONTEXT, VTABLE)
        self.word(VTABLE + 0x0C, CREATE)
        self.word(VTABLE + 0x38, MAP)
        self.word(VTABLE + 0x3C, UNMAP)
        self.heap, self.slices, self.hardware = HEAP, GPU, 0x1000
        self.allocations, self.maps = [], 0
        self.mapped = {}                # resource: (slice, bytes) while mapped
        self.gpu = {}                   # resource: its contents on the GPU
        self.sizes = {}                 # resource: its ByteWidth
        self.counting, self.steps = False, 0
        emu.hook_add(UC_HOOK_CODE, self.model, begin=MALLOC_IMPL, end=MALLOC_IMPL)
        emu.hook_add(UC_HOOK_CODE, self.model, begin=CREATE, end=UNMAP)
        emu.hook_add(UC_HOOK_CODE, self.count)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def count(self, _emu, _address, _size, _data):
        self.steps += 1

    def model(self, emu, address, _size, _data):
        sp = emu.reg_read(UC_X86_REG_ESP)
        if address == MALLOC_IMPL:
            size = emu.reg_read(UC_X86_REG_EAX)
            block = self.heap
            self.heap += (size + 0x3F) & ~0x1F
            emu.mem_write(block, bytes(random.getrandbits(8) for _ in range(min(size, 64))))   # not zeros
            self.allocations.append(size)
            emu.reg_write(UC_X86_REG_EAX, block)
            emu.reg_write(UC_X86_REG_EIP, self.read(sp))
            emu.reg_write(UC_X86_REG_ESP, sp + 8)
        elif address == CREATE:
            desc, out = self.read(sp + 8), self.read(sp + 16)
            self.hardware += 0x10
            self.sizes[self.hardware] = self.read(desc)
            self.gpu[self.hardware] = None          # undefined until the first upload
            self.word(out, self.hardware)
            emu.reg_write(UC_X86_REG_EAX, 0)
        elif address == MAP:
            resource, kind, out = self.read(sp + 8), self.read(sp + 16), self.read(sp + 24)
            assert kind == 4, f"Map type {kind}, not WRITE_DISCARD"
            self.maps += 1
            size = self.sizes[resource]
            self.mapped[resource] = (self.slices, size)
            emu.mem_write(self.slices, bytes(size))
            self.word(out, self.slices)             # D3D11_MAPPED_SUBRESOURCE.pData
            self.word(out + 4, 0)
            self.word(out + 8, 0)
            self.slices += (size + 0xF) & ~0xF
            emu.reg_write(UC_X86_REG_EAX, 0)
        elif address == UNMAP:
            resource = self.read(sp + 8)
            at, size = self.mapped.pop(resource)
            self.gpu[resource] = bytes(emu.mem_read(at, size))

    def call(self, start, regs, args=(), xmm=None):
        """Runs `start` as called from RETURN with `args` on the stack and
        `regs` set; the instructions it took (models excluded)."""
        emu = self.emu
        sp = STACK + 0xF000
        for value in reversed((RETURN,) + tuple(args)):
            sp -= 4
            self.word(sp, value)
        for reg, value in regs.items():
            emu.reg_write(reg, value)
        for reg, value in (xmm or {}).items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, sp)
        self.steps = 0
        emu.emu_start(start, RETURN, count=5_000_000)
        if emu.reg_read(UC_X86_REG_EIP) != RETURN:
            raise RuntimeError(f"{start:#x} did not return (at {emu.reg_read(UC_X86_REG_EIP):#x})")
        return self.steps

    def state(self):
        return [self.emu.reg_read(r) for r in GPRS], [self.emu.reg_read(r) for r in XMMS]


def set_memory_cases(rng):
    sizes = [16, 32, 48, 64, 128, 192, 256, 512, 1024, 4096, 17, 33, 100, 255]
    for n in range(4000):
        size = rng.choice(sizes)
        offset = rng.choice([0, 0, 16, rng.randrange(0, size + 1), size, size + rng.randrange(1, 64)])
        count = rng.choice([0, 4, 12, 16, 48, 64, 192, rng.randrange(0, size + 40), size, size + 16])
        shadow = bytes(rng.getrandbits(8) for _ in range(size))
        source = bytearray(shadow[offset:offset + count].ljust(count, b"\0")) if offset < size \
            else bytearray(rng.getrandbits(8) for _ in range(count))
        kind = rng.choice(["same", "first", "middle", "last", "all", "two"])
        if count and kind != "same":
            if kind == "all":
                source = bytearray(rng.getrandbits(8) for _ in range(count))
            else:
                spots = {"first": [0], "middle": [rng.randrange(count)], "last": [count - 1],
                         "two": [rng.randrange(count), rng.randrange(count)]}[kind]
                for spot in spots:
                    source[spot] ^= rng.randrange(1, 256)
        yield n, size, offset, count, shadow, bytes(source)


def check_set_memory(stock, built, rng):
    failed, costs = 0, {}
    for n, size, offset, count, shadow, source in set_memory_cases(rng):
        regs = {UC_X86_REG_EAX: SOURCE, UC_X86_REG_ECX: count, UC_X86_REG_ESI: offset,
                UC_X86_REG_EBX: rng.getrandbits(32), UC_X86_REG_EDI: rng.getrandbits(32),
                UC_X86_REG_EBP: rng.getrandbits(32), UC_X86_REG_EDX: rng.getrandbits(32)}
        xmm = {r: rng.getrandbits(128) for r in XMMS}
        obj = bytes(rng.getrandbits(8) for _ in range(OBJECT_SIZE))
        out = []
        for m in (stock, built):
            emu = m.emu
            block = HEAP
            emu.mem_write(block, shadow + bytes(64))
            emu.mem_write(SOURCE, source + bytes(16))
            emu.mem_write(BUFFERS, obj)
            m.word(BUFFERS + DATA, block)
            m.word(BUFFERS + SIZE, size)
            steps = m.call(SET_MEMORY, regs, (BUFFERS,), xmm)
            gprs, xmms = m.state()
            out.append((gprs, xmms, bytes(emu.mem_read(block, size + 64)), bytes(emu.mem_read(BUFFERS, OBJECT_SIZE)),
                        steps))
        (g0, x0, s0, o0, n0), (g1, x1, s1, o1, n1) = out
        key = (min(count, max(size - offset, 0)) if offset <= size else count)
        bucket = costs.setdefault(min((16, 64, 192, 512, 1024, 4096), key=lambda b: abs(b - key)), [0, 0, 0])
        bucket[0] += n0
        bucket[1] += n1
        bucket[2] += 1
        if (g0, x0, s0, o0) != (g1, x1, s1, o1):
            failed += 1
            if failed <= 5:
                diff = [f"{GPR_NAMES[i]} {a:#x}/{b:#x}" for i, (a, b) in enumerate(zip(g0, g1)) if a != b]
                diff += [f"xmm{i}" for i, (a, b) in enumerate(zip(x0, x1)) if a != b]
                diff += ["shadow"] * (s0 != s1) + ["object"] * (o0 != o1)
                print(f"  FAIL  set_memory case {n}: size {size}, offset {offset}, count {count}: {', '.join(diff)}")
    print(f"  {'ok' if not failed else 'FAIL'}  set_memory: 4000 cases, registers, XMM0..7, shadow and object "
          f"the original's" + (f" ({failed} differ)" if failed else ""))
    for bucket in sorted(costs):
        stock_n, built_n, cases = costs[bucket]
        print(f"        ~{bucket:4} bytes: {stock_n / cases:7.0f} instructions, now {built_n / cases:5.0f} "
              f"({cases} cases)")
    return failed


def construct(m, size):
    """A buffer made by the exe's constructor (0x3DDE90): its object."""
    obj = m.heap
    m.heap += 0x80
    m.emu.mem_write(obj, bytes(0x80))           # what the constructor leaves alone: alike in both
    m.emu.mem_write(NAME + 8, b"constants\0")
    m.word(NAME, NAME + 8)
    m.word(NAME + 4, NAME + 8 + 9)
    m.allocations = []
    m.call(CTOR, {UC_X86_REG_EDI: obj, UC_X86_REG_EAX: NAME, UC_X86_REG_ESI: 0x11111111}, (1, 0, size))
    return obj


def check_constructor(stock, built):
    failed = 0
    for size in (16, 64, 256, 4096):
        objects = [construct(m, size) for m in (stock, built)]
        asked = [m.allocations for m in (stock, built)]
        blocks = [m.read(o + DATA) for m, o in zip((stock, built), objects)]
        zeros = [bytes(m.emu.mem_read(b, n[0])) == bytes(n[0]) for m, b, n in zip((stock, built), blocks, asked)]
        fields = [bytearray(m.emu.mem_read(o, OBJECT_SIZE)) for m, o in zip((stock, built), objects)]
        for f in fields:
            f[DATA:DATA + 4] = bytes(4)
            f[0:16] = bytes(16)                     # the name's pointers (into the object)
        ok = asked == [[size], [2 * size + 16]] and all(zeros) and fields[0] == fields[1]
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  constructor, {size} bytes: asks {asked[0]} / {asked[1]}, "
              f"zeros {zeros}, the object the same {fields[0] == fields[1]}")
    return failed


def check_frames(stock, built, rng):
    """Draws over four buffers: (size, constants [(offset, bytes, how often new)])."""
    layout = [(64, [(0, 64, 0.0)]),                                   # the view-projection: one a frame
              (256, [(0, 64, 1.0), (64, 64, 0.0), (128, 16, 0.3), (144, 112, 0.0)]),   # world, misc
              (1024, [(0, 1024, 0.5)]),                                # a bone palette
              (128, [(0, 16, 0.0), (16, 16, 0.02), (32, 96, 0.0)])]    # material: rarely a change
    machines = (stock, built)
    objects = [[construct(m, size) for size, _ in layout] for m in machines]
    values = {}
    failed, draws = 0, 0
    maps = [m.maps for m in machines]
    steps = [0, 0]
    for frame in range(6):
        for b, (_, constants) in enumerate(layout):
            for c, (_, length, _) in enumerate(constants):
                if (b, c) not in values or b == 0:
                    values[b, c] = bytes(rng.getrandbits(8) for _ in range(length))
        for draw in range(25):
            draws += 1
            for b, (_, constants) in enumerate(layout):
                for c, (offset, length, new) in enumerate(constants):
                    if rng.random() < new:
                        values[b, c] = bytes(rng.getrandbits(8) for _ in range(length))
                    for i, m in enumerate(machines):
                        m.emu.mem_write(SOURCE, values[b, c])
                        steps[i] += m.call(SET_MEMORY, {UC_X86_REG_EAX: SOURCE, UC_X86_REG_ECX: length,
                                                        UC_X86_REG_ESI: offset}, (objects[i][b],))
            if rng.random() < 0.2:      # one of the 67 inlined copies: a byte into the shadow, m_changed set
                b = rng.randrange(len(layout))
                at, value = rng.randrange(layout[b][0]), rng.getrandbits(8)
                for m, o in zip(machines, objects):
                    m.emu.mem_write(m.read(o[b] + DATA) + at, bytes([value]))
                    m.emu.mem_write(o[b] + CHANGED, b"\x01")
            for b in range(len(layout)):
                for i, m in enumerate(machines):
                    steps[i] += m.call(UPDATE, {UC_X86_REG_ESI: objects[i][b]})
            for b in range(len(layout)):
                gpu = [m.gpu[m.read(o[b] + HARDWARE)] for m, o in zip(machines, objects)]
                changed = [m.emu.mem_read(o[b] + CHANGED, 1)[0] for m, o in zip(machines, objects)]
                if gpu[0] != gpu[1] or changed[0] != changed[1] or gpu[0] is None:
                    failed += 1
                    if failed <= 5:
                        print(f"  FAIL  frame {frame} draw {draw} buffer {b}: the GPU's bytes or m_changed differ")
    stock_maps, built_maps = (m.maps - n for m, n in zip(machines, maps))
    print(f"  {'ok' if not failed else 'FAIL'}  {draws} draws over 4 buffers: after every update every buffer's "
          f"bytes on the GPU and m_changed the original's; Map {stock_maps} -> {built_maps}, instructions "
          f"(set_memory and update, Map/Unmap not counted inside) {steps[0]} -> {steps[1]}")
    return failed + (built_maps >= stock_maps)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late = open(base.late_join_bin(), "rb").read()
    built, _ = base.build(original, "section", code, None, late)
    hooks = {h[0]: h[2] for h in base.RUST_HOOKS if h[2].startswith("constant_upload_")}
    if hooks != SITES:
        sys.exit(f"the build hooks {[(hex(v), s) for v, s in hooks.items()]}, not the four sites")
    random.seed(134)
    stock, patched = Machine(original), Machine(bytes(built))
    failed = check_set_memory(stock, patched, random.Random(1))
    failed += check_constructor(stock, patched)
    failed += check_frames(stock, patched, random.Random(2))
    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
