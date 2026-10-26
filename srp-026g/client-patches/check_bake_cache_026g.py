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

- the folder (game::game from its `call 0x1E8DA0` at 0x1E9455 to 0x1E945A,
  eax the folder's buffer_string; the game's imports a made-up folder
  through their IAT slots, the stock delete 0x1E8DA0 a stub that empties it):
  a folder stamped with this build's revision keeps its whole DDS files and
  they are in the baker's list (CRC 0, their size, an age each, the list's
  bytes grown by theirs), a file cut short and the files past 512 deleted,
  the stub not called; a folder of another revision or none is deleted (the
  stub) and stamped. The original calls the stub every time;
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
  0x12F18F to 0x12F194, its frame's texture +10h, query +604h, surface
  +610h, type +620h; on_baked_texture_loaded from 0x12DDB4 to 0x12DDB9, edi
  the texture, ebp its frame): each waiting query's surface slot of the type
  holds the texture (one reference more each), its query gets a resource of
  the game's (0x12DE30 on the allocator's 108h bytes: stubs that record) and
  is finished with success (finish_query, its entry hooked, the function a
  stub from 0xF0D06 that records), its model instance released; eax the
  call's ecx + 8 and esi kept; with the texture not in the owner's slot
  nothing is shared; a loaded texture is held (one reference more): a later
  query of its path (a respawn) gets it at once with no look-up, and past
  256 MB the least used is let go;
- the CRC-32 step (0x42050, run from its entry as its 43 callers call it; a
  separate emulator with no hook on every instruction): the build's against
  the original's, with the game's own table (0x1C210 run first), on random
  buffers of 0 to 10 MB from starts 0 to 3 bytes past a dword and random
  CRCs: the CRC and eax, ecx, edx, ebx, esi, edi, ebp and esp after it the
  same; and how many instructions each runs on 256 KB;
- a folder whose delete leaves a file (held open): not stamped;
- an owner that ends otherwise (finish_query 0xF0D00 called as the game
  calls it, stdcall, with an error, or a success nothing shared): the
  function, then each waiting query made again (translate_query 0x12D1D0, a
  stub that records the cook and the query), its reference released; any
  other query: the function with the same arguments, the registers kept.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE, UC_HOOK_INTR
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import patch_026g as base

COOK = 0xA264E80                                   # the baker (camouflage_decal_texture_type_baker_cook)
LIST, CAPACITY, BYTES, AGES = 0xA2650B0, 0xA2AE0BC, 0xA2AE0BC, 0x2260920
STORAGE = 0xA2650BC
RECORD, CRC, SIZE, AGE, LOADING = 0x124, 0x114, 0x118, 0x11C, 0x120
START_SITE, START_BACK, WIPE = 0x1E9455, 0x1E945A, 0x1E8DA0
LOOKUP_FROM, LOOKUP_TEST, NOT_BAKED_RET = 0x12D4C5, 0x12D4D2, None
FIND_RECORD, TRANSLATE, FINISH, FINISH_BODY = 0x131ED0, 0x12D1D0, 0xF0D00, 0xF0D06
BAKED_SITE, BAKED_BACK, LOADED_SITE, LOADED_BACK = 0x12F18F, 0x12F194, 0x12DDB4, 0x12DDB9
ALLOCATE, RESOURCE, SET_RESOURCE, RELEASE_TEXTURE, DESTROY_MODEL = 0xE4F90, 0x12DE30, 0xF0080, 0x3BEE00, 0xE0210
CHECK, CRC_OF, LOG_FILTER, ASSERTION = 0x131B40, 0x3C0F70, 0x111860, 0x113450
LOG_FILTER_ON, IS_DEBUGGER_PRESENT = 0x1F1D60C, 0x870194
NAMES = 0xAC3334
REVISION_BYTE = 0x380866
CRC_STEP, TABLE = 0x42050, 0x1C210                   # the CRC-32 step; the table's maker (0x225FD68)
BUFFER, CRC_STATE = 0x40000000, 0x31000000 + 0xF0000
IAT = {"FindFirstFileA": 0x870238, "FindNextFileA": 0x87023C, "FindClose": 0x870228, "CreateFileA": 0x870048,
       "ReadFile": 0x870064, "WriteFile": 0x870108, "CloseHandle": 0x870040, "DeleteFileA": 0x87021C,
       "CreateDirectoryA": 0x870220, "GetTickCount": 0x870090, "GetCurrentThreadId": 0x870118}
POPS = {"FindFirstFileA": 8, "FindNextFileA": 8, "FindClose": 4, "CreateFileA": 28, "ReadFile": 20,
        "WriteFile": 20, "CloseHandle": 4, "DeleteFileA": 4, "CreateDirectoryA": 8, "GetTickCount": 0,
        "GetCurrentThreadId": 0}
