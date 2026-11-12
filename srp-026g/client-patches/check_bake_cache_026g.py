#!/usr/bin/env python3
"""Run patch 70 (the Rust patch bake_cache: the baked camouflage and decal
textures kept from game to game, and made once while asked for again) in the
default build on a made-up baker, folder and queries.

    python check_bake_cache_026g.py <original survarium.exe>

Requires Python `unicorn` (2.1.4) and the arena's tools (`nix develop
./srp-026g`: the build links the arena). The exe's sections are loaded as
they are, the arena with them; the baker is the game's own static
(0xA264E80), its list at +230h set up as its static constructor (0x12CCE0)
sets it. Every site runs from the game's own code:

- the folder (game::game from its `lea eax,[esp+470h]` before its `call
  0x1E8DA0` (0x1E944E) to 0x1E945A, [esp+470h] the folder's buffer_string,
  the Rust lohook's site; the game's imports a made-up folder
  through their IAT slots, the stock delete 0x1E8DA0 a stub that empties it;
  the game data made-up patches, their file tables laid out as
  `resources-db` writes them, VirtualAlloc a stub of fresh pages): a folder
  stamped with this build's bake format and the patches' fingerprint (the
  check computes its own, in Python, from the tables) keeps its whole DDS
  files and they are in the baker's list (CRC 0, their size, an age each,
  the list's bytes grown by theirs), a file cut short and the files past the
  list's room less 512 deleted, the stub not called, the tables read in pages
  given back, the list in 4096 records of pages kept (begin, end, capacity),
  a line to srp_patches.log; a release whose patch has only item stats keeps it; a
  patch with a texture, a decal item or one that cannot be read, another
  format, a stamp of other bytes, no stamp or no folder: deleted (the stub)
  and stamped anew; revision 54's stamp (before formats: format 1's) and
  revision 53's are deleted too; with no resources.db where the game
  mounts it from, deleted and not stamped. The original calls the stub every time;
- a kept file's first load (check_loaded_baked_texture 0x131B40, the game's
  own, with the CRC 0x3C0F70 a stub as in check_baked_texture_026g.py): it is
  used and its record takes the CRC read; a second load of other bytes is
  erased and made again, as the original does any record;
- the look-up (translate_query from 0x12D4C5, its `lea edx,[esp+1B8h] / push
  edx / call`, to its `test al,al` at 0x12D4D2, or out of translate_query by
  its own end at 0x12D42D: `ret 4` to the caller, its model instance
  released, its registers back): the first query of a path goes on to the
  game's look-up 0x131ED0 (the game's own; a kept file is found); a second
  and third query of that path, while it is under way, leave translate_query
  with no look-up and no bake, their model instance kept; another path, the
  same path of another type, and a 17th query go on;
- the texture made (process_compression from its `call 0x420789` at
  0x12F18F through the Rust lohook's site after it, `push eax / mov
  eax,108h` at 0x12F194, to the allocation's call at 0x12F19A, its frame's
  texture +10h, query +604h, surface +610h, type +620h;
  on_baked_texture_loaded from 0x12DDB4 through 0x12DDB9 to 0x12DDBF, edi
  the texture, ebp its frame): each waiting query's surface slot of the type
  holds the texture (one reference more each), its query gets a resource of
  the game's (0x12DE30 on the allocator's 108h bytes: stubs that record) and
  is finished with success (finish_query, its entry hooked, the function a
  stub from 0xF0D06 that records), its model instance released; the
  call's ecx + 8 pushed, eax 108h and esi kept; with the texture not in the owner's slot
  nothing is shared; a loaded texture is held (one reference more): a later
  query of its path (a respawn) gets it at once with no look-up, and past
  256 MB the least used is let go;
- the CRC-32 step (0x42050, run from its entry as its 43 callers call it; a
  separate emulator with no hook on every instruction, SSE on as Windows has
  it): the build's against the original's, with the game's own table
  (0x1C210 run first), on random buffers of 0 to 10 MB from starts 0 to 3
  bytes past a dword and random CRCs, on a processor model without
  PCLMULQDQ (Penryn: the sliced way) and one with it (Westmere: the fold):
  the CRC and eax, ecx, edx, ebx, esi, edi, ebp and esp after it the same;
  and how many instructions each runs on 256 KB;
- a folder whose delete leaves a file (held open): not stamped;
- the trim (remove_unwanted_files 0x131F90 from its entry, as
  process_compression calls it after a bake's push_back: the cook pushed,
  `ret 4`; GetDiskFreeSpaceExW a stub): records of 3 GiB stay; past it the
  oldest record with no load under way goes with its file (DeleteFileA of
  the folder's physical path and its name), the last record in its place
  with its own path's pointers, the list's bytes less its file's, a line to
  srp_patches.log; with less than 50 MB free on the drive, down to 50 MB,
  the loading record left; a list with no room for the next record lets
  its oldest go; with no folder known (the baker's own 1024 records) the
  record goes and no file is deleted; ebx, esi, edi, ebp and the stack kept;
- an owner that ends otherwise (finish_query 0xF0D00 called as the game
  calls it, stdcall, with an error, or a success nothing shared): the
  function, then each waiting query made again (translate_query 0x12D1D0, a
  stub that records the cook and the query), its reference released; any
  other query: the function with the same arguments, the registers kept.
"""
import fnmatch
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE, UC_HOOK_INTR
from unicorn.x86_const import UC_CPU_X86_PENRYN, UC_CPU_X86_WESTMERE, UC_X86_REG_CR0, UC_X86_REG_CR4, UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import patch_026g as base

COOK = 0xA264E80                                   # the baker (camouflage_decal_texture_type_baker_cook)
LIST, CAPACITY, BYTES, AGES = 0xA2650B0, 0xA2AE0BC, 0xA2AE0BC, 0x2260920
STORAGE = 0xA2650BC
RECORD, CRC, SIZE, AGE, LOADING = 0x124, 0x114, 0x118, 0x11C, 0x120
START_SITE, START_BACK, WIPE = 0x1E944E, 0x1E945A, 0x1E8DA0     # the site: lea eax,[esp+470h] / call WIPE
LOOKUP_FROM, LOOKUP_TEST, NOT_BAKED_RET = 0x12D4C5, 0x12D4D2, None
FIND_RECORD, TRANSLATE, FINISH, FINISH_BODY = 0x131ED0, 0x12D1D0, 0xF0D00, 0xF0D06
BAKED_SITE, BAKED_BACK, LOADED_SITE, LOADED_BACK = 0x12F18F, 0x12F19A, 0x12DDB4, 0x12DDBF   # past push eax / mov eax,108h
ALLOCATE, RESOURCE, SET_RESOURCE, RELEASE_TEXTURE, DESTROY_MODEL = 0xE4F90, 0x12DE30, 0xF0080, 0x3BEE00, 0xE0210
CHECK, CRC_OF, LOG_FILTER, ASSERTION = 0x131B40, 0x3C0F70, 0x111860, 0x113450
TRIM, LIST_RECORDS, STOCK_RECORDS, FOLDER_BYTES, DISK_LOW = 0x131F90, 4096, 1024, 3 << 30, 50 << 20
LOG_FILTER_ON, IS_DEBUGGER_PRESENT = 0x1F1D60C, 0x870194
NAMES = 0xAC3334
CRC_STEP, TABLE = 0x42050, 0x1C210                   # the CRC-32 step; the table's maker (0x225FD68)
BUFFER, CRC_STATE = 0x40000000, 0x31000000 + 0xF0000
IAT = {"FindFirstFileA": 0x870238, "FindNextFileA": 0x87023C, "FindClose": 0x870228, "CreateFileA": 0x870048,
       "ReadFile": 0x870064, "WriteFile": 0x870108, "CloseHandle": 0x870040, "DeleteFileA": 0x87021C,
       "CreateDirectoryA": 0x870220, "GetTickCount": 0x870090, "GetCurrentThreadId": 0x870118,
       "VirtualAlloc": 0x87015C, "VirtualFree": 0x870160, "GetDiskFreeSpaceExW": 0x870174}
