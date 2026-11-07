#!/usr/bin/env python3
"""Run the original's use scan (base_player::detect_usable_object, 0x2D84C0)
and the build's (patch 23's rule, the Rust patch use_scan) on made-up ray
hits.

    python check_use_scan_026g.py <original survarium.exe> [--against <exe>]

Requires Python `unicorn`. The scan, victory_item_core::can_use and the
build's Rust run as machine code; the ray query, collision groups, the
usable objects' query, get_transform, another object's can_use and the
allocator's free are stubs. The default build of patch_026g.py must give
the original's result for a battery in reach and in view, refuse one behind
a wall or out of reach, and take one under the floor within 1 m of the feet
across, with the feet as its point. `--against` runs every case on another
build too (patch 23 as pickup_below_026g.asm, a build of main before the
port) and requires the same answer, point, frees, ray length and calls
(the port's gate; the frees compared as a set). Says nothing about which hits the real collision world
returns.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EIP, UC_X86_REG_ESP, \
    UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_FPSW

import patch_026g as base

SCAN = 0x2D84C0
GATHER = 0x2DC390      # query_objects_by_type<usable_object>: the hit's usable objects
FREE = 0xEB5B0         # mt_allocator::call_free (the Rust calls it directly; the original by the vtable)
BATTERY_TRANSFORM = 0x246ED0   # a battery's get_transform (vtable+44h of both battery vtables)
BATTERY_VTABLES = {"battery": 0xA13E8C, "client battery": 0xA0437C}   # victory_item_core's, victory_item's
ALLOCATOR = 0xA8001C
P, PHYS, VT, HITS, STATE, INV, WEAPON, REF = [0x20000000 + i * 0x1000 for i in range(8)]
STACK, STOP, STUB = 0x21008000, 0x22000000, 0x22000100
POINT = P + 0x8000
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
        self.floats(POINT, (-99.0,) * 3)                   # the point returned
        self.hits, self.centres, self.freed, self.calls, self.reach = [], {}, [], [], None
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
            assert self.read(out + 8) == ALLOCATOR, "the hits' allocator is the multithreaded one"
            self.word(out, HITS)
            self.word(out + 4, HITS + 40 * len(self.hits))
            self.calls.append(("ray", self.reach))
            self.returns(0, 24)
        elif address in (STUB + 0x10, STUB + 0x20):        # collision group; the hit's owner
            self.calls.append(("group" if address == STUB + 0x10 else "geometry", this))
            self.returns(self.read(this + 4))
        elif address == STUB + 0x30 or address == FREE:    # free
            assert this == ALLOCATOR
            self.freed.append(self.read(sp + 4))
            self.calls.append(("free", self.read(sp + 4), self.read(sp + 8), self.read(sp + 12), self.read(sp + 16)))
            self.returns(0, 16)
        elif address == STUB + 0x40:
            self.returns(REF)
        elif address == STUB + 0x50:                       # the weapon does not forbid it
            self.returns(0)
        elif address in (STUB + 0x60, STUB + 0x80):        # another usable object's can_use: yes, no
            self.calls.append(("can_use", this))
            self.returns(1 if address == STUB + 0x60 else 0, 4)
        elif address == BATTERY_TRANSFORM:                 # get_transform(out)
            out = self.read(sp + 4)
            self.calls.append(("transform", this))
            self.floats(out, (0.0,) * 12 + self.centres[this] + (1.0,))
            self.returns(out, 4)
        elif address == GATHER:
            esi = emu.reg_read(UC_X86_REG_ESI)
            assert self.read(esi + 8) == ALLOCATOR, "the list's allocator is the multithreaded one"
            assert (self.read(sp + 8), self.read(sp + 12)) == (0x2DD730, 0)
            self.calls.append(("gather", self.read(sp + 4)))
            self.word(esi, self.read(sp + 4))
            self.returns(0, 12)
        elif address == 0x300710:
            self.calls.append(("can_use", this))

    def add(self, kind, point, centre=None):
        """A hit: "wall" (walkable and hittable), "walkable", "hittable", "battery", "client battery",
        "other" (its can_use yes) or "refusing" (no)."""
        block = P + 0x10000 + len(self.hits) * 0x1000
        obj, owner, usable, table, vector, table2 = (block, block + 0x100, block + 0x300, block + 0x800,
                                                     block + 0x900, block + 0xA00)
        hit = HITS + 40 * len(self.hits)
        self.word(hit + 0x1C, obj)
        self.floats(hit, point)
        self.word(obj, table)
        self.word(table, STUB + 0x10)
        self.word(obj + 4, {"wall": 0xA, "walkable": 0x2, "hittable": 0x8}.get(kind, 0x80))
        for at, value in ((obj + 0xC, owner), (owner, table), (table + 4, STUB + 0x20), (owner + 4, vector),
                          (vector, usable)):
            self.word(at, value)
        if kind in BATTERY_VTABLES:
            self.word(usable, BATTERY_VTABLES[kind])
        else:
            self.word(usable, table2)
            self.word(table2 + 0x30, STUB + 0x80 if kind == "refusing" else STUB + 0x60)
        self.centres[usable] = tuple(centre or point)
        self.hits.append(usable)
        return usable

    def run(self, alive=True, point=True):
        emu = self.emu
        emu.mem_write(P + 0x33C, b"\1" if alive else b"\0")
        for at, value in ((STACK, STOP), (STACK + 4, P), (STACK + 8, POINT if point else 0)):
            self.word(at, value)
        emu.reg_write(UC_X86_REG_ESP, STACK)
        for register in REGISTERS:
            emu.reg_write(register, 0x12345678)
        emu.emu_start(SCAN, STOP, count=200000)
        assert emu.reg_read(UC_X86_REG_EIP) == STOP and emu.reg_read(UC_X86_REG_ESP) == STACK + 12
        assert all(emu.reg_read(register) == 0x12345678 for register in REGISTERS)
        assert emu.reg_read(UC_X86_REG_FPSW) & 0x3800 == 0, "x87 stack left non-empty"
        assert not alive or not self.hits or HITS in self.freed
        return emu.reg_read(UC_X86_REG_EAX)

    def point(self):
        return struct.unpack("<3f", self.emu.mem_read(POINT, 12))


def cases():
    """(name, hits, taken by the original, taken by patch 23, the point); taken: the last hit, or the
    index of the one, or False."""
    x, y, z = FEET
    nan = float("nan")
    floor = ("wall", (x, y, z))
    below = lambda across, depth=0.3, kind="battery": (kind, (x + across, y - depth + 0.1, z), (x + across, y - depth, z))
    yield "in view", [("battery", (x + 1, y + 0.1, z))], True, True, (x + 1, y + 0.1, z)
    yield "none", [], False, False, None
    yield "behind a wall", [("wall", (x + 0.4, y + 1, z)), ("battery", (x + 0.7, y + 0.1, z))], False, False, None
    # The stub returns hits beyond the ray's reach; the real query would not.
    yield "out of reach", [("battery", (x + 2.5, y + 0.4, z))], True, False, None
    for across, taken in ((0.0, True), (0.6, True), (0.99, True), (1.0, True), (1.01, False), (1.5, False)):
        yield f"under the floor, {across} m across", [floor, below(across)], False, taken, FEET if taken else None
    yield "the client's battery under the floor", [floor, below(0.2, kind="client battery")], False, True, FEET
    yield "two floors down", [floor, ("wall", (x, y - 1, z)), below(0, 2.3)], False, True, FEET
    yield "a walkable hit only", [("walkable", (x + 0.3, y + 0.6, z)), ("battery", (x + 0.8, y + 0.1, z))], True, True, None
    yield "walkable, then hittable: a wall", [("walkable", (x + 0.3, y + 0.6, z)), ("hittable", (x + 0.4, y + 0.5, z)),
                                              ("battery", (x + 0.8, y + 0.1, z))], False, False, None
    yield "walkable, hittable, then under the floor", [("walkable", (x + 0.3, y + 0.6, z)), ("hittable", (x, y, z)),
                                                       below(0.1)], False, True, FEET
    yield "centre above the feet", [floor, ("battery", (x, y - 0.1, z), (x, y + 0.05, z))], False, False, None
    yield "another object under the floor", [floor, ("other", (x, y - 0.2, z))], False, False, None
    yield "a NaN centre", [floor, ("battery", (x, y - 0.2, z), (nan, y - 0.3, z))], False, False, None
    yield "a NaN point", [("other", (nan, y, z))], True, True, None
    yield "another object, then a battery", [floor, ("other", (x, y - 0.1, z)), below(0)], False, True, FEET
    yield "a refusal, then one that is used", [("refusing", (x + 0.3, y + 0.5, z)), ("other", (x + 0.6, y + 0.3, z))], \
        True, True, (x + 0.6, y + 0.3, z)
    yield "a refusal only: kept for the prompt", [("refusing", (x + 0.3, y + 0.5, z)), ("wall", (x + 0.5, y, z))], \
        0, 0, (x + 0.3, y + 0.5, z)
    yield "two refusals: the first kept", [("refusing", (x + 0.3, y + 0.5, z)), ("refusing", (x + 0.4, y + 0.4, z))], \
        0, 0, (x + 0.3, y + 0.5, z)
    yield "a refusal beyond the reach: not kept", [("refusing", (x + 2.5, y + 0.4, z))], True, False, None


def outcome(scan, wanted, taken):
    if taken is False:
        return 0
    return wanted[-1] if taken is True else wanted[taken]


def run_case(data, case, alive=True, point=True):
    name, hits = case[0], case[1]
    scan = Scan(data)
    wanted = [scan.add(*hit) for hit in hits]
    eax = scan.run(alive=alive, point=point)
    # The frees as a set: the Rust frees a usable list as soon as it took its
    # first object, the game after that object's tests; nothing reads it between.
    calls = [call for call in scan.calls if call[0] != "free"]
    frees = sorted(call for call in scan.calls if call[0] == "free")
    return scan, wanted, eax, (eax, bytes(scan.emu.mem_read(POINT, 12)), scan.reach, calls, frees)


def main():
    args = sys.argv[1:]
    against = None
    if "--against" in args:
        at = args.index("--against")
        against = open(args[at + 1], "rb").read()
        del args[at:at + 2]
    original = open(args[0], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    patched, _ = base.build(original, "section", code, None, late_join)
    checks = 0
    for data, reach, column in ((original, 2.0, 2), (patched, 4.0, 3)):
        for case in cases():
            name, point = case[0], case[4]
            scan, wanted, eax, _ = run_case(data, case)
            expected = outcome(scan, wanted, case[column])
            assert eax == expected, (data is patched, name, hex(eax), hex(expected))
            assert scan.reach == reach, (name, scan.reach)
            if expected and point is not None and data is patched:
                assert scan.point() == tuple(struct.unpack("<3f", struct.pack("<3f", *point))), (name, scan.point())
            checks += 1
    for data in (original, patched):
        # A dead player: nothing, no ray, the point untouched.
        scan, _, eax, _ = run_case(data, ("dead", [("battery", (FEET[0] + 1, FEET[1] + 0.1, FEET[2]))]), alive=False)
        assert eax == 0 and scan.reach is None and scan.point() == (-99.0,) * 3
        # No point asked: the answer only.
        scan, wanted, eax, _ = run_case(data, ("no point", [("battery", (FEET[0] + 1, FEET[1] + 0.1, FEET[2]))]),
                                        point=False)
        assert eax == wanted[0] and scan.point() == (-99.0,) * 3
        checks += 2
    # Already carrying one: as the original, an unusable battery comes back
    # for the prompt (can_use is false), once the scan reaches it.
    scan = Scan(patched)
    scan.word(INV + 0x178, 1)
    scan.add(*("wall", FEET))
    battery = scan.add("battery", (FEET[0], 9.8, FEET[2]), (FEET[0], 9.7, FEET[2]))
    assert scan.run() == battery
    checks += 1
    print(f"use scan: {checks} scans as expected (the original and the build)")
    if against is None:
        return
    same = 0
    for case in cases():
        for alive, point in ((True, True), (True, False), (False, True)):
            mine = run_case(patched, case, alive, point)[3]
            theirs = run_case(against, case, alive, point)[3]
            assert mine == theirs, (case[0], alive, point, mine, theirs)
            same += 1
    print(f"use scan: {same} scans the same as the other build (answer, point, frees, ray, calls)")


if __name__ == "__main__":
    main()
