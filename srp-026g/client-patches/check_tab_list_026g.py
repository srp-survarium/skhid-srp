#!/usr/bin/env python3
"""Run patch 39 (the scoreboard shows the places in the match and no place
whose player left) with Unicorn (2.1.4).

    python check_tab_list_026g.py <original survarium.exe>

The exe is built as patch_026g.py builds it (the checked-in late-join code);
the original is run the same way for the same cases. The game's own
instructions run on made-up objects (check_revision6_026g.py's: a hud with
its three movies, the options with their places, values whose interface
methods and the game's Scaleform calls are taken at their entry and written
down). The rules: ../docs/notes/tab-list-left-places.md.

1. update_ui's players' rows (0x23B421..0x23B477), the list shown, the
   places in the match given by the hud's state (+0C0h + 8 x place, as
   kd_stats_rule's fill_match_hud_state 0x323980 leaves it):
   - the build makes the three lists again (rebuild_lists, the game's own
     0x234880 from 0x234E71, lists_hook) when the places in the match are
     not the ones the scoreboard was last made with, and only then; the
     scoreboard (root.list_set_players) has a row for each place in the
     match and for no other, in place order, with its name; the second
     list (root.set_players_list) and the enemies' ids are the original's
     (every place);
   - set_online_player (0x23A7F0) and list_update_player (0x236D30) are
     called for the places in the match only, with the original's
     arguments;
   - every value made is released once; ebx/esi/edi/ebp and the stack at
     0x23B477 are what the original leaves (ebp is the state's hash, stored
     after the rows); the hud and the options are not written;
   - the list hidden: nothing is called, as in the original.
   The original, run the same way, keeps every place's row (the one left
   grey: set_online_player(name, 0) for it each time).
2. A list made by another run of 0x234880 (the late join's install, the
   match's start: here rebuild_lists from install_context) puts in the rows
   of the places in the match and leaves the kept mask at -1, so the next
   update of the shown list makes the lists again.
3. A player who came back as a new place with the same name: one row, the
   new place's, and set_online_player is called for it alone, so its row
   is not greyed by the place it left.
"""
import struct
import sys
from pathlib import Path

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_ESI, UC_X86_REG_EIP, UC_X86_REG_ESP, UC_X86_REG_EBP,
                               UC_X86_REG_EFLAGS)

import patch_026g as P
import check_revision6_026g as R

ROWS_VA, ROWS_END = 0x23B421, 0x23B477
SET_ONLINE, UPDATE = 0x23A7F0, 0x236D30
GAME = 0x5000000                       # [hud+18h]: the game world; +4D4Ch, then +124h: the options
ONLINE = 0xC0                          # the hud: kd_stats' online of place i at +0C0h + 8i
SHOWN = 0xBA                           # the hud: the list shown
SAVED = {UC_X86_REG_EBX: 0x11111100, UC_X86_REG_EBP: 0x22222222,
         UC_X86_REG_ESI: 0x33333333, UC_X86_REG_EDI: R.HUD}


