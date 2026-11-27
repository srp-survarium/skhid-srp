#!/usr/bin/env python3
"""Run patch 81 (rust/patches/src/lobby_scene.rs, its lohooks of
lobby_scene.toml: the lobby's scene is the player's pick, the squad commander's in a squad;
docs/notes/lobby-scenes.md) in the default build on made-up game objects,
each from the game's own code before its site to past it, the original
beside it.

    python check_lobby_scene_026g.py <original survarium.exe>

Requires Python `unicorn`. check_language_026g.py's harness (patch 79 shares
configs.start with it, a chain).

- on_configs_loaded's start: `srp_lobby`, a saved string of 4 bytes,
  registered once with the game's own cc_string::cc_string, after patch 79's
  `localization` and patch 83's settings; the game's cc_string::execute sets it from a user.cfg line;
- a whole user.cfg through the console's own load (console_commands::load
  0xEBF00, as load_cc_script gives it user.cfg: 1, 2), run.py's file (LF,
  `r_resolution 1280x720` last) and the game's (CRLF) with the pick's
  line: every line to its variable (a stand-in `r_resolution` registered
  with the game's own cc_string::cc_string), the stack as it was;
- lobby_menu::query_resources (0x21E450) from its start to its query
  (0x21E5ED): the fourth request is the saved pick's project ({"lobby_scene/<id>",
  40h}, the lobby menu's variant as its user data; no value or an unknown one
  the Room's), the other seven requests and their user data the original's;
  its results go on into the original's code (its span replayed), a project
  without its track too (logged);
- a change shown at once (lobby-scenes.md, "A change shown at once"): the
  lobby camera's tick (0x2840F2, past the site's chain: patch 45's flight,
  the sweep, ours) tells the lobby window its state (root.srp_lobby_state)
  and, with the pick shown, asks nothing; with another pick it steps the black
  in a frame each (root.srp_scene_fade(per mille, loading) on the lobby movie
  [[menu+690h]+108h]+4), then whole with the spinner asks the query
  (query_resources with the lobby menu) with that lobby's project alone
  (every other request its config, class 1Fh, no user data); its project's
  results are put in the old project's place, returned from (ret 4): the
  calls made (the old project's remove, its static collisions' remove with
  the physics world, the track player's stop, the release at a count of 0;
  the new project's vtable+0Ch, the stop, its track's emit through the
  emitter's vtable, the play with the menu's listener, the made effect's
  destroy at 0, the insert, its collisions' insert) and the lobby menu's
  +114h, +118h and the counts are what the original's clear_resources
  (0x21D080..0x21D100) and on_render_scenes_ready (0x220DA2..0x220EC3,
  0x22193E..0x221975) do on the same objects; the eye adaptation's byte
  (0xAC914C) set, the camera at its ctor's pose; six frames later the
  spinner off and the byte again, the black stepped out; a stall's frame
  steps the black by a 30th of a second; a project without its track behind
  the black: nothing put in, the black stepped out, not asked again;
- while patch 45's camera is the table's (a weapon's upgrade window: its
  state's TABLE 1, or its flight there or back), the lobby shown is kept: a
  pick then asks nothing and steps no black, a fade begun steps back out
  without a query, and a project that comes behind the black is let go (no
  swap: the spot patch 45 read from the lobby shown's project stays its
  lobby's); once the window is closed and the camera home (the flight's word
  0 again, the ray a stub), the pick's change plays and the new lobby's first
  frame puts the camera at its ctor's pose;
- the Room's weather (room-weather.md): `srp_room_rain`, a saved string of
  4 bytes, registered beside `srp_lobby`; each entry (the start's query, the
  return from a match: on_activate after on_deactivate) reads the local time
  (kernel32's GetLocalTime, its import slot, stubbed: the case's hour) and,
  out of a spell, rolls the dice seeded by QueryPerformanceCounter (stubbed:
  a counter whose first roll is dry, or wet); the Room asked is
  lobby_scene/camp_069 by day, camp_069_night from 21:00 to 06:00,
  camp_069_rain in a spell or a rain rolled (srp_room_rain then the entries
  left, user.cfg saved); another lobby is asked as it is; a return into
  another weather steps the black whole with the spinner at once and asks
  the new Room alone at the next frame, and a return into the same asks
  nothing;
- 0xA1 (read_squad_info at its id, 0x2747C3, and a squad's revision,
  0x2748BC): srp's two bytes after the squad (the receiver's pick and its
  commander's, lobby ids): picks 0 (the sign-in) send the saved pick (3B
  through the lobby client's 0x270070); the own pick saved; a squad's
  commander's pick asked; a match entered (lobby_menu::on_deactivate 0x21D4A0) during the fade
  or behind the black: the black gone, a project that comes put in at once;
  back (on_activate 0x21D2E0) while it loads: the black again;
  game::clear_resources (0x1ED030, the game closing): its on_deactivate of
  the lobby asks nothing, and a change's results after it are let go
  untouched.
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
CREATE_FILE_SLOT = 0x870048                   # kernel32's CreateFileA import (engine_create_file_a)
QPC_SLOT, LOCAL_TIME_SLOT = 0x870158, 0x870184  # kernel32's QueryPerformanceCounter and GetLocalTime imports
CC_STRING, CONSOLE_LOAD = 0xEC860, 0xEBF00
STAND_IN, CFG = 0x30400000, 0x30410000       # a stand-in r_resolution; a user.cfg's bytes
LOG_STUB = STAND_IN + 0x18000                 # the stand-in CreateFileA (fails)
QPC_STUB, LOCAL_TIME_STUB = STAND_IN + 0x18100, STAND_IN + 0x18200   # their stand-ins (`ret 4`)
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
RAY = 0x2844D0                                          # the lobby camera's collision ray (`ret 4`): a stub
# patch 45's state (weapon_table_026g.asm's STATE, .srpd) and its fields
TABLE_STATE = base.late_join_state_va(open(base.late_join_bin(), "rb").read()) + base.WEAPON_TABLE_STATE_OFFSET
TABLE_ON, TABLE_FLIGHT, TABLE_TARGETS, TABLE_REQUEST, TABLE_ZOOM = 0x00, 0x04, 0x08, 0x2C, 0x4C
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
        # srp_patches.log's open (kernel32's CreateFileA, its import slot): fails (no filesystem
        # here), each one counted: a line the patch logged
        self.logged = 0
        self.word(CREATE_FILE_SLOT, LOG_STUB)
        self.emu.mem_write(LOG_STUB, bytes.fromhex('b8ffffffffc21c00'))
        self.emu.hook_add(UC_HOOK_CODE, self.log_open, begin=LOG_STUB, end=LOG_STUB)
        self.emu.hook_add(UC_HOOK_CODE, self.players_range, begin=PLAYERS_RANGE, end=PLAYERS_RANGE)
        for at in SWAP_STUBS:
            self.emu.hook_add(UC_HOOK_CODE, self.swap_stub, begin=at, end=at)
        self.emu.hook_add(UC_HOOK_CODE, self.ray_stub, begin=RAY, end=RAY)
        # the entries' clock: noon, a counter whose first roll is dry
        self.hour, self.counter, self.local_times = 12, DRY_COUNTER, 0
        for slot, stub, hook in ((QPC_SLOT, QPC_STUB, self.qpc), (LOCAL_TIME_SLOT, LOCAL_TIME_STUB, self.local_time)):
            self.word(slot, stub)
            self.emu.mem_write(stub, b"\xc2\x04\x00")
            self.emu.hook_add(UC_HOOK_CODE, hook, begin=stub, end=stub)

    def qpc(self, emu, address, size, _):
        """QueryPerformanceCounter(out), stdcall: the case's counter."""
        out = self.read(emu.reg_read(UC_X86_REG_ESP) + 4)
        emu.mem_write(out, struct.pack("<Q", self.counter))
        emu.reg_write(UC_X86_REG_EAX, 1)

    def local_time(self, emu, address, size, _):
        """GetLocalTime(out), stdcall: a SYSTEMTIME of 2026-11-02 at the case's hour."""
        self.local_times += 1
        out = self.read(emu.reg_read(UC_X86_REG_ESP) + 4)
        emu.mem_write(out, struct.pack("<8H", 2026, 11, 1, 2, self.hour, 30, 0, 0))

    def ray_stub(self, emu, address, size, _):
        """The camera's collision ray (patch 45's flight home casts it; `ret 4`): nothing in the way."""
        sp = emu.reg_read(UC_X86_REG_ESP)
        emu.reg_write(UC_X86_REG_ESP, sp + 8)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def table(self, on):
        """Patch 45's camera entry as a weapon's upgrade window leaves it (weapon_table_026g.asm
        `camera`): the table on or off, a request counted, its targets (on: the classic's spot and
        view; off: the origin and the yaw and pitch the camera had)."""
        targets = (4.25, 3.05, -1.5707964, -0.2) if on else (0.0, 0.0, 2.5, -0.5)
        self.word(TABLE_STATE + TABLE_ON, on)
        self.emu.mem_write(TABLE_STATE + TABLE_TARGETS, struct.pack("<4f", *targets))
        self.emu.mem_write(TABLE_STATE + TABLE_ZOOM, struct.pack("<f", 1.0))
        self.word(TABLE_STATE + TABLE_REQUEST, self.read(TABLE_STATE + TABLE_REQUEST) + 1)

    def lobby_variable(self):
        """on_configs_loaded's start run: `srp_lobby`'s node, found by its name (a later handler of
        the chain, patch 88's, registers after it)."""
        before = self.read(language.LIST_HEAD)
        _, _, head = self.start()
        nodes = language.registered(self, head, before)
        rain = language.named(self, nodes, "srp_room_rain")
        self.rain = lambda text=None: self.text(self.read(rain + 0x40)) if text is None \
            else self.emu.mem_write(self.read(rain + 0x40), text.encode() + b"\0")
        return language.named(self, nodes, "srp_lobby")

    def log_open(self, emu, address, size, _):
        self.logged += 1

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


