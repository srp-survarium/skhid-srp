#!/usr/bin/env python3
"""Patch 143 (the Rust patch battery_hit, rust/patches/src/battery_hit.rs):
a battery's carrier hits with it (docs/notes/battery-hit.md).

    python check_battery_hit_026g.py <original survarium.exe>

Requires Python `unicorn`. check_hide_ui_026g.py's harness (the exe's
sections and the arena loaded as they are), the original beside the build.
The game's own code runs: initialize_logic (0x301920) making a made-up
battery's take, carry and put with their transitions, the cook callback's
site after it (`battery.logic`, 0x2FA7E9), the take state's constructor
(0x338940), fsm::append_transition (0x38C970), fsm::tick (0x38CA20) with the
states' initialize, execute and finalize, the functors' invokers and the
game's carry -> put predicate (0x301B30). Stubbed: the allocator
(0xE4F90, a bump of ours), the allocation name (0x420789), a clip's time
scale (0x30D750: its time argument kept), the animation player's event
channels (subscribe 0x391C40, unsubscribe 0x3921A0), the battery's own
get_transform / set_transform (its vtable +24h, +0A4h) and emit_bullet
(0x322830: its EDI and eight arguments kept). Checked:

- the cook's site: the original leaves the three states; the patched exe
  appends a fourth, the swing (the machine 4, its list take, carry, put,
  swing), a take state of 0ECh bytes from the game's allocator with the
  take's allocation name, over the put's six clips, its time 0.3 s, its
  kind 3 (the carry's), its vtable the take's copy (RTTI and every slot the
  game's but finalize, +0Ch, ours in the arena); the carry's transitions
  put then swing (a function-pointer functor, 0xA13CAC | 1, its function
  ours, the battery its buffer's second word), the swing's put (a byte copy
  of carry -> put's functor) then carry (of take -> carry's);
- fsm::tick from the carry: the melee key down (bit 27) and alive: the
  swing, its clip waited for; the key up, another key, a dead carrier: the
  carry stays; a switch away (held != next, alive): the put, before the
  swing; the original stays in the carry with the key down;
- fsm::tick in the swing: before its clip ended nothing; once ended, back to
  the carry with one strike: emit_bullet with EDI the carrier's slot-7 gun,
  its bullet manager (+5A0h), the eye's place (+44Ch + 30h), its forward row
  (+20h) times the gun's melee distance (+578h), the gun's melee data
  (+53Ch), the carrier's time, its owner (+110h) and +118h, 0; slot 7 empty:
  slot 10's gun; a switch away at the end: the put, the strike still made;
- the swing left before its clip ended (finalize, as a drop on a death
  calls it): no strike; no gun in either slot: none.
Says nothing about what the clip looks like or about the bullet's flight.
"""
import struct
import sys

from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_ESI,
                               UC_X86_REG_ESP, UC_X86_REG_XMM0)

import patch_026g as base
import check_hide_ui_026g as hide

INITIALIZE_LOGIC = 0x301920      # EAX the battery; take clips, take time, carry clips, put clips, put time; ret 14h
SITE, SITE_AFTER = 0x2FA7E9, 0x2FA7EF   # battery.logic: mov ecx,[esp+20h] / mov eax,[ecx] replayed
TICK = 0x38CA20                  # fsm::tick, stdcall (the fsm)
MALLOC, NAME, SCALE = 0xE4F90, 0x420789, 0x30D750
SUBSCRIBE, UNSUBSCRIBE, EMIT = 0x391C40, 0x3921A0, 0x322830
TAKE_VTABLE, FUNCTION_PREDICATE, TAKE_NAME = 0xA1BF84, 0xA13CAC, 0xAE1E18
WEAPON_VTABLE = 0xA04174
MELEE = 1 << 27

DATA = 0x30000000
BATTERY, BATTERY_VTABLE, IDENTITY = DATA, DATA + 0x800, DATA + 0x900
PLAYER, INVENTORY = DATA + 0x10000, DATA + 0x20000
GUN7, GUN10, MANAGER = DATA + 0x30000, DATA + 0x34000, DATA + 0x38000
CLIPS = DATA + 0x40000           # the three states' six clips each, then the clips
HEAP = DATA + 0x50000
STUBS = DATA + 0xF0000           # get_transform / set_transform of the battery: ret 4
SIZE = 0x100000
ALLOCATOR, AI_ALLOCATOR = DATA + 0xE8000, DATA + 0xE9000   # the slots 0x1F1D9C4, 0x1F1DA44 (run-time in the game)
FSM = BATTERY + 0x168


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


