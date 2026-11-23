#!/usr/bin/env python3
"""Patch 201 (sprint_fire.rs): a shot in a sprint.

    python check_sprint_fire_026g.py <original survarium.exe>

The simulation's five tests are run in the original and in the built exe on
made-up memory:
- player_input::is_sprinting (0x2F1220), is_trying_to_sprint (0x2FC1F0) and
  sprint_predicate (0x2FBC90), each to where its key test passes or refuses:
  SPRINT and FORWARD sprint; with FIRE the original refuses and the build
  passes; with BACK or CROUCH, or without SPRINT or FORWARD, both refuse;
- weapon_core's fire predicate (0x2E3CB0), to its return: with no sprint's
  clip to wait for both fire; in a sprint (its clip selected, the posture the
  sprint) the original refuses and the build fires; after a sprint (its clip
  still running) both refuse; without the FIRE action both refuse;
- weapon_core::can_sprint (0x2E3A10): idle sprints in both; the fire state
  with a sprint's clip selected sprints in the build only; the fire state
  without one, and a reload, in neither; a melee as the original's.
The registers the original's wherever the answers agree (the fire
predicate's two scratch registers aside).
"""
import os
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI,
                               UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP)

if len(sys.argv) > 1:
    os.environ.setdefault("SRP_026G_EXE", sys.argv[1])
import patch_026g as base

THREAD_SLOT = 0x870118
HEAP, STUBS, STACK = 0x30000000, 0x31000000, 0x32000000
INPUT, HOLDER, PLAYER, WEAPON, CLIPS, POSTURE, STATE, VTABLE = (HEAP + 0x1000 * n for n in (0, 1, 2, 4, 6, 7, 8, 9))
DONE = STUBS + 0xF00
SP = STACK + 0x8000
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP,
        UC_X86_REG_ESP, UC_X86_REG_EIP)
FORWARD, BACK, FIRE, CROUCH, SPRINT = 0x1, 0x2, 0x20, 0x100, 0x200
failed = 0


def check(ok, what):
    global failed
    failed += not ok
    print(("  ok  " if ok else "  FAIL  ") + what)


