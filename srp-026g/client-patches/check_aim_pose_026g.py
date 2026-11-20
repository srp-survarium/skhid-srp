#!/usr/bin/env python3
"""Run patch 165's aim alignment (the Rust patch `aim_pose`) in the default
build beside the original, on a made-up weapon.

    python check_aim_pose_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, and
process_hands_ik (0x2959D0) is entered at its aimed branch's last two
instructions before calculate_aim_alignment (0x295AA5: the site), as the
game reaches them: the Item bone's matrix pushed, the weapon in its frame,
ECX the gun's bones. The game's own calculate_aim_alignment, calculate_locator
and matrix code run; a run stops at the call's return (0x295AB2), the
alignment in its frame (ESI).

The weapon: its aim locator on bone 1 (as a ported gun's, on `offset`), an
aim offset of -0.11 m, a user whose mixing tree holds the animations given.

- Its idle clip alone poses the gun (the idle state, one animation of this
  gun in the tree, others' beside it): the locator is moved into the model at
  that pose (locator x bone, bone FFFFh) and the alignment is the original's.
- After that, a fire clip's kick of the bone: the alignment is the one the
  original gives for the idle bones (the kick is not followed); the original
  follows it.
- Not the idle state, a second animation of the gun (a clip fading), none of
  the gun's, no tree, no user: the locator untouched, the alignment the
  original's bit for bit.
- A locator on no bone: untouched.
In every run the registers at the return are the original's.
"""
import math
import os
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI,
                               UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP)

if len(sys.argv) > 1:
    os.environ.setdefault("SRP_026G_EXE", sys.argv[1])    # a span hook's bytes (patch_026g.RUST_HOOKS)
import patch_026g as base

SITE, RETURNED = 0x295AA5, 0x295AB2
SPAN = bytes.fromhex("8b44241c8d742424")                  # mov eax,[esp+1Ch] / lea esi,[esp+24h]
THREAD_SLOT = 0x870118
HEAP, STUBS, STACK = 0x30000000, 0x31000000, 0x32000000
WEAPON, STATE, USER, TREE, RECORDS, NODES, BONES, ITEM, OTHER = (HEAP + 0x1000 * n for n in (0, 2, 3, 0x13, 0x14, 0x16, 0x17, 0x18, 0x19))
SP = STACK + 0x8000
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP)
STEADY = {UC_X86_REG_EAX: 0x0A0A0A0A, UC_X86_REG_EBX: 3, UC_X86_REG_EDX: 0x0D0D0D0D, UC_X86_REG_ESI: 0x05050505,
          UC_X86_REG_EDI: 0x0E0E0E0E, UC_X86_REG_EBP: STACK + 0x9000}
IDLE, FIRE, NO_BONE, RECORD = 3, 5, 0xFFFF, 0xB0

failed = 0


def check(ok, what):
    global failed
    failed += not ok
    print(f"  {'ok' if ok else 'FAIL'}  {what}")


def matrix(turn_x=0.0, place=(0.0, 0.0, 0.0)):
    """A row-vector 4x4: a turn about x, then a move."""
    c, s = math.cos(turn_x), math.sin(turn_x)
    return (1, 0, 0, 0, 0, c, s, 0, 0, -s, c, 0, *place, 1)


def product(a, b):
    return tuple(sum(a[r * 4 + k] * b[k * 4 + c] for k in range(4)) for r in range(4) for c in range(4))


def near(a, b, by=2e-6):
    return all(abs(x - y) <= by for x, y in zip(a, b))


LOCATOR = matrix(0.009, (0.0089, 0.1019, 0.1328))         # the aim's place on its bone
ROOT = matrix()                                            # bone 0
IDLE_BONE = matrix(-0.0148, (0.0088, 0.0214, 0.05))        # bone 1 as the idle clip poses it
KICKED_BONE = matrix(-0.0148 + 0.02, (0.0088, 0.0234, 0.05 - 0.021))   # 21 mm back, 2 mm up, 1.1 degrees
AIM_OFFSET = matrix(place=(0.0, 0.0, -0.11))
ITEM_BONE = matrix(0.1, (100.0, 6.5, -50.0))               # the camera


