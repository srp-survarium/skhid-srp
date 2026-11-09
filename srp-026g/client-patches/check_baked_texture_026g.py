#!/usr/bin/env python3
"""Run the original's and patch 35's baked-texture list of the camouflage /
decal baker on made-up loads, the crash of 2026-10-17 among them.

    python check_baked_texture_026g.py <original survarium.exe>

Requires Python `unicorn` (2.1.4). Three functions of
camouflage_decal_texture_type_baker_cook run as machine code from their
first instruction, on a made-up baker (its list at +230h, its used bytes at
+4923Ch) and the game's own counter of ages (0x2260920):

- 0x131ED0, the look-up translate_query makes before it loads a saved
  texture: the first record of the path, marked "a load is under way";
- 0x132640, the list's push_back, which process_compression calls when it
  has baked a texture (the script adds the size and the age, as the caller
  does);
- 0x131B40, check_loaded_baked_texture, which on_baked_texture_loaded calls
  with the loaded file: the CRC of its bytes (0x3C0F70, a stub returning the
  CRC the case names) against the first record of the path. With the game's
  own erase (0x132E40) and record copy (0x133120).

Stubs: the log filter (0x111860, says no), the assertion (0x113450, writes
down its text and asks for the break, as the player's game did) and
IsDebuggerPresent (no). An `int3` is the game stopping.

The case of the crash: two characters wearing one item bake its texture at
once (both look-ups find nothing, so two records of one path), then three
loads of that path are under way, and the file each reads is not the bytes
baked (a file another game or an earlier one wrote, read before this game's
own save has landed: the save is asynchronous). The first two loads erase a
record each ("Texture %s is corrupted!", made again); the third finds none:
the original stops at the assertion "Saved baked texture %s not found",
the build returns false and the load is made again as the first two were,
by a bake (the look-up finds nothing). Says nothing about why the bytes
differed in the player's game (the dump does not hold the files).
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE, UC_HOOK_INTR
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_ESP

import patch_026g as base

FIND, PUSH_BACK, CHECK = 0x131ED0, 0x132640, 0x131B40
CRC_OF, LOG_FILTER, ASSERTION = 0x3C0F70, 0x111860, 0x113450
IS_DEBUGGER_PRESENT_IAT = 0x870194
LOG_FILTER_ON = 0x1F1D60C                     # nonzero: the filter is asked
AGES = 0x2260920                              # ++ for each record and each good load
LIST, USED = 0x230, 0x4923C
RECORD = 0x124
PATH, CRC, SIZE, AGE, LOADING = 0x0, 0x114, 0x118, 0x11C, 0x120
BAKER, STORAGE, SCRATCH = 0x40000000, 0x40100000, 0x40200000
CAPACITY = 64
STACK, STOP, STUB = 0x41008000, 0x42000000, 0x42000100
NOT_FOUND = "Saved baked texture %s not found"

X = "user_data/streamed_textures/edge_researcher_03_legs_s3d.dds"
OTHER = "user_data/streamed_textures/edge_researcher_03_legs_s2d.dds"
BAKED, STALE = 0x64308FFD, 0x0BADF11E          # the CRC baked, the CRC of what a load read
SIZE_X = 699177


class Baker:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        headers = base.Headers(data)
        for _, rva, vsize, raw, rsize, _ in headers.sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((BAKER, 0x50000), (STORAGE, 0x10000), (SCRATCH, 0x10000), (0x41000000, 0x10000),
                         (STOP, 0x10000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        self.word(BAKER + LIST, STORAGE)
        self.word(BAKER + LIST + 4, STORAGE)
        self.word(BAKER + LIST + 8, STORAGE + CAPACITY * RECORD)
        self.word(LOG_FILTER_ON, 1)
        self.word(IS_DEBUGGER_PRESENT_IAT, STUB)
        self.stubs = {CRC_OF: "crc", LOG_FILTER: "log filter", ASSERTION: "assertion", STUB: "IsDebuggerPresent"}
        self.assertions, self.stopped, self.crc = [], False, None
        emu.hook_add(UC_HOOK_CODE, self.stub)
        emu.hook_add(UC_HOOK_INTR, self.interrupt)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def ascii(self, at):
        out = b""
        while (c := bytes(self.emu.mem_read(at + len(out), 1))) != b"\0":
            out += c
        return out.decode("latin-1")

    def returns(self, value, pop=0):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        self.emu.reg_write(UC_X86_REG_EAX, value)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pop)

    def stub(self, emu, address, size, _):
        name = self.stubs.get(address)
        if name is None:
            return
        sp = emu.reg_read(UC_X86_REG_ESP)
        arg = lambda n: self.read(sp + 4 + 4 * n)
        if name == "crc":                          # the loaded resource by value: `ret 4`
            assert self.crc is not None, "a CRC asked for outside a load"
            self.returns(self.crc, 4)
        elif name == "log filter":                 # cdecl (channel, level): no line
            self.returns(0)
        elif name == "assertion":                  # cdecl: (&break, ..., format, path)
            self.assertions.append((self.ascii(arg(9)), self.ascii(arg(10))))
            emu.mem_write(arg(0), b"\x01")
            self.returns(0)
        elif name == "IsDebuggerPresent":
            self.returns(0)

    def interrupt(self, emu, number, _):
        self.stopped = True
        emu.emu_stop()

    def call(self, function, *args, eax=0, edi=0):
        sp = STACK
        for value in reversed(args):
            sp -= 4
            self.word(sp, value)
        sp -= 4
        self.word(sp, STOP)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.reg_write(UC_X86_REG_EAX, eax)
        self.emu.reg_write(UC_X86_REG_EDI, edi)
        self.emu.emu_start(function, STOP, count=200000)
        if self.stopped:
            return None
        assert self.emu.reg_read(UC_X86_REG_EIP) == STOP
        assert self.emu.reg_read(UC_X86_REG_ESP) == STACK, f"{function:#x}: the stack is not as it was"
        return self.emu.reg_read(UC_X86_REG_EAX) & 0xFF

    def path(self, text, at=SCRATCH + 0x8000):
        """A buffer_string of the path, as the callers make them."""
        raw = text.encode("ascii")
        self.word(at, at + 0xC)
        self.word(at + 4, at + 0xC + len(raw))
        self.word(at + 8, at + 0xC + 0x104)
        self.emu.mem_write(at + 0xC, raw + b"\0")
        self.emu.mem_write(at + 0x110, b"/")
        return at

    # What the game does, made of the three functions.
    def baked(self, text, crc, size):
        """process_compression: a record of the bytes it just baked."""
        record = SCRATCH
        self.path(text, record)
        age = self.read(AGES) + 1
        self.word(AGES, age)
        for offset, value in ((CRC, crc), (SIZE, size), (AGE, age)):
            self.word(record + offset, value)
        self.emu.mem_write(record + LOADING, b"\0")
        assert self.call(PUSH_BACK, BAKER + LIST, edi=record) is not None
        self.word(BAKER + USED, self.read(BAKER + USED) + size)

    def look_up(self, text):
        """translate_query: True, a load of the saved file is under way."""
        return bool(self.call(FIND, self.path(text), eax=BAKER))

    def loaded(self, text, crc):
        """on_baked_texture_loaded: True the file is used, False made again,
        None the game stopped."""
        self.crc = crc
        result = self.call(CHECK, BAKER, 0, self.path(text))
        self.crc = None
        return None if result is None else bool(result)

    def records(self):
        begin, end = self.read(BAKER + LIST), self.read(BAKER + LIST + 4)
        out = []
        for at in range(begin, end, RECORD):
            assert self.read(at) == at + 0xC, "a record's path is not in the record"
            out.append((self.ascii(self.read(at)), self.read(at + CRC), self.read(at + SIZE), self.read(at + AGE),
                        self.emu.mem_read(at + LOADING, 1)[0]))
        return out


def run(data):
    """What each case gives with this exe."""
    out = {}

    # A saved texture loaded: the record is used and made young again.
    baker = Baker(data)
    baker.baked(OTHER, 0x1111, 1000)
    baker.baked(X, BAKED, SIZE_X)
    assert baker.look_up(X) and baker.records()[1][4] == 1
    out["good load"] = (baker.loaded(X, BAKED), baker.records(), baker.assertions)

    # One load of bytes that are not the ones baked: erased, made again.
    baker = Baker(data)
    baker.baked(OTHER, 0x1111, 1000)
    baker.baked(X, BAKED, SIZE_X)
    assert baker.look_up(X)
    out["one stale load"] = (baker.loaded(X, BAKED + 1), baker.records(), baker.read(BAKER + USED),
                             baker.assertions)

    # The crash: two bakes of one path, three loads of what is on the disk.
    baker = Baker(data)
    baker.baked(OTHER, 0x1111, 1000)
    assert not baker.look_up(X) and not baker.look_up(X)      # characters A and B: nothing saved
    baker.baked(X, BAKED, SIZE_X)
    baker.baked(X, BAKED, SIZE_X)
    assert all(baker.look_up(X) for _ in range(3))           # C, D, E (or a texture's next quality)
    results = []
    for _ in range(3):
        results.append(baker.loaded(X, STALE))
        if results[-1] is None:
            break
    case = [results, baker.records(), baker.read(BAKER + USED), baker.assertions]
    if results[-1] is not None:
        # Each load made again: the look-up finds nothing, so it bakes; then
        # a load of the file once the save has landed is used.
        assert not baker.look_up(X)
        baker.baked(X, BAKED, SIZE_X)
        assert baker.look_up(X)
        case.append((baker.loaded(X, BAKED), baker.records()))
    out["crash"] = case
    return out


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = True
    patched, _ = base.build(original, "section", code, None, late_join)
    before, _ = base.build(original, "section", code, None, None)
    va, old, new = base.BAKED_TEXTURE_MISSING
    at = base.file_offset(va)
    assert original[at:at + 2] == old and before[at:at + 2] == old and patched[at:at + 2] == new
    stock, built, without = run(original), run(patched), run(before)
    assert without == stock, "the build without the late join is not the original here"

    for result in (stock, built):
        # The same for both: a good load, one stale load.
        good, records, assertions = result["good load"]
        assert good is True and not assertions
        assert records == [(OTHER, 0x1111, 1000, 1, 0), (X, BAKED, SIZE_X, 3, 0)], records
        made_again, records, used, assertions = result["one stale load"]
        assert made_again is False and not assertions
        assert records == [(OTHER, 0x1111, 1000, 1, 0)] and used == 1000, (records, used)

    # The original: two loads made again, the third stops the game.
    results, records, used, assertions = stock["crash"]
    assert results == [False, False, None], results
    assert assertions == [(NOT_FOUND, X)], assertions
    assert records == [(OTHER, 0x1111, 1000, 1, 0)] and used == 1000, (records, used)

    # The build: all three made again, then baked and loaded as usual.
    results, records, used, assertions, (good, after) = built["crash"]
    assert results == [False, False, False] and not assertions, (results, assertions)
    assert records == [(OTHER, 0x1111, 1000, 1, 0)] and used == 1000, (records, used)
    assert good is True and after == [(OTHER, 0x1111, 1000, 1, 0), (X, BAKED, SIZE_X, 5, 0)], (good, after)

    print("patch 35: the original stops at \"Saved baked texture ... not found\" on the third of three "
          "stale loads of a path baked twice; the build makes all three again and bakes it; "
          "a good load and one stale load are the original's in both")


if __name__ == "__main__":
    main()