class Run(hide.Run):
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_map(DATA, SIZE, UC_PROT_ALL)
        for at, handler in ((MALLOC, self.malloc), (NAME, self.name), (SCALE, self.scale),
                            (SUBSCRIBE, self.subscribe), (UNSUBSCRIBE, self.unsubscribe), (EMIT, self.emit)):
            self.emu.hook_add(UC_HOOK_CODE, handler, begin=at, end=at)

    def ret(self, pops, eax=None):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        if eax is not None:
            self.emu.reg_write(UC_X86_REG_EAX, eax)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)

    def arg(self, i):
        return self.read(self.emu.reg_read(UC_X86_REG_ESP) + 4 * i)

    def malloc(self, emu, *_):
        size = emu.reg_read(UC_X86_REG_EAX)
        at = self.next
        self.next = (at + size + 15) & ~15
        self.allocated.append((size, self.arg(1), emu.reg_read(UC_X86_REG_ESI), at))
        self.ret(4, at)

    def name(self, emu, *_):
        self.ret(0, emu.reg_read(UC_X86_REG_ECX))

    def scale(self, emu, *_):
        self.times.append(struct.unpack("<f", struct.pack("<I", self.arg(1)))[0])
        emu.reg_write(UC_X86_REG_XMM0, struct.unpack("<I", struct.pack("<f", 2.5))[0])
        self.ret(0)

    def subscribe(self, emu, *_):
        self.channels.append(("subscribe", tuple(self.arg(i) for i in range(1, 6))))
        self.ret(0x14)

    def unsubscribe(self, emu, *_):
        self.channels.append(("unsubscribe", self.arg(2)))
        self.ret(8)

    def emit(self, emu, *_):
        args = [self.arg(i) for i in range(1, 9)]
        point = struct.unpack("<3f", self.read(args[1], 12))
        velocity = struct.unpack("<3f", self.read(args[2], 12))
        self.emitted.append((emu.reg_read(UC_X86_REG_EDI), args[0], args[1], point, velocity, *args[3:]))
        self.ret(0x20, 0)

    def floats(self, at, values):
        self.emu.mem_write(at, struct.pack(f"<{len(values)}f", *values))

    def made(self):
        """A battery as initialize_logic makes it, carried by PLAYER with two
        guns; the machine's current state the carry."""
        self.emu.mem_write(DATA, bytes(SIZE))
        self.next, self.allocated, self.times, self.channels, self.emitted = HEAP, [], [], [], []
        self.word(0x1F1D9C4, ALLOCATOR)
        self.word(0x1F1DA44, AI_ALLOCATOR)
        self.word(BATTERY, BATTERY_VTABLE)
        self.word(BATTERY + 8, PLAYER)
        self.emu.mem_write(STUBS, b"\xB8" + struct.pack("<I", IDENTITY) + b"\xC2\x04\x00")
        self.word(BATTERY_VTABLE + 0x24, STUBS)
        self.word(BATTERY_VTABLE + 0xA4, STUBS)
        self.floats(IDENTITY, [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1])
        for state in range(3):
            for i in range(6):
                clip = CLIPS + 0x1000 + 0x100 * (6 * state + i)
                self.word(clip + 0xDC, 100)
                self.word(CLIPS + 0x18 * state + 4 * i, clip)
        # the carrier: alive, holding the battery, at 1000 ms; its eye at
        # (1, 2, 3) looking along (0, 0, 1); its guns in slots 7 and 10
        self.emu.mem_write(PLAYER + 0x33C, b"\x01")
        self.word(PLAYER + 0x128, BATTERY)
        self.word(PLAYER + 0x12C, BATTERY)
        self.word(PLAYER + 0x324, 1000)
        self.floats(PLAYER + 0x44C, [1, 0, 0, 0, 0, 1, 0, 0, 0, 0.6, 0.8, 0, 1, 2, 3, 1])
        self.word(PLAYER + 0x10C, INVENTORY)
        for gun, slot in ((GUN7, 7), (GUN10, 10)):
            self.word(gun, WEAPON_VTABLE)
            self.word(gun + 0x5A0, MANAGER)
            self.floats(gun + 0x578, [1.7 if gun == GUN7 else 1.5])
            self.word(INVENTORY + 0x10C + 4 * slot, gun + 0x10)
        sp = hide.STACK + 0x9000
        self.word(sp, hide.SENTINEL)
        for i, value in enumerate((CLIPS, struct.unpack("<I", struct.pack("<f", 0.5))[0], CLIPS + 0x18, CLIPS + 0x30,
                                   struct.unpack("<I", struct.pack("<f", 0.6))[0])):
            self.word(sp + 4 + 4 * i, value)
        self.run(INITIALIZE_LOGIC, (hide.SENTINEL,), sp, {**hide.GAME, UC_X86_REG_EAX: BATTERY})
        assert self.stopped == hide.SENTINEL, hex(self.stopped or 0)
        self.take, self.carry, self.put = self.states()[:3]
        for state in (self.take, self.carry, self.put):
            self.word(state + 0xC4, 1000)    # executed at this tick already: no carried transform
        self.word(BATTERY + 0x178, self.carry)

    def states(self):
        out, at = [], self.read(BATTERY + 0x170)
        while at:
            out.append(at)
            at = self.read(at + 4)
        return out

    def transitions(self, state):
        out, at = [], self.read(state + 0x10)
        while at:
            out.append((self.read(at + 0x20), self.read(at, 0x20), at))
            at = self.read(at + 0x24)
        return out

    def site(self):
        """The cook callback's site: ESI the battery, ESP+28h the put's clips."""
        sp = hide.STACK + 0x9000
        dummy = DATA + 0xE0000
        self.word(dummy, 0x7E57)
        self.word(sp + 0x20, dummy)
        self.emu.mem_write(sp + 0x28, self.read(CLIPS + 0x30, 0x18))
        self.times, self.allocated = [], []
        regs, after = self.run(SITE, (SITE_AFTER,), sp, {**hide.GAME, UC_X86_REG_ESI: BATTERY})
        assert self.stopped == SITE_AFTER and after == sp, hex(self.stopped or 0)
        assert regs[UC_X86_REG_ECX] == dummy and regs[UC_X86_REG_EAX] == 0x7E57, "the replayed moves"
        assert regs[UC_X86_REG_ESI] == BATTERY

    def tick(self):
        """fsm::tick on the battery's machine: the state it is in after."""
        sp = hide.STACK + 0x9000
        self.word(sp, hide.SENTINEL)
        self.word(sp + 4, FSM)
        self.emitted, self.channels = [], []
        _, after = self.run(TICK, (hide.SENTINEL,), sp, hide.GAME)
        assert self.stopped == hide.SENTINEL and after == sp + 8, hex(self.stopped or 0)
        return self.read(BATTERY + 0x178)

    def finalize(self, state):
        """The state's finalize through its vtable (+0Ch), as a drop calls it."""
        sp = hide.STACK + 0x9000
        self.word(sp, hide.SENTINEL)
        self.emitted, self.channels = [], []
        self.run(self.read(self.read(state) + 0xC), (hide.SENTINEL,), sp, {**hide.GAME, UC_X86_REG_ECX: state})
        assert self.stopped == hide.SENTINEL

    def actions(self, bits):
        self.word(PLAYER + 0x330, bits)


