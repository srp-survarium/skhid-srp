#!/usr/bin/env python3
"""Patch 133 (the Rust patch console_where, rust/patches/src/console_where.rs):
the console command `where` (client_patch.md section 3.80).

    python check_console_where_026g.py <original survarium.exe>

Requires Python `unicorn`. check_language_026g.py's harness (the exe's
sections and the arena loaded as they are), the original beside the build.
Stubbed: kernel32's GetTickCount (the case's clock), CreateFileA / WriteFile /
CloseHandle (srp_patches.log's bytes kept), has_passed_filters (0x111860: the
case's answer, its arguments kept) and the log helper's write (0x111D10: its
helper, format and va_list kept, `ret 8`), which is where every line of the
game's log goes from its helper to the console and Survarium_<user>.log.
The game's own code runs the rest: on_configs_loaded's start, update_camera
(0x1F3480, from its start to its call of 0x243440, with patch 46's
death_view and patch 76's eye in the build), the console's execute
(0xEB980) with its lookup, the copy of the delegate (0x28F040) and the
log helper's constructor (0x111C90). Checked:

- on_configs_loaded's start (0x1EA210 to 0x1EA216): `where` registered once
  at the console list's head, a cc_delegate of 60h bytes: kind 2, status 0,
  +18h 1, an empty delegate, its vtable our copy of cc_delegate's 0x9E7C0C
  (RTTI and every slot the game's but execute, +4, ours in patches.code);
  the original has no `where`;
- a line `where` before any view, the engine world not running: "where: map - no
  camera yet", to the game's log
  (format "%s", channel "game:", level 4, the template 0x21E73B8, the
  function {0x9DAF4C | 1, 0, the sink} as the game's callers make it, the
  context word) and to srp_patches.log; the helper our call fills is,
  field by field, the one the game's own log_message (0x111ED0) fills for
  the same line; no sink: an empty function; the log filtered: written only
  when has_passed_filters("game", 4) says so; the original logs "unknown
  command [%s]" instead;
- update_camera in a spectator's first person (mode 4) and in the third
  person (mode 2): the handler's view (+48h) the original's bit for bit, and
  then `where` (and `where with arguments`) prints the player's place
  (+40Ch's), the view's place when it is not that, the view's forward as a
  unit vector, its yaw (degrees from +Z towards +X) and pitch, each to its
  decimals; ten seconds later the same and "(10.0 s ago)";
- the map first: with the engine world's game (0x1F1D310, +2D0h) in a match
  (its world's +4D4Ch set), the game's map name (+55E0h, the buffer_string
  query_players assigns) and the maps_dict id of the client's own options
  (match_options, vtable +34h, the game's own getter: a live network client's
  [+1F1C48h]+0Ch, a replay client's +1Ch): `map <name> (id <n>)`; the world
  without a match (the lobby after one, its name still there) and the engine
  not running: `map -`.
Says nothing about the real console's window or the sink's file.
"""
import math
import struct
import sys

from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_EIP

import patch_026g as base
import check_hide_ui_026g as hide
import check_language_026g as language

EXECUTE = 0xEB980                       # console_commands::execute (cdecl line, filter, mask)
UPDATE_CAMERA, CAMERA_END = 0x1F3480, 0x1F365C     # stdcall (player); `push eax` before the call of 0x243440
LOG_MESSAGE, LOG_WRITE, LOG_PASSES = 0x111ED0, 0x111D10, 0x111860
LOG_FILTERED, LOG_CONTEXT, LOG_SINK = 0x1F1D60C, 0x1F1D610, 0x1F1D614
LOG_TEMPLATE, LOG_MANAGER = 0x21E73B8, 0x9DAF4C
CC_DELEGATE_VTABLE, VTABLE_SLOTS = 0x9E7C0C, 7
TICK_SLOT, CREATE_SLOT, WRITE_SLOT, CLOSE_SLOT = 0x870090, 0x870048, 0x870108, 0x870040
OBJECTS = 0x40000000
PLAYER, HANDLER, WORLD, PRESENTERS, TEXT, STUBS = (OBJECTS + 0x10000 * i for i in range(6))
SINK = 0x5EED0000                       # a made-up sink: never called (the write is stubbed)
CONTEXT = 0x0C0FFEE0
ENGINE_SLOT = 0x1F1D310                 # the engine world's global (0: not running)
NETWORK_CLIENT_VTABLE, REPLAY_CLIENT_VTABLE = 0x9FFB14, 0x9FF344
MAPS = 0x50000000                       # the engine world, the game, its options' holder and a client
ENGINE, GAME, HOLDER, CLIENT = MAPS, MAPS + 0x10000, MAPS + 0x100000, MAPS + 0x200000


