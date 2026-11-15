#!/usr/bin/env python3
"""Run patch 147 (rust/patches/src/run_logs.rs, vostok/run_log.rs and the
patches' log, vostok/log.rs: every run's logs in files of their own,
docs/notes/run-logs.md) in the default build, on a made-up folder of files.

    python check_run_logs_026g.py <original survarium.exe>

Requires Python `unicorn`. Run as the game: logging_initialize from its
site after generate_log_file_name (0xE31F8) to its call of new_log_file
(0xE31FD), EDI and [esp+0Ch] its frame's buffer_string over 104h bytes as the
game's own code made them (0xE31CA..0xE31EB). kernel32 is a made-up disk:
FindFirstFileA/FindNextFileA/FindClose list it (a pattern's `*` any run of
characters, as Windows matches a long name), DeleteFileA deletes, CreateFileA
makes (CREATE_NEW refused on a name there, OPEN_ALWAYS appends), WriteFile
appends; the local time is 2026-11-15 14:03:07. Checked:

- the two hooks in place (`jmp` to their entries); allocate_arenas' chain
  this patch's first, patch 83's after it; the engine's site the
  original's `mov eax,[esp+0Ch]` / `push eax`;
- a first run: new_log_file is given `Survarium_tester.2026-11-15_14-03-07.log`
  in the same folder (the string's end and NUL as assign makes them, its room
  as it was); nothing deleted; the patches' log a new file made with
  CREATE_NEW, `srp_patches.2026-11-15_14-03-07.log`, holding the patch's line;
- a folder with 100 of the user's run logs, the old `Survarium_tester.log`,
  another user's run logs and other files: the oldest run log alone deleted
  (100 with the new one), nothing else touched;
- 600 MiB in three run logs: the oldest deleted (400 MiB left, under 500);
- a name taken (another game started in the same second): `_2`, and the
  patches' log's `_2` likewise, the files there kept as they were;
- 120 old patches' logs and the old srp_patches.log / srp_patches.old.log:
  the 21 oldest run logs deleted, the two old ones kept;
- a folder whose path leaves no room for the stamp: the engine's name kept,
  the line says so;
- the clock set back (the earliest stamp written last): the least recently
  written goes, not the earliest stamp;
- a read-only oldest (DeleteFileA refuses it): passed over, the next ones go;
- this second's files (the run's own parts, 600 MiB): kept, an older run
  deleted, the next name past the highest of the second (`_4` after `_3`);
- a Shift-JIS user name whose trail byte is 5Ch, in a folder with one
  (IsDBCSLeadByte by GetProcAddress): the run's name in the same folder, the
  kind's oldest deleted;
- patch 100's ad_log (C) called as the game does: its line in the run's file
  through the entry log.c_line, no srp_patches.log made.
"""
import fnmatch
import struct
import sys

from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EDI, UC_X86_REG_ESP, UC_X86_REG_EIP

import patch_026g as base

SITE, CALL = 0xE31F8, 0xE31FD
KERNEL32 = {   # IAT slot: (name, stdcall argument count)
    0x870244: ("GetSystemTimeAsFileTime", 1), 0x870230: ("FileTimeToLocalFileTime", 2),
    0x87022C: ("FileTimeToSystemTime", 2), 0x870238: ("FindFirstFileA", 2), 0x87023C: ("FindNextFileA", 2),
    0x870228: ("FindClose", 1), 0x87021C: ("DeleteFileA", 1), 0x870048: ("CreateFileA", 7),
    0x870108: ("WriteFile", 5), 0x870040: ("CloseHandle", 1), 0x870118: ("GetCurrentThreadId", 0),
    0x87003C: ("GetModuleHandleA", 1), 0x870084: ("GetProcAddress", 2),
}
DYNAMIC = {"IsDBCSLeadByte": 1}           # kernel32's, by GetProcAddress (vostok::run_log::lead_byte)
KERNEL32_MODULE = 0x7C800000
RETURN = 0x30000F00                       # where a made-up call of ours returns
STUBS, STACK, HEAP = 0x30000000, 0x31000000, 0x32000000
INVALID = 0xFFFFFFFF
DOCS = "c:\\users\\tester\\documents\\survarium\\"
STAMP = "2026-11-15_14-03-07"
MB = 1 << 20

failed = 0
DEBUG = False


def shift_jis(byte):
    """Code page 932's lead bytes."""
    return 0x81 <= byte <= 0x9F or 0xE0 <= byte <= 0xFC


def check(ok, what):
    global failed
    failed += not ok
    print(f"  {'ok' if ok else 'FAIL'}  {what}")


