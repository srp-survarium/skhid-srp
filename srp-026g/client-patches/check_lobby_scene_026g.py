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
  not again for the same one; none (0) or an unknown id saved as the
  Scavengers (1);
- lobby_menu::query_resources (0x21E450) from its start to its query
  (0x21E5ED): the fourth request is the original's ({"lobby_scene/day", 40h},
  the lobby menu's variant as its user data) for Renaissance; for another
  faction, no faction saved or an unknown value (the Scavengers) its lobby's config
  first ({"resources/projects/lobby_scene/<id>/client_project", 1Fh}, no user
  data), the other seven requests and their user data the original's;
- lobby_menu::on_render_scenes_ready (0x220B20): after a probe the patch asks
  the query again (query_resources with the lobby menu) and returns (ret 4)
  without the original's code, then asks the lobby's project, or the
  classic's when the config was not there; a project without its camera
  track is asked again as the classic; a usable project, or the classic's,
  goes on into the original's code (its span replayed);
- a change shown at once (lobby-scenes.md, "A change shown at once"), the
  lobby camera's tick (0x2840F2, past the site's chain: patch 45's flight,
  the sweep, ours) with the faction of the lobby shown asks nothing; with
  another faction saved it asks the query (query_resources with the lobby
  menu), its lobby's config then its project alone (every other request that
  lobby's config, class 1Fh, no user data); its project's results are held
  (one reference more), returned from (ret 4), the lobby movie asked for the
  fade to black (root.srp_scene_fade 1, 250 ms; [[menu+690h]+108h]+4); after
  the black's ticks the swap: the calls made (the old project's remove, its
  static collisions' remove with the physics world, the track player's stop,
  the release at a count of 0; the new project's vtable+0Ch, the stop, its
  track's emit through the emitter's vtable, the play with the menu's
  listener, the made effect's destroy at 0, the insert, its collisions'
  insert) and the lobby menu's +114h, +118h and the counts are what the
  original's clear_resources (0x21D080..0x21D100) and on_render_scenes_ready
  (0x220DA2..0x220EC3, 0x22193E..0x221975) do on the same objects; the eye
  adaptation's byte (0xAC914C) set, the camera at its ctor's pose; six
  frames later the fade from black (2, 400 ms) and the byte again; a
  missing pack keeps the lobby shown with no fade and is not asked again;
