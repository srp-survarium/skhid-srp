#!/usr/bin/env python3
"""Patch 199 (profile_kit): the lobby is given the kit it shows whole, in one
call (docs/notes/client_patch.md "The kit shown, in one call"), the default
build beside the original.

    python check_profile_kit_026g.py <original survarium.exe>

Requires Python `unicorn`. lobby_menu::on_profile_changed (0x222330) runs from
each site to the instruction after its Invoke, in both exes, on a made-up
lobby menu and frame: ESI the menu (+690h its movie's holder), ESP the frame
([esp+0Fh] the kit's containers, [esp+10h] its pockets, [esp+58h] the kit's
items value, [esp+164h] the counts' first value as the function set it).
Stand-ins: Scaleform's Invoke (0x511600) keeps the call's name
and values; the AS3 object interface's SetMember (0x7C6A20) keeps the member's
name and uint.

What the movie is given:
- at the items' Invoke (0x222A23): the original gives the items value alone;
  the build gives nothing;
- at the counts' Invoke (0x222CD6): the original gives update_profile_slots_count
  (containers, pockets); the build gives the items value, its item_slots the
  pockets and its artifact_slots the containers, and nothing else;
- a value of no AS3 movie (its interface not the AS3 one): the build gives the
  items alone, then the original's counts call;
- both go on where the original does, the stack and the kept registers as it.
The function's bytes but the two hooks' are the original's.
"""
import os
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_EDI, UC_X86_REG_ESI, UC_X86_REG_ESP,
                               UC_X86_REG_EIP)

if len(sys.argv) > 1:
    os.environ.setdefault("SRP_026G_EXE", sys.argv[1])    # a span hook's bytes (patch_026g.RUST_HOOKS)
import patch_026g as base

FUNCTION, FUNCTION_END = 0x222330, 0x222FD5
ITEMS_SITE, ITEMS_SPAN, ITEMS_AFTER = 0x222A07, 6, 0x222A28
COUNTS_SITE, COUNTS_SPAN, COUNTS_AFTER = 0x222CA1, 10, 0x222CDB
INVOKE, SET_MEMBER = 0x511600, 0x7C6A20
AS3_OBJECT_INTERFACE = 0x8FC288
THREAD_SLOT = 0x870118
HEAP, STUBS, STACK = 0x30000000, 0x38000000, 0x39000000
MENU, HOLDER, VIEW, MOVIE, INTERFACE, OBJECT = (HEAP + n * 0x1000 for n in range(6))
FRAME = STACK + 0x4000
CONTAINERS, POCKETS = 1, 3
UINT, OBJECT_KIND = 4, 0x0A

failed = 0


def check(ok, what):
    global failed
    failed += not ok
    print(f"  {'ok' if ok else 'FAIL'}  {what}")