class Machine(R.Machine):
    def __init__(self, data, teams, online, names=None, own_team=1):
        super().__init__(data, teams, own_team, True)
        uc = self.uc
        self.state = P.LATE_JOIN_STATE_VA
        uc.mem_map(GAME, 0x10000)
        self.w(R.HUD + 0x18, GAME)
        self.w(GAME + 0x4D4C, GAME + 0x1000)
        self.w(GAME + 0x1000 + 0x124, R.OPTIONS)
        for place, name in (names or {}).items():
            uc.mem_write(R.OPTIONS + place * R.PROFILE + 8, name + b"\0")
        self.set_online(online)
        self.online_calls, self.updates = [], []
        uc.hook_add(UC_HOOK_CODE, self.calls)

    def set_online(self, online):
        for place in range(20):
            # A world brings 0xFF, a tick's 0xE9 brings 1 (snapshot_rules.md section 2.7).
            self.uc.mem_write(R.HUD + ONLINE + 8 * place, bytes([(0xFF if place % 2 else 1) if place in online else 0]))

    def calls(self, uc, address, size, _):
        if address == R.REBUILD:
            self.frame = uc.reg_read(UC_X86_REG_ESP) - R.FRAME
        if address not in (SET_ONLINE, UPDATE):
            return
        esp = uc.reg_read(UC_X86_REG_ESP)
        assert uc.reg_read(UC_X86_REG_EDI) == R.HUD
        if address == SET_ONLINE:      # eax = the clan tag, ecx = the name; (the hud, online), ret 8
            assert self.r(esp + 4) == R.HUD
            place = (uc.reg_read(UC_X86_REG_ECX) - 8 - R.OPTIONS) // R.PROFILE
            assert uc.reg_read(UC_X86_REG_EAX) == R.OPTIONS + place * R.PROFILE + 0x48
            self.online_calls.append((place, self.r(esp + 8) & 0xFF))
            taken = 8
        else:                          # (the place's byte), ret 4
            self.updates.append(self.r(esp + 4) & 0xFF)
            taken = 4
        self.leave(taken)

    def kept(self):
        return self.r(self.state + P.TAB_LIST_STATE_OFFSET)

    def rows(self, shown=True):
        """Run the players' rows of update_ui; (the lists made, if any)."""
        self.events.clear()
        self.made.clear()
        self.released.clear()
        self.online_calls.clear()
        self.updates.clear()
        self.outside.clear()
        self.uc.mem_write(R.HUD + SHOWN, bytes([1 if shown else 0]))
        for register, was in SAVED.items():
            self.uc.reg_write(register, was)
        self.uc.reg_write(UC_X86_REG_ESP, R.STACK)
        self.uc.mem_write(R.STACK - 0x2000, b"\xCC" * 0x2000)
        self.written -= set(range(R.STACK - 0x2000, R.STACK))   # what this run writes counts
        self.frame = None
        self.uc.emu_start(ROWS_VA, ROWS_END, count=600000)
        assert self.uc.reg_read(UC_X86_REG_EIP) == ROWS_END, "the rows did not end at 0x23B477"
        assert self.uc.reg_read(UC_X86_REG_ESP) == R.STACK, "the stack is not what the rows leave"
        count = self.uc.mem_read(R.OPTIONS + R.COUNT_AT, 1)[0] if shown else 0
        for register, was in SAVED.items():
            # bl is the loop's place, the count at its end, as the original's.
            want = (was & ~0xFF) | count if register == UC_X86_REG_EBX else was
            if not shown and register == UC_X86_REG_EBX:
                want = was
            assert self.uc.reg_read(register) == want, "a register the code after the rows reads was changed"
        assert sorted(self.released) == sorted(self.made), "a value made and not released once"
        assert not self.strays, "a slot of 0x234880's frame read before anything wrote it"
        assert not self.outside, "the hud or the options written"
        return self.lists()

    def lists(self):
        """What the lists made hold (None: none made): the scoreboard's rows
        (names, in the array's order) and the indexes they were put at (none
        when appended), the second list's rows, the enemies' ids, the
        movies' functions invoked."""
        invoked = [event[1] for event in self.events if event[0] == "invoke"]
        if not invoked:
            return None
        arrays = [event[1] for event in self.events if event[0] == "array"]
        names = {event[1]: event[3] for event in self.events if event[0] == "member" and event[2] == "name"}
        put = {array: [] for array in arrays}
        indexes = []
        for event in self.events:
            if event[0] == "element" and event[1] in put:
                put[event[1]].append(event[3])
                if event[1] == arrays[0]:
                    indexes.append(event[2])
            elif event[0] == "append" and event[1] in put:
                put[event[1]].append(event[2])
        return {"scoreboard": [names[row] for row in put[arrays[0]]], "indexes": indexes,
                "enemies": put[arrays[1]], "second": [names[row] for row in put[arrays[2]]],
                "invoked": invoked}

    def original_lists(self):
        """The original's 0x234880 from 0x234E71 to the third list
        (0x23572B), in the frame rebuild_lists makes for it."""
        self.events.clear()
        esp = R.STACK - R.FRAME
        self.uc.mem_write(R.STACK - 0x2000, b"\xCC" * 0x2000)
        self.uc.mem_write(R.STACK, struct.pack("<III", R.STOP, R.HUD, R.OPTIONS))
        self.uc.mem_write(esp + 0x98, struct.pack("<I", R.HUD + 8))
        self.uc.mem_write(esp + 0xDC, bytes(4))
        self.uc.mem_write(esp + 0x160, bytes(4))
        self.uc.reg_write(UC_X86_REG_ESP, esp)
        self.uc.reg_write(UC_X86_REG_EBP, R.HUD)
        self.uc.reg_write(UC_X86_REG_EBX, R.HUD + 8)
        self.frame = None
        self.uc.emu_start(0x234E71, 0x23572B, count=600000)
        assert self.uc.reg_read(UC_X86_REG_EIP) == 0x23572B
        return self.lists()


def name(place, names=None):
    if names and place in names:
        return names[place].decode()
    return f"[CLAN] player{place}" if place % 2 else f"player{place}"