def rolls(counter, n=2):
    """lobby_scene.rs's Dice: xorshift64 seeded by the counter (never 0), stirred 8 steps; its first
    `n` rolls (0..99), each its state's high half."""
    x, mask, out = max(counter, 1), (1 << 64) - 1, []
    for step in range(8 + n):
        x ^= (x << 13) & mask
        x ^= x >> 7
        x ^= (x << 17) & mask
        if step >= 8:
            out.append(((x >> 32) * 100) >> 32)
    return out


# counters of an hour up (10 MHz): two dry rolls first; a wet one
DRY_COUNTER = next(c for c in range(36_000_000_000, 36_000_100_000) if min(rolls(c)) >= 50)
WET_COUNTER = next(c for c in range(36_000_000_000, 36_000_100_000) if rolls(c)[0] < 15)


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

    # on_configs_loaded's start: the variable, once, after patch 79's and patch 83's, before patch 88's
    head_before = ours.read(language.LIST_HEAD)
    _, _, head = ours.start()
    nodes = language.registered(ours, head, head_before)
    names = [ours.text(ours.read(node + 0x10)).decode() for node in nodes]
    node = language.named(ours, nodes, "srp_lobby")
    variable = language.console_variable(ours, node)
    expect(variable[0] == "srp_lobby" and variable[2] == 4 and variable[3] == 1 and names.count("srp_lobby") == 1
           and names[-1] == "localization" and names[0] == "s_music_volume"
           and names.index("s_music_volume") < names.index("srp_lobby") < names.index("localization"),
           f"configs.start: `srp_lobby`, a saved string of 4 bytes, in the console's list once, after patch 79's "
           f"`localization` and patch 83's, before patch 88's `s_music_volume` (newest first: {', '.join(names)})")
    _, _, again = ours.start()
    expect(again == head, "configs.start again: registered once")
    rain = language.console_variable(ours, language.named(ours, nodes, "srp_room_rain"))
    expect(rain[0] == "srp_room_rain" and rain[2] == 4 and rain[3] == 1 and names.count("srp_room_rain") == 1
           and "srp_room_weather" not in names,
           "configs.start: `srp_room_rain`, a saved string of 4 bytes, once; the diagnostics' `srp_room_weather` "
           "not in the default build")
    buffer = variable[1]
    ours.emu.mem_write(language.TEXT, b"2\0")
    ours.call_at(language.EXECUTE, (language.TEXT,), {UC_X86_REG_ECX: node})
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
        pick = text.split(b"srp_lobby ")[1][:1]
        expect(ours.text(buffer) == pick and ours.text(STAND_IN + 0x80) == b"1280x720" and popped == 4,
               f"user.cfg {label}: `srp_lobby {pick.decode()}` and `r_resolution 1280x720` each to its "
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
        ("no pick saved, the Room", "", [(project, ("lobby_scene/camp_069", 0x40), True, "replayed")]),
        ("the Basement (2)", "2", [(project, ("lobby_scene/day", 0x40), True, "replayed")]),
        ("an unknown value, the Room", "9", [(project, ("lobby_scene/camp_069", 0x40), True, "replayed")]),
        ("the Hangar (4)", "4", [(project, ("lobby_scene/hangar_046", 0x40), True, "replayed")]),
        ("the Bunker (3)", "3", [(project, ("lobby_scene/stk2_bunker", 0x40), True, "replayed")]),
        ("the Bunker (3) without its track: logged, the lobby menu takes it as it is", "3",
         [("no track", ("lobby_scene/stk2_bunker", 0x40), True, "replayed")]),
    ]
    for label, value, steps in cases:
        fresh = Run(patched)
        fresh_node = fresh.lobby_variable()
        fresh.emu.mem_write(fresh.read(fresh_node + 0x40), value.encode() + b"\0")
        for i, (result, asked, keeps_data, flow) in enumerate(steps):
            requests, data, _ = fresh.query()
            others_same = requests[:3] + requests[4:] == stock_requests[:3] + stock_requests[4:] \
                and data[:3] + data[4:] == stock_data[:3] + stock_data[4:]
            scene = project(fresh, 1) if result is project else project(fresh, 0) if result == "no track" else result
            got = fresh.ready(scene)
            again = [MENU] if flow == "returned" else []
            logs = fresh.logged
            expect(requests[3] == asked and (data[3] != 0) == keeps_data and others_same and got == (flow, again)
                   and (logs == 1) == (result == "no track"),
                   f"{label}, query {i + 1}: asks {asked[0]} (class {asked[1]:#x}, user data "
                   f"{'kept' if keeps_data else 'none'}), the other requests the original's; its results "
                   f"{'taken' if flow == 'replayed' else 'let go, the query asked again'}")

    weather(patched, stock_requests, expect)
    a_change(patched, stock, stock_requests, expect)
    print("all cases passed")