class Disk:
    """Files by full path (the exe's folder is ''): their sizes, last writes (by default in the
    order of their names) and the ones DeleteFileA refuses (read-only); made files' bytes."""

    def __init__(self, files, written=None, stuck=()):
        self.sizes = dict(files)
        self.written = {path: n for n, path in enumerate(sorted(self.sizes))}
        self.written.update(written or {})
        self.stuck = set(stuck)
        self.bytes = {}
        self.deleted, self.made = [], []

    def folder(self, path):
        """Up to the last separator, as Windows reads a Shift-JIS path (a lead byte's trail
        byte is none); the made-up paths have no other bytes past 7Fh."""
        at, end = 0, 0
        while at < len(path):
            if path[at] == "\\":
                end = at + 1
            at += 2 if shift_jis(ord(path[at])) else 1
        return path[:end]

    def listing(self, pattern):
        folder = self.folder(pattern)
        mask = pattern[len(folder):].lower()
        return [p[len(folder):] for p in sorted(self.sizes)
                if self.folder(p) == folder and fnmatch.fnmatchcase(p[len(folder):].lower(), mask)]


class Game:
    def __init__(self, exe, disk):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(exe).sections(exe):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(exe[raw:raw + min(rsize, vsize or rsize)]))
        for at in (STUBS, STACK, HEAP):
            emu.mem_map(at, 0x100000, UC_PROT_ALL)
        self.stubs = {}
        for n, (slot, (name, args)) in enumerate(KERNEL32.items()):
            stub = STUBS + 0x10 * n
            self.word(slot, stub)
            self.stubs[stub] = (name, args)
            emu.hook_add(UC_HOOK_CODE, self.kernel32, begin=stub, end=stub)
        for n, (name, args) in enumerate(DYNAMIC.items()):
            stub = STUBS + 0x800 + 0x10 * n
            self.stubs[stub] = (name, args)
            emu.hook_add(UC_HOOK_CODE, self.kernel32, begin=stub, end=stub)
        emu.hook_add(UC_HOOK_CODE, self.stop, begin=CALL, end=CALL)
        emu.hook_add(UC_HOOK_CODE, self.stop, begin=RETURN, end=RETURN)
        self.disk, self.finds, self.handles = disk, {}, {}
        self.next_handle = 0x100
        self.lead = lambda byte: False

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def text(self, at):
        out = bytearray()
        while (byte := self.emu.mem_read(at + len(out), 1)[0]) != 0:
            out.append(byte)
        return out.decode("latin-1")

    def find_data(self, at, name):
        """A WIN32_FIND_DATAA: attributes, last write (+14h), sizes (+1Ch high, +20h low),
        cFileName (+2Ch)."""
        path = self.disk.folder(self.find_pattern) + name
        size = self.disk.sizes[path]
        data = bytearray(320)
        struct.pack_into("<I", data, 0, 0x80)
        struct.pack_into("<Q", data, 20, self.disk.written.get(path, 1 << 40))
        struct.pack_into("<II", data, 28, size >> 32, size & 0xFFFFFFFF)
        data[44:44 + len(name)] = name.encode("latin-1")
        self.emu.mem_write(at, bytes(data))

    def kernel32(self, emu, address, size, _):
        name, count = self.stubs[address]
        sp = emu.reg_read(UC_X86_REG_ESP)
        a = [self.read(sp + 4 + 4 * i) for i in range(count)]
        result = 1
        if DEBUG: print("   ", name, [hex(x) for x in a], self.text(a[0]) if name in ("FindFirstFileA", "CreateFileA", "DeleteFileA") else "")
        if name == "GetSystemTimeAsFileTime":
            emu.mem_write(a[0], struct.pack("<Q", 0x1DC_0000_0000_0000))
        elif name == "FileTimeToLocalFileTime":
            emu.mem_write(a[1], bytes(emu.mem_read(a[0], 8)))
        elif name == "FileTimeToSystemTime":    # 2026-11-15 (a Sunday) 14:03:07.250
            emu.mem_write(a[1], struct.pack("<8H", 2026, 11, 0, 15, 14, 3, 7, 250))
        elif name == "FindFirstFileA":
            self.find_pattern = self.text(a[0])
            names = self.disk.listing(self.find_pattern)
            if not names:
                result = INVALID
            else:
                handle, self.next_handle = self.next_handle, self.next_handle + 4
                self.finds[handle] = (self.find_pattern, names[1:])
                self.find_data(a[1], names[0])
                result = handle
        elif name == "FindNextFileA":
            pattern, rest = self.finds[a[0]]
            if rest:
                self.find_pattern = pattern
                self.find_data(a[1], rest[0])
                self.finds[a[0]] = (pattern, rest[1:])
            else:
                result = 0
        elif name == "FindClose":
            # INVALID_HANDLE_VALUE too: files.rs drops the Find it made of a failed FindFirstFileA
            # (`then_some` builds it either way), as Windows a FindClose that fails
            result = int(self.finds.pop(a[0], None) is not None)
        elif name == "DeleteFileA":
            path = self.text(a[0])
            if path in self.disk.stuck:
                result = 0
            else:
                self.disk.deleted.append(path)
                result = int(self.disk.sizes.pop(path, None) is not None)
        elif name == "CreateFileA":
            path, disposition = self.text(a[0]), a[4]
            if disposition == 1 and path in self.disk.sizes:      # CREATE_NEW on a name there
                result = INVALID
            else:
                self.disk.made.append((path, disposition, a[1], a[2]))
                self.disk.sizes.setdefault(path, 0)
                self.disk.bytes.setdefault(path, b"")
                handle, self.next_handle = self.next_handle, self.next_handle + 4
                self.handles[handle] = path
                result = handle
        elif name == "WriteFile":
            path = self.handles[a[0]]
            self.disk.bytes[path] += bytes(emu.mem_read(a[1], a[2]))
            self.disk.sizes[path] += a[2]
            self.word(a[3], a[2])
        elif name == "CloseHandle":
            self.handles.pop(a[0])
        elif name == "GetCurrentThreadId":
            result = 1
        elif name == "GetModuleHandleA":
            result = KERNEL32_MODULE if self.text(a[0]).lower() == "kernel32.dll" else 0
        elif name == "GetProcAddress":
            wanted = self.text(a[1])
            result = next((at for at, (n, _) in self.stubs.items() if n == wanted and n in DYNAMIC), 0) \
                if a[0] == KERNEL32_MODULE else 0
        elif name == "IsDBCSLeadByte":
            result = int(self.lead(a[0] & 0xFF))
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + 4 * count)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def stop(self, emu, *_):
        emu.emu_stop()

    def name_log(self, path):
        """logging_initialize at the site, its frame as 0xE31CA..0xE31F3 leave it with `path`
        named: the string's text, begin and room after, and new_log_file's argument."""
        sp = STACK + 0x8000
        string, buffer = sp + 0xC, sp + 0x18
        raw = path.encode("latin-1")
        self.emu.mem_write(buffer, raw.ljust(0x104, b"\0") + b"\\")
        self.emu.mem_write(string, struct.pack("<III", buffer, buffer + len(raw), buffer + 0x104))
        self.emu.reg_write(UC_X86_REG_EDI, string)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        try:
            self.emu.emu_start(SITE, 0, count=5_000_000)
        except UcError as error:
            return None, f"fault at {self.emu.reg_read(UC_X86_REG_EIP):#x}: {error}"
        at = self.emu.reg_read(UC_X86_REG_EIP)
        if at != CALL:
            return None, f"stopped at {at:#x}"
        begin, end, room = struct.unpack("<III", self.emu.mem_read(string, 12))
        argument = self.read(self.emu.reg_read(UC_X86_REG_ESP))
        whole = bytes(self.emu.mem_read(begin, end - begin + 1))
        intact = (begin, room, self.emu.mem_read(buffer + 0x104, 1)[0]) == (buffer, buffer + 0x104, 0x5C)
        return (whole[:-1].decode("latin-1"), whole[-1] == 0 and intact, argument == begin), None

    def call(self, at, *arguments):
        """A cdecl call of ours (from C) to `at`, its arguments words; the registers it leaves."""
        sp = STACK + 0x4000
        for n, word in enumerate((RETURN,) + arguments):
            self.word(sp + 4 * n, word)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.emu_start(at, 0, count=5_000_000)
        return self.emu.reg_read(UC_X86_REG_EIP), self.emu.reg_read(UC_X86_REG_ESP) - sp


