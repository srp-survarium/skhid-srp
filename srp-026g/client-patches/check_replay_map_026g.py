#!/usr/bin/env python3
"""Patch 135 (replay_map): a replay shows the enemies too (README "A replay
shows every player", docs/notes/replay-map.md).

    python check_replay_map_026g.py <original survarium.exe>

Its three sites run in the original and the patched exe on made-up objects.
The patch's state (replay_map's WATCHING, patches.state) is written as the
options reads leave it: 0 a live match (and before any), 1 a replay. The
local player is on team 1; the other player on team 1 (a teammate) or 2 (an
enemy), spotted (+348h 5) or not (0).

- update_minimap_players' tests (0x2394EB, the team test, to where they part:
  the dot listed 0x23951B or dropped 0x23950D);
- render_name's tests (0x1F2B3D, ECX the local team, to where they part: the
  marker goes on 0x1F2B5D or is hidden 0x1F2B59), and with no local player
  (local team 2) every player going on;

  the original: a teammate goes on, an enemy only while spotted; the patched
  exe the same in a live match, every enemy in a replay; each with every
  register (ESP too) the original's where it stops (the original's ja
  between the two changes none);
- initialize_match before its root.set_enemy_icon_ids (0x235709, ESI the ui)
  to past the replayed `mov ecx,[esi+10h] / mov edx,[ecx+108h]` (0x235712):
  the icons movie ([[ui+10h]+108h]+4) told root.srp_show_enemies(false) in a
  live match, (true) in a replay; no icons movie (its holder's movie 0),
  nothing told; the registers and the stack at and above ESP the original's
  (Scaleform's Invoke 0x511600 a stub that records).
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

THREAD_SLOT = 0x870118                    # GetCurrentThreadId's import
STATE = "10replay_map8WATCHING"           # the static's mangled name holds this
LIVE, REPLAY = 0, 1
TEAMMATE, ENEMY, NO_TEAM = 1, 2, 2

DOTS, DOT_LISTED, DOT_DROPPED = 0x2394EB, 0x23951B, 0x23950D
NAMES, NAME_ON, NAME_HIDDEN = 0x1F2B3D, 0x1F2B5D, 0x1F2B59
IDS, IDS_PAST = 0x235709, 0x235712
INVOKE = 0x511600                         # GFx::Movie::Invoke: thiscall (name, result, arguments, count), ret 10h
GFX_BOOL = 2

DATA = 0x30000000
LOCAL, PLAYER, LOCAL_TEAM, PLAYER_TEAM, THREAD, UI, HOLDER, MOVIE_UI = (DATA + 0x10000 * i for i in range(8))
ICONS_MOVIE = DATA + 0x90000
DATA_SIZE = 0xA0000
STACK = 0x21000000
GAME = {UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_ECX: 0x0C0C0C0C, UC_X86_REG_EDX: 0x0D0D0D0D,
        UC_X86_REG_EBP: 0x0019FF00, UC_X86_REG_ESI: 0, UC_X86_REG_EDI: 0x0E0E0E0E}
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP, UC_X86_REG_ESP)


class Game:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(DATA, DATA_SIZE, UC_PROT_ALL)
        emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
        emu.hook_add(UC_HOOK_CODE, self.stop)
        emu.hook_add(UC_HOOK_CODE, self.invoke, begin=INVOKE, end=INVOKE)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def stop(self, emu, address, size, _):
        if address in self.stops and self.started:
            self.stopped = address
            emu.emu_stop()
        self.started = True

    def invoke(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        name, _result, arguments, count = (self.read(sp + 4 * i) for i in range(1, 5))
        values = [struct.unpack("<II", emu.mem_read(arguments + 24 * i + 4, 8)) for i in range(count)]
        self.told.append((emu.reg_read(UC_X86_REG_ECX), bytes(emu.mem_read(name, 64)).split(b"\0")[0].decode(),
                          tuple(v & 0xFF if t == GFX_BOOL else (t, v) for t, v in values)))
        emu.reg_write(UC_X86_REG_EAX, 1)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + 16)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def players(self, team, spotted):
        self.emu.mem_write(DATA, bytes(DATA_SIZE))
        self.emu.mem_write(THREAD, bytes.fromhex("b801000000c3"))     # GetCurrentThreadId: mov eax,1 / ret
        self.word(THREAD_SLOT, THREAD)
        self.word(LOCAL + 0x6B8, LOCAL_TEAM)
        self.word(LOCAL_TEAM + 0x45EC, TEAMMATE)
        self.word(PLAYER + 0x6B8, PLAYER_TEAM)
        self.word(PLAYER_TEAM + 0x45EC, team)
        self.word(PLAYER + 0x348, spotted)

    def run(self, start, stops, regs, stack=()):
        sp = STACK + 0x8000
        self.emu.mem_write(STACK, bytes(0x10000))
        for i, value in enumerate(stack):
            self.word(sp + 4 * i, value)
        for reg, value in {**GAME, **regs}.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.stops, self.stopped, self.started, self.told = stops, None, False, []
        self.emu.emu_start(start, 0, count=500000)
        assert self.stopped is not None, hex(self.emu.reg_read(UC_X86_REG_EIP))
        return (self.stopped, {reg: self.emu.reg_read(reg) for reg in REGS},
                bytes(self.emu.mem_read(sp, 0x100)), self.told)

    def dot(self, team, spotted):
        self.players(team, spotted)
        return self.run(DOTS, (DOT_LISTED, DOT_DROPPED), {UC_X86_REG_EAX: PLAYER, UC_X86_REG_EDI: LOCAL})

    def name(self, team, spotted, local_team=TEAMMATE):
        self.players(team, spotted)
        return self.run(NAMES, (NAME_ON, NAME_HIDDEN), {UC_X86_REG_ESI: PLAYER, UC_X86_REG_ECX: local_team})

    def ids(self, movie):
        self.players(TEAMMATE, 0)
        self.word(UI + 0x10, HOLDER)
        self.word(HOLDER + 0x108, MOVIE_UI)
        self.word(MOVIE_UI + 4, ICONS_MOVIE if movie else 0)
        return self.run(IDS, (IDS_PAST,), {UC_X86_REG_ESI: UI})


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    state = [va for name, va, block, _ in base.arena().every if STATE in name and block == "patches.state"]
    assert len(state) == 1, state
    stock, patched = Game(original), Game(bytes(built))
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    players = (("a teammate", TEAMMATE, 0), ("a teammate, spotted", TEAMMATE, 5),
               ("an enemy, not spotted", ENEMY, 0), ("an enemy, spotted", ENEMY, 5))
    stock_goes = (True, True, False, True)
    for watching, match in ((LIVE, "a live match"), (REPLAY, "a replay")):
        patched.emu.mem_write(state[0], bytes([watching]))
        for (who, team, spotted), stock_on in zip(players, stock_goes):
            goes = stock_on or watching == REPLAY
            for what, run, on, off in (("its dot", Game.dot, DOT_LISTED, DOT_DROPPED),
                                       ("its marker and name", Game.name, NAME_ON, NAME_HIDDEN)):
                a = run(stock, team, spotted)
                b = run(patched, team, spotted)
                report(a[0] == (on if stock_on else off) and b[0] == (on if goes else off) and a[1] == b[1],
                       f"{match}, {who}: {what} {'shown' if goes else 'hidden'}; the registers the original's")
        a = stock.name(ENEMY, 0, NO_TEAM)
        b = patched.name(ENEMY, 0, NO_TEAM)
        report(a[0] == b[0] == NAME_ON and a[1] == b[1] and b[3] == [],
               f"{match}, no local player: an enemy's marker and name shown, as the original")

        for movie in (True, False):
            a = stock.ids(movie)
            b = patched.ids(movie)
            want = [(ICONS_MOVIE, "root.srp_show_enemies", (watching,))] if movie else []
            report(a[0] == b[0] == IDS_PAST and a[1] == b[1] and a[2] == b[2] and a[3] == [] and b[3] == want,
                   f"{match}, initialize_match's enemy list: "
                   + (f"the icons movie told srp_show_enemies({bool(watching)})" if movie else "no icons movie, nothing told")
                   + "; the replayed movs, the registers and the stack the original's")

    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
