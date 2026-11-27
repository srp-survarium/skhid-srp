#!/usr/bin/env python3
"""Run patch 123 (rust/patches/src/faces.rs, the hooks of faces.toml: every
player drawn with srp's face, ../docs/notes/face-choice.md) in the default
build and the game's own code in the original exe, on made-up game objects.

    python check_faces_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them; the config lookups (0xF6B10, 0xF6C00) and buffer_string's
assign_config_text (0x129BB0) are the game's own code. Stubbed: GetCurrentThreadId (one
thread), QueryPerformanceCounter/-Frequency and the log's kernel32 (its lines
kept). Each run starts at a site
and stops at its span's end (or the head's join, 0x2056F0, where the native
head ends too). Checked:

- the three sites of its own in place (`jmp` to its entry, the original's
  bytes there); the two options reads and the match scene's end its chain's;
- data without `heads`: the head the native makes (base_model, part_name,
  tag 0, the count, the skin's name), byte for byte as the original exe's;
- with `heads`: a player srp said nothing of is face 0: heads[0], its helmet
  variant under a helmet, the skin's name as it was;
- `0x87`'s byte (`lobby_client.own_account`, the account's profile's name):
  its face's mask variant under a mask and a helmet; its hair a record of the
  head (tag 0) after the kit's, the count one more; `f2_` on the skin's name;
  the span's own effect on the registers and the client (ESI = EBX, the
  field written);
- a missing variant is the face's `data`;
- `0x88`'s byte (`lobby_client.squad_member`, the profile in EDI): the
  member's face;
- the match's options (`static_info.options_read`, srp's block entry 5): a
  profile that is one of the options' places has its place's face, whatever
  the lobby said of its name; a face the data does not list drawn as face 0,
  with no suffix; a profile that is no place of the options (the lobby's)
  still the lobby's face; an entry shorter than the roster (the places past
  it face 0) and longer; a new match without the entry forgets the old
  match's faces;
- a replay's chunk 1 (`replay.options_read`): the same, its span replayed;
- every record in use (the kit's 19 and the head): the hair gets none, the
  count stays 20, logged;
- a `0x87` without srp's byte: logged, nothing learnt;
- a late joiner's proposal at the dispatch (`match_dispatch.message`, after
  patch 114's check, its profile stepped over with the dictionary's own
  entries, never item_by_id): srp's
  block after the profile (the faces entry of its one place) gives the
  joiner's place its face, and its own copy of the profile (no place) the
  same by its name; the block taken off the message (its reader 7 bytes
  shorter, the span replayed); without a block, face 0 and the message as it
  was; bytes that are no block: logged, left for the late join's check; an
  item the dictionary does not have: the proposal left as it is, nothing
  learnt;
- a worn item's entry (`skin_cook.item_data`, run to the copy of the entry
  the records are read from, 0x204B89): data without `heads`, face 0 and an
  item without the face's key byte for byte the original's `data`; a mask's
  and a helmet's `data_head_<face>` for the account's face, `f<face>_` on
  the skin's name, which the head's cook then does not repeat; a torso's
  key left as it is; a face the data does not list is face 0's `data`, as
  its head; a match's place by its options; a key that is no table: `data`.

Not run: the match scene's end (`game_world.deactivate`, after patch 66's
handler in its chain); the faces forgotten there are the crate's unit test.
"""
import struct
import sys
import zlib

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI,
                               UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP)

import patch_026g as base