def weather(patched, stock_requests, expect):
    """The Room's weather at the start's entry and at a return from a match (room-weather.md)."""
    # (label, saved pick, srp_room_rain, hour, counter, the project asked, srp_room_rain after, user.cfg saves)
    cases = [
        ("the Room by day, no spell, a dry roll", "1", "", 12, DRY_COUNTER, "camp_069", "", 0),
        ("the Room at night (22:30)", "1", "", 22, DRY_COUNTER, "camp_069_night", "", 0),
        ("the Room at 05:30: still night", "", "0", 5, DRY_COUNTER, "camp_069_night", "0", 0),
        ("the Room at 06:30: day", "", "0", 6, DRY_COUNTER, "camp_069", "0", 0),
        ("the Room, a rain rolled: a spell of 3 more", "1", "", 12, WET_COUNTER, "camp_069_rain", "3", 1),
        ("the Room in a saved spell (2 left), the roll unused", "1", "2", 12, DRY_COUNTER, "camp_069_rain", "1", 1),
        ("the Room's last spell entry", "1", "1", 12, WET_COUNTER, "camp_069_rain", "0", 1),
        ("the Room at night in a spell: the night, the entry counted", "1", "3", 23, DRY_COUNTER, "camp_069_night", "2", 1),
        ("the Basement in a spell: the classic, the entry counted", "2", "3", 12, DRY_COUNTER, "day", "2", 1),
        ("the Hangar at night", "4", "", 22, DRY_COUNTER, "hangar_046", "", 0),
    ]
    for label, pick, spell, hour, counter, asked, after, saves in cases:
        r = Run(patched)
        node = r.lobby_variable()
        r.emu.mem_write(r.read(node + 0x40), pick.encode() + b"\0")
        r.rain(spell)
        r.hour, r.counter, r.saves = hour, counter, 0
        requests, data, _ = r.query()
        others_same = requests[:3] + requests[4:] == stock_requests[:3] + stock_requests[4:]
        again, _, _ = r.query()
        expect(requests[3] == (f"lobby_scene/{asked}", 0x40) and data[3] != 0 and others_same
               and r.rain() == after.encode() and r.saves == saves and r.local_times == 1 and again[3] == requests[3],
               f"start, {label}: asks lobby_scene/{asked}; srp_room_rain {after or '(none)'} after"
               f"{', user.cfg saved' if saves else ''}; the clock read once (the query asked again: the same)")

    # a return from a match: another weather, the black at once and the new Room asked alone
    r = Run(patched)
    node = r.lobby_variable()
    r.emu.mem_write(r.read(node + 0x40), b"1\0")
    requests, _, _ = r.query()
    assert requests[3] == ("lobby_scene/camp_069", 0x40), requests[3]
    assert r.ready(project(r, 1))[0] == "replayed"
    r.lobby()
    r.camera()
    r.tick()
    quiet = [r.tick() for _ in range(5)]
    r.deactivate()
    r.hour = 22
    r.invokes()
    r.activate()
    at_return = [values for movie, name, values in r.invokes() if name == "root.srp_scene_fade" and movie == MOVIE]
    asked, _, _ = r.tick()
    alone, _, _ = r.query()
    config = "resources/projects/lobby_scene/camp_069_night/client_project"
    expect(all(t == ([], [], []) for t in quiet) and at_return == [(1000, 1)] and asked == [MENU]
           and alone[3] == ("lobby_scene/camp_069_night", 0x40) and all(q == (config, 0x1F) for q in alone[:3] + alone[4:])
           and r.local_times == 2,
           "a return from a match at 22:30 (the Room by day shown): the black whole with the spinner at once "
           "(srp_scene_fade 1000 1), the next frame asks lobby_scene/camp_069_night alone (its config in every "
           "other request); the clock read at the start and the return only")
    # a return into the same weather: nothing; lobby hops between entries: no clock read
    r = Run(patched)
    node = r.lobby_variable()
    r.emu.mem_write(r.read(node + 0x40), b"1\0")
    r.query()
    r.ready(project(r, 1))
    r.lobby()
    r.camera()
    r.tick()
    r.emu.mem_write(r.read(node + 0x40), b"2\0")
    for _ in range(3):
        r.tick()
    r.emu.mem_write(r.read(node + 0x40), b"1\0")
    for _ in range(60):
        r.tick()
    hops = r.local_times
    r.deactivate()
    r.invokes()
    r.activate()
    fades = [values for movie, name, values in r.invokes() if name == "root.srp_scene_fade" and movie == MOVIE]
    expect(hops == 1 and r.local_times == 2 and fades == [],
           "picks between entries (the Room, the Basement, the Room) read no clock; a return into the same "
           "weather (noon, a dry roll) steps no black")