def check(original):
    changed = R.build(original)
    teams = [1, 2, 1, 2, 1, 2]
    three = [R.LIST_SET_PLAYERS, R.SET_PLAYERS_LIST, R.SET_ENEMY_ICON_IDS]
    every = [name(p) for p in range(len(teams))]
    plain = [f"player{p}" for p in range(len(teams))]       # the second list: the names alone
    enemies = [place for place, team in enumerate(teams) if team != 1]

    # 1. Six places: 0, 1, 3 and 5 in the match, 2 left, 4 not in yet.
    m = Machine(changed, teams, {0, 1, 3, 5})
    made = m.rows()
    assert made is not None and made["invoked"] == three, "the lists were not made again"
    assert made["scoreboard"] == [name(p) for p in (0, 1, 3, 5)] and not made["indexes"], made
    assert made["enemies"] == enemies and made["second"] == plain, made
    assert m.online_calls == [(0, 1), (1, 0xFF), (3, 0xFF), (5, 0xFF)], m.online_calls
    assert m.updates == [0, 1, 3, 5], m.updates
    assert m.kept() == 0b101011, hex(m.kept())
    # The same places again: no list made, the rows refreshed.
    assert m.rows() is None and m.updates == [0, 1, 3, 5] and m.kept() == 0b101011
    # Place 3 leaves (0xEA): made again without it.
    m.set_online({0, 1, 5})
    made = m.rows()
    assert made["scoreboard"] == [name(p) for p in (0, 1, 5)] and m.updates == [0, 1, 5], made
    assert made["second"] == plain and m.kept() == 0b100011
    # Place 4 enters; a late joiner (place 6, count 7) enters too.
    m.uc.mem_write(R.OPTIONS + R.COUNT_AT, bytes([7]))
    m.w(R.OPTIONS + 6 * R.PROFILE + R.TEAM_AT, 1)
    m.uc.mem_write(R.OPTIONS + 6 * R.PROFILE + 8, b"player6\0")
    m.set_online({0, 1, 4, 5, 6})
    made = m.rows()
    assert made["scoreboard"] == [name(p) for p in (0, 1, 4, 5, 6)] and m.updates == [0, 1, 4, 5, 6], made
    assert made["second"] == plain + ["player6"] and m.kept() == 0b1110011
    # Nobody in the match: an empty scoreboard, nothing refreshed.
    m.set_online(set())
    made = m.rows()
    assert made["scoreboard"] == [] and len(made["second"]) == 7 and m.updates == [] and m.kept() == 0, made
    # The list hidden: nothing called, the mask kept; as the original.
    m.set_online({0, 1})
    assert m.rows(shown=False) is None and not m.updates and not m.online_calls and m.kept() == 0

    # The original, the same six places: every place's row, put at its
    # index; set_online_player greys the one that left and the one not in
    # yet, and no row is ever taken out.
    o = Machine(original, teams, {0, 1, 3, 5})
    assert o.rows() is None, "the original made the lists at an update"
    assert o.online_calls == [(0, 1), (1, 0xFF), (2, 0), (3, 0xFF), (4, 0), (5, 0xFF)], o.online_calls
    assert o.updates == [0, 1, 2, 3, 4, 5], o.updates
    assert o.rows(shown=False) is None and not o.updates
    made = o.original_lists()
    assert made["scoreboard"] == every and made["indexes"] == list(range(6)) and made["second"] == plain, made
    print("PASS (the rows): the shown scoreboard is made again when a place enters or leaves, with the places in the "
          "match alone, in place order, and only then; the second list and the enemies' ids as before; "
          "set_online_player and list_update_player for the places in the match alone, with the original's "
          "arguments; every value released once, registers and stack as the original leaves them, the hud and the "
          "options not written; the list hidden: nothing; the original keeps every place's row")

    # 2. A list made by another run (the install's; the match's start is
    # the same code): the rows of the places in the match, and the mask -1,
    # so the next update of the shown list makes the lists again, once.
    m = Machine(changed, teams, {1, 2})
    m.call()
    made = m.lists()
    assert made["scoreboard"] == [name(p) for p in (1, 2)] and made["second"] == plain, made
    assert m.kept() == 0xFFFFFFFF, hex(m.kept())
    made = m.rows()
    assert made is not None and made["scoreboard"] == [name(p) for p in (1, 2)] and m.kept() == 0b110
    assert m.rows() is None
    print("PASS (another run): the install's lists have the rows of the places in the match; the mask is -1 after "
          "it, so the next update of the shown list makes them again, once")

    # 3. Place 0 left and came back as place 6, under the same name.
    names = {0: b"Returner", 6: b"Returner"}
    m = Machine(changed, teams + [1], {6}, names)
    made = m.rows()
    assert made["scoreboard"] == ["Returner"] and m.online_calls == [(6, 1)], (made, m.online_calls)
    o = Machine(original, teams + [1], {6}, names)
    o.rows()
    assert o.online_calls == [(0, 0)] + [(p, 0) for p in range(1, 6)] + [(6, 1)], o.online_calls
    assert o.original_lists()["scoreboard"].count("Returner") == 2
    print("PASS (a player back as a new place): one row, the new place's, and its online state the only one set by "
          "that name; the original has two rows and sets the name's state with the old place's first")


if __name__ == "__main__":
    check(Path(sys.argv[1]).read_bytes())