HEAD, MADE = 0x205662, 0x2056F0            # skin_cook.head and its join
OWN, OWN_END = 0x27030B, 0x270313           # lobby_client.own_account, span 8
MEMBER, MEMBER_END = 0x274C01, 0x274C07     # lobby_client.squad_member, span 6
OPTIONS, OPTIONS_END = 0x27B92D, 0x27B932   # static_info.options_read, span 5
REPLAY, REPLAY_END = 0x1F0971, 0x1F0976     # replay.options_read, span 5 (pop esi / pop ebx / add esp,34h)
WORLD_LEFT = 0x1F9520                       # game_world.deactivate
DISPATCH, DISPATCH_END = 0x27A31F, 0x27A327 # match_dispatch.message, span 8 (patch 114's check first)
ITEM_BY_ID = 0x1F0090                       # items_dictionary::item_by_id
ITEM, ITEM_COPIED = 0x204B36, 0x204B89      # skin_cook.item_data, and past the entry's copy to [esp+20h]
DATA_KEY = 0x9FD6C4                         # "data"
THREAD_SLOT, QPC_SLOT, QPF_SLOT = 0x870118, 0x870158, 0x870150
CREATE_FILE_SLOT, WRITE_FILE_SLOT, CLOSE_SLOT = 0x870048, 0x870108, 0x870040
HEAP, STUBS, STACK = 0x30000000, 0x31000000, 0x32000000
CONFIG, TEXTS = HEAP, HEAP + 0x40000
COOK, PROFILE, CLIENT = HEAP + 0x80000, HEAP + 0xA0000, 0x33000000
OPTIONS_OBJECT, MESSAGE = 0x34000000, HEAP + 0xB0000
GAME, DICTIONARY, ITEMS = HEAP + 0xC0000, HEAP + 0xD0000, HEAP + 0xD1000
HOLDER = HEAP + 0xE0000
SP, BP = STACK + 0x8000, STACK + 0xC000
RECORD, RECORDS, COUNT, TAG = 0xC0C, 0x234, 0xF324, 0xC08
OWN_PROFILE, PLACE = 0x1E6344, 0x45FC
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP)

failed = 0


def check(ok, what):
    global failed
    failed += not ok
    print(f"  {'ok' if ok else 'FAIL'}  {what}")