def a_change(patched, stock, stock_requests, expect):
    """A pick shown at once (lobby-scenes.md, "A change shown at once"): a lobby shown, a pick or a
    squad changed, the lobby's ticks."""
    config = lambda lobby: f"resources/projects/lobby_scene/{lobby}/client_project"
    fades = lambda invokes: [values for movie, name, values in invokes if name == "root.srp_scene_fade" and movie == MOVIE]
    states = lambda invokes: [values for movie, name, values in invokes if name == "root.srp_lobby_state" and movie == MOVIE]

    def shown(value, lobby):
        """A fresh game whose lobby `lobby` was taken at its start (the saved pick `value`), the
        lobby and its camera made up, its first frame ticked (`r.first`: its invokes)."""
        r = Run(patched)
        node = r.lobby_variable()
        r.saved = lambda text: r.emu.mem_write(r.read(node + 0x40), text.encode() + b"\0")
        r.saved_text = lambda: r.text(r.read(node + 0x40))
        r.saved(value)
        requests, _, _ = r.query()
        assert requests[3] == (lobby, 0x40), requests[3]
        assert r.ready(project(r, 1))[0] == "replayed"
        r.lobby()
        r.camera()
        r.first = r.tick()
        r.word(EYE_ADAPTATION, 0)
        return r

    def to_black(r):
        """Ticks until the black is whole and the query asked: their fades."""
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

    # the first frame: the window told; then nothing
    r = shown("2", "lobby_scene/day")
    expect(r.first[0] == [] and states(r.first[2]) == [(2, 2)] and fades(r.first[2]) == [],
           "the first frame tells the lobby window its state (the pick 2, the lobby shown 2), asks nothing")
    ticks = [r.tick() for _ in range(5)]
    expect(all(t == ([], [], []) for t in ticks) and r.read(MENU + 0x114) == OLD - 0x38,
           "five more ticks with the pick shown: nothing asked, no fade, the window not told again")

    # a pick: the black stepped in, then the project alone behind it
    r.saved("3")
    out = to_black(r)
    steps = [f for _, _, f in out]
    expect(15 <= len(out) <= 17 and all(len(f) <= 1 for f in steps)
           and [f[0][0] for f in steps[1:-1]] == sorted(f[0][0] for f in steps[1:-1])
           and all(0 < f[0][0] < 1000 and f[0][1] == 0 for f in steps[1:-1])
           and steps[-1] == [(1000, 1)] and out[-1][0] == [MENU],
           f"the Bunker picked (3): the fade to black, a step a frame ({len(out)} frames: srp_scene_fade at "
           f"{', '.join(str(f[0][0]) for f in steps[1:5])}... per mille), then whole with the loading spinner "
           "(1000, 1) and the lobby's query asked")
    requests, data, _ = r.query()
    expect(requests == [(config("stk2_bunker"), 0x1F)] * 3 + [("lobby_scene/stk2_bunker", 0x40)]
           + [(config("stk2_bunker"), 0x1F)] * 4 and data == [0] * 3 + data[3:4] + [0] * 4 and data[3] != 0,
           "the query asks stk2_bunker's project alone (40h, the lobby menu's variant), its config (1Fh, no user "
           "data) in every other place")
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
    expect(r.pose() == (0.0, 0.0, 1.96, 1.96, 0.0) and states(invokes) == [(3, 3)],
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
    r.saved("4")
    r.tick()
    f = fades(r.tick(3.0)[2])
    expect(len(f) == 1 and 100 <= f[0][0] <= 140,
           f"a 3 s frame (a stall) during the fade: the black at {f[0][0] if f else None} per mille (a 30th of a "
           "second's step), not whole")

    # a project that fails behind the black: logged, faded back over the lobby shown, not asked again
    r = shown("2", "lobby_scene/day")
    r.saved("1")
    to_black(r)
    r.query()
    flow = r.ready(project(r, 0))
    fade_back = fades(r.invokes())
    later = []
    while len(later) < 40:
        later.append(r.tick())
        if fades(later[-1][2]) == [(0, 0)]:
            break
    after = [r.tick() for _ in range(20)]
    expect(flow == ("returned", []) and fade_back == [(1000, 0)] and r.read(MENU + 0x114) == OLD - 0x38 and r.logged == 1
           and fades(later[-1][2]) == [(0, 0)] and all(t == ([], [], []) for t in after),
           "camp_069's project without its track behind the black: logged (srp_patches.log), nothing put in, the "
           "black stepped out over the classic, not asked again")

    # 0xA1: the picks; srp has none (the sign-in): the saved pick sent; a squad's commander's pick shown
    r = shown("2", "lobby_scene/day")
    r.squad(0, b"\x00\x00")
    expect(r.sent == [(CLIENT, PICK_BASE + 2, 1)] and r.saved_text() == b"2",
           "0xA1 of no squad, srp's picks 0 0 (the sign-in): the saved pick sent (3B at 2 095 000 002, count 1, "
           "the handler's lobby client)")
    r.sent = []
    r.squad(0, b"\x04\x00")
    _, _, invokes = r.tick()
    expect(r.sent == [] and r.saved_text() == b"4" and states(invokes) == [(4, 2)],
           "0xA1 of no squad with the own pick 4 (a click's, srp's answer): saved (srp_lobby 4), the window told, "
           "the change begun")
    r = shown("2", "lobby_scene/day")
    r.squad(7, b"\x02\x01")
    out = to_black(r)
    requests, _, _ = r.query()
    expect(out[-1][0] == [MENU] and requests[3] == ("lobby_scene/camp_069", 0x40) and r.saved_text() == b"2",
           "0xA1 of squad 7, picks 2 1: the commander's pick (the Room, 1) asked behind the black; the own pick 2 "
           "kept")

    # a match entered during the fade: the black gone; behind the black: gone, back when shown,
    # the project that comes while hidden put in at once
    r = shown("2", "lobby_scene/day")
    r.saved("1")
    r.tick()
    r.tick()
    asked = r.deactivate()
    expect(asked == [] and fades(r.invokes()) == [(0, 0)],
           "a match entered during the fade to black: the black gone (srp_scene_fade 0)")
    r = shown("2", "lobby_scene/day")
    r.saved("1")
    to_black(r)
    r.query()
    r.deactivate()
    gone = fades(r.invokes())
    r.log = []
    flow = r.ready(PROJECT, swap=True)
    expect(gone == [(0, 0)] and flow == ("returned", []) and r.log == stock_log and r.state() == stock_state
           and fades(r.invokes()) == [(0, 0)],
           "a match entered behind the black: the black gone; the project's results after: put in at once (the "
           "original's halves)")
    r = shown("2", "lobby_scene/day")
    r.saved("1")
    to_black(r)
    r.query()
    r.deactivate()
    r.activate()
    expect(fades(r.invokes()) == [(1000, 1)],
           "back from the match while the project loads: the black and the spinner again")

    under_the_table(shown, to_black, fades, stock_log, expect)

    # the game closing: results let go untouched
    r = shown("2", "lobby_scene/day")
    r.saved("4")
    to_black(r)
    r.query()
    r.word(CLIENT + 0x535C, 0x13579BDF)
    esi, ecx = r.closing()
    before = r.state()
    flow = r.ready(PROJECT, swap=True)
    expect(esi == CLIENT and ecx == 0x13579BDF and flow == ("returned", []) and r.log == [] and r.state() == before
           and r.deactivate() == [],
           "game::clear_resources' start (its span replayed), then a change's results: let go, nothing touched; "
           "its on_deactivate of the lobby asks nothing")


def under_the_table(shown, to_black, fades, stock_log, expect):
    """A pick while patch 45's camera is the table's (a weapon's upgrade window: its spot read from
    the lobby shown's project): the lobby shown kept until the window is closed and the camera home."""
    def quiet(r, ticks):
        """`ticks` frames: whether none asked a query, stepped the black or swapped."""
        return all(asked == [] and log == [] and fades(invokes) == [] for asked, log, invokes in
                   (r.tick() for _ in range(ticks)))

    def then_the_change(r):
        """The window closed: the flight home, then the pick's change and the new lobby at the
        camera's ctor's pose."""
        r.table(0)
        home = 0
        while r.read(TABLE_STATE + TABLE_FLIGHT) != 0 and home < 60:
            asked, log, invokes = r.tick()
            assert asked == [] and log == [] and fades(invokes) == [], (home, invokes)
            home += 1
        out = to_black(r)
        requests, _, _ = r.query()
        flow = r.ready(PROJECT, swap=True)
        swapped = r.log
        r.tick()
        return (20 <= home <= 30 and out[-1][0] == [MENU] and requests[3] == ("lobby_scene/stk2_bunker", 0x40)
                and flow == ("returned", []) and swapped == stock_log and r.pose() == (0.0, 0.0, 1.96, 1.96, 0.0)), home

    r = shown("2", "lobby_scene/day")
    r.table(1)
    r.saved("3")
    kept = quiet(r, 60) and r.read(MENU + 0x114) == OLD - 0x38
    changed, home = then_the_change(r)
    expect(kept and changed,
           "the Bunker picked with a weapon's upgrade window open (patch 45's table on, its camera flown to the "
           "spot): 60 frames, nothing asked, no black, the classic's project kept; the window closed: the camera "
           f"flies home ({home} frames, still nothing), then the change: the black, the Bunker's query, its project "
           "put in, the camera at the ctor's pose")

    r = shown("2", "lobby_scene/day")
    r.saved("3")
    r.tick()
    r.tick()
    r.table(1)
    back = []
    while len(back) < 40:
        asked, log, invokes = r.tick()
        back.append((asked, log, fades(invokes)))
        if back[-1][2] == [(0, 0)]:
            break
    stepped_out = back[-1][2] == [(0, 0)] and all(asked == [] and log == [] for asked, log, _ in back)
    changed, _ = then_the_change(r)
    expect(stepped_out and changed,
           f"the window opened during the fade to black: the black stepped back out ({len(back)} frames), nothing "
           "asked; closed: the change plays")

    r = shown("2", "lobby_scene/day")
    r.saved("3")
    to_black(r)
    r.query()
    r.table(1)
    flow = r.ready(PROJECT, swap=True)
    let_go = flow == ("returned", []) and r.log == [] and r.read(MENU + 0x114) == OLD - 0x38 \
        and fades(r.invokes()) == [(1000, 0)]
    out = []
    while len(out) < 40:
        f = fades(r.tick()[2])
        out.append(f)
        if f == [(0, 0)]:
            break
    changed, _ = then_the_change(r)
    expect(let_go and out[-1] == [(0, 0)] and changed,
           "the window opened while the Bunker's project loads behind the black: its results let go, nothing put "
           "in (the classic's project, the spot's, kept), the spinner off and the black stepped out; closed: the "
           "change plays")


if __name__ == "__main__":
    main()