class Game:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(HEAP, 0x20000, UC_PROT_ALL)
        for at in (STUBS, STACK):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        self.u32(THREAD_SLOT, STUBS + 0x80)
        emu.mem_write(STUBS + 0x80, b"\xB8\x01\x00\x00\x00\xC3")

    def u32(self, at, value=None):
        if value is None:
            return struct.unpack("<I", self.emu.mem_read(at, 4))[0]
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def floats(self, at, n=16):
        return struct.unpack(f"<{n}f", self.emu.mem_read(at, 4 * n))

    def put(self, at, m):
        self.emu.mem_write(at, struct.pack("<16f", *m))

    def weapon(self, bone=1, state=IDLE, animations=(WEAPON,), tree=True, user=True):
        """A new weapon: its locator on `bone`, in `state`, its user's tree
        holding one animation for each of `animations` (whose bones it poses)."""
        emu = self.emu
        emu.mem_write(HEAP, bytes(0x20000))
        self.put(WEAPON + 0x6A0, AIM_OFFSET)
        self.put(WEAPON + 0x7A8 + 0x20, LOCATOR)
        emu.mem_write(WEAPON + 0x7A8 + 0x60, struct.pack("<H", bone))
        self.u32(WEAPON + 0x330, STATE)
        self.u32(STATE + 0x124, state)
        self.u32(WEAPON + 0x8, USER if user else 0)
        self.u32(USER + 0xE850, TREE if tree else 0)
        self.u32(TREE + 0x10, RECORDS)
        self.u32(TREE + 0x20, len(animations))
        for n, whose in enumerate(animations):
            self.u32(RECORDS + n * RECORD + 0xA8, NODES + n * 0x100)
            self.u32(NODES + n * 0x100 + 0x30, whose)
        self.put(ITEM, ITEM_BONE)

    def locator(self):
        return self.floats(WEAPON + 0x7A8 + 0x20), struct.unpack("<H", self.emu.mem_read(WEAPON + 0x7A8 + 0x60, 2))[0]

    def align(self, bone):
        """A frame's alignment with bone 1 at `bone`: (the matrix the call
        left in the frame, the registers at its return)."""
        emu = self.emu
        self.put(BONES, ROOT)
        self.put(BONES + 0x40, bone)
        emu.mem_write(SP, bytes(0x100))
        self.u32(SP, ITEM)                                  # the pushed Item bone
        self.u32(SP + 0x14, 0x77770000)                     # the controller: not read
        self.u32(SP + 0x1C, WEAPON)
        for reg, value in STEADY.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ECX, BONES)
        emu.reg_write(UC_X86_REG_ESP, SP)
        emu.emu_start(SITE, RETURNED, count=200000)
        regs = {r: emu.reg_read(r) for r in REGS}
        return self.floats(SP + 0x24), regs


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    built = bytes(built)
    linked = base.arena()
    installs = {h[0]: h for h in base.rust_hooks(original)}
    va, old, symbol, opcode = installs[SITE]
    at = base.file_offset(va)
    new = base.hook_bytes(linked.symbols[symbol], va, old, 0, opcode)
    check(opcode == 0xE9 and old == SPAN and original[at:at + 8] == SPAN and built[at:at + len(new)] == new,
          f"{va:#x}: {old.hex()} in the original, `jmp {symbol}` here")

    stock, ours = Game(original), Game(built)

    def both(pose, **weapon):
        stock.weapon(**weapon)
        ours.weapon(**weapon)
        return stock.align(pose), ours.align(pose)

    # the idle clip alone: the locator goes into the model at the idle pose
    (s_out, s_regs), (o_out, o_regs) = both(IDLE_BONE, animations=(OTHER, WEAPON, OTHER))
    check(s_regs[UC_X86_REG_EIP] == RETURNED and o_regs == s_regs, "the idle gun: both return, the registers the original's")
    place, bone = ours.locator()
    check(bone == NO_BONE and near(place, product(LOCATOR, IDLE_BONE)),
          "the idle gun: the locator on no bone, at locator x the idle bone")
    check(stock.locator() == (struct.unpack("<16f", struct.pack("<16f", *LOCATOR)), 1), "the original leaves its locator")
    check(near(o_out, s_out), "the idle gun: the alignment is the original's")
    idle_out = s_out

    # then a shot: the bone kicked, the fire state, the fire clip and the fading idle one
    for game in (stock, ours):
        game.u32(STATE + 0x124, FIRE)
    posed = ours.locator()
    (s_out, s_regs), (o_out, o_regs) = stock.align(KICKED_BONE), ours.align(KICKED_BONE)
    check(o_regs == s_regs and ours.locator() == posed, "a shot after it: the registers the original's, the locator kept")
    check(near(o_out, idle_out), "a shot after it: the alignment is the idle pose's (the kick is not followed)")
    moved = math.dist(s_out[12:15], idle_out[12:15])
    check(0.015 < moved < 0.03, f"the original follows the kick ({moved * 1000:.1f} mm)")

    # nothing else poses a locator
    for what, weapon in (("the fire state", dict(state=FIRE)),
                         ("a second animation of the gun (a clip fading)", dict(animations=(WEAPON, WEAPON))),
                         ("no animation of the gun", dict(animations=(OTHER,))),
                         ("an empty tree", dict(animations=())),
                         ("no tree", dict(tree=False)),
                         ("no user", dict(user=False)),
                         ("a locator on no bone", dict(bone=NO_BONE))):
        (s_out, s_regs), (o_out, o_regs) = both(KICKED_BONE, **weapon)
        check(o_out == s_out and o_regs == s_regs and ours.locator() == stock.locator(),
              f"{what}: the locator untouched, the alignment and the registers the original's bit for bit")
    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
