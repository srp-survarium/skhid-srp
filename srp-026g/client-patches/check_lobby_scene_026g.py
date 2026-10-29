#!/usr/bin/env python3
"""Run patch 81 (rust/patches/src/lobby_scene.rs, its lohooks of
lobby_scene.toml: the lobby's scene follows the player's quest faction;
docs/notes/lobby-scenes.md) in the default build on made-up game objects,
each from the game's own code before its site to past it, the original
beside it.

    python check_lobby_scene_026g.py <original survarium.exe>

Requires Python `unicorn`. check_language_026g.py's harness (patch 79 shares
configs.start with it, a chain).

- on_configs_loaded's start: `srp_lobby_faction`, a saved string of 4 bytes,
  registered once with the game's own cc_string::cc_string, after patch 79's
  `localization`; the game's cc_string::execute sets it from a user.cfg line;
- a whole user.cfg through the console's own load (console_commands::load
  0xEBF00, as load_cc_script gives it user.cfg: 1, 2), run.py's file (LF,
  `r_resolution 1280x720` last) and the game's (CRLF) with the faction's
  line: every line to its variable (a stand-in `r_resolution` registered
  with the game's own cc_string::cc_string), the stack as it was;
- the 0x8E handler as it keeps the faction (0x2717CF to its `pop ebp`): the
  faction kept at the lobby client's +1EA998h as the original does, and the
  saved value the faction's id, user.cfg written once when it changed and
  not again for the same one; none (0) or an unknown id saved as none;
- lobby_menu::query_resources (0x21E450) from its start to its query
  (0x21E5ED): the fourth request is the original's ({"lobby_scene/day", 40h},
  the lobby menu's variant as its user data) for no faction saved,
  Renaissance or an unknown value; for another faction its lobby's config
  first ({"resources/projects/lobby_scene/<id>/client_project", 1Fh}, no user
  data), the other seven requests and their user data the original's;
- lobby_menu::on_render_scenes_ready (0x220B20): after a probe the patch asks
  the query again (query_resources with the lobby menu) and returns (ret 4)
  without the original's code, then asks the lobby's project, or the
  classic's when the config was not there; a project without its camera
  track is asked again as the classic; a usable project, or the classic's,
  goes on into the original's code (its span replayed).
"""
import struct
import sys

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_EIP

import patch_026g as base
import check_hide_ui_026g as hide
import check_language_026g as language

QUERY, QUERY_CALL = 0x21E450, 0x21E5ED
READY, READY_REPLAYED = 0x220B20, 0x220B26
FACTION, FACTION_END = 0x2717CF, 0x2717D5     # mov [ebx+1EA998h],eax ... pop ebp
CC_STRING, CONSOLE_LOAD = 0xEC860, 0xEBF00
STAND_IN, CFG = 0x30400000, 0x30410000       # a stand-in r_resolution; a user.cfg's bytes
CLIENT = 0x30100000
MENU, RESULTS = 0x30050000, 0x30060000
PROJECT = 0x30070000                  # a project's resource part (the project is at -38h)
CONFIG = 0x30078000                   # a binary config's resource


