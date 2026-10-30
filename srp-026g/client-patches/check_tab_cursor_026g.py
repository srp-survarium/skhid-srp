#!/usr/bin/env python3
"""Run patch 86 (rust/patches/src/tab_cursor.rs, its lohooks of tab_cursor.toml:
a cursor over the Tab list, and a friend request from its rows;
docs/notes/tab-cursor-friends.md) in the default build on made-up game
objects, the original beside it.

    python check_tab_cursor_026g.py <original survarium.exe>

Requires Python `unicorn`. check_blackout_026g.py's harness (itself
check_hide_ui_026g.py's: Scaleform's Invoke 0x511600 and show_cursor 0x23A930
stubs that record each call, the clock's imports a counter of the case's).
Stubbed besides: messaging_client::send_important_message (0x26AFB0, recorded,
`ret 0Ch`).

- the game world's keys (game_world::on_keyboard_action 0x25CD00, from its
  start to past the patch's span 0x25CD09 or to its return): Ctrl pressed
  (1Dh or 9Dh, kind 1) while the Tab list is up (the hud's +0BAh) shows the
  hud's cursor and is taken (al 1, `ret 0Ch`); its held and up events are
  taken until it is let go, after Tab too; Tab let go hides the cursor (not
  while the kits are shown); Ctrl without the list, a Ctrl held into the
  list, any other key, and every key of a Blackout match go on, registers
  and stack as the original's, which takes none and calls nothing;
- the hud movie's calls (game_world_ui::scaleform_callback 0x233D30 to its
  return): `srp_player_menu(place)` answers root.srp_player_menu(place,
  offer), the offer 1 only for a place of the match that is not the
  account's own name nor a friend's (ASCII case ignored);
  `srp_add_friend(place)` sends the lobby's friend request (the messaging
  client, the place's name, type 0, "") when it is offered, the lobby
  connection's state 0 or 4 and the lobby's allowance has one (five at once,
  one more every 10 s), and answers
  root.srp_friend_request(place, sent); a place that is not a number answers
  nothing; both return as the function does (`ret 10h`); an unknown method
  goes on to the original's dispatch in both builds; the original answers
  and sends nothing.
"""
import struct
import sys

from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESP

import patch_026g as base
import check_hide_ui_026g as hide
import check_blackout_026g as blackout

KEY_START, KEY_PAST = 0x25CD00, 0x25CD09     # on_keyboard_action's start, past the patch's span
CALLBACK = 0x233D30
SEND = 0x26AFB0
LIST, CURSOR, KITS = 0xBA, 0x2DC, 0x2DF        # the hud's flags
HUD = hide.HANDLER + 4                          # the hud inside the game world's input handler
TAB, LCTRL, RCTRL, W = 0x0F, 0x1D, 0x9D, 0x11
DOWN, UP, HELD = 1, 2, 3
# the hud movie's call: a hud of its own, its game world, game, network client
UI, WORLD, GAME, NC = 0x40600000, 0x40610000, 0x40620000, 0x40000000
OPTIONS_HOLDER, LINK, CONNECTION, FRIENDS = 0x40300000, 0x40400000, 0x40410000, 0x40500000
ARGS, TEXT, CALLER = 0x40700000, 0x40701000, 0x40702000
OPTIONS = OPTIONS_HOLDER + 0xC
LOBBY = NC + 0x1E0
MESSAGING = LOBBY + 0x1E56C4
PLACES = ["sheep", "Wolf", "Bear", "Fox"]
OWN, FRIEND_NAMES = "Sheep", ["bear", "Owl"]
FREQUENCY = 1000                                # the clock's ticks a second (check_blackout's stub)


