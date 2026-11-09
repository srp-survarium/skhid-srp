#!/usr/bin/env python3
"""Run patch 131 (rust/patches/src/live_watch.rs, its lohooks of live_watch.toml: the
Settings' "Allow others to watch my matches live", an account setting srp keeps) in the
default build on made-up game objects: game_options::scaleform_callback (0x1FF5D0) and
read_squad_info's two sites, after patch 81's hooks at the same places.

    python check_live_watch_026g.py <original survarium.exe>

Requires Python `unicorn` (check_lobby_scene_026g.py's harness). The lobby connection is the
one every messaging sender checks ([[client+1E55C8h]]+0B01F4h, states 0 and 4 send). Checked:

- the stock exe does not answer the name; another method's call is left to the rest of the chain;
- before srp says (no 0xA1, or a 0xA1 of an older srp without the third byte) the box is told
  "" and an Apply sends nothing;
- 0xA1's third byte is told (1 "1", 2 "0"; another byte changes nothing), for no squad and for a
  squad, patch 81's picks as before;
- an Apply ("0", "1") sends 3B at 2 096 000 000 + id, count 1, with the game's lobby client, and
  answers the new state; nothing goes to user.cfg; another value sends nothing;
- a lobby connection that is down (or none) answers "" and sends nothing, its setting kept for
  when it is up;
- the login screen's chain (login_menu::on_activate 0x21EAC8) after a 0xA1 leaves the state: the box does not
  depend on the order of a sign-in; with the connection down there is no box, and the next 0xA1 says the state.
"""
import sys

import struct

from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP

import patch_026g as base
import check_hide_ui_026g as hide
import check_hud_options_026g as hud
import check_language_026g as language
import check_lobby_scene_026g as lobby

NET = 0x30800000                     # a network client; its lobby client inside
LINK, CONNECTION = 0x30A00000, 0x30A10000
LOBBY_CLIENT = NET + 0x1E0
SENDING, DOWN = 0, 1                 # a connection's state a sender sends at, and another
BASE = 2_096_000_000
NAME = "srp_live_watch"
LABEL = "Allow others to watch my matches live"
ANSWER = "root.srp_check_option"
language.ENGLISH["st_srp_live_watch_option"] = LABEL.encode()


class Run(hud.Run):
    """check_hud_options_026g.py's game (the options callback, the login screen's chain), with
    read_squad_info's sites and a lobby connection added (check_lobby_scene_026g.py's objects)."""
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_map(lobby.MENU, 0x40000, UC_PROT_ALL)
        self.emu.mem_map(lobby.CLIENT, 0x200000, UC_PROT_ALL)
        self.emu.mem_map(NET, 0x200000, UC_PROT_ALL)
        self.emu.mem_map(LINK, 0x1000, UC_PROT_ALL)
        self.emu.mem_map(CONNECTION, 0xC0000, UC_PROT_ALL)
        self.word(LINK, CONNECTION)
        self.word(language.GAME + 0x53AC, NET)
        self.connect(True)
        self.sent = []
        self.emu.hook_add(UC_HOOK_CODE, self.players_range, begin=lobby.PLAYERS_RANGE, end=lobby.PLAYERS_RANGE)

    def players_range(self, emu, address, size, _):
        """lobby_client's 3B (EAX the client; ret 8): recorded, not sent."""
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.sent.append((emu.reg_read(UC_X86_REG_EAX), self.read(sp + 4), self.read(sp + 8) & 0xFF))
        emu.reg_write(UC_X86_REG_ESP, sp + 12)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def squad(self, squad_id, rest):
        """read_squad_info at its two sites (check_lobby_scene_026g.py's), `rest` the message's bytes left."""
        self.word(lobby.CLIENT + 0x1EAAA4, squad_id)
        self.emu.mem_write(lobby.MESSAGE, b"\xA1" * 8 + rest)
        self.emu.mem_write(lobby.READER, struct.pack("<III", lobby.MESSAGE, lobby.MESSAGE + 8, 8 + len(rest)))
        sp = hide.STACK + 0x9000
        self.run(lobby.SQUAD_ID, (lobby.SQUAD_ID_END,), sp, {**hide.GAME, UC_X86_REG_EBX: lobby.CLIENT, UC_X86_REG_ESI: lobby.READER})
        assert self.stopped == lobby.SQUAD_ID_END, hex(self.stopped or 0)
        if squad_id == 0:
            return
        self.run(lobby.SQUAD, (lobby.SQUAD_END,), sp, {**hide.GAME, UC_X86_REG_EBX: lobby.CLIENT, UC_X86_REG_ESI: lobby.READER,
                                                        UC_X86_REG_ECX: 0x77})
        assert self.stopped == lobby.SQUAD_END, hex(self.stopped or 0)

    def connect(self, up):
        self.word(LOBBY_CLIENT + 0x1E55C8, LINK if up is not None else 0)
        self.word(CONNECTION + 0xB01F4, SENDING if up else DOWN)

    def ask(self, value=None):
        """The Settings' call: (what it invoked, the 3B sent, saves)."""
        self.sent = []
        invokes, saves = self.callback(NAME, value)
        return invokes, list(self.sent), saves


