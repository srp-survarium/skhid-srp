#!/usr/bin/env python3
"""Run patch 67 (kill_feed_026g.asm: the kill feed's wall and pain icons) in
the default build on made-up game objects.

    python check_kill_feed_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them; the patch's three sites run from the game's own code:

- the event: base_player::generate_killed_event (0x2DA170) is run from
  after its event's allocation (0x2DA3AD, eax = the node, zeroed as 0x2D43B0
  zeroes it) to its queueing call (0x2DA3DA), with a made-up victim (esi: its
  place +114h, its world +120h, its damage model +2E0h whose parts +108h are a
  list of named pools, health +0A0h) and bullet (ebp: its segments +84h, its
  reflections +88h), the cause icon in bl. The game's own part lookup
  (0x2E58B0) runs. The node must be the original's but for +5: 1 when the
  bullet has segments and no reflection and someone else killed (the cause
  icon not 3; 0.69d0's penetration_icon), 2 when the pain pool is empty
  and body and head are not; edx (the world), eax, ebx, esi, edi, ebp and esp
  as the original's; the victim, its pools and the bullet unchanged. Cases:
  a bullet death, a pain death, both pools empty, the head empty, pain all
  but empty, a bullet through a wall, a ricochet, pierces and a reflection,
  through another player, through a wall with no killer, a headshot through
  a wall, no bullet, no damage model, the parts in another order;
- the argument: the hud's event handler (0x23B5F0) from 0x23B6F0 to its call
  of on_player_killed (0x23B704), edi = that node: the original pushes the
  cause icon as the fifth argument, the build the cause icon and the flags
  in its second byte, the other four alike;
- the row: on_player_killed (0x237C70) from 0x23814F (after extra_icon) to
  its Invoke of root.add_log_message (0x2381A5), with that fifth argument, a
  made-up description object (its interface's SetMember a stub that records
  the member, the GFx value's type and data) and the hud: the original sets
  mastery_icon; the build penetration_icon and pain_icon (ints, 0 or 1, the
  flags) and then mastery_icon, and arrives with the registers, the frame and
  the value as the original's.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import patch_026g as base

EVENT_FROM, EVENT_QUEUE = 0x2DA3AD, 0x2DA3DA          # generate_killed_event after the node's allocation
HANDLER_FROM, HANDLER_CALL = 0x23B6F0, 0x23B704       # the hud's event handler, its kill case
ROW_FROM, ROW_INVOKE = 0x23814F, 0x2381A5             # on_player_killed after extra_icon, its Invoke
OBJECTS, STACK, TEXT, STUB = 0x20000000, 0x21000000, 0x22000000, 0x23000000
VICTIM, MODEL, BULLET, NODE, WORLD = OBJECTS, OBJECTS + 0x1000, OBJECTS + 0x2000, OBJECTS + 0x3000, OBJECTS + 0x4000
PARTS = OBJECTS + 0x5000                               # the pools' list nodes, 0x100 apart
HUD, HOLDER, UI, MOVIE, DESCRIPTION = (OBJECTS + 0x8000 + 0x100 * i for i in range(5))
REGS = (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP)
NAMES = ("infection", "body", "head", "pain", "left_leg")
WALL, PAIN = 1, 2


def bits(value):
    return struct.unpack("<I", struct.pack("<f", value))[0]


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at in (OBJECTS, STACK, TEXT, STUB):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        for i, name in enumerate(NAMES):
            emu.mem_write(TEXT + 0x40 * i, name.encode() + b"\0")
        emu.mem_write(STUB, b"\xC3")
        emu.hook_add(UC_HOOK_CODE, self.set_member, begin=STUB, end=STUB)
        self.members = []

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at, size=4):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0] if size == 4 else bytes(self.emu.mem_read(at, size))

    def text(self, at):
        return bytes(self.emu.mem_read(at, 64)).split(b"\0")[0].decode()

    def set_member(self, emu, address, size, _):
        """A GFx object interface's SetMember: thiscall (data, name, &value, is
        a display object), `ret 10h`; al 1."""
        sp = emu.reg_read(UC_X86_REG_ESP)
        data, name, value, display = (self.read(sp + 4 * i) for i in range(1, 5))
        self.members.append((emu.reg_read(UC_X86_REG_ECX), data, self.text(name), self.read(value + 4),
                             self.read(value + 8), display))
        emu.reg_write(UC_X86_REG_EAX, 1)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + 16)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def objects(self):
        """The victim, its damage model, the bullet, the world and the pools (not the node)."""
        return self.read(OBJECTS, NODE - OBJECTS) + self.read(WORLD, OBJECTS + 0x8000 - WORLD)

    def run(self, start, until, sp, regs):
        for reg, value in regs.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.emu_start(start, until, count=100000)
        return {reg: self.emu.reg_read(reg) for reg in REGS}, self.emu.reg_read(UC_X86_REG_ESP)

    def event(self, health, segments, reflections, cause, bullet=True, model=True, order=NAMES):
        """generate_killed_event from 0x2DA3AD to 0x2DA3DA. health: {pool: value}."""
        emu = self.emu
        emu.mem_write(OBJECTS, bytes(0x8000))
        emu.mem_write(VICTIM + 0x114, bytes([7]))
        self.word(VICTIM + 0x120, WORLD)
        self.word(VICTIM + 0x2E0, MODEL if model else 0)
        previous = 0
        for i, name in reversed(list(enumerate(order))):   # a list by name: [+0] next, [+80h] name, [+0A0h] health
            node = PARTS + 0x100 * i
            self.word(node, previous)
            self.word(node + 0x80, TEXT + 0x40 * NAMES.index(name))
            self.word(node + 0xA0, health.get(name, bits(1.0)))
            previous = node
        self.word(MODEL + 0x108 + 8, previous)
        self.word(BULLET + 0x84, segments)
        emu.mem_write(BULLET + 0x88, bytes([reflections]))
        emu.mem_write(NODE, bytes(0x30))
        sp = STACK + 0x8000
        emu.mem_write(STACK, bytes(0x10000))
        for at, value in ((0x44, 12345), (0x48, 3), (0x4C, 17)):   # the time, the killer, the weapon's icon
            self.word(sp + at, value)
        regs = {UC_X86_REG_EAX: NODE, UC_X86_REG_ECX: 0x0C0C0C0C, UC_X86_REG_EDX: 0x0D0D0D0D,
                UC_X86_REG_EBX: 0x0B0B0B00 | cause, UC_X86_REG_ESI: VICTIM, UC_X86_REG_EDI: 0x0E0E0E0E,
                UC_X86_REG_EBP: BULLET if bullet else 0}
        objects = self.objects()
        out, esp = self.run(EVENT_FROM, EVENT_QUEUE, sp, regs)
        return out, esp - sp, self.read(NODE, 0x30), objects == self.objects(), \
            self.read(sp, 0x100)

    def handler(self, node):
        """The event handler from 0x23B6F0 to its call of on_player_killed:
        the five arguments it pushed."""
        self.emu.mem_write(NODE, node)
        sp = STACK + 0x8000
        regs = {UC_X86_REG_EBX: HUD, UC_X86_REG_EDI: NODE, UC_X86_REG_ESI: 0x05050505, UC_X86_REG_EBP: 0x19FF00}
        out, esp = self.run(HANDLER_FROM, HANDLER_CALL, sp, regs)
        return [self.read(esp + 4 * i) for i in range(5)], esp - sp, out

    def row(self, fifth):
        """on_player_killed from 0x23814F to its Invoke: the members set."""
        emu = self.emu
        emu.mem_write(OBJECTS + 0x8000, bytes(0x800))
        self.word(HUD + 8, HOLDER)
        self.word(HOLDER + 0x108, UI)
        self.word(UI + 4, MOVIE)
        self.word(DESCRIPTION, DESCRIPTION + 0x40)                 # the interface's vtable
        self.word(DESCRIPTION + 0x40 + 0x14, STUB)                 # SetMember
        sp = STACK + 0x8000
        emu.mem_write(STACK, bytes(0x10000))
        frame = {0x14: 0, 0x18: 3, 0x1C: fifth & 0xFF,             # the value: extra_icon's int
                 0x2C: DESCRIPTION, 0x30: 0x0A, 0x34: 0x0DA7A,      # the description (interface, type, data)
                 0xF4: HUD, 0xF8: 1, 0xFC: 2, 0x100: 17, 0x104: fifth}
        for at, value in frame.items():
            self.word(sp + at, value)
        self.members = []
        regs = {UC_X86_REG_EBX: 4, UC_X86_REG_ESI: 3, UC_X86_REG_EDI: 0, UC_X86_REG_EBP: 0x19FF00,
                UC_X86_REG_EAX: 0x0A0A0A0A, UC_X86_REG_ECX: 0x0C0C0C0C, UC_X86_REG_EDX: 0x0D0D0D0D}
        out, esp = self.run(ROW_FROM, ROW_INVOKE, sp, regs)
        return out, esp - sp, list(self.members), self.read(sp, 0x110)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    stock, patched = Run(original), Run(bytes(built))
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    full, empty = bits(1.0), 0
    cases = (
        ("a bullet to the body: body empty, pain left", {"body": empty, "pain": bits(0.3)}, 0, 0, 0, {}, 0),
        ("a pain death: pain empty, body and head left", {"pain": empty, "body": bits(0.6), "head": bits(0.4)},
         0, 0, 0, {}, PAIN),
        ("one hit empties pain and body together: damage, not pain (the owner)", {"pain": empty, "body": empty},
         0, 0, 0, {}, 0),
        ("one hit empties pain and head together, not a headshot bone: damage, not pain",
         {"pain": empty, "head": empty, "body": bits(0.7)}, 0, 0, 0, {}, 0),
        ("a headshot: head and pain empty", {"pain": empty, "head": empty}, 0, 0, 1, {}, 0),
        ("a headshot that left the head: pain empty", {"pain": empty, "head": bits(0.1)}, 0, 0, 1, {}, PAIN),
        ("pain all but empty", {"pain": bits(1e-7), "body": empty}, 0, 0, 0, {}, 0),
        ("a toxic death: infection empty", {"infection": empty}, 0, 0, 3, {}, 0),
        ("through a wall to the body", {"body": empty}, 1, 0, 0, {}, WALL),
        ("a ricochet to the body", {"body": empty}, 1, 1, 0, {}, 0),
        ("two pierces and a ricochet (0.69d0: its ricochet, no wall)", {"body": empty}, 3, 1, 0, {}, 0),
        ("through another player to the body", {"body": empty}, 1, 0, 0, {}, WALL),
        ("through a wall, no killer or the victim himself", {"body": empty}, 1, 0, 3, {}, 0),
        ("through a wall, a headshot", {"head": empty}, 1, 0, 1, {}, WALL),
        ("through a wall to pain", {"pain": empty, "body": bits(0.2)}, 2, 0, 0, {}, WALL | PAIN),
        ("no bullet (a grenade), pain empty", {"pain": empty}, 5, 0, 0, {"bullet": False}, PAIN),
        ("no damage model", {"pain": empty}, 1, 0, 0, {"model": False}, WALL),
        ("pain first in the list", {"pain": empty}, 0, 0, 0, {"order": ("pain", "head", "body")}, PAIN),
        ("no head pool", {"pain": empty}, 0, 0, 0, {"order": ("pain", "body")}, 0),
    )
    print("the event (generate_killed_event from 0x2DA3AD to its queueing at 0x2DA3DA)")
    nodes = []
    for what, health, segments, reflections, cause, extra, flags in cases:
        a = stock.event(health, segments, reflections, cause, **extra)
        b = patched.event(health, segments, reflections, cause, **extra)
        keep = [UC_X86_REG_EAX, UC_X86_REG_EDX, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP]
        node = bytearray(a[2])
        node[5] = flags
        ok = all(a[0][r] == b[0][r] for r in keep) and a[0][UC_X86_REG_EDX] == WORLD and a[1] == b[1] == 0 \
            and a[2][4] == b[2][4] == cause and a[2][5] == 0 and b[2] == bytes(node) and a[3] and b[3] \
            and a[4] == b[4]
        nodes.append((what, b[2], flags, cause))
        report(ok, f"{what}: cause {cause}, flags {b[2][5]} (want {flags}); node, registers, frame and "
                   f"objects the original's otherwise")

    print("the argument (the event handler from 0x23B6F0 to its call of on_player_killed)")
    for what, node, flags, cause in nodes:
        a, b = stock.handler(node), patched.handler(node)
        ok = a[0][:4] == b[0][:4] == [HUD, 7, 3, 17] and a[0][4] == cause and b[0][4] == cause | flags << 8 \
            and a[1] == b[1] == -20
        report(ok, f"{what}: the fifth argument {b[0][4]:#x} (the original's {a[0][4]:#x})")

    print("the row (on_player_killed from 0x23814F to its Invoke at 0x2381A5)")
    for what, node, flags, cause in nodes:
        fifth = cause | flags << 8
        a, b = stock.row(cause), patched.row(fifth)
        mastery = (DESCRIPTION, 0x0DA7A, "mastery_icon", 3, 0, 1)
        want = [(DESCRIPTION, 0x0DA7A, "penetration_icon", 3, flags & WALL and 1, 1),
                (DESCRIPTION, 0x0DA7A, "pain_icon", 3, flags & PAIN and 1, 1), mastery]
        ok = a[2] == [mastery] and b[2] == want and a[0] == b[0] and a[1] == b[1] \
            and a[3][:0x104] == b[3][:0x104]
        report(ok, f"{what}: " + ", ".join(f"{name} {value}" for _, _, name, _, value, _ in b[2]))
    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