class Run(language.Run):
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_map(MENU, 0x40000)
        self.emu.mem_map(CLIENT, 0x200000)
        self.emu.mem_map(STAND_IN, 0x20000)
        self.asked_again = []
        self.stub_query = False
        self.emu.hook_add(UC_HOOK_CODE, self.query_stub, begin=QUERY, end=QUERY)

    def query_stub(self, emu, address, size, _):
        """lobby_menu::query_resources (eax the lobby menu; ret) while the
        results are taken: recorded, not run."""
        if not self.stub_query:
            return
        self.asked_again.append(emu.reg_read(UC_X86_REG_EAX))
        sp = emu.reg_read(UC_X86_REG_ESP)
        emu.reg_write(UC_X86_REG_ESP, sp + 4)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def query(self):
        """query_resources from its start to its call of the query: the
        eight {name, class} requests and their user data pointers."""
        sp = hide.STACK + 0x9000
        self.word(sp, hide.SENTINEL)
        _, _ = self.run(QUERY, (QUERY_CALL,), sp, {**hide.GAME, UC_X86_REG_EAX: MENU})
        assert self.stopped == QUERY_CALL, hex(QUERY)
        ebp = self.emu.reg_read(UC_X86_REG_EBP)
        requests = [(self.text(self.read(ebp - 0x80 + 8 * i)).decode(), self.read(ebp - 0x7C + 8 * i))
                    for i in range(8)]
        data = [self.read(ebp - 0x40 + 4 * i) for i in range(8)]
        return requests, data, ebp

    def ready(self, scene):
        """on_render_scenes_ready(results) with result 3's resource `scene`
        (0 a failed query): "replayed" (on into the original's code, its
        span replayed) or "returned" (ret 4), and the lobby menus asked again."""
        self.emu.mem_write(RESULTS, bytes(0x2E0 * 8 + 0x200))
        self.word(RESULTS + 0x12C + 3 * 0x2E0, scene)
        sp = hide.STACK + 0x9000
        self.word(sp, hide.SENTINEL)
        self.word(sp + 4, RESULTS)
        self.asked_again, self.stub_query = [], True
        try:
            _, end = self.run(READY, (READY_REPLAYED, hide.SENTINEL), sp, {**hide.GAME, UC_X86_REG_ECX: MENU})
        finally:
            self.stub_query = False
        if self.stopped == READY_REPLAYED:
            return "replayed", self.asked_again
        assert self.stopped == hide.SENTINEL and end == sp + 8, (hex(self.stopped or 0), hex(end), hex(sp))
        return "returned", self.asked_again

    def faction(self, faction):
        """0x8E's handler at its keeping of the faction (eax), the lobby
        client in ebx, to the `pop ebp` after it."""
        sp = hide.STACK + 0x9000
        self.saves = 0
        after, end = self.run(FACTION, (FACTION_END,), sp, {**hide.GAME, UC_X86_REG_EAX: faction,
                                                              UC_X86_REG_EBX: CLIENT})
        assert self.stopped == FACTION_END and end == sp
        return self.read(CLIENT + 0x1EA998), after, self.saves


def project(r, track):
    """A project's resource part at PROJECT, its track (project+1E4h) given."""
    r.emu.mem_write(PROJECT - 0x38, bytes(0x400))
    r.word(PROJECT + 0x1AC, track)
    return PROJECT


