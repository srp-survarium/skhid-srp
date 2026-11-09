#!/usr/bin/env python3
"""Run the original's and the build's texture streaming bookkeeping (patch 134, the Rust patch
texture_streaming) on made-up scenes and texture registries, and count the instructions each
runs.

    python check_texture_streaming_026g.py <original survarium.exe>

Requires Python `unicorn` (2.1.4) and the arena's tools (`nix develop ./srp-026g`: the build
links the arena). The exe's sections are loaded as they are; each function runs from its
entry, as its one caller calls it, to its `ret`:

- scene::update_streaming_data (0x155980, EAX the scene): a scene's streamable textures (148h
  bytes each) with lists of records whose proxies are the exe's three classes (their own
  vtables: the original runs their methods, the math::sqrt 0x38960 among them), the values
  random and the special ones (zeros of both signs, infinities, quiet and signalling NaNs of
  both signs, denormals, ones whose squares overflow or vanish), proxies shared between lists;
  every byte of the made-up memory after the run is the same in both (the records, the
  surfaces' marks, and nothing else written), under the x87 control word 27Fh (53 bits, the
  MSVC runtime's) and 37Fh (64 bits). And an empty vector, of zeros or not. No signalling
  NaN where the native loads a float with `fld` (a surface's time and factor): Unicorn's x87
  keeps it signalling there, the CPU makes it quiet; the patch does as the CPU, which its host
  tests prove against this CPU's x87 (`cargo test`, vostok::streaming).
- resource_manager::get_texture_video_memory_size (0x3BE280, EAX the manager): a registry map
  (an STLport red-black tree, its header at +8809Ch) of names with and without "$user$" at
  every place, at every alignment, inline in their node as the engine keeps them or elsewhere,
  one ending on the last byte of a page with nothing mapped after it; the same EAX in both,
  and no byte written. And an empty registry.
- the callee-saved registers, ESP and the x87 stack after each, as the original leaves them.

Then both on a scene and a registry of a match's size (1000 streamable textures with 9000
records; 3000 textures named as the game names them), and how many instructions each runs.
Says nothing about the real scene's sizes, which only the game has (the measured share of the
render thread is in client_patch.md 3.79).
"""
import random
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_EDI, UC_X86_REG_EIP,
                               UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_FPCW, UC_X86_REG_FPSW)

import patch_026g as base

UPDATE, UPDATE_RET = 0x155980, 0x1559FE
TEXTURE_MEMORY, TEXTURE_MEMORY_RET = 0x3BE280, 0x3BE2C7
SURFACE_VTABLE, GRASS_VTABLE, DECAL_VTABLE = 0x9F7060, 0x9F7080, 0x9F70A0
SCENE_STREAMABLE, REGISTRY = 0x126158, 0x8809C
DATA, DATA_SIZE = 0x30000000, 0x01000000      # the made-up objects
EDGE = 0x33000000                              # one page, nothing mapped after it
STACK, STOP = 0x2F000000, 0x2E000000
SAVED = (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)


class Memory:
    """The made-up objects, laid out once and written into each machine the same."""

    def __init__(self, seed):
        self.rng = random.Random(seed)
        self.data = bytearray(self.rng.randbytes(DATA_SIZE))   # canaries: a stray write shows
        self.edge = bytearray(self.rng.randbytes(0x1000))
        self.next = DATA + 0x1000

    def alloc(self, size, align=16):
        at = (self.next + align - 1) & ~(align - 1)
        self.next = at + size
        assert self.next <= DATA + DATA_SIZE
        return at

    def put(self, at, raw):
        if EDGE <= at < EDGE + 0x1000:
            self.edge[at - EDGE:at - EDGE + len(raw)] = raw
        else:
            self.data[at - DATA:at - DATA + len(raw)] = raw

    def word(self, at, value):
        self.put(at, struct.pack("<I", value & 0xFFFFFFFF))

    def floats(self, at, bits):
        self.put(at, struct.pack(f"<{len(bits)}I", *bits))


