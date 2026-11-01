#!/usr/bin/env python3
"""Run patch 89 (rust/patches/src/pose_marker.rs, its three lohooks of
pose_marker.toml: the local build's pose marker, ../docs/notes/pose-marker.md)
on made-up game objects, in the default build and in the diagnostics build
(patch_026g.py --diagnostics), each hook from its site to its span's end.

    python check_pose_marker_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them; check_map_names_026g.py's harness for the game world's keys
(game_world::on_keyboard_action 0x25CD00 to past its show_player_list,
0x25CD39, the game's own keyboard code). The game's own code runs the rest:
the config lookups (cfg_has 0xF6C00, cfg_child 0xF6B10, on a mode config made
here, its records sorted by CRC as the lookup's binary search needs) and the
network client's match_options (0x219990). Stubs: kernel32's CreateFileA,
WriteFile, GetFileSize, CloseHandle (files kept in a dict by path),
GetLocalTime (a fixed time), GetCurrentThreadId (one thread), and the user
data folder (0x18BD0). Checked:

- the three sites in place: the original's bytes in the original, a jmp to
  patches.code in the build; every hook returns to its span's end with the
  registers and ESP the original's code leaves (level_loading.map 0x220482:
  mov edx,[eax+8] / add eax,8; camera.eye 0x1F3643: mov ecx,[ebx+0F470h]);
- the default build: the comma key down and the next eye write nothing
  (no file opened), the level is not kept;
- the diagnostics build: the level's map, mode and project kept at its
  loading; the comma key down, then the next eye: srp_poses.tsv made, its
  header and one row (the time's name and stamp, the match's map from its
  options, the level's project and mode, the eye's place, the yaw and pitch
  of its forward row, uro_fov), and a line in srp_patches.log; a second eye
  writes nothing; the key held or let go, another key, write nothing; a
  second press adds one row and no second header; a level of another map
  leaves the project and mode out (`-`); the key never taken (the game's
  run of on_keyboard_action is the original's).
"""
import math
import struct
import sys
import zlib

from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESP,
                               UC_X86_REG_EIP)

import patch_026g as base
import check_hide_ui_026g as hide
import check_map_names_026g as keys

LEVEL_SITE, LEVEL_NEXT = 0x220482, 0x220488
EYE_SITE, EYE_NEXT = 0x1F3643, 0x1F3649
USER_DATA = 0x18BD0
CREATE_FILE, WRITE_FILE, CLOSE, FILE_SIZE, LOCAL_TIME = 0x870048, 0x870108, 0x870040, 0x870044, 0x870184
URO_FOV = 0xA6FA28
COMMA, W, DOWN, UP, HOLD = 0x33, 0x11, 1, 2, 3

HEAP = 0x50000000                       # 4 MiB: the client (+1F1C48h) and the rest
CLIENT, HOLDER = HEAP, HEAP + 0x200000  # the client's options holder; its options at +0Ch
OPTIONS = HOLDER + 0xC
PLAYER, WORLD, GAME = HEAP + 0x280000, HEAP + 0x2A0000, HEAP + 0x2A1000
DESCRIPTOR, CONFIG, NODES, TEXTS = HEAP + 0x2B0000, HEAP + 0x2B1000, HEAP + 0x2B2000, HEAP + 0x2B8000
STUBS, FOLDER = HEAP + 0x2C0000, HEAP + 0x2C1000
FOLDER_TEXT = b"C:\\users\\me\\Documents\\survarium"
POSES = FOLDER_TEXT.decode() + "\\srp_poses.tsv"
HEADER = "name\ttime\tmap\tproject\tmode\tx\ty\tz\tyaw\tpitch\tvfov"
# GetLocalTime's SYSTEMTIME: 2026-11-01 (a Sunday) 09:05:07.042
TIME = (2026, 11, 0, 1, 9, 5, 7, 42)


def eye(yaw, pitch, at):
    """A view's transform (rows right, up, forward, place) looking along yaw and pitch (degrees)."""
    y, p = math.radians(yaw), math.radians(pitch)
    forward = (math.sin(y) * math.cos(p), math.sin(p), math.cos(y) * math.cos(p))
    right = (math.cos(y), 0.0, -math.sin(y))
    up = (forward[1] * right[2] - forward[2] * right[1], forward[2] * right[0] - forward[0] * right[2],
          forward[0] * right[1] - forward[1] * right[0])
    return list(right) + [0.0] + list(up) + [0.0] + list(forward) + [0.0] + list(at) + [1.0]


