#!/usr/bin/env python3
"""Patch 197's gun behind a turn (camera_trail.rs, "The gun has weight on a turn").

    python check_gun_sway_026g.py <original survarium.exe>

On check_head_clearance_026g's rig, in the original and in the built exe:
- process_hands_ik from where its ways join (0x295B00, `hands_ik.solve`) to
  its call of the hands' IK (0x295B17), the first person's, a client weapon in
  hand, the holder's bones of the frame in memory: with no turn before it the
  bones and the registers are the original's;
- update_camera for two frames 16 ms apart, the eyes turned 2 degrees to the
  right between them, then the same stretch of process_hands_ik: the
  Weapon bone is swung about the Item bone's place to the left by this file's
  own arithmetic of the arms' lag (0.872 degrees), every other bone and the
  registers the original's; the view of the second frame is the original's
  (the view does not lag);
- fully aimed, in the third person and with another item in hand: the bones
  the original's.
"""
import math
import struct
import sys

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_EIP

import check_head_clearance_026g as rig

SOLVE, CALL = 0x295B00, 0x295B17
SPAN = bytes.fromhex("8b550c8b442414")                    # mov edx,[ebp+0Ch] / mov eax,[esp+14h]
WEAPON_VTABLE, WEAPON_CORE_VTABLE = 0xA04174, 0xA11094
CONTROLLER, BONES, HOLDER, SKELETON, NAMES, WEAPON = (rig.HEAP + 0x28000 + 0x800 * n for n in (0, 1, 2, 3, 5, 8))
FRAME, ARGS = rig.STACK + 0x6000, rig.STACK + 0x7000
NAMED = ("object_movement", "Root", "Weapon", "Item")
SMOOTHING, ARMS_LAG, SWAY = 25.0, 20.0, 0.5
failed = 0

ROOT = (1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0.0, 0.9, 0.0, 1)
GUN = (0.98, 0.05, -0.19, 0, -0.04, 0.998, 0.06, 0, 0.19, -0.05, 0.98, 0, 0.21, 1.35, 0.42, 1)
EYE = (1, 0, 0, 0, 0, 0.995, -0.0998, 0, 0, 0.0998, 0.995, 0, 0.0, 1.62, 0.13, 1)


def check(ok, what):
    global failed
    failed += not ok
    print(("  ok  " if ok else "  FAIL  ") + what)


def about(v, axis, angle):
    s, c = math.sin(angle), math.cos(angle)
    cross = (axis[1] * v[2] - axis[2] * v[1], axis[2] * v[0] - axis[0] * v[2], axis[0] * v[1] - axis[1] * v[0])
    dot = sum(a * b for a, b in zip(axis, v))
    return tuple(x * c + k * s + a * dot * (1 - c) for x, k, a in zip(v, cross, axis))


def swung(gun, eye, yaw, pitch):
    pivot, right = eye[12:15], eye[0:3]
    swing = lambda v: about(about(v, (0, 1, 0), yaw), right, -pitch)
    rows = [swing(gun[r * 4:r * 4 + 3]) for r in range(3)]
    at = tuple(p + d for p, d in zip(pivot, swing(tuple(g - p for g, p in zip(gun[12:15], pivot)))))
    return (*rows[0], 0, *rows[1], 0, *rows[2], 0, *at, 1)