OBJECTS, STACK, STUBS, STOP = 0x30000000, 0x31000000, 0x32000000, 0x32100000
FOLDER = "c:\\users\\player\\documents\\survarium\\streamed_textures"
STAMP = "srp_baked_textures.txt"
PREFIX = "user_data/streamed_textures/"
INVALID = 0xFFFFFFFF


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
        self.calls.append((name,) + tuple(a[:1]))
        if name == "CreateFileA":
            path, access, disposition = self.ascii(a[0]), a[1], a[4]
            folder = path.rsplit("\\", 1)[0]
            if disposition == 3:                     # OPEN_EXISTING
                if path not in self.files:
                    return INVALID
            elif disposition == 2 and folder in self.dirs:
                self.files[path] = b""
            else:
                return INVALID
            handle, self.next_handle = self.next_handle, self.next_handle + 4
            self.handles[handle] = [path, 0]
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
            assert mask == "*.dds", pattern
            names = sorted(p[len(folder) + 1:] for p in self.files
                           if p.startswith(folder + "\\") and p.lower().endswith(".dds") and "\\" not in p[len(folder) + 1:])
            names += sorted(d[len(folder) + 1:] + "\\" for d in self.dirs if d.startswith(folder + "\\") and d.lower().endswith(".dds"))
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
        """game::game from its delete's call to 0x1E945A."""
        string = self.string(FOLDER, OBJECTS + 0x70000)
        sp = STACK + 0x8000
        regs = {UC_X86_REG_EAX: string, UC_X86_REG_EBX: 0x1111, UC_X86_REG_ESI: 0x2222, UC_X86_REG_EDI: 0x3333,
                UC_X86_REG_EBP: 0x4444}
        eip, esp = self.run(START_SITE, sp, regs, until=[START_BACK])
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
        """process_compression from 0x12F18F to 0x12F194."""
        frame = STACK + 0x60000
        self.emu.mem_write(frame, bytes(0x700))
        for at, value in ((0x10, texture), (0x604, query), (0x610, surface), (0x620, kind)):
            self.word(frame + at, value)
        before = len(self.calls)
        eip, esp = self.run(BAKED_SITE, frame, {UC_X86_REG_ECX: NAMES, UC_X86_REG_ESI: 0xA11C, UC_X86_REG_EBX: 0xB0,
                                                UC_X86_REG_EDI: 0xD0, UC_X86_REG_EBP: query}, until=[BAKED_BACK])
        ok = eip == BAKED_BACK and esp == frame and self.emu.reg_read(UC_X86_REG_EAX) == NAMES + 8 \
            and [self.emu.reg_read(r) for r in (UC_X86_REG_ESI, UC_X86_REG_EBX, UC_X86_REG_EDI, UC_X86_REG_EBP)] \
            == [0xA11C, 0xB0, 0xD0, query]
        return ok, self.calls[before:]

    def loaded(self, query, texture, surface, parameters):
        """on_baked_texture_loaded from 0x12DDB4 to 0x12DDB9."""
        ebp = STACK + 0x70000
        sp = ebp - 0x300
        for at, value in ((ebp + 0xC, parameters), (ebp + 0x10, surface), (ebp + 0x14, query)):
            self.word(at, value)
        before = len(self.calls)
        eip, esp = self.run(LOADED_SITE, sp, {UC_X86_REG_ECX: NAMES, UC_X86_REG_ESI: 0xA11C, UC_X86_REG_EDI: texture,
                                              UC_X86_REG_EBX: surface, UC_X86_REG_EBP: ebp}, until=[LOADED_BACK])
        ok = eip == LOADED_BACK and esp == sp and self.emu.reg_read(UC_X86_REG_EAX) == NAMES + 8 \
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
    step on large buffers at full speed."""
    def __init__(self, data, size):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
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

    revision = built[base.file_offset(REVISION_BYTE)]
    stamp = f"Skhid baked textures, revision {revision}\r\n".encode()
    other = f"Skhid baked textures, revision {revision - 1}\r\n".encode()
    a, b = dds(2048, 1024, 1398097), dds(1024, 1024, 1398097, b"DXT5")

    print("the folder (game::game from 0x1E9455 to 0x1E945A)")
    for label, data in (("the original", original), ("the build", built)):
        game = Game(bytes(data))
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
        report(ok and not wiped and records == want and game.read(BYTES) == len(a) + len(b)
               and game.read(AGES) == 2 and FOLDER + "\\cut_s1d.dds" not in game.files
               and FOLDER + "\\notes.txt" in game.files and game.files[FOLDER + "\\" + STAMP] == stamp,
               f"this revision's folder kept: {[r[0] for r in records]} in the list (CRC 0, sizes, ages 1 and 2, "
               f"{game.read(BYTES)} bytes), the file cut short deleted, the rest left, no delete")

        # a kept file's loads (check_loaded_baked_texture, the game's own)
        first = game.check_loaded(PREFIX + "a_s1d.dds", 0x64308FFD)
        report(first == 1 and game.records()[0][1] == 0x64308FFD and game.records()[0][3] == 3,
               "a kept file's first load is used and its record takes the CRC read (age renewed)")
        again = game.check_loaded(PREFIX + "a_s1d.dds", 0x0BADF11E)
        report(again == 0 and [r[0] for r in game.records()] == [PREFIX + "b_s1n.dds"]
               and game.read(BYTES) == len(b) and ("assertion",) not in game.calls,
               "a load of other bytes after it: erased and made again, as the original does")

        for what, files in (("another revision's folder", {STAMP: other, "a_s1d.dds": a}),
                            ("a folder with no stamp (the stock game's, or none)", {"a_s1d.dds": a}),
                            ("no folder", None)):
            game = Game(bytes(built))
            if files is not None:
                game.dirs.add(FOLDER)
                for name, content in files.items():
                    game.files[FOLDER + "\\" + name] = content
            ok = game.start()
            wiped = [c for c in game.calls if c[0] == "wipe"]
            report(ok and wiped == [("wipe", FOLDER)] and game.records() == [] and FOLDER in game.dirs
                   and game.files == {FOLDER + "\\" + STAMP: stamp},
                   f"{what}: deleted (0x1E8DA0), made again and stamped \"{stamp.decode().strip()}\"")

        game = Game(bytes(built))
        game.dirs.add(FOLDER)
        game.files[FOLDER + "\\" + STAMP] = other
        game.files[FOLDER + "\\a_s1d.dds"] = a
        game.files[FOLDER + "\\held_s1d.dds"] = b
        game.locked.add(FOLDER + "\\held_s1d.dds")
        ok = game.start()
        report(ok and FOLDER + "\\" + STAMP not in game.files and FOLDER + "\\held_s1d.dds" in game.files
               and FOLDER + "\\a_s1d.dds" not in game.files and game.records() == [],
               "another revision's folder whose delete leaves a file (held open): not stamped, deleted again at "
               "the next start")

        game = Game(bytes(built))
        game.dirs.add(FOLDER)
        game.files[FOLDER + "\\" + STAMP] = stamp
        small = dds(64, 64, 2048)
        for i in range(600):
            game.files[FOLDER + f"\\t{i:03}_s1d.dds"] = small
        ok = game.start()
        kept = [p for p in game.files if p.endswith(".dds")]
        report(ok and len(game.records()) == 512 and len(kept) == 512 and game.read(BYTES) == 512 * len(small),
               "600 kept files: 512 in the list, the 88 after them deleted")

    print("the look-up and the texture made (translate_query 0x12D4C5..0x12D4D2, process_compression "
          "0x12F18F, on_baked_texture_loaded 0x12DDB4, finish_query 0xF0D00)")
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
               "their model instances released; eax ecx+8, esi and the frame kept")
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
    stock, patched = Crc(bytes(original), size), Crc(bytes(built), size)
    data = bytes(rng.getrandbits(8) for _ in range(1 << 16)) * (size >> 16) + bytes(64)
    data = bytearray(data)
    for i in range(0, len(data), 4099):                # not a repeat of 64 KB
        data[i] = rng.getrandbits(8)
    for game in (stock, patched):
        game.emu.mem_write(BUFFER, bytes(data))
        game.call(TABLE, {}, [])                       # the table at 0x225FD68, as the callers make it first
    lengths = [0, 1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 63, 64, 65, 255, 1000, 4093, 65541, (1 << 20) + 3,
               1398225, 5592529, 10 * 1024 * 1024]
    same = True
    for i, length in enumerate(lengths):
        start = BUFFER + (i % 4)
        crc = rng.getrandbits(32)
        a_out, b_out = (game.crc(start, length, crc) for game in (stock, patched))
        if a_out != b_out:
            same = False
            print(f"    differs: {length} bytes from +{i % 4}: {a_out} against {b_out}")
    report(same, f"{len(lengths)} buffers of 0 to 10 MB, starts 0 to 3 bytes past a dword, random CRCs: the "
                 f"CRC, eax, ecx, edx, ebx, esi, edi, ebp and esp after it are the original's")
    counted = []
    for data_of in (original, built):              # fresh: a hook added once code is translated misses it
        game = Crc(bytes(data_of), 1 << 20)
        game.emu.mem_write(BUFFER, bytes(data[:1 << 20]))
        game.call(TABLE, {}, [])
        counted.append(game.count(BUFFER + 1, 262144))
    report(counted[1] * 3 < counted[0], f"256 KB: {counted[0]} instructions in the original, {counted[1]} here "
                                         f"({counted[0] / 262144:.2f} and {counted[1] / 262144:.2f} a byte, "
                                         f"{counted[0] / counted[1]:.1f} times fewer)")

    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
