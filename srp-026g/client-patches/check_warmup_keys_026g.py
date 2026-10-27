#!/usr/bin/env python3
"""Run patch 31's warmup keys (a mouse button held from before the start is
not the player's until it is let go; README patch 31, client_patch.md
section 3.29) of the original and the build on made-up mouse events.

    python check_warmup_keys_026g.py <original survarium.exe>

Requires Python `unicorn` (2.1.4). What runs: the game world's
on_mouse_key_action 0x25CF60 (ECX the game world's input handler, its
cursor shown at +2E0h; the input world, the key's code and its kind on the
stack, `ret 0Ch`) from its start, the patch's code and the original's first
ten bytes (`push ebp` / `push esi` / `mov esi,ecx` / `mov ecx,[esp+0Ch]` /
`mov eax,[ecx]`), to 0x25CF6A, where the function goes on as the
original's; or to a return. One machine runs a whole sequence of events (the
build keeps the marked buttons between them).

Each event must give, as a model of the rule says: the original's (at
0x25CF6A every register, ESP and the two dwords pushed as the original's
ten bytes leave them, from the same input), or taken (a return, `ret 0Ch`:
EAX the button + 1, never 0; EBX, ESI, EDI, EBP kept). The rule: the
mouse's buttons are the codes 151h to 158h; one going down (kind 0) while
the cursor is shown is marked, while it is not, unmarked; a marked one held
(kind 2, or any kind above 1) or let go (kind 1, which unmarks it) while
the cursor is not shown is taken; anything else is the original's. The
original's own exe gives the original's for every event.
"""
import random
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import patch_026g as base

SITE, RESUME = 0x25CF60, 0x25CF6A
HANDLER, WORLD, STACK, STOP = 0x20000000, 0x21000000, 0x22008000, 0x23000000
FIRST = 0x151
REGS = (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP)
KEPT = (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)


class Machine:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at in (HANDLER, WORLD, STACK - 0x8000, STOP):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        emu.mem_write(WORLD, struct.pack("<I", 0x13572468))
        emu.hook_add(UC_HOOK_CODE, lambda uc, *_: uc.emu_stop(), begin=STOP, end=STOP)

    def event(self, code, kind, shown, seed):
        """One event: ("original", what the ten bytes left) or ("taken",
        EAX)."""
        emu, rng = self.emu, random.Random(seed)
        emu.mem_write(HANDLER + 0x2E0, bytes([0x01 if shown is True else shown]))
        emu.mem_write(STACK, struct.pack("<IIII", STOP, WORLD, code & 0xFFFFFFFF, kind & 0xFFFFFFFF))
        regs = {r: rng.getrandbits(32) for r in REGS}
        regs[UC_X86_REG_ECX] = HANDLER
        for r, value in regs.items():
            emu.reg_write(r, value)
        emu.reg_write(UC_X86_REG_ESP, STACK)
        emu.emu_start(SITE, RESUME, count=10000)
        if emu.reg_read(UC_X86_REG_EIP) == STOP:
            assert emu.reg_read(UC_X86_REG_ESP) == STACK + 16, "`ret 0Ch`"
            assert all(emu.reg_read(r) == regs[r] for r in KEPT), "EBX, ESI, EDI and EBP kept"
            return "taken", emu.reg_read(UC_X86_REG_EAX)
        assert emu.reg_read(UC_X86_REG_EIP) == RESUME
        esp = emu.reg_read(UC_X86_REG_ESP)
        pushed = struct.unpack("<II", emu.mem_read(esp, 8))
        # the ten bytes from these registers, as the original runs them
        expected = dict(regs)
        expected[UC_X86_REG_ESI] = HANDLER
        expected[UC_X86_REG_ECX] = WORLD
        expected[UC_X86_REG_EAX] = 0x13572468
        got = {r: emu.reg_read(r) for r in REGS if r != UC_X86_REG_EDX}
        assert got == {r: v for r, v in expected.items() if r != UC_X86_REG_EDX}, (got, expected)
        assert esp == STACK - 8 and pushed == (regs[UC_X86_REG_ESI], regs[UC_X86_REG_EBP]), "the two pushes"
        return "original", None


class Rule:
    """The rule, as a model."""

    def __init__(self):
        self.marked = set()

    def event(self, code, kind, shown):
        button = (code - FIRST) & 0xFFFFFFFF
        if button >= 8:
            return "original", None
        kind &= 0xFF
        if kind == 0:
            (self.marked.add if shown else self.marked.discard)(button)
            return "original", None
        was = button in self.marked
        if kind == 1:
            self.marked.discard(button)
        if not was or shown:
            return "original", None
        return "taken", button + 1


def sequences(rng):
    left, right = FIRST, FIRST + 1
    yield "a drag on the kits held into the start", [
        (left, 0, True), (left, 2, True), (left, 2, False), (left, 2, False), (left, 1, False), (left, 2, False),
        (left, 0, False), (left, 2, False), (left, 1, False)]
    yield "a pick on the death screen held into the respawn", [
        (right, 0, True), (right, 1, True), (right, 0, True), (right, 2, False), (right, 1, False)]
    yield "pressed while the cursor is shown, let go while it still is", [
        (left, 0, True), (left, 2, True), (left, 1, True), (left, 2, False), (left, 1, False)]
    yield "a release missed: a fresh press unmarks", [(left, 0, True), (left, 0, False), (left, 2, False),
                                                       (left, 1, False)]
    yield "every button, the cursor's byte any non-zero", [
        (FIRST + b, k, s) for b in range(8) for k, s in ((0, 0x80), (2, False), (1, False), (2, False))]
    yield "not a button: 150h, 159h, a keyboard key, codes that wrap", [
        (code, kind, shown) for code in (0x150, 0x159, 0x10, 0, 0xFFFFFFFF, FIRST + 0x100000000 - 1)
        for kind, shown in ((0, True), (2, False), (1, False))]
    yield "kinds above 2 and in the high bytes", [
        (left, 0, True), (left, 3, False), (left, 0x102, False), (left, 0x100, False), (left, 0x201, False),
        (left, 2, False)]
    for i in range(40):
        yield f"random sequence {i}", [
            (rng.choice([FIRST + rng.randrange(8), FIRST + rng.randrange(-2, 10), rng.getrandbits(32)]),
             rng.choice([0, 1, 2, 2, 3, rng.getrandbits(32)]), rng.choice([True, False, False, 0x10]))
            for _ in range(rng.randrange(1, 40))]


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    rng = random.Random(31)
    count, taken = 0, 0
    for name, events in sequences(rng):
        stock, build, rule = Machine(original), Machine(built), Rule()
        for i, (key, kind, shown) in enumerate(events):
            want = rule.event(key, kind, shown)
            assert stock.event(key, kind, shown, i) == ("original", None), (name, i)
            got = build.event(key, kind, shown, i)
            assert got == want, (name, i, (hex(key), kind, shown), got, want)
            count += 1
            taken += got[0] == "taken"
    print(f"the warmup keys: {count} mouse events as the rule says ({taken} taken), the original's own all the "
          "original's")


if __name__ == "__main__":
    main()
