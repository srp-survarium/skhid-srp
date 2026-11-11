#!/usr/bin/env python3
"""Patch 120 (unplayed_match): a match its game never played closes too
(client_patch.md section 3.74, README "A match closed before it played").

    python check_unplayed_match_026g.py <original survarium.exe>

The original and the built exe run the game's own code on a made-up network
client of the game's vtable 0x9FFB14 (its game at +14h, the game's world at
+18h, the lobby client at +1E0h, the match link at +1F1C48h), the patch's
state carried from one step to the next as in a game. Stubbed and written
down: discard_playing_order (0x2717E0, `31` to the lobby), on_finish_match
(0x1F96F0, the world ended), show_match_statistic (0x29AAF0), the match
link's calls (+14h the callback cleared, +18h/+1Ch/+20h the 0xE0 of reason 2),
the log, the lobby's 0x26F8C0. The steps, each from the game's code:
- connect: connect_to_match's span at 0x21C07C (EBX the client);
- disconnect: on_match_disconnected (0x21BE50) with a reason, to its `ret 4`;
- load end: on_project_loaded from its abort test (0x1F8AF3, EAX the lobby
  client) to the unload of the abort (0x1F8AFC) or the callback (0x1F8B24);
- ready: on_match_ready's epilogue (0x27AEBF) to its `ret 4`;
- logoff: network_client::disconnect (0x27B110) to its `ret`.

What the game holds of a match: the time rule's state (+1F1C60h, 0 never
played), the match core (+1F1C40h, set by on_match_ready), the world's
loading flag (+4E20h), the lobby client's status (+1E5790h, 0 the lobby).
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX,
                               UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

ON_MATCH_DISCONNECTED = 0x21BE50
CONNECT, CONNECT_AFTER = 0x21C07C, 0x21C086
LOAD_END, LOAD_ABORT, LOAD_ON = 0x1F8AF3, 0x1F8AFC, 0x1F8B24
READY_END, LOGOFF = 0x27AEBF, 0x27B110
NETWORK_CLIENT_VTABLE = 0x9FFB14
STATE, CORE, LINK = 0x1F1C60, 0x1F1C40, 0x1F1C48
LOBBY_CLIENT, LOBBY_STATUS = 0x1E0, 0x1E5790
GAME, GAME_WORLD, LOBBY_MENU, LOADING = 0x14, 0x18, 0x5390, 0x4E20
LOG_FILTERED, JOURNAL_FLAG = 0x1F1D60C, 0xA8394C
DISCARD, FINISH, STATISTIC, LOBBY_RESET = 0x2717E0, 0x1F96F0, 0x29AAF0, 0x26F8C0
FILTERS, LOG, CLEAR, OPTION = 0x111860, 0x111ED0, 0x11140, 0x145C0
NET, SCRATCH, STACK = 0x42000000, 0x41000000, 0x41100000
GAME_OBJECT, LINK_OBJECT, LINK_VTABLE, MENU = SCRATCH, SCRATCH + 0x8000, SCRATCH + 0x9000, SCRATCH + 0xA000
LINK_CALLBACK, LINK_PACKET, LINK_SEND, LINK_FLUSH = (SCRATCH + 0xB000 + 0x10 * i for i in range(4))
SP, RETURN, CORE_OBJECT = STACK + 0x8000, SCRATCH + 0xC000, SCRATCH + 0xD000
REGS = {UC_X86_REG_EBX: 0x1111, UC_X86_REG_ESI: 0x2222, UC_X86_REG_EDI: 0x3333, UC_X86_REG_EBP: 0x4444}

LOBBY, WORLD = NET + LOBBY_CLIENT, GAME_OBJECT + GAME_WORLD
TOLD = ("31 to the lobby", LOBBY, 1)
ENDED = ("world ended", WORLD)
# What close_current_match does past its guard for each reason (#1495's
# check): 0 the world ended; 1, 4, 5 `31` and the world; 2 0xE0 first; 3 the
# world and the results.
PAST_GUARD = {0: [ENDED], 1: [TOLD, ENDED], 2: [("0xE0", 0xE0), TOLD, ENDED], 3: [ENDED, ("results shown", MENU)],
              4: [TOLD, ENDED], 5: [TOLD, ENDED]}


class Game:
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
            LINK_CALLBACK: (4, None),
            LINK_PACKET: (4, lambda arg: ("0xE0", arg(0))),
            LINK_SEND: (4, None),
            LINK_FLUSH: (4, None),
            DISCARD: (8, lambda arg: ("31 to the lobby", arg(0), arg(1))),
            FINISH: (0, lambda arg: ("world ended", self.emu.reg_read(UC_X86_REG_EAX))),
            STATISTIC: (0, lambda arg: ("results shown", self.emu.reg_read(UC_X86_REG_ECX))),
            LOBBY_RESET: (0, None),
            LOG: (None, None),
            FILTERS: (None, None),
            CLEAR: (0, None),
            OPTION: (0, None),
            0x1B810: (0, None),                   # timer::get_elapsed_msec (reason 2's)
        }
        self.stops = set()
        emu.hook_add(UC_HOOK_CODE, self.stub)
        self.word(NET, NETWORK_CLIENT_VTABLE)
        self.word(NET + GAME, GAME_OBJECT)
        self.word(GAME_OBJECT + LOBBY_MENU, MENU)
        self.word(NET + LINK, LINK_OBJECT)
        self.word(LINK_OBJECT, LINK_VTABLE)
        for slot, at in ((0x14, LINK_CALLBACK), (0x18, LINK_PACKET), (0x1C, LINK_SEND), (0x20, LINK_FLUSH)):
            self.word(LINK_VTABLE + slot, at)
        self.word(JOURNAL_FLAG, 0)
        self.word(LOG_FILTERED, 1)                # the log filtered out
        self.match(state=0, core=False, loading=False, status=2)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def match(self, *, state=None, core=None, loading=None, status=None):
        """What the game holds of its match."""
        if state is not None:
            self.word(NET + STATE, state)
        if core is not None:
            self.word(NET + CORE, CORE_OBJECT if core else 0)
        if loading is not None:
            self.emu.mem_write(WORLD + LOADING, bytes([int(loading)]))
        if status is not None:
            self.word(LOBBY + LOBBY_STATUS, status)

    def stub(self, emu, address, size, _):
        if address in self.stops:
            self.stopped = address
            emu.emu_stop()
            return
        if address not in self.stubs:
            return
        pops, record = self.stubs[address]
        sp = emu.reg_read(UC_X86_REG_ESP)
        if record is not None:
            self.calls.append(record(lambda n: self.read(sp + 4 + 4 * n)))
        emu.reg_write(UC_X86_REG_EAX, 0)          # 0x111860: filtered out; 0x145C0: no
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + (pops or 0))

    def run(self, at, stack, regs, stops):
        self.emu.mem_write(STACK, bytes(0x10000))
        self.emu.mem_write(SP, struct.pack(f"<{len(stack)}I", *stack))
        for reg, value in {**REGS, **regs}.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, SP)
        self.calls, self.stops, self.stopped = [], set(stops), None
        self.emu.emu_start(at, RETURN, count=3000000)
        eip = self.emu.reg_read(UC_X86_REG_EIP)
        assert self.stopped is not None or eip == RETURN, hex(eip)
        return self.stopped or eip

    def kept(self, without=()):
        for reg, value in REGS.items():
            if reg not in without:
                assert self.emu.reg_read(reg) == value, f"register {reg} changed"

    def connect(self):
        assert self.run(CONNECT, [], {UC_X86_REG_EBX: NET}, {CONNECT_AFTER}) == CONNECT_AFTER
        assert self.read(NET + 0x1F1C64) == 0 and self.calls == []

    def disconnect(self, reason):
        assert self.run(ON_MATCH_DISCONNECTED, [RETURN, reason], {UC_X86_REG_ECX: NET}, ()) == RETURN
        assert self.emu.reg_read(UC_X86_REG_ESP) == SP + 8, "not on_match_disconnected's `ret 4`"
        self.kept()
        assert self.read(NET + STATE) == 0, "the state is not cleared"
        return self.calls

    def load_end(self):
        at = self.run(LOAD_END, [], {UC_X86_REG_EAX: LOBBY, UC_X86_REG_EDI: WORLD}, {LOAD_ABORT, LOAD_ON})
        assert self.calls == []
        self.kept(without=(UC_X86_REG_EDI,))
        return {LOAD_ABORT: "abort", LOAD_ON: "load on"}[at]

    def ready(self):
        """on_match_ready's epilogue: EDI, ESI, EBX popped, then the frame's EBP and `ret 4`."""
        frame = SP + 0x40
        stack = [0x3333, 0x2222, 0x1111, 0, 0, 0, NET] + [0] * 9 + [0x4444, RETURN, 0x5A5A5A5A]
        assert len(stack) * 4 == 0x4C and stack[0x18 // 4] == NET
        assert self.run(READY_END, stack, {UC_X86_REG_EBP: frame, UC_X86_REG_EBX: 0x7777,
                                           UC_X86_REG_ESI: 0x7777, UC_X86_REG_EDI: 0x7777}, ()) == RETURN
        assert self.emu.reg_read(UC_X86_REG_ESP) == frame + 12, "not on_match_ready's `ret 4`"
        self.kept()
        return self.calls

    def logoff(self):
        assert self.run(LOGOFF, [RETURN], {UC_X86_REG_ECX: NET}, ()) == RETURN
        assert self.emu.reg_read(UC_X86_REG_ESP) == SP + 4
        return self.calls


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    old, new = Game(original), Game(bytes(built))

    def both(step, *args):
        return getattr(old, step)(*args), getattr(new, step)(*args)

    def holds(**what):
        old.match(**what)
        new.match(**what)

    reasons = range(6)
    # 1. The handshake (no level asked for): the lobby told 31, once; no world.
    for reason in reasons:
        if reason == 3:
            continue      # the 0xEA is skipped without a match core (0x27C6A2): no close in the handshake
        holds(state=0, core=False, loading=False, status=2)
        both("connect")
        o, n = both("disconnect", reason)
        assert o == [] and n == [TOLD], (reason, n)
        assert both("disconnect", reason) == ([], []), "closed twice"
    print("ok the handshake: a close for reasons 0, 1, 2, 4, 5 tells the lobby 31 once, no world ended "
          "(the original: nothing)")

    # 2. Loaded, waiting for its first world (a late joiner kicked by NoWorldToJoin, reason 4;
    #    a 0xEA farewell to it, reason 3): the reason's close past the guard, once.
    for reason in reasons:
        holds(state=0, core=True, loading=False, status=2)
        both("connect")
        o, n = both("disconnect", reason)
        assert o == [] and n == PAST_GUARD[reason], (reason, n)
        holds(core=False)                         # on_finish_match released it (game_world::unload)
        assert both("disconnect", reason) == ([], []), "closed twice"
    print("ok loaded with no world yet: each reason's own close as a played match's (0xFD: 31 and the world "
          "ended), once (the original: nothing)")

    # 3. Its level loading: nothing under the load; on_match_ready's end closes it, for the reason it came with.
    for reason in reasons:
        if reason == 3:
            continue
        for lobby_status in (2, 0):               # 0: srp's 0x85 00 came before the load ended
            holds(state=0, core=False, loading=True, status=2)
            both("connect")
            assert both("disconnect", reason) == ([], [])
            holds(status=lobby_status)
            o, n = both("load_end")
            assert o == ("load on" if lobby_status else "abort") and n == "load on", (reason, lobby_status, o, n)
            holds(core=True, loading=False)       # on_match_ready: the core set, the world shown
            o, n = both("ready")
            assert o == [] and n == PAST_GUARD[reason], (reason, lobby_status, n)
            holds(core=False)
            assert both("disconnect", 1) == ([], []), "closed twice"
            assert both("ready") == ([], [])
    print("ok a level loading: nothing ended under it; the load goes on (also past a lobby's 0x85 00, which made "
          "the original unload it under the loading screen) and on_match_ready's end makes the reason's close")

    # 4. The lobby says "the lobby" while an open match loads, no close: closed as a timeout once loaded.
    holds(state=0, core=False, loading=True, status=0)
    both("connect")
    o, n = both("load_end")
    assert o == "abort" and n == "load on"
    holds(core=True, loading=False)
    assert both("ready") == ([], [ENDED])
    print("ok a lobby's 0x85 00 during an open match's load: loaded, then the world ended (reason 0's close)")

    # 5. No match open: the game's code as it is.
    holds(state=0, core=False, loading=True, status=0)
    assert both("load_end") == ("abort", "abort")
    holds(core=True, loading=False, status=2)
    assert both("ready") == ([], [])
    for reason in reasons:
        assert both("disconnect", reason) == ([], [])
    print("ok no match open (closed, or never connected): the guard's return, the load's abort, as the original")

    # 6. A logoff gives the match up.
    holds(state=0, core=True, loading=False, status=2)
    both("connect")
    assert both("logoff") == ([], [])
    assert both("disconnect", 4) == ([], [])
    print("ok a logoff: the match given up, a later close at the guard as the original")

    # 7. Played (states 1, 2, 3, a resync's): the game's close (patch 119's for 4), and then closed.
    for state in (1, 2, 3):
        for reason in reasons:
            for connected in (False, True):
                holds(state=state, core=True, loading=False, status=2)
                if connected:
                    both("connect")
                o, n = both("disconnect", reason)
                assert n == PAST_GUARD[reason], (state, reason, n)
                assert o == ([] if reason == 4 else PAST_GUARD[reason]), (state, reason, o)
                holds(core=False)
                assert both("disconnect", reason) == ([], []), "closed twice"
    print("ok played (states 1, 2, 3): each reason's close as before this patch (the original: 4 nothing), "
          "and a later close at the guard")

    # 8. A game back in its match on a new connection (NoWorldToResync): closed played, connected again,
    #    loaded again with the state 0, kicked.
    holds(state=3, core=True, loading=False, status=2)
    both("connect")
    both("disconnect", 1)
    holds(state=0, core=False, loading=True)
    both("connect")
    holds(core=True, loading=False)
    assert both("ready") == ([], [])
    assert both("disconnect", 4) == ([], PAST_GUARD[4])
    print("ok a game back in its match on a new connection, kicked before its world: 31 and the world ended")


if __name__ == "__main__":
    main()
