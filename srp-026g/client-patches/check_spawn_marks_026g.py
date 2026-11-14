#!/usr/bin/env python3
"""The spawn marks (rust/patches/src/spawn_marks.rs, vostok/free_fly.rs;
client_patch.md section 3.82): a local build's tool, `patch_026g.py
--spawn-marks`, that no other build has.

    python check_spawn_marks_026g.py <original survarium.exe>

Requires Python `unicorn`. check_tab_cursor_026g.py's harness (the exe's
sections and the arena as they are, Scaleform's Invoke 0x511600 recorded),
three exes beside each other: the original, the default build and the
--spawn-marks build. Made up: the game world with its hud, game, network
client, camera director, the player's input handler and the free-fly camera
(each camera with the game's own vtables), and an input world that records
the handlers added and removed. Stubbed: kernel32's CreateFileA, WriteFile,
CloseHandle, FindFirstFileA, FindClose (files kept by name) and the three
time imports. The game's own code runs the rest: on_keyboard_action's start,
camera_director::switch_to_camera (0x243510), switch_to_player_camera
(0x1F8200), each camera's on_activate, on_deactivate and on_focus, the
free-fly camera's on_keyboard_action (0x2494D0), on_mouse_key_action
(0x249660), on_mouse_move (0x2496A0) and tick (0x249A30). Checked:

- the default build: F7, F8 and F9 (down, held, up) through
  on_keyboard_action's start leave the registers and the stack the
  original's, open no file, invoke nothing and change no byte of the world,
  the director, the handler or the free-fly camera; the lock's arena has no
  symbol of the tool but the empty handler's shim;
- the --spawn-marks build, F8 and F9 on the player's camera (each press
  taken: al 1, `ret 0Ch`): a line each of
  srp_spawn_marks.tsv after its header (written once: a second mark finds
  the file not empty), its columns the header's, its numbers the director's
  view's and the local player's feet, the map and its id; the hud's centre
  message and srp_patches.log say the mark's number; a held or released key
  marks nothing; the file refused: no line, the message says so and the
  next mark has the number again; no camera on: no mark;
- F7 in a live match: the handler's mode 1 and the director's camera the
  free-fly one, by the game's own switch (the player's handler removed from
  the input world, the free-fly camera's added; its view the director's
  with its up the world's); a mark there says `free`; F7 again: the game's
  switch_to_player_camera (mode 0, the handlers the other way round); no
  player bound: nothing switched; a replay: nothing switched;
- the free-fly camera itself, as the game's own code steers it: a held key
  bound to an action and a held mouse button are kept for the tick; a tick
  of 16 ms moves 0.096 m: the left button along the view's forward, the
  right one back, D and A along its right, W and S along its up, 20 times
  as far with Ctrl, a tenth with Shift; a mouse move turns the view;
- the markers, through game_world::tick's join after the move (every
  handler of the site run, the others' too) with query_resources and the
  scene renderer's play_particle_system and remove_particle_system_instance
  stubbed (recorded) and the query's callback called through the game's
  invoker: the first frame of a match reads the file and asks one query of
  a particle system instance (class 61) for each mark of its map, the
  spawns' green spores first, then the no-spawns' bonfire; nothing is
  played before they come, and none twice; the next frame plays each at
  its spot (the feet of a walked mark, the camera of a flown one or of one
  without a player), upright, no attachment, one reference of ours kept;
  a mark (F8) is shown the same way; F10 writes the file without the map's
  last line (srp_spawn_marks.tsv.new, then MoveFileExA over the file),
  takes that marker out of the scene and lets its particle go, and says
  so; another map's first frame takes the markers out and shows its own;
  a particle that did not load is said once and never asked again; a
  world without a match, a file past 256 KiB: nothing asked; the default
  build's frame asks, reads and plays nothing.
Says nothing about a real game: the screen, the input world's real
dispatch, a match going on meanwhile, how the particles look.
"""
import math
import struct
import sys

from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base
import check_hide_ui_026g as hide
import check_tab_cursor_026g as tab
from check_console_where_026g import transform

F7, F8, F9, F10, W, A, S, D, LCTRL, LSHIFT = 0x41, 0x42, 0x43, 0x44, 0x11, 0x1E, 0x1F, 0x20, 0x1D, 0x2A
LEFT_BUTTON, RIGHT_BUTTON = 0x151, 0x152
DOWN, UP, HELD = 1, 2, 3
# the game world of the key: the input handler is its +1F8h, the game holds it at +18h
WORLD = hide.HANDLER - 0x1F8
GAME = WORLD - 0x18
HUD = hide.HANDLER + 4
BELOW = hide.HANDLER - 0x10000
OBJECTS = 0x48000000
DIRECTOR, PLAYER_HANDLER, FREE, PLAYER, INPUT, TABLES, STUBS, ENGINE, EVENTS = (OBJECTS + 0x10000 * i for i in range(9))
CLIENT, HOLDER = tab.NC, tab.OPTIONS_HOLDER
ENGINE_SLOT = 0x1F1D310
LIVE_CLIENT, REPLAY_CLIENT = 0x9FFB14, 0x9FF344
HANDLER_CAMERA_VTABLE, HANDLER_VTABLE = 0xA0406C, 0xA04048       # player_input_handler: +4 game_camera, +0
FREE_VTABLE, FREE_INPUT_VTABLE = 0xA047F0, 0xA04808              # free_fly_camera: +0 game_camera, +94h input
FREE_KEY, FREE_BUTTON, FREE_MOVE, FREE_TICK = 0x2494D0, 0x249660, 0x2496A0, 0x249A30
SLOTS = dict(create=(0x870048, 0x1C), write=(0x870108, 0x14), close=(0x870040, 4), find=(0x870238, 8),
             find_close=(0x870228, 4), now=(0x870244, 4), to_local=(0x870230, 8), to_system=(0x87022C, 8),
             read_file=(0x870064, 0x14), module=(0x87003C, 4), proc=(0x870084, 8), delete=(0x87021C, 4))
