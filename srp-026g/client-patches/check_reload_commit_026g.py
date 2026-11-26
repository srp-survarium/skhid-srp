#!/usr/bin/env python3
"""Patch 205 (reload_commit): a magazine reload counts the new ammunition at its
clip's reload_commit event (the seat), so a reload left after it keeps the new
magazine (docs/notes/reload-commit.md).

    python check_reload_commit_026g.py <original survarium.exe>

The game's own reload state runs whole in Unicorn, in the original and in the
patched exe, on a made-up weapon, its user and the user's animation player:
the reload's initialize (its vtable slot 1), its finalize (slot 3, what a
melee, a sprint, a weapon switch or a grenade does to it: the state machine
leaves the state) and its deserialize (slot 5, a world read mid-reload), the
game's subscribe_event_channel / unsubscribe_event_channel on the player's
real channel list, and the events delivered as the animation player does:
each subscription of a channel's name entered through its functor's invoker
(the manager's slot 1) with an animation_callback_params. The clip's end is
the game's own "\\x01" channel (on_animation_end 0x30DB60, then
on_animation_end_impl 0x310E40 and load_magazine 0x2E17D0); the seat is the
content's reload_commit channel (domain 1). Nothing of the game is stubbed.

For every reload class (the AK-74M's sound-handler 303 state 0xA04DEC, the
pistol 314 one 0xA051CC, a double barrel's 311 one 0xA050D4, the plain 314
0xA151FC):
1. the full reload: the seat, then the end: the magazine full, the stack less
   by what it took, the end loading nothing more; the reload unsubscribed;
2. a melee (the reload left) before the seat, at 10 %, 30 % and 45 % of the
   clip: the old magazine as it was, the stack too (both exes);
3. a melee after the seat, at 55 %, 80 % and 95 %: the original keeps the
   old magazine (the bug: the new one seen in the gun, not counted), the
   patched exe the new one; no round made or lost (magazine + chamber +
   stack the same);
4. an empty reload, a tactical one (rounds left), a stack short of a
   magazine (all it has goes in), a stack of none;
5. a chamber-on-reload weapon (the VSSK's, +5F2h) with a round chambered:
   the initialize puts it back into the magazine, the seat loads and
   chambers again, the end changes nothing (without patch 205's end-load
   hook it would take one more round and lose one);
6. death after the seat: the respawn's unload_ammo (0x2E18E0) gives the
   stack the magazine and the chamber, no round lost either way;
7. a world read mid-reload (deserialize, the late joiner's game): the
   reload subscribed again, its seat counting;
8. an event of another object, or of another domain: nothing.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

RELOAD_VTABLES = {"303 (sound handler)": 0xA04DEC, "314 (sound handler)": 0xA051CC,
                  "311 (sound handler)": 0xA050D4, "314 (plain)": 0xA151FC}
UNLOAD_AMMO = 0x2E18E0                    # eax the weapon
CHANNELS_LIST, CHANNELS_ROOM, CHANNELS_LEFT = 0xE108, 0xE10C, 0xE110
THREAD_SLOT = 0x870118                    # GetCurrentThreadId's IAT slot

DATA = 0x30000000
PLAYER = DATA                             # its animation player's channels at +750h (0xF000 bytes)
WEAPON = DATA + 0x10000
STATE = DATA + 0x14000
AMMO, AMMO_DATA = DATA + 0x16000, DATA + 0x16800
PARAMS, READER, BYTES = DATA + 0x17000, DATA + 0x17400, DATA + 0x17800
ROOM = DATA + 0x20000                     # the animation player's room for its channel records
STUBS = DATA + 0x30000
THREAD, RETURN = STUBS, STUBS + 0x10
STACK = 0x21000000
EVENTS = PLAYER + 0x750

CAPACITY = 30
HANDLE = 0x0BADC0DE                       # the clip the state waits the end of


class Machine:
    def __init__(self, image):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(image).sections(image):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(image[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(DATA, 0x40000, UC_PROT_ALL)
        emu.mem_map(STACK, 0x20000, UC_PROT_ALL)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def byte(self, at, value):
        self.emu.mem_write(at, bytes([value & 0xFF]))

    def read(self, at, size=4):
        return int.from_bytes(bytes(self.emu.mem_read(at, size)), "little")

    def call(self, va, ecx=0, args=(), eax=0):
        """`va` entered with ECX, EAX and the stack arguments, run to its return."""
        sp = STACK + 0x1F000
        for value in reversed(list(args)):
            sp -= 4
            self.word(sp, value)
        sp -= 4
        self.word(sp, RETURN)
        regs = {UC_X86_REG_ESP: sp, UC_X86_REG_ECX: ecx, UC_X86_REG_EAX: eax, UC_X86_REG_EBX: 0x0B0B0B0B,
                UC_X86_REG_ESI: 0x05050505, UC_X86_REG_EDI: 0x0E0E0E0E, UC_X86_REG_EBP: 0x0019FF00,
                UC_X86_REG_EDX: 0x0D0D0D0D}
        for reg, value in regs.items():
            self.emu.reg_write(reg, value)
        self.emu.emu_start(va, RETURN, count=2_000_000)
        kept = {r: self.emu.reg_read(r) for r in (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)}
        assert kept == {r: regs[r] for r in kept}, f"{va:#x} kept no callee-saved register: {kept}"
        return self.emu.reg_read(UC_X86_REG_EAX)

    # ---- the made-up objects ----

    def setup(self, vtable, loaded, stack, chamber_on_reload=False, chambered=False):
        self.emu.mem_write(DATA, bytes(0x40000))
        self.emu.mem_write(STUBS, bytes([0xC3]) * 0x20)
        self.emu.mem_write(THREAD, bytes.fromhex("b801000000c3"))     # GetCurrentThreadId: mov eax,1 / ret
        self.word(THREAD_SLOT, THREAD)
        self.word(EVENTS + CHANNELS_ROOM, ROOM)
        self.word(EVENTS + CHANNELS_LEFT, 0x10000)
        self.word(WEAPON + 0x8, PLAYER)                 # its user
        self.word(WEAPON + 0x538, AMMO)                 # the retained ammunition stack
        self.byte(WEAPON + 0x5E4, CAPACITY)
        self.byte(WEAPON + 0x5E5, loaded)
        self.byte(WEAPON + 0x5F1, 1 if chambered else 0)
        self.byte(WEAPON + 0x5F2, 1 if chamber_on_reload else 0)
        self.byte(WEAPON + 0x5F5, 1)                    # m_need_to_auto_reload
        self.word(AMMO + 0xD0, 5)                       # its references: never the last
        self.word(AMMO + 0x25C, AMMO_DATA)
        self.byte(AMMO_DATA + 0x22A, 1)                 # a stack (its count at +260h)
        self.emu.mem_write(AMMO + 0x260, struct.pack("<H", stack))
        self.word(STATE, vtable)
        self.word(STATE + 0x120, WEAPON)
        self.word(STATE + 0x130, STATE + 0x144)         # its handles (every constructor)
        self.word(STATE + 0x144, HANDLE)

    def ammo(self):
        return (self.read(WEAPON + 0x5E5, 1), self.read(WEAPON + 0x5F1, 1), self.read(AMMO + 0x260, 2))

    def total(self):
        loaded, chambered, stack = self.ammo()
        return loaded + chambered + stack

    # ---- the state ----

    def method(self, slot):
        return self.read(self.read(STATE) + slot)

    def initialize(self):
        self.call(self.method(0x4), ecx=STATE)
        self.byte(STATE + 0x134, 0)                     # get_weapon_lexeme's pick: the first handle

    def finalize(self):
        self.call(self.method(0xC), ecx=STATE)

    def deserialize(self):
        """A world's bytes of the state: its end not yet, its index 0, the sounds' last index 0FFh."""
        self.emu.mem_write(BYTES, bytes([0, 0, 0xFF, 0, 0, 0]))
        self.word(READER + 4, BYTES)
        self.call(self.method(0x14), ecx=STATE, args=(READER, READER, 0))

    # ---- the animation player's channels ----

    def channel(self, name):
        """The subscriptions of the channel `name` (subscribe_event_channel's list)."""
        node = self.read(EVENTS + CHANNELS_LIST)
        while node:
            text = bytes(self.emu.mem_read(self.read(node), 32)).split(b"\0")[0]
            if text == name:
                out, record = [], self.read(node + 8)
                while record:
                    out.append(record)
                    record = self.read(record + 0x28)
                return out
            node = self.read(node + 4)
        return []

    def subscribed(self, name):
        return [r for r in self.channel(name) if self.read(r + 0x2C) == STATE and self.read(r) != 0]

    def event(self, name, obj, domain=1, clip=0):
        """The event delivered to each subscription of `name`, as the animation player does."""
        self.emu.mem_write(PARAMS, bytes(0x40))
        self.word(PARAMS, obj)
        self.word(PARAMS + 4, clip)
        self.byte(PARAMS + 0x20, domain)
        for record in self.subscribed(name):
            manager = self.read(record) & ~1
            self.call(self.read(manager + 4), args=(record + 8, PARAMS))

    def seat(self, obj=WEAPON, domain=1):
        self.event(b"reload_commit", obj, domain)

    def end(self):
        self.event(b"\x01", PLAYER, 0, HANDLE)
        assert self.read(STATE + 0x12D, 1) == 1, "the clip's end ended the reload"

    def unload(self):
        self.call(UNLOAD_AMMO, eax=WEAPON)


def topped(loaded, stack, capacity=CAPACITY):
    take = min(stack, capacity - loaded)
    return loaded + take, stack - take


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    exes = {"original": Machine(original), "patched": Machine(bytes(built))}
    old, new = exes["original"], exes["patched"]

    # The clip's timeline: the seat at 50 % (its sound), the end at 100 %.
    def reload(m, vtable, loaded, stack, leave_at, **chamber):
        """A reload started, its clip run to `leave_at` (a fraction), then left (a melee) or ended."""
        m.setup(vtable, loaded, stack, **chamber)
        m.initialize()
        if leave_at > 0.5:
            m.seat()
        if leave_at >= 1.0:
            m.end()
        m.finalize()
        return m.ammo()

    for label, vtable in RELOAD_VTABLES.items():
        print(f"reload state {label} {vtable:#x}")
        for loaded, stack, what in [(0, 120, "empty"), (12, 120, "tactical"), (3, 10, "a stack short of a magazine"),
                                    (5, 0, "a stack of none")]:
            full = topped(loaded, stack)
            for m in exes.values():
                assert reload(m, vtable, loaded, stack, 1.0) == (full[0], 0, full[1]), (label, what)
                assert not m.subscribed(b"reload_commit") and not m.subscribed(b"\x01"), "left subscribed"
            print(f"  ok the whole reload, {what}: {loaded} + {stack} -> {full[0]} in the magazine, {full[1]} left;"
                  " the end once (both exes), nothing left subscribed")
            for at in (0.10, 0.30, 0.45):
                for m in exes.values():
                    assert reload(m, vtable, loaded, stack, at) == (loaded, 0, stack), (label, what, at)
            print(f"  ok a melee at 10, 30, 45 % (before the seat), {what}: the old magazine, the stack untouched"
                  " (both exes)")
            for at in (0.55, 0.80, 0.95):
                assert reload(old, vtable, loaded, stack, at) == (loaded, 0, stack), (label, what, at)
                assert reload(new, vtable, loaded, stack, at) == (full[0], 0, full[1]), (label, what, at)
                assert new.total() == loaded + stack
            if (full[0], full[1]) != (loaded, stack):
                print(f"  ok a melee at 55, 80, 95 % (after the seat), {what}: the original {loaded} in the magazine"
                      f" (the new one lost), the patched exe {full[0]}, no round made or lost")

        # The seat then the end: the end loads nothing more; after the end, nothing more either.
        new.setup(vtable, 7, 100)
        new.initialize()
        new.seat()
        assert new.ammo() == (30, 0, 77)
        assert new.read(WEAPON + 0x5F5, 1) == 0, "the seat clears m_need_to_auto_reload as the end does"
        assert new.subscribed(b"reload_commit"), "still listening until the reload is left"
        new.seat()
        new.end()
        assert new.ammo() == (30, 0, 77)
        print("  ok the seat loads once: a second seat and the end take nothing more")

        # Another object's event, another domain: nothing.
        new.setup(vtable, 7, 100)
        new.initialize()
        new.seat(obj=WEAPON + 0x4000)
        new.seat(domain=0)
        new.seat(domain=2)
        assert new.ammo() == (7, 0, 100)
        new.seat(obj=PLAYER)
        assert new.ammo() == (30, 0, 77)
        print("  ok an event of another object or domain loads nothing; the user's own does")

        # Death after the seat: the respawn's unload gives the stack all of it.
        for m in exes.values():
            m.setup(vtable, 7, 100)
            m.initialize()
            m.seat()
            m.unload()
            assert m.ammo() == (0, 0, 107)
        print("  ok death after the seat: the respawn's unload_ammo, 107 rounds in the stack (both exes)")

        # A world read mid-reload: subscribed again, the seat counts.
        new.setup(vtable, 7, 100)
        new.deserialize()
        assert new.subscribed(b"reload_commit"), "a deserialized reload listens"
        new.seat()
        new.finalize()
        assert new.ammo() == (30, 0, 77) and not new.subscribed(b"reload_commit")
        print("  ok a world read mid-reload (a late joiner's game): subscribed again, the seat counts")

        # A weapon that chambers on reload (the VSSK's, the TOZ-122's), a round chambered.
        for m in exes.values():
            m.setup(vtable, 9, 100, chamber_on_reload=True, chambered=True)
            m.initialize()
            assert m.ammo() == (10, 0, 100), "the initialize puts the chambered round back"
        new.seat()
        assert new.ammo() == (29, 1, 80), new.ammo()
        new.end()
        assert new.ammo() == (29, 1, 80), f"the end took a round more: {new.ammo()}"
        new.finalize()
        old.end()
        old.finalize()
        assert old.ammo() == (29, 1, 80)
        print("  ok a chamber-on-reload weapon: the seat loads 29 and chambers one, the end the same (the"
              " original's one end too)")
    print("reload_commit: every check passed")


if __name__ == "__main__":
    main()
