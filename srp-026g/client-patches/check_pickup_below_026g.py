#!/usr/bin/env python3
"""Run the original and patch 23's use scan (detect_usable_object, 0x2D84C0)
on made-up ray hits.

    python check_pickup_below_026g.py <original survarium.exe>

Requires Python `unicorn`. The scan and victory_item_core::can_use run as
machine code; the ray query, collision groups, get_transform and the
allocator are stubs. The default build of patch_026g.py must give the
original's result for a battery in reach and in view, refuse one behind a
wall or out of reach, and take one under the floor within 1 m of the feet
across, with the feet as its point. Says nothing about which hits the real
collision world returns.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EIP, UC_X86_REG_ESP, \
    UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_FPSW

import patch_026g as base

SCAN = 0x2D84C0
GATHER = 0x2DC390      # query_objects_by_type<usable_object>: the hit's usable objects
ALLOCATOR = 0xA8001C
P, PHYS, VT, HITS, STATE, INV, WEAPON, REF = [0x20000000 + i * 0x1000 for i in range(8)]
STACK, STOP, STUB = 0x21008000, 0x22000000, 0x22000100
FEET, EYE = (12.0, 10.0, -7.0), (12.0, 11.7, -7.0)
REGISTERS = (UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI)


class Scan:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        headers = base.Headers(data)
        for _, rva, vsize, raw, rsize, _ in headers.sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(P, 0x80000, UC_PROT_ALL)
        emu.mem_map(0x21000000, 0x10000, UC_PROT_ALL)
        emu.mem_map(STOP, 0x10000, UC_PROT_ALL)
        for at, value in ((P + 0x350, PHYS), (P + 0x130, P), (P + 0x108, VT + 0x100),
                          (VT + 0x108, STUB + 0x40), (REF, STATE), (P + 0x10C, INV),
                          (P + 0x128, WEAPON), (WEAPON, VT + 0x200), (VT + 0x250, STUB + 0x50),
                          (PHYS, VT), (VT + 0x4C, STUB), (ALLOCATOR, VT + 0x300), (VT + 0x318, STUB + 0x30)):
            self.word(at, value)
        emu.mem_write(P + 0x33C, b"\1")                    # the player can use
        self.floats(P + 0x43C, FEET)
        self.floats(P + 0x47C, EYE)
        self.floats(P + 0x46C, (0.0, -1.0, 0.0))
        self.floats(P + 0x8000, (-99.0,) * 3)              # the point returned
        self.hits, self.centres, self.freed, self.reach = [], {}, [], None
        emu.hook_add(UC_HOOK_CODE, self.stub)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def floats(self, at, values):
        self.emu.mem_write(at, struct.pack(f"<{len(values)}f", *values))

    def returns(self, value, pop=0):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        self.emu.reg_write(UC_X86_REG_EAX, value)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pop)

    def stub(self, emu, address, size, _):
        sp, this = emu.reg_read(UC_X86_REG_ESP), emu.reg_read(UC_X86_REG_ECX)
        if address == STUB:                                # ray_query(from, to, reach, out, group, mask)
            assert (self.read(sp + 4), self.read(sp + 8)) == (P + 0x47C, P + 0x46C)
            assert (self.read(sp + 20), self.read(sp + 24)) == (0x114, 0x8A)
            assert emu.reg_read(UC_X86_REG_FPSW) & 0x3800 == 0, "x87 stack not empty"
            self.reach = struct.unpack("<f", emu.mem_read(sp + 12, 4))[0]
            out = self.read(sp + 16)
            self.word(out, HITS)
            self.word(out + 4, HITS + 40 * len(self.hits))
            self.word(out + 8, ALLOCATOR)
            self.returns(0, 24)
        elif address in (STUB + 0x10, STUB + 0x20):        # collision group; the hit's owner
            self.returns(self.read(this + 4))
        elif address == STUB + 0x30:                       # free
            self.freed.append(self.read(sp + 4))
            self.returns(0, 16)
        elif address == STUB + 0x40:
            self.returns(REF)
        elif address == STUB + 0x50:                       # the weapon does not forbid it
            self.returns(0)
        elif address == STUB + 0x60:                       # another usable object's can_use
            self.returns(1, 4)
        elif address == STUB + 0x70:                       # get_transform(out)
            out = self.read(sp + 4)
            self.floats(out, (0.0,) * 12 + self.centres[this] + (1.0,))
            self.returns(out, 4)
        elif address == GATHER:
            self.word(emu.reg_read(UC_X86_REG_ESI), self.read(sp + 4))
            self.returns(0, 12)

    def add(self, kind, point, centre=None):
        """A hit: "wall" (walkable and hittable), "walkable", "battery" or "other"."""
        block = P + 0x10000 + len(self.hits) * 0x1000
        obj, owner, usable, table, vector = block, block + 0x100, block + 0x300, block + 0x800, block + 0x900
        hit = HITS + 40 * len(self.hits)
        self.word(hit + 0x1C, obj)
        self.floats(hit, point)
        self.word(obj, table)
        self.word(table, STUB + 0x10)
        self.word(obj + 4, {"wall": 0xA, "walkable": 0x2}.get(kind, 0x80))
        for at, value in ((obj + 0xC, owner), (owner, table), (table + 4, STUB + 0x20), (owner + 4, vector),
                          (vector, usable), (usable, table), (table + 0x44, STUB + 0x70)):
            self.word(at, value)
        self.word(table + 0x30, 0x300710 if kind == "battery" else STUB + 0x60)
        self.centres[usable] = tuple(centre or point)
        self.hits.append(usable)
        return usable

    def run(self):
        emu = self.emu
        for at, value in ((STACK, STOP), (STACK + 4, P), (STACK + 8, P + 0x8000)):
            self.word(at, value)
        emu.reg_write(UC_X86_REG_ESP, STACK)
        for register in REGISTERS:
            emu.reg_write(register, 0x12345678)
        emu.emu_start(SCAN, STOP, count=20000)
        assert emu.reg_read(UC_X86_REG_EIP) == STOP and emu.reg_read(UC_X86_REG_ESP) == STACK + 12
        assert all(emu.reg_read(register) == 0x12345678 for register in REGISTERS)
        assert emu.reg_read(UC_X86_REG_FPSW) & 0x3800 == 0, "x87 stack left non-empty"
        assert not self.hits or HITS in self.freed
        return emu.reg_read(UC_X86_REG_EAX)

    def point(self):
        return struct.unpack("<3f", self.emu.mem_read(P + 0x8000, 12))


def cases():
    """(name, hits, taken by the original, taken by patch 23, the point)."""
    x, y, z = FEET
    nan = float("nan")
    floor = ("wall", (x, y, z))
    below = lambda across, depth=0.3: ("battery", (x + across, y - depth + 0.1, z), (x + across, y - depth, z))
    yield "in view", [("battery", (x + 1, y + 0.1, z))], True, True, (x + 1, y + 0.1, z)
    yield "none", [], False, False, None
    yield "behind a wall", [("wall", (x + 0.4, y + 1, z)), ("battery", (x + 0.7, y + 0.1, z))], False, False, None
    # The stub returns hits beyond the ray's reach; the real query would not.
    yield "out of reach", [("battery", (x + 2.5, y + 0.4, z))], True, False, None
    for across, taken in ((0.0, True), (0.6, True), (0.99, True), (1.01, False), (1.5, False)):
        yield f"under the floor, {across} m across", [floor, below(across)], False, taken, FEET if taken else None
    yield "two floors down", [floor, ("wall", (x, y - 1, z)), below(0, 2.3)], False, True, FEET
    yield "a walkable hit only", [("walkable", (x + 0.3, y + 0.6, z)), ("battery", (x + 0.8, y + 0.1, z))], True, True, None
    yield "centre above the feet", [floor, ("battery", (x, y - 0.1, z), (x, y + 0.05, z))], False, False, None
    yield "another object under the floor", [floor, ("other", (x, y - 0.2, z))], False, False, None
    yield "a NaN centre", [floor, ("battery", (x, y - 0.2, z), (nan, y - 0.3, z))], False, False, None
    yield "another object, then a battery", [floor, ("other", (x, y - 0.1, z)), below(0)], False, True, FEET


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = True
    patched, _ = base.build(original, "section", code, None, late_join)
    checks = 0
    for data, reach, column in ((original, 2.0, 2), (patched, 4.0, 3)):
        for case in cases():
            name, hits, point = case[0], case[1], case[4]
            scan = Scan(data)
            wanted = [scan.add(*hit) for hit in hits]
            expected = wanted[-1] if case[column] else 0
            assert scan.run() == expected, (data is patched, name)
            assert scan.reach == reach, (name, scan.reach)
            if expected and point is not None and data is patched:
                assert scan.point() == tuple(struct.unpack("<3f", struct.pack("<3f", *point))), (name, scan.point())
            checks += 1
    # Already carrying one: as the original, an unusable battery comes back
    # for the prompt (can_use is false), once the scan reaches it.
    scan = Scan(patched)
    scan.word(INV + 0x178, 1)
    scan.add(*("wall", FEET))
    battery = scan.add("battery", (FEET[0], 9.8, FEET[2]), (FEET[0], 9.7, FEET[2]))
    assert scan.run() == battery
    checks += 1
    print(f"patch 23: {checks} scans as expected (the original and the build)")


if __name__ == "__main__":
    main()
