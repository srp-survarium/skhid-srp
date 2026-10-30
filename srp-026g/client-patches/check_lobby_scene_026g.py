#!/usr/bin/env python3
"""Run patch 81 (rust/patches/src/lobby_scene.rs, its lohooks of
lobby_scene.toml: the lobby's scene follows the player's quest faction;
docs/notes/lobby-scenes.md) in the default build on made-up game objects,
each from the game's own code before its site to past it, the original
beside it.

    python check_lobby_scene_026g.py <original survarium.exe>

Requires Python `unicorn`. check_language_026g.py's harness (patch 79 shares
configs.start with it, a chain).

- on_configs_loaded's start: `srp_lobby`, a saved string of 4 bytes,
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
PLAYERS_RANGE = 0x270070                      # lobby_client's 3B sender (EAX the client, start, count; ret 8)
PICK_BASE = 2_095_000_000
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
        self.sent = []                    # the 3B requests the exe sent (the client, start, count)
        self.emu.hook_add(UC_HOOK_CODE, self.players_range, begin=PLAYERS_RANGE, end=PLAYERS_RANGE)
        for at in SWAP_STUBS:
            self.emu.hook_add(UC_HOOK_CODE, self.swap_stub, begin=at, end=at)

    def players_range(self, emu, address, size, _):
        """lobby_client's 3B (EAX the client; ret 8): recorded, not sent."""
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.sent.append((emu.reg_read(UC_X86_REG_EAX), self.read(sp + 4), self.read(sp + 8) & 0xFF))
        emu.reg_write(UC_X86_REG_ESP, sp + 12)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

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
        """read_squad_info with its squad id kept (0x2747C3, EBX the lobby client, ESI the reader past
        the id) to the je after;
        for a squad, as it keeps the revision (0x2748BC, ECX 77h, ESI the reader past it) to past
        that, with `rest` the message's bytes left there."""
        self.word(CLIENT + 0x1EAAA4, squad_id)
        self.emu.mem_write(MESSAGE, b"\xA1" * 8 + rest)
        self.emu.mem_write(READER, struct.pack("<III", MESSAGE, MESSAGE + 8, 8 + len(rest)))
        sp = hide.STACK + 0x9000
        self.run(SQUAD_ID, (SQUAD_ID_END,), sp, {**hide.GAME, UC_X86_REG_EBX: CLIENT, UC_X86_REG_ESI: READER})
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
    expect(variable[0] == "srp_lobby" and variable[2] == 4 and variable[3] == 1
           and localization[0] == "localization" and ours.read(ours.read(head + 0x0C) + 0x0C) == head_before,
           "configs.start: `srp_lobby`, a saved string of 4 bytes, at the head of the console's list, "
           "after patch 79's `localization` (the site's chain)")
    _, _, again = ours.start()
    expect(again == head, "configs.start again: registered once")
    buffer = variable[1]
    ours.emu.mem_write(language.TEXT, b"2\0")
    ours.call_at(language.EXECUTE, (language.TEXT,), {UC_X86_REG_ECX: head})
    expect(ours.text(buffer) == b"2", "user.cfg's line `srp_lobby 2` sets it "
           "(cc_string::execute 0xEC8C0)")

    # a whole user.cfg through the console's load, a stand-in r_resolution beside ours
    ours.emu.mem_write(STAND_IN, bytes(0x200))
    ours.emu.mem_write(STAND_IN + 0x100, b"r_resolution\0")
    ours.call_at(CC_STRING, (STAND_IN + 0x100, STAND_IN + 0x80, 32, 1), {UC_X86_REG_EAX: STAND_IN})
    for label, text in [
        ("run.py's (LF, r_resolution last)", b"localization english\nsrp_lobby 4\nr_resolution 1280x720\n"),
        ("the game's (CRLF)", b"localization czech\r\nr_resolution 1280x720\r\nsrp_lobby 1\r\n"),
        ("no end of line", b"srp_lobby 3\nr_resolution 1280x720"),
    ]:
        ours.emu.mem_write(CFG, text + b"\0" * 16)
        ours.emu.mem_write(CFG + 0x1000, struct.pack("<III", CFG, CFG, len(text)))
        ours.emu.mem_write(buffer, b"\0" * 4)
        ours.emu.mem_write(STAND_IN + 0x80, bytes(32))
        _, popped = ours.call_at(CONSOLE_LOAD, (1, 2), {UC_X86_REG_ESI: CFG + 0x1000})
        faction = text.split(b"srp_lobby ")[1][:1]
        expect(ours.text(buffer) == faction and ours.text(STAND_IN + 0x80) == b"1280x720" and popped == 4,
               f"user.cfg {label}: `srp_lobby {faction.decode()}` and `r_resolution 1280x720` each to its "
               "variable through the console's own load; the stack as it was")

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
        ("no pick saved, the Room", "", [(CONFIG, (config("camp_069"), 0x1F), False, "returned"),
                                         (project, ("lobby_scene/camp_069", 0x40), True, "replayed")]),
        ("the Basement (2)", "2", [(project, ("lobby_scene/day", 0x40), True, "replayed")]),
        ("an unknown value, the Room", "9", [(CONFIG, (config("camp_069"), 0x1F), False, "returned"),
                                             (project, ("lobby_scene/camp_069", 0x40), True, "replayed")]),
        ("the Hangar (4), hangar_046 there", "4", [(CONFIG, (config("hangar_046"), 0x1F), False, "returned"),
                                                   (project, ("lobby_scene/hangar_046", 0x40), True, "replayed")]),
        ("the Room (1), camp_069 missing", "1", [(0, (config("camp_069"), 0x1F), False, "returned"),
                                                 (project, ("lobby_scene/day", 0x40), True, "replayed")]),
        ("the Bunker (3), stk2_bunker without its track", "3", [
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
    """A pick shown at once (lobby-scenes.md, "A change shown at once"): a lobby shown, the first
    frame's census, a pick or a squad changed, the lobby's ticks."""
    config = lambda lobby: f"resources/projects/lobby_scene/{lobby}/client_project"
    every = [config(lobby) for lobby in ("camp_069", "day", "stk2_bunker", "hangar_046")]
    fades = lambda invokes: [values for movie, name, values in invokes if name == "root.srp_scene_fade" and movie == MOVIE]
    states = lambda invokes: [values for movie, name, values in invokes if name == "root.srp_lobby_state" and movie == MOVIE]

    def shown(value, lobby, there=(1, 1, 1, 1)):
        """A fresh game whose lobby `lobby` was taken at its start (the saved pick `value`), the
        lobby and its camera made up, the first frame's census answered (`there`: each lobby's
        config, in id order)."""
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
        asked, _, invokes = r.tick()
        requests, data, _ = r.query()
        assert asked == [MENU] and [name for name, _ in requests] == every + [config("day")] * 4 \
            and all(c == 0x1F for _, c in requests) and data == [0] * 8, (asked, requests)
        r.census = invokes
        r.emu.mem_write(RESULTS, bytes(0x2E0 * 8 + 0x200))
        for i, found in enumerate(there):
            r.word(RESULTS + 0x12C + i * 0x2E0, CONFIG if found else 0)
        sp = hide.STACK + 0x9000
        r.word(sp, hide.SENTINEL)
        r.word(sp + 4, RESULTS)
        r.asked_again, r.stub_query = [], True
        try:
            r.run(READY, (READY_REPLAYED, hide.SENTINEL), sp, {**hide.GAME, UC_X86_REG_ECX: MENU})
        finally:
            r.stub_query = False
        assert r.stopped == hide.SENTINEL and r.asked_again == []
        r.word(EYE_ADAPTATION, 0)
        return r

    def to_black(r):
        """Ticks until the black is whole: their fades."""
        out = []
        while len(out) < 40:
            asked, log, invokes = r.tick()
            out.append((asked, log, fades(invokes)))
            if asked:
                break
        return out

    stock.lobby()
    stock_log, stock_state = stock.stock_swap()
    expect(stock_log[:2] == [("remove", OLD - 0x38), ("collision_remove", OLD_COLLISIONS, WORLD)]
           and ("release", OLD, OLD - 0x38 + 0x108) in stock_log
           and stock_log[-1] == ("collision_insert", NEW_COLLISIONS + 0xA0, WORLD)
           and stock_state == (PROJECT - 0x38, 0, 0, 3, 0),
           f"the original's project halves on the made-up lobby: {len(stock_log)} calls, the new project at +114h, "
           "+118h 0, the old one's count 0 (released), the new one's 3, the effect's 0 (destroyed)")

    # the first frame: the census, every lobby's config, then the window told
    r = shown("2", "lobby_scene/day")
    expect(True, "the first frame asks the census: the four lobbies' configs (the Room's, the Basement's, the "
           "Bunker's, the Hangar's) then the classic's, every one a binary config (1Fh) with no user data; its "
           "results let go (ret 4)")
    expect(states(r.census) == [(2, 2, 0b1111)] and fades(r.census) == [],
           "that frame tells the lobby window its state: the pick 2, the lobby shown 2, every lobby there (1111b)")
    ticks = [r.tick() for _ in range(5)]
    expect(all(t == ([], [], []) for t in ticks) and r.read(MENU + 0x114) == OLD - 0x38,
           "five more ticks with the pick shown: nothing asked, no fade, the window not told again")

    # a pick: its config alone while the old lobby is drawn, then the black, then its project
    r.saved("3")
    asked, log, invokes = r.tick()
    expect(asked == [MENU] and log == [] and fades(invokes) == [] and states(invokes) == [(3, 2, 0b1111)],
           "the Bunker picked (3), a tick: the lobby's query asked, no fade; the window told the pick")
    requests, data, _ = r.query()
    alone = lambda name: requests == [(config("stk2_bunker"), 0x1F)] * 3 + [name] + [(config("stk2_bunker"), 0x1F)] * 4 \
        and data == [0] * 3 + data[3:4] + [0] * 4
    expect(alone((config("stk2_bunker"), 0x1F)) and data[3] == 0,
           "the query asks stk2_bunker's config alone (every other request that config, 1Fh, no user data)")
    flow = r.ready(CONFIG)
    expect(flow == ("returned", []) and fades(r.invokes()) == [],
           "its config there: the results let go, no query yet, no fade from the callback")
    out = to_black(r)
    steps = [f for _, _, f in out]
    expect(14 <= len(out) <= 16 and all(len(f) == 1 and 0 < f[0][0] < 1000 and f[0][1] == 0 for f in steps[:-1])
           and [f[0][0] for f in steps[:-1]] == sorted(f[0][0] for f in steps[:-1])
           and steps[-1] == [(1000, 1)] and out[-1][0] == [MENU],
           f"the fade to black, a step a frame ({len(out)} frames: srp_scene_fade at "
           f"{', '.join(str(f[0][0]) for f in steps[:4])}... per mille), then whole with the loading spinner "
           "(1000, 1) and the lobby's query asked")
    requests, data, _ = r.query()
    expect(alone(("lobby_scene/stk2_bunker", 0x40)) and data[3] != 0,
           "behind the black the query asks stk2_bunker's project alone (40h, the lobby menu's variant)")
    expect(r.tick() == ([], [], []), "a tick while its project loads: nothing (black)")
    r.log = []
    flow = r.ready(PROJECT, swap=True)
    expect(flow == ("returned", []) and r.log == stock_log and r.state() == stock_state
           and r.read(EYE_ADAPTATION) & 0xFF == 1,
           f"its project: put in the old one's place in the results' callback, returned from (ret 4): the "
           f"{len(r.log)} calls and the lobby menu's +114h, +118h, the counts and the effect's what the "
           "original's halves do; the eye adaptation reset (0xAC914C 1)")
    r.word(EYE_ADAPTATION, 0)
    _, _, invokes = r.tick()
    expect(r.pose() == (0.0, 0.0, 1.96, 1.96, 0.0) and states(invokes) == [(3, 3, 0b1111)],
           "the next frame: the camera at the ctor's pose (yaw, pitch, side 0, distance 1.96); the window told "
           "the Bunker shown")
    held = [r.tick() for _ in range(5)]
    expect(fades(held[-1][2]) == [(1000, 0)] and r.read(EYE_ADAPTATION) & 0xFF == 1
           and all(t == ([], [], []) for t in held[:-1]),
           "six frames behind the black, then the eye adaptation reset again and the spinner off (1000, 0)")
    out = []
    while len(out) < 40:
        f = fades(r.tick()[2])
        out.append(f)
        if f == [(0, 0)]:
            break
    expect(22 <= len(out) <= 26 and out[-1] == [(0, 0)] and all(0 < f[0][0] < 1000 for f in out[:-1]),
           f"the fade from black, a step a frame ({len(out)} frames), then gone (0, 0)")
    expect(all(r.tick() == ([], [], []) for _ in range(20)), "then nothing: the Bunker shown, settled")

    # a stall: the fade still steps by a 30th of a second
    r = shown("2", "lobby_scene/day")
    r.tick()
    r.saved("4")
    r.tick()
    r.query()
    r.ready(CONFIG)
    f = fades(r.tick(3.0)[2])
    expect(len(f) == 1 and 100 <= f[0][0] <= 140,
           f"a 3 s frame (a stall) after the Hangar's config: the black at {f[0][0] if f else None} per mille "
           "(a 30th of a second's step), not whole")

    # a pack the census found missing: its card dimmed, never asked
    r = shown("2", "lobby_scene/day", there=(1, 1, 0, 1))
    _, _, invokes = r.tick()
    invokes = invokes or []
    r.saved("3")
    later = [r.tick() for _ in range(30)]
    expect(states(invokes) == [(2, 2, 0b1011)] and all(t[0] == [] and fades(t[2]) == [] for t in later),
           "the census without stk2_bunker's config: the window told 1011b; the Bunker picked: nothing asked, "
           "no fade")

    # 0xA1: the picks; srp has none: the saved pick sent; a squad's commander's pick shown
    r = shown("2", "lobby_scene/day")
    r.tick()
    r.squad(0, b"\x00\x00")
    expect(r.sent == [(CLIENT, PICK_BASE + 2, 1)] and r.saved_text() == b"2",
           "0xA1 of no squad, srp's picks 0 0 (the sign-in): the saved pick sent (3B at 2 095 000 002, count 1, "
           "the handler's lobby client)")
    r.sent = []
    r.squad(0, b"\x04\x00")
    asked, _, invokes = r.tick()
    expect(r.sent == [] and r.saved_text() == b"4" and asked == [MENU] and states(invokes) == [(4, 2, 0b1111)],
           "0xA1 of no squad with the own pick 4 (a click's, srp's answer): saved (srp_lobby 4), the Hangar "
           "asked, the window told")
    r.ready(0)
    r.squad(7, b"\x02\x01")
    asked, _, _ = r.tick()
    requests, _, _ = r.query()
    expect(asked == [MENU] and requests[3] == (config("camp_069"), 0x1F) and r.saved_text() == b"2",
           "0xA1 of squad 7, picks 2 1: the commander's pick (the Room, 1) asked; the own pick 2 saved")
    r.ready(0)
    r.sent = []
    r.squad(7, b"")
    asked, _, _ = r.tick()
    expect(asked == [] and r.sent == [(CLIENT, PICK_BASE + 2, 1)],
           "a squad without srp's bytes (an older srp): the player's own lobby (shown: nothing asked), the saved "
           "pick sent")

    # a match entered during the fade: the black gone at once; behind the black, a project put in at once
    r = shown("2", "lobby_scene/day")
    r.tick()
    r.saved("1")
    r.tick()
    r.query()
    r.ready(CONFIG)
    r.tick()
    asked = r.deactivate()
    expect(asked == [] and fades(r.invokes()) == [(0, 0)],
           "a match entered during the fade to black: the black gone (srp_scene_fade 0)")
    r.activate()
    r = shown("2", "lobby_scene/day")
    r.tick()
    r.saved("1")
    r.tick()
    r.query()
    r.ready(CONFIG)
    to_black(r)
    r.query()
    r.deactivate()
    r.log = []
    flow = r.ready(PROJECT, swap=True)
    expect(flow == ("returned", []) and r.log == stock_log and r.state() == stock_state
           and fades(r.invokes()) == [(0, 0)],
           "a match entered behind the black, the project's results after: put in at once (the original's "
           "halves), the black gone")

    # the game closing: results let go untouched
    r = shown("2", "lobby_scene/day")
    r.tick()
    r.saved("4")
    r.tick()
    r.query()
    r.word(CLIENT + 0x535C, 0x13579BDF)
    esi, ecx = r.closing()
    before = r.state()
    flow = r.ready(CONFIG, swap=True)
    expect(esi == CLIENT and ecx == 0x13579BDF and flow == ("returned", []) and r.log == [] and r.state() == before
           and r.deactivate() == [],
           "game::clear_resources' start (its span replayed), then a change's results: let go, nothing touched; "
           "its on_deactivate of the lobby asks nothing")


if __name__ == "__main__":
    main()
