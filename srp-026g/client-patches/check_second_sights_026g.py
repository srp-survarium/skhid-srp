#!/usr/bin/env python3
"""Run patch 206 (second_sights: a weapon's second sight and the switch to
it and back, rust/patches/src/second_sights.rs; the rules:
../docs/notes/second-sights.md, "The simulation (0.26g)") in the default
build on made-up game objects, beside the original.

    python check_second_sights_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them; check_hide_ui_026g.py's and check_item_hotkeys_026g.py's
harnesses. Each hook runs from its own site in the game's code, the build
and the original from the same machine, to the instruction after the
site's span (or the branch after it):

- the input builder (process_first_person_mode 0x241250, whole, with its
  handler's events as check_item_hotkeys_026g.py pushes them): bit 30 set
  in the build's mask exactly when scope_next (5Ah) went down, every other
  bit the original's;
- weapon_core::weapon_core's entry (0x2DF0D0) with a weapon config (the
  config's child and has stubbed as check_attachments_026g.py does):
  `data/srp_second_sight` nonzero with `srp_second_side` 0 or 1 makes a
  weapon of two sights, anything else one; ~weapon_core's entry (0x2DF570)
  forgets it;
- apply_new_input after process_quick_slots (0x2D877C, the press: EDI the
  player, the actions before at [esp+24h], the time at [ebp+8]),
  weapon_core::tick's join (0x2E0A45, EBP the weapon, EBX the time) and
  weapon_core::deactivate's entry (0x2E2DB0): seen through the player's
  state's end, base_player::serialize's epilogue (0x2DB47E, the game's byte
  writer 0x1F3DE0 recorded: patch 78's 32 bytes, then the record) and its
  load (0x2DBAB5, a reader over a made-up state's end);
- the shot's reads of the aim: get_dispersed_bullet_direction's aim
  progress (0x2E1D68, XMM1 before the compare with 1.0), dispersion_
  calculator::tick's m_aimed test (0x331375, to the branch it takes:
  0x33137E aimed, 0x331388 the hip's), get_skill_influence's and
  get_target_koef's loads of it (0x33149D: CL; 0x331572: DL, after the push
  of EDI).

Checked: a press of a weapon with a second sight switches it (once: a
press while switching, a bit held from the input before, a weapon of one
sight and a weapon in no weapon slot switch nothing); its tick ends it 500
ms after the press, not before; putting it away undoes it; while it lowers
the aim the shot's aim is the effective one and every m_aimed read takes
the hip's, before its first tick after the press and when it is settled the
original's bit for bit (every register, the flags' branch, ESP); the record
saved is the states, a load gives them back and the next save writes what
was loaded; a state without a record leaves the player unkept (no record
written, no press taken) until its next life; another gun in the slot is on
its main sight; a weapon destroyed is no longer one of two sights.
"""
import os
import struct
import sys

if len(sys.argv) > 1:
    os.environ.setdefault("SRP_026G_EXE", sys.argv[1])    # a span hook's bytes (patch_026g.RUST_HOOKS)

from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EFLAGS, UC_X86_REG_XMM1, UC_X86_REG_EIP

import patch_026g as base
import check_hide_ui_026g as hide
import check_item_hotkeys_026g as hotkeys

MADE, MADE_AFTER = 0x2DF0D0, 0x2DF0D5
DESTROYED, DESTROYED_AFTER = 0x2DF570, 0x2DF578
PRESS, PRESS_AFTER = 0x2D877C, 0x2D8782
TICK, TICK_AFTER = 0x2E0A45, 0x2E0A4B
DEACTIVATE, DEACTIVATE_AFTER = 0x2E2DB0, 0x2E2DB6
SAVE, SAVE_AFTER = 0x2DB47E, 0x2DB483
LOAD, LOAD_AFTER = 0x2DBAB5, 0x2DBABC
BULLET, BULLET_AFTER = 0x2E1D68, 0x2E1D6D
DISPERSION, AIMED, HIP = 0x331375, 0x33137E, 0x331388
SKILL, SKILL_AFTER = 0x33149D, 0x3314A3
TARGET, TARGET_AFTER = 0x331572, 0x331579
WRITE_U8 = 0x1F3DE0                     # the byte writer: EAX the value, [esp+4] the writer; ret 14h
CHILD, HAS = 0x0F6B10, 0x0F6C00         # a config table's child (EAX, EDI the name) and has (EAX, ESI)
SCOPE_NEXT, BIT = 0x5A, 1 << 30