def run(exe, files, path=DOCS + "Survarium_tester.log", written=None, stuck=(), lead=None, then=None):
    disk = Disk(files, written, stuck)
    game = Game(exe, disk)
    if lead:
        game.lead = lead
    named, fault = game.name_log(path)
    if then:
        then(game)
    return disk, named, fault


def patches_log(disk):
    made = [(path, disposition) for path, disposition, *_ in disk.made if not path.startswith("c:")]
    return made, {path: disk.bytes[path] for path, _ in made}


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    built, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None,
                          open(base.late_join_bin(), "rb").read())
    built = bytes(built)
    linked = base.arena()
    reader = base.hooks_026g.original_reader(original)
    ours = [h for h in base.hooks_026g.hooks(exe=reader)
            if any(m["patch"] == "run_logs" for m in h.get("chain", [h])) and h["site"] is not None]   # log.c_line: an entry, no site
    installs = {h[0]: h for h in base.rust_hooks(original)}
    for h in ours:
        va, old, symbol, opcode = installs[h["site"]]
        at = base.file_offset(va)
        new = base.hook_bytes(linked.symbols[symbol], va, old, 0, opcode)
        check(opcode == 0xE9 and original[at:at + len(old)] == old and built[at:at + len(new)] == new,
              f"{va:#x}: {old.hex()} in the original, `jmp {symbol}` here")
    chain = next((h.get("chain") for h in ours if installs[h["site"]][0] == 0xE9580), None)
    check(chain is not None and [m["patch"] for m in chain] == ["run_logs", "hud_options"],
          f"allocate_arenas: run_logs first, then hud_options ({chain and [m['patch'] for m in chain]})")
    site = [installs[h["site"]] for h in ours if installs[h["site"]][0] == SITE]
    check(len(ours) == 2 and site and site[0][1] == bytes.fromhex("8b44240c50"),
          "the engine's site: mov eax,[esp+0Ch] / push eax")

    new_engine = DOCS + f"Survarium_tester.{STAMP}.log"
    new_patches = f"srp_patches.{STAMP}.log"

    print("a first run")
    disk, named, fault = run(built, {})
    check(fault is None and named == (new_engine, True, True),
          f"new_log_file opens {new_engine}, the string as assign leaves it ({fault or named})")
    made, logs = patches_log(disk)
    check(made == [(new_patches, 1)] and not disk.deleted,
          f"the patches' log: {new_patches} made with CREATE_NEW, nothing deleted ({made}, {disk.deleted})")
    check(logs.get(new_patches) == b"srp: run_logs: the game's log is the run's, 0 old ones deleted\r\n",
          f"its line ({logs.get(new_patches)!r})")
    access = [(a, s) for p, _, a, s in disk.made]
    check(access == [(4, 7)], f"appends only, every sharing ({access})")

    print("100 run logs of the user's, and others")
    old = {DOCS + f"Survarium_tester.2026-10-{day:02}_{hour:02}-00-00.log": 1000
           for day in range(1, 26) for hour in (1, 2, 3, 4)}
    others = {DOCS + "Survarium_tester.log": 5000, DOCS + f"Survarium_other.2026-09-01_00-00-00.log": 10,
              DOCS + "user.cfg": 10, DOCS + "Survarium_tester.2026-09-01_00-00-00.txt": 10}
    disk, named, fault = run(built, {**old, **others})
    oldest = DOCS + "Survarium_tester.2026-10-01_01-00-00.log"
    check(fault is None and named == (new_engine, True, True), f"the run's name ({fault or named})")
    check(disk.deleted == [oldest], f"the oldest alone deleted ({disk.deleted})")
    kept = [p for p in disk.sizes if p.startswith(DOCS + "Survarium_tester.2026") and p.endswith(".log")]
    check(len(kept) == 99 and all(p in disk.sizes for p in others),
          f"99 kept (100 with the new one), the others untouched ({len(kept)})")
    _, logs = patches_log(disk)
    check(logs.get(new_patches, b"").endswith(b", 1 old ones deleted\r\n"), f"the line counts it ({logs})")

    print("600 MiB in three")
    big = {DOCS + f"Survarium_tester.2026-10-0{day}_00-00-00.log": 200 * MB for day in (1, 2, 3)}
    disk, named, fault = run(built, big)
    check(disk.deleted == [DOCS + "Survarium_tester.2026-10-01_00-00-00.log"] and named and named[0] == new_engine,
          f"the oldest deleted, 400 MiB left ({disk.deleted})")

    print("a name taken")
    taken = {new_engine: 7, new_patches: 9}
    disk, named, fault = run(built, taken)
    second = DOCS + f"Survarium_tester.{STAMP}_2.log"
    made, logs = patches_log(disk)
    check(named and named[0] == second, f"the engine's {second} ({named})")
    check(made == [(f"srp_patches.{STAMP}_2.log", 1)] and disk.sizes[new_engine] == 7
          and disk.sizes[new_patches] == 9 and not disk.deleted,
          f"the patches' `_2`, the files there as they were ({made})")

    print("120 old patches' logs")
    srp = {f"srp_patches.2026-10-{day:02}_{hour:02}-00-00.log": 100 for day in range(1, 31) for hour in (1, 2, 3, 4)}
    legacy = {"srp_patches.log": 50, "srp_patches.old.log": 60}
    disk, named, fault = run(built, {**srp, **legacy})
    gone = sorted(srp)[:21]
    check(disk.deleted == gone, f"the 21 oldest deleted ({len(disk.deleted)}: {disk.deleted[:2]}...)")
    check(all(p in disk.sizes for p in legacy) and sum(p in disk.sizes for p in srp) == 99,
          "the old srp_patches.log and srp_patches.old.log kept, 99 run logs and the new one")

    print("no room for the stamp")
    deep = "c:\\" + "d" * 210 + "\\survarium\\Survarium_tester.log"
    assert len(deep) < 0x104 and len(deep) + len(STAMP) + 1 > 259
    disk, named, fault = run(built, {}, deep)
    _, logs = patches_log(disk)
    check(fault is None and named == (deep, True, True), f"the engine's name kept ({fault or named})")
    check(logs.get(new_patches, b"").startswith(b"srp: run_logs: the game's log keeps its name"),
          f"the line says so ({logs})")

    print("the clock set back")
    hundred = {DOCS + f"Survarium_tester.2026-10-{day:02}_{hour:02}-00-00.log": 1 for day in range(1, 26)
               for hour in (1, 2, 3, 4)}
    first = DOCS + "Survarium_tester.2026-10-01_01-00-00.log"     # the earliest stamp, written last
    disk, named, fault = run(built, hundred, written={first: 1 << 30})
    check(disk.deleted == [DOCS + "Survarium_tester.2026-10-01_02-00-00.log"] and first in disk.sizes,
          f"the least recently written deleted, not the earliest stamp ({disk.deleted})")

    print("a read-only oldest")
    disk, named, fault = run(built, {**hundred, DOCS + "Survarium_tester.2026-09-30_00-00-00.log": 1},
                             stuck=[DOCS + "Survarium_tester.2026-09-30_00-00-00.log"])
    check(disk.deleted == [first, DOCS + "Survarium_tester.2026-10-01_02-00-00.log"]
          and named and named[0] == new_engine,
          f"passed over, the next two deleted: 99 left ({disk.deleted})")

    print("this second's files")
    mine = {f"srp_patches.{STAMP}.log": 300 * MB, f"srp_patches.{STAMP}_3.log": 300 * MB,
            "srp_patches.2026-10-01_00-00-00.log": 1}
    disk, named, fault = run(built, mine)
    made, _ = patches_log(disk)
    check(disk.deleted == ["srp_patches.2026-10-01_00-00-00.log"] and made == [(f"srp_patches.{STAMP}_4.log", 1)],
          f"600 MiB of this second's kept, the older run deleted, the next name past the highest ({disk.deleted}, {made})")

    print("a Japanese user name (Shift-JIS, a trail byte 5Ch)")
    user = "Survarium_\x83\x5c\x83\x6a\x81\x5b"
    folder = "c:\\users\\\x95\x5c\\documents\\survarium\\"
    old_run = folder + user + ".2026-10-01_00-00-00.log"
    full = {folder + user + f".2026-10-{day:02}_{hour:02}-00-00.log": 1 for day in range(1, 26) for hour in (1, 2, 3, 4)}
    disk, named, fault = run(built, full, folder + user + ".log", lead=shift_jis)
    check(fault is None and named and named[0] == folder + user + f".{STAMP}.log",
          f"the run's name in the same folder ({fault or named and named[0]!a})")
    check(disk.deleted == [folder + user + ".2026-10-01_01-00-00.log"] and old_run not in disk.deleted,
          f"its kind's oldest deleted ({[d.encode('latin-1') for d in disk.deleted]})")

    print("patch 100's C log line")
    ad_log = linked.symbols.get("ad_log")
    outcome = {}

    def c_line(game):
        text = HEAP + 0x100
        game.emu.mem_write(text, b"a send to a voice not made again, left out\0")
        outcome["stop"] = game.call(ad_log, text, 0x1234)

    disk, named, fault = run(built, {}, then=c_line)
    _, logs = patches_log(disk)
    check(ad_log is not None and outcome.get("stop", (0,))[0] == RETURN
          and logs.get(new_patches, b"").endswith(
              b"\r\nsrp: audio_device: a send to a voice not made again, left out 0x00001234\r\n")
          and "srp_patches.log" not in disk.sizes,
          f"ad_log's line in the run's file, nothing in srp_patches.log ({outcome}, {logs})")

    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
