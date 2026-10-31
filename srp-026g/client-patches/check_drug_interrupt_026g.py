#!/usr/bin/env python3
"""Patch 84 (drug_interrupt): a bullet that hits a player stops the healing
drug he is using (docs/notes/drug-interrupt.md).

    python check_drug_interrupt_026g.py <original survarium.exe>

1. The installed Rust hook at hit_impl's `player.hit_applied` (0x2D9CB1, once
   damage_model::hit_body_part said the hit took health) run in the patched
   exe to the je after it (0x2D9CB7), on made-up objects, medkit::set_active
   (0x33D390) stubbed to record its calls: a ranged hit stops each healing
   drug in use in the victim's slots, in slot order, as set_active(0, drug,
   the hit's time), whether in its use time or its active time; nothing for
   a drug not in use, the painkiller (no influences), an item of another
   vtable, a victim without an inventory, any other hit type (1..7) or one
   the game has no name for (8); the replayed `mov edi,esi / cmp dword
   [edi+40h],0` (EDI, ZF either way), every other register, the stack and
   the objects' bytes unchanged.
2. The game's own set_active(0) on a made-up drug in the original exe (what
   the stop does): in the use time it clears the use and unlinks the drug's
   tick node and tells the listeners, and leaves the drug_activity node alone
   (it was never linked); in the active time it unlinks that node too;
   between the end of the use time and the first active tick it does not;
   the healed-so-far floats zeroed, the time counters left.
apply_modifier (0x2D5AE0) is stubbed to return its value (no modifier set),
the holder's two virtual calls to return the made-up damage model and
player, the listener (0x3FE2C0) and the node unlink (0x2D5D00) to record.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EFLAGS, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP,
                               UC_X86_REG_XMM0)

import patch_026g as base

SITE, AFTER = 0x2D9CB1, 0x2D9CB7          # mov edi,esi / cmp dword [edi+40h],0 (6 bytes), then the je
SET_ACTIVE = 0x33D390                     # al active; medkit, time; ret 8
APPLY_MODIFIER, UNLINK_NODE, LISTENER_CALL = 0x2D5AE0, 0x2D5D00, 0x3FE2C0
MEDKIT_VTABLE, THREAD_SLOT = 0xA1C79C, 0x870118
ZF = 1 << 6

DATA = 0x30000000
PLAYER, INVENTORY, HIT, OTHER = DATA, DATA + 0x1000, DATA + 0x2000, DATA + 0x3000
DRUGS = DATA + 0x4000                     # drug i at DRUGS + 0x1000 x i
HOLDER, OWNER, OWNER_VT, PDM, DM, PLAYER2 = (DATA + 0x20000 + 0x400 * i for i in range(6))
NODE, TOTALS, PROFILE = DATA + 0x22000, DATA + 0x22400, DATA + 0x23000
STUBS = DATA + 0x30000                    # DM_CALL, PLAYER_CALL, THREAD
DM_CALL, PLAYER_CALL, THREAD = STUBS, STUBS + 0x10, STUBS + 0x20
RETURN = STUBS + 0x40
WORLD = 0x31000000                        # its tick lists at +18840h, 16 bytes each; list 6 +188A0h
STACK = 0x21000000
GAME = {UC_X86_REG_EAX: 0x0A0A0A0A, UC_X86_REG_ECX: 0x0C0C0C0C, UC_X86_REG_EDX: 0x0D0D0D0D,
        UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_ESI: 0x05050505, UC_X86_REG_EDI: 0x0E0E0E0E,
        UC_X86_REG_EBP: 0x0019FF00}
REGS = tuple(GAME)
HEALS, PROTECTS = 7, 0                    # influences: the medkit's seven, the painkiller's none


class Run:
    def __init__(self, data, stub_set_active):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(DATA, 0x40000, UC_PROT_ALL)
        emu.mem_map(WORLD, 0x20000, UC_PROT_ALL)
        emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
        self.calls = []
        self.stops = ()
        emu.hook_add(UC_HOOK_CODE, self.stop)
        stubs = [APPLY_MODIFIER, UNLINK_NODE, LISTENER_CALL, DM_CALL, PLAYER_CALL]
        for stub in stubs + ([SET_ACTIVE] if stub_set_active else []):
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=stub, end=stub)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at, size=4):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0] if size == 4 else bytes(self.emu.mem_read(at, size))

    def fresh(self):
        self.emu.mem_write(DATA, bytes(0x40000))
        self.emu.mem_write(WORLD, bytes(0x20000))
        self.emu.mem_write(STUBS, bytes([0xC3]) * 0x40)
        self.emu.mem_write(THREAD, bytes.fromhex("b801000000c3"))     # GetCurrentThreadId: mov eax,1 / ret
        self.word(THREAD_SLOT, THREAD)

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        pops = 0
        if address == SET_ACTIVE:
            self.calls.append(("set_active", emu.reg_read(UC_X86_REG_EAX) & 0xFF, self.read(sp + 4), self.read(sp + 8)))
            pops = 8
        elif address == APPLY_MODIFIER:          # eax the modifiers, esi the index; value, weight; ret 8
            emu.reg_write(UC_X86_REG_XMM0, self.read(sp + 4))
            pops = 8
        elif address == UNLINK_NODE:             # esi the kit's list, eax the node
            self.calls.append(("unlink", emu.reg_read(UC_X86_REG_ESI), emu.reg_read(UC_X86_REG_EAX)))
        elif address == LISTENER_CALL:           # eax the listener; time, medkit, flag; ret 0Ch
            self.calls.append(("listener", emu.reg_read(UC_X86_REG_EAX), self.read(sp + 4), self.read(sp + 8),
                               self.read(sp + 12) & 0xFF))
            pops = 12
        elif address == DM_CALL:                 # the holder's vtable +8: a pointer to the damage model
            emu.reg_write(UC_X86_REG_EAX, PDM)
        elif address == PLAYER_CALL:             # vtable +0Ch: the player
            emu.reg_write(UC_X86_REG_EAX, PLAYER2)
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


class Hook(Run):
    """The patched exe at the site, set_active stubbed."""

    def setup(self, drugs, kind=0, time=123456, weapon=0x55, inventory=True):
        """`drugs`: per slot None (empty), "other" (an item of another vtable, its +2B0h set) or
        (active, influences, delay, remaining)."""
        self.fresh()
        self.word(PLAYER + 0x10C, INVENTORY if inventory else 0)
        self.emu.mem_write(PLAYER + 0x33C, b"\x01")
        self.word(OTHER, 0xA00000)
        self.emu.mem_write(OTHER + 0x2B0, b"\x01")
        self.emu.mem_write(OTHER + 0x288, bytes([HEALS]))
        for i, drug in enumerate(drugs):
            at = DRUGS + 0x1000 * i
            if drug == "other":
                self.word(INVENTORY + 0x10C + 4 * i, OTHER)
            elif drug is not None:
                active, influences, delay, remaining = drug
                self.word(at, MEDKIT_VTABLE)
                self.emu.mem_write(at + 0x2B0, bytes([active]))
                self.emu.mem_write(at + 0x288, bytes([influences]))
                self.word(at + 0x2AC, delay)
                self.word(at + 0x2A8, remaining)
                self.word(INVENTORY + 0x10C + 4 * i, at)
        self.word(HIT + 0x1C, kind)
        self.word(HIT + 0x3C, time)
        self.word(HIT + 0x40, weapon)

    def hit(self):
        """The site to the je: the set_active calls."""
        before = self.read(DATA, 0x40000)
        regs = {**GAME, UC_X86_REG_EBX: PLAYER, UC_X86_REG_ESI: HIT}
        after, sp = self.run(SITE, (AFTER,), STACK + 0x8000, regs)
        assert self.stopped == AFTER, hex(self.stopped or 0)
        assert sp == STACK + 0x8000
        assert after == {**regs, UC_X86_REG_EDI: HIT}, {hex(k): hex(v) for k, v in after.items()}
        zf = bool(self.emu.reg_read(UC_X86_REG_EFLAGS) & ZF)
        assert zf == (self.read(HIT + 0x40) == 0), "the replayed cmp's ZF"
        assert self.read(DATA, 0x40000) == before, "the hook wrote the objects"
        return [c for c in self.calls if c[0] == "set_active"]


class Game(Run):
    """The original exe's own set_active(0) on a drug."""

    def setup(self, delay, remaining, duration=3000):
        self.fresh()
        drug = DRUGS
        self.word(drug, MEDKIT_VTABLE)
        self.word(drug + 0x248, WORLD)
        self.word(drug + 0x250, HOLDER)
        self.word(HOLDER + 0x174, OWNER)
        self.word(OWNER, OWNER_VT)
        self.word(OWNER_VT + 8, DM_CALL)
        self.word(OWNER_VT + 0xC, PLAYER_CALL)
        self.word(PDM, DM)
        self.word(DM + 0x518 + 4, NODE)          # one listener: [node] set, its next (+20h) none
        self.word(NODE, 1)
        self.emu.mem_write(PLAYER2 + 0x114, bytes([4, 1]))    # place 4, kit 1
        self.word(PLAYER2 + 0x6B8, PROFILE)
        # the tick node +268h, alone in the world's list 6 (count, head, tail)
        self.emu.mem_write(drug + 0x268 + 0x14, b"\x06")
        self.word(WORLD + 0x188A0, 1)
        self.word(WORLD + 0x188A4, drug + 0x268)
        self.word(WORLD + 0x188A8, drug + 0x268)
        self.word(drug + 0x284, TOTALS)
        self.emu.mem_write(drug + 0x288, b"\x02")
        self.emu.mem_write(TOTALS, struct.pack("<ff", 0.25, 0.5))
        self.word(drug + 0x29C, duration)
        self.emu.mem_write(drug + 0x2A0, struct.pack("<f", 1.0))
        self.word(drug + 0x2A4, 2000)
        self.word(drug + 0x2A8, remaining)
        self.word(drug + 0x2AC, delay)
        self.emu.mem_write(drug + 0x2B0, b"\x01")
        self.word(drug + 0x2BC, 3)                # its modifier's index
        return drug

    def stop_drug(self, drug, time):
        sp = STACK + 0x8000 - 12
        self.emu.mem_write(sp, struct.pack("<III", RETURN, drug, time))
        _, sp_after = self.run(SET_ACTIVE, (RETURN,), sp, {**GAME, UC_X86_REG_EAX: 0})
        assert self.stopped == RETURN and sp_after == sp + 12, (hex(self.stopped or 0), hex(sp_after))
        return self.calls


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())

    print("the hook (patched exe, player.hit_applied 0x2D9CB1)")
    h = Hook(bytes(built), stub_set_active=True)
    using, healing = (1, HEALS, 900, 3000), (1, HEALS, 0, 1200)
    for label, drug in [("in its use time", using), ("in its active time", healing),
                        ("the first active tick not yet", (1, HEALS, 0, 3000))]:
        h.setup([None, drug], time=777)
        assert h.hit() == [("set_active", 0, DRUGS + 0x1000, 777)], label
        print("  ok a ranged hit stops a healing drug", label)

    slots = [using, None, "other", (0, HEALS, 900, 3000), (1, PROTECTS, 500, 10000), healing]
    h.setup(slots, time=4242)
    assert h.hit() == [("set_active", 0, DRUGS, 4242), ("set_active", 0, DRUGS + 0x5000, 4242)]
    print("  ok two in use: both, in slot order; not one out of use, the painkiller, another item")

    for weapon in (0, 0x55):
        h.setup([healing], weapon=weapon)
        assert len(h.hit()) == 1
    print("  ok the replayed cmp: ZF either way, EDI the hit, the rest kept")

    for kind in range(1, 9):
        h.setup([using, healing], kind=kind)
        assert h.hit() == [], kind
    print("  ok blunt, slash, explosive, toxic, radiation, fire, shock and an unnamed type: nothing")

    h.setup([healing], inventory=False)
    assert h.hit() == []
    h.setup([None] * 23)
    assert h.hit() == []
    h.setup([None] * 22 + [healing], time=9)
    assert h.hit() == [("set_active", 0, DRUGS + 0x16000, 9)]
    print("  ok no inventory, empty slots: nothing; the 23rd slot is walked")

    print("the game's own set_active(0) (original exe)")
    g = Game(original, stub_set_active=False)
    for label, delay, remaining, unlinked in [("in the use time", 900, 3000, False),
                                              ("in the active time", 0, 1200, True),
                                              ("before the first active tick", 0, 3000, False)]:
        drug = g.setup(delay, remaining)
        calls = g.stop_drug(drug, 555)
        kit_list = PROFILE + 0x704 * 1 + 20 * 3 + 0x204
        assert (("unlink", kit_list, drug + 0x2B4) in calls) == unlinked, (label, calls)
        assert ("listener", NODE, 555, drug, 0) in calls, (label, calls)
        assert g.read(drug + 0x2B0, 1) == b"\x00", label
        assert g.read(TOTALS, 8) == bytes(8), label
        assert (g.read(drug + 0x2AC), g.read(drug + 0x2A8)) == (delay, remaining), label
        assert (g.read(WORLD + 0x188A0), g.read(WORLD + 0x188A4), g.read(WORLD + 0x188A8)) == (0, 0, 0), label
        assert g.read(drug + 0x268 + 0x14, 1) == b"\xff", label
        print(f"  ok {label}: the use ended, the tick node out of list 6, the listener told,"
              f" healed-so-far zeroed, the drug_activity node {'unlinked' if unlinked else 'left (never linked)'}")


if __name__ == "__main__":
    main()