MARKS, LOG, NEW_MARKS = b"srp_spawn_marks.tsv", b"srp_patches.log", b"srp_spawn_marks.tsv.new"
GENERIC_READ, GENERIC_WRITE, CREATE_ALWAYS, OPEN_EXISTING = 1 << 31, 1 << 30, 2, 3
# the markers: game_world::tick's join after the move (game_world.after_move, 5 bytes), the resource
# query, the bound function's invoker, the scene renderer's play and remove of a particle system
AFTER_MOVE, AFTER_MOVE_PAST = 0x1F7C3C, 0x1F7C41
QUERY, INVOKER, FUNCTION_VTABLE = 0xE5B60, 0x3AAA20, 0x9DB9E0
PLAY, REMOVE = 0x22660, 0x229D0
RENDERER, SCENE_RENDERER, SCENE, INSTANCES, MATCH = 0x52000000, 0x52200040, 0x0BADC0DE, 0x53000000, 0x5A5A0000
SPORES, BONFIRE = "fx/level_4_anomaly/spori", "fx/lights/bonfire"
OTHER_MAP = "level_01/day_8"
HEADER = b"n\ttime\tmap\tmap_id\tkind\tcamera\tx\ty\tz\tyaw\tpitch\tdx\tdy\tdz\tplayer_x\tplayer_y\tplayer_z\n"
MAP, MAP_ID = "v021d_level_03/day_2", 61
FEET = (-48.12, 5.414, 10.002)
MESSAGE = "root.set_parameterized_message"
NOW_MS = 600_000


