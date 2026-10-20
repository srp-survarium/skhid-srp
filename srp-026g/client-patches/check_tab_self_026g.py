#!/usr/bin/env python3
"""Run the original's and patch 43's end of game_world::on_activate, where
the hud learns its player's name, with Unicorn (2.1.4).

    python check_tab_self_026g.py <original survarium.exe>

The exe is built as patch_026g.py builds it (with the late join), without
the late join and over on_hash_mismatch; the original is run the same way.
on_activate runs from 0x1F9395 (the network client's vtable +20h test) to
0x1F949D on a made-up game world: a game whose network client (+53ACh) is a
network client of the game's own vtable 0x9FFB14 (+20h 0x372420, `mov
al,1`, and lobby_client 0x219960 run as they are), its lobby client's
clan and name
(lobby client +1E5650h, +1E55D0h), the chat handler (+5398h), and the
journal's flag (0xA8394C) and use ([0xA83948]). The game's own string code
runs (the lengths, memcpy 0x424580, the copy 0x1F53D0); stubs: the CRT's
_snprintf (0x4257D8, the format checked), chat_handler::set_mode (0x209430,
its arguments written down) and chat_handler::set_local_player_name
(0x209AC0, which invokes root.set_local_player on the hud: the name and the
chat handler written down).

The hud's PlayerList marks the row of that name `isLocal`, and
PlayerListRenderer gives it Colors.PLAYER (0xF1E545), the kill list's own
colour (hud.swf; the rule: ../docs/notes/tab-list-left-places.md):

- the journal off (the players' games since revision 4): the original
  never gives the name; the build gives "[clan] name", or the name without
  a clan, to the chat handler set_mode gave the hud's movie;
- the journal recording (the stock default): both give the same name;
- a journal played back: the original gives none, the build the name;
- a network client whose +20h says no: neither calls set_mode or gives a
  name, as before.
The stack is as the original leaves it in every case.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_ESI, UC_X86_REG_EIP, UC_X86_REG_ESP)

import patch_026g as base

START, END = 0x1F9395, 0x1F949D
SET_MODE, SET_NAME, SNPRINTF = 0x209430, 0x209AC0, 0x4257D8
NETWORK_CLIENT_VTABLE = 0x9FFB14              # +20h 0x372420 (mov al,1), +3Ch lobby_client 0x219960
JOURNAL_FLAG, JOURNAL_USE = 0xA8394C, 0xA83948
LOBBY_CLIENT = 0x1E0                          # network client +1E0h (0x219960)
CLAN, NAME = 0x1E5650, 0x1E55D0               # in the lobby client
GAME_CHAT, GAME_NETWORK = 0x5398, 0x53AC      # in the game
WORLD, GAME, CHAT, MOVIE = 0x41000000, 0x41010000, 0x41020000, 0x41030000
NET, SCRATCH, STACK = 0x42000000, 0x41040000, 0x41108000
OFFLINE_VTABLE, NO_BANDWIDTH = SCRATCH, SCRATCH + 0x100


class World:
    def __init__(self, data, journal, clan, name, bandwidth=True):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        headers = base.Headers(data)
        for _, rva, vsize, raw, rsize, _ in headers.sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((WORLD, 0x10000), (GAME, 0x10000), (CHAT, 0x10000), (MOVIE, 0x10000),
                         (SCRATCH, 0x10000), (STACK - 0x8000, 0x10000), (NET, 0x200000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        self.word(WORLD + 0x9C, GAME)
        self.word(WORLD + 0x204, MOVIE)
        self.word(GAME + GAME_CHAT, CHAT)
        self.word(GAME + GAME_NETWORK, NET)
        if bandwidth:
            self.word(NET, NETWORK_CLIENT_VTABLE)
        else:
            # The same vtable, +20h saying no.
            emu.mem_write(OFFLINE_VTABLE, bytes(emu.mem_read(NETWORK_CLIENT_VTABLE, 0x40)))
            self.word(OFFLINE_VTABLE + 0x20, NO_BANDWIDTH)
            emu.mem_write(NO_BANDWIDTH, b"\x30\xC0\xC3")       # xor al,al / ret
            self.word(NET, OFFLINE_VTABLE)
        emu.mem_write(NET + LOBBY_CLIENT + CLAN, clan.encode() + b"\0")
        emu.mem_write(NET + LOBBY_CLIENT + NAME, name.encode() + b"\0")
        flag, use = journal
        self.word(JOURNAL_FLAG, flag)
        self.word(SCRATCH + 0x200, use)
        self.word(JOURNAL_USE, SCRATCH + 0x200)
        self.modes, self.names = [], []
        emu.hook_add(UC_HOOK_CODE, self.stub)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def text(self, at):
        out = b""
        while (c := bytes(self.emu.mem_read(at + len(out), 1))) != b"\0":
            out += c
        return out.decode()

    def returns(self, pop, value=None):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        if value is not None:
            self.emu.reg_write(UC_X86_REG_EAX, value)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pop)

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        arg = lambda n: self.read(sp + 4 + 4 * n)
        if address == SET_MODE:                 # thiscall: ecx the movie's holder; (chat, game mode), ret 8
            self.modes.append((emu.reg_read(UC_X86_REG_ECX), arg(0), arg(1) & 0xFF))
            self.returns(8)
        elif address == SET_NAME:               # edi the name; (chat), ret 4
            self.names.append((self.text(emu.reg_read(UC_X86_REG_EDI)), arg(0)))
            self.returns(4)
        elif address == SNPRINTF:               # cdecl (buffer, count, format, ...)
            assert self.text(arg(2)) == "[%s] %s", self.text(arg(2))
            out = f"[{self.text(arg(3))}] {self.text(arg(4))}".encode()[:arg(1) - 1]
            emu.mem_write(arg(0), out + b"\0")
            self.returns(0, len(out))

    def run(self):
        emu = self.emu
        emu.reg_write(UC_X86_REG_ESP, STACK)
        emu.reg_write(UC_X86_REG_ESI, WORLD)
        emu.reg_write(UC_X86_REG_EBX, 0)                # on_activate's ebx (0x1F934D)
        emu.reg_write(UC_X86_REG_EDI, WORLD + 0x1F8)
        emu.emu_start(START, END, count=100000)
        assert emu.reg_read(UC_X86_REG_EIP) == END, f"stopped at {emu.reg_read(UC_X86_REG_EIP):#x}"
        assert emu.reg_read(UC_X86_REG_ESP) == STACK, "the stack is not what on_activate's code leaves"
        assert emu.reg_read(UC_X86_REG_ESI) == WORLD
        return self.modes, self.names


OFF, RECORD, REPLAY = (0, 0), (1, 1), (1, 2)


def run(data):
    out = {}
    for label, journal, clan, name, bandwidth in (
            ("off", OFF, "SRP", "sheep", True), ("off, no clan", OFF, "", "sheep", True),
            ("record", RECORD, "SRP", "sheep", True), ("replay", REPLAY, "", "sheep", True),
            ("offline", OFF, "SRP", "sheep", False), ("offline, record", RECORD, "SRP", "sheep", False)):
        out[label] = World(data, journal, clan, name, bandwidth).run()
    return out


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    patched, _ = base.build(original, "section", code, None, late_join)
    before, _ = base.build(original, "section", code, None, None)
    mismatch, _ = base.build(original, "hash-mismatch", open(base.default_bin("hash-mismatch"), "rb").read())
    va, old, new = base.TAB_SELF
    at = base.file_offset(va)
    assert original[at:at + 2] == old and all(image[at:at + 2] == new for image in (patched, before, mismatch))
    stock, built = run(original), run(patched)
    assert run(before) == built and run(mismatch) == built, "the other builds do not give the name as this one"

    mode = [(WORLD + 0x204, CHAT, 1)]               # set_mode(the world's movie, chat, game mode)
    for result in (stock, built):
        assert all(result[k][0] == mode for k in ("off", "off, no clan", "record", "replay")), result
        assert result["offline"] == ([], []) and result["offline, record"] == ([], []), result
        assert result["record"][1] == [("[SRP] sheep", CHAT)], result["record"]
    # The original: a name only while the journal records.
    assert stock["off"][1] == [] and stock["off, no clan"][1] == [] and stock["replay"][1] == [], stock
    # The build: the name, as the rows have it, whatever the journal does.
    assert built["off"][1] == [("[SRP] sheep", CHAT)], built["off"]
    assert built["off, no clan"][1] == [("sheep", CHAT)], built["off, no clan"]
    assert built["replay"][1] == [("sheep", CHAT)], built["replay"]

    print("patch 43: with the journal off (the players' games) the original never gives the hud its player's "
          "name; the build gives \"[clan] name\" or the name; recording, both give it; played back, the build "
          "alone; when +20h says no neither; every full build the same; the stack as the original leaves it")


if __name__ == "__main__":
    main()