- 0xA1 (read_squad_info at its id, 0x2747C3, and a squad's revision,
  0x2748BC): a squad's srp byte is its commander's lobby asked, the saved
  value still the player's own; no squad, or a squad without the byte, the
  player's own lobby; a match entered (lobby_menu::on_deactivate 0x21D4A0)
  behind the black puts the held project in at once (the original's halves)
  and the fade gone (0); game::clear_resources (0x1ED030, the game closing)
  during the black gives our reference back and puts nothing in, its
  on_deactivate of the lobby asks nothing, and a change's results after it
  are let go untouched.
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
TICK, TICK_END = 0x2840F2, 0x2840F8                     # lobby_camera.view: the tick's yaw and pitch final
TICKER = MENU + 0x8000                                  # the lobby camera's ticker (camera+4), made up
HOLDER, VIEW, MOVIE = MENU + 0x9000, MENU + 0x9200, MENU + 0x9400   # the lobby movie [[+690h]+108h]+4
SQUAD_ID, SQUAD_ID_END = 0x2747C3, 0x2747CA             # lobby_client.squad_id: the id kept, both paths
SQUAD, SQUAD_END = 0x2748BC, 0x2748C2                   # lobby_client.squad_read: a squad's revision kept
READER, MESSAGE = MENU + 0xA000, MENU + 0xA100          # a 0xA1's reader {begin, cursor, size} and bytes
EYE_ADAPTATION = 0xAC914C                               # stage_postprocess's reset byte
FRAME = 1 / 60
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


    def tick(self, dt=FRAME):
        """A frame of the lobby: lobby_camera::tick at 0x2840F2 (EBX the ticker, the frame's dt at
        [esp+10h]) past the chain (patch 45's flight, the sweep, ours): the lobby menus asked, the
        stubs' record (self.log) and the movie's invokes."""
        sp = hide.STACK + 0x9000
        self.emu.mem_write(sp, bytes(0x40))
        self.emu.mem_write(sp + 0x10, struct.pack("<f", dt))
        self.log, self.asked_again, self.stub_query = [], [], True
        try:
            self.run(TICK, (TICK_END,), sp, {**hide.GAME, UC_X86_REG_EBX: TICKER})
        finally:
            self.stub_query = False
        assert self.stopped == TICK_END, hex(self.stopped or 0)
        log, self.log = self.log, None
        return self.asked_again, log, self.invokes()

    def camera(self):
        """The ticker of the lobby camera at its stock pose turned away, its menu the lobby's; the
        lobby movie under the menu's +690h."""
        self.emu.mem_write(TICKER, bytes(0x100))
        self.word(TICKER + 0x84, MENU)
        self.emu.mem_write(TICKER + 0xB4, struct.pack("<ff", 2.5, -0.5))       # yaw, pitch
        self.emu.mem_write(TICKER + 0xBC, struct.pack("<ff", 4.6, 4.6))        # distance target, distance
        self.emu.mem_write(TICKER + 0xA8, struct.pack("<f", 0.3))              # side
        self.word(MENU + 0x690, HOLDER)
        self.word(HOLDER + 0x108, VIEW)
        self.word(VIEW + 4, MOVIE)

    def pose(self):
        f = lambda at: struct.unpack("<f", self.emu.mem_read(TICKER + at, 4))[0]
        return (f(0xB4), f(0xB8), round(f(0xBC), 4), round(f(0xC0), 4), f(0xA8))

    def squad(self, squad_id, rest):
        """read_squad_info with its squad id kept (0x2747C3, EBX the lobby client) to the je after;
        for a squad, as it keeps the revision (0x2748BC, ECX 77h, ESI the reader past it) to past
        that, with `rest` the message's bytes left there."""
        self.word(CLIENT + 0x1EAAA4, squad_id)
        self.emu.mem_write(MESSAGE, b"\xA1" * 8 + rest)
        self.emu.mem_write(READER, struct.pack("<III", MESSAGE, MESSAGE + 8, 8 + len(rest)))
        sp = hide.STACK + 0x9000
        self.run(SQUAD_ID, (SQUAD_ID_END,), sp, {**hide.GAME, UC_X86_REG_EBX: CLIENT})
        assert self.stopped == SQUAD_ID_END and self.read(CLIENT + 0x1EAAAC) & 0xFF == 0, hex(self.stopped or 0)
        if squad_id == 0:
            return
        self.run(SQUAD, (SQUAD_END,), sp, {**hide.GAME, UC_X86_REG_EBX: CLIENT, UC_X86_REG_ESI: READER,
                                            UC_X86_REG_ECX: 0x77})
        assert self.stopped == SQUAD_END and self.read(CLIENT + 0x1EAAA0) == 0x77, hex(self.stopped or 0)


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
        ("Edge again", 4, b"4", 0),
        ("none chosen (0), the Scavengers", 0, b"1", 1),
        ("none again", 0, b"1", 0),
        ("an unknown id (9), the Scavengers", 9, b"1", 0),
        ("Renaissance (3)", 3, b"3", 1),
    ]:
        kept, after, written = ours.faction(faction)
        stock_kept, stock_after, stock_written = stock.faction(faction)
        expect(kept == faction == stock_kept and after == stock_after and ours.text(buffer) == saved_after
               and written == saves and stock_written == 0,
               f"0x8E with {label}: kept at +1EA998h and the registers as the original's; saved "
               f"{saved_after.decode()}, user.cfg written {saves} time(s)")

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
        ("no faction saved, the Scavengers", "", [(CONFIG, (config("camp_069"), 0x1F), False, "returned"),
                                                  (project, ("lobby_scene/camp_069", 0x40), True, "replayed")]),
        ("Renaissance (3)", "3", [(project, ("lobby_scene/day", 0x40), True, "replayed")]),
        ("an unknown value, the Scavengers", "9", [(CONFIG, (config("camp_069"), 0x1F), False, "returned"),
                                                   (project, ("lobby_scene/camp_069", 0x40), True, "replayed")]),
        ("Edge (4), hangar_046 there", "4", [(CONFIG, (config("hangar_046"), 0x1F), False, "returned"),
                                             (project, ("lobby_scene/hangar_046", 0x40), True, "replayed")]),
        ("Scavengers (1), camp_069 missing", "1", [(0, (config("camp_069"), 0x1F), False, "returned"),
                                                  (project, ("lobby_scene/day", 0x40), True, "replayed")]),
        ("Black Market (2), stk2_bunker without its track", "2", [
            (CONFIG, (config("stk2_bunker"), 0x1F), False, "returned"),
            ("no track", ("lobby_scene/stk2_bunker", 0x40), True, "returned"),
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

    a_change(patched, stock, stock_requests, expect)
    print("all cases passed")


def a_change(patched, stock, stock_requests, expect):
    """A change of lobby shown at once (lobby-scenes.md, "A change shown at once"): a lobby shown, the
    faction or the squad changed, the lobby's ticks."""
    config = lambda lobby: f"resources/projects/lobby_scene/{lobby}/client_project"
    fade = lambda state, ms: ((MOVIE, "root.srp_scene_fade", (state, ms)))

    def shown(value, lobby):
        """A fresh game whose lobby `lobby` was taken at its start (the saved faction `value`), the
        lobby and its camera made up."""
        r = Run(patched)
        _, _, head = r.start()
        r.saved = lambda text: r.emu.mem_write(r.read(head + 0x40), text.encode() + b"\0")
        r.saved_text = lambda: r.text(r.read(head + 0x40))
        r.saved(value)
        while True:
            requests, _, _ = r.query()
            flow = r.ready(project(r, 1) if requests[3][1] == 0x40 else CONFIG)
            if flow[0] == "replayed":
                break
        assert requests[3][0] == lobby, requests[3]
        r.lobby()
        r.camera()
        r.word(EYE_ADAPTATION, 0)
        return r

    def to_project(r, lobby):
        """Its probe and its project, alone: the results of each."""
        requests, data, _ = r.query()
        alone = lambda name: requests == [(config(lobby), 0x1F)] * 3 + [name] + [(config(lobby), 0x1F)] * 4 \
            and data == [0] * 3 + data[3:4] + [0] * 4
        probe = alone((config(lobby), 0x1F)) and data[3] == 0
        flow = r.ready(CONFIG)
        requests, data, _ = r.query()
        asked = alone((f"lobby_scene/{lobby}", 0x40)) and data[3] != 0
        return probe and flow == ("returned", [MENU]) and asked

    stock.lobby()
    stock_log, stock_state = stock.stock_swap()
    expect(stock_log[:2] == [("remove", OLD - 0x38), ("collision_remove", OLD_COLLISIONS, WORLD)]
           and ("release", OLD, OLD - 0x38 + 0x108) in stock_log
           and stock_log[-1] == ("collision_insert", NEW_COLLISIONS + 0xA0, WORLD)
           and stock_state == (PROJECT - 0x38, 0, 0, 3, 0),
           f"the original's project halves on the made-up lobby: {len(stock_log)} calls, the new project at +114h, "
           "+118h 0, the old one's count 0 (released), the new one's 3, the effect's 0 (destroyed)")

    # the same lobby: nothing asked, ever
    r = shown("4", "lobby_scene/hangar_046")
    ticks = [r.tick() for _ in range(5)]
    expect(all(t == ([], [], []) for t in ticks) and r.read(MENU + 0x114) == OLD - 0x38,
           "five ticks with the faction of the lobby shown (Edge, the hangar): nothing asked, no fade")

    # a change: the lobby's config, then its project, alone, the old lobby drawn
    r = shown("3", "lobby_scene/day")
    r.saved("1")
    asked, log, invokes = r.tick()
    expect(asked == [MENU] and log == [] and invokes == [],
           "the classic shown, Scavengers saved, a tick: the lobby's query asked, nothing else")
    expect(to_project(r, "camp_069"),
           "the query asks camp_069's config alone (every other request that config, 1Fh, no user data); its "
           "config there: let go, asked again: its project alone (40h, the lobby menu's variant)")
    expect(r.tick() == ([], [], []), "a tick while its project loads: nothing (the old lobby drawn)")
    before = r.state()
    r.log = []
    flow = r.ready(PROJECT, swap=True)
    held = r.read(PROJECT - 0x38 + 0x108)
    expect(flow == ("returned", []) and r.log == [] and r.read(MENU + 0x114) == OLD - 0x38 and held == before[3] + 1,
           "its project: held (one reference more, nothing put in), returned from (ret 4)")
    # (the fade's invoke is the results' callback's own; read it from the invokes of that run)
    expect(fade(1, 250) in r.invokes(), "the fade to black asked of the lobby movie with it (srp_scene_fade 1, 250 ms)")
    out = []
    while r.read(MENU + 0x114) == OLD - 0x38 and len(out) < 40:
        out.append(r.tick())
    swap = out[-1]
    expect(14 <= len(out) <= 16 and all(t == ([], [], []) for t in out[:-1]),
           f"the black's ticks (0.25 s at 60 Hz): nothing until the swap, at tick {len(out)}")
    expect(swap[1] == stock_log and r.state() == stock_state and r.read(EYE_ADAPTATION) & 0xFF == 1
           and r.pose() == (0.0, 0.0, 1.96, 1.96, 0.0),
           f"the swap: the {len(swap[1])} calls and the lobby menu's +114h, +118h, the counts (ours given back) and "
           "the effect's what the original's halves do; the eye adaptation reset (0xAC914C 1); the camera at the "
           "ctor's pose (yaw, pitch, side 0, distance 1.96)")
    r.word(EYE_ADAPTATION, 0)
    held_ticks = []
    while len(held_ticks) < 20:
        held_ticks.append(r.tick())
        if held_ticks[-1][2]:
            break
    expect(held_ticks[-1][2] == [fade(2, 400)] and len(held_ticks) == 6 and r.read(EYE_ADAPTATION) & 0xFF == 1
           and all(t == ([], [], []) for t in held_ticks[:-1]),
           f"six frames behind the black, then the fade from black (srp_scene_fade 2, 400 ms) and the eye "
           f"adaptation reset again ({len(held_ticks)} ticks)")
    after = [r.tick() for _ in range(40)]
    expect(all(t == ([], [], []) for t in after), "then nothing: the camp shown, settled")

    # a missing pack: the lobby shown kept, no fade, not asked again
    r = shown("3", "lobby_scene/day")
    r.saved("2")
    r.tick()
    r.query()
    flow = r.ready(0)
    fades = r.invokes()
    later = [r.tick() for _ in range(30)]
    expect(flow == ("returned", []) and fades == [] and all(t == ([], [], []) for t in later)
           and r.read(MENU + 0x114) == OLD - 0x38,
           "Black Market saved, stk2_bunker's config missing: let go, no fade, the classic kept and not asked again")

    # a squad: its commander's lobby; out of it the player's own; the saved value the player's own
    r = shown("3", "lobby_scene/day")
    r.squad(7, b"\x04")
    asked, _, _ = r.tick()
    requests, _, _ = r.query()
    expect(asked == [MENU] and requests[3] == (config("hangar_046"), 0x1F) and r.saved_text() == b"3",
           "0xA1 of squad 7 with srp's byte 4: the commander's lobby (the hangar's config asked); "
           "srp_lobby_faction still the player's own (3)")
    r.ready(0)
    r.squad(0, b"")
    expect(r.tick() == ([], [], []), "0xA1 of no squad: the player's own lobby (the classic shown), nothing asked")
    r.squad(7, b"")
    expect(r.tick() == ([], [], []), "0xA1 of a squad without srp's byte (an older srp): the player's own lobby")
    r.squad(9, b"\x01")
    asked, _, _ = r.tick()
    expect(asked == [MENU], "a commander of the Scavengers: the camp asked")

    # a match entered during the black: the project put in at once, the fade gone
    r = shown("3", "lobby_scene/day")
    r.saved("1")
    r.tick()
    to_project(r, "camp_069")
    r.ready(PROJECT)
    r.tick()
    r.log = []
    asked = r.deactivate()
    log, r.log = r.log, None
    expect(asked == [] and log == stock_log and r.state() == stock_state and fade(0, 0) in r.invokes(),
           "a match entered behind the black: the held project put in at once (the original's halves), the "
           "fade gone (srp_scene_fade 0)")
    r.activate()
    expect(r.tick() == ([], [], []), "back in the lobby: the camp shown, nothing asked")

    # the game closing during the black: the held project let go, nothing put in
    r = shown("3", "lobby_scene/day")
    r.saved("1")
    r.tick()
    to_project(r, "camp_069")
    r.ready(PROJECT)
    held = r.read(PROJECT - 0x38 + 0x108)
    r.word(CLIENT + 0x535C, 0x13579BDF)
    esi, ecx = r.closing()
    expect(esi == CLIENT and ecx == 0x13579BDF and r.read(PROJECT - 0x38 + 0x108) == held - 1
           and r.read(MENU + 0x114) == OLD - 0x38 and r.deactivate() == [],
           "game::clear_resources' start (its span replayed) during the black: our reference given back, "
           "nothing put in; its on_deactivate of the lobby asks nothing")

    # results after the close: let go untouched
    r = shown("3", "lobby_scene/day")
    r.saved("4")
    r.tick()
    r.query()
    r.closing()
    before = r.state()
    flow = r.ready(CONFIG, swap=True)
    expect(flow == ("returned", []) and r.log == [] and r.state() == before,
           "a change's results after the game began closing: let go, nothing touched")


if __name__ == "__main__":
    main()
