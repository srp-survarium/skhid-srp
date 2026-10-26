#!/usr/bin/env python3
"""Run patch 34's commit_suicide key (the Rust patch `suicide_key`) in the
default build on made-up input handlers.

    python check_suicide_key_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are; the
game's own player_input_handler code runs as machine code: the event
buffer's push_back (0x242580) fills a handler's buffer (+98h: the write and
read indexes, then 65 pairs {action, state}), its held actions are a vector
at +2A8h..+2ACh, and the input builder's last action call (the spot call at
0x242196, ebx = the handler, &state pushed with action 22h) runs with the
hook, its code and the game's 0x242460 until it returns to 0x24219B. Each
case must give bit 24 of the action mask (+33Ch) exactly when action 1Fh
went down (state 0) in the buffer, every other bit of the mask as it was,
spot's answer (al) as the original's and, when it is 1, its state (with 0
the builder reads no state, 0x24219D), the handler's buffer and
vector unchanged, ebx/esi/edi/ebp as they were and the stack as the call
leaves it. The original exe, run the same way, never sets bit 24.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI

import patch_026g as base

SITE, RETURN = base.RUST_HOOKS[0][0], base.RUST_HOOKS[0][0] + 5
PUSH_BACK = 0x242580                    # circular_buffer<pair<action, state>, 64>::push_back
HANDLER, STACK, SENTINEL = 0x20000000, 0x21000000, 0x22000000
BUFFER, HELD, MASK = 0x98, 0x2A8, 0x33C
STATE = STACK + 0x9000                  # the builder's [esp+44h]
HELD_ITEMS = HANDLER + 0x800
SUICIDE, SPOT, BIT = 0x1F, 0x22, 0x1000000
GAME = {UC_X86_REG_ESI: 0x33333333, UC_X86_REG_EDI: 0x44444444, UC_X86_REG_EBP: 0x0019FF00}
DOWN, REPEAT, UP = 0, 1, 2

# (what, events in the buffer, held actions, the mask before, bit 24 expected)
CASES = [
    ("nothing pressed", [], [], 0, False),
    ("K down", [(SUICIDE, DOWN)], [], 0, True),
    ("K repeat", [(SUICIDE, REPEAT)], [], 0, False),
    ("K up", [(SUICIDE, UP)], [], 0, False),
    ("K held from before", [], [SUICIDE], 0, False),
    ("spot down alone", [(SPOT, DOWN)], [], 0, False),
    ("K and spot down", [(SUICIDE, DOWN), (SPOT, DOWN)], [], 0, True),
    ("spot up, then K down", [(SPOT, UP), (SUICIDE, DOWN)], [], 0, True),
    ("K down among moves, fire and aim kept", [(0x0B, DOWN), (SUICIDE, DOWN), (0x26, DOWN)], [0x28], 0xA1, True),
    ("another action down (1Eh, weapon 2)", [(0x1E, DOWN)], [], 0x2000, False),
    ("K down after a full buffer's wrap", [(0x0C, DOWN)] * 70 + [(SUICIDE, DOWN)], [], 0, True),
]


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at in (HANDLER, STACK, SENTINEL):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def call(self, start, until, sp):
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.emu_start(start, until, count=200000)

    def case(self, events, held, mask):
        emu = self.emu
        emu.mem_write(HANDLER, bytes(0x1000))
        for action, state in events:
            pair = HANDLER + 0xA00
            self.word(pair, action)
            self.word(pair + 4, state)
            emu.reg_write(UC_X86_REG_ECX, HANDLER + BUFFER)
            emu.reg_write(UC_X86_REG_EDX, pair)
            sp = STACK + 0x8000
            self.word(sp, SENTINEL)
            self.call(PUSH_BACK, SENTINEL, sp)
        for i, action in enumerate(held):
            self.word(HELD_ITEMS + 4 * i, action)
        self.word(HANDLER + HELD, HELD_ITEMS)
        self.word(HANDLER + HELD + 4, HELD_ITEMS + 4 * len(held))
        self.word(HANDLER + MASK, mask)
        self.word(STATE, 0x5A5A5A5A)
        before = bytes(emu.mem_read(HANDLER, 0x1000))
        # The builder's frame at the spot call: push &state, push 22h, push
        # ebx; the call pushes 0x24219B.
        sp = STACK + 0x8000
        top = sp - 16
        for i, value in enumerate((RETURN, HANDLER, SPOT, STATE)):
            self.word(top + 4 * i, value)
        for reg, value in GAME.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_EBX, HANDLER)
        emu.reg_write(UC_X86_REG_EAX, 0x77777777)
        # Run the instruction at the site itself: the call (or the original's).
        emu.reg_write(UC_X86_REG_ESP, top + 4)
        emu.emu_start(SITE, RETURN, count=200000)
        after = bytes(emu.mem_read(HANDLER, 0x1000))
        changed = [i for i in range(0x1000) if before[i] != after[i] and not MASK <= i < MASK + 4]
        regs_kept = all(emu.reg_read(reg) == value for reg, value in GAME.items()) \
            and emu.reg_read(UC_X86_REG_EBX) == HANDLER
        return {"mask": self.read(HANDLER + MASK), "al": emu.reg_read(UC_X86_REG_EAX) & 0xFF,
                "state": self.read(STATE), "changed": changed, "regs": regs_kept,
                "esp": emu.reg_read(UC_X86_REG_ESP) == top + 16}


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    stock, patched = Run(original), Run(bytes(built))
    failed = 0
    for what, events, held, mask, expected in CASES:
        a, b = stock.case(events, held, mask), patched.case(events, held, mask)
        want = mask | (BIT if expected else 0)
        ok = (a["mask"] == mask and b["mask"] == want and a["al"] == b["al"]
              and (a["al"] == 0 or a["state"] == b["state"]) and not a["changed"] and not b["changed"]
              and a["regs"] and b["regs"] and a["esp"] and b["esp"])
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {what}: mask {mask:#x} -> {b['mask']:#x} (original {a['mask']:#x}), "
              f"spot al {b['al']} state {b['state']:#x} (original al {a['al']} state {a['state']:#x})"
              + ("" if not b["changed"] else f", handler bytes changed at {b['changed'][:4]}"))
    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