class Game:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(HEAP, 0x10000, UC_PROT_ALL)
        for at in (STUBS, STACK):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        self.u32(THREAD_SLOT, STUBS + 0x80)
        emu.mem_write(STUBS + 0x80, b"\xB8\x01\x00\x00\x00\xC3")
        emu.mem_write(STUBS + 0x90, b"\xB0\x01\xC2\x04\x00")           # the weapon's vtable +4: al = 1, ret 4
        # 0x2FC2A0 (can_sprint's clip set test, EDX its argument): al = 0
        emu.hook_add(UC_HOOK_CODE, self.no, begin=0x2FC2A0, end=0x2FC2A0)

    def no(self, emu, *_):
        esp = emu.reg_read(UC_X86_REG_ESP)
        emu.reg_write(UC_X86_REG_EAX, emu.reg_read(UC_X86_REG_EAX) & ~0xFF)
        emu.reg_write(UC_X86_REG_EIP, self.u32(esp))
        emu.reg_write(UC_X86_REG_ESP, esp + 4)

    def u32(self, at, value=None):
        if value is None:
            return struct.unpack("<I", self.emu.mem_read(at, 4))[0]
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def run(self, start, stops, regs):
        """From `start` to the first of `stops` (or the return): the registers there."""
        emu = self.emu
        for reg, value in ((UC_X86_REG_EAX, 0x0A0A0A0A), (UC_X86_REG_EBX, 0x0B0B0B0B), (UC_X86_REG_ECX, 0x0C0C0C0C),
                           (UC_X86_REG_EDX, 0x0D0D0D0D), (UC_X86_REG_ESI, 0x05050505), (UC_X86_REG_EDI, 0x0E0E0E0E),
                           (UC_X86_REG_EBP, STACK + 0x9000), *regs.items()):
            emu.reg_write(reg, value)
        self.u32(SP, DONE)
        emu.reg_write(UC_X86_REG_ESP, SP)
        hooks = [emu.hook_add(UC_HOOK_CODE, lambda e, *_: e.emu_stop(), begin=stop, end=stop) for stop in stops]
        emu.emu_start(start, DONE, count=200000)
        for hook in hooks:
            emu.hook_del(hook)
        return {r: emu.reg_read(r) for r in REGS}

    def keys(self, which, mask):
        """One of the three key tests under `mask`: where it went (its pass, its refusal), the registers."""
        emu = self.emu
        emu.mem_write(HEAP, bytes(0x10000))
        self.u32(INPUT + 4, mask)
        self.u32(HOLDER + 0x44, PLAYER)
        self.u32(PLAYER + 0x330, mask)
        start, passed, refused, regs = {
            "is_sprinting": (0x2F1220, 0x2F1238, 0x2F123E, {UC_X86_REG_EAX: INPUT}),
            "is_trying_to_sprint": (0x2FC1F0, 0x2FC20E, 0x2FC21E, {UC_X86_REG_EAX: HOLDER}),
            "sprint_predicate": (0x2FBC90, 0x2FBCB1, 0x2FBCD6, {UC_X86_REG_ECX: HOLDER}),
        }[which]
        out = self.run(start, (passed, refused), regs)
        return out[UC_X86_REG_EIP] == passed, out

    def weapon(self, clip, sprinting, state, mask=FIRE, motion=3):
        emu = self.emu
        emu.mem_write(HEAP, bytes(0x10000))
        self.u32(WEAPON, VTABLE)
        self.u32(VTABLE + 4, STUBS + 0x90)
        self.u32(WEAPON + 8, PLAYER)
        self.u32(PLAYER + 0x330, mask)
        self.u32(WEAPON + 0x278, CLIPS)
        self.u32(CLIPS + 0x20, POSTURE)
        self.u32(POSTURE + 0x24, motion)
        emu.mem_write(POSTURE + 0x2C, bytes([sprinting]))
        self.u32(WEAPON + 0x5EC, clip)
        self.u32(WEAPON + 0x330, STATE)
        self.u32(STATE + 0x124, state)
        emu.mem_write(WEAPON + 0x5E5, b"\x01")                  # a round to fire
        emu.mem_write(WEAPON + 0x5EA, b"\x01")                  # can_sprint's first test
        emu.mem_write(WEAPON + 0x5FC, bytes(emu.mem_read(0xA0D984, 4)))  # the aim's progress: not aimed

    def answer(self, start, **weapon):
        self.weapon(**weapon)
        out = self.run(start, (), {UC_X86_REG_ECX: WEAPON})
        return out[UC_X86_REG_EAX] & 0xFF, out


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    built = bytes(built)
    linked = base.arena()
    installs = {h[0]: h for h in base.rust_hooks(original)}
    for va, span in ((0x2F1231, "a922010000"), (0x2FC207, "a922010000"), (0x2FBCAA, "a922010000"),
                     (0x2E3CD3, "83beec05000006"), (0x2E3A43, "8b8630030000")):
        _, old, symbol, opcode = installs[va]
        at = base.file_offset(va)
        new = base.hook_bytes(linked.symbols[symbol], va, old, 0, opcode)
        check(opcode == 0xE9 and old == bytes.fromhex(span) and original[at:at + len(old)] == old and built[at:at + len(new)] == new,
              f"{va:#x}: {span} in the original, `jmp {symbol}` here")

    stock, ours = Game(original), Game(built)
    for which in ("is_sprinting", "is_trying_to_sprint", "sprint_predicate"):
        for what, mask, was, now in (("SPRINT and FORWARD", SPRINT | FORWARD, True, True),
                                     ("with FIRE", SPRINT | FORWARD | FIRE, False, True),
                                     ("with BACK", SPRINT | FORWARD | BACK, False, False),
                                     ("with CROUCH and FIRE", SPRINT | FORWARD | CROUCH | FIRE, False, False),
                                     ("FORWARD and FIRE, no SPRINT", FORWARD | FIRE, False, False),
                                     ("SPRINT and FIRE, no FORWARD", SPRINT | FIRE, False, False)):
            s_pass, s_regs = stock.keys(which, mask)
            o_pass, o_regs = ours.keys(which, mask)
            same = was != now or o_regs == s_regs
            check(s_pass == was and o_pass == now and same,
                  f"{which}, {what}: the original {'passes' if was else 'refuses'}, the build {'passes' if now else 'refuses'}"
                  + ("" if was != now else ", the registers the original's"))

    for what, weapon, was, now in (("no sprint's clip to wait for", dict(clip=6, sprinting=0, state=3, motion=0), 1, 1),
                                   ("in a sprint", dict(clip=2, sprinting=1, state=3), 0, 1),
                                   ("in a sprint, its first clip", dict(clip=0, sprinting=1, state=3), 0, 1),
                                   ("after a sprint, its clip still running", dict(clip=4, sprinting=0, state=3, motion=2), 0, 0),
                                   ("in a sprint without the FIRE action", dict(clip=2, sprinting=1, state=3, mask=0), 0, 0),
                                   ("in a jump", dict(clip=6, sprinting=0, state=3, motion=4), 0, 0)):
        s, s_regs = stock.answer(0x2E3CB0, **weapon)
        o, o_regs = ours.answer(0x2E3CB0, **weapon)
        # ECX and EDX are the predicate's own scratch: a refusal past the wait leaves other values in them
        kept = lambda regs: {r: v for r, v in regs.items() if r not in (UC_X86_REG_ECX, UC_X86_REG_EDX)}
        check(s == was and o == now and (was != now or kept(o_regs) == kept(s_regs)),
              f"the fire predicate, {what}: the original {s}, the build {o}" + ("" if was != now else ", the kept registers the original's"))

    for what, weapon, was, now in (("idle", dict(clip=6, sprinting=0, state=3), 1, 1),
                                   ("the fire state begun in a sprint", dict(clip=2, sprinting=1, state=5), 0, 1),
                                   ("the fire state with no sprint's clip", dict(clip=6, sprinting=0, state=5), 0, 0),
                                   ("a reload", dict(clip=2, sprinting=1, state=4), 0, 0),
                                   ("a melee in a sprint", dict(clip=2, sprinting=1, state=8), 1, 1),
                                   ("a melee with no sprint's clip", dict(clip=6, sprinting=0, state=8), 0, 0)):
        s, s_regs = stock.answer(0x2E3A10, **weapon)
        o, o_regs = ours.answer(0x2E3A10, **weapon)
        check(s == was and o == now and (was != now or o_regs == s_regs),
              f"can_sprint, {what}: the original {s}, the build {o}" + ("" if was != now else ", the registers the original's"))
    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