SPECIAL = [0x00000000, 0x80000000, 0x7F800000, 0xFF800000, 0x7FC00000, 0xFFC00000, 0x7FC12345, 0xFFC00001,
           0x7F800001, 0xFF80ABCD, 0x7FA00000, 0x00000001, 0x807FFFFF, 0x00400000, 0x7F7FFFFF, 0x5F800000,
           0x1F800000, 0x9F000001]


def fbits(x):
    return struct.unpack("<I", struct.pack("<f", x))[0]


def signalling(bits):
    return bits & 0x7F800000 == 0x7F800000 and bits & 0x7FFFFF and not bits & 0x400000


def a_float(rng, special, loaded=False):
    """Random bits, one in `special` a special value; for a float the native loads with the
    x87's `fld` (`loaded`), no signalling NaN: Unicorn's x87 keeps one signalling there, where
    the CPU makes it quiet (Intel SDM, FLD's #IA); the host tests compare the patch with this
    CPU's x87 on them (vostok::streaming, the_x87_model_is_this_cpus_x87)."""
    if rng.random() < special:
        return rng.choice([bits for bits in SPECIAL if not (loaded and signalling(bits))])
    return fbits(rng.uniform(-50.0, 50.0) * rng.choice((1.0, 0.01, 100.0)))


def scene(memory, textures, records, special, share=0.3):
    """A scene of `textures` streamable textures and about `records` records, proxies of the
    three classes (one in `share` taken again from an earlier record); returns its address."""
    rng = memory.rng
    table = memory.alloc(0x148 * textures + 16)
    proxies = []
    for t in range(textures):
        head = 0
        for _ in range(rng.choice((0, 1, 2)) if records == 0 else max(0, int(rng.gauss(records / textures, 3)))):
            if proxies and rng.random() < share:
                proxy = rng.choice(proxies)
            else:
                proxy = memory.alloc(0x20)
                kind = rng.choices(("surface", "grass", "decal"), (8, 1, 1))[0]
                memory.word(proxy + 4, rng.getrandbits(32))       # its owner
                if kind == "surface":
                    instance, surface, matrix = memory.alloc(0x60), memory.alloc(0x50), memory.alloc(0x40, 4)
                    memory.word(proxy, SURFACE_VTABLE)
                    memory.word(proxy + 0xC, instance)
                    memory.floats(instance + 0x10, [a_float(rng, special) for _ in range(4)])
                    memory.word(instance + 0x38, surface)
                    memory.floats(instance + 0x48, [a_float(rng, special, loaded=True)])
                    memory.put(instance + 0x4C, bytes([rng.choice((0, 1))]))
                    memory.word(instance + 0x50, matrix)
                    memory.floats(surface + 0x4C, [a_float(rng, special, loaded=True)])
                    memory.floats(matrix, [a_float(rng, special) for _ in range(16)])
                elif kind == "grass":
                    memory.word(proxy, GRASS_VTABLE)
                else:
                    decal = memory.alloc(0x90)
                    memory.word(proxy, DECAL_VTABLE)
                    memory.word(proxy + 0xC, decal)
                    memory.floats(decal + 0x74, [a_float(rng, special) for _ in range(6)])
                proxies.append(proxy)
            node = memory.alloc(0x24, 4)
            memory.word(node, proxy)
            memory.word(node + 4, head)
            head = node
        memory.word(table + 0x148 * t + 0x110, head)
    holder = memory.alloc(0x10)
    memory.word(holder, table)
    memory.word(holder + 4, table + 0x148 * textures)
    return holder - SCENE_STREAMABLE


NAMES = ("textures/terrain/grass_01", "textures/weapons/ak74/ak74_d", "$user$rt_scene_0", "$user$/camo/baked_17",
         "textures/characters/stalker/body_n", "lightmaps/level_14/lm_0007", "$use$r", "$user", "x$user$",
         "textures/objects/barrel_$user$", "$$user$$", "$us$user$", "textures/very/long/path/to/an/" + "x" * 60)


