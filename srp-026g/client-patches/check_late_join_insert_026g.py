#!/usr/bin/env python3
"""Patch 82 (late_join_insert): a world loaded from the server (0xE8) puts
its alive players into it, as their spawn would have.

Runs the installed Rust hook at network_client::process_dynamic_match_info's
join after game_world_core::deserialize (0x27C116) to the je after it, in the
patched exe, on a made-up client and players. The client's match_options
(0x219990), get_active_player (0x27A9C0) and player::insert (0x1F2720) are
stubbed: what is checked is which players the hook inserts and how. An alive
player outside the world (m_is_alive +33Ch 1, m_has_been_inserted +33Fh 0) is
inserted once, with 1 (as the rule spawn, 0x316EA5); an inserted or a dead
player is not; a second load inserts nobody again; an empty place is skipped;
each reference get_active_player adds is given back; the registers (EAX the
replaced mov's), the stack and every other byte unchanged.
"""
import struct
import sys
from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import *
import patch_026g as base
import check_hide_ui_026g as hide

SITE, AFTER = 0x27C116, 0x27C11B          # mov eax,[edi+4] / test eax,eax, then je 0x27C12B
MATCH_OPTIONS = 0x219990                  # ecx the client; eax the options; ret
ACTIVE_PLAYER = 0x27A9C0                  # ecx the client; out, place; eax out; ret 8
INSERT = 0x1F2720                         # ecx the player; link; ret 4
ORIGINAL_BYTES = bytes.fromhex("8b470485c0")

DATA = 0x30000000
CLIENT, OPTIONS, LOCAL = DATA, DATA + 0x1000, DATA + 0x2000
PLAYERS = DATA + 0x100000                  # 0x1000 bytes a player
PLACES = 0x577B5                           # match_options' places byte
ALIVE, INSERTED, REFERENCES = 0x33C, 0x33F, 0xD0


def player(place):
    return PLAYERS + 0x1000 * place


class Run(hide.Run):
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_map(DATA, 0x200000, UC_PROT_ALL)
        self.inserted = []
        self.places = {}
        for stub in (MATCH_OPTIONS, ACTIVE_PLAYER, INSERT):
            self.emu.hook_add(UC_HOOK_CODE, self.engine, begin=stub, end=stub)

    def engine(self, emu, address, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        ecx = emu.reg_read(UC_X86_REG_ECX)
        if address == MATCH_OPTIONS:
            assert ecx == CLIENT
            emu.reg_write(UC_X86_REG_EAX, OPTIONS)
            pops = 0
        elif address == ACTIVE_PLAYER:
            assert ecx == CLIENT
            out, place = self.read(sp + 4), self.read(sp + 8)
            found = self.places.get(place)
            self.word(out, found or 0)
            if found:
                self.word(found + REFERENCES, self.read(found + REFERENCES) + 1)
            emu.reg_write(UC_X86_REG_EAX, out)
            pops = 8
        else:
            link = self.read(sp + 4)
            self.inserted.append((ecx, link))
            emu.mem_write(ecx + INSERTED, b"\x01")
            pops = 4
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def setup(self, players, count=None):
        """`players`: {place: (alive, inserted)}; the options' places `count`."""
        self.emu.mem_write(DATA, bytes(0x200000))
        self.word(CLIENT + 4, LOCAL)
        self.emu.mem_write(OPTIONS + PLACES, bytes([len(players) if count is None else count]))
        self.places = {}
        for place, (alive, inserted) in players.items():
            at = player(place)
            self.emu.mem_write(at + ALIVE, bytes([alive]))
            self.emu.mem_write(at + INSERTED, bytes([inserted]))
            self.word(at + REFERENCES, 1)           # the place's own
            self.places[place] = at

    def load(self):
        """The site to the je: the players inserted, what changed of DATA."""
        before = self.read(DATA, 0x200000)
        regs = {**hide.GAME, UC_X86_REG_EDI: CLIENT}
        self.inserted = []
        after, sp = self.run(SITE, (AFTER,), hide.STACK + 0x9000, regs)
        assert self.stopped == AFTER, hex(self.stopped or 0)
        assert sp == hide.STACK + 0x9000
        assert after == {**regs, UC_X86_REG_EAX: LOCAL}, after
        now = self.read(DATA, 0x200000)
        changed = {DATA + i for i in range(len(now)) if before[i] != now[i]}
        return self.inserted, changed


def main():
    original = open(sys.argv[1], 'rb').read()
    at = hide.file_at(original, SITE)
    assert original[at:at + 5] == ORIGINAL_BYTES, original[at:at + 5].hex()
    patched, _ = base.build(original, 'section', open(base.default_bin('section'), 'rb').read(), None,
                            open(base.late_join_bin(), 'rb').read())
    r = Run(patched)

    # A late joiner's world: places 0..4 alive, never inserted; 5 dead; 6 alive and inserted.
    r.setup({0: (1, 0), 1: (1, 0), 2: (1, 0), 3: (1, 0), 4: (1, 0), 5: (0, 0), 6: (1, 1)})
    inserted, changed = r.load()
    assert inserted == [(player(p), 1) for p in range(5)], inserted
    assert changed == {player(p) + INSERTED for p in range(5)}, sorted(map(hex, changed))
    print('ok a loaded world: each alive player outside it inserted once, with 1; the dead and the '
          'inserted untouched; every reference given back')

    inserted, changed = r.load()
    assert inserted == [] and changed == set()
    print('ok a second load (a resync): nobody inserted twice')

    r.setup({0: (1, 0), 2: (1, 0)}, count=4)
    inserted, _ = r.load()
    assert inserted == [(player(0), 1), (player(2), 1)], inserted
    print('ok an empty place skipped')

    r.setup({}, count=0)
    assert r.load() == ([], set())
    print('ok no places: nothing')

    r.setup({0: (0, 1)})
    assert r.load() == ([], set())
    print('ok a dead player still inserted: left as it is (its death is the rule\'s)')


if __name__ == '__main__':
    main()