class Run(keys.Run):
    def __init__(self, data):
        super().__init__(data)
        emu = self.emu
        emu.mem_map(HEAP, 0x400000, UC_PROT_ALL)
        self.files, self.handles = {}, {}
        self.stubs = {}
        for i, (slot, handler) in enumerate(((CREATE_FILE, self.create_file), (WRITE_FILE, self.write_file),
                                             (CLOSE, self.close), (FILE_SIZE, self.file_size),
                                             (LOCAL_TIME, self.local_time))):
            stub = STUBS + 0x10 * i
            emu.mem_write(stub, b"\xC3")
            self.word(slot, stub)
            self.stubs[stub] = handler
            emu.hook_add(UC_HOOK_CODE, self.import_stub, begin=stub, end=stub)
        emu.mem_write(FOLDER, FOLDER_TEXT + b"\0")
        emu.hook_add(UC_HOOK_CODE, self.user_data, begin=USER_DATA, end=USER_DATA)
        # the camera's player: its game world, the world's game, the game's network client, its options
        self.word(PLAYER + 0xF474, WORLD)
        self.word(WORLD + 0x9C, GAME)
        self.word(GAME + 0x53AC, CLIENT)
        self.word(CLIENT + 0x1F1C48, HOLDER)
        self.match_map(204)
        self.texts = TEXTS

    # ---- kernel32 ------------------------------------------------------------------------
    def import_stub(self, emu, address, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 * i) for i in range(1, 8)]
        result, pops = self.stubs[address](args)
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + 4 * pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def create_file(self, args):
        path = bytes(self.emu.mem_read(args[0], 300)).split(b"\0")[0].decode()
        self.files.setdefault(path, b"")
        handle = 0x100 + len(self.handles) * 4
        self.handles[handle] = path
        return handle, 7

    def write_file(self, args):
        handle, at, size, out = args[:4]
        self.files[self.handles[handle]] += bytes(self.emu.mem_read(at, size))
        self.word(out, size)
        return 1, 5

    def close(self, args):
        self.handles.pop(args[0])
        return 1, 1

    def file_size(self, args):
        if args[1]:
            self.word(args[1], 0)
        return len(self.files[self.handles[args[0]]]), 2

    def local_time(self, args):
        self.emu.mem_write(args[0], struct.pack("<8H", *TIME))
        return 0, 1

    def user_data(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        emu.reg_write(UC_X86_REG_EAX, FOLDER)
        emu.reg_write(UC_X86_REG_ESP, sp + 4)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    # ---- the game's objects -----------------------------------------------------------------
    def match_map(self, map_id):
        self.emu.mem_write(OPTIONS + 0x577B4, bytes([map_id]))

    def put_text(self, text):
        at = self.texts
        self.emu.mem_write(at, text.encode() + b"\0")
        self.texts += (len(text) + 16) & ~15
        return at

    def records(self, entries, at):
        """A table's children at `at` (records of 18h bytes: data, name, CRC, type, count; sorted by CRC,
        as cfg_child's binary search needs), {name: int, str or dict}, a table's own children after them:
        the next free byte."""
        kids = sorted((zlib.crc32(k.encode()), k, v) for k, v in entries.items())
        free = at + 0x18 * len(kids)
        for i, (crc, key, value) in enumerate(kids):
            if isinstance(value, dict):
                first = free
                free = self.records(value, first)
                record = struct.pack("<QQIHH", first if value else 0, self.put_text(key), crc, 3, len(value))
            elif isinstance(value, int):
                record = struct.pack("<QQIHH", value, self.put_text(key), crc, 1, 0)
            else:
                record = struct.pack("<QQIHH", self.put_text(value), self.put_text(key), crc, 5, len(value) + 1)
            self.emu.mem_write(at + 0x18 * i, record)
        return free

    def table(self, entries, at):
        """A loaded binary config's root table at `at`, its children after it."""
        self.records(entries, at + 0x18)
        self.emu.mem_write(at, struct.pack("<QQIHH", at + 0x18 if entries else 0, 0, 0, 3, len(entries)))

    def level(self, map_id, config):
        """The lobby loads the level: its maps_dict entry (id, config) at the site; the registers after."""
        self.texts = TEXTS
        self.emu.mem_write(NODES, bytes(0x6000))
        self.table(config, NODES)
        self.emu.mem_write(DESCRIPTOR, struct.pack("<III", map_id, 0x77777777, CONFIG))
        self.word(CONFIG + 0x108, NODES)
        regs, esp = self.run(LEVEL_SITE, (LEVEL_NEXT,), hide.STACK + 0x8000, {**hide.GAME, UC_X86_REG_EAX: DESCRIPTOR})
        return self.stopped, regs, esp

    def view(self, matrix):
        """update_camera's join with the first person's eye (&player+44Ch, the matrix written there)."""
        self.emu.mem_write(PLAYER + 0x44C, struct.pack("<16f", *matrix))
        self.word(PLAYER + 0xF470, 0x5EED0000)
        regs, esp = self.run(EYE_SITE, (EYE_NEXT,), hide.STACK + 0x8000,
                             {**hide.GAME, UC_X86_REG_EAX: PLAYER + 0x44C, UC_X86_REG_EBX: PLAYER})
        return self.stopped, regs, esp


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    diagnostics = base.with_diagnostics(built)
    stock, default, traced = Run(original), Run(bytes(built)), Run(diagnostics)
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    image = lambda data, va, size: bytes(data[hide.file_at(data, va):hide.file_at(data, va) + size])
    sites = ((LEVEL_SITE, "8b500883c008", 6), (0x25CD00, "558bec83e4f883ec0c", 9), (EYE_SITE, "8b8b70f40000", 6))
    report(all(image(original, va, n).hex() == old and image(diagnostics, va, 1) == b"\xE9" for va, old, n in sites),
           "level_loading.map 0x220482, tab_cursor.key 0x25CD00, camera.eye 0x1F3643: the original's bytes in the "
           "original, a jmp in the build")
    mode = {"mode": {"mode": 4, "name": "level_01_2", "match_making_params": {"map_name": "level_01/day_tdm",
                                                                              "map_id": 1, "map_weather_id": 204},
                     "ui": {"loading_screen_id": 1}}}

    def transparent(run, a, b, regs_of=lambda r: r):
        return a[0] == b[0] and regs_of(a[1]) == regs_of(b[1]) and a[2] == b[2]

    # the level's loading, the span replayed
    want = stock.level(204, mode)
    for name, run in (("default", default), ("diagnostics", traced)):
        got = run.level(204, mode)
        report(transparent(run, want, got) and got[0] == LEVEL_NEXT and got[1][UC_X86_REG_EAX] == DESCRIPTOR + 8
               and got[1][UC_X86_REG_EDX] == CONFIG,
               f"{name}: the level's loading returns to 0x220488 with EDX the config, EAX the entry +8 (the "
               "original's registers)")

    def press(run, key=COMMA, kind=DOWN):
        a = stock.key((key,) if kind != UP else (), key, kind)
        b = run.key((key,) if kind != UP else (), key, kind)
        regs = lambda r: {k: v & 0xFF if k == UC_X86_REG_ECX else v for k, v in r.items()}
        return a[0] == b[0] == keys.PAST and regs(a[1]) == regs(b[1]) and a[2] == b[2]

    pose = eye(-45.0, -2.0, (-48.5371, 24.5, -103.0804))
    want_eye = stock.view(pose)

    def look(run, matrix=pose):
        got = run.view(matrix)
        return transparent(run, want_eye if matrix is pose else stock.view(matrix), got) and got[0] == EYE_NEXT \
            and got[1][UC_X86_REG_ECX] == 0x5EED0000

    report(press(default) and look(default) and default.files == {},
           "default build: the comma key down goes on as the original's, the next eye too; nothing written "
           "(no file opened)")

    # the diagnostics build
    ok = press(traced, W) and look(traced) and traced.files == {}
    report(ok, "diagnostics: another key (W) and the next eye write nothing")
    ok = press(traced) and look(traced)
    poses = traced.files.get(POSES, b"").decode()
    row = ("mark_20261101_090507_042\t2026-11-01 09:05:07.042\t204\tlevel_01/day_tdm\t4\t-48.537\t24.500\t"
           "-103.080\t-45.00\t-2.00\t65.00")
    log = [v for k, v in traced.files.items() if k.endswith("srp_patches.log")]
    report(ok and poses == HEADER + "\n" + row + "\n" and not traced.handles,
           f"diagnostics: the comma key down, then the eye: {POSES} made with its header and the row "
           "(the time, map 204 of the options, the level's project and mode, the place, yaw -45 and pitch -2, "
           "uro_fov 65), the file closed")
    report(len(log) == 1 and b"pose_marker: mark_20261101_090507_042 written to srp_poses.tsv" in log[0],
           "diagnostics: srp_patches.log says the row was written")
    before = dict(traced.files)
    ok = look(traced) and press(traced, COMMA, HOLD) and look(traced) and press(traced, COMMA, UP) and look(traced)
    report(ok and traced.files == before, "diagnostics: the next eye, the key held, the key let go write nothing")
    traced.emu.mem_write(URO_FOV, struct.pack("<f", 80.0))
    second = eye(152.5, 30.0, (1.0, 2.0, 3.0))
    ok = press(traced) and press(traced) and look(traced, second) and look(traced, second)
    rows = traced.files[POSES].decode().splitlines()
    report(ok and len(rows) == 3 and rows[0] == HEADER and rows[2].split("\t")[2:] == [
        "204", "level_01/day_tdm", "4", "1.000", "2.000", "3.000", "152.50", "30.00", "80.00"],
           "diagnostics: pressed twice before an eye, one row more and no second header (yaw 152.5, pitch 30, "
           "uro_fov 80)")
    traced.level(61, mode)
    ok = press(traced) and look(traced, second)
    rows = traced.files[POSES].decode().splitlines()
    report(ok and len(rows) == 4 and rows[3].split("\t")[2:5] == ["204", "-", "-"],
           "diagnostics: a level loaded of another map (61): the match's map alone, no project or mode")

    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