def a_game_name(rng):
    """A name as the game's are: a texture's path, a few of the game's own "$user$" ones."""
    if rng.random() < 0.05:
        return f"$user$rt_{rng.randrange(100)}".encode()
    folder = rng.choice(("terrain", "weapons/ak74", "characters/stalker", "objects/barrels", "lightmaps/level_14",
                         "vegetation/trees", "decals/blood", "items/armor/suit_seva"))
    return f"textures/{folder}/{rng.choice(('wall', 'metal', 'body', 'leaf', 'lm'))}_{rng.randrange(1000):03}_" \
        f"{rng.choice('dnsf')}".encode()


def a_name(rng, game=False):
    if game:
        return a_game_name(rng)
    name = rng.choice(NAMES)
    if rng.random() < 0.3:
        cut = rng.randrange(len(name) + 1)
        name = name[:cut] + rng.choice(("$user$", "$", "$user", "user$", "/")) + name[cut:]
    return name.encode()


def registry(memory, count, edge=True, game=False):
    """A texture registry of `count` names (an STLport map's tree, its header at +8809Ch of the
    returned manager), names inline in their node or elsewhere at any alignment, one ending on
    the edge page's last byte; returns the manager and the expected sum."""
    rng = memory.rng
    header = memory.alloc(0x20, 4)
    nodes = [memory.alloc(0x128, 4) for _ in range(count)]
    total = 0
    for n, node in enumerate(nodes):
        name = a_name(rng, game)
        if n == 0 and edge:
            where = EDGE + 0x1000 - len(name) - 1
        elif rng.random() < 0.6:
            where = node + 0x1C                                  # the buffer string's own room
            name = name[:0x103]
        else:
            where = memory.alloc(len(name) + 1, 1) + rng.randrange(16)
            memory.next += 16
        memory.put(where, name + b"\0")
        memory.word(node + 0x10, where)
        memory.word(node + 0x14, where + len(name))
        texture = memory.alloc(0x40, 4)
        size = rng.getrandbits(32) if rng.random() < 0.05 else rng.randrange(1 << 24)
        memory.word(texture + 0x30, size)
        memory.word(node + 0x124, texture)
        if b"$user$" not in name:
            total = (total + size) & 0xFFFFFFFF

    def link(lo, hi, parent):
        if lo >= hi:
            return 0
        mid = (lo + hi) // 2 if rng.random() < 0.7 else rng.randrange(lo, hi)
        node = nodes[mid]
        memory.word(node + 4, parent)
        memory.word(node + 8, link(lo, mid, node))
        memory.word(node + 0xC, link(mid + 1, hi, node))
        return node

    root = link(0, count, header)
    memory.word(header + 4, root)
    memory.word(header + 8, nodes[0] if count else header)
    memory.word(header + 0xC, nodes[-1] if count else header)
    return header - REGISTRY, total


