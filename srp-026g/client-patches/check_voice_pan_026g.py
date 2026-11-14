#!/usr/bin/env python3
"""Patch 66's plain voice is panned by the engine's own function: this runs that function, of
the original exe, in Unicorn as the patch's boundary calls it (vostok/voice.rs, Listener::pan).

    python check_voice_pan_026g.py <original survarium.exe>

- the panning table is filled by its own constructor (panning_lut::panning_lut 0x5E150);
- the listener is given to a made-up scene by the engine's set_listener_properties (0x587D0),
  and read back by get_listenet_position (0x57560);
- sound_scene::calculate_3d_pan (0x57D70) is called with the scene, the table, a proxy of 144
  zero bytes (a point sound), a position and the distance.

It holds what the patch relies on: the pan's law (constant power: 0.7071 in both ears ahead
and behind, 1 and 0 at the side, towards the centre above; no gain by distance), a listener
turned and moved, and that of a point sound the function reads only the proxy's +80h, the
position, the table and the scene's three listener vectors, and writes only the two gains and
the low-pass coefficient (its 28 bytes of arguments popped).
"""
import math
import os
import struct
import sys

from unicorn import UC_ARCH_X86, UC_HOOK_MEM_READ, UC_HOOK_MEM_WRITE, UC_MEM_WRITE, UC_MODE_32, Uc
from unicorn.x86_const import (UC_X86_REG_CR0, UC_X86_REG_CR4, UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX,
                               UC_X86_REG_EDI, UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

PANNING_LUT, SET_LISTENER, LISTENER_POSITION, PAN = 0x5E150, 0x587D0, 0x57560, 0x57D70
LUT_VTABLE, DIRECTIONS, SPEAKERS, PROXY_SIZE, SOUND_TYPE = 0x9E150C, 512, 9, 0x90, 0x80
STACK, HEAP, END = 0x7000_0000, 0x6000_0000, 0x5000_0000
LUT, SCENE, PROXY, POSITION, MATRIX, COEFFICIENT, VECTORS = (HEAP + at for at in (0, 0x8000, 0x9000, 0xA000, 0xA100, 0xA200, 0xA300))
LUT_SIZE = 4 + 4 * SPEAKERS * DIRECTIONS
POISON = 777.0


class Engine:
    def __init__(self, exe):
        data = open(exe, "rb").read()
        pe = struct.unpack_from("<I", data, 0x3C)[0]
        sections, optional = struct.unpack_from("<H", data, pe + 6)[0], pe + 24
        base, size = struct.unpack_from("<I", data, optional + 28)[0], struct.unpack_from("<I", data, optional + 56)[0]
        assert base == 0x10000, hex(base)
        table = optional + struct.unpack_from("<H", data, pe + 20)[0]
        self.uc = uc = Uc(UC_ARCH_X86, UC_MODE_32)
        uc.mem_map(base, (size + 0xFFF) & ~0xFFF)
        for i in range(sections):
            _, virtual, address, raw_size, raw = struct.unpack_from("<8sIIII", data, table + 40 * i)
            uc.mem_write(base + address, data[raw:raw + min(raw_size, virtual or raw_size)])
        uc.mem_map(STACK - 0x100000, 0x100000)
        uc.mem_map(HEAP, 0x100000)
        uc.mem_map(END, 0x1000)
        # SSE on, as the game's threads have it
        uc.reg_write(UC_X86_REG_CR0, (uc.reg_read(UC_X86_REG_CR0) | 2) & ~4)
        uc.reg_write(UC_X86_REG_CR4, uc.reg_read(UC_X86_REG_CR4) | 0x600)
        self.accesses = None
        uc.hook_add(UC_HOOK_MEM_READ | UC_HOOK_MEM_WRITE, self.access)

    def access(self, _uc, kind, address, size, _value, _user):
        if self.accesses is not None and not STACK - 0x100000 <= address < STACK:
            self.accesses.add(("write" if kind == UC_MEM_WRITE else "read", address, size))

    def call(self, address, stack=(), registers=None):
        """The bytes of arguments the callee popped; every register it is not given is poison."""
        uc, sp = self.uc, STACK - 0x1000
        words = [END, *stack]
        sp -= 4 * len(words)
        uc.mem_write(sp, struct.pack("<%dI" % len(words), *words))
        for register in (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP):
            uc.reg_write(register, 0xDEAD0000)
        for register, value in (registers or {}).items():
            uc.reg_write(register, value)
        uc.reg_write(UC_X86_REG_ESP, sp)
        uc.emu_start(address, END, count=2_000_000)
        assert uc.reg_read(UC_X86_REG_EIP) == END, hex(uc.reg_read(UC_X86_REG_EIP))
        return uc.reg_read(UC_X86_REG_ESP) - sp - 4

    def floats(self, address, count):
        return struct.unpack("<%df" % count, self.uc.mem_read(address, 4 * count))

    def listener(self, position, front, top):
        self.uc.mem_write(VECTORS, struct.pack("<9f", *position, *front, *top))
        assert self.call(SET_LISTENER, [VECTORS + 12, VECTORS + 24], {UC_X86_REG_ESI: SCENE, UC_X86_REG_EAX: VECTORS}) == 8
        assert self.uc.mem_read(SCENE + 0x2FA, 1) == b"\x01"
        assert self.call(LISTENER_POSITION, [], {UC_X86_REG_EAX: SCENE, UC_X86_REG_ESI: VECTORS + 64}) == 0
        assert self.uc.reg_read(UC_X86_REG_EAX) == VECTORS + 64
        return self.floats(VECTORS + 64, 3)

    def pan(self, position, distance):
        """As Listener::pan calls it: the two gains and the low-pass coefficient."""
        uc = self.uc
        uc.mem_write(PROXY, bytes(PROXY_SIZE))
        uc.mem_write(POSITION, struct.pack("<3f", *position))
        uc.mem_write(MATRIX, struct.pack("<4f", *[POISON] * 4))
        uc.mem_write(COEFFICIENT, struct.pack("<f", POISON))
        bits = struct.unpack("<I", struct.pack("<f", distance))[0]
        assert self.call(PAN, [SCENE, LUT, PROXY, POSITION, bits, MATRIX, COEFFICIENT]) == 28
        left, right, third, fourth = self.floats(MATRIX, 4)
        assert third == fourth == POISON, (third, fourth)
        return left, right, self.floats(COEFFICIENT, 1)[0]


def near(a, b, tolerance=2e-3):
    return abs(a - b) <= tolerance


def main():
    exe = sys.argv[1] if len(sys.argv) > 1 else os.environ["SRP_026G_EXE"]
    engine = Engine(exe)

    assert engine.call(PANNING_LUT, [LUT]) == 4
    assert struct.unpack("<I", engine.uc.mem_read(LUT, 4))[0] == LUT_VTABLE
    table = bytes(engine.uc.mem_read(LUT, LUT_SIZE))
    for direction in range(DIRECTIONS):
        gains = engine.floats(LUT + 4 + 4 * SPEAKERS * direction, SPEAKERS)
        assert near(gains[0] ** 2 + gains[1] ** 2, 1.0, 1e-4) and not any(gains[2:]), (direction, gains)
    print("ok: the panning table: %d directions, two speakers, constant power" % DIRECTIONS)

    # The listener at the origin, looking down +Z, up +Y: its right is +X.
    assert engine.listener((0, 0, 0), (0, 0, 1), (0, 1, 0)) == (0.0, 0.0, 0.0)
    engine.accesses = set()
    half = math.sqrt(0.5)
    law = {}
    for degrees in range(-180, 181, 15):
        angle = math.radians(degrees)
        left, right, low_pass = engine.pan((5 * math.sin(angle), 0, 5 * math.cos(angle)), 5.0)
        law[degrees] = (left, right)
        assert near(left ** 2 + right ** 2, 1.0), (degrees, left, right)
        # the table's angle: 45 degrees and half the bearing off straight ahead or behind
        fold = math.radians(degrees if abs(degrees) <= 90 else math.copysign(180 - abs(degrees), degrees))
        assert near(left, math.cos(math.pi / 4 + fold / 2), 0.01) and near(right, math.sin(math.pi / 4 + fold / 2), 0.01), (degrees, left, right)
        assert (low_pass == 1.0) if abs(degrees) <= 90 else (0.79 < low_pass < 1.0), (degrees, low_pass)
    assert near(law[0][0], half) and near(law[0][1], half) and near(law[180][0], half) and near(law[-180][1], half)
    assert near(law[90][0], 0.0) and near(law[90][1], 1.0) and near(law[-90][0], 1.0) and near(law[-90][1], 0.0)
    assert all(near(law[d][0], law[-d][1]) for d in law)
    print("ok: ahead and behind %.4f in both ears; 30 degrees right %.4f / %.4f; at the side %.4f / %.4f"
          % (law[0][0], *law[30], *law[90]))

    # No gain by distance: the same from 0.3 m to 50 m, and whatever distance is said.
    for metres in (0.3, 1, 2, 5, 9, 10, 50):
        x = metres * half
        left, right, _ = engine.pan((x, 0, x), max(metres, 1.0))
        assert near(left, law[45][0]) and near(right, law[45][1]), (metres, left, right)
    assert engine.pan((5, 0, 0), 1.0)[:2] == engine.pan((5, 0, 0), 50.0)[:2]
    # At the listener, and straight above: the centre. Above the side: between.
    assert all(near(g, half) for g in engine.pan((0, 0, 0), 1.0)[:2]) and all(near(g, half) for g in engine.pan((0, 5, 0), 5.0)[:2])
    left, right, _ = engine.pan((5 * half, 5 * half, 0), 5.0)
    assert 0.15 < left < 0.3 and 0.85 < right < 0.95, (left, right)
    print("ok: no gain by distance; a sound at the listener or above it is centred")
    reads = sorted({(a, s) for kind, a, s in engine.accesses if kind == "read" and not 0x10000 <= a < HEAP})
    writes = sorted({(a, s) for kind, a, s in engine.accesses if kind == "write"})
    inside = lambda address, start, size: start <= address < start + size
    for address, size in reads:
        assert (address == PROXY + SOUND_TYPE and size == 4 or inside(address, POSITION, 12) or inside(address, LUT + 4, LUT_SIZE - 4)
                or inside(address, SCENE + 0x120, 0x18)), ("read", hex(address), size)
    # The listener's vectors are read by a locked cmpxchg8b: a write of what was there.
    listener = bytes(engine.uc.mem_read(SCENE + 0x120, 0x18))
    for address, size in writes:
        assert (inside(address, MATRIX, 8) or address == COEFFICIENT or address in (SCENE + 0x120, SCENE + 0x128, SCENE + 0x130)), ("write", hex(address), size)
    engine.accesses = None
    engine.pan((3, 0, 4), 5.0)
    assert bytes(engine.uc.mem_read(SCENE + 0x120, 0x18)) == listener and bytes(engine.uc.mem_read(LUT, LUT_SIZE)) == table
    assert bytes(engine.uc.mem_read(PROXY, PROXY_SIZE)) == bytes(PROXY_SIZE)
    print("ok: of a point sound it reads the proxy's +80h, the position, the table and the listener; writes two gains and the coefficient")

    # A listener somewhere on a map, turned to look down +X (its right is -Z): a speaker 4 m
    # along -Z is at its right, one along +X ahead. The offset is added to the place the
    # engine keeps (halves: a quarter of a metre apart out here), so the direction is exact.
    position = engine.listener((301.3, 12.7, -148.2), (1, 0, 0), (0, 1, 0))
    assert all(abs(got - want) <= 0.25 for got, want in zip(position, (301.3, 12.7, -148.2))), position
    at = lambda offset: tuple(p + o for p, o in zip(position, offset))
    left, right, _ = engine.pan(at((0, 0, -4)), 4.0)
    assert near(left, 0.0) and near(right, 1.0), (left, right)
    left, right, _ = engine.pan(at((0, 0, 4)), 4.0)
    assert near(left, 1.0) and near(right, 0.0), (left, right)
    left, right, _ = engine.pan(at((4, 0, 0)), 4.0)
    assert near(left, half) and near(right, half), (left, right)
    print("ok: a listener moved and turned: the speaker at its right is in the right ear")
    print("all cases passed")


if __name__ == "__main__":
    main()