class Run(language.Run):
    def __init__(self, data):
        super().__init__(data)
        emu = self.emu
        emu.mem_map(OBJECTS, 0x60000, UC_PROT_ALL)
        emu.mem_map(MAPS, 0x400000, UC_PROT_ALL)
        self.now, self.writes, self.logged, self.passes, self.passing = 0, [], b"", [], True
        stubs = ((TICK_SLOT, b"\xC3"),
                 (CREATE_SLOT, b"\xB8\x07\x00\x00\x00\xC2\x1C\x00"),
                 (WRITE_SLOT, b"\xB8\x01\x00\x00\x00\xC2\x14\x00"),
                 (CLOSE_SLOT, b"\xB8\x01\x00\x00\x00\xC2\x04\x00"))
        for i, (slot, code) in enumerate(stubs):
            self.word(slot, STUBS + 0x40 * i)
            emu.mem_write(STUBS + 0x40 * i, code)
        emu.hook_add(UC_HOOK_CODE, self.tick, begin=STUBS, end=STUBS)
        emu.hook_add(UC_HOOK_CODE, self.write_file, begin=STUBS + 0x80, end=STUBS + 0x80)
        emu.hook_add(UC_HOOK_CODE, self.log_write, begin=LOG_WRITE, end=LOG_WRITE)
        emu.hook_add(UC_HOOK_CODE, self.log_passes, begin=LOG_PASSES, end=LOG_PASSES)

    def tick(self, emu, *_):
        emu.reg_write(UC_X86_REG_EAX, self.now)

    def write_file(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.logged += bytes(emu.mem_read(self.read(sp + 8), self.read(sp + 12)))

    def ret(self, pops, eax=None):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        if eax is not None:
            self.emu.reg_write(UC_X86_REG_EAX, eax)
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def log_write(self, emu, *_):
        """helper::write (ESI the helper; format, va_list; ret 8): the line kept."""
        sp = emu.reg_read(UC_X86_REG_ESP)
        helper, form, va = emu.reg_read(UC_X86_REG_ESI), self.read(sp + 4), self.read(sp + 8)
        fields = struct.unpack("<8I", emu.mem_read(helper + 0x228, 32))
        function = bytes(emu.mem_read(fields[0], 0x20))
        self.writes.append(dict(
            format=self.text(form), argument=self.long_text(self.read(va)), function=function,
            template=fields[1], context=fields[2], channel=self.text(fields[3]), file=self.text(fields[4]),
            caller=self.text(fields[5]), line=fields[6], level=fields[7],
            head=bytes(emu.mem_read(helper, 0x228))))
        self.ret(8)

    def log_passes(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.passes.append((self.text(self.read(sp + 4)), self.read(sp + 8)))
        self.ret(0, 1 if self.passing else 0)

    def long_text(self, at):
        return bytes(self.emu.mem_read(at, 256)).split(b"\0")[0]

    def commands(self):
        out, node = {}, self.read(language.LIST_HEAD)
        while node:
            out.setdefault(self.text(self.read(node + 0x10)).decode(), node)
            node = self.read(node + 0x0C)
        return out

    def call(self, start, args, regs=None, pops=0):
        sp = hide.STACK + 0x8000
        self.word(sp, hide.SENTINEL)
        for i, value in enumerate(args):
            self.word(sp + 4 + 4 * i, value)
        _, end = self.run(start, (hide.SENTINEL,), sp, {**hide.GAME, **(regs or {})})
        assert self.stopped == hide.SENTINEL and end == sp + 4 + pops, (hex(start), self.stopped, end - sp)

    def execute(self, line):
        """console_commands::execute(line, 2, 2), as the console's on_text_commit: the lines written."""
        self.emu.mem_write(TEXT, line.encode() + b"\0")
        self.writes, self.logged = [], b""
        self.call(EXECUTE, (TEXT, 2, 2))
        return self.writes, self.logged

    def game_log(self, text):
        """The game's own log_message of `text` with "%s" as our line's: its helper."""
        self.emu.mem_write(TEXT, b"%s\0srp\0game\0" + text + b"\0")
        function = TEXT + 0x100
        self.emu.mem_write(function, bytes(0x20))
        if self.read(LOG_SINK):
            self.word(function, LOG_MANAGER | 1)
            self.word(function + 8, self.read(LOG_SINK))
        self.writes = []
        self.call(LOG_MESSAGE, (function, self.read(LOG_CONTEXT), LOG_TEMPLATE, TEXT + 3, 0, TEXT + 3, TEXT + 7, 4,
                                TEXT, TEXT + 12))
        return self.writes

    def map(self, name=None, map_id=0, replay=False, playing=True):
        """The engine world's game in a match of map `name` (None: the engine not running)."""
        self.emu.mem_write(MAPS, bytes(0x400000))
        if name is None:
            self.word(ENGINE_SLOT, 0)
            return
        self.word(ENGINE_SLOT, ENGINE)
        self.word(ENGINE + 0x2D0, GAME)
        self.word(GAME + 0x18 + 0x4D4C, 0x5A5A0000 if playing else 0)    # the world's match
        text = GAME + 0x55EC
        self.emu.mem_write(text, name.encode() + b"\0")
        self.word(GAME + 0x55E0, text)
        self.word(GAME + 0x55E4, text + len(name))
        self.word(GAME + 0x55E8, text + 0x200)
        self.word(GAME + 0x53AC, CLIENT)
        if replay:
            self.word(CLIENT, REPLAY_CLIENT_VTABLE)
            self.emu.mem_write(CLIENT + 0x1C + 0x577B4, bytes([map_id]))
        else:
            self.word(CLIENT, NETWORK_CLIENT_VTABLE)
            self.word(CLIENT + 0x1F1C48, HOLDER)
            self.emu.mem_write(HOLDER + 0xC + 0x577B4, bytes([map_id]))

    def camera(self, mode, body, eye, warmup=(0.0, 0.0, 0.0)):
        """update_camera(player) with the camera's handler in `mode`: the view it leaves (+48h)."""
        self.emu.mem_write(OBJECTS, bytes(0x40000))
        self.word(PLAYER + 0xF470, HANDLER)
        self.word(PLAYER + 0xF478, PRESENTERS)
        self.emu.mem_write(PLAYER + 0x33C, b"\x01")           # alive
        self.emu.mem_write(PLAYER + 0x40C, body)
        self.emu.mem_write(PLAYER + 0x44C, eye)
        self.word(HANDLER + 0x358, mode)
        self.word(HANDLER + 0x334, WORLD)
        self.emu.mem_write(HANDLER + 0x34C, struct.pack("<3f", *warmup))
        self.emu.mem_write(HANDLER + 0x48, b"\xEE" * 64)
        sp = hide.STACK + 0x8000
        self.word(sp, hide.SENTINEL)
        self.word(sp + 4, PLAYER)
        _, end = self.run(UPDATE_CAMERA, (CAMERA_END,), sp, {**hide.GAME, UC_X86_REG_EBX: 0x0B0B0B0B})
        assert self.stopped == CAMERA_END, self.stopped
        return bytes(self.emu.mem_read(HANDLER + 0x48, 64))


def transform(yaw, pitch, place):
    """A row-vector 4x4 looking along (sin yaw cos pitch, sin pitch, cos yaw cos pitch): its rows
    right, up, forward, then the place."""
    f = (math.sin(yaw) * math.cos(pitch), math.sin(pitch), math.cos(yaw) * math.cos(pitch))
    r = (math.cos(yaw), 0.0, -math.sin(yaw))
    u = (f[1] * r[2] - f[2] * r[1], f[2] * r[0] - f[0] * r[2], f[0] * r[1] - f[1] * r[0])
    return struct.pack("<16f", *r, 0, *u, 0, *f, 0, *place, 1)


def expected(body, view):
    """The numbers the line must give: pos, eye (or None), dir, yaw, pitch."""
    v = struct.unpack("<16f", view)
    eye = v[12:15]
    f = v[8:11]
    n = math.sqrt(sum(c * c for c in f))
    d = [c / n for c in f]
    yaw = math.degrees(math.atan2(d[0], d[2])) % 360
    pitch = math.degrees(math.atan2(d[1], math.hypot(d[0], d[2])))
    shown_eye = None if all(abs(a - b) < 0.0005 for a, b in zip(body, eye)) else eye
    return body, shown_eye, d, yaw, pitch


def parse(text):
    """`where: pos x y z [eye x y z] dir x y z yaw a pitch b [(s s ago)]` as numbers."""
    words = text.split()
    assert words[0] == "where:" and words[1] == "map", text
    i = 5 if words[3] == "(id" else 3
    out = {"map": " ".join(words[2:i])}
    assert words[i] == "pos", text
    out["pos"], i = [float(w) for w in words[i + 1:i + 4]], i + 4
    if words[i] == "eye":
        out["eye"], i = [float(w) for w in words[i + 1:i + 4]], i + 4
    assert words[i] == "dir" and words[i + 4] == "yaw" and words[i + 6] == "pitch", text
    out["dir"] = [float(w) for w in words[i + 1:i + 4]]
    out["yaw"], out["pitch"] = float(words[i + 5]), float(words[i + 7])
    out["rest"] = " ".join(words[i + 8:])
    for key, decimals in (("pos", 3), ("eye", 3), ("dir", 3)):
        assert key not in out or all(len(w.split(".")[1]) == decimals for w in text.split(key + " ")[1].split()[:3]), text
    return out


def close(got, want, within):
    return all(abs(a - b) <= within for a, b in zip(got, want))


def check(ok, text):
    """A case: its label printed, or the run stopped."""
    print("ok", text) if ok else sys.exit("FAIL " + text)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    built, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None,
                          open(base.late_join_bin(), "rb").read())
    old, new = Run(original), Run(bytes(built))

    # on_configs_loaded's start
    old.start()
    new.start()
    ours = new.commands().get("where")
    check("where" not in old.commands() and ours == new.read(language.LIST_HEAD),
           "configs.start: `where` at the head of the console's list; the original has none")
    vtable = new.read(ours)
    slots = [new.read(vtable - 4 + 4 * i) for i in range(VTABLE_SLOTS + 1)]
    stock = [new.read(CC_DELEGATE_VTABLE - 4 + 4 * i) for i in range(VTABLE_SLOTS + 1)]
    arena = base.ARENA_VA if hasattr(base, "ARENA_VA") else 0xA42D000
    check(slots[:2] + slots[3:] == stock[:2] + stock[3:] and slots[2] != stock[2] and slots[2] >= arena
           and vtable != CC_DELEGATE_VTABLE,
           "its vtable: cc_delegate's RTTI and slots, execute (+4) ours in the arena")
    check(new.read(ours + 0x14) == 2 and new.read(ours + 0x18) == 1 and new.read(ours + 0x1C, 2) == b"\x00\x01"
           and new.read(ours + 0x40) == 0 and new.read(ours + 0x20) == 0,
           "a cc_delegate of kind 2 (the console's mask), +18h 1, status 0, saved byte 1, an empty delegate and "
           "no on-executed one")
    head = new.read(language.LIST_HEAD)
    new.start()
    check(new.read(language.LIST_HEAD) == head, "configs.start again: registered once")

    # before any view; the log's function and filter
    for r in (old, new):
        r.word(LOG_SINK, SINK)
        r.word(LOG_CONTEXT, CONTEXT)
    writes, logged = new.execute("where")
    want_function = struct.pack("<3I", LOG_MANAGER | 1, 0, SINK) + bytes(0x14)
    check(len(writes) == 1 and writes[0]["argument"] == b"where: map - no camera yet" and writes[0]["format"] == b"%s"
           and writes[0]["channel"] == b"game:" and writes[0]["level"] == 4 and writes[0]["template"] == LOG_TEMPLATE
           and writes[0]["context"] == CONTEXT and writes[0]["function"] == want_function
           and logged == b"srp: where: map - no camera yet\r\n",
           "where through console_commands::execute, before any view: \"where: map - no camera yet\" to the game's log "
           "(\"%s\", game:, info, the template, the sink's function, the context) and to srp_patches.log")
    game = new.game_log(b"where: map - no camera yet")
    same = len(game) == 1 and all(game[0][k] == writes[0][k] for k in
                                  ("format", "argument", "function", "template", "context", "channel", "file",
                                   "caller", "line", "level", "head"))
    check(same, "the game's own log_message of that line fills its helper as ours, field by field")
    for r in (old, new):
        r.word(LOG_SINK, 0)
    writes, _ = new.execute("where")
    check(len(writes) == 1 and writes[0]["function"] == bytes(0x20), "no sink: an empty function, as the game's")
    new.word(LOG_FILTERED, 1)
    for passing in (False, True):
        new.passing, new.passes = passing, []
        writes, logged = new.execute("where")
        check(new.passes == [(b"game", 4)] and len(writes) == int(passing) and logged.startswith(b"srp: where:"),
               f"the log filtered: has_passed_filters(\"game\", 4) asked; {'written' if passing else 'not written'} "
               "(srp_patches.log either way)")
    new.word(LOG_FILTERED, 0)
    writes, _ = old.execute("where")
    check(len(writes) == 1 and writes[0]["format"] == b"unknown command [%s]",
           "the original: \"unknown command [where]\"")

    # update_camera, then where
    body = (-51.899, 5.414, 12.914)
    cases = (("a spectator's first person (mode 4)", 4, transform(math.radians(135.2), math.radians(-8.6),
                                                                   (-51.9, 7.114, 12.914)), (0.0, 0.0, 0.0)),
             ("the third person (mode 2)", 2, transform(math.radians(20.0), math.radians(5.0), (-51.9, 7.0, 12.9)),
              (0.4, -0.25, 2.5)))
    for name, mode, eye, warmup in cases:
        body_transform = transform(math.radians(30.0), 0.0, body)
        new.now = 100_000
        o, n = old.camera(mode, body_transform, eye, warmup), new.camera(mode, body_transform, eye, warmup)
        check(o == n and n != b"\xEE" * 64, f"{name}: update_camera leaves the original's view bit for bit")
        pos, shown_eye, d, yaw, pitch = expected(struct.unpack("<16f", body_transform)[12:15], n)
        for line in ("where", "where with arguments"):
            new.now = 100_400
            writes, logged = new.execute(line)
            text = writes[0]["argument"].decode()
            got = parse(text)
            ok = (close(got["pos"], pos, 0.0006) and (shown_eye is None) == ("eye" not in got)
                  and (shown_eye is None or close(got["eye"], shown_eye, 0.0006)) and close(got["dir"], d, 0.0011)
                  and abs((got["yaw"] - yaw + 180) % 360 - 180) <= 0.06 and abs(got["pitch"] - pitch) <= 0.06
                  and got["map"] == "-" and got["rest"] == "" and logged == b"srp: " + text.encode() + b"\r\n")
            check(ok, f"{name}, {line!r}: {text}")
        new.now = 110_000
        writes, _ = new.execute("where")
        check(writes[0]["argument"].decode().endswith(" (10.0 s ago)"), f"{name}, 10 s later: (10.0 s ago)")

    # the map first
    new.now = 110_400
    tail = new.execute("where")[0][0]["argument"].decode().split(" pos ", 1)[1]
    for name, args, want in (
            ("a live match", dict(name="v021d_level_03/day_2", map_id=61), "v021d_level_03/day_2 (id 61)"),
            ("a replay", dict(name="level_01/day_2", map_id=117, replay=True), "level_01/day_2 (id 117)"),
            ("the world with no match (the lobby after one)",
             dict(name="level_01/day_2", map_id=117, playing=False), "-"),
            ("the engine not running", dict(), "-")):
        new.map(**args)
        writes, logged = new.execute("where")
        text = writes[0]["argument"].decode()
        check(text == f"where: map {want} pos {tail}" and logged == b"srp: " + text.encode() + b"\r\n",
              f"the map first, {name}: {text}")


if __name__ == "__main__":
    main()