class Game:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((HEAP, 0x100000), (STUBS, 0x1000), (STACK, 0x10000), (CLIENT, 0x200000),
                         (OPTIONS_OBJECT, 0x60000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        self.word(THREAD_SLOT, STUBS + 0x80)
        emu.mem_write(STUBS + 0x80, b"\xB8\x01\x00\x00\x00\xC3")
        # QueryPerformanceCounter / -Frequency (the crash breadcrumbs' clock): 0 ticks of 10 MHz
        for slot, stub, value in ((QPC_SLOT, STUBS + 0xC0, 0), (QPF_SLOT, STUBS + 0xE0, 10_000_000)):
            self.word(slot, stub)
            # mov eax,[esp+4] / mov dword [eax],value / mov dword [eax+4],0 / mov eax,1 / ret 4
            emu.mem_write(stub, b"\x8B\x44\x24\x04\xC7\x00" + struct.pack("<I", value)
                          + b"\xC7\x40\x04\x00\x00\x00\x00\xB8\x01\x00\x00\x00\xC2\x04\x00")
        self.logged = b""
        for slot, stub, code in ((CREATE_FILE_SLOT, STUBS + 0x90, b"\xB8\x07\x00\x00\x00\xC2\x1C\x00"),
                                 (WRITE_FILE_SLOT, STUBS + 0xA0, b"\xB8\x01\x00\x00\x00\xC2\x14\x00"),
                                 (CLOSE_SLOT, STUBS + 0xB0, b"\xB8\x01\x00\x00\x00\xC2\x04\x00")):
            self.word(slot, stub)
            emu.mem_write(stub, code)
        emu.hook_add(UC_HOOK_CODE, self.write_file, begin=STUBS + 0xA0, end=STUBS + 0xA0)
        self.texts = TEXTS
        self.configs = CONFIG
        self.item_by_id = 0
        emu.hook_add(UC_HOOK_CODE, self.count_item_by_id, begin=ITEM_BY_ID, end=ITEM_BY_ID)

    def count_item_by_id(self, *_):
        self.item_by_id += 1

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at, size=4):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0] if size == 4 else bytes(self.emu.mem_read(at, size))

    def text(self, at):
        return bytes(self.emu.mem_read(at, 0x110)).split(b"\0")[0].decode()

    def put_text(self, text):
        at = self.texts
        self.emu.mem_write(at, text.encode() + b"\0")
        self.texts += (len(text) + 16) & ~15
        return at

    def write_file(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.logged += self.read(self.read(sp + 8), self.read(sp + 12))

    def records(self, count):
        at = self.configs
        self.configs += (24 * count + 15) & ~15
        return at

    def node(self, value, name=None):
        """A loaded binary config node's record (24 bytes: data, name, CRC, type, count): a text (5), a table (3,
        its children sorted by their names' CRC, as cfg_child's binary search needs) or an array (4)."""
        key = (self.put_text(name), zlib.crc32(name.encode())) if name is not None else (0, 0)
        if isinstance(value, str):
            return struct.pack("<QQIHH", self.put_text(value), key[0], key[1], 5, len(value) + 1)
        if isinstance(value, dict):
            kids = sorted(value.items(), key=lambda item: zlib.crc32(item[0].encode()))
            kind = 3
        else:
            kids = [(None, v) for v in value]
            kind = 4
        first = self.records(len(kids))
        for i, (kid, v) in enumerate(kids):
            self.emu.mem_write(first + 24 * i, self.node(v, kid))
        return struct.pack("<QQIHH", first if kids else 0, key[0], key[1], kind, len(kids))

    def go(self, start, stop, regs, sp):
        emu = self.emu
        for reg, value in regs.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, sp)
        try:
            emu.emu_start(start, stop, count=2_000_000)
        except Exception as error:
            sys.exit(f"FAIL: {start:#x}: {error} at {emu.reg_read(UC_X86_REG_EIP):#x}")
        return {reg: emu.reg_read(reg) for reg in REGS}, emu.reg_read(UC_X86_REG_ESP)

    def string(self, at, text=""):
        """A buffer_string {begin, end, capacity} over its own 104h bytes, as 0x204500 makes them."""
        room = at + 0xC
        self.emu.mem_write(room, text.encode() + b"\0")
        self.word(at, room)
        self.word(at + 4, room + len(text))
        self.word(at + 8, room + 0x104)

    def cook(self, body_parts, name, tags, skin="105_0_0_207_0_0_0_", place=None):
        """on_configs_loaded at its head: its cook data with records 1.. tagged `tags` (records of the kit's
        items), the frame's body_parts copy and skin name, the profile `name`'s: a profile of the lobby's, or
        with `place` that place of the options. Run to the join; what it made: each record's (base_model,
        part_name, material, tag), the skin's name."""
        profile = PROFILE if place is None else OPTIONS_OBJECT + place * PLACE
        self.emu.mem_write(COOK, bytes(0xF328))
        for i in range(20):
            record = COOK + RECORDS + i * RECORD
            for field in (0, 0x114, 0x228):
                self.string(record + field)
        for i, tag in enumerate(tags, 1):
            record = COOK + RECORDS + i * RECORD
            self.string(record, f"models/item_{i}")
            self.string(record + 0x114, f"part_{i}")
            self.word(record + TAG, tag)
        self.emu.mem_write(COOK + COUNT, bytes([1 + len(tags)]))
        self.emu.mem_write(profile, bytes(0x100))
        self.emu.mem_write(profile + 8, name.encode() + b"\0")
        self.emu.mem_write(SP, bytes(0x500))
        self.word(SP + 0x1C, COOK)
        self.emu.mem_write(SP + 0xA8, body_parts)
        self.word(SP + 0x348, SP + 0x354)
        self.emu.mem_write(SP + 0x354, skin.encode() + b"\0")
        self.word(SP + 0x34C, SP + 0x354 + len(skin))
        self.word(SP + 0x350, SP + 0x458)
        self.word(BP + 0x10, profile)
        regs = {r: 0x0A0A0A0A + i for i, r in enumerate(REGS)}
        regs[UC_X86_REG_EBP] = BP
        regs[UC_X86_REG_ESI] = self.read(SP + 0x1C)          # the span before the site: mov esi,[esp+1Ch]
        regs[UC_X86_REG_EDI] = 0x9FD770                      # mov edi,"head"
        self.go(HEAD, MADE, regs, SP)
        count = self.read(COOK + COUNT, 1)[0]
        made = []
        for i in range(count):
            record = COOK + RECORDS + i * RECORD
            made.append((self.text(self.read(record)), self.text(self.read(record + 0x114)),
                         self.text(self.read(record + 0x228)), self.read(record + TAG)))
        return made, self.text(self.read(SP + 0x348))

    def item_data(self, body_parts, name, slot, item, skin="105_0_0_", place=None):
        """on_configs_loaded at a worn item, its key `data` chosen: the frame's body_parts copy, skin name, item
        config (its root `item`) and profile slot `slot`, the profile `name`'s (a lobby's, or with `place` that
        place of the options). Run past the entry's copy; the copied entry's key (the node's name), its
        type and count, the skin's name."""
        profile = PROFILE if place is None else OPTIONS_OBJECT + place * PLACE
        self.emu.mem_write(profile + 8, name.encode() + b"\0")
        self.emu.mem_write(SP, bytes(0x500))
        root = self.records(1)
        self.emu.mem_write(root, self.node(item))
        self.word(HOLDER + 0x108, root)
        self.word(SP + 0x10, HOLDER)
        self.word(SP + 0x60, slot)
        self.emu.mem_write(SP + 0xA8, body_parts)
        self.word(SP + 0x348, SP + 0x354)
        self.emu.mem_write(SP + 0x354, skin.encode() + b"\0")
        self.word(SP + 0x34C, SP + 0x354 + len(skin))
        self.word(SP + 0x350, SP + 0x458)
        self.word(BP + 0x10, profile)
        regs = {r: 0x0A0A0A0A + i for i, r in enumerate(REGS)}
        regs.update({UC_X86_REG_EBP: BP, UC_X86_REG_EDI: DATA_KEY})
        self.go(ITEM, ITEM_COPIED, regs, SP)
        entry = self.read(SP + 0x20, 24)
        return (self.text(struct.unpack("<I", entry[8:12])[0]), struct.unpack("<HH", entry[20:24]),
                self.text(self.read(SP + 0x348)))

    def own_account(self, name, srp):
        """0x87 read to its last field: the client's profile named `name`, srp's bytes after."""
        self.emu.mem_write(CLIENT + OWN_PROFILE + 8, name.encode() + b"\0")
        reader = self.message(srp)
        regs = {r: 0x0A0A0A0A + i for i, r in enumerate(REGS)}
        regs.update({UC_X86_REG_EBX: CLIENT, UC_X86_REG_ESI: reader, UC_X86_REG_ECX: 0x1234})
        return self.go(OWN, OWN_END, regs, SP)

    def squad_member(self, name, srp):
        self.emu.mem_write(PROFILE + 0x200, name.encode() + b"\0")
        regs = {r: 0x0A0A0A0A + i for i, r in enumerate(REGS)}
        regs.update({UC_X86_REG_EBX: CLIENT, UC_X86_REG_ESI: self.message(srp), UC_X86_REG_EDI: PROFILE + 0x1F8})
        return self.go(MEMBER, MEMBER_END, regs, SP)

    def message(self, srp):
        """A reader {begin, cursor, size} whose cursor is at `srp`, past 4 bytes already read."""
        self.emu.mem_write(MESSAGE, b"\x11\x22\x33\x44" + srp)
        self.word(MESSAGE + 0x80, MESSAGE)
        self.word(MESSAGE + 0x84, MESSAGE + 4)
        self.word(MESSAGE + 0x88, 4 + len(srp))
        return MESSAGE + 0x80

    def proposal(self, place, name, after, item=101):
        """A late join's proposal (0xE4: the header, a profile named `name` of one kit and one item `item`, no
        stack, then `after`) at the dispatch, the client's game's dictionary with item 101. Run the span; the
        reader's size after it, less what it was."""
        self.word(CLIENT + 0x14, GAME)
        self.word(GAME + 0x53A4, DICTIONARY)
        self.word(DICTIONARY + 0x130, ITEMS)
        self.word(DICTIONARY + 0x134, 1)
        self.emu.mem_write(ITEMS, bytes(0x3A0))
        self.emu.mem_write(ITEMS, struct.pack("<H", 101))
        profile = b"\x00" + bytes([len(name)]) + name.encode() + b"\x00" + struct.pack("<IBB", 0, 1, 1)
        profile += b"\x00" + struct.pack("<I", 1) + b"\x01K" + struct.pack("<IHH", 1, 0, 1) + bytes(16)
        profile += struct.pack("<IH", 1, item) + bytes(12) + b"\x00\x00"
        payload = b"\xffLJ\x01" + struct.pack("<II", 0x80001234, 7) + bytes([place, 0]) + profile + after
        self.emu.mem_write(MESSAGE, b"\xE4" + payload)
        reader = MESSAGE + 0x2000
        self.word(reader, MESSAGE)
        self.word(reader + 4, MESSAGE + 1)
        self.word(reader + 8, 1 + len(payload))
        regs = {r: 0x0A0A0A0A + i for i, r in enumerate(REGS)}
        regs.update({UC_X86_REG_EAX: 0xE4, UC_X86_REG_EBX: reader, UC_X86_REG_ESI: CLIENT, UC_X86_REG_EBP: BP})
        after_regs, esp = self.go(DISPATCH, DISPATCH_END, regs, SP)
        return self.read(reader + 8) - 1 - len(payload), after_regs, esp

    def roster(self, names, srp):
        """The options with places named `names`, and the message's bytes after them: srp's block."""
        self.emu.mem_write(OPTIONS_OBJECT, bytes(0x578A0))
        for place, name in enumerate(names):
            self.emu.mem_write(OPTIONS_OBJECT + place * PLACE + 8, name.encode() + b"\0")
        self.emu.mem_write(OPTIONS_OBJECT + 0x577B5, bytes([len(names)]))
        self.emu.mem_write(MESSAGE, srp)

    def replay_options(self, names, srp):
        """A replay's chunk 1 after its deserialize: the options its first argument ([esp+40h]), its local
        reader (esp+14h) at srp's block; the span's pops and its add to the `ret 8`."""
        self.roster(names, srp)
        self.emu.mem_write(SP, bytes(0x100))
        self.word(SP, 0x55555555)
        self.word(SP + 4, 0x66666666)
        for i, value in enumerate((MESSAGE, MESSAGE, len(srp))):
            self.word(SP + 0x14 + 4 * i, value)
        self.word(SP + 0x40, OPTIONS_OBJECT)
        regs = {r: 0x0A0A0A0A + i for i, r in enumerate(REGS)}
        return self.go(REPLAY, REPLAY_END, regs, SP)

    def options(self, names, srp):
        """process_static_match_info after its deserialize: the options (ESI) with places named `names`, the
        frame's reader (EBP-2Ch) at srp's block."""
        self.roster(names, srp)
        self.word(BP - 0x2C, MESSAGE)
        self.word(BP - 0x28, MESSAGE)
        self.word(BP - 0x24, len(srp))
        regs = {r: 0x0A0A0A0A + i for i, r in enumerate(REGS)}
        # the span replayed reads EDI's object: mov edx,[edi] / mov eax,[edx+40h]
        self.word(MESSAGE + 0x100, MESSAGE + 0x100)
        regs.update({UC_X86_REG_ESI: OPTIONS_OBJECT, UC_X86_REG_EBP: BP, UC_X86_REG_EDI: MESSAGE + 0x100})
        return self.go(OPTIONS, OPTIONS_END, regs, SP)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    linked = base.arena()
    reader = base.hooks_026g.original_reader(original)
    ours = [h for h in base.hooks_026g.hooks(exe=reader) if any(m["patch"] == "faces" for m in h.get("chain", [h]))]
    installs = {h[0]: h for h in base.rust_hooks(original)}
    for h in ours:
        va, old, symbol, opcode = installs[h["site"]]
        at = base.file_offset(va)
        new = base.hook_bytes(linked.symbols[symbol], va, old, 0, opcode)
        check(opcode == 0xE9 and original[at:at + len(old)] == old and built[at:at + len(new)] == new,
              f"{va:#x}: {old.hex()} in the original, `jmp {symbol}` here")
    check(sorted(h["site"] for h in ours) == sorted([HEAD, ITEM, OWN, MEMBER, OPTIONS, REPLAY, WORLD_LEFT, DISPATCH]),
          f"faces' eight hooks: its four sites, the two options reads, the match scene's end and the dispatch "
          f"({len(ours)})")
    dispatch = next(h for h in ours if h["site"] == DISPATCH)
    check([m["patch"] for m in dispatch.get("chain", [])] == ["message_check", "faces"],
          "the dispatch's chain: patch 114's check, then the late joiner's face")

    head = {"data": {"base_model": "character/human/base/base_character_head", "part_name": "black_market_01_head_s"}}
    plain = lambda game: game.node({"head": head, "bind_pose": "x"})
    heads = [
        {"data": {"base_model": "head/face_0", "part_name": "face_0"},
         "data_helmet": {"base_model": "head/face_0_helmet", "part_name": "face_0_helmet"},
         "data_mask": {"base_model": "head/face_0_mask", "part_name": "face_0_mask"}},
        {"data": {"base_model": "head/face_1", "part_name": "face_1"}},
        {"data": {"base_model": "head/face_2", "part_name": "face_2", "material": "skin_2",
                  "additional_models": [{"base_model": "head/face_2_hair", "part_name": "hair_2"}]},
         "data_mask": {"base_model": "head/face_2_mask", "part_name": "face_2_mask",
                       "additional_models": [{"base_model": "head/face_2_hair_short", "part_name": "hair_2s"}]}},
    ]
    faces = lambda game: game.node({"head": head, "heads": heads, "bind_pose": "x"})

    stock, patched = Game(original), Game(bytes(built))
    native = stock.cook(plain(stock), "me", [2, 5])
    check(native[0][0] == ("character/human/base/base_character_head", "black_market_01_head_s", "", 0)
          and native[1] == "105_0_0_207_0_0_0_", "the original exe's head: body_parts.head.data, tag 0")
    check(patched.cook(plain(patched), "me", [2, 5]) == native,
          "data without heads: the native head, byte for byte the original's (records, count, the skin's name)")

    made, skin = patched.cook(faces(patched), "nobody", [2, 5])
    check(made[0] == ("head/face_0_helmet", "face_0_helmet", "", 0) and len(made) == 3 and skin == "105_0_0_207_0_0_0_",
          "a player srp said nothing of: face 0, its helmet variant under a helmet; the skin's name as it was")

    after, esp = patched.own_account("me", b"\x02")
    check(after[UC_X86_REG_ESI] == CLIENT and patched.read(CLIENT + 0x1EA96C) == 0x1234 and esp == SP,
          "0x87's hook: the span replayed (mov esi,ebx / mov [ebx+1EA96Ch],ecx), ESP as it was")
    made, skin = patched.cook(faces(patched), "me", [2, 5, 6])
    check(made[0] == ("head/face_2_mask", "face_2_mask", "", 0)
          and made[1:4] == [("models/item_1", "part_1", "", 2), ("models/item_2", "part_2", "", 5),
                            ("models/item_3", "part_3", "", 6)]
          and made[4] == ("head/face_2_hair_short", "hair_2s", "", 0) and len(made) == 5
          and skin == "105_0_0_207_0_0_0_f2_",
          "0x87's face 2 under a mask and a helmet: its mask variant, its hair after the kit's (tag 0), f2_ named")
    made, skin = patched.cook(faces(patched), "me", [2])
    check(made[0] == ("head/face_2", "face_2", "skin_2", 0) and made[2] == ("head/face_2_hair", "hair_2", "", 0)
          and skin.endswith("f2_"), "bare: face 2's data, its material and its hair")
    made, _ = patched.cook(faces(patched), "me", [5])
    check(made[0] == ("head/face_2", "face_2", "skin_2", 0), "a missing variant (face 2's helmet): its data")

    patched.squad_member("mate", b"\x01")
    made, skin = patched.cook(faces(patched), "mate", [2])
    check(made[0][:2] == ("head/face_1", "face_1") and skin.endswith("f1_"), "0x88's byte: the member's face")

    face = lambda made: made[0][0][:2]
    of = lambda n: (f"head/face_{n}", f"face_{n}")
    patched.options(["me", "you", "them"], b"SRPX\x02\x01\x01\x05\x03\x00\x01\x09")
    me, you, them = (patched.cook(faces(patched), name, [2], place=place)
                     for place, name in enumerate(("me", "you", "them")))
    check(face(you) == of(1) and you[1].endswith("f1_") and face(them) == of(0) and them[1] == "105_0_0_207_0_0_0_"
          and face(me) == of(0) and me[1] == "105_0_0_207_0_0_0_",
          "the match's places by its options (entry 5 after the Blackout one): place 1 face 1; face 9, not in the "
          "data, drawn as face 0, no suffix; the account's own place the match's face 0, not the lobby's 2")
    check(face(patched.cook(faces(patched), "me", [2])) == of(2)
          and face(patched.cook(faces(patched), "mate", [2])) == of(1)
          and face(patched.cook(faces(patched), "you", [2])) == of(0),
          "a profile that is no place of the options, the match shown: the lobby's own and squad faces by name, "
          "a match's name face 0")
    patched.options(["a", "b", "c"], b"SRPX\x05\x02\x02\x01")
    short = [face(patched.cook(faces(patched), name, [2], place=place)) for place, name in enumerate("abc")]
    patched.options(["a", "b", "c"], b"SRPX\x05\x05\x01\x02\x01\x02\x02")
    long = [face(patched.cook(faces(patched), name, [2], place=place)) for place, name in enumerate("abc")]
    check(short == [of(2), of(1), of(0)] and long == [of(1), of(2), of(1)],
          "an entry shorter than the roster: the places past it face 0; a longer one: the roster's places theirs")
    patched.options(["you"], b"")
    you = patched.cook(faces(patched), "you", [2], place=0)
    check(face(you) == of(0), "a match without the entry: every face 0, the old match's gone")

    after, esp = patched.replay_options(["seen", "me"], b"SRPX\x05\x02\x02\x01")
    seen, me = (patched.cook(faces(patched), name, [2], place=place) for place, name in enumerate(("seen", "me")))
    check(esp == SP + 0x3C and after[UC_X86_REG_ESI] == 0x55555555 and after[UC_X86_REG_EBX] == 0x66666666
          and face(seen) == of(2) and face(me) == of(1) and face(patched.cook(faces(patched), "me", [2])) == of(2),
          "a replay's chunk 1: the span replayed (pop esi / pop ebx / add esp,34h); its places by its options, "
          "the viewer's own place the recorded face, the lobby's own character still the account's")
    patched.replay_options(["you"], b"")

    before = len(patched.logged)
    made, skin = patched.cook(faces(patched), "me", [2] * 19)
    check(len(made) == 20 and made[0][:2] == ("head/face_2", "face_2") and made[19] == ("models/item_19", "part_19", "", 2)
          and skin.endswith("f2_") and b"no record left" in patched.logged[before:],
          "every record in use (the kit's 19 and the head): the head made, its hair without a record, the count "
          "still 20, logged")

    before = len(patched.logged)
    patched.own_account("me", b"")
    check(b"without srp's face byte" in patched.logged[before:]
          and patched.cook(faces(patched), "me", [2])[0][0][:2] == ("head/face_2", "face_2"),
          "a 0x87 without srp's byte: logged, the face kept")

    patched.options(["me", "you"], b"SRPX\x05\x02\x00\x02")
    cut, after, esp = patched.proposal(2, "late", b"SRPX\x05\x01\x01")
    check(cut == -7 and after[UC_X86_REG_EAX] == 1 and patched.read(BP + 0xC) == 0xE4 and esp == SP,
          "a late joiner's proposal: srp's block taken off (its reader 7 bytes shorter), the span replayed "
          "(mov [ebp+0Ch],eax / add eax,-E3h)")
    made, skin = patched.cook(faces(patched), "late", [2])
    check(made[0][:2] == of(1) and skin.endswith("f1_")
          and face(patched.cook(faces(patched), "late", [2], place=2)) == of(1),
          "the late joiner: its own copy of the profile (the late join cooks from it) by its name, its place once "
          "installed")
    check(face(patched.cook(faces(patched), "you", [2], place=1)) == of(2)
          and face(patched.cook(faces(patched), "me", [2], place=0)) == of(0),
          "the match's other places as they were")
    cut, _, _ = patched.proposal(3, "plain", b"")
    check(cut == 0 and face(patched.cook(faces(patched), "plain", [2], place=3)) == of(0),
          "a proposal without a block: face 0, the message as it was")
    before = len(patched.logged)
    cut, _, _ = patched.proposal(4, "odd", b"SRPX\x05\x02\x01\x01")
    check(cut == 0 and b"no srp block" in patched.logged[before:]
          and face(patched.cook(faces(patched), "odd", [2])) == of(0),
          "bytes after the profile that are no block: logged, left for the late join's check, nothing learnt")
    before = len(patched.logged)
    cut, _, _ = patched.proposal(5, "unknown", b"SRPX\x05\x01\x01", item=999)
    check(cut == 0 and patched.logged[before:] == b""
          and face(patched.cook(faces(patched), "unknown", [2])) == of(0)
          and face(patched.cook(faces(patched), "unknown", [2], place=5)) == of(0),
          "an item the dictionary does not have: the proposal left as it is, nothing learnt")
    check(patched.item_by_id == 0, "the proposals' items looked up among the dictionary's entries, never by "
          "item_by_id (0x1F0090)")
    patched.options(["you"], b"")

    data = {"base_model": "models/mask", "part_name": "mask"}
    mask = {"data": data, "data_head_2": {"base_model": "models/mask_f2", "part_name": "mask_f2"},
            "data_head_1": {"base_model": "models/mask_f1", "part_name": "mask_f1"}, "data_head_9": data}
    native = stock.item_data(plain(stock), "me", 1, mask)
    check(native == ("data", (3, 2), "105_0_0_") and patched.item_data(plain(patched), "me", 1, mask) == native,
          "data without heads: the item's data as the original's (its table of two)")
    check(patched.item_data(faces(patched), "nobody", 1, mask) == native,
          "face 0: the item's data as the original's, the skin's name as it was")
    key, _, skin = patched.item_data(faces(patched), "me", 1, mask)
    check(key == "data_head_2" and skin == "105_0_0_f2_",
          "the account's face 2 in a mask with data_head_2: that entry, f2_ on the skin's name")
    helmet = {"data": data, "data_head_2": {"base_model": "models/helmet_f2", "part_name": "helmet_f2"}}
    check(patched.item_data(faces(patched), "me", 0, helmet)[0] == "data_head_2",
          "a helmet with data_head_2: that entry")
    check(patched.item_data(faces(patched), "me", 0, {"data": data}) == native,
          "an item without the face's key: its data, the skin's name as it was")
    check(patched.item_data(faces(patched), "me", 2, mask)[0] == "data",
          "a torso: its key as the native chose it (here data), whatever keys it has")
    check(patched.item_data(faces(patched), "me", 1, {"data": data, "data_head_2": "x"})[0] == "data",
          "a data_head_2 that is no table: data")
    made, skin = patched.cook(faces(patched), "me", [2, 6], skin="105_0_0_f2_207_")
    check(made[0][:2] == ("head/face_2_mask", "face_2_mask") and skin == "105_0_0_f2_207_",
          "the head after a mask's face entry: face 2's, the skin's name not named twice")
    patched.options(["a", "b", "c"], b"SRPX\x05\x03\x00\x01\x09")
    check(patched.item_data(faces(patched), "b", 1, mask, place=1)[0] == "data_head_1"
          and patched.item_data(faces(patched), "c", 1, mask, place=2) == native,
          "a match's place by its options: place 1 face 1's entry; face 9, not in the data: face 0's data, as its "
          "head")
    patched.options(["you"], b"")

    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