def check(ok, what):
    if not ok:
        sys.exit(f"FAIL {what}")
    print(f"  ok {what}")


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    built, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None,
                          open(base.late_join_bin(), "rb").read())
    old, new = Run(original), Run(bytes(built))

    print("the cook's site (battery.logic 0x2FA7E9)")
    for r in (old, new):
        r.made()
        r.site()
    check(len(old.states()) == 3 and old.read(FSM) == 3, "the original: take, carry, put")
    states = new.states()
    check(len(states) == 4 and new.read(FSM) == 4 and states[:3] == [new.take, new.carry, new.put]
          and new.read(BATTERY + 0x174) == states[3], "the patched: take, carry, put, then the swing, the tail")
    swing = states[3]
    new.swing = swing
    check(new.allocated[0] == (0xEC, TAKE_NAME, ALLOCATOR, swing) and len(new.allocated) == 4
          and all(size == 0x28 and allocator == AI_ALLOCATOR for size, _, allocator, _ in new.allocated[1:]),
          "0ECh bytes of the game's allocator, the take's allocation name; three transitions of the ai arena's")
    clips = [new.read(CLIPS + 0x30 + 4 * i) for i in range(6)]
    stored = [new.read(swing + o) for o in (0xE0, 0xE4, 0xD0, 0xD8, 0xD4, 0xDC)]
    check(stored == clips and new.times == [f32(0.3)] and new.read(swing + 0xE8) == 0x40200000,
          "a take state over the put's six clips, scaled to 0.3 s")
    check(new.read(swing + 0x20) == 3 and new.read(swing + 0x1C) == BATTERY and new.read(swing + 0x28, 1) == b"\0"
          and new.read(swing + 4) == 0, "its kind the carry's (3), its battery, not ended, the list's last")
    vtable = new.read(swing)
    ours = [new.read(vtable - 4 + 4 * i) for i in range(13)]
    stock = [new.read(TAKE_VTABLE - 4 + 4 * i) for i in range(13)]
    check(vtable != TAKE_VTABLE and ours[:4] + ours[5:] == stock[:4] + stock[5:] and ours[4] != stock[4]
          and ours[4] >= 0xA42D000, "its vtable: the take's RTTI and slots, finalize (+0Ch) ours in the arena")
    carry = new.transitions(new.carry)
    take = new.transitions(new.take)
    check([d for d, _, _ in carry] == [new.put, swing] and [d for d, _, _ in take] == [new.carry],
          "the carry's transitions: put, then the swing")
    wants = carry[1][1]
    manager, _, function, battery = struct.unpack_from("<4I", wants)
    check(manager == FUNCTION_PREDICATE | 1 and function >= 0xA42D000 and battery == BATTERY,
          "carry -> swing: a function-pointer functor (0xA13CAC | 1), its function ours, the battery its buffer's")
    out = new.transitions(swing)
    check([d for d, _, _ in out] == [new.put, new.carry] and out[0][1] == carry[0][1] and out[1][1] == take[0][1],
          "the swing's: put (carry -> put's functor), then carry (take -> carry's)")

    print("fsm::tick from the carry (0x38CA20)")
    for r in (old, new):
        r.actions(MELEE)
    check(old.tick() == old.carry, "the original: the key down, the carry stays")
    for bits, alive, want in ((0, 1, "carry"), (1 << 26, 1, "carry"), (MELEE, 0, "carry"), (MELEE, 1, "swing"),
                              (MELEE | 0x41, 1, "swing")):
        new.made()
        new.site()
        new.actions(bits)
        new.emu.mem_write(PLAYER + 0x33C, bytes([alive]))
        state = new.tick()
        check(state == (new.states()[3] if want == "swing" else new.carry) and new.emitted == [],
              f"actions {bits:#x}, alive {alive}: the {want}, no strike")
    check(any(kind == "subscribe" and new.states()[3] in args for kind, args in new.channels)
          and new.read(new.states()[3] + 0x28, 1) == b"\0",
          "the swing waits for its clip's end (subscribed, not ended)")
    new.made()
    new.site()
    new.actions(MELEE)
    new.word(PLAYER + 0x12C, GUN7 + 0x10)
    check(new.tick() == new.put, "a switch away (next item another), the key down: the put first")

    print("fsm::tick in the swing")
    eye = (1.0, 2.0, 3.0)

    def swinging():
        new.made()
        new.site()
        new.actions(MELEE)
        swing = new.states()[3]
        assert new.tick() == swing
        new.actions(0)
        return swing

    swing = swinging()
    check(new.tick() == swing and new.emitted == [], "before its clip ended: nothing")
    new.emu.mem_write(swing + 0x28, b"\x01")
    check(new.tick() == new.carry, "once ended: back to the carry")
    gun = (GUN7, MANAGER, PLAYER + 0x47C, eye, (0.0, f32(f32(0.6) * f32(1.7)), f32(f32(0.8) * f32(1.7))), GUN7 + 0x53C, 1000,
           PLAYER + 0x110, PLAYER + 0x118, 0)
    check(new.emitted == [gun], "one strike: the slot-7 gun's melee bullet from the eye along its forward, now")
    check(("unsubscribe", swing) in new.channels, "the swing's own finalize ran (its channel left)")
    swing = swinging()
    new.word(INVENTORY + 0x10C + 4 * 7, 0)
    new.emu.mem_write(swing + 0x28, b"\x01")
    check(new.tick() == new.carry and len(new.emitted) == 1 and new.emitted[0][0] == GUN10
          and new.emitted[0][5] == GUN10 + 0x53C and new.emitted[0][4][2] == f32(f32(0.8) * f32(1.5)),
          "slot 7 empty: slot 10's gun")
    swing = swinging()
    new.word(PLAYER + 0x12C, GUN7 + 0x10)
    new.emu.mem_write(swing + 0x28, b"\x01")
    check(new.tick() == new.put and new.emitted == [gun], "a switch away at its end: the put, the strike made")

    print("the swing left early")
    swing = swinging()
    new.finalize(swing)
    check(new.emitted == [] and ("unsubscribe", swing) in new.channels, "before its clip ended (a drop): no strike")
    swing = swinging()
    new.word(INVENTORY + 0x10C + 4 * 7, 0)
    new.word(INVENTORY + 0x10C + 4 * 10, 0)
    new.emu.mem_write(swing + 0x28, b"\x01")
    new.finalize(swing)
    check(new.emitted == [], "no gun in slot 7 or 10: none")
    print("battery_hit: all checks passed")


if __name__ == "__main__":
    main()