class Run(blackout.Run):
    def __init__(self, data):
        super().__init__(data)
        for at, size in ((NC, 0x200000), (OPTIONS_HOLDER, 0x60000), (LINK, 0x1000), (CONNECTION, 0xC0000),
                         (FRIENDS, 0x1000), (UI, 0x1000), (WORLD, 0x1000), (GAME, 0x10000), (ARGS, 0x4000)):
            self.emu.mem_map(at, size, UC_PROT_ALL)
        self.sent = []
        self.emu.hook_add(UC_HOOK_CODE, self.send, begin=SEND, end=SEND)

    def send(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.sent.append((self.read(sp + 4), self.text(self.read(sp + 8)).decode(), self.read(sp + 12) & 0xFF,
                          self.text(emu.reg_read(UC_X86_REG_EDX)).decode()))
        emu.reg_write(UC_X86_REG_ESP, sp + 16)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    # ---- the keys

    def hud(self, list_shown, kits=False, cursor=None):
        self.emu.mem_write(HUD + LIST, bytes([list_shown]))
        self.emu.mem_write(HUD + KITS, bytes([kits]))
        if cursor is not None:
            self.emu.mem_write(HUD + CURSOR, bytes([cursor]))

    def key(self, key, kind):
        """on_keyboard_action(input world, key, kind) from its start: where it
        stopped, al, what it popped, its registers there, the cursor calls."""
        sp = hide.STACK + 0x8000
        for i, value in enumerate((hide.SENTINEL, 0x50505050, key, kind)):
            self.word(sp + 4 * i, value)
        regs, esp = self.run(KEY_START, (KEY_PAST, hide.SENTINEL), sp, {**hide.GAME, UC_X86_REG_ECX: hide.HANDLER})
        cursor = [call for call in self.calls if call[0] == "cursor"]
        return self.stopped, regs[UC_X86_REG_EAX] & 0xFF, esp - sp, regs, cursor

    # ---- the hud movie's calls

    def lobby(self, state=0, link=True, own=OWN, friends=FRIEND_NAMES, count=len(PLACES)):
        self.emu.mem_write(UI, bytes(0x1000))
        self.word(UI + 0x18, WORLD)
        self.word(WORLD + 0x9C, GAME)
        self.word(GAME + 0x53AC, NC)
        self.movies(UI, True, False)
        self.word(NC + 0x1F1C48, OPTIONS_HOLDER)
        for i, name in enumerate(PLACES):
            self.emu.mem_write(OPTIONS + 0x45FC * i + 8, name.encode() + b"\0")
        self.emu.mem_write(OPTIONS + 0x577B5, bytes([count]))
        self.word(LOBBY + 0x1E55C8, LINK if link else 0)
        self.word(LINK, CONNECTION)
        self.word(CONNECTION + 0xB01F4, state)
        self.emu.mem_write(LOBBY + 0x1E55D0, own.encode() + b"\0")
        self.emu.mem_write(FRIENDS, bytes(0x1000))
        for i, name in enumerate(friends):
            self.emu.mem_write(FRIENDS + 0x5C * i + 8, name.encode() + b"\0")
        self.word(MESSAGING + 0xC, FRIENDS)
        self.word(MESSAGING + 0x10, FRIENDS + 0x5C * len(friends))

    def call(self, method, argument=None):
        """scaleform_callback(caller, method, arguments, count) on the hud:
        what it popped, the movie's Invokes, the requests sent."""
        self.emu.mem_write(TEXT, method.encode() + b"\0")
        count = 0
        if argument is not None:
            self.emu.mem_write(TEXT + 0x100, argument.encode() + b"\0")
            self.emu.mem_write(ARGS, struct.pack("<IIIIII", 0, 6, TEXT + 0x100, 0, 0, 0))
            count = 1
        sp = hide.STACK + 0x8000
        for i, value in enumerate((hide.SENTINEL, CALLER, TEXT, ARGS, count)):
            self.word(sp + 4 * i, value)
        self.sent = []
        _, esp = self.run(CALLBACK, (hide.SENTINEL,), sp, {**hide.GAME, UC_X86_REG_ECX: UI})
        assert self.stopped == hide.SENTINEL, method
        return esp - sp, self.invokes(), list(self.sent)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    stock, ours = Run(original), Run(bytes(built))
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    print("the game world's keys (on_keyboard_action 0x25CD00)")
    taken = lambda cursor: (hide.SENTINEL, 1, 4 + 0xC, cursor)
    shown, hidden = [("cursor", HUD, 1)], [("cursor", HUD, 0)]
    # (what, the list shown, the kits shown, the key, its kind, taken (the cursor calls) or on (None))
    keys = [
        ("Ctrl without the list: the player's (crouch)", 0, 0, LCTRL, DOWN, None),
        ("its held and up", 0, 0, LCTRL, HELD, None),
        ("", 0, 0, LCTRL, UP, None),
        ("Tab down, then a key: on", 1, 0, W, DOWN, None),
        ("Ctrl held into the list: still the player's", 1, 0, RCTRL, HELD, None),
        ("", 1, 0, RCTRL, UP, None),
        ("left Ctrl pressed on the list: the cursor, the key taken", 1, 0, LCTRL, DOWN, shown),
        ("its held events taken", 1, 0, LCTRL, HELD, []),
        ("another key on: the moves", 1, 0, W, HELD, None),
        ("left Ctrl let go: taken, the cursor stays", 1, 0, LCTRL, UP, []),
        ("right Ctrl pressed: the cursor again, taken", 1, 0, RCTRL, DOWN, shown),
        ("Tab let go: the cursor hidden, Tab on", 1, 0, TAB, UP, None),
        ("right Ctrl still held after Tab: taken", 0, 0, RCTRL, HELD, []),
        ("right Ctrl let go: taken", 0, 0, RCTRL, UP, []),
        ("Ctrl after it: the player's again", 0, 0, RCTRL, DOWN, None),
        ("", 0, 0, RCTRL, UP, None),
        ("dead (the kits' cursor): Ctrl on the list taken", 1, 1, LCTRL, DOWN, shown),
        ("", 1, 1, LCTRL, UP, []),
        ("Tab let go: the kits keep their cursor", 1, 1, TAB, UP, None),
    ]
    for what, list_shown, kits, key, kind, want in keys:
        results = []
        for r in (stock, ours):
            r.hud(list_shown, kits)
            results.append(r.key(key, kind))
        a, b = results
        on = a[:4]
        original_on = a[0] == KEY_PAST and a[4] == []
        if want is None:
            cursor = hidden if what.startswith("Tab let go: the cursor hidden") else []
            ok = original_on and b[:4] == on and b[4] == cursor
        else:
            ok = original_on and (b[0], b[1], b[2], b[4]) == taken(want)
        report(ok, f"{what or '...'}: {'on, as the original' if want is None else 'taken'}"
                   f"{', cursor ' + str([c[2] for c in b[4]]) if b[4] else ''}")
        assert b[0] in (KEY_PAST, hide.SENTINEL)

    print("a Blackout match: no list, every key on")
    payload = blackout.empty_options(blackout.srpx((2, b"\x01")))
    report(ours.static_info(payload)[0], "the Blackout options read")
    ours.hud(1, 0)
    b = ours.key(LCTRL, DOWN)
    report(b[0] == KEY_PAST and b[4] == [], "Ctrl with Tab down in Blackout: on (crouch), no cursor")
    b = ours.key(LCTRL, UP)
    report(b[0] == KEY_PAST and b[4] == [], "its release on")
    report(ours.static_info(blackout.empty_options(b""))[0], "a plain match's options read again")
    ours.hud(1, 0)
    b = ours.key(LCTRL, DOWN)
    report(b[0] == hide.SENTINEL and b[4] == shown, "Ctrl on the list of a plain match: the cursor")
    ours.hud(1, 0)
    ours.key(TAB, UP)
    ours.key(LCTRL, UP)

    print("the hud movie's calls (scaleform_callback 0x233D30)")
    menu = lambda place, offer: [(hide.HUD_MOVIE, "root.srp_player_menu", (place, offer))]
    answer = lambda place, sent: [(hide.HUD_MOVIE, "root.srp_friend_request", (place, sent))]
    request = lambda name: [(MESSAGING, name, 0, "")]
    for r in (stock, ours):
        r.lobby()
        r.ticks = 10 * FREQUENCY
    calls = [
        # (what, the lobby's change (None: as it is), the clock's ticks after (None: as they are), the method,
        # its argument, the Invokes wanted, the requests wanted)
        ("the menu of another player's place", None, None, "srp_player_menu", "1", menu(1, 1), []),
        ("the menu of the own place (its name in another case)", None, None, "srp_player_menu", "0", menu(0, 0), []),
        ("the menu of a friend's place (in another case)", None, None, "srp_player_menu", "2", menu(2, 0), []),
        ("the menu of a place past the match's count", None, None, "srp_player_menu", "4", menu(4, 0), []),
        ("the menu of a place that is no number: no answer", None, None, "srp_player_menu", "x", [], []),
        ("the menu with no argument: no answer", None, None, "srp_player_menu", None, [], []),
        ("Add to friends: the lobby's request, sent", None, None, "srp_add_friend", "1", answer(1, 1),
         request("Wolf")),
        ("the second at once: sent", None, None, "srp_add_friend", "3", answer(3, 1), request("Fox")),
        ("the third", None, None, "srp_add_friend", "1", answer(1, 1), request("Wolf")),
        ("the fourth", None, None, "srp_add_friend", "3", answer(3, 1), request("Fox")),
        ("the fifth", None, None, "srp_add_friend", "1", answer(1, 1), request("Wolf")),
        ("the sixth at once: not sent (the allowance)", None, None, "srp_add_friend", "3", answer(3, 0), []),
        ("9.9 s after: not sent", None, 19.9, "srp_add_friend", "3", answer(3, 0), []),
        ("10 s after: sent", None, 20, "srp_add_friend", "3", answer(3, 1), request("Fox")),
        ("to a friend: not sent", None, 100, "srp_add_friend", "2", answer(2, 0), []),
        ("to the own name: not sent", None, 110, "srp_add_friend", "0", answer(0, 0), []),
        ("the lobby connection in state 4: sent", dict(state=4), 120, "srp_add_friend", "1", answer(1, 1),
         request("Wolf")),
        ("in state 1: not sent", dict(state=1), 130, "srp_add_friend", "1", answer(1, 0), []),
        ("no connection: not sent", dict(link=False), 140, "srp_add_friend", "1", answer(1, 0), []),
        ("an empty friend list: the friend's place offered", dict(friends=[]), 150, "srp_player_menu", "2",
         menu(2, 1), []),
    ]
    for what, lobby, ticks, method, argument, invokes, sent in calls:
        results = []
        for r in (stock, ours):
            if lobby is not None:
                r.lobby(**lobby)
            if ticks is not None:
                r.ticks = int(ticks * FREQUENCY)
            results.append(r.call(method, argument))
        a, b = results
        ok = a == (4 + 0x10, [], []) and b == (4 + 0x10, invokes, sent)
        report(ok, f"{what}: {b[1] or 'nothing'} {b[2] or ''}; the original answers and sends nothing")
    for r, which in ((stock, "the original"), (ours, "the build")):
        popped, invokes, sent = r.call("no_such_method", "1")
        report(popped == 4 + 0x10 and invokes == [] and sent == [],
               f"{which}: an unknown method goes on to the dispatch and returns, nothing answered or sent")

    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