class Lobby:
    def __init__(self, data, as3=True):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at in (HEAP, STUBS, STACK):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        self.u32(THREAD_SLOT, STUBS + 0x80)
        emu.mem_write(STUBS + 0x80, b"\xB8\x01\x00\x00\x00\xC3")    # GetCurrentThreadId: mov eax,1 / ret
        self.u32(MENU + 0x690, HOLDER)
        self.u32(HOLDER + 0x108, VIEW)
        self.u32(VIEW + 4, MOVIE)
        self.u32(INTERFACE, AS3_OBJECT_INTERFACE if as3 else 0x8CE2A8)
        emu.mem_write(FRAME + 0xF, bytes([CONTAINERS]))
        self.u32(FRAME + 0x10, POCKETS)
        self.u32(FRAME + 0x58, INTERFACE)
        self.u32(FRAME + 0x5C, OBJECT_KIND)
        self.u32(FRAME + 0x60, OBJECT)
        # update_profile_slots_count's first value, the containers, as the function wrote it before
        # the site (0x222C61..0x222C6C); the site writes the second
        self.u32(FRAME + 0x164 + 4, UINT)
        self.u32(FRAME + 0x164 + 8, CONTAINERS)
        self.given = []
        emu.hook_add(UC_HOOK_CODE, self.invoke, begin=INVOKE, end=INVOKE)
        emu.hook_add(UC_HOOK_CODE, self.set_member, begin=SET_MEMBER, end=SET_MEMBER)

    def u32(self, at, value=None):
        if value is None:
            return struct.unpack("<I", self.emu.mem_read(at, 4))[0]
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def ret(self, pops):
        """A `ret pops` that answers 1 (taken)."""
        esp = self.emu.reg_read(UC_X86_REG_ESP)
        self.emu.reg_write(UC_X86_REG_ESP, esp + 4 + pops)
        self.emu.reg_write(UC_X86_REG_EIP, self.u32(esp))
        self.emu.reg_write(UC_X86_REG_EAX, 1)

    def string(self, at):
        return bytes(self.emu.mem_read(at, 64)).split(b"\0")[0].decode()

    def invoke(self, *_):
        """Movie::Invoke (thiscall, `ret 10h`): the name and its values, each
        the items value's address or a uint's number."""
        esp = self.emu.reg_read(UC_X86_REG_ESP)
        name, values, count = self.string(self.u32(esp + 4)), self.u32(esp + 0xC), self.u32(esp + 0x10)
        shown = []
        for n in range(count):
            value = values + 0x18 * n
            shown.append("kit" if value == FRAME + 0x58 else self.u32(value + 8) if self.u32(value + 4) == UINT
                         else ("?", self.u32(value + 4)))
        self.given.append((name, tuple(shown)))
        self.ret(0x10)

    def set_member(self, *_):
        """The AS3 object interface's SetMember (thiscall, `ret 10h`): the
        object, the name, the value, whether a display object."""
        esp = self.emu.reg_read(UC_X86_REG_ESP)
        target, name, value = self.u32(esp + 4), self.string(self.u32(esp + 8)), self.u32(esp + 0xC)
        kind, number = self.u32(value + 4), self.u32(value + 8)
        self.given.append(("set", (target == OBJECT, name, number if kind == UINT else ("?", kind))))
        self.ret(0x10)

    def run(self, site, after):
        """From `site` to `after`: what the movie was given."""
        self.given = []
        regs = {UC_X86_REG_ESI: MENU, UC_X86_REG_EDI: 0x1234, UC_X86_REG_EBX: 0, UC_X86_REG_EBP: 0x5678}
        for reg, value in regs.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, FRAME)
        self.emu.emu_start(site, after, count=200_000)
        kept = all(self.emu.reg_read(reg) == value for reg, value in regs.items())
        return self.given, self.emu.reg_read(UC_X86_REG_ESP) == FRAME and kept


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    built = bytes(built)
    installs = {h[0]: h for h in base.rust_hooks(original)}
    for site, span in ((ITEMS_SITE, ITEMS_SPAN), (COUNTS_SITE, COUNTS_SPAN)):
        at = base.file_offset(site)
        check(site in installs and built[at] == 0xE9 and original[at] != 0xE9, f"{site:#x}: a jump to the build's hook")
    cuts = [(FUNCTION, ITEMS_SITE), (ITEMS_SITE + ITEMS_SPAN, COUNTS_SITE), (COUNTS_SITE + COUNTS_SPAN, FUNCTION_END)]
    same = all(built[base.file_offset(a):base.file_offset(b)] == original[base.file_offset(a):base.file_offset(b)]
               for a, b in cuts)
    check(same, f"{FUNCTION:#x}..{FUNCTION_END:#x} but the hooks: the original's bytes")

    stock, ours = Lobby(original), Lobby(built)
    items = ("root.fill_player_profile_items", ("kit",))
    given = [lobby.run(ITEMS_SITE, ITEMS_AFTER) for lobby in (stock, ours)]
    check(given[0] == ([items], True), f"the items' Invoke: the original gives the items alone: {given[0]}")
    check(given[1] == ([], True), f"the items' Invoke: the build gives nothing, the stack as the original's: {given[1]}")

    counts = ("root.update_profile_slots_count", (CONTAINERS, POCKETS))
    given = [lobby.run(COUNTS_SITE, COUNTS_AFTER) for lobby in (stock, ours)]
    check(given[0] == ([counts], True), f"the counts' Invoke: the original gives the counts: {given[0]}")
    whole = [("set", (True, "item_slots", POCKETS)), ("set", (True, "artifact_slots", CONTAINERS)), items]
    check(given[1] == (whole, True), f"the counts' Invoke: the build gives the items with the counts in them, once: {given[1]}")

    other = Lobby(built, as3=False).run(COUNTS_SITE, COUNTS_AFTER)
    check(other == ([items, counts], True), f"no AS3 object: the items alone, then the original's counts: {other}")

    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