class Run(tab.Run):
    def __init__(self, data):
        super().__init__(data)
        emu = self.emu
        # the keys the free-fly camera reads, bound as the defaults bind them (W is the harness's already)
        actions = {self.text(self.read(hide.ACTIONS + 20 * n)): self.read(hide.ACTIONS + 20 * n + 4)
                   for n in range((hide.KEY_NAMES - hide.ACTIONS) // 20)}
        for name, key in ((b"back", S), (b"lstrafe", A), (b"rstrafe", D), (b"crouch", LCTRL), (b"accel", LSHIFT)):
            self.word(hide.BINDER + 12 * actions[name] + 4, hide.key_entry(data, key))
        emu.mem_map(BELOW, 0x10000, UC_PROT_ALL)
        emu.mem_map(OBJECTS, 0x90000, UC_PROT_ALL)
        emu.mem_map(RENDERER + 0x200000, 0x1000, UC_PROT_ALL)
        emu.mem_map(INSTANCES, 0x10000, UC_PROT_ALL)
        emu.mem_map(MATCH, 0x40000, UC_PROT_ALL)              # the world's match, zeros: the other frame handlers find no core
        self.files, self.handles, self.opened, self.refused_files, self.input = {}, {}, [], (), []
        self.queries, self.played, self.removed, self.moved = [], [], [], []
        for i, (name, (slot, pops)) in enumerate(SLOTS.items()):
            at = STUBS + 0x10 * i
            emu.mem_write(at, b"\xC2" + struct.pack("<H", pops))
            self.word(slot, at)
            emu.hook_add(UC_HOOK_CODE, getattr(self, "import_" + name), begin=at, end=at)
        # MoveFileExA, where the stubbed GetProcAddress says it is (stdcall, 3 arguments)
        self.move_file = STUBS + 0x10 * len(SLOTS)
        emu.mem_write(self.move_file, b"\xC2\x0C\x00")
        emu.hook_add(UC_HOOK_CODE, self.import_move, begin=self.move_file, end=self.move_file)
        for at, what in ((QUERY, self.on_query), (PLAY, self.on_play), (REMOVE, self.on_remove)):
            emu.hook_add(UC_HOOK_CODE, what, begin=at, end=at)
        # the game's vtable: +38h its input world; the input world's: +10h add_handler, +14h remove_handler
        game_table, input_table, code = TABLES, TABLES + 0x100, TABLES + 0x200
        emu.mem_write(code, b"\xB8" + struct.pack("<I", INPUT) + b"\xC3")
        self.word(game_table + 0x38, code)
        for slot, what in ((0x10, "add"), (0x14, "remove")):
            emu.mem_write(code + slot, b"\xC2\x04\x00")
            self.word(input_table + slot, code + slot)
            emu.hook_add(UC_HOOK_CODE, self.handler_call, begin=code + slot, end=code + slot, user_data=what)
        self.tables = (game_table, input_table)

    # ---- kernel32

    def argument(self, i):
        return self.read(self.emu.reg_read(UC_X86_REG_ESP) + 4 + 4 * i)

    def import_create(self, emu, *_):
        name, disposition = self.text(self.argument(0)), self.argument(4)
        self.opened.append((name, self.argument(1), disposition))
        if name in self.refused_files or (disposition == OPEN_EXISTING and name not in self.files):
            return emu.reg_write(UC_X86_REG_EAX, 0xFFFFFFFF)
        handle = 0x100 + 4 * len(self.handles)
        self.handles[handle] = [name, 0]
        if disposition == CREATE_ALWAYS:
            self.files[name] = b""
        self.files.setdefault(name, b"")
        emu.reg_write(UC_X86_REG_EAX, handle)

    def import_read_file(self, emu, *_):
        handle, at, size, got = (self.argument(i) for i in range(4))
        name, position = self.handles[handle]
        chunk = self.files[name][position:position + size]
        emu.mem_write(at, chunk)
        self.handles[handle][1] = position + len(chunk)
        self.word(got, len(chunk))
        emu.reg_write(UC_X86_REG_EAX, 1)

    def import_module(self, emu, *_):
        emu.reg_write(UC_X86_REG_EAX, 0x7C800000 if self.text(self.argument(0)) == b"kernel32.dll" else 0)

    def import_proc(self, emu, *_):
        known = self.argument(0) == 0x7C800000 and self.text(self.argument(1)) == b"MoveFileExA"
        emu.reg_write(UC_X86_REG_EAX, self.move_file if known else 0)

    def import_move(self, emu, *_):
        old, new, flags = self.text(self.argument(0)), self.text(self.argument(1)), self.argument(2)
        self.moved.append((old, new, flags))
        self.files[new] = self.files.pop(old)
        emu.reg_write(UC_X86_REG_EAX, 1)

    def import_delete(self, emu, *_):
        emu.reg_write(UC_X86_REG_EAX, int(self.files.pop(self.text(self.argument(0)), None) is not None))

    def import_write(self, emu, *_):
        handle, at, size, written = (self.argument(i) for i in range(4))
        self.files[self.handles[handle][0]] += bytes(emu.mem_read(at, size))
        self.word(written, size)
        emu.reg_write(UC_X86_REG_EAX, 1)

    def import_close(self, emu, *_):
        emu.reg_write(UC_X86_REG_EAX, 1)

    def import_find(self, emu, *_):
        name, data = self.text(self.argument(0)), self.argument(1)
        if name not in self.files:
            return emu.reg_write(UC_X86_REG_EAX, 0xFFFFFFFF)
        emu.mem_write(data, bytes(28) + struct.pack("<II", 0, len(self.files[name])) + bytes(284))
        emu.reg_write(UC_X86_REG_EAX, 0x55)

    def import_find_close(self, emu, *_):
        emu.reg_write(UC_X86_REG_EAX, 1)

    def import_now(self, emu, *_):
        emu.mem_write(self.argument(0), struct.pack("<Q", 134_000_000_000_000_000))

    def import_to_local(self, emu, *_):
        emu.mem_write(self.argument(1), bytes(emu.mem_read(self.argument(0), 8)))
        emu.reg_write(UC_X86_REG_EAX, 1)

    def import_to_system(self, emu, *_):
        # SYSTEMTIME: year, month, weekday, day, hour, minute, second, milliseconds
        emu.mem_write(self.argument(1), struct.pack("<8H", 2026, 11, 6, 14, 9, 5, 33, 0))
        emu.reg_write(UC_X86_REG_EAX, 1)

    # ---- the markers' engine calls

    def returns(self, pops):
        """The stubbed function returns at once, `pops` bytes of arguments popped."""
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)

    def on_query(self, emu, *_):
        """query_resources (cdecl: EAX the requests, ECX their count; the function, the allocator, the user
        data, the parent, the flag): its requests (path, class) and its function, recorded."""
        requests, count = emu.reg_read(UC_X86_REG_EAX), emu.reg_read(UC_X86_REG_ECX)
        paths = [(self.text(self.read(requests + 8 * i)).decode(), self.read(requests + 8 * i + 4))
                 for i in range(count)]
        self.queries.append((paths, self.read(self.read(emu.reg_read(UC_X86_REG_ESP) + 4), 0x20),
                             self.argument(2), self.argument(3)))
        self.returns(0)

    def on_play(self, emu, *_):
        """scene_renderer::play_particle_system (renderer, scene, instance by value, two transforms, two
        attachments; ret 1Ch): recorded; the by-value reference let go, as the real one does at its end."""
        renderer, scene, instance, first, second, one, two = (self.argument(i) for i in range(7))
        self.played.append((renderer, scene, instance, struct.unpack("<16f", self.read(first, 64)),
                            struct.unpack("<16f", self.read(second, 64)), self.read(one), self.read(two)))
        self.word(instance + 0xD0, self.read(instance + 0xD0) - 1)
        self.returns(0x1C)

    def on_remove(self, emu, *_):
        """scene_renderer::remove_particle_system_instance (renderer, scene, &instance; ret 0Ch): recorded."""
        self.removed.append((self.argument(0), self.argument(1), self.read(self.argument(2))))
        self.returns(0xC)

    def frame(self):
        """game_world::tick's join after the move, every handler of the site, EBX the game world: what it
        asked, played and took out."""
        self.queries, self.played, self.removed, self.opened = [], [], [], []
        sp = hide.STACK + 0xFF00                          # the handlers' frames take more than 8000h
        self.word(sp, hide.SENTINEL)
        _, end = self.run(AFTER_MOVE, (AFTER_MOVE_PAST,), sp, {**hide.GAME, UC_X86_REG_EBX: WORLD})
        assert self.stopped == AFTER_MOVE_PAST and end == sp, (self.stopped, end - sp)
        return self.queries, self.played, self.removed

    def instances(self, count, first=0):
        """`count` particle system instances (made up: each its count +0D0h, 1: the result's)."""
        out = [INSTANCES + 0x100 * (first + i) for i in range(count)]
        for at in out:
            self.word(at + 0xD0, 1)
        return out

    def came(self, query, instances):
        """The query's callback through the game's invoker, a result each (0: one that failed; 2E0h apart)."""
        _, function, _, _ = query
        results, at_function = INSTANCES + 0x4000, INSTANCES + 0xF000     # past the instances (0x100 each)
        self.emu.mem_write(results, bytes(0x12C + 0x2E0 * len(instances)))
        self.word(results + 0x38, len(instances))
        for i, instance in enumerate(instances):
            self.word(results + 0x12C + 0x2E0 * i, instance)
        self.emu.mem_write(at_function, function)
        sp = hide.STACK + 0x8000
        for i, value in enumerate((hide.SENTINEL, at_function + 8, results)):
            self.word(sp + 4 * i, value)
        _, end = self.run(INVOKER, (hide.SENTINEL,), sp, dict(hide.GAME))
        assert self.stopped == hide.SENTINEL and end == sp + 4, (self.stopped, end - sp)

    def handler_call(self, emu, address, size, what):
        assert emu.reg_read(UC_X86_REG_ECX) == INPUT
        self.input.append((what, self.argument(0)))

    # ---- the game's objects

    def world(self, client=LIVE_CLIENT, bound=True, camera="player", view=None, hud_movie=True, map=MAP, match=True):
        """A match's game world as the key's handler has it: the screen on `camera` ("player", "free" or
        None), the director's view `view`, a live or a replay client, the local player bound or not."""
        emu = self.emu
        emu.mem_write(BELOW, bytes(0x10000))
        emu.mem_write(hide.HANDLER, bytes(0x10000))
        emu.mem_write(OBJECTS, bytes(0x50000))
        emu.mem_write(ENGINE, bytes(0x20000))
        emu.mem_write(CLIENT, bytes(0x200000))
        emu.mem_write(HOLDER, bytes(0x60000))
        game_table, input_table = self.tables
        self.movies(HUD, hud_movie, False)
        self.word(HUD + 0x18, WORLD)
        self.word(WORLD + 0x9C, GAME)
        self.word(WORLD + 0x98, DIRECTOR)
        self.word(WORLD + 0x4D50, FREE)
        self.word(WORLD + 0x4D58, PLAYER_HANDLER if bound else 0)
        self.word(WORLD + 0x4D4C, MATCH if match else 0)      # the world's match
        emu.mem_write(MATCH, bytes(0x40000))
        self.word(WORLD + 4, SCENE)                           # its render scene
        self.word(GAME + 0x5360, RENDERER)
        self.word(RENDERER + 0x200064, SCENE_RENDERER)
        emu.mem_write(WORLD + 0xD4, b"\x01")                  # the scene active
        self.word(GAME, game_table)
        self.word(GAME + 0x5344, hide.BINDER)
        self.word(GAME + 0x584C, NOW_MS)
        self.word(GAME + 0x53AC, CLIENT)
        text = GAME + 0x55EC
        emu.mem_write(text, map.encode() + b"\0")
        for i, value in enumerate((text, text + len(map), text + 0x200)):
            self.word(GAME + 0x55E0 + 4 * i, value)
        self.word(ENGINE_SLOT, ENGINE)
        self.word(ENGINE + 0x2D0, GAME)
        self.word(CLIENT, client)
        self.word(CLIENT + 4, PLAYER if bound else 0)
        if client == LIVE_CLIENT:
            self.word(CLIENT + 0x1F1C48, HOLDER)
            emu.mem_write(HOLDER + 0xC + 0x577B4, bytes([MAP_ID]))
        else:
            emu.mem_write(CLIENT + 0x1C + 0x577B4, bytes([MAP_ID]))
        emu.mem_write(PLAYER + 0x43C, struct.pack("<3f", *FEET))
        self.word(INPUT, input_table)
        emu.mem_write(DIRECTOR + 4, view or transform(0.0, 0.0, (0.0, 0.0, 0.0)))
        self.word(DIRECTOR + 0x88, WORLD)
        self.word(DIRECTOR + 0x84, {"player": PLAYER_HANDLER + 4, "free": FREE, None: 0}[camera])
        self.word(PLAYER_HANDLER, HANDLER_VTABLE)
        self.word(PLAYER_HANDLER + 4, HANDLER_CAMERA_VTABLE)
        self.word(PLAYER_HANDLER + 0x334, WORLD)
        self.word(FREE, FREE_VTABLE)
        self.word(FREE + 0x94, FREE_INPUT_VTABLE)
        self.word(FREE + 0x84, WORLD)
        self.word(FREE + 0xE0, DIRECTOR)
        for vector in (0xEC, 0xF8):                           # the keys and buttons of a tick: empty, room for 64
            at = EVENTS + (0x1000 if vector == 0xF8 else 0)
            for i, value in enumerate((at, at, at + 0x100)):
                self.word(FREE + vector + 4 * i, value)

    def snapshot(self):
        return b"".join(self.read(at, size) for at, size in
                        ((BELOW, 0x10000), (hide.HANDLER, 0x10000), (OBJECTS, 0x40000)))

    def press(self, key, kind=DOWN):
        """on_keyboard_action's start for the key: what tab.Run.key gives, the files opened, the movie's
        messages and the input world's calls."""
        self.opened, self.input = [], []
        result = self.key(key, kind)
        messages = [values for _, name, values in self.invokes() if name == MESSAGE]
        return result[:4], self.opened, messages, self.input

    def thiscall(self, function, this, *args):
        sp = hide.STACK + 0x8000
        self.word(sp, hide.SENTINEL)
        for i, value in enumerate(args):
            self.word(sp + 4 + 4 * i, value)
        _, end = self.run(function, (hide.SENTINEL,), sp, {**hide.GAME, UC_X86_REG_ECX: this})
        assert self.stopped == hide.SENTINEL and end == sp + 4 + 4 * len(args), (hex(function), self.stopped)

    def tick(self, keys=(), buttons=(), mouse=(0, 0, 0), ms=16):
        """The free-fly camera fed as the input world feeds it, then its tick `ms` later: the view it had
        (the director's, which the tick starts from) and the view it leaves (+44h)."""
        before = struct.unpack("<16f", self.read(DIRECTOR + 4, 64))
        for key in keys:
            self.thiscall(FREE_KEY, FREE + 0x94, INPUT, key, HELD)
        for button in buttons:
            self.thiscall(FREE_BUTTON, FREE + 0x94, INPUT, button, 2)
        if any(mouse):
            self.thiscall(FREE_MOVE, FREE + 0x94, INPUT, *mouse)
        self.word(GAME + 0x584C, self.read(GAME + 0x584C) + ms)
        self.thiscall(FREE_TICK, FREE)
        return before, struct.unpack("<16f", self.read(FREE + 0x44, 64))


def check(ok, text):
    print("ok", text) if ok else sys.exit("FAIL " + text)


def taken(result):
    """Whether on_keyboard_action returned at once with the key taken: al 1, `ret 0Ch`, the callee-saved
    registers the caller's."""
    stopped, al, popped, regs = result
    return stopped == hide.SENTINEL and al == 1 and popped == 16 and all(
        regs[r] == hide.GAME[r] for r in (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP))


def close(got, want, within):
    return len(got) == len(want) and all(abs(a - b) <= within for a, b in zip(got, want))


def columns(line):
    return line.rstrip(b"\n").decode().split("\t")


def mark_is(line, n, kind, camera, view, feet=FEET):
    """Whether a file's line is the mark `n` of the director's view `view`."""
    c = columns(line)
    v = struct.unpack("<16f", view)
    f = v[8:11]
    length = math.sqrt(sum(x * x for x in f))
    d = [x / length for x in f]
    yaw = math.degrees(math.atan2(d[0], d[2])) % 360
    pitch = math.degrees(math.atan2(d[1], math.hypot(d[0], d[2])))
    return (len(c) == len(columns(HEADER)) and line.endswith(b"\n")
            and c[:6] == [str(n), "11-14 09:05", MAP, str(MAP_ID), kind, camera]
            and close([float(x) for x in c[6:9]], v[12:15], 0.0006)
            and abs((float(c[9]) - yaw + 180) % 360 - 180) <= 0.06 and abs(float(c[10]) - pitch) <= 0.06
            and close([float(x) for x in c[11:14]], d, 0.0011)
            and (c[14:17] == ["-"] * 3 if feet is None else close([float(x) for x in c[14:17]], feet, 0.0006))
            and all(len(x.split(".")[1]) == 3 for x in c[6:9] + c[11:14]))


def line_of(n, map_, kind, camera, place, feet):
    """A line of the file as the tool writes it (its numbers to three decimals), `feet` None for dashes."""
    numbers = lambda values: [f"{v:.3f}" for v in values]
    feet_columns = ["-"] * 3 if feet is None else numbers(feet)
    columns = [str(n), "11-14 01:41", map_, "61", kind, camera, *numbers(place), "90.0", "0.0",
               "1.000", "0.000", "0.000", *feet_columns]
    return ("\t".join(columns) + "\n").encode()


def at(matrix):
    """A transform's place (its last row) and whether its rows are the world's axes."""
    return matrix[12:15], matrix[:12] == (1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0) and matrix[15] == 1.0


def markers(marked, view):
    """The markers: shown from the file at a map's first frame, a mark's, F10, another map, a failed particle."""
    r = Run(marked)
    r.world(view=view)
    walked, flown, other, bare = (1.5, -2.25, 3.125), (10.0, 20.5, -30.75), (7.0, 8.0, 9.0), (-4.5, 0.25, 6.0)
    r.files = {MARKS: HEADER + line_of(1, MAP, "spawn", "player", (0.5, 3.0, 0.5), walked)
               + line_of(2, OTHER_MAP, "no_spawn", "player", (0.0, 0.0, 0.0), other)
               + line_of(3, MAP, "no_spawn", "free", flown, walked)
               + line_of(4, MAP, "spawn", "player", bare, None)}
    kept = r.files[MARKS]
    asked, played, removed = r.frame()
    check((MARKS, GENERIC_READ, OPEN_EXISTING) in r.opened and len(asked) == 1 and played == removed == [],
          "a match's first frame: the file read, one query, nothing played before its particles come")
    paths, function, user, parent = asked[0]
    vtable, _entry, word = struct.unpack_from("<I4xII", function)
    check(paths == [(SPORES, 61), (SPORES, 61), (BONFIRE, 61)] and (vtable, word) == (FUNCTION_VTABLE | 1, 2)
          and user == 0 and parent == 0,
          "it asks a particle system instance (class 61) for each mark of the map, the two spawns' green spores, "
          "then the no-spawn's bonfire, the other map's none; tag 2, no user data, no parent")
    check(r.files[LOG].endswith(f"srp: spawn_marks: {MAP}: 3 marks of srp_spawn_marks.tsv shown\r\n".encode()),
          "srp_patches.log says how many marks of the map are shown")
    check(r.frame() == ([], [], []), "the next frame: no second query while one is out, nothing to play")
    spores_1, spores_2, fire = r.instances(3)
    r.came(asked[0], [spores_1, spores_2, fire])
    check(all(r.read(x + 0xD0) == 2 for x in (spores_1, spores_2, fire)) and r.played == [],
          "the callback keeps a reference of each instance and plays none (the scene is the frame's)")
    asked, played, removed = r.frame()
    check(asked == [] and removed == [] and [(p[2], at(p[3])) for p in played]
          == [(spores_1, (walked, True)), (fire, (flown, True)), (spores_2, (bare, True))],
          "the next frame plays each where its mark is: a walked spawn at the player's feet, a flown no-spawn at "
          "the camera, a spawn without a player at the camera; upright")
    check(all(p[:2] == (SCENE_RENDERER, WORLD + 4) and p[3] == p[4] and p[5:] == (0, 0) for p in played)
          and all(r.read(x + 0xD0) == 2 for x in (spores_1, spores_2, fire)),
          "each through the game's scene renderer into the world's scene, both transforms the same, no "
          "attachment; a reference of ours kept on each (the call's own let go)")
    check(r.frame() == ([], [], []), "and once: the next frame plays nothing again")

    got, opened, messages, calls = r.press(F8)
    check(taken(got) and messages == [("mark 1: spawn here", 0x21, 0x14, 2000)],
          "F8 on a shown map: marked as before")
    asked, played, removed = r.frame()
    check(len(asked) == 1 and asked[0][0] == [(SPORES, 61)] and played == [],
          "the next frame asks the new mark's particle alone")
    (new_spores,) = r.instances(1, first=3)
    r.came(asked[0], [new_spores])
    asked, played, removed = r.frame()
    check(len(played) == 1 and played[0][2] == new_spores and at(played[0][3])[1]
          and close(at(played[0][3])[0], FEET, 0.0006), "and plays it at the player's feet (to the file's millimetre)")
    marked_file = r.files[MARKS]

    got, opened, messages, calls = r.press(F10)
    check(taken(got) and r.files[MARKS] == kept and NEW_MARKS not in r.files
          and (NEW_MARKS, GENERIC_WRITE, CREATE_ALWAYS) in opened and r.moved[-1] == (NEW_MARKS, MARKS, 1),
          "F10: the file written whole without the map's last line (srp_spawn_marks.tsv.new, then MoveFileExA "
          "with MOVEFILE_REPLACE_EXISTING over the file), the other lines as they were")
    check(r.removed == [(SCENE_RENDERER, WORLD + 4, new_spores)] and r.read(new_spores + 0xD0) == 1
          and messages == [("took back the last mark: spawn here (3 shown)", 0x21, 0x14, 2000)]
          and r.files[LOG].endswith(b"srp: spawn_marks: took back the last mark: spawn here (3 shown)\r\n"),
          "its marker taken out of the scene and its particle let go; the hud and the log say so")
    r.removed = []
    r.press(F10)
    lines = r.files[MARKS].split(b"\n")
    check(len(lines) == 5 and lines[3].split(b"\t")[4] == b"no_spawn" and r.removed[-1][2] == spores_2
          and r.read(spores_2 + 0xD0) == 1 and marked_file != r.files[MARKS],
          "F10 again: the map's line before it (the spawn without a player) and its marker")

    r.world(view=view, map=OTHER_MAP)
    asked, played, removed = r.frame()
    check(sorted(x[2] for x in removed) == sorted([spores_1, fire]) and played == []
          and r.read(spores_1 + 0xD0) == r.read(fire + 0xD0) == 1
          and len(asked) == 1 and asked[0][0] == [(BONFIRE, 61)],
          "another map's first frame: the last map's markers out of the scene and let go, its own mark asked")
    r.came(asked[0], [0])
    check(r.files[LOG].endswith(f"srp: spawn_marks: the particle system {BONFIRE} did not load: a marker of it is "
                                "not shown\r\n".encode()) and r.frame() == ([], [], []) and r.frame() == ([], [], []),
          "a particle that did not load: said once, its marker never asked again")
    r.removed = []
    got, opened, messages, calls = r.press(F10)
    check(r.removed == [] and messages == [("took back the last mark: NO spawn near here (0 shown)", 0x21, 0x14, 2000)]
          and OTHER_MAP.encode() not in r.files[MARKS],
          "F10 of that marker, never shown: its line leaves the file, nothing is taken out of the scene")
    got, opened, messages, calls = r.press(F10)
    check(messages == [("NOTHING TAKEN BACK: no mark of this map in srp_spawn_marks.tsv", 0x21, 0x14, 2000)],
          "F10 with no mark of the map left: nothing taken, and the message says so")

    r.world(view=view, match=False)
    check(r.frame() == ([], [], []) and r.opened == [], "a world without a match: nothing read, asked or played")
    big = Run(marked)
    big.world(view=view)
    big.files = {MARKS: HEADER + b"x" * (256 * 1024)}
    asked, played, removed = big.frame()
    check(asked == played == [] and big.files[LOG].endswith(
        b"srp: spawn_marks: srp_spawn_marks.tsv is over 256 KiB: none of its marks shown (begin a new file)\r\n"),
        "a file past 256 KiB: none of its marks shown, and the log says so")


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code, late_join = open(base.default_bin("section"), "rb").read(), open(base.late_join_bin(), "rb").read()
    default, _ = base.build(original, "section", code, None, late_join)
    plain = base.arena()
    base.linker.RUST_CFGS = ("srp_spawn_marks",)
    marked, _ = base.build(original, "section", code, None, late_join)
    tooled = base.arena()
    base.linker.RUST_CFGS = ()
    old, new, tool = Run(original), Run(bytes(default)), Run(bytes(marked))
    view = transform(math.radians(135.2), math.radians(-8.6), (-51.9, 7.114, 12.914))

    print("the default build: none of the tool")
    of_tool = lambda linked: sorted(name for name, _, _, _ in linked.every if "spawn_marks" in name or "free_fly" in name)
    shims = ["hook_spawn_marks_on_frame", "hook_spawn_marks_on_key", "hook_spawn_marks_on_loaded"]
    check(of_tool(plain) == sorted(shims + ["spawn_marks_on_loaded"]) and len(of_tool(tooled)) > 4,
          f"the lock's arena has of the tool only its three handlers' shims and the generated entry of its "
          f"callback ({', '.join(of_tool(plain))}); --spawn-marks's has {len(of_tool(tooled))} symbols")
    for name in shims:
        shim = plain.symbols[name]
        body = plain.image[shim - plain.va:shim - plain.va + 3]
        check(body == bytes.fromhex("31c0c3"), f"{name} is `xor eax,eax / ret`: the chain goes on, nothing done")
    for key in (F7, F8, F9, F10):
        for kind, name in ((DOWN, "down"), (HELD, "held"), (UP, "up")):
            for r in (old, new):
                r.world(view=view)
            before = new.snapshot()
            want = old.press(key, kind)
            got = new.press(key, kind)
            check(got == want and got[1:] == ([], [], []) and got[0][0] == tab.KEY_PAST
                  and new.snapshot() == before and new.files == {},
                  f"the default build, key {key:#x} {name}: registers and stack the original's past the span, "
                  "no file, no message, no camera switched, no byte of the world changed")

    new.world(view=view)
    new.files = {MARKS: HEADER + line_of(1, MAP, "spawn", "player", (1.0, 2.0, 3.0), (4.0, 5.0, 6.0))}
    asked, played, removed = new.frame()
    check(asked == played == removed == [] and not any(name == MARKS for name, _, _ in new.opened),
          "the default build's frame after the move: no file read, nothing asked, played or taken out")

    print("the --spawn-marks build: marks")
    old.world(view=view)
    tool.world(view=view)
    want = old.press(F8)
    got, opened, messages, calls = tool.press(F8)
    lines = tool.files.get(MARKS, b"").split(b"\n")
    check(taken(got) and want[0][0] == tab.KEY_PAST and calls == [],
          "F8: the key taken (al 1, ret 0Ch, ebx esi edi ebp kept; the original goes on), no camera switched")
    check(tool.files.get(MARKS, b"").startswith(HEADER) and len(lines) == 3 and lines[2] == b""
          and mark_is(lines[1] + b"\n", 1, "spawn", "player", view),
          f"F8: the header, then {lines[1].decode()!r}")
    check((MARKS, 4, 4) in opened, "the file opened to append (FILE_APPEND_DATA, OPEN_ALWAYS)")
    check(messages == [("mark 1: spawn here", 0x21, 0x14, 2000)]
          and tool.files[LOG].endswith(b"srp: spawn_marks: mark 1: spawn here\r\n"),
          "the hud's centre message and srp_patches.log: mark 1: spawn here")
    far = transform(math.radians(300.0), math.radians(40.0), (120.5, -3.25, -77.125))
    tool.emu.mem_write(DIRECTOR + 4, far)
    got, opened, messages, calls = tool.press(F9)
    lines = tool.files[MARKS].split(b"\n")
    check(tool.files[MARKS].count(HEADER) == 1 and len(lines) == 4
          and mark_is(lines[2] + b"\n", 2, "no_spawn", "player", far)
          and messages == [("mark 2: NO spawn near here", 0x21, 0x14, 2000)],
          f"F9: no second header, then {lines[2].decode()!r}; the message mark 2: NO spawn near here")
    size = len(tool.files[MARKS])
    for kind, name in ((HELD, "held"), (UP, "up")):
        want = old.press(F8, kind)
        got, opened, messages, calls = tool.press(F8, kind)
        check(len(tool.files[MARKS]) == size and messages == [] and opened == [] and got == want[0],
              f"F8 {name}: no mark, the key goes on as in the original")
    tool.refused_files = (MARKS,)
    got, opened, messages, calls = tool.press(F8)
    check(len(tool.files[MARKS]) == size
          and messages == [("NO MARK: srp_spawn_marks.tsv could not be written", 0x21, 0x14, 2000)],
          "the file refused: no line, and the message says so")
    tool.refused_files = ()
    tool.press(F8)
    check(mark_is(tool.files[MARKS].split(b"\n")[3] + b"\n", 3, "spawn", "player", far),
          "the next mark is 3: the refused one took no number")
    kept = tool.files
    tool.world(view=view, camera=None)
    tool.files = kept
    size = len(tool.files[MARKS])
    got, opened, messages, calls = tool.press(F8)
    check(len(tool.files[MARKS]) == size and messages == [("NO MARK: no camera is on", 0x21, 0x14, 2000)],
          "no camera on: no mark, and the message says so")
    tool.world(view=view, bound=False, camera="free", client=REPLAY_CLIENT, hud_movie=False)
    tool.files = kept
    got, opened, messages, calls = tool.press(F9)
    line = tool.files[MARKS].split(b"\n")[4] + b"\n"
    check(mark_is(line, 4, "no_spawn", "free", view, feet=None) and messages == [],
          f"a replay's free view, no hud movie: {line.decode()!r} (no player: dashes), srp_patches.log alone told")

    print("the --spawn-marks build: F7")
    tool.world(view=view)
    tool.files = kept
    got, opened, messages, calls = tool.press(F7)
    free_view = struct.unpack("<16f", tool.read(FREE + 0x44, 64))
    v = struct.unpack("<16f", view)
    right = (v[10], 0.0, -v[8])                                # up x forward, up the world's
    check(tool.read(PLAYER_HANDLER + 0x358) == 1 and tool.read(DIRECTOR + 0x84) == FREE
          and calls == [("remove", PLAYER_HANDLER), ("add", FREE + 0x94)],
          "F7 on the player's camera: the handler's mode 1, the director's camera the free-fly one, by the game's "
          "own switch: the player's handler out of the input world, the free-fly camera's in")
    check(close(free_view[12:15], v[12:15], 1e-6) and close(free_view[8:11], v[8:11], 1e-6)
          and close(free_view[4:7], (0.0, 1.0, 0.0), 1e-6) and close(free_view[0:3], right, 1e-6),
          "the free-fly camera starts at the screen's view: its place and forward, its up the world's")
    check(messages == [("free camera (F7: back to the player)", 0x21, 0x14, 2000)] and taken(got),
          "the message says so; the key taken, so the input world asks no handler after the change")
    got, opened, messages, calls = tool.press(F8)
    line = tool.files[MARKS].split(b"\n")[5] + b"\n"
    check(mark_is(line, 5, "spawn", "free", view), f"a mark from the free-fly camera: {line.decode()!r}")
    got, opened, messages, calls = tool.press(F7)
    check(tool.read(PLAYER_HANDLER + 0x358) == 0 and tool.read(DIRECTOR + 0x84) == PLAYER_HANDLER + 4
          and calls == [("remove", FREE + 0x94), ("add", PLAYER_HANDLER)]
          and messages == [("player camera", 0x21, 0x14, 2000)] and taken(got),
          "F7 again: the game's switch_to_player_camera: mode 0 (first person), the director's camera the "
          "handler's, the handlers the other way round")
    for name, args, text in (
            ("no player bound (the free-fly camera the game's own)", dict(bound=False, camera="free"),
             "no player's camera to leave or come back to yet"),
            ("a replay following a player", dict(client=REPLAY_CLIENT),
             "a replay: F2 is its free view, F1 follows a player")):
        tool.world(view=view, **args)
        tool.files = kept
        before = tool.snapshot()
        got, opened, messages, calls = tool.press(F7)
        check(calls == [] and tool.snapshot() == before and messages == [(text, 0x21, 0x14, 2000)],
              f"F7, {name}: nothing switched; the message: {text}")

    print("the --spawn-marks build: the markers")
    markers(bytes(marked), view)

    print("the free-fly camera, as the game's own code steers it")
    tool.world(view=view)
    tool.press(F7)
    step = 0.06 * 16 * 0.1                                     # a 16 ms tick's move, m
    i, j, k = v[0:3], v[4:7], v[8:11]                          # the director's view: right, up, forward
    for name, feed, axis, factor in (
            ("the left mouse button: forward", dict(buttons=(LEFT_BUTTON,)), k, 1.0),
            ("the right mouse button: back", dict(buttons=(RIGHT_BUTTON,)), k, -1.0),
            ("D: to the right", dict(keys=(D,)), i, 1.0),
            ("A: to the left", dict(keys=(A,)), i, -1.0),
            ("W: up (the view's up)", dict(keys=(W,)), j, 1.0),
            ("S: down", dict(keys=(S,)), j, -1.0),
            ("Ctrl and the left button: 20 times as far", dict(keys=(LCTRL,), buttons=(LEFT_BUTTON,)), k, 20.0),
            ("Shift and the left button: a tenth", dict(keys=(LSHIFT,), buttons=(LEFT_BUTTON,)), k, 0.1)):
        tool.emu.mem_write(DIRECTOR + 4, view)
        tool.emu.mem_write(FREE + 0xE8, struct.pack("<f", -1.0))      # the first tick after its activation
        tool.word(FREE + 0xE4, tool.read(GAME + 0x584C))
        before, after = tool.tick(**feed)
        moved = [a - b for a, b in zip(after[12:15], before[12:15])]
        want_move = [factor * step * x for x in axis]
        check(close(moved, want_move, 2e-4 * max(1.0, abs(factor))) and close(after[8:11], before[8:11], 1e-4),
              f"{name}: moved {math.sqrt(sum(x * x for x in moved)):.3f} m in a 16 ms tick, the look kept")
    tool.emu.mem_write(DIRECTOR + 4, view)
    before, after = tool.tick(mouse=(40, 0, 0))
    turned = math.degrees(math.acos(max(-1.0, min(1.0, sum(a * b for a, b in zip(after[8:11], before[8:11]))))))
    flat = math.degrees(math.atan2(after[8], after[10]) - math.atan2(before[8], before[10]))
    check(close(after[12:15], before[12:15], 1e-4) and abs(abs((flat + 180) % 360 - 180) - 40 * 0.375) < 0.05
          and abs(after[9] - before[9]) < 1e-4,
          f"a mouse move of 40 counts sideways: the heading turned {abs((flat + 180) % 360 - 180):.2f} degrees "
          f"(0.375 a count; {turned:.2f} along the look), the place and the pitch kept")
    tool.emu.mem_write(DIRECTOR + 4, view)
    kept_view = tool.read(FREE + 0x44, 64)
    tool.tick()
    check(tool.read(FREE + 0x44, 64) == kept_view, "no key, button or move: the tick leaves the view as it is")
    check(tool.read(FREE + 0xEC) == tool.read(FREE + 0xF0) and tool.read(FREE + 0xF8) == tool.read(FREE + 0xFC),
          "each tick takes the keys and buttons it was fed")


if __name__ == "__main__":
    main()
