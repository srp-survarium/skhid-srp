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
  goes on into the original's code (its span replayed);
- after a match (lobby-scenes.md, "After a match"): lobby_menu::on_deactivate
  (0x21D4A0) with another faction saved than the lobby shown asks the
  query (query_resources with the lobby menu) and goes on into its own code;
  with the same faction it asks nothing; the swap's query asks the lobby's
  config, then its project, alone (every other request that lobby's config,
  class 1Fh, no user data); its project's results are put in the old
  project's place and returned from (ret 4): the calls made (the old
  project's remove, its static collisions' remove with the physics world,
  the track player's stop, the release at a count of 0; the new project's
  vtable+0Ch, the stop, its track's emit through the emitter's vtable, the
  play with the menu's listener, the made effect's destroy at 0, the insert,
  its collisions' insert) and the lobby menu's +114h, +118h and the counts
  are what the original's clear_resources (0x21D080..0x21D100) and
  on_render_scenes_ready (0x220DA2..0x220EC3, 0x22193E..0x221975) do on the
  same objects; results after on_activate (0x21D2E0) are let go untouched,
  and the next match asks again; a missing pack is the classic's project;
  game::clear_resources (0x1ED030, the game closing) makes its on_deactivate
  ask nothing, and a swap's results after it are let go untouched.