DATA, SIZE = 0x30000000, 0x200000
CORE = DATA + 0x10000
PLAYER = DATA + 0x20000                 # 10000h
INVENTORY = DATA + 0x40000
WEAPONS = DATA + 0x50000                # 2000h each
CONFIGS = DATA + 0x80000
WRITER, READER, STATE = DATA + 0x90000, DATA + 0x90100, DATA + 0x91000
PLACE, KIT, SPAWNED = 3, 1, 90_000
SIGHTED, SECOND, PLAIN, LOOSE = (WEAPONS + 0x2000 * n for n in range(4))   # slot 7's, slot 10's, ...
AK, PKM, SVD = 62, 9, 63                # their guns' dict ids
CARRY = 32                              # patch 78's snapshot, before the record
T = 104_520


def f32(value):
    return struct.unpack("<I", struct.pack("<f", value))[0]


def record(first=(0, 0), second=(0, 0)):
    """second_sight::Record's bytes: (tag, pressed) of slot 7, of slot 10, the version."""
    return struct.pack("<BIBIB", first[0], first[1], second[0], second[1], 1)


class Run(hide.Run):
    def __init__(self, data):
        super().__init__(data)
        emu = self.emu
        emu.mem_map(DATA, SIZE, UC_PROT_ALL)
        self.written, self.children, self.next = bytearray(), {}, CONFIGS
        emu.hook_add(UC_HOOK_CODE, self.write, begin=WRITE_U8, end=WRITE_U8)
        emu.hook_add(UC_HOOK_CODE, self.config_stub, begin=CHILD, end=CHILD)
        emu.hook_add(UC_HOOK_CODE, self.config_stub, begin=HAS, end=HAS)
        emu.mem_write(PLAYER + 0x114, bytes([PLACE, KIT]))
        self.word(PLAYER + 0x120, CORE)
        self.word(PLAYER + 0x10C, INVENTORY)
        self.word(PLAYER + 0x328, SPAWNED)
        emu.mem_write(PLAYER + 0x33C, b"\x01")
        for weapon, gun in ((SIGHTED, AK), (SECOND, PKM), (PLAIN, SVD), (LOOSE, AK)):
            self.word(weapon + 8, PLAYER)
            self.word(weapon + 0x10 + 0x25C, weapon + 0x1000)      # its item's dictionary entry
            emu.mem_write(weapon + 0x1000, struct.pack("<H", gun))
        self.slots(SIGHTED, SECOND)

    def slots(self, first, second):
        self.word(INVENTORY + 0x10C + 4 * 7, first + 0x10 if first else 0)
        self.word(INVENTORY + 0x10C + 4 * 10, second + 0x10 if second else 0)

    # a binary config: a node 18h bytes (the value, the type at +14h, the count +16h); children by name
    def block(self, size):
        at, self.next = self.next, self.next + ((size + 15) & ~15)
        return at

    def node(self, kind, data=0, count=0):
        at = self.block(0x18)
        self.word(at, data)
        self.emu.mem_write(at + 0x14, struct.pack("<HH", kind, count))
        return at

    def table(self, entries):
        at = self.node(3, self.block(0x18 * max(1, len(entries))), len(entries))
        for name, child in entries.items():
            self.children[(at, name.encode())] = child
        return at

    def config(self, data):
        entries = {"object": self.table({})}
        if data is not None:
            entries["data"] = self.table({k: self.node(1, v) for k, v in data.items()})
        return self.table(entries)

    def config_stub(self, emu, address, *_):
        table = emu.reg_read(UC_X86_REG_EAX)
        key = self.text(emu.reg_read(UC_X86_REG_EDI if address == CHILD else UC_X86_REG_ESI))
        child = self.children.get((table, key))
        emu.reg_write(UC_X86_REG_EAX, (child or 0) if address == CHILD else int(child is not None))
        sp = emu.reg_read(UC_X86_REG_ESP)
        emu.reg_write(UC_X86_REG_ESP, sp + 4)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def write(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        assert self.read(sp + 4) == WRITER, hex(self.read(sp + 4))
        self.written.extend(emu.mem_read(emu.reg_read(UC_X86_REG_EAX), 1))
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 24)

    def site(self, start, stops, regs, frame=None, flags=0x202):
        """The code at `start` to one of `stops` from GAME's registers and `regs`, ESP at the
        stack with `frame` {offset: bytes}: (where it stopped, every register, ESP, the flags)."""
        sp = hide.STACK + 0x9000
        self.emu.mem_write(sp - 0x800, bytes(0x1000))
        for offset, value in (frame or {}).items():
            self.emu.mem_write(sp + offset, value)
        self.emu.reg_write(UC_X86_REG_EFLAGS, flags)
        out, esp = self.run(start, stops, sp, {**hide.GAME, **regs})
        assert self.stopped in stops, (hex(start), hex(self.stopped or 0))
        return self.stopped, out, esp, self.emu.reg_read(UC_X86_REG_EFLAGS) & 0x8D5

    def made(self, weapon, data):
        frame = {4: struct.pack("<III", weapon, self.config(data), 0x77)}
        return self.site(MADE, (MADE_AFTER,), {}, frame)

    def destroyed(self, weapon):
        return self.site(DESTROYED, (DESTROYED_AFTER,), {}, {4: struct.pack("<I", weapon)})

    def press(self, now, before=0, actions=BIT, held=SIGHTED):
        self.word(PLAYER + 0x128, held)
        self.word(PLAYER + 0x330, actions)
        frame, ebp = {0x24: struct.pack("<I", before)}, hide.STACK + 0xA000
        self.word(ebp + 8, now)
        return self.site(PRESS, (PRESS_AFTER,), {UC_X86_REG_EDI: PLAYER, UC_X86_REG_EBP: ebp}, frame)

    def tick(self, now, weapon=SIGHTED):
        self.word(weapon + 0x5A4, now)
        return self.site(TICK, (TICK_AFTER,), {UC_X86_REG_EBP: weapon, UC_X86_REG_EBX: now})

    def deactivate(self, weapon=SIGHTED):
        return self.site(DEACTIVATE, (DEACTIVATE_AFTER,), {UC_X86_REG_ECX: weapon})

    def save(self):
        """The state's end as base_player::serialize writes it: patch 78's bytes and what follows."""
        self.written = bytearray()
        self.site(SAVE, (SAVE_AFTER,), {UC_X86_REG_EBX: PLAYER, UC_X86_REG_EDI: WRITER,
                                       UC_X86_REG_EBP: hide.STACK + 0xB000})
        return bytes(self.written)

    def load(self, tail):
        """base_player::deserialize's end over a state ending with `tail` (the reader at it): the
        bytes it read."""
        self.emu.mem_write(STATE, bytes(0x40) + tail)
        for at, value in ((0, STATE), (4, STATE + 0x40), (8, 0x40 + len(tail))):
            self.word(READER + at, value)
        self.site(LOAD, (LOAD_AFTER,), {UC_X86_REG_EBX: PLAYER, UC_X86_REG_ESI: READER})
        return self.read(READER + 4) - (STATE + 0x40)

    def aim(self, read, now, aimed=1, progress=1.0, weapon=SIGHTED):
        """A read of the weapon's aim at `now`: (where it stopped, the value read, ESP)."""
        self.word(weapon + 0x5A4, now)
        self.emu.mem_write(weapon + 0x5E8, bytes([aimed]))
        self.emu.mem_write(weapon + 0x5FC, struct.pack("<f", progress))
        if read == "bullet":
            self.emu.reg_write(UC_X86_REG_XMM1, f32(progress))
            stopped, out, esp, _ = self.site(BULLET, (BULLET_AFTER,), {UC_X86_REG_EBX: weapon},
                                             {0x70: struct.pack("<I", 0x5151)})
            return stopped, self.emu.reg_read(UC_X86_REG_XMM1) & 0xFFFFFFFF, esp, out
        if read == "dispersion":
            stopped, out, esp, _ = self.site(DISPERSION, (AIMED, HIP), {UC_X86_REG_EAX: weapon})
            return stopped, None, esp, out
        if read == "skill":
            stopped, out, esp, _ = self.site(SKILL, (SKILL_AFTER,), {UC_X86_REG_ECX: weapon})
            return stopped, out[UC_X86_REG_ECX] & 0xFF, esp, out
        stopped, out, esp, _ = self.site(TARGET, (TARGET_AFTER,), {UC_X86_REG_EAX: weapon})
        return stopped, out[UC_X86_REG_EDX] & 0xFF, esp, out


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    built = bytes(built)
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    print("the key (process_first_person_mode 0x241250, the input builder's end 0x2421B0)")
    keys_a, keys_b = hotkeys.Game(original), hotkeys.Game(built)
    D, U, H = hotkeys.DOWN, hotkeys.UP, hotkeys.HOLD
    for what, events, held, want in (("scope_next down", [(SCOPE_NEXT, D)], [], True),
                                     ("scope_next down with W and fire", [(0x0B, D), (SCOPE_NEXT, D), (0x26, D)], [], True),
                                     ("scope_next held", [(SCOPE_NEXT, H)], [SCOPE_NEXT], False),
                                     ("scope_next up", [(SCOPE_NEXT, U)], [], False),
                                     ("another key (spot) down", [(0x22, D)], [], False),
                                     ("nothing", [], [], False)):
        (a, a_kept), (b, b_kept) = keys_a.build(events, held), keys_b.build(events, held)
        report(a_kept and b_kept and not a & BIT and b == a | (BIT if want else 0),
               f"{what}: the mask {b:#010x} (the original {a:#010x}), bit 30 {'set' if want else 'clear'}")

    print("a weapon made and destroyed (weapon_core 0x2DF0D0, ~weapon_core 0x2DF570), its sights through a press")
    stock, run = Run(original), Run(built)
    for weapon, data in ((SIGHTED, {"srp_gun": AK, "srp_second_sight": 2112, "srp_second_side": 1}),
                         (SECOND, {"srp_gun": PKM, "srp_second_sight": 2112, "srp_second_side": 5}),
                         (PLAIN, {"srp_gun": SVD}), (LOOSE, None)):
        a, b = stock.made(weapon, data), run.made(weapon, data)
        report(a == b, f"made {weapon:#x} with data {data}: every register, ESP and the flags the original's")
    a, b = stock.save(), run.save()
    report(a == b"" and len(b) == CARRY + 11 and b[0] == 2 and b[CARRY:] == record(),
           f"the state's end: the original writes nothing here, the build patch 78's {CARRY} bytes and the "
           f"record of two main sights {b[CARRY:].hex()}")
    run.press(T, held=SECOND)
    run.press(T, held=LOOSE)
    run.press(T, before=BIT)
    run.press(T, actions=0x21)
    report(run.save()[CARRY:] == record(),
           "no switch: a press of a weapon whose side is no side, of a weapon in no weapon slot, a bit held "
           "from the input before, an input without it")
    a, b = stock.press(T), run.press(T)
    report(a == b and run.save()[CARRY:] == record((2, T)),
           f"the press at {T} ms of the weapon with a second sight in slot 7: switching to it since then "
           f"{run.save()[CARRY:].hex()}; registers, ESP, flags the original's")

    print("the shot's reads of the aim while the switch lowers it, and else (the original's)")
    for now, switching in ((T, False), (104_550, False), (104_600, True), (104_800, True), (105_000, True)):
        for read in ("bullet", "dispersion", "skill", "target"):
            a, b = stock.aim(read, now), run.aim(read, now)
            if not switching:
                ok, what = a == b, "the original's"
            elif read == "bullet":
                s = (now - 104_550) / 500
                want = f32(max(0.0, min(1.0, 1.0 - min(s, 1 - s))))
                ok, what = a[0] == b[0] and a[1] == f32(1.0) and b[1] == want and a[2] == b[2], \
                    f"the effective aim {struct.unpack('<f', struct.pack('<I', b[1]))[0]:.3f}"
            elif read == "dispersion":
                ok, what = a[0] == AIMED and b[0] == HIP and a[2] == b[2], "the hip's branch"
            else:
                ok, what = a[1] == 1 and b[1] == 0 and a[0] == b[0] and a[2] == b[2], "m_aimed read as 0"
            report(ok, f"{read} at {now} ms: {what}")
    for read in ("bullet", "dispersion", "skill", "target"):
        for aimed, progress in ((0, 0.0), (1, 0.4)):
            a, b = stock.aim(read, 104_800, aimed, progress, SECOND), run.aim(read, 104_800, aimed, progress, SECOND)
            report(a == b, f"{read} of the slot 10 weapon (not switching), m_aimed {aimed}, aim {progress}: the "
                           "original's")

    print("the switch's end (weapon_core::tick 0x2E0A45) and putting the weapon away (deactivate 0x2E2DB0)")
    run.press(T + 100)
    a, b = stock.tick(105_000), run.tick(105_000)
    report(a == b and run.save()[CARRY:] == record((2, T)),
           "a press while switching is nothing; a tick before the end (105000) leaves it switching; "
           "registers the original's")
    run.tick(105_050)
    report(run.save()[CARRY:] == record((1, 0)), "the tick at 105050 ms (past 105020): on the second sight")
    run.press(200_000)
    a, b = stock.deactivate(), run.deactivate()
    report(a == b and run.save()[CARRY:] == record((1, 0)),
           "a press at 200000, then the weapon put away: back on the second sight; registers the original's")
    run.press(300_000)
    run.tick(300_500)
    report(run.save()[CARRY:] == record(), "a press at 300000 and the tick at 300500: on the main sight")

    print("the world: the player's state's end (base_player::deserialize 0x2DBAB5, serialize 0x2DB47E)")
    carry = run.save()[:CARRY]
    loaded = record((1, 0), (3, 400_000))
    report(run.load(carry + loaded) == CARRY + 11 and run.save() == carry + loaded,
           f"a state ending with a record ({loaded.hex()}): read whole, the next save writes what was loaded")
    report(run.load(carry) == CARRY and run.save() == carry,
           "a state without one (an older game's): nothing past the carry read, nothing written after it")
    run.press(500_000)
    report(run.save() == carry, "an unkept life takes no press")
    run.word(PLAYER + 0x328, SPAWNED + 1)
    report(run.save()[CARRY:] == record(), "the player's next life: two main sights, written")
    run.press(600_000)
    run.tick(600_550)
    run.slots(PLAIN, SECOND)
    report(run.save()[CARRY:] == record(), "another gun in slot 7: on its main sight")
    run.slots(SIGHTED, SECOND)
    run.destroyed(SIGHTED)
    run.word(PLAYER + 0x328, SPAWNED + 2)
    run.press(700_000)
    a, b = stock.destroyed(SIGHTED), run.destroyed(SIGHTED)
    report(a == b and run.save()[CARRY:] == record(),
           "the weapon destroyed: no longer one of two sights, a press switches nothing; registers the original's")
    print("all cases passed" if not failed else f"{failed} FAILED")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