class Hands:
    def __init__(self, data):
        self.game = rig.Game(data)
        self.yaw = None
        self.ms = 0
        self.game.emu.hook_add(UC_HOOK_CODE, self.turn, begin=rig.UPDATE_CAMERA, end=rig.UPDATE_CAMERA)

    def turn(self, emu, *_):
        if self.yaw is not None:
            c, s = math.cos(self.yaw), math.sin(self.yaw)
            emu.mem_write(rig.PLAYER + 0x44C, struct.pack("<16f", c, 0, -s, 0, 0, 1, 0, 0, s, 0, c, 0, *rig.EYE_AT, 1))

    def frame(self, yaw, ms):
        """A frame's camera with the eyes turned `yaw` to the right about the up."""
        self.yaw = yaw
        self.ms += ms
        view, regs, _ = self.game.update_camera(0, ms=self.ms)
        return view, regs

    def solve(self, first_view=1, vtable=WEAPON_VTABLE, raised=0.0):
        """process_hands_ik's last stretch: (the three bones after it, the registers at the IK's call)."""
        emu, u32 = self.game.emu, self.game.u32
        emu.mem_write(CONTROLLER, bytes(0x4000))
        u32(CONTROLLER + 0x1F8, rig.PLAYER)
        u32(HOLDER, SKELETON)
        u32(SKELETON + 0x108, len(NAMED))
        for n, name in enumerate(NAMED):
            record = SKELETON + 0x110 + 0x1C * n
            emu.mem_write(NAMES + 0x20 * n, name.encode() + b"\0")
            u32(record, NAMES + 0x20 * n)
            u32(record + 8, record + 0x1C if n + 1 < len(NAMED) else 0)
        for n, bone in enumerate((ROOT, GUN, EYE)):
            emu.mem_write(BONES + 0x40 * n, struct.pack("<16f", *bone))
        u32(WEAPON, vtable)
        emu.mem_write(WEAPON + 0x5FC, struct.pack("<f", raised))
        emu.mem_write(FRAME, struct.pack("<8I", 0, 0, 0, 0, CONTROLLER, first_view, WEAPON, 0))
        emu.mem_write(ARGS, struct.pack("<6I", 0, 0, 0x5A5A0008, BONES, 0x5A5A0010, HOLDER))
        for reg, value in rig.STEADY.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESI, CONTROLLER)
        emu.reg_write(UC_X86_REG_EBP, ARGS)
        emu.reg_write(UC_X86_REG_ESP, FRAME)
        emu.emu_start(SOLVE, CALL, count=200000)
        regs = {r: emu.reg_read(r) for r in rig.REGS + (UC_X86_REG_EIP,)}
        return [self.game.floats(BONES + 0x40 * n, 16) for n in range(3)], regs


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(rig.base.default_bin("section"), "rb").read()
    built, _ = rig.base.build(original, "section", code, None, open(rig.base.late_join_bin(), "rb").read())
    built = bytes(built)
    linked = rig.base.arena()
    va, old, symbol, opcode = {h[0]: h for h in rig.base.rust_hooks(original)}[SOLVE]
    at = rig.base.file_offset(va)
    new = rig.base.hook_bytes(linked.symbols[symbol], va, old, 0, opcode)
    check(opcode == 0xE9 and old == SPAN and original[at:at + len(SPAN)] == SPAN and built[at:at + len(new)] == new,
          f"{va:#x}: {old.hex()} in the original, `jmp {symbol}` here")

    stock, ours = Hands(original), Hands(built)
    posed = [struct.unpack("<16f", struct.pack("<16f", *bone)) for bone in (ROOT, GUN, EYE)]
    s_bones, s_regs = stock.solve()
    o_bones, o_regs = ours.solve()
    check(s_bones == posed and s_regs[UC_X86_REG_EIP] == CALL, "the original leaves the bones")
    check(o_bones == s_bones and o_regs == s_regs, "no turn before it: the bones and the registers the original's")

    # two frames 16 ms apart, the eyes 2 degrees to the right between them
    turn = math.radians(2.0)
    for hands in (stock, ours):
        hands.frame(0.4, 1000)
    s_view, s_cam = stock.frame(0.4 + turn, 16)
    o_view, o_cam = ours.frame(0.4 + turn, 16)
    check(o_view == s_view and o_cam == s_cam, "the turn's frame: the view and the registers the original's (the view does not lag)")
    smooth = turn * min(0.016 * SMOOTHING, 1.0)
    arms = smooth * min(min(0.016, 1 / 30) * ARMS_LAG, 1.0)
    behind = (arms - turn) * SWAY
    want = struct.unpack("<16f", struct.pack("<16f", *swung(GUN, EYE, behind, 0.0)))
    s_bones, s_regs = stock.solve()
    o_bones, o_regs = ours.solve()
    near = lambda a, b: all(abs(x - y) < 2e-5 for x, y in zip(a, b))
    check(s_bones == posed, "after the turn the original leaves the bones")
    check(near(o_bones[1], want) and o_bones[1] != posed[1],
          f"after the turn: the Weapon bone swung {math.degrees(-behind):.3f} degrees to the left about the Item bone's place")
    check(o_bones[0] == posed[0] and o_bones[2] == posed[2] and o_regs == s_regs, "the other bones and the registers the original's")
    for what, hand in (("fully aimed", dict(raised=1.0)), ("the third person's", dict(first_view=0)),
                       ("another item in hand", dict(vtable=WEAPON_CORE_VTABLE))):
        s_bones, s_regs = stock.solve(**hand)
        o_bones, o_regs = ours.solve(**hand)
        check(o_bones == posed and o_bones == s_bones and o_regs == s_regs, f"{what}: the bones and the registers the original's")
    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