class Machine:
    def __init__(self, exe, memory):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        headers = base.Headers(exe)
        for _, rva, vsize, raw, rsize, _ in headers.sections(exe):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(exe[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(DATA, DATA_SIZE, UC_PROT_ALL)
        emu.mem_write(DATA, bytes(memory.data))
        emu.mem_map(EDGE, 0x1000, UC_PROT_ALL)
        emu.mem_write(EDGE, bytes(memory.edge))
        emu.mem_map(STACK - 0x10000, 0x20000, UC_PROT_ALL)
        emu.mem_map(STOP, 0x1000, UC_PROT_ALL)
        self.counted = 0
        self.counting = None

    def count(self, on):
        if on and self.counting is None:
            def one(*_):
                self.counted += 1
            self.counting = self.emu.hook_add(UC_HOOK_CODE, one)
        elif not on and self.counting is not None:
            self.emu.hook_del(self.counting)
            self.counting = None

    def call(self, entry, eax, control=0x27F):
        emu = self.emu
        emu.mem_write(STACK, struct.pack("<I", STOP))
        emu.reg_write(UC_X86_REG_ESP, STACK)
        emu.reg_write(UC_X86_REG_EAX, eax)
        emu.reg_write(UC_X86_REG_FPCW, control)
        for n, register in enumerate(SAVED):
            emu.reg_write(register, 0x51510000 + n)
        self.counted = 0
        emu.emu_start(entry, STOP, count=200_000_000)
        assert emu.reg_read(UC_X86_REG_EIP) == STOP
        assert emu.reg_read(UC_X86_REG_ESP) == STACK + 4, hex(emu.reg_read(UC_X86_REG_ESP))
        assert all(emu.reg_read(register) == 0x51510000 + n for n, register in enumerate(SAVED)), "callee-saved"
        assert emu.reg_read(UC_X86_REG_FPSW) & 0x3800 == 0, "the x87 stack left non-empty"
        assert emu.reg_read(UC_X86_REG_FPCW) == control
        return emu.reg_read(UC_X86_REG_EAX)

    def memory(self):
        return bytes(self.emu.mem_read(DATA, DATA_SIZE)), bytes(self.emu.mem_read(EDGE, 0x1000))


def differences(before, after):
    out = []
    for i in range(0, len(before), 4):
        if before[i:i + 4] != after[i:i + 4]:
            out.append((DATA + i, before[i:i + 4].hex(), after[i:i + 4].hex()))
            if len(out) > 8:
                break
    return out


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    patched, _ = base.build(original, "section", code, None, late_join)
    checks = 0

    # Streaming records: random and special values, both control words.
    for seed in range(12):
        memory = Memory(seed)
        at = scene(memory, textures=40, records=160, special=0.15 if seed % 3 else 0.5)
        for control in (0x27F, 0x37F):
            states = []
            for exe in (original, patched):
                machine = Machine(exe, memory)
                machine.call(UPDATE, at, control)
                states.append(machine.memory())
            assert states[0] == states[1], (seed, hex(control), differences(states[0][0], states[1][0]))
            assert states[0][0] != bytes(memory.data), "the records written"
            checks += 1
    for empty in ("zeros", "begin == end"):
        memory = Memory(99)
        holder = memory.alloc(0x10)
        memory.word(holder, 0 if empty == "zeros" else holder + 0x100)
        memory.word(holder + 4, 0 if empty == "zeros" else holder + 0x100)
        for exe in (original, patched):
            machine = Machine(exe, memory)
            machine.call(UPDATE, holder - SCENE_STREAMABLE)
            assert machine.memory() == (bytes(memory.data), bytes(memory.edge)), empty
            checks += 1
    print(f"update_streaming_data: {checks} scenes, every byte the same in the original and the build")

    # The registry's texture memory.
    sums = 0
    for seed in range(12):
        memory = Memory(1000 + seed)
        manager, total = registry(memory, count=rng_count(seed))
        for exe in (original, patched):
            machine = Machine(exe, memory)
            assert machine.call(TEXTURE_MEMORY, manager) == total, (seed, exe is patched)
            assert machine.memory() == (bytes(memory.data), bytes(memory.edge)), "nothing written"
            sums += 1
    memory = Memory(77)
    manager, total = registry(memory, count=0, edge=False)
    for exe in (original, patched):
        assert Machine(exe, memory).call(TEXTURE_MEMORY, manager) == total == 0
        sums += 1
    print(f"get_texture_video_memory_size: {sums} registries, the same sum in the original and the build")

    # A match's size, counted.
    memory = Memory(4242)
    at = scene(memory, textures=1000, records=9000, special=0.0)
    manager, total = registry(memory, count=3000, game=True)
    counts = {}
    for name, exe in (("original", original), ("build", patched)):
        machine = Machine(exe, memory)
        machine.count(True)
        machine.call(UPDATE, at)
        update = machine.counted
        assert machine.call(TEXTURE_MEMORY, manager) == total
        counts[name] = (update, machine.counted)
    (u0, t0), (u1, t1) = counts["original"], counts["build"]
    print(f"a frame of 1000 streamable textures, 9000 records: update_streaming_data {u0} instructions in the "
          f"original, {u1} in the build ({u1 / u0:.0%})")
    print(f"a registry of 3000 textures: get_texture_video_memory_size {t0} instructions in the original, "
          f"{t1} in the build ({t1 / t0:.0%})")


def rng_count(seed):
    return (1, 2, 3, 7, 31, 64, 200, 500, 1, 15, 100, 300)[seed]


if __name__ == "__main__":
    main()