def answered(state):
    return [(language.MOVIE, ANSWER, (NAME, LABEL, state))]


def normal(invokes):
    return [(movie, method, tuple(v.decode() if isinstance(v, bytes) else v for v in values))
            for movie, method, values in invokes]


def main():
    original = open(sys.argv[1], "rb").read()
    patched, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None,
                            open(base.late_join_bin(), "rb").read())
    expect = lambda ok, text: print("ok", text) if ok else sys.exit("FAIL " + text)

    stock = Run(original)
    invokes, sent, _ = stock.ask()
    expect(invokes == [] and sent == [], "the stock exe answers nothing to the name")

    r = Run(patched)
    invokes, sent, saves = r.ask()
    expect(normal(invokes) == answered("") and sent == [], f"no 0xA1 yet: the box is told \"\" (state not known): {invokes}")
    invokes, sent, saves = r.ask("0")
    expect(normal(invokes) == answered("") and sent == [], "an Apply before the state is known: nothing sent")

    r.squad(0, b"\x00\x00")
    invokes, sent, _ = r.ask()
    expect(normal(invokes) == answered("") and sent == [], "a 0xA1 of an older srp (no third byte): still not known")
    r.sent = []
    r.squad(0, b"\x00\x00\x09")
    invokes, _, _ = r.ask()
    expect(normal(invokes) == answered(""), "a third byte that is no setting (9): still not known")

    r.squad(0, b"\x00\x00\x01")
    invokes, sent, _ = r.ask()
    expect(normal(invokes) == answered("1") and sent == [], "0xA1 of no squad, third byte 1: ticked, nothing sent")
    r.squad(7, b"\x00\x00\x02")
    invokes, sent, _ = r.ask()
    expect(normal(invokes) == answered("0") and sent == [], "0xA1 of squad 7, third byte 2: unticked")
    r.squad(0, b"\x00\x00\x01")

    invokes, sent, saves = r.ask("0")
    expect(sent == [(LOBBY_CLIENT, BASE + 2, 1)] and normal(invokes) == answered("0") and saves == 0,
           f"Apply 0: 3B at 2 096 000 002, count 1, the game's lobby client; answered 0; not user.cfg's: {sent}")
    invokes, sent, _ = r.ask("1")
    expect(sent == [(LOBBY_CLIENT, BASE + 1, 1)] and normal(invokes) == answered("1"), f"Apply 1: 3B at 2 096 000 001: {sent}")
    for bad in ("2", "", "yes", "01"):
        invokes, sent, saves = r.ask(bad)
        expect(sent == [] and normal(invokes) == answered("1") and saves == 0, f"Apply {bad!r}: nothing sent, unchanged")

    r.connect(False)
    invokes, sent, _ = r.ask()
    expect(normal(invokes) == answered("") and sent == [], "the lobby connection not sending: no box")
    invokes, sent, _ = r.ask("0")
    expect(normal(invokes) == answered("") and sent == [], "and an Apply then sends nothing")
    r.connect(None)
    invokes, sent, _ = r.ask("0")
    expect(normal(invokes) == answered("") and sent == [], "no lobby connection at all: no box, nothing sent")
    r.connect(True)
    invokes, _, _ = r.ask()
    expect(normal(invokes) == answered("1"), "up again: the state kept")

    invokes, saves = r.callback("srp_no_such")
    expect(invokes == [] and saves == 0, "another method: left to the chain, nothing answered")

    r.login()
    invokes, sent, _ = r.ask()
    expect(normal(invokes) == answered("1") and sent == [],
           "the login screen's chain after a 0xA1 (any order of a sign-in): the state kept, the box shown")
    r.connect(False)
    r.login()
    invokes, _, _ = r.ask()
    expect(normal(invokes) == answered(""), "signed out (the connection down): no box")
    r.connect(True)
    r.squad(0, b"\x00\x00\x02")
    invokes, _, _ = r.ask()
    expect(normal(invokes) == answered("0"), "the next sign-in's 0xA1 says the state again")


if __name__ == "__main__":
    main()
