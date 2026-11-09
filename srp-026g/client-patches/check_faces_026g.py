#!/usr/bin/env python3
"""Run patch 123 (rust/patches/src/faces.rs, the hooks of faces.toml: every
player drawn with srp's face, ../docs/notes/face-choice.md) in the default
build and the game's own code in the original exe, on made-up game objects.

    python check_faces_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them; the config lookups (0xF6B10, 0xF6C00) and buffer_string's
assign (0x129BB0) are the game's own code. Stubbed: GetCurrentThreadId (one
thread), QueryPerformanceCounter/-Frequency and the log's kernel32 (its lines
kept). Each run starts at a site
and stops at its span's end (or the head's join, 0x2056F0, where the native
head ends too). Checked:

- the three sites of its own in place (`jmp` to its entry, the original's
  bytes there); the two options reads its chain's;
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
- the match's options (`static_info.options_read`, srp's block entry 3): each
  place's face by its name; a face the data does not list drawn as face 0,
  with no suffix; the lobby's face asked first; a new match without the entry
  forgets the old match's faces;
- a `0x87` without srp's byte: logged, nothing learnt;
- a late joiner's proposal through the late join's 0xE4 case (patch 11,
  check_late_join_026g.py's `Join`: its match ready, the case's Rust, the
  game's own profile reader into the proposal's storage) in the same
  emulator, then the cook: srp's block after the profile (`SRPX 03 01
  <face>`) gives the joiner's face by its name, the profile stored and its
  players asked for; the match's other places as they were; without a block,
  face 0 and the joiner taken; a block srp does not send, the proposal
  answered failed (nothing stored, nothing learnt).
"""
import struct
import sys
import zlib

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI,
                               UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP)

import patch_026g as base
import check_late_join_026g as late

HEAD, MADE = 0x205662, 0x2056F0            # skin_cook.head and its join
OWN, OWN_END = 0x27030B, 0x270313           # lobby_client.own_account, span 8
MEMBER, MEMBER_END = 0x274C01, 0x274C07     # lobby_client.squad_member, span 6
OPTIONS, OPTIONS_END = 0x27B92D, 0x27B932   # static_info.options_read, span 5
THREAD_SLOT, QPC_SLOT, QPF_SLOT = 0x870118, 0x870158, 0x870150
CREATE_FILE_SLOT, WRITE_FILE_SLOT, CLOSE_SLOT = 0x870048, 0x870108, 0x870040
HEAP, STUBS, STACK = 0x30000000, 0x31000000, 0x32000000
CONFIG, TEXTS = HEAP, HEAP + 0x40000
COOK, PROFILE, CLIENT = HEAP + 0x80000, HEAP + 0xA0000, 0x33000000
OPTIONS_OBJECT, MESSAGE = 0x34000000, HEAP + 0xB0000
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
    def __init__(self, data, emu=None):
        """The exe's sections in an emulator of their own, or in `emu`'s pages it has not mapped (a late join's
        fixture, whose own pages and what it wrote there are kept)."""
        fresh = emu is None
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32) if fresh else emu
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            size = (max(vsize, rsize) + 0xFFF) & ~0xFFF
            content = bytes(data[raw:raw + min(rsize, vsize or rsize)])
            if fresh:
                emu.mem_map(start, size, UC_PROT_ALL)
                emu.mem_write(start, content)
                continue
            taken = [(begin, end + 1) for begin, end, _ in emu.mem_regions()]
            free = [page for page in range(start, start + size, 0x1000)
                    if not any(begin <= page < end for begin, end in taken)]
            runs = []                                       # the free pages, one region a run of them
            for page in free:
                if runs and runs[-1][1] == page:
                    runs[-1][1] = page + 0x1000
                else:
                    runs.append([page, page + 0x1000])
            for begin, end in runs:
                emu.mem_map(begin, end - begin, UC_PROT_ALL)
                piece = content[begin - start:end - start]
                if piece:
                    emu.mem_write(begin, piece)
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

    def cook(self, body_parts, name, tags, skin="105_0_0_207_0_0_0_"):
        """on_configs_loaded at its head: its cook data with records 1.. tagged `tags` (records of the kit's
        items), the frame's body_parts copy and skin name, the profile `name`'s. Run to the join; what it made:
        each record's (base_model, part_name, material, tag), the skin's name."""
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
        self.emu.mem_write(PROFILE, bytes(0x100))
        self.emu.mem_write(PROFILE + 8, name.encode() + b"\0")
        self.emu.mem_write(SP, bytes(0x500))
        self.word(SP + 0x1C, COOK)
        self.emu.mem_write(SP + 0xA8, body_parts)
        self.word(SP + 0x348, SP + 0x354)
        self.emu.mem_write(SP + 0x354, skin.encode() + b"\0")
        self.word(SP + 0x34C, SP + 0x354 + len(skin))
        self.word(SP + 0x350, SP + 0x458)
        self.word(BP + 0x10, PROFILE)
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

    def options(self, names, srp):
        """process_static_match_info after its deserialize: the options (ESI) with places named `names`, the
        frame's reader (EBP-2Ch) at srp's block."""
        self.emu.mem_write(OPTIONS_OBJECT, bytes(0x578A0))
        for place, name in enumerate(names):
            self.emu.mem_write(OPTIONS_OBJECT + place * PLACE + 8, name.encode() + b"\0")
        self.emu.mem_write(OPTIONS_OBJECT + 0x577B5, bytes([len(names)]))
        self.emu.mem_write(MESSAGE, srp)
        self.word(BP - 0x2C, MESSAGE)
        self.word(BP - 0x28, MESSAGE)
        self.word(BP - 0x24, len(srp))
        regs = {r: 0x0A0A0A0A + i for i, r in enumerate(REGS)}
        # the span replayed reads EDI's object: mov edx,[edi] / mov eax,[edx+40h]
        self.word(MESSAGE + 0x100, MESSAGE + 0x100)
        regs.update({UC_X86_REG_ESI: OPTIONS_OBJECT, UC_X86_REG_EBP: BP, UC_X86_REG_EDI: MESSAGE + 0x100})
        return self.go(OPTIONS, OPTIONS_END, regs, SP)


