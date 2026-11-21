#!/usr/bin/env python3
"""Patch 176 (breath_hit): a hit that takes health breaks the victim's breath
hold (docs/notes/breath-hit.md).

    python check_breath_hit_026g.py <original survarium.exe>

Requires Python `unicorn`. In the patched exe, on made-up players and guns:

1. The hook at hit_impl's `player.hit_applied` (0x2D9CB1, after patch 84's
   handler there) run to the je after it (0x2D9CB7): a victim whose held gun
   (the client's weapon or weapon_core) is in the holding state gets its
   reserve (calculator +50h) 0, for every hit type; nothing written for the
   normal or the short breathing state, another held item, no held item or
   a hit type the game has no name for; patch 84 still stops a drug in use
   at the same hit; the replayed `mov edi,esi / cmp`, the registers and the
   stack kept.
2. The game's own breath machine after it: the calculator made by
   initialize_logic (0x31D450, the allocator 0xE4F90 a bump of ours), its
   states' holder set as set_user does, ticked by
   breath_vibration_calculator::tick (0x31D6E0) with apply_modifier
   (0x2D5AE0) stubbed to return its value. Holding with the key held stays
   holding a tick; after the hit the next tick is in short breathing, the
   reserve refilled (holder +268h), the delay 0, the stamina untouched; with
   the key released a moment ago, the same; then no hold until the penalty
   has risen to 1 and come back to 0. Running out by itself (for the
   difference): the tick that spends the reserve empties the stamina too
   (0x31D7B0), the next is in short breathing.
The breath code's bytes are the original's (fsm::tick runs with patch 143's
lohook at 0x38CAA0, which leaves states not its own alone).
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_XMM0)

import patch_026g as base

SITE, AFTER = 0x2D9CB1, 0x2D9CB7
SET_ACTIVE, APPLY_MODIFIER, MALLOC = 0x33D390, 0x2D5AE0, 0xE4F90
INITIALIZE_LOGIC, CALC_TICK = 0x31D450, 0x31D6E0     # EDI the calculator; ret / (dispersion, ms) ret 8
ORIGINAL_CODE = ((0x31D040, 0x31D970),)   # fsm::tick has patch 143's lohook (0x38CAA0), for its own states only
WEAPON_VTABLE, WEAPON_CORE_VTABLE, MEDKIT_VTABLE = 0xA04174, 0xA11094, 0xA1C79C
NORMAL, HOLDING, SHORT = 0xA16F64, 0xA16FAC, 0xA16F88
THREAD_SLOT = 0x870118
AIM_HOLD = 0x80 | 0x100000

DATA = 0x30000000
PLAYER, HIT, GUN, HEAP, INVENTORY, DRUG = (DATA + 0x2000 * i for i in range(6))
CALC = GUN + 0x288
STUBS = DATA + 0x30000
THREAD, RETURN = STUBS + 0x20, STUBS + 0x40
STACK = 0x21000000
GAME = {UC_X86_REG_EAX: 0x0A0A0A0A, UC_X86_REG_ECX: 0x0C0C0C0C, UC_X86_REG_EDX: 0x0D0D0D0D,
        UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_ESI: 0x05050505, UC_X86_REG_EDI: 0x0E0E0E0E,
        UC_X86_REG_EBP: 0x0019FF00}
REGS = tuple(GAME)
HOLD_TIME = 3.0            # the holder's +268h, breath_holding_time
STAMINA = 0.5              # the holder's stamina (+6C0h, its +6Ch)


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(DATA, 0x40000, UC_PROT_ALL)
        emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
        self.stops, self.calls = (), []
        emu.hook_add(UC_HOOK_CODE, self.stop)
        for stub in (SET_ACTIVE, APPLY_MODIFIER, MALLOC):
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=stub, end=stub)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def float(self, at, value):
        self.emu.mem_write(at, struct.pack("<f", value))

    def read(self, at, size=4):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0] if size == 4 else bytes(self.emu.mem_read(at, size))

    def readf(self, at):
        return struct.unpack("<f", self.emu.mem_read(at, 4))[0]

    def fresh(self):
        self.emu.mem_write(DATA, bytes(0x40000))
        self.emu.mem_write(STUBS, bytes([0xC3]) * 0x40)
        self.emu.mem_write(THREAD, bytes.fromhex("b801000000c3"))     # GetCurrentThreadId: mov eax,1 / ret
        self.word(THREAD_SLOT, THREAD)
        self.next = HEAP

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        pops = 0
        if address == SET_ACTIVE:
            self.calls.append(("set_active", emu.reg_read(UC_X86_REG_EAX) & 0xFF, self.read(sp + 4)))
            pops = 8
        elif address == APPLY_MODIFIER:          # value, weight; ret 8: no modifier set
            emu.reg_write(UC_X86_REG_XMM0, self.read(sp + 4))
            pops = 8
        else:                                    # malloc: EAX the size; the name; ret 4
            at = self.next
            self.next = (at + emu.reg_read(UC_X86_REG_EAX) + 15) & ~15
            emu.reg_write(UC_X86_REG_EAX, at)
            pops = 4
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def stop(self, emu, address, size, _):
        if address in self.stops and self.started:
            self.stopped = address
            emu.emu_stop()
        self.started = True

    def run(self, start, stops, sp, regs):
        for reg, value in regs.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.stops, self.stopped, self.started, self.calls = stops, None, False, []
        self.emu.emu_start(start, 0, count=500000)
        return {reg: self.emu.reg_read(reg) for reg in REGS}, self.emu.reg_read(UC_X86_REG_ESP)

    def call(self, at, args, regs):
        sp = STACK + 0x8000 - 4 - 4 * len(args)
        self.emu.mem_write(sp, struct.pack("<I", RETURN) + b"".join(struct.pack("<I", a) for a in args))
        _, after = self.run(at, (RETURN,), sp, {**GAME, **regs})
        assert self.stopped == RETURN, hex(self.stopped or 0)
        return after - sp

    # the victim and his gun

    def victim(self, held=GUN, vtable=WEAPON_VTABLE, kind=0, keys=AIM_HOLD):
        self.word(PLAYER + 0x128, held)
        self.word(GUN, vtable)
        self.word(HIT + 0x1C, kind)
        self.word(HIT + 0x3C, 1000)
        self.word(HIT + 0x40, 0x55)
        self.emu.mem_write(PLAYER + 0x33C, b"\x01")
        self.word(PLAYER + 0x330, keys)
        self.float(PLAYER + 0x268, HOLD_TIME)
        for at in (0x26C, 0x270, 0x278, 0x27C, 0x288, 0x6D4):    # params' times, the stamina's maximum
            self.float(PLAYER + at, 1.0)
        self.float(PLAYER + 0x72C, STAMINA)
        self.word(PLAYER + 0x6B8, DATA + 0x3F000)
        self.emu.mem_write(GUN + 0x5E8, b"\x01")
        for at in (0x27C, 0x280, 0x284, 0x288 + 0x40):
            self.float(GUN + at, 1.0)

    def machine(self):
        """The game's own machine (initialize_logic), its holder set as set_user does; holding."""
        self.call(INITIALIZE_LOGIC, (), {UC_X86_REG_EDI: CALC})
        states, at = [], self.read(CALC + 8)
        while at:
            states.append(at)
            self.word(at + 0x1C, PLAYER)
            self.word(at + 0x20, GUN + 0x27C)
            at = self.read(at + 4)
        assert [self.read(s) for s in states] == [NORMAL, HOLDING, SHORT], [hex(self.read(s)) for s in states]
        self.word(CALC + 0x38, GUN)
        self.word(CALC + 0x3C, PLAYER)
        self.states = dict(zip((NORMAL, HOLDING, SHORT), states))
        self.enter(HOLDING, reserve=2.0, delay=1.0)

    def enter(self, state, reserve, delay, penalty=0.0):
        self.word(CALC + 0x10, self.states[state])
        self.float(CALC + 0x50, reserve)
        self.float(CALC + 0x54, 0.5)
        self.float(CALC + 0x58, penalty)
        self.float(CALC + 0x5C, delay)

    def state(self):
        return self.read(self.read(CALC + 0x10))

    def tick(self, ms=100):
        assert self.call(CALC_TICK, (struct.unpack("<I", struct.pack("<f", 0.2))[0], ms), {UC_X86_REG_EDI: CALC}) == 12

    def hit(self, writes=None):
        before = self.read(DATA, 0x40000)
        regs = {**GAME, UC_X86_REG_EBX: PLAYER, UC_X86_REG_ESI: HIT}
        after, sp = self.run(SITE, (AFTER,), STACK + 0x8000, regs)
        assert self.stopped == AFTER and sp == STACK + 0x8000, (hex(self.stopped or 0), hex(sp))
        assert after == {**regs, UC_X86_REG_EDI: HIT}, {hex(k): hex(v) for k, v in after.items()}
        now = self.read(DATA, 0x40000)
        changed = {i for i in range(0, len(now), 4) if now[i:i + 4] != before[i:i + 4]}
        assert changed == set(writes or ()), sorted(hex(DATA + i) for i in changed)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    r = Run(bytes(built))
    o = Run(original)
    for lo, hi in ORIGINAL_CODE:
        assert r.read(lo, hi - lo) == o.read(lo, hi - lo), hex(lo)
    print("ok the breath code: the original's bytes")
    reserve = CALC + 0x50 - DATA

    print("the hook (patched exe, player.hit_applied 0x2D9CB1)")
    for kind in range(8):
        for vtable in (WEAPON_VTABLE, WEAPON_CORE_VTABLE):
            r.fresh()
            r.victim(vtable=vtable, kind=kind)
            r.machine()
            r.hit(writes=[reserve])
            assert r.readf(CALC + 0x50) == 0.0
    print("  ok a held gun holding its breath: its reserve 0, every hit type, the client's gun and weapon_core")
    for state in (NORMAL, SHORT):
        r.fresh()
        r.victim()
        r.machine()
        r.enter(state, reserve=2.0, delay=0.0, penalty=0.5)
        r.hit()
    print("  ok normal, short breathing: nothing")
    for label, held, vtable, kind in (("another item", GUN, MEDKIT_VTABLE, 0), ("nothing held", 0, WEAPON_VTABLE, 0),
                                      ("a hit type with no name", GUN, WEAPON_VTABLE, 8)):
        r.fresh()
        r.victim(held=held, vtable=vtable, kind=kind)
        r.machine()
        r.hit()
        print("  ok", label + ": nothing")
    r.fresh()
    r.victim()
    r.machine()
    r.word(PLAYER + 0x10C, INVENTORY)
    r.word(INVENTORY + 0x10C, DRUG)
    r.word(DRUG, MEDKIT_VTABLE)
    r.emu.mem_write(DRUG + 0x2B0, b"\x01")
    r.emu.mem_write(DRUG + 0x288, b"\x01")
    r.word(DRUG + 0x280, DRUG + 0x400)
    r.emu.mem_write(DRUG + 0x400, b"body\0")
    r.float(DRUG + 0x410, 0.25)
    r.hit(writes=[reserve])
    assert r.calls == [("set_active", 0, DRUG)], r.calls
    print("  ok patch 84 at the same hit: the drug stopped, then the breath")

    print("the game's machine after it (patched exe, the original's code)")
    r.fresh()
    r.victim()
    r.machine()
    r.tick()
    assert r.state() == HOLDING and abs(r.readf(CALC + 0x50) - 1.9) < 1e-6
    print("  ok no hit: holding, the reserve 0.1 s less")
    for label, keys, delay in (("the key held", AIM_HOLD, 1.0), ("the key up a moment ago", 0x80, 0.5)):
        r.fresh()
        r.victim(keys=keys)
        r.machine()
        r.enter(HOLDING, reserve=2.0, delay=delay)
        r.hit(writes=[reserve])
        r.tick()
        assert r.state() == SHORT, (label, hex(r.state()))
        assert r.readf(CALC + 0x50) == HOLD_TIME and r.readf(CALC + 0x5C) == 0.0, label
        assert r.readf(PLAYER + 0x72C) == f32(STAMINA), label
        print(f"  ok {label}: the next tick in short breathing, the reserve refilled, the delay 0, the stamina kept")
    r.word(PLAYER + 0x330, AIM_HOLD)
    seen, ticks = [], 0
    while r.state() != HOLDING:
        seen.append((r.state(), r.readf(CALC + 0x58)))
        r.tick()
        ticks += 1
        assert ticks < 100
    assert seen[0][0] == SHORT and any(s == NORMAL and p > 0 for s, p in seen)
    assert all(s != HOLDING for s, _ in seen) and seen[-1] == (NORMAL, 0.0), seen[-3:]
    print(f"  ok the key held: short breathing to a penalty of 1, normal back to 0, a hold only then ({ticks} ticks)")

    print("running out by itself (the original exe)")
    o.fresh()
    o.victim()
    o.machine()
    o.enter(HOLDING, reserve=0.05, delay=1.0)
    o.tick()
    assert o.state() == HOLDING and o.readf(CALC + 0x50) == 0.0 and o.readf(PLAYER + 0x72C) == 0.0
    o.tick()
    assert o.state() == SHORT and o.readf(CALC + 0x50) == HOLD_TIME
    print("  ok the tick that spends the reserve empties the stamina, the next is in short breathing")


if __name__ == "__main__":
    main()