def main():
    original = open(sys.argv[1], "rb").read()
    patched, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None,
                            open(base.late_join_bin(), "rb").read())
    stock, ours = Run(original), Run(patched)
    expect = lambda ok, text: print("ok", text) if ok else sys.exit("FAIL " + text)

    # on_configs_loaded's start: the variable, once, after patch 79's
    head_before = ours.read(language.LIST_HEAD)
    _, _, head = ours.start()
    variable = language.console_variable(ours, head)
    localization = language.console_variable(ours, ours.read(head + 0x0C))
    expect(variable[0] == "srp_lobby_faction" and variable[2] == 4 and variable[3] == 1
           and localization[0] == "localization" and ours.read(ours.read(head + 0x0C) + 0x0C) == head_before,
           "configs.start: `srp_lobby_faction`, a saved string of 4 bytes, at the head of the console's list, "
           "after patch 79's `localization` (the site's chain)")
    _, _, again = ours.start()
    expect(again == head, "configs.start again: registered once")
    buffer = variable[1]
    ours.emu.mem_write(language.TEXT, b"2\0")
    ours.call_at(language.EXECUTE, (language.TEXT,), {UC_X86_REG_ECX: head})
    expect(ours.text(buffer) == b"2", "user.cfg's line `srp_lobby_faction 2` sets it "
           "(cc_string::execute 0xEC8C0)")

    # a whole user.cfg through the console's load, a stand-in r_resolution beside ours
    ours.emu.mem_write(STAND_IN, bytes(0x200))
    ours.emu.mem_write(STAND_IN + 0x100, b"r_resolution\0")
    ours.call_at(CC_STRING, (STAND_IN + 0x100, STAND_IN + 0x80, 32, 1), {UC_X86_REG_EAX: STAND_IN})
    for label, text in [
        ("run.py's (LF, r_resolution last)", b"localization english\nsrp_lobby_faction 4\nr_resolution 1280x720\n"),
        ("the game's (CRLF)", b"localization czech\r\nr_resolution 1280x720\r\nsrp_lobby_faction 1\r\n"),
        ("no end of line", b"srp_lobby_faction 3\nr_resolution 1280x720"),
    ]:
        ours.emu.mem_write(CFG, text + b"\0" * 16)
        ours.emu.mem_write(CFG + 0x1000, struct.pack("<III", CFG, CFG, len(text)))
        ours.emu.mem_write(buffer, b"\0" * 4)
        ours.emu.mem_write(STAND_IN + 0x80, bytes(32))
        _, popped = ours.call_at(CONSOLE_LOAD, (1, 2), {UC_X86_REG_ESI: CFG + 0x1000})
        faction = text.split(b"srp_lobby_faction ")[1][:1]
        expect(ours.text(buffer) == faction and ours.text(STAND_IN + 0x80) == b"1280x720" and popped == 4,
               f"user.cfg {label}: `srp_lobby_faction {faction.decode()}` and `r_resolution 1280x720` each to its "
               "variable through the console's own load; the stack as it was")

    # 0x8E's faction: kept as the original keeps it, and saved when it changed
    for label, faction, saved_after, saves in [
        ("Scavengers (1) over Black Market", 1, b"1", 1),
        ("Scavengers again", 1, b"1", 0),
        ("Edge (4)", 4, b"4", 1),
        ("none chosen (0)", 0, b"", 1),
        ("none again", 0, b"", 0),
        ("an unknown id (9)", 9, b"", 0),
        ("Renaissance (3)", 3, b"3", 1),
    ]:
        kept, after, written = ours.faction(faction)
        stock_kept, stock_after, stock_written = stock.faction(faction)
        expect(kept == faction == stock_kept and after == stock_after and ours.text(buffer) == saved_after
               and written == saves and stock_written == 0,
               f"0x8E with {label}: kept at +1EA998h and the registers as the original's; saved "
               f"{saved_after.decode() or 'none'}, user.cfg written {saves} time(s)")

    def saved(value):
        ours.emu.mem_write(buffer, value.encode() + b"\0")

    # the lobby's query and its results, a fresh scene for each case
    stock_requests, stock_data, _ = stock.query()
    stock_ready = stock.ready(project(stock, 1))
    expect(stock_requests[3] == ("lobby_scene/day", 0x40) and stock_data[3] != 0 and stock_ready == ("replayed", []),
           "the original asks lobby_scene/day (class 40h, the lobby menu's variant) and takes its results")
    config = lambda lobby: f"resources/projects/lobby_scene/{lobby}/client_project"
    # (label, saved value, [(result 3 for the query before it, the request asked, its user data kept)])
    cases = [
        ("no faction saved", "", [(project, ("lobby_scene/day", 0x40), True, "replayed")]),
        ("Renaissance (3)", "3", [(project, ("lobby_scene/day", 0x40), True, "replayed")]),
        ("an unknown value", "9", [(project, ("lobby_scene/day", 0x40), True, "replayed")]),
        ("Edge (4), hangar_046 there", "4", [(CONFIG, (config("hangar_046"), 0x1F), False, "returned"),
                                             (project, ("lobby_scene/hangar_046", 0x40), True, "replayed")]),
        ("Scavengers (1), camp_069 missing", "1", [(0, (config("camp_069"), 0x1F), False, "returned"),
                                                  (project, ("lobby_scene/day", 0x40), True, "replayed")]),
        ("Black Market (2), stk2_blockpost without its track", "2", [
            (CONFIG, (config("stk2_blockpost"), 0x1F), False, "returned"),
            ("no track", ("lobby_scene/stk2_blockpost", 0x40), True, "returned"),
            (project, ("lobby_scene/day", 0x40), True, "replayed")]),
    ]
    for label, value, steps in cases:
        fresh = Run(patched)
        _, _, fresh_head = fresh.start()
        fresh.emu.mem_write(fresh.read(fresh_head + 0x40), value.encode() + b"\0")
        for i, (result, asked, keeps_data, flow) in enumerate(steps):
            requests, data, _ = fresh.query()
            others_same = requests[:3] + requests[4:] == stock_requests[:3] + stock_requests[4:] \
                and data[:3] + data[4:] == stock_data[:3] + stock_data[4:]
            scene = project(fresh, 1) if result is project else project(fresh, 0) if result == "no track" else result
            got = fresh.ready(scene)
            again = [MENU] if flow == "returned" else []
            expect(requests[3] == asked and (data[3] != 0) == keeps_data and others_same and got == (flow, again),
                   f"{label}, query {i + 1}: asks {asked[0]} (class {asked[1]:#x}, user data "
                   f"{'kept' if keeps_data else 'none'}), the other requests the original's; its results "
                   f"{'taken' if flow == 'replayed' else 'let go, the query asked again'}")

    print("all cases passed")


if __name__ == "__main__":
    main()
