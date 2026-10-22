#!/usr/bin/env python3
"""Run patch 27, the mount of resources.db's patches (mount_patches_026g.asm),
in the default build on made-up game folders.

    python check_mount_patches_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are and
again 1 MiB higher with its base relocations applied (a rebased image).
What runs as machine code: query_db's call at 0xE7E3E with the code it
calls, and on_mounted_resources 0x147E0 with the code its first six bytes
jump to and the game's own code after them, to the start of its next step
(0x14827: the reference it takes, the store into engine_world, the release
of the mount that was there). Stubs: CreateFileA, GetLastError and
CloseHandle (a folder's files, each there or failing with a Windows error),
the path formatting 0x18190 (as its code reads: the text from the string's
start, within its buffer, a '/' or '\\' made the string's separator; the C
library under it needs a started process), query_mount_archive 0xE7EF0
(it records what it was asked to mount), the fatal error 0x113580 (it
records the message), the unmount of a mount nobody holds 0xDFE30 (it
records the mount) and the game's next step at 0x14827 (it records what
engine_world keeps and goes to the function's own end, 0x148A0). Each
callback's caller holds the mount (a count of 1) while it runs and lets it
go after, as the file system does.

Each folder must give: the mounts queued in order (resources.db as the game
asked, then every patch n from 1 to 255 that is there, in order, a missing
number skipped) with the game's arguments but the two paths; every register
the game had and the stack as it was; no handle left open, no access asked
for a file, GetLastError asked only after a failed CreateFileA; every
callback held but the last, which goes on into the game's code with
resources.db's mount; at the end resources.db held once (engine_world's
reference, as in the retail game), each patch once (the state's), nothing
unmounted. A file that is there and cannot be opened (a folder by that
name, access denied, a sharing violation: any error but 2 and 3) stops the
game before anything is queued, with its number and the error; a failed
mount stops it with the patch's number, or the game's own message for
resources.db. Says nothing about what the real file system does.

Patch 49, the patches an exe requires: the same folders and more with a
build that requires patches 1, 12, 14, 17 and 255 (--required-patches): a
required patch that is not there (error 2 or 3) stops the game before
anything is queued with its number and the error, the lowest such number;
one that is not required and not there is skipped; one that is there and not
required is mounted as before. The default build (no patch required) runs
every folder above as it did before patch 49. And the case of 2026-10-21: a
build that requires revision 27's list (12, 14, 17, 19 to 27) in a folder
with 12 to 25 alone stops at patch 26; with all of them it mounts them.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import patch_026g as base

IMPORTS = {0x870040: "CloseHandle", 0x870048: "CreateFileA", 0x87005C: "GetLastError"}
ARGUMENTS = {"CloseHandle": 1, "CreateFileA": 7, "GetLastError": 0}
FATAL = 0x113580
FORMAT_PATH = 0x18190
UNMOUNT = 0xDFE30
NEXT_STEP, END = 0x14827, 0x148A0       # on_mounted_resources' next step, its own end
GAME_FATAL = 0x9DB7A0                   # "Cannot mount resources. ..."
STUBS, HEAP, STACK = 0x30000000, 0x31000000, 0x32000000
RETURN = STUBS + 0x800                  # where the made-up callers continue
ALLOCATOR, HELPER = 0x0A7B688, 0x31000F00
WORLD = HEAP + 0x8000                   # engine_world+2D4h, m_resources_mount
GAME = {UC_X86_REG_EBX: 0x22222222, UC_X86_REG_EBP: 0x0019FF00, UC_X86_REG_ESI: ALLOCATOR,
        UC_X86_REG_EDI: HELPER}
STOP = "stop"
NOT_FOUND, PATH_NOT_FOUND, ACCESS_DENIED, SHARING_VIOLATION = 2, 3, 5, 32


def path_string(at, text, separator):
    """A native/virtual path string as query_db makes them: begin, end of
    the text, end of the buffer, 260 characters, the separator."""
    raw = text.encode("ascii")
    return struct.pack("<III", at + 12, at + 12 + len(raw), at + 0x110) + raw.ljust(0x104, b"\0") \
        + separator.encode("ascii")


class Run:
    def __init__(self, data, delta, files):
        """`files`: path -> None (there) or the Windows error CreateFileA
        gives for it; a path not in it is not there (error 2)."""
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        self.delta, self.files = delta, dict(files)
        headers = base.Headers(data)
        for _, rva, vsize, raw, rsize, _ in headers.sections(data):
            start = base.IMAGE_BASE + rva + delta
            size = (max(vsize, rsize) + 0xFFF) & ~0xFFF
            emu.mem_map(start, size, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        if delta:
            reloc_rva, reloc_size = struct.unpack_from("<II", data, headers.reloc_at)
            table = next(raw + reloc_rva - rva for _, rva, vsize, raw, rsize, _ in headers.sections(data)
                         if rva <= reloc_rva < rva + vsize)
            at = table
            while at < table + reloc_size:
                page, block = struct.unpack_from("<II", data, at)
                if block < 8:
                    break
                for i in range((block - 8) // 2):
                    entry = struct.unpack_from("<H", data, at + 8 + 2 * i)[0]
                    if entry >> 12 == 3:
                        where = base.IMAGE_BASE + page + (entry & 0xFFF) + delta
                        self.word(where, self.read(where) + delta)
                at += block
        for at in (STUBS, HEAP, STACK):
            emu.mem_map(at, 0x100000, UC_PROT_ALL)
        self.stubs = {STUBS + 0x10 * i: name for i, name in enumerate(ARGUMENTS)}
        for slot, name in IMPORTS.items():
            self.word(slot + delta, next(at for at, n in self.stubs.items() if n == name))
        self.stubs[base.MOUNT_QUERY_MOUNT_ARCHIVE + delta] = "query_mount_archive"
        self.stubs[FATAL + delta] = "fatal"
        self.stubs[FORMAT_PATH + delta] = "format_path"
        self.stubs[UNMOUNT + delta] = "unmount"
        self.stubs[NEXT_STEP + delta] = "next_step"
        self.stubs[RETURN] = STOP
        self.opened, self.closed, self.mounts, self.kept, self.fatal = [], [], [], [], None
        self.unmounted, self.made, self.last_error = [], [], None
        for at in self.stubs:
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=at, end=at)
        # The query's own strings, as query_db has them.
        self.virtual, self.archive, self.descriptor, self.callback = HEAP, HEAP + 0x200, HEAP + 0x400, HEAP + 0x500
        emu.mem_write(self.virtual, path_string(self.virtual, "resources", "/"))
        emu.mem_write(self.archive, path_string(self.archive, "..\\..\\resources.db", "\\"))
        emu.mem_write(self.descriptor, b"resources\0")
        self.next_mount = HEAP + 0x10000

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def text(self, at):
        out = bytearray()
        while (byte := self.emu.mem_read(at + len(out), 1)[0]) != 0:
            out.append(byte)
        return out.decode("ascii")

    def string(self, at):
        """A path string's text, which must be within its buffer and end
        with its zero."""
        begin, end, buffer_end = struct.unpack("<III", self.emu.mem_read(at, 12))
        assert begin == at + 12 and buffer_end == at + 0x110 and begin <= end < buffer_end
        raw = bytes(self.emu.mem_read(begin, end - begin + 1))
        assert raw[-1] == 0 and 0 not in raw[:-1]
        return raw[:-1].decode("ascii"), chr(self.emu.mem_read(at + 0x110, 1)[0])

    def references(self, mount):
        return self.read(mount + 0x1C)

    def stub(self, emu, address, size, _):
        name = self.stubs[address]
        if name == STOP:
            emu.emu_stop()
            return
        sp = emu.reg_read(UC_X86_REG_ESP)
        if name == "fatal":
            fmt, args, out = self.text(self.read(sp + 4)), sp + 8, ""
            pieces = fmt.split("%u")
            for piece in pieces[:-1]:
                out += piece + str(self.read(args))
                args += 4
            self.fatal = out + pieces[-1]
            emu.emu_stop()
            return
        if name == "next_step":
            # on_mounted_resources at 0x14827: ebp is its frame, the mount
            # kept. What engine_world keeps, then the function's own end
            # (`pop esi` / `mov esp,ebp` / `pop ebp` / `ret`).
            frame = emu.reg_read(UC_X86_REG_EBP)
            assert (sp + 0x30) & 7 == 0 and sp + 0x30 <= frame, "the game's frame is as its own code makes it"
            self.kept.append(self.read(self.read(frame + 8)))
            emu.reg_write(UC_X86_REG_EIP, END + self.delta)
            return
        if name == "unmount":
            # eax = the mount; keeps esi, edi, ebx and ebp as the game's does.
            self.unmounted.append(emu.reg_read(UC_X86_REG_EAX))
            for register in (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX):
                emu.reg_write(register, 0xDEAD0000 | register)
            emu.reg_write(UC_X86_REG_EIP, self.read(sp))
            emu.reg_write(UC_X86_REG_ESP, sp + 4)
            return
        if name == "format_path":
            # esi = the string; cdecl (format, ...); returns the string.
            string = emu.reg_read(UC_X86_REG_ESI)
            begin, _, buffer_end = struct.unpack("<III", emu.mem_read(string, 12))
            separator = emu.mem_read(string + 0x110, 1)[0]
            pieces, args = self.text(self.read(sp + 4)).split("%"), sp + 8
            out = pieces[0]
            for piece in pieces[1:]:
                kind, piece = piece[0], piece[1:]
                out += self.text(self.read(args)) if kind == "s" else str(self.read(args))
                assert kind in "su"
                out += piece
                args += 4
            raw = out.encode("ascii")[:buffer_end - begin - 1]
            raw = raw.replace(b"\\" if separator == 0x2F else b"/", bytes([separator]))
            emu.mem_write(begin, raw + b"\0")
            self.word(string + 4, begin + len(raw))
            result = string
        elif name == "query_mount_archive":
            assert emu.reg_read(UC_X86_REG_ESI) == ALLOCATOR + self.delta, "esi is the allocator"
            args = [self.read(sp + 4 + 4 * i) for i in range(5)]
            virtual, fat, archive, descriptor, callback = args
            assert fat == archive, "the file table is the archive's"
            self.mounts.append((self.string(virtual), self.string(archive), self.text(descriptor), callback))
            result = 0
        else:
            args = [self.read(sp + 4 + 4 * i) for i in range(ARGUMENTS[name])]
            result = getattr(self, name)(*args)
            sp += 4 * len(args)                 # stdcall
        # Every register but esp may be another after a call of the game's.
        emu.reg_write(UC_X86_REG_EAX, result)
        for register in (UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, UC_X86_REG_ESI,
                         UC_X86_REG_EDI, UC_X86_REG_EBP):
            emu.reg_write(register, 0xDEAD0000 | register)
        emu.reg_write(UC_X86_REG_EIP, self.read(emu.reg_read(UC_X86_REG_ESP)))
        emu.reg_write(UC_X86_REG_ESP, sp + 4)

    def CreateFileA(self, path, access, share, security, disposition, attributes, template):
        assert (access, disposition) == (0, 3), "no access, an existing file only"
        path = self.text(path)
        error = self.files.get(path, NOT_FOUND)
        if error is not None:
            self.last_error = error
            return 0xFFFFFFFF
        self.last_error = None
        self.opened.append(0x100 + 4 * len(self.opened))
        return self.opened[-1]

    def GetLastError(self):
        assert self.last_error is not None, "GetLastError right after a failed CreateFileA"
        error, self.last_error = self.last_error, None
        return error

    def CloseHandle(self, handle):
        assert handle in self.opened and handle not in self.closed
        self.closed.append(handle)
        return 1

    def query(self):
        """query_db's call at 0xE7E3E with query_db's arguments. Returns the
        archives queued (none when it stopped at a fatal error)."""
        emu = self.emu
        sp = STACK + 0x80000
        for value in (HELPER, self.descriptor, self.archive, self.archive, self.virtual, RETURN):
            sp -= 4
            self.word(sp, value)
        for register, value in GAME.items():
            emu.reg_write(register, value + (self.delta if register == UC_X86_REG_ESI else 0))
        # The patched call at 0xE7E3E, with a made-up return address.
        call = bytes(emu.mem_read(base.MOUNT_QUERY_VA + self.delta, 5))
        assert call[0] == 0xE8
        target = base.MOUNT_QUERY_VA + self.delta + 5 + struct.unpack("<i", call[1:])[0]
        emu.reg_write(UC_X86_REG_ESP, sp)
        emu.emu_start(target, RETURN, count=2_000_000)
        assert self.closed == self.opened, "a handle left open"
        if self.fatal is not None:
            return [archive for _, (archive, _), _, _ in self.mounts]
        kept = {register: emu.reg_read(register) for register in GAME}
        expected = {register: value + (self.delta if register == UC_X86_REG_ESI else 0)
                    for register, value in GAME.items()}
        assert kept == expected, f"registers changed: {kept} != {expected}"
        assert emu.reg_read(UC_X86_REG_ESP) == sp + 4, "the stack as it was (the caller takes the arguments)"
        for (virtual, archive, descriptor, callback) in self.mounts:
            assert virtual == ("resources", "/") and descriptor == "resources" and callback == HELPER
        return [archive for _, (archive, _), _, _ in self.mounts]

    def mounted(self, mount, holder=WORLD):
        """One mount's callback: on_mounted_resources(holder, the start-up's
        next step, &mount) with a made-up mount whose reference count is 1
        (the caller's), or a null one; then the caller lets its reference
        go. Returns (the mount, whether the game's next step ran)."""
        emu = self.emu
        if mount:
            mount = self.next_mount
            self.next_mount += 0x40
            self.word(mount + 0x1C, 1)
            self.made.append(mount)
        argument = HEAP + 0x9200
        self.word(argument, mount)
        sp = STACK + 0x60000 - 4
        for value in (argument, HEAP + 0x9000, holder, RETURN):
            sp -= 4
            self.word(sp, value)
        before = len(self.kept)
        for register, value in GAME.items():
            emu.reg_write(register, value)
        emu.reg_write(UC_X86_REG_ESP, sp)
        emu.emu_start(base.MOUNTED_VA + self.delta, RETURN, count=100000)
        if self.fatal is None:
            assert emu.reg_read(UC_X86_REG_ESP) == sp + 4, "cdecl: the stack as the caller left it"
            kept = {register: emu.reg_read(register) for register in GAME}
            assert kept == GAME, f"registers changed: {kept} != {GAME}"
            if mount:
                count = self.references(mount) - 1
                self.word(mount + 0x1C, count)
                if count == 0:
                    self.unmounted.append(mount)
        return mount, len(self.kept) > before

    def state_mounts(self):
        state = struct.unpack_from("<I", self.emu.mem_read(base.MOUNT_PATCHES_VA + self.delta
                                                           + len(open(base.mount_patches_bin(), "rb").read()) - 4,
                                                           4))[0] + self.delta
        return [self.read(state + 8 + 4 * i) for i in range(self.read(state + 4))]


def patch(n):
    return f"..\\..\\resources.db.patch_{n}"


def folder(*numbers, **errors):
    """Patches n that are there, and patch_<n>=error for one that fails."""
    files = {patch(n): None for n in numbers}
    files.update({patch(int(key.split("_")[1])): error for key, error in errors.items()})
    return files


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    upload = open(base.default_bin("section"), "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    data, _ = base.build(original, "section", upload, None, late_join)
    needed = (1, 12, 14, 17, 255)
    requiring, _ = base.build(original, "section", upload, None, late_join, required_patches=needed)
    live = (12, 14, 17, *range(19, 28))
    requiring_live, _ = base.build(original, "section", upload, None, late_join, required_patches=live)
    assert base.required_patches_of(data) == [] and base.required_patches_of(requiring) == list(needed) \
        and base.required_patches_of(requiring_live) == list(live)
    db = "..\\..\\resources.db"
    folders = [
        ("no patch", folder(), [db]),
        ("patches 1 to 3", folder(1, 2, 3), [db, patch(1), patch(2), patch(3)]),
        ("patches 1 to 4 and 6", folder(1, 2, 3, 4, 6), [db] + [patch(n) for n in (1, 2, 3, 4, 6)]),
        ("patches 14 and 17 alone", folder(14, 17), [db, patch(14), patch(17)]),
        ("patch 255 alone", folder(255), [db, patch(255)]),
        ("not a patch: .patch_1.db, _01, _0, _256",
         {db + ".patch_1.db": None, db + ".patch_01": None, db + ".patch_0": None, db + ".patch_256": None}, [db]),
        ("patches 0 to 300: 1 to 255", folder(*range(0, 301)), [db] + [patch(n) for n in range(1, 256)]),
        ("patches 1 and 3, 2 path not found (error 3): not there", folder(1, 3, patch_2=PATH_NOT_FOUND),
         [db, patch(1), patch(3)]),
    ]
    stopped = [
        ("a folder named resources.db.patch_3 (error 5, as CreateFileA gives for a folder)",
         folder(1, 2, patch_3=ACCESS_DENIED), 3, ACCESS_DENIED),
        ("patch 1 access denied (error 5)", folder(2, patch_1=ACCESS_DENIED), 1, ACCESS_DENIED),
        ("patch 255 in use, a sharing violation (error 32)", folder(14, patch_255=SHARING_VIOLATION),
         255, SHARING_VIOLATION),
    ]
    all_needed = folder(*needed)
    required = [
        ("every required patch there", all_needed, [db] + [patch(n) for n in needed]),
        ("every required patch there, and 2, 13 and 20 that are not required",
         folder(*needed, 2, 13, 20), [db] + [patch(n) for n in sorted(needed + (2, 13, 20))]),
        ("every required patch there, 13 path not found (error 3): not required, not there",
         {**all_needed, **folder(patch_13=PATH_NOT_FOUND)}, [db] + [patch(n) for n in needed]),
    ]
    missing = [
        ("no patch", folder(), 1, NOT_FOUND),
        ("patch 1 alone", folder(1), 12, NOT_FOUND),
        ("every required patch but 14", folder(1, 12, 17, 255, 20), 14, NOT_FOUND),
        ("every required patch but 14 and 17: the first", folder(1, 12, 255), 14, NOT_FOUND),
        ("every required patch but 255", folder(1, 12, 14, 17, 2), 255, NOT_FOUND),
        ("every required patch, 12 path not found (error 3)", {**all_needed, **folder(patch_12=PATH_NOT_FOUND)},
         12, PATH_NOT_FOUND),
        ("every required patch, 17 a folder by that name (error 5)", {**all_needed, **folder(patch_17=ACCESS_DENIED)},
         17, ACCESS_DENIED),
        ("the patches of revision 27 (12 to 27, no 13, 15, 16, 18)", folder(12, 14, 17, *range(19, 28)), 1, NOT_FOUND),
    ]
    failed = 0

    def say(ok, text):
        nonlocal failed
        failed += not ok
        print(f"{'ok  ' if ok else 'FAIL'} {text}")

    for delta in (0, 0x100000):
        rebased = " (rebased)" if delta else ""
        for name, files, expected in folders:
            run = Run(data, delta, files)
            got = run.query()
            names = [g.split("\\")[-1] for g in got]
            shown = ", ".join(names if len(names) < 6 else names[:3] + ["..."] + names[-1:])
            say(got == expected and run.fatal is None, f"{name}{rebased}: mounts {len(got)}: {shown}")
            # Every callback in order: all held, the last goes on into the
            # game with resources.db's mount; then resources.db has the
            # world's reference alone, each patch the state's alone.
            results = [run.mounted(True) for _ in got]
            first = results[0][0]
            ok = not any(went for _, went in results[:-1]) and results[-1][1] and run.kept == [first] \
                and run.read(WORLD) == first and run.state_mounts() == run.made and not run.unmounted
            say(ok, f"{name}{rebased}: {len(got) - 1} callbacks held, the last goes on with resources.db's mount")
            counts = [run.references(mount) for mount in run.made]
            say(counts == [1] * len(got) and not run.unmounted,
                f"{name}{rebased}: resources.db held once (engine_world's, as the retail game), "
                f"{len(got) - 1} patches once each (the state's), nothing unmounted")
            # A later call is the game's own, its argument untouched.
            mount, went = run.mounted(True, HEAP + 0x8100)
            say(went and run.references(mount) == 1 and run.kept[-1] == mount and run.read(HEAP + 0x8100) == mount
                and not run.unmounted and run.references(first) == 1,
                f"{name}{rebased}: a later callback is the game's own")
        for name, files, n, error in stopped:
            run = Run(data, delta, files)
            got = run.query()
            say(run.fatal == f"Cannot open resources.db.patch_{n} (error {error}). Please run the Skhid launcher "
                "again." and got == [] and run.closed == run.opened,
                f"{name}{rebased}: stops before any mount: {run.fatal!r}")
        # A failed mount: the fatal names the file.
        for which, n in ((1, 14), (2, 17), (3, 20)):
            run = Run(data, delta, folder(14, 17, 20))
            run.query()
            for k in range(4):
                run.mounted(k != which)
                if run.fatal is not None:
                    break
            say(run.fatal == f"Cannot mount resources.db.patch_{n}. Please run the Skhid launcher again."
                and not run.kept and not run.unmounted,
                f"patches 14, 17, 20, {n} fails{rebased}: {run.fatal!r}")
        # Patch 49: a build that requires 1, 12, 14, 17 and 255.
        for name, files, expected in required:
            run = Run(requiring, delta, files)
            got = run.query()
            results = [run.mounted(True) for _ in got]
            say(got == expected and run.fatal is None and results[-1][1] and run.kept == [results[0][0]]
                and not run.unmounted and [run.references(mount) for mount in run.made] == [1] * len(got),
                f"requiring {', '.join(map(str, needed))}: {name}{rebased}: mounts {len(got)}, the last callback "
                "goes on with resources.db's mount")
        for name, files, n, error in missing:
            run = Run(requiring, delta, files)
            got = run.query()
            say(run.fatal == f"Cannot open resources.db.patch_{n} (error {error}). Please run the Skhid launcher "
                "again." and got == [] and run.closed == run.opened,
                f"requiring {', '.join(map(str, needed))}: {name}{rebased}: stops before any mount: {run.fatal!r}")
        run = Run(requiring_live, delta, folder(*live[:-2]))
        got = run.query()
        say(run.fatal == "Cannot open resources.db.patch_26 (error 2). Please run the Skhid launcher again."
            and got == [], f"requiring revision 27's list: without 26 and 27{rebased}: stops before any mount: "
            f"{run.fatal!r}")
        run = Run(requiring_live, delta, folder(*live))
        got = run.query()
        say(got == [db] + [patch(n) for n in live] and run.fatal is None,
            f"requiring revision 27's list: all of it{rebased}: mounts {len(got)}")
        run = Run(data, delta, folder(1))
        run.query()
        run.mounted(False)
        say(run.fatal == run.text(GAME_FATAL + delta) and run.fatal.startswith("Cannot mount resources.")
            and not run.kept, f"resources.db fails{rebased}: the game's own fatal: {run.fatal!r}")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
