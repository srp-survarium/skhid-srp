#!/usr/bin/env python3
"""Patch 119 (KICK_TO_LOBBY): a game the match server kicks goes back to the
lobby as one whose link was lost does (client_patch.md section 3.72).

    python check_kick_lobby_026g.py <original survarium.exe>

network_client::on_match_disconnected (0x21BE50, the match link's disconnect
callback, which instant_disconnect calls with its reason) runs, in the
original and in the built exe, to its `ret 4`, on a made-up network client
of the game's own vtable 0x9FFB14 (close_current_match 0x27A790 at +58h and
lobby_client 0x219960 at +3Ch run as they are). Stubbed and written down:
the match link's +14h (the callback cleared), discard_playing_order
(0x2717E0, `31` to the lobby), on_finish_match (0x1F96F0, the world ended),
show_match_statistic (0x29AAF0), the log (has_passed_filters 0x111860,
log_message 0x111ED0, function_base::clear 0x11140); 0x145C0 says no
(the statistics handler's call is not reached).

For each state of the time rule the client keeps (+1F1C60h): 0 (the
handshake, a level loading, a loaded level waiting for its first world: no
tick played) and 1, 2, 3 (waiting for players, the countdown, in play; a
resync keeps it):
- reason 4 (0xFD, a kick): the original ends nothing in any state; the build
  does what reason 1 does, less its log line: in 1..3 the lobby gets
  `discard_playing_order(the lobby client, 1)` and the world
  `on_finish_match`; in 0 nothing, as reason 1 in 0 (the guard 0x27A7AA);
- reasons 0, 1, 3, 5 do in the build what they do in the original;
- every run ends at the caller's return with ESP past the argument, EBX,
  ESI, EDI and EBP kept, and the state 0.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX,
                               UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

ON_MATCH_DISCONNECTED = 0x21BE50
NETWORK_CLIENT_VTABLE = 0x9FFB14              # +3Ch lobby_client 0x219960, +58h close_current_match
STATE = 0x1F1C60                              # the time rule's state the client keeps
LINK = 0x1F1C48                               # the match link
LOBBY_CLIENT = 0x1E0                          # lobby_client 0x219960: this + 1E0h
GAME, GAME_WORLD, LOBBY_MENU = 0x14, 0x18, 0x5390
LOG_FILTERED, JOURNAL_FLAG = 0x1F1D60C, 0xA8394C
DISCARD, FINISH, STATISTIC = 0x2717E0, 0x1F96F0, 0x29AAF0
FILTERS, LOG, CLEAR, OPTION = 0x111860, 0x111ED0, 0x11140, 0x145C0
NET, SCRATCH, STACK = 0x42000000, 0x41000000, 0x41100000
GAME_OBJECT, LINK_OBJECT, LINK_VTABLE, MENU = SCRATCH, SCRATCH + 0x8000, SCRATCH + 0x9000, SCRATCH + 0xA000
LINK_CALLBACK = SCRATCH + 0xB000
SP, RETURN = STACK + 0x8000, SCRATCH + 0xC000
NAMES = {0xA075F4: "disconnected_by_connection_lost", 0xA075A4: "disconnected_on_expired_multipacket_ordered_message",
         0xA075D8: "disconnected_by_initiator"}


class Client:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(NET, 0x200000, UC_PROT_ALL)
        emu.mem_map(SCRATCH, 0x10000, UC_PROT_ALL)
        emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
        self.stubs = {
            LINK_CALLBACK: (4, lambda arg: ("link +14h", self.read(arg(0)))),   # an empty functor: no callback
            DISCARD: (8, lambda arg: ("31 to the lobby", arg(0), arg(1))),
            FINISH: (0, lambda arg: ("world ended", self.emu.reg_read(UC_X86_REG_EAX))),
            STATISTIC: (0, lambda arg: ("results shown", self.emu.reg_read(UC_X86_REG_ECX))),
            LOG: (None, lambda arg: ("log", NAMES.get(self.emu.reg_read(UC_X86_REG_EBX)))),
            FILTERS: (None, None),
            CLEAR: (0, None),
            OPTION: (0, None),
        }
        emu.hook_add(UC_HOOK_CODE, self.stub)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def stub(self, emu, address, size, _):
        if address not in self.stubs:
            return
        pops, record = self.stubs[address]
        sp = emu.reg_read(UC_X86_REG_ESP)
        if record is not None:
            self.calls.append(record(lambda n: self.read(sp + 4 + 4 * n)))
        emu.reg_write(UC_X86_REG_EAX, 0)                       # 0x111860: filtered out; 0x145C0: no
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + (pops or 0))     # None: cdecl

    def disconnect(self, state, reason, log):
        emu = self.emu
        emu.mem_write(NET, bytes(0x200000))
        emu.mem_write(SCRATCH, bytes(0x10000))
        emu.mem_write(STACK, bytes(0x10000))
        self.word(NET, NETWORK_CLIENT_VTABLE)
        self.word(NET + GAME, GAME_OBJECT)
        self.word(GAME_OBJECT + LOBBY_MENU, MENU)
        self.word(NET + LINK, LINK_OBJECT)
        self.word(LINK_OBJECT, LINK_VTABLE)
        self.word(LINK_VTABLE + 0x14, LINK_CALLBACK)
        self.word(NET + STATE, state)
        self.word(JOURNAL_FLAG, 0)
        # The log on: no filters, every line written; off: the filter refuses it.
        self.word(LOG_FILTERED, 0 if log else 1)
        self.word(SP, RETURN)
        self.word(SP + 4, reason)
        regs = {UC_X86_REG_EBX: 0x1111, UC_X86_REG_ESI: 0x2222, UC_X86_REG_EDI: 0x3333, UC_X86_REG_EBP: 0x4444}
        for reg, value in regs.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ECX, NET)
        emu.reg_write(UC_X86_REG_ESP, SP)
        self.calls = []
        emu.emu_start(ON_MATCH_DISCONNECTED, RETURN, count=20000)
        assert emu.reg_read(UC_X86_REG_EIP) == RETURN, hex(emu.reg_read(UC_X86_REG_EIP))
        assert emu.reg_read(UC_X86_REG_ESP) == SP + 8, "not on_match_disconnected's `ret 4`"
        assert all(emu.reg_read(reg) == value for reg, value in regs.items()), "a callee-saved register changed"
        assert self.read(NET + STATE) == 0, "the state is not cleared"
        return [call for call in self.calls if call[0] != "link +14h"], self.calls[0]


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, True)
    va, old, new = base.KICK_TO_LOBBY
    at = base.file_offset(va)
    assert original[at:at + 4] == old and built[at:at + 4] == new
    table = base.file_offset(0x27A958)
    assert original[table:table + 24] == struct.pack("<6I", 0x27A941, 0x27A867, 0x27A828, 0x27A806, 0x27A94C,
                                                     0x27A867), "close_current_match's table is not the one read"
    assert built[table:table + 16] + built[table + 20:table + 24] == \
        original[table:table + 16] + original[table + 20:table + 24], "another reason's entry changed"

    stock, patched = Client(original), Client(bytes(built))
    lobby, world, menu = NET + LOBBY_CLIENT, GAME_OBJECT + GAME_WORLD, MENU
    ends = [("31 to the lobby", lobby, 1), ("world ended", world)]
    expected = {
        0: [("world ended", world)],
        1: [("log", NAMES[0xA075F4])] + ends,
        3: [("world ended", world), ("results shown", menu)],
        5: [("log", NAMES[0xA075A4])] + ends,
    }
    for log in (False, True):
        for state in range(4):
            for client in (stock, patched):
                for reason, calls in expected.items():
                    got, cleared = client.disconnect(state, reason, log)
                    assert cleared == ("link +14h", 0), cleared
                    want = [] if state == 0 else [c for c in calls if log or c[0] != "log"]
                    assert got == want, (state, reason, log, got, want)
            kicked, _ = stock.disconnect(state, 4, log)
            assert kicked == [], ("the original ends something for a kick", state, kicked)
            kicked, _ = patched.disconnect(state, 4, log)
            lost, _ = patched.disconnect(state, 1, log)
            assert kicked == [c for c in lost if c[0] != "log"], (state, log, kicked, lost)
            assert kicked == ([] if state == 0 else ends), (state, kicked)
    print("patch 119: a kick (reason 4) in states 1, 2, 3 (waiting, countdown, in play, a resync) now tells the "
          "lobby 31 and ends the world, as a lost connection (reason 1) does, without its log line; in state 0 "
          "(handshake, loading, no world yet) every reason ends nothing, the original's guard; reasons 0, 1, 3, 5 "
          "as the original")


if __name__ == "__main__":
    main()