def joiner_profile(name):
    """A late joiner's profile named `name`: one usable kit, no items (check_late_join_026g.py's profile_wire)."""
    wire = b"\x01" + bytes([len(name)]) + name.encode() + b"\0" + struct.pack("<I", 10) + b"\x01\x00"
    return wire + b"\x00" + struct.pack("<I", 1) + b"\0" + bytes(20)


def late_joiner(built, faces, name, block, places=("me", "you"), srp=b"SRPX\x03\x02\x00\x02"):
    """A match of `places` (its options' faces `srp`), its late join ready, then the server's proposal of place 2
    for `name`, its profile followed by `block`, through the 0xE4 case; the cook of `name` and of each place after
    it, in the same emulator. The late join's fixture, and what each cook made (its head's record, the skin's
    name)."""
    join = late.Join(bytes(built))
    game = Game(built, join.m.uc)
    game.options(list(places), srp)
    join.ready()
    join.prepare(joiner_profile(name) + block)
    drawn = {who: game.cook(faces(game), who, [2]) for who in (name, *places)}
    return join, {who: (made[0][:2], skin) for who, (made, skin) in drawn.items()}


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, True)
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
    check(sorted(h["site"] for h in ours) == sorted([HEAD, OWN, MEMBER, OPTIONS, 0x1F0971]),
          f"faces' five hooks: its three sites and the two options reads ({len(ours)})")

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

    patched.options(["me", "you", "them"], b"SRPX\x02\x01\x01\x03\x03\x00\x01\x09")
    you, them, me = (patched.cook(faces(patched), name, [2]) for name in ("you", "them", "me"))
    check(you[0][0][:2] == ("head/face_1", "face_1") and you[1].endswith("f1_")
          and them[0][0][:2] == ("head/face_0", "face_0") and them[1] == "105_0_0_207_0_0_0_"
          and me[0][0][:2] == ("head/face_2", "face_2"),
          "the match's faces by name (entry 3 after the Blackout one); face 9, not in the data, drawn as face 0, "
          "no suffix; the lobby's face asked first")
    patched.options(["you"], b"")
    you = patched.cook(faces(patched), "you", [2])
    check(you[0][0][:2] == ("head/face_0", "face_0"), "a match without the entry: every face 0, the old match's gone")

    before = len(patched.logged)
    patched.own_account("me", b"")
    check(b"without srp's face byte" in patched.logged[before:]
          and patched.cook(faces(patched), "me", [2])[0][0][:2] == ("head/face_2", "face_2"),
          "a 0x87 without srp's byte: logged, the face kept")

    face = lambda n: ((f"head/face_{n}", f"face_{n}"), "105_0_0_207_0_0_0_" + (f"f{n}_" if n else ""))
    join, drawn = late_joiner(built, faces, "late", b"SRPX\x03\x01\x01")
    stored = join.context + late.PROFILE_AT
    check(join.calls == ["allocate", "query"] and join.size == late.PROFILE_AT + 0x45FC + len(joiner_profile("late")) + 7
          and bytes(join.m.uc.mem_read(stored + 8, 5)) == b"late\0" and not join.messages,
          "a late joiner's proposal with srp's block: taken (its body stored whole, the profile read into the "
          "storage's body by the game's reader), its players asked for")
    check(drawn == {"late": face(1), "me": face(0), "you": face(2)},
          "the late joiner's face by its name (srp's block after its profile), the match's places as they were")
    join, drawn = late_joiner(built, faces, "plain", b"")
    check(join.calls == ["allocate", "query"] and drawn["plain"] == face(0) and drawn["you"] == face(2),
          "a proposal without a block: face 0, the joiner taken")
    join, drawn = late_joiner(built, faces, "you", b"SRPX\x03\x01\x01")
    check(join.calls == ["allocate", "query"] and drawn["you"] == face(1),
          "one who left and comes back (its old place's face 2): its new place's face")
    for block in (b"SRPX\x03\x02\x01\x01", b"SRPX\x03\x01", b"SRPY\x03\x01\x01", b"\x00"):
        join, drawn = late_joiner(built, faces, "odd", block)
        check(join.calls == ["new_packet", "enqueue"] and not join.constructed
              and join.messages == [join.ack(123, 2, 1)] and drawn["odd"] == face(0),
              f"bytes after the profile that are no block srp sends ({block.hex()}): answered failed, nothing "
              "stored, nothing learnt")

    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