"""
import struct
import sys

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_ESP, \
    UC_X86_REG_EBP, UC_X86_REG_EIP

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
DEACTIVATE, DEACTIVATE_REPLAYED = 0x21D4A0, 0x21D4A6
ACTIVATE, ACTIVATE_REPLAYED = 0x21D2E0, 0x21D2E6
CLOSING, CLOSING_REPLAYED = 0x1ED030, 0x1ED03A          # game::clear_resources' start, its 10 bytes
CLEAR, CLEAR_PROJECT_END = 0x21D080, 0x21D100            # clear_resources' project half
READY_PROJECT, READY_PROJECT_END = 0x220DA2, 0x220EC3    # on_render_scenes_ready's, then its insert
READY_INSERT, READY_INSERT_END = 0x22193E, 0x221975
OLD = 0x30080000                      # the shown project's resource part
OLD_COLLISIONS, NEW_COLLISIONS = 0x30084000, 0x30074000
EMITTER, EFFECT = 0x30076000, 0x30076100
WORLD, LISTENER = 0x3008F000, MENU + 0x684
SIMPLE_GAME_PROJECT, STATIC_COLLISION, HUD_EMITTER = 0xA0A064, 0xA029A8, 0xA0BC7C
# the game's functions the swap reaches, stubbed: name, what it records, its pops
SWAP_STUBS = {0x2855B0: "remove", 0x285540: "insert", 0x2F8720: "on_inserted", 0x2F8850: "collision_insert",
              0x2F8970: "collision_remove", 0x2DE7F0: "stop", 0x2DE090: "play", 0x299FF0: "emit",
              0x1F65D0: "destroy", 0xE0210: "release"}


class Run(language.Run):
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_map(MENU, 0x40000)
        self.emu.mem_map(CLIENT, 0x200000)
        self.emu.mem_map(STAND_IN, 0x20000)
        self.asked_again = []
        self.stub_query = False
        self.emu.hook_add(UC_HOOK_CODE, self.query_stub, begin=QUERY, end=QUERY)
        self.log = None                   # the swap's stubs' record, while one runs
        for at in SWAP_STUBS:
            self.emu.hook_add(UC_HOOK_CODE, self.swap_stub, begin=at, end=at)

    def swap_stub(self, emu, address, size, _):
        """The game's functions the swap reaches (SWAP_STUBS), recorded and
        returned from with their own pops; emit makes EFFECT (its count
        taken) into its out."""
        if self.log is None:
            return
        name, sp = SWAP_STUBS[address], emu.reg_read(UC_X86_REG_ESP)
        arg = lambda i: self.read(sp + 4 * i)
        reg = lambda r: emu.reg_read(r)
        pops = {"collision_insert": 4, "collision_remove": 4, "play": 0x14, "emit": 0xC, "destroy": 4}.get(name, 0)
        if name in ("remove",):
            self.log.append((name, reg(UC_X86_REG_ESI)))
        elif name == "insert":
            self.log.append((name, reg(UC_X86_REG_EAX)))
        elif name in ("on_inserted",):
            self.log.append((name, reg(UC_X86_REG_ECX)))
        elif name in ("collision_insert", "collision_remove", "destroy"):
            self.log.append((name, reg(UC_X86_REG_ECX), arg(1)))
        elif name == "stop":
            self.log.append((name, reg(UC_X86_REG_EDI)))
            emu.reg_write(UC_X86_REG_EAX, 0)
        elif name == "release":
            self.log.append((name, reg(UC_X86_REG_ECX), reg(UC_X86_REG_EAX)))
        elif name == "emit":
            floats = struct.unpack("<ff", emu.mem_read(sp + 8, 8))
            self.log.append((name, reg(UC_X86_REG_ECX), floats))         # its out: written whatever it held
            self.word(arg(1), EFFECT)
            self.word(EFFECT + 0x1C, self.read(EFFECT + 0x1C) + 1)
            emu.reg_write(UC_X86_REG_EAX, arg(1))
        else:                             # play: EDI the player, &the effect, three dwords, the listener
            self.log.append((name, reg(UC_X86_REG_EDI), self.read(arg(1)), arg(2), arg(3), arg(4), arg(5)))
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, arg(0))

    def lobby(self):
        """The lobby menu with its project shown (OLD, two static collisions,
        one reference) and a new project at PROJECT (three collisions, its
        track a hud emitter, two references: the results'), all made up."""
        self.emu.mem_write(MENU, bytes(0x3200))
        self.emu.mem_write(OLD - 0x38, bytes(0x400))
        self.emu.mem_write(PROJECT - 0x38, bytes(0x400))
        for resource, collisions, count, references in ((OLD, OLD_COLLISIONS, 2, 1), (PROJECT, NEW_COLLISIONS, 3, 2)):
            project = resource - 0x38
            self.word(project, SIMPLE_GAME_PROJECT)
            self.word(project + 0x28, collisions)
            self.word(project + 0x2C, count)
            self.word(project + 0x108, references)
            self.word(project + 0x1E4, EMITTER if resource == PROJECT else EMITTER + 0x40)
            self.emu.mem_write(collisions, bytes(0x50 * count))
            for i in range(count):
                self.word(collisions + 0x50 * i, STATIC_COLLISION)
        self.word(EMITTER, HUD_EMITTER)
        self.emu.mem_write(EFFECT, bytes(0x40))
        self.word(EFFECT + 0x0C, EMITTER)
        self.word(MENU + 0x114, OLD - 0x38)
        self.word(MENU + 0x118, 0x55)
        self.word(MENU + 0x3104, WORLD)

    def state(self):
        return (self.read(MENU + 0x114), self.read(MENU + 0x118), self.read(OLD - 0x38 + 0x108),
                self.read(PROJECT - 0x38 + 0x108), self.read(EFFECT + 0x1C))

    def stock_swap(self):
        """The original's project halves on the made-up lobby: the calls
        and the state after them."""
        self.log = []
        sp = hide.STACK + 0x6000
        self.word(sp, hide.SENTINEL)
        self.run(CLEAR, (CLEAR_PROJECT_END,), sp, {**hide.GAME, UC_X86_REG_ECX: MENU})
        assert self.stopped == CLEAR_PROJECT_END
        self.word(RESULTS + 0x9CC, PROJECT)
        self.run(READY_PROJECT, (READY_PROJECT_END,), sp, {**hide.GAME, UC_X86_REG_EBX: MENU,
                                                            UC_X86_REG_EDI: RESULTS})
        assert self.stopped == READY_PROJECT_END
        self.run(READY_INSERT, (READY_INSERT_END,), sp, {**hide.GAME, UC_X86_REG_EBX: MENU, UC_X86_REG_ESI: 0})
        assert self.stopped == READY_INSERT_END
        log, self.log = self.log, None
        return log, self.state()

    def deactivate(self):
        """on_deactivate (ECX the lobby menu) to its own code past the hook:
        the lobby menus the query was asked for."""
        sp = hide.STACK + 0x9000
        self.word(sp, hide.SENTINEL)
        self.asked_again, self.stub_query = [], True
        try:
            self.run(DEACTIVATE, (DEACTIVATE_REPLAYED,), sp, {**hide.GAME, UC_X86_REG_ECX: MENU})
        finally:
            self.stub_query = False
        assert self.stopped == DEACTIVATE_REPLAYED, hex(self.stopped or 0)
        return self.asked_again

    def closing(self):
        """game::clear_resources' start (ECX the game: CLIENT, made up) past the hook."""
        sp = hide.STACK + 0x9000
        self.word(sp, hide.SENTINEL)
        self.run(CLOSING, (CLOSING_REPLAYED,), sp, {**hide.GAME, UC_X86_REG_ECX: CLIENT})
        assert self.stopped == CLOSING_REPLAYED
        return self.emu.reg_read(UC_X86_REG_ESI), self.emu.reg_read(UC_X86_REG_ECX)

    def activate(self):
        sp = hide.STACK + 0x9000
        self.word(sp, hide.SENTINEL)
        self.run(ACTIVATE, (ACTIVATE_REPLAYED,), sp, {**hide.GAME, UC_X86_REG_ECX: MENU})
        assert self.stopped == ACTIVATE_REPLAYED

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

    def ready(self, scene, swap=False):
        """on_render_scenes_ready(results) with result 3's resource `scene`
        (0 a failed query): "replayed" (on into the original's code, its
        span replayed) or "returned" (ret 4), and the lobby menus asked again;
        `swap`: the swap's stubs recorded (self.log)."""
        self.log = [] if swap else None
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

    after_a_match(patched, stock, stock_requests, expect)
    print("all cases passed")


def after_a_match(patched, stock, stock_requests, expect):
    """The swap: a lobby shown, the faction changed, a match entered."""
    config = lambda lobby: f"resources/projects/lobby_scene/{lobby}/client_project"

    def shown(value, lobby):
        """A fresh game whose lobby `lobby` was taken at its start (the saved
        faction `value`), the lobby made up."""
        r = Run(patched)
        _, _, head = r.start()
        r.saved = lambda text: r.emu.mem_write(r.read(head + 0x40), text.encode() + b"\0")
        r.saved(value)
        while True:
            requests, _, _ = r.query()
            flow = r.ready(project(r, 1) if requests[3][1] == 0x40 else CONFIG)
            if flow[0] == "replayed":
                break
        assert requests[3][0] == lobby, requests[3]
        r.lobby()
        return r

    stock.lobby()
    stock_log, stock_state = stock.stock_swap()
    expect(stock_log[:2] == [("remove", OLD - 0x38), ("collision_remove", OLD_COLLISIONS, WORLD)]
           and ("release", OLD, OLD - 0x38 + 0x108) in stock_log
           and stock_log[-1] == ("collision_insert", NEW_COLLISIONS + 0xA0, WORLD)
           and stock_state == (PROJECT - 0x38, 0, 0, 3, 0),
           f"the original's project halves on the made-up lobby: {len(stock_log)} calls, the new project at +114h, "
           "+118h 0, the old one's count 0 (released), the new one's 3, the effect's 0 (destroyed)")

    # the same faction: nothing asked
    r = shown("4", "lobby_scene/hangar_046")
    expect(r.deactivate() == [] and r.read(MENU + 0x114) == OLD - 0x38,
           "a match entered with the faction of the lobby shown (Edge, the hangar): nothing asked")

    # a change of faction: the lobby's config, then its project, alone; then the swap
    r = shown("", "lobby_scene/day")
    r.saved("1")
    asked = r.deactivate()
    requests, data, _ = r.query()
    alone = lambda name: requests == [(config("camp_069"), 0x1F)] * 3 + [name] + [(config("camp_069"), 0x1F)] * 4 \
        and data == [0] * 3 + data[3:4] + [0] * 4
    expect(asked == [MENU] and alone((config("camp_069"), 0x1F)) and data[3] == 0,
           "the classic shown, Scavengers saved, a match entered: on_deactivate asks the query and goes on into "
           "its own code; the query asks camp_069's config alone (every other request that config, 1Fh, no "
           "user data)")
    flow = r.ready(CONFIG)
    requests, data, _ = r.query()
    expect(flow == ("returned", [MENU]) and alone(("lobby_scene/camp_069", 0x40)) and data[3] != 0,
           "its config there: the results let go, the query asked again: camp_069's project alone (40h, the "
           "lobby menu's variant kept), the other seven its config")
    flow = r.ready(PROJECT, swap=True)
    expect(flow == ("returned", []) and r.log == stock_log and r.state() == stock_state,
           f"its project's results: put in the old one's place, returned from: the {len(r.log)} calls and the "
           "lobby menu's +114h, +118h, the two counts and the effect's what the original's halves do")
    r.activate()
    expect(r.deactivate() == [], "back in the lobby, the next match: the camp is shown, nothing asked")

    # results that come after the return: let go untouched; the next match asks again
    r = shown("", "lobby_scene/day")
    r.saved("4")
    r.deactivate()
    r.query()
    r.activate()
    before = r.state()
    flow = r.ready(CONFIG, swap=True)
    expect(flow == ("returned", []) and r.log == [] and r.state() == before,
           "Edge saved, a match entered and left before the results: they are let go, nothing touched")
    asked = r.deactivate()
    requests, _, _ = r.query()
    expect(asked == [MENU] and requests[3] == (config("hangar_046"), 0x1F),
           "the next match asks the hangar's config again")

    # the game closing: its on_deactivate of the lobby asks nothing; a swap out is let go
    r = shown("", "lobby_scene/day")
    r.saved("4")
    r.word(CLIENT + 0x535C, 0x13579BDF)
    esi, ecx = r.closing()
    expect(esi == CLIENT and ecx == 0x13579BDF and r.deactivate() == [],
           "game::clear_resources' start (its span replayed: esi the game, ecx [esi+535Ch]) with Edge saved: its "
           "on_deactivate of the lobby asks nothing")
    r = shown("", "lobby_scene/day")
    r.saved("4")
    r.deactivate()
    r.query()
    r.closing()
    before = r.state()
    flow = r.ready(CONFIG, swap=True)
    expect(flow == ("returned", []) and r.log == [] and r.state() == before,
           "a swap's results after the game began closing: let go, nothing touched")

    # a missing pack: the classic's project (here shown already: let go)
    r = shown("4", "lobby_scene/hangar_046")
    r.saved("2")
    r.deactivate()
    r.query()
    flow = r.ready(0)
    requests, _, _ = r.query()
    expect(flow == ("returned", [MENU]) and requests[3] == ("lobby_scene/day", 0x40)
           and requests[0] == ("resources/projects/lobby_scene/day/client_project", 0x1F),
           "the hangar shown, Black Market saved, stk2_blockpost's config missing: the classic's project asked "
           "alone, never stk2_blockpost's")


if __name__ == "__main__":
    main()
