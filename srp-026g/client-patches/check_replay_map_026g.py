#!/usr/bin/env python3
"""Patch 135 (replay_map): a replay's minimap shows the enemies too (README
"A replay's map shows every player").

    python check_replay_map_026g.py <original survarium.exe>

update_minimap_players' tests of a place's player, run from its team test
(0x2394EB) to where they part, in the original and the patched exe, on
made-up players: listed (0x23951B, the icon object made) or dropped
(0x23950D). The local player (EDI) on team 1; the place's player (EAX) on
team 1 (a teammate) or 2 (an enemy), spotted (+348h 5) or not (0). The
patch's state (replay_map's WATCHING, patches.state) is written as the
options reads leave it: 0 a live match (and before any), 1 a replay.

- the original: a teammate listed, an enemy only while spotted;
- the patched exe in a live match: the same;
- the patched exe in a replay: every enemy listed, spotted or not;

each with every register (ESP too) the original's where it stops (the
original's ja between the two changes none).
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

START = 0x2394EB                          # mov edx,[edi+6B8h]: the team test
LISTED, DROPPED = 0x23951B, 0x23950D
THREAD_SLOT = 0x870118                    # GetCurrentThreadId's import
STATE = "10replay_map8WATCHING"           # the static's mangled name holds this
LIVE, REPLAY = 0, 1

DATA = 0x30000000
LOCAL, PLAYER, LOCAL_TEAM, PLAYER_TEAM, THREAD = (DATA + 0x10000 * i for i in range(5))
DATA_SIZE = 0x50000
STACK = 0x21000000
GAME = {UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_ECX: 0x0C0C0C0C, UC_X86_REG_EDX: 0x0D0D0D0D,
        UC_X86_REG_EBP: 0x0019FF00, UC_X86_REG_ESI: 0}
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP, UC_X86_REG_ESP)


class Minimap:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(DATA, DATA_SIZE, UC_PROT_ALL)
        emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
        emu.hook_add(UC_HOOK_CODE, self.stop)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def stop(self, emu, address, size, _):
        if address in (LISTED, DROPPED) and self.started:
            self.stopped = address
            emu.emu_stop()
        self.started = True

    def test(self, team, spotted):
        """The tests for a player of `team` spotted for `spotted`: where they go, the registers there."""
        self.emu.mem_write(DATA, bytes(DATA_SIZE))
        self.emu.mem_write(THREAD, bytes.fromhex("b801000000c3"))     # GetCurrentThreadId: mov eax,1 / ret
        self.word(THREAD_SLOT, THREAD)
        self.word(LOCAL + 0x6B8, LOCAL_TEAM)
        self.word(LOCAL_TEAM + 0x45EC, 1)
        self.word(PLAYER + 0x6B8, PLAYER_TEAM)
        self.word(PLAYER_TEAM + 0x45EC, team)
        self.word(PLAYER + 0x348, spotted)
        sp = STACK + 0x8000
        self.emu.mem_write(STACK, bytes(0x10000))
        for reg, value in {**GAME, UC_X86_REG_EAX: PLAYER, UC_X86_REG_EDI: LOCAL}.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.stopped, self.started = None, False
        self.emu.emu_start(START, 0, count=500000)
        assert self.stopped is not None, hex(self.emu.reg_read(UC_X86_REG_EIP))
        return self.stopped, {reg: self.emu.reg_read(reg) for reg in REGS}


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    state = [va for name, va, block, _ in base.arena().every if STATE in name and block == "patches.state"]
    assert len(state) == 1, state
    stock, patched = Minimap(original), Minimap(bytes(built))
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    players = (("a teammate", 1, 0), ("a teammate, spotted", 1, 5), ("an enemy, not spotted", 2, 0),
               ("an enemy, spotted", 2, 5))
    want = {LIVE: (LISTED, LISTED, DROPPED, LISTED), REPLAY: (LISTED, LISTED, LISTED, LISTED)}
    for watching, name in ((LIVE, "a live match"), (REPLAY, "a replay")):
        for (who, team, spotted), goes, stock_goes in zip(players, want[watching], want[LIVE]):
            patched.emu.mem_write(state[0], bytes([watching]))
            a_at, a = stock.test(team, spotted)
            b_at, b = patched.test(team, spotted)
            report(b_at == goes and a_at == stock_goes and a == b,
                   f"{name}, {who}: {'listed' if goes == LISTED else 'dropped'}; the registers the original's")

    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