POPS = {"FindFirstFileA": 8, "FindNextFileA": 8, "FindClose": 4, "CreateFileA": 28, "ReadFile": 20,
        "WriteFile": 20, "CloseHandle": 4, "DeleteFileA": 4, "CreateDirectoryA": 8, "GetTickCount": 0,
        "GetCurrentThreadId": 0, "VirtualAlloc": 16, "VirtualFree": 12, "GetDiskFreeSpaceExW": 16}
OBJECTS, STACK, STUBS, STOP, PAGES = 0x30000000, 0x31000000, 0x32000000, 0x32100000, 0x70000000
FOLDER = "c:\\users\\player\\documents\\survarium\\streamed_textures"
STAMP = "srp_baked_textures.txt"
PREFIX = "user_data/streamed_textures/"
INVALID = 0xFFFFFFFF
# where the game mounts its data from (its working folder binaries\x86)
DATA = "..\\..\\"


# ---- the game data: resources.db's patches' file tables (srp-026g/crates/resources-db) ----
def archive(entries):
    """A patch of `entries` (pre-order: a folder's path, None, before its own; a file's path,
    (size, crc)): the header and a file table as the game's archives lay it out (`resources-db`)."""
    folder_base, file_base, root_base, flags_at, name_at, next_at = 16, 24, 936 + 16, 48, 51, 24
    table = bytearray(root_base + 64)
    struct.pack_into("<H", table, root_base + flags_at, 1)
    nodes = {"": [root_base, 0]}
    for path, file in entries:
        parent, _, name = path.rpartition("/")
        start = len(table)
        base_at = start + (file_base if file else folder_base)
        table += bytes(base_at + name_at + len(name) + 1 + 8 - len(table))
        if file:
            struct.pack_into("<I", table, start, file[0])
            struct.pack_into("<I", table, start + 16, file[1])
        struct.pack_into("<H", table, base_at + flags_at, 4 if file else 1)
        table[base_at + name_at:base_at + name_at + len(name)] = name.encode()
        parent_base, last = nodes[parent]
        struct.pack_into("<Q", table, (parent_base - folder_base) if last == 0 else (last + next_at), base_at)
        nodes[parent][1] = base_at
        nodes[path] = [base_at, 0]
    return b"little-endian\0\0\0" + struct.pack("<II", len(entries), len(table)) + bytes(table)


def fnv(data, h=0xCBF29CE484222325):
    for b in data:
        h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


BAKE_FOLDERS = ("textures/", "models/", "material_instances/", "shaders/", "gameplay/items/upgrades/")


def fingerprint(patches):
    """The game data's fingerprint a bake reads (bake_cache/inputs.rs) of {n: entries, or None for a
    patch that cannot be read}: the files under the bake's folders, their path, size and crc."""
    h = fnv(b"")
    for n in sorted(patches):
        if patches[n] is None:
            tag, value = 3, 0
        else:
            files = [(p, f) for p, f in patches[n] if f and p.startswith(BAKE_FOLDERS)]
            if not files:
                continue
            value = fnv(b"".join(p.encode() + b"\0" + struct.pack("<II", *f) for p, f in files))
            tag = 1
        h = fnv(struct.pack("<IIII", n, tag, value & 0xFFFFFFFF, value >> 32), h)
    return h


BAKE_FORMAT = 2                                # bake_cache.rs's


def made(inputs, form=BAKE_FORMAT):
    return f"Skhid baked textures, format {form}, inputs {inputs:016x}\r\n".encode()


TEXTURES = [("textures", None), ("textures/character", None), ("textures/character/a.dds", (1000, 0x1234))]
STATS = [("gameplay", None), ("gameplay/items", None), ("gameplay/items/weapons", None),
         ("gameplay/items/weapons/ak.options", (10, 0x55))]
DECALS = [("gameplay", None), ("gameplay/items", None), ("gameplay/items/upgrades", None),
          ("gameplay/items/upgrades/decals", None), ("gameplay/items/upgrades/decals/d.options", (12, 0x66))]


def dds(width, height, linear, fourcc=b"DXT1", cut=0):
    header = bytearray(128)
    struct.pack_into("<4sIIIIIII", header, 0, b"DDS ", 124, 0xA1007, height, width, linear, 0, 9)
    header[84:88] = fourcc
    return bytes(header) + bytes(linear - cut)


class Game:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at in (OBJECTS, STACK, STUBS, STOP):
            emu.mem_map(at, 0x100000, UC_PROT_ALL)
        emu.mem_map(PAGES, 0x1000000, UC_PROT_ALL)
        self.pages = PAGES
        # the baker's list as its static constructor makes it (0x12CD39..0x12CDA6)
        self.word(LIST, STORAGE)
        self.word(LIST + 4, STORAGE)
        self.word(LIST + 8, CAPACITY)
        self.word(LOG_FILTER_ON, 1)
        self.stubs = {}
        for i, name in enumerate(IAT):
            self.word(IAT[name], STUBS + 0x10 * i)
            self.stubs[STUBS + 0x10 * i] = name
        self.word(IS_DEBUGGER_PRESENT, STUBS + 0x200)
        self.stubs.update({STUBS + 0x200: "IsDebuggerPresent", WIPE: "wipe", TRANSLATE: "translate",
                           FINISH_BODY: "finish", ALLOCATE: "allocate", RESOURCE: "resource",
                           SET_RESOURCE: "set resource", RELEASE_TEXTURE: "release texture",
                           DESTROY_MODEL: "destroy model", CRC_OF: "crc", LOG_FILTER: "log filter",
                           ASSERTION: "assertion"})
        self.calls = []
        self.files, self.dirs = {}, set()
        self.handles, self.next_handle = {}, 0x100
        self.crc, self.heap, self.now = None, OBJECTS + 0x80000, 1000
        self.stop_at = set()
        self.locked = set()                          # files another game holds open: not deleted
        self.unopenable = set()                      # files CreateFileA refuses (shared otherwise)
        self.disk_free = 100 << 30                   # the drive's free bytes; None: Windows does not say
        emu.hook_add(UC_HOOK_CODE, self.code)
        emu.hook_add(UC_HOOK_INTR, lambda emu, n, _: (self.calls.append(("int3",)), emu.emu_stop()))

    # memory
    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def ascii(self, at, n=400):
        return bytes(self.emu.mem_read(at, n)).split(b"\0")[0].decode("latin-1")

    def string(self, text, at):
        """A buffer_string of `text` at `at` (begin, end, capacity end, its text)."""
        raw = text.encode("latin-1")
        self.word(at, at + 0xC)
        self.word(at + 4, at + 0xC + len(raw))
        self.word(at + 8, at + 0xC + 0x104)
        self.emu.mem_write(at + 0xC, raw + b"\0")
        return at

    def returns(self, value, pop=0):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        self.emu.reg_write(UC_X86_REG_EAX, value & 0xFFFFFFFF)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pop)

    # the game's functions this needs, and Windows's
    def code(self, emu, address, size, _):
        if address in self.stop_at:
            emu.emu_stop()
            return
        name = self.stubs.get(address)
        if name is None:
            return
        sp = emu.reg_read(UC_X86_REG_ESP)
        arg = lambda n: self.read(sp + 4 + 4 * n)
        reg = emu.reg_read
        if name in POPS:
            self.returns(self.windows(name, [arg(i) for i in range(POPS[name] // 4)]), POPS[name])
        elif name == "IsDebuggerPresent":
            self.returns(0)
        elif name == "wipe":                         # eax the folder's buffer_string; nothing popped
            folder = self.ascii(self.read(reg(UC_X86_REG_EAX)))
            self.calls.append(("wipe", folder))
            for path in [p for p in self.files if p.startswith(folder + "\\") and p not in self.locked]:
                del self.files[path]
            if not any(p.startswith(folder + "\\") for p in self.files):
                self.dirs.discard(folder)
            self.returns(0)
        elif name == "translate":                    # thiscall (the query); `ret 4`
            self.calls.append(("translate", reg(UC_X86_REG_ECX), arg(0)))
            self.returns(0, 4)
        elif name == "finish":                       # finish_query past its prologue: ebp its frame
            ebp = reg(UC_X86_REG_EBP)
            self.calls.append(("finish", self.read(ebp + 8), self.read(ebp + 12), self.read(ebp + 16),
                               self.read(ebp + 20)))
            emu.reg_write(UC_X86_REG_ESP, ebp + 8 + 16)
            emu.reg_write(UC_X86_REG_EBP, self.read(ebp))
            emu.reg_write(UC_X86_REG_EIP, self.read(ebp + 4))
        elif name == "allocate":                     # esi the allocator, eax the size, the name; `ret 4`
            self.calls.append(("allocate", reg(UC_X86_REG_ESI), reg(UC_X86_REG_EAX), arg(0)))
            at, self.heap = self.heap, self.heap + 0x200
            self.returns(at, 4)
        elif name == "resource":                     # the texture's resource made at eax
            self.word(reg(UC_X86_REG_EAX) + 0xD0, 0)
            self.calls.append(("resource", reg(UC_X86_REG_EAX)))
            self.returns(reg(UC_X86_REG_EAX))
        elif name == "set resource":                 # edx the query; the pointer, the allocator, the size; `ret 0Ch`
            self.calls.append(("set resource", reg(UC_X86_REG_EDX), arg(0), arg(1), arg(2)))
            self.returns(0, 12)
        elif name == "release texture":
            self.calls.append(("release texture", reg(UC_X86_REG_EAX)))
            self.returns(0, 4)
        elif name == "destroy model":
            self.calls.append(("destroy model", reg(UC_X86_REG_ECX)))
            self.returns(0)
        elif name == "crc":                          # the loaded resource by value: `ret 4`
            self.returns(self.crc, 4)
        elif name == "log filter":
            self.returns(0)
        elif name == "assertion":
            emu.mem_write(arg(0), b"\x01")
            self.calls.append(("assertion",))
            self.returns(0)

    def windows(self, name, a):
        if name == "GetCurrentThreadId":           # the tables' lock (vostok::Lock): the one thread
            return 0x1D0C
        if name == "GetTickCount":
            return self.now
        if name == "VirtualAlloc":                   # fresh zeroed pages (64 KiB aligned)
            at, size = self.pages, (a[1] + 0xFFFF) & ~0xFFFF
            self.pages += size
            self.calls.append(("VirtualAlloc", a[1]))
            return at
        if name == "VirtualFree":
            self.calls.append(("VirtualFree", a[0]))
            return 1
        if name == "GetDiskFreeSpaceExW":            # the wide path, the caller's free bytes, two totals
            root = bytes(self.emu.mem_read(a[0], 16)).decode("utf-16-le").split("\0")[0]
            self.calls.append((name, root, a[2], a[3]))
            if self.disk_free is None:
                return 0
            self.emu.mem_write(a[1], struct.pack("<Q", self.disk_free))
            return 1
        self.calls.append((name,) + tuple(a[:1]))
        if name == "CreateFileA":
            path, access, disposition = self.ascii(a[0]), a[1], a[4]
            folder = path.rsplit("\\", 1)[0]
            if disposition == 3:                     # OPEN_EXISTING
                if path not in self.files or path in self.unopenable:
                    return INVALID
            elif disposition == 2 and folder in self.dirs:
                self.files[path] = b""
            elif disposition == 4 and path == "srp_patches.log":     # OPEN_ALWAYS, appended to
                self.files.setdefault(path, b"")
            else:
                return INVALID
            handle, self.next_handle = self.next_handle, self.next_handle + 4
            self.handles[handle] = [path, len(self.files[path]) if disposition == 4 else 0]
            return handle
        if name in ("ReadFile", "WriteFile"):
            path, at = self.handles[a[0]]
            if name == "ReadFile":
                got = self.files[path][at:at + a[2]]
                self.emu.mem_write(a[1], got)
            else:
                got = bytes(self.emu.mem_read(a[1], a[2]))
                self.files[path] = self.files[path][:at] + got
            self.handles[a[0]][1] += len(got)
            self.word(a[3], len(got))
            return 1
        if name in ("CloseHandle", "FindClose"):
            self.handles.pop(a[0], None)
            return 1
        if name == "DeleteFileA":
            path = self.ascii(a[0])
            if path in self.locked:
                return 0
            return 1 if self.files.pop(path, None) is not None else 0
        if name == "CreateDirectoryA":
            self.dirs.add(self.ascii(a[0]))
            return 1
        if name == "FindFirstFileA":
            pattern = self.ascii(a[0])
            folder, mask = pattern.rsplit("\\", 1)
            assert mask in ("*.dds", "resources.db", "resources.db.patch_*"), pattern
            names = sorted(p[len(folder) + 1:] for p in self.files
                           if p.startswith(folder + "\\") and fnmatch.fnmatch(p[len(folder) + 1:].lower(), mask)
                           and "\\" not in p[len(folder) + 1:])
            names += sorted(d[len(folder) + 1:] + "\\" for d in self.dirs
                            if d.startswith(folder + "\\") and fnmatch.fnmatch(d[len(folder) + 1:].lower(), mask))
            if not names:
                return INVALID
            handle, self.next_handle = self.next_handle, self.next_handle + 4
            self.handles[handle] = [folder, names]
            return handle if self.find_data(handle, a[1]) else INVALID
        if name == "FindNextFileA":
            return 1 if self.find_data(a[0], a[1]) else 0
        raise AssertionError(name)

    def find_data(self, handle, at):
        folder, names = self.handles[handle]
        if not names:
            return False
        name = names.pop(0)
        directory = name.endswith("\\")
        name = name.rstrip("\\")
        size = 0 if directory else len(self.files[folder + "\\" + name])
        record = bytearray(320)
        struct.pack_into("<I", record, 0, 0x10 if directory else 0x80)
        struct.pack_into("<II", record, 28, 0, size)
        record[44:44 + len(name)] = name.encode()
        self.emu.mem_write(at, bytes(record))
        return True

    def data(self, patches, db=True):
        """The game data where the game mounts it from: resources.db and the patches {n: entries}."""
        if db:
            self.files[DATA + "resources.db"] = b"little-endian\0"
        for n, entries in patches.items():
            self.files[DATA + f"resources.db.patch_{n}"] = archive(entries or [])
            if entries is None:
                self.unopenable.add(DATA + f"resources.db.patch_{n}")

    # running
    def run(self, start, sp, regs, until=None, count=2000000):
        for r, v in regs.items():
            self.emu.reg_write(r, v & 0xFFFFFFFF)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.stop_at = set(until or ())
        try:
            self.emu.emu_start(start, STOP, count=count)
        except Exception:
            print("  emulation stopped at", hex(self.emu.reg_read(UC_X86_REG_EIP)), "calls", self.calls[-6:])
            raise
        return self.emu.reg_read(UC_X86_REG_EIP), self.emu.reg_read(UC_X86_REG_ESP)

    def records(self):
        out = []
        for at in range(self.read(LIST), self.read(LIST + 4), RECORD):
            assert self.read(at) == at + 0xC and self.read(at + 8) == at + 0xC + 0x104
            assert self.read(at + 4) == at + 0xC + len(self.ascii(at + 0xC))
            out.append((self.ascii(at + 0xC), self.read(at + CRC), self.read(at + SIZE), self.read(at + AGE),
                        self.emu.mem_read(at + LOADING, 1)[0], bytes(self.emu.mem_read(at + 0x110, 1))))
        return out

    # the sites
    def start(self):
        """game::game from before its delete's call (the folder's buffer_string at [esp+470h]) to
        0x1E945A."""
        sp = STACK + 0x8000
        string = self.string(FOLDER, sp + 0x470)
        regs = {UC_X86_REG_EAX: string, UC_X86_REG_EBX: 0x1111, UC_X86_REG_ESI: 0x2222, UC_X86_REG_EDI: 0x3333,
                UC_X86_REG_EBP: 0x4444}
        eip, esp = self.run(START_SITE, sp, regs, until=[START_BACK], count=200000000)
        kept = all(self.emu.reg_read(r) == v for r, v in regs.items() if r != UC_X86_REG_EAX)
        return eip == START_BACK and esp == sp and kept

    def lookup(self, path, query, surface, parameters, model):
        """translate_query from 0x12D4C5: "on" (to its test at 0x12D4D2, al
        the look-up's) or "waits" (out of it by its end, `ret 4`)."""
        ebp = STACK + 0x40000
        frame = ebp - 0x500
        self.emu.mem_write(frame, bytes(0x500))
        for at, value in ((frame, 0xED1), (frame + 4, 0xE51), (frame + 8, 0xEB1), (frame + 0x1C, parameters),
                          (ebp, 0xEB9), (ebp + 4, STOP), (ebp + 8, query)):
            self.word(at, value)
        self.string(path, frame + 0x1B8)
        before = len(self.calls)
        found = []
        hook = self.emu.hook_add(UC_HOOK_CODE, lambda emu, a, s, _: found.append(a), begin=FIND_RECORD,
                                 end=FIND_RECORD)
        eip, esp = self.run(LOOKUP_FROM, frame, {UC_X86_REG_EAX: COOK, UC_X86_REG_EBX: surface,
                                                 UC_X86_REG_EDI: model, UC_X86_REG_ESI: 0x5555,
                                                 UC_X86_REG_EBP: ebp}, until=[LOOKUP_TEST])
        self.emu.hook_del(hook)
        if eip == LOOKUP_TEST and esp == frame and self.emu.reg_read(UC_X86_REG_EBX) == surface \
                and self.emu.reg_read(UC_X86_REG_EDI) == model and self.emu.reg_read(UC_X86_REG_EBP) == ebp:
            return "on", self.emu.reg_read(UC_X86_REG_EAX) & 0xFF, len(found), self.calls[before:]
        regs = [self.emu.reg_read(r) for r in (UC_X86_REG_EDI, UC_X86_REG_ESI, UC_X86_REG_EBX, UC_X86_REG_EBP)]
        if eip == STOP and esp == ebp + 12 and regs == [0xED1, 0xE51, 0xEB1, 0xEB9]:
            return "waits", None, len(found), self.calls[before:]
        return f"lost at {eip:#x} esp {esp:#x}", None, len(found), self.calls[before:]

    def baked(self, query, texture, surface, kind):
        """process_compression from 0x12F18F to 0x12F19A (the name pushed, eax 108h)."""
        frame = STACK + 0x60000
        self.emu.mem_write(frame, bytes(0x700))
        for at, value in ((0x10, texture), (0x604, query), (0x610, surface), (0x620, kind)):
            self.word(frame + at, value)
        before = len(self.calls)
        eip, esp = self.run(BAKED_SITE, frame, {UC_X86_REG_ECX: NAMES, UC_X86_REG_ESI: 0xA11C, UC_X86_REG_EBX: 0xB0,
                                                UC_X86_REG_EDI: 0xD0, UC_X86_REG_EBP: query}, until=[BAKED_BACK])
        ok = eip == BAKED_BACK and esp == frame - 4 and self.read(esp) == NAMES + 8 \
            and self.emu.reg_read(UC_X86_REG_EAX) == 0x108 \
            and [self.emu.reg_read(r) for r in (UC_X86_REG_ESI, UC_X86_REG_EBX, UC_X86_REG_EDI, UC_X86_REG_EBP)] \
            == [0xA11C, 0xB0, 0xD0, query]
        return ok, self.calls[before:]

    def loaded(self, query, texture, surface, parameters):
        """on_baked_texture_loaded from 0x12DDB4 to 0x12DDBF (the name pushed, eax 108h)."""
        ebp = STACK + 0x70000
        sp = ebp - 0x300
        for at, value in ((ebp + 0xC, parameters), (ebp + 0x10, surface), (ebp + 0x14, query)):
            self.word(at, value)
        before = len(self.calls)
        eip, esp = self.run(LOADED_SITE, sp, {UC_X86_REG_ECX: NAMES, UC_X86_REG_ESI: 0xA11C, UC_X86_REG_EDI: texture,
                                              UC_X86_REG_EBX: surface, UC_X86_REG_EBP: ebp}, until=[LOADED_BACK])
        ok = eip == LOADED_BACK and esp == sp - 4 and self.read(esp) == NAMES + 8 \
            and self.emu.reg_read(UC_X86_REG_EAX) == 0x108 \
            and [self.emu.reg_read(r) for r in (UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBX, UC_X86_REG_EBP)] \
            == [0xA11C, texture, surface, ebp]
        return ok, self.calls[before:]

    def finish(self, query, result):
        """finish_query called as the game calls it: stdcall (query, result, 1, 0)."""
        sp = STACK + 0x80000
        for i, value in enumerate((STOP, query, result, 1, 0)):
            self.word(sp + 4 * i, value)
        before = len(self.calls)
        regs = {UC_X86_REG_EBX: 0xB1, UC_X86_REG_ESI: 0x51, UC_X86_REG_EDI: 0xD1, UC_X86_REG_EBP: 0xEB}
        eip, esp = self.run(FINISH, sp, regs)
        ok = eip == STOP and esp == sp + 20 and all(self.emu.reg_read(r) == v for r, v in regs.items())
        return ok, self.calls[before:]

    def push(self, name, size, loading=0):
        """A record at the list's end as the baker's push_back (0x132640) leaves one, of a file of
        `size` bytes in the folder (its bytes here: one)."""
        at = self.read(LIST + 4)
        assert at + RECORD <= self.read(LIST + 8)
        self.string(PREFIX + name, at)
        self.emu.mem_write(at + 0x110, b"/")
        age = self.read(AGES) + 1
        self.word(AGES, age)
        for field, value in ((CRC, 0xC0DE0000 + age), (SIZE, size), (AGE, age)):
            self.word(at + field, value)
        self.emu.mem_write(at + LOADING, bytes([loading]))
        self.word(LIST + 4, at + RECORD)
        self.word(BYTES, self.read(BYTES) + size)
        self.files[FOLDER + "\\" + name] = b"x"
        return (PREFIX + name, 0xC0DE0000 + age, size, age, loading, b"/")

    def trim(self):
        """remove_unwanted_files from its entry as process_compression calls it (0x12F405: the cook
        pushed): whether it left by its `ret 4`, ebx, esi, edi and ebp kept."""
        sp = STACK + 0x90000
        self.word(sp, STOP)
        self.word(sp + 4, COOK)
        regs = {UC_X86_REG_EBX: 0x1111, UC_X86_REG_ESI: COOK, UC_X86_REG_EDI: 0x3333, UC_X86_REG_EBP: 0x4444}
        eip, esp = self.run(TRIM, sp, regs, count=200000000)
        return eip == STOP and esp == sp + 8 and all(self.emu.reg_read(r) == v for r, v in regs.items())

    def check_loaded(self, path, crc):
        """check_loaded_baked_texture (the baker, no resource, the path): its al."""
        self.crc = crc
        sp = STACK + 0x90000
        string = self.string(path, OBJECTS + 0x71000)
        for i, value in enumerate((STOP, COOK, 0, string)):
            self.word(sp + 4 * i, value)
        eip, esp = self.run(CHECK, sp, {})
        assert eip == STOP and esp == sp + 16, (hex(eip), hex(esp))
        return self.emu.reg_read(UC_X86_REG_EAX) & 0xFF


class Crc:
    """The exe's sections alone, with no hook on every instruction: the CRC-32
    step on large buffers at full speed, on a processor model (`cpu`, unicorn's: what CPUID tells)."""
    def __init__(self, data, size, cpu=None):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        if cpu is not None:
            emu.ctl_set_cpu_model(cpu)
        # SSE on, as Windows has it (CR4.OSFXSR, OSXMMEXCPT; CR0.MP, not EM)
        emu.reg_write(UC_X86_REG_CR4, emu.reg_read(UC_X86_REG_CR4) | 0x600)
        emu.reg_write(UC_X86_REG_CR0, (emu.reg_read(UC_X86_REG_CR0) | 0x2) & ~0x4)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            begin = base.IMAGE_BASE + rva
            emu.mem_map(begin, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(begin, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at in (STACK, STOP):
            emu.mem_map(at, 0x100000, UC_PROT_ALL)
        emu.mem_map(BUFFER, (size + 0x2000 + 0xFFF) & ~0xFFF, UC_PROT_ALL)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def call(self, start, regs, stack):
        sp = STACK + 0xA0000
        for i, value in enumerate([STOP] + stack):
            self.word(sp + 4 * i, value)
        for r, v in regs.items():
            self.emu.reg_write(r, v & 0xFFFFFFFF)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        try:
            self.emu.emu_start(start, STOP, count=400000000)
        except Exception:
            print('    stopped at', hex(self.emu.reg_read(UC_X86_REG_EIP)), 'esi', hex(self.emu.reg_read(UC_X86_REG_ESI)),
                  'edi', hex(self.emu.reg_read(UC_X86_REG_EDI)), 'edx', hex(self.emu.reg_read(UC_X86_REG_EDX)))
            raise
        assert self.emu.reg_read(UC_X86_REG_EIP) == STOP
        return self.emu.reg_read(UC_X86_REG_ESP) - sp

    def crc(self, start, length, crc):
        """The CRC-32 step on `length` bytes at `start` from `crc`: the CRC, the stack's change and
        the registers after it."""
        self.word(CRC_STATE, crc)
        regs = {UC_X86_REG_EAX: CRC_STATE, UC_X86_REG_EDX: start, UC_X86_REG_EBX: 0xB2, UC_X86_REG_ESI: 0x52,
                UC_X86_REG_EDI: 0xD2, UC_X86_REG_EBP: 0xE2, UC_X86_REG_ECX: 0xC2}
        moved = self.call(CRC_STEP, regs, [length])
        return (self.read(CRC_STATE), moved,
                [self.emu.reg_read(r) for r in (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX,
                                                 UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)])

    def count(self, start, length):
        """How many instructions the CRC-32 step runs on `length` bytes."""
        counted = [0]

        def one(emu, address, size, _):
            counted[0] += 1
        hook = self.emu.hook_add(UC_HOOK_CODE, one)
        self.crc(start, length, 0xFFFFFFFF)
        self.emu.hook_del(hook)
        return counted[0]


class Objects:
    """Made-up queries, surfaces, parameters, model instances and textures."""
    def __init__(self, game):
        self.game, self.at = game, OBJECTS

    def new(self, size=0x800):
        at, self.at = self.at, self.at + size
        self.game.emu.mem_write(at, bytes(size))
        return at

    def request(self, kind):
        """(query, surface, parameters, model) of one bake request."""
        model = self.new(0x100)
        self.game.word(model + 0xD0, 5)
        parameters = self.new(0x800)
        self.game.word(parameters + 0x7A0, kind)
        self.game.word(parameters + 0x790, model)
        return self.new(0x100), self.new(0x40), parameters, model

    def texture(self, references=1):
        texture = self.new(0x40)
        self.game.word(texture + 4, references)
        return texture


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    # the players' game data: patches with textures (12, 54), item stats only (53)
    data = {12: TEXTURES, 53: STATS, 54: [("textures", None), ("textures/b.dds", (500, 0x99))] + DECALS}
    inputs = fingerprint(data)
    stamp = made(inputs)
    a, b = dds(2048, 1024, 1398097), dds(1024, 1024, 1398097, b"DXT5")

    print("the folder (game::game from 0x1E944E to 0x1E945A)")
    for label, exe in (("the original", original), ("the build", built)):
        game = Game(bytes(exe))
        game.data(data)
        game.dirs.add(FOLDER)
        game.files[FOLDER + "\\" + STAMP] = stamp
        game.files[FOLDER + "\\a_s1d.dds"] = a
        game.files[FOLDER + "\\b_s1n.dds"] = b
        game.files[FOLDER + "\\cut_s1d.dds"] = dds(2048, 1024, 1398097, cut=4096)
        game.files[FOLDER + "\\notes.txt"] = b"x"
        game.dirs.add(FOLDER + "\\folder.dds")
        ok = game.start()
        wiped = [c for c in game.calls if c[0] == "wipe"]
        records = game.records()
        if label == "the original":
            report(ok and wiped == [("wipe", FOLDER)] and records == [],
                   "the original: a stamped folder is deleted (0x1E8DA0), the list empty")
            continue
        want = [(PREFIX + "a_s1d.dds", 0, len(a), 1, 0, b"/"), (PREFIX + "b_s1n.dds", 0, len(b), 2, 0, b"/")]
        tables = [c for c in game.calls if c[0] in ("VirtualAlloc", "VirtualFree")]
        report(ok and not wiped and records == want and game.read(BYTES) == len(a) + len(b)
               and game.read(AGES) == 2 and FOLDER + "\\cut_s1d.dds" not in game.files
               and FOLDER + "\\notes.txt" in game.files and game.files[FOLDER + "\\" + STAMP] == stamp
               and tables[0] == ("VirtualAlloc", LIST_RECORDS * RECORD) and len(tables) == 7
               and all(c[0] == ("VirtualAlloc", "VirtualFree")[i % 2] for i, c in enumerate(tables[1:]))
               and (game.read(LIST), game.read(LIST + 8)) == (PAGES, PAGES + LIST_RECORDS * RECORD),
               f"a folder stamped with this format and the game data's fingerprint ({stamp.decode().strip()}, the "
               f"three patches' tables read in pages given back after): kept, {[r[0] for r in records]} in the "
               f"list (CRC 0, sizes, ages 1 and 2, {game.read(BYTES)} bytes), the file cut short deleted, the rest "
               f"left, no delete; the list in {LIST_RECORDS} records of pages kept, not the baker's "
               f"{STOCK_RECORDS}")
        log = game.files.get("srp_patches.log", b"")
        report(log == b"srp: bake_cache: the baked textures kept: " + stamp,
               f"srp_patches.log: {log.decode().strip()!r}")

        # a kept file's loads (check_loaded_baked_texture, the game's own)
        first = game.check_loaded(PREFIX + "a_s1d.dds", 0x64308FFD)
        report(first == 1 and game.records()[0][1] == 0x64308FFD and game.records()[0][3] == 3,
               "a kept file's first load is used and its record takes the CRC read (age renewed)")
        again = game.check_loaded(PREFIX + "a_s1d.dds", 0x0BADF11E)
        report(again == 0 and [r[0] for r in game.records()] == [PREFIX + "b_s1n.dds"]
               and game.read(BYTES) == len(b) and ("assertion",) not in game.calls,
               "a load of other bytes after it: erased and made again, as the original does")

    def start(stamped, patches, db=True):
        """A folder with `stamped` (none: no stamp) and a_s1d.dds, the game data `patches`: whether it
        was kept (a_s1d.dds in the list, no delete) and the stamp after."""
        game = Game(bytes(built))
        game.data(patches, db)
        game.dirs.add(FOLDER)
        if stamped is not None:
            game.files[FOLDER + "\\" + STAMP] = stamped
        game.files[FOLDER + "\\a_s1d.dds"] = a
        ok = game.start()
        wiped = [c for c in game.calls if c[0] == "wipe"]
        kept = not wiped and [r[0] for r in game.records()] == [PREFIX + "a_s1d.dds"]
        gone = wiped == [("wipe", FOLDER)] and game.records() == [] and FOLDER in game.dirs \
            and FOLDER + "\\a_s1d.dds" not in game.files
        assert ok and kept != gone, (ok, kept, gone, game.calls[-8:])
        return kept, game.files.get(FOLDER + "\\" + STAMP)

    stats55 = dict(data | {55: STATS})
    texture55 = dict(data | {55: TEXTURES})
    cases = (
        ("a release whose patch has no file a bake reads (55: item stats)", stamp, stats55, True, stamp),
        ("a release whose patch has a decal item (55)", stamp, dict(data | {55: DECALS}), False,
         made(fingerprint(dict(data | {55: DECALS})))),
        ("a release whose patch has a texture (55)", stamp, texture55, False, made(fingerprint(texture55))),
        ("a release whose patch cannot be read (55)", stamp, dict(data | {55: None}), False,
         made(fingerprint(dict(data | {55: None})))),
        ("another format's folder (format 1)", made(inputs, 1), data, False, stamp),
        ("revision 54's folder (before format stamps: format 1's), no patch past 54 with bake data",
         b"Skhid baked textures, revision 54\r\n", stats55, False, stamp),
        ("revision 54's folder, a texture in patch 55", b"Skhid baked textures, revision 54\r\n", texture55,
         False, made(fingerprint(texture55))),
        ("revision 53's folder", b"Skhid baked textures, revision 53\r\n", data, False, stamp),
        ("a folder with no stamp (the stock game's, or none)", None, data, False, stamp),
        ("a stamp of other bytes", f"Skhid baked textures, format {BAKE_FORMAT}, inputs ".encode() + b"x" * 16 + b"\r\n",
         data, False, stamp),
    )
    for what, stamped, patches, keeps, after in cases:
        kept, now = start(stamped, patches)
        report(kept == keeps and now == after,
               f"{what}: {'kept' if kept else 'deleted (0x1E8DA0) and made again'}, stamped "
               f"\"{(now or b'none').decode().strip()}\"")
    kept, now = start(stamp, data, db=False)
    report(not kept and now is None, "no resources.db where the game mounts it from (the game data not known): "
                                     "deleted, not stamped (deleted again at the next start)")
    report(fingerprint(stats55) == fingerprint(data) != fingerprint(texture55),
           "the fingerprint here (Python, of the tables) is kept by a patch of item stats, changed by a texture")

    game = Game(bytes(built))
    game.dirs.add(FOLDER)
    ok = game.start()
    report(ok and [c for c in game.calls if c[0] == "wipe"] == [("wipe", FOLDER)]
           and FOLDER + "\\" + STAMP not in game.files, "no game data and no stamp: deleted, not stamped")

    game = Game(bytes(built))
    game.data(data)
    ok = game.start()
    report(ok and FOLDER in game.dirs and game.files[FOLDER + "\\" + STAMP] == stamp,
           "no folder: made and stamped")

    game = Game(bytes(built))
    game.data(data)
    game.dirs.add(FOLDER)
    game.files[FOLDER + "\\" + STAMP] = made(inputs ^ 1)
    game.files[FOLDER + "\\a_s1d.dds"] = a
    game.files[FOLDER + "\\held_s1d.dds"] = b
    game.locked.add(FOLDER + "\\held_s1d.dds")
    ok = game.start()
    report(ok and FOLDER + "\\" + STAMP not in game.files and FOLDER + "\\held_s1d.dds" in game.files
           and FOLDER + "\\a_s1d.dds" not in game.files and game.records() == [],
           "a folder of other game data whose delete leaves a file (held open): not stamped, deleted again at "
           "the next start")

    game = Game(bytes(built))
    game.data(data)
    game.dirs.add(FOLDER)
    game.files[FOLDER + "\\" + STAMP] = stamp
    small = dds(64, 64, 2048)
    room = LIST_RECORDS - 512
    for i in range(room + 100):
        game.files[FOLDER + f"\\t{i:04}_s1d.dds"] = small
    ok = game.start()
    kept = [p for p in game.files if p.endswith(".dds")]
    report(ok and len(game.records()) == room and len(kept) == room and game.read(BYTES) == room * len(small),
           f"{room + 100} kept files: {room} in the list (its room less 512), the 100 after them deleted")

    print("the trim (remove_unwanted_files 0x131F90, whole; `ret 4`)")
    game = Game(bytes(built))
    game.data(data)
    game.dirs.add(FOLDER)
    game.files[FOLDER + "\\" + STAMP] = stamp
    ok = game.start()
    part = FOLDER_BYTES // 4
    loading = game.push("loading_s1d.dds", part, loading=1)
    b, c, d = (game.push(f"{name}_s1d.dds", part) for name in "bcd")
    before = len(game.files.get("srp_patches.log", b""))
    kept = game.trim()
    report(ok and kept and game.records() == [loading, b, c, d] and game.read(BYTES) == FOLDER_BYTES
           and not any(call[0] == "DeleteFileA" for call in game.calls)
           and len(game.files["srp_patches.log"]) == before
           and ("GetDiskFreeSpaceExW", "c:\\", 0, 0) in game.calls,
           "four records of 3 GiB: none goes, no file deleted, no line (the drive c:\\ asked for its free bytes); "
           "`ret 4`, ebx, esi, edi and ebp kept")
    e = game.push("e_s1d.dds", 5 << 20)
    kept = game.trim()
    line = game.files["srp_patches.log"][before:]
    report(kept and game.records() == [loading, e, c, d] and game.read(BYTES) == 3 * part + (5 << 20)
           and FOLDER + "\\b_s1d.dds" not in game.files
           and all(FOLDER + f"\\{name}_s1d.dds" in game.files for name in ("loading", "c", "d", "e"))
           and line == b"srp: bake_cache: the oldest baked textures let go: 1; the folder holds 2309 MB in 4 "
                       b"files, at most 3072 MB and 4095 files\r\n",
           f"a bake of 5 MB more: the oldest record with no load under way goes with its file (the older one "
           f"is loading), the last record in its place, {game.read(BYTES)} bytes listed; {line.decode().strip()!r}")
    game.disk_free, game.now, before = DISK_LOW - 1, game.now + 2000, len(game.files["srp_patches.log"])
    kept = game.trim()
    line = game.files["srp_patches.log"][before:]
    report(kept and game.records() == [loading] and game.read(BYTES) == part
           and [p for p in game.files if p.endswith(".dds")] == [FOLDER + "\\loading_s1d.dds"]
           and line == b"srp: bake_cache: the oldest baked textures let go: 3; the folder holds 768 MB in 1 "
                       b"files, at most 50 MB and 4095 files\r\n",
           f"less than 50 MB free on the drive: down to 50 MB, every record but the loading one gone with its "
           f"file; {line.decode().strip()!r}")
    game.disk_free = None
    f = game.push("f_s1d.dds", part)
    kept = game.trim()
    report(kept and game.records() == [loading, f], "Windows does not say the drive's free bytes: as with room")

    game = Game(bytes(built))
    game.data(data)
    game.dirs.add(FOLDER)
    game.files[FOLDER + "\\" + STAMP] = stamp
    ok = game.start()
    pushed = [game.push(f"t{i:04}_s1d.dds", 1000) for i in range(LIST_RECORDS)]
    kept = game.trim()
    report(ok and kept and game.records() == [pushed[-1]] + pushed[1:-1] and game.read(BYTES) == 1000 * (LIST_RECORDS - 1)
           and FOLDER + "\\t0000_s1d.dds" not in game.files and FOLDER + "\\t0001_s1d.dds" in game.files
           and game.read(LIST + 4) + RECORD == game.read(LIST + 8),
           f"a list of {LIST_RECORDS} records, far under 3 GiB: the oldest goes (room for the next bake's "
           f"record: the game's push_back writes it without asking)")

    game = Game(bytes(built))
    pushed = [game.push(f"t{i:04}_s1d.dds", 1000) for i in range(STOCK_RECORDS)]
    kept = game.trim()
    report(kept and game.read(LIST) == STORAGE and game.records() == [pushed[-1]] + pushed[1:-1]
           and FOLDER + "\\t0000_s1d.dds" in game.files
           and not any(call[0] in ("DeleteFileA", "GetDiskFreeSpaceExW") for call in game.calls),
           f"no folder known (the game's start not passed: the baker's own {STOCK_RECORDS} records, full): the "
           f"oldest record goes, no file deleted, the drive not asked")

    print("the look-up and the texture made (translate_query 0x12D4C5..0x12D4D2, process_compression "
          "0x12F194, on_baked_texture_loaded 0x12DDB9, finish_query 0xF0D00)")
    x = PREFIX + "scavengers_02_torso_s3d.dds"
    y = PREFIX + "scavengers_02_legs_s3d.dds"
    for label, data in (("the original", original), ("the build", built)):
        game = Game(bytes(data))
        objects = Objects(game)
        q1, q2 = objects.request(0), objects.request(0)
        r1, r2 = game.lookup(x, q1[0], q1[1], q1[2], q1[3]), game.lookup(x, q2[0], q2[1], q2[2], q2[3])
        if label == "the original":
            report(r1[:3] == ("on", 0, 1) and r2[:3] == ("on", 0, 1),
                   "the original: two queries of one path both look up, find nothing and bake")
            continue
        report(r1[:3] == ("on", 0, 1), "the first query of a path looks it up (0x131ED0), nothing saved: it bakes")
        q3 = objects.request(0)
        r3 = game.lookup(x, q3[0], q3[1], q3[2], q3[3])
        report(r2[:3] == ("waits", None, 0) and r3[:3] == ("waits", None, 0)
               and game.read(q2[3] + 0xD0) == 5 and game.read(q3[3] + 0xD0) == 5,
               "a second and third query of it while it bakes: no look-up, no bake, out of translate_query by its "
               "end (`ret 4`, registers back), their model instance kept (translate_query's released, ours held)")
        q4 = objects.request(0)
        r4 = game.lookup(y, *q4)
        q5 = objects.request(1)
        r5 = game.lookup(x, *q5)
        report(r4[:3] == ("on", 0, 1) and r5[:3] == ("on", 0, 1),
               "another path, and the same path of another type, go on to the look-up")

        texture = objects.texture()
        game.word(q1[1], texture)                      # the bake put it in the owner's slot 0
        old = objects.texture(1)
        game.word(q3[1], old)                          # a texture the third's slot held
        ok, calls = game.baked(q1[0], texture, q1[1], 0)
        finished = [c for c in calls if c[0] == "finish"]
        resources = [c for c in calls if c[0] == "set resource"]
        report(ok and game.read(q2[1]) == texture and game.read(q3[1]) == texture and game.read(texture + 4) == 3
               and ("release texture", old) in calls
               and finished == [("finish", q2[0], 3, 1, 0), ("finish", q3[0], 3, 1, 0)]
               and [c[1] for c in resources] == [q2[0], q3[0]]
               and all(c[3] == 0x21E76C8 and c[4] == 0x108 for c in resources)
               and [c[2] for c in calls if c[0] == "allocate"] == [0x108, 0x108]
               and game.read(q2[3] + 0xD0) == 4 and game.read(q3[3] + 0xD0) == 4
               and not [c for c in calls if c[0] == "translate"],
               "the bake's texture made: the two waiting queries' slots hold it (one reference each more, the "
               "slot's old texture released), each has the texture's resource and is finished with success, "
               "their model instances released; ecx+8 pushed, eax 108h, esi and the frame kept")
        ok, calls = game.finish(q1[0], 3)
        report(ok and calls == [("finish", q1[0], 3, 1, 0)],
               "then the owner's own finish: the function, nothing made again")
        q6 = objects.request(0)
        r6 = game.lookup(x, *q6)
        report(r6[:3] == ("on", 0, 1), "after it, a query of the path looks it up again (the list has it then)")

        # an owner that fails: the waiting query made again
        q7 = objects.request(0)
        r7 = game.lookup(x, *q7)
        ok, calls = game.finish(q6[0], 1)
        report(r7[:3] == ("waits", None, 0) and ok
               and calls == [("finish", q6[0], 1, 1, 0), ("translate", COOK, q7[0])]
               and game.read(q7[3] + 0xD0) == 4,
               "an owner that ends with an error: the function, then the waiting query made again "
               "(translate_query, the cook), its model instance released")

        # a texture not where the owner's slot is: nothing shared, made again at the owner's end
        q8, q9 = objects.request(2), objects.request(2)
        z = PREFIX + "edge_hunter_01_torso_s1f.dds"
        game.lookup(z, *q8)
        r9 = game.lookup(z, *q9)
        texture = objects.texture()
        ok, calls = game.baked(q8[0], texture, q8[1], 2)
        shared = game.read(q9[1] + 8)
        ok2, calls2 = game.finish(q8[0], 3)
        report(r9[0] == "waits" and ok and not calls and shared == 0 and ok2
               and calls2 == [("finish", q8[0], 3, 1, 0), ("translate", COOK, q9[0])],
               "a texture not in the owner's slot: nothing shared; at the owner's success the waiting query "
               "is made again")

        # a load: a kept file found, the second query waits, the loaded texture shared
        game.word(LIST + 4, STORAGE)
        game.string(PREFIX + "kept_s1s.dds", STORAGE)
        game.word(STORAGE + CRC, 0)
        game.word(STORAGE + SIZE, 1000)
        game.emu.mem_write(STORAGE + LOADING, b"\0")
        game.word(LIST + 4, STORAGE + RECORD)
        p1, p2 = objects.request(3), objects.request(3)
        l1 = game.lookup(PREFIX + "kept_s1s.dds", *p1)
        l2 = game.lookup(PREFIX + "kept_s1s.dds", *p2)
        texture = objects.texture()
        game.word(p1[1] + 12, texture)
        ok, calls = game.loaded(p1[0], texture, p1[1], p1[2])
        report(l1[:3] == ("on", 1, 1) and game.emu.mem_read(STORAGE + LOADING, 1)[0] == 1
               and l2[:3] == ("waits", None, 0) and ok and game.read(p2[1] + 12) == texture
               and [c for c in calls if c[0] == "finish"] == [("finish", p2[0], 3, 1, 0)]
               and game.read(texture + 4) == 3,
               "a kept file: the first query finds its record and loads it, the second waits and gets the "
               "loaded texture (slot 3), finished with success; the texture held for the game (a reference)")
        ok, calls = game.finish(p1[0], 3)
        p3 = objects.request(3)
        l3 = game.lookup(PREFIX + "kept_s1s.dds", *p3)
        report(ok and l3[:3] == ("waits", None, 0) and game.read(p3[1] + 12) == texture
               and [c for c in l3[3] if c[0] == "finish"] == [("finish", p3[0], 3, 1, 0)]
               and [c[1] for c in l3[3] if c[0] == "set resource"] == [p3[0]]
               and game.read(texture + 4) == 4 and game.read(p3[3] + 0xD0) == 4
               and game.emu.mem_read(STORAGE + LOADING, 1)[0] == 1,
               "a respawn's query of a held texture: no look-up, no load; it gets the texture at once (slot, "
               "resource, finished with success) and leaves translate_query by its end")

        # held textures past 256 MB: the least used let go
        big = 200 << 20
        for i, name in enumerate(("big_a_s1d.dds", "big_b_s1d.dds")):
            at = STORAGE + RECORD * (1 + i)
            game.string(PREFIX + name, at)
            game.word(at + SIZE, big)
            game.word(at + CRC, 0)
            game.emu.mem_write(at + LOADING, b"\0")
        game.word(LIST + 4, STORAGE + RECORD * 3)
        held = []
        for name in ("big_a_s1d.dds", "big_b_s1d.dds"):
            r = objects.request(0)
            game.lookup(PREFIX + name, *r)
            t = objects.texture()
            game.word(r[1], t)
            game.loaded(r[0], t, r[1], r[2])
            game.finish(r[0], 3)
            held.append(t)
        r = objects.request(0)
        again = game.lookup(PREFIX + "big_a_s1d.dds", *r)
        report(game.read(held[0] + 4) == 1 and game.read(held[1] + 4) == 2 and again[:3] == ("on", 1, 1),
               "two textures of 200 MB: the first let go when the second is held (256 MB at most); a query of "
               "the first loads it again")

        # an owner that never ends: after 10 s the next query of its path takes it over, waiters and all
        game = Game(bytes(built))
        objects = Objects(game)
        s1, s2, s3 = objects.request(0), objects.request(0), objects.request(0)
        game.lookup(x, *s1)
        w = game.lookup(x, *s2)
        game.now += 10001
        t = game.lookup(x, *s3)
        texture = objects.texture()
        game.word(s3[1], texture)
        ok, calls = game.baked(s3[0], texture, s3[1], 0)
        ok2, calls2 = game.finish(s1[0], 3)
        report(w[0] == "waits" and t[:3] == ("on", 0, 1) and ok and game.read(s2[1]) == texture
               and [c for c in calls if c[0] == "finish"] == [("finish", s2[0], 3, 1, 0)]
               and ok2 and calls2 == [("finish", s1[0], 3, 1, 0)],
               "an owner listed for over 10 s: the next query of its path takes it over and bakes; its texture goes "
               "to the waiting query; the old owner's late finish passes through")

        # the 17th query of a path goes on
        game = Game(bytes(built))
        objects = Objects(game)
        results = [game.lookup(x, *objects.request(0))[0] for _ in range(18)]
        report(results == ["on"] + ["waits"] * 16 + ["on"],
               "one owner and 16 waiting queries; the 17th query of the path goes on (looks up, bakes)")

        # finish_query of any other query: the function at once
        game = Game(bytes(built))
        ok, calls = game.finish(0x30001234, 3)
        report(ok and calls == [("finish", 0x30001234, 3, 1, 0)],
               "finish_query of a query that owns nothing: the function with its arguments, registers kept")

    print("the CRC-32 step (0x42050, eax &the CRC, edx the bytes, [esp+4] the count; `ret 4`)")
    import random
    rng = random.Random(70)
    size = 10 * 1024 * 1024 + 64
    # Penryn (Core 2) has no PCLMULQDQ: the sliced way; Westmere has it: the fold
    models = (("Penryn, sliced", UC_CPU_X86_PENRYN), ("Westmere, folded", UC_CPU_X86_WESTMERE))
    stock = Crc(bytes(original), size)
    builds = [(name, Crc(bytes(built), size, cpu)) for name, cpu in models]
    data = bytes(rng.getrandbits(8) for _ in range(1 << 16)) * (size >> 16) + bytes(64)
    data = bytearray(data)
    for i in range(0, len(data), 4099):                # not a repeat of 64 KB
        data[i] = rng.getrandbits(8)
    for game in [stock] + [game for _, game in builds]:
        game.emu.mem_write(BUFFER, bytes(data))
        game.call(TABLE, {}, [])                       # the table at 0x225FD68, as the callers make it first
    lengths = [0, 1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 63, 64, 65, 127, 128, 129, 143, 144, 191, 192, 255, 256,
               1000, 4093, 65541, (1 << 20) + 3, 1398225, 5592529, 10 * 1024 * 1024]
    for name, patched in builds:
        same = True
        for i, length in enumerate(lengths):
            start = BUFFER + (i % 4)
            crc = rng.getrandbits(32)
            a_out, b_out = (game.crc(start, length, crc) for game in (stock, patched))
            if a_out != b_out:
                same = False
                print(f"    differs: {length} bytes from +{i % 4}: {a_out} against {b_out}")
        report(same, f"{name}: {len(lengths)} buffers of 0 to 10 MB, starts 0 to 3 bytes past a dword, random "
                     f"CRCs: the CRC, eax, ecx, edx, ebx, esi, edi, ebp and esp after it are the original's")
    counted = []
    for data_of, cpu in ((original, None), (built, UC_CPU_X86_PENRYN), (built, UC_CPU_X86_WESTMERE)):
        game = Crc(bytes(data_of), 1 << 20, cpu)   # fresh: a hook added once code is translated misses it
        game.emu.mem_write(BUFFER, bytes(data[:1 << 20]))
        game.call(TABLE, {}, [])
        counted.append(game.count(BUFFER + 1, 262144))
    report(counted[1] * 3 < counted[0] and counted[2] * 10 < counted[0],
           f"256 KB: {counted[0]} instructions in the original, {counted[1]} sliced, {counted[2]} folded "
           f"({counted[0] / 262144:.2f}, {counted[1] / 262144:.2f} and {counted[2] / 262144:.2f} a byte)")

    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
