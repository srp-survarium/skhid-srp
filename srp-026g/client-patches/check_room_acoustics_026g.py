#!/usr/bin/env python3
"""Run patch 155 (rust/patches/src/room_acoustics.rs, the room acoustics,
../docs/notes/room-acoustics.md) in the default build on made-up game objects.

    python check_room_acoustics_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the arena
with them; the config lookups (0xF6B10, 0xF6C00) and the emit's square root are
the game's own code. Stubs: query_resources (0xE5B60, its requests recorded),
the emit's listener and proxy positions (0x57560, 0x5C1F0), the items' sounds
(their vtables: the propagator emitter, its play recorded), GetCurrentThreadId
and the log's kernel32. Checked:

- the cook's item site: an item's `sound_environment` numbered (the outdoors'
  "default" 1, the others from 2 in the order first read, the same name the
  same number), an item without one 0; the hook returns to its span's end with
  the registers as they were;
- composite_sound::emit_sound_propagators (0x63D60) run whole, in the original
  and the patched exe, on a composite of an untagged, a `school_class` and a
  `default` item: the original plays all three wherever; the patched exe,
  before any map with zones, the untagged and `default` ones (today's sound
  with today's content); with a map's zones (its client project's
  `srp_sound_environments` read at project.insert), a placed sound in the
  classroom's box plays the untagged and the classroom's, one outside it the
  untagged and `default`, one in the floor below the box the classroom's (the
  ray up meets the box from below, as 0.69d's two-sided faces); a hud sound
  the listener's room's;
- the listener (game_world.listener): the camera's position without a living
  local player; entering the classroom asks its bed (a class 256 query of its
  path) after the beds' four scenes (class 38), the hall asks the hall's, back
  outdoors asks nothing;
- the project's removal: no zones again (a placed sound in the box plays as
  outdoors), the map's data with a wrong version refused and logged.
"""
import struct
import sys
import zlib

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI,
                               UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP)

import patch_026g as base

ITEM_READ, ITEM_READ_SPAN = 0x60B14, 8
EMIT = 0x63D60
LISTENER, LISTENER_SPAN = 0x1F7642, 6
INSERT, INSERT_SPAN = 0x285750, 6
REMOVED = 0x285614
QUERY, LISTENER_POSITION, PROXY_POSITION = 0xE5B60, 0x57560, 0x5C1F0
THREAD_SLOT = 0x870118
CREATE_FILE_SLOT, WRITE_FILE_SLOT, CLOSE_SLOT = 0x870048, 0x870108, 0x870040
HEAP, STUBS, STACK = 0x30000000, 0x31000000, 0x32000000
CONFIGS, TEXTS = HEAP + 0x40000, HEAP + 0x80000
GAME, WORLD, USER, GAME_WORLD, PROJECT, BINARY, PHYSICS, PROXY = (HEAP + 0x10000 + 0x2000 * i for i in range(8))
COMPOSITE, ITEMS, SOUNDS = HEAP + 0x30000, HEAP + 0x31000, HEAP + 0x32000
GET_EMITTER, PLAY = STUBS + 0x100, STUBS + 0x110
DONE = STUBS + 0xF00
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
        for at in (HEAP, STUBS, STACK - 0x100000):
            emu.mem_map(at, 0x100000, UC_PROT_ALL)
        self.word(THREAD_SLOT, STUBS + 0x80)
        emu.mem_write(STUBS + 0x80, b"\xB8\x01\x00\x00\x00\xC3")
        self.logged = b""
        for slot, stub, code in ((CREATE_FILE_SLOT, STUBS + 0x90, b"\xB8\x07\x00\x00\x00\xC2\x1C\x00"),
                                 (WRITE_FILE_SLOT, STUBS + 0xA0, b"\xB8\x01\x00\x00\x00\xC2\x14\x00"),
                                 (CLOSE_SLOT, STUBS + 0xB0, b"\xB8\x01\x00\x00\x00\xC2\x04\x00")):
            self.word(slot, stub)
            emu.mem_write(stub, code)
        emu.hook_add(UC_HOOK_CODE, self.write_file, begin=STUBS + 0xA0, end=STUBS + 0xA0)
        emu.mem_write(GET_EMITTER, b"\x8B\xC1\xC3")              # mov eax,ecx / ret: the sound is its emitter
        emu.mem_write(PLAY, b"\xC2\x10\x00")                     # ret 10h
        emu.hook_add(UC_HOOK_CODE, self.played, begin=PLAY, end=PLAY)
        for stub in (QUERY, LISTENER_POSITION, PROXY_POSITION):
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=stub, end=stub)
        self.texts, self.configs = TEXTS, CONFIGS
        self.queries, self.plays = [], []
        self.listener = self.source = (0.0, 0.0, 0.0)
        # the game: its sound world and its logic user; no network client
        self.word(GAME + 0x5350, WORLD)
        self.word(WORLD + 0xB0, USER)
        self.word(GAME_WORLD + 0x9C, GAME)
        # a project: its binary config, its scene, physics
        self.word(PROJECT + 0x17C, BINARY)
        self.word(PROJECT + 0x180, HEAP + 0x3F000)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at, size=4):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0] if size == 4 else bytes(self.emu.mem_read(at, size))

    def text(self, at):
        return bytes(self.emu.mem_read(at, 96)).split(b"\0")[0].decode()

    def put_text(self, text):
        at = self.texts
        self.emu.mem_write(at, text.encode() + b"\0")
        self.texts += (len(text) + 16) & ~15
        return at

    def cfg(self, value):
        """A loaded binary config node of `value` (records of 24 bytes: data, name, CRC, type, count; a
        table's children sorted by CRC, as cfg_child's binary search needs): its record's address."""
        at = self.configs
        self.configs += 24
        self.record(at, "", value)
        return at

    def record(self, at, key, value):
        crc = zlib.crc32(key.encode())
        name = self.put_text(key)
        if isinstance(value, dict):
            kids = sorted(((zlib.crc32(k.encode()), k, v) for k, v in value.items()))
            first = self.configs
            self.configs += 24 * len(kids)
            for i, (_, k, v) in enumerate(kids):
                self.record(first + 24 * i, k, v)
            head = struct.pack("<QQIHH", first if kids else 0, name, crc, 3, len(kids))
        elif isinstance(value, str):
            head = struct.pack("<QQIHH", self.put_text(value), name, crc, 5, len(value) + 1)
        elif isinstance(value, float):
            head = struct.pack("<fIQIHH", value, 0, name, crc, 2, 0)
        else:
            head = struct.pack("<QQIHH", value, name, crc, 1, 0)
        self.emu.mem_write(at, head)

    def write_file(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.logged += self.read(self.read(sp + 8), self.read(sp + 12))

    def played(self, emu, *_):
        self.plays.append(SOUNDS_OF.index(emu.reg_read(UC_X86_REG_ECX)))

    def stub(self, emu, address, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        if address == QUERY:              # eax requests, ecx count
            requests, count = emu.reg_read(UC_X86_REG_EAX), emu.reg_read(UC_X86_REG_ECX)
            self.queries.append((self.text(self.read(requests)), self.read(requests + 4), count))
        else:                             # eax the scene or proxy, esi out: eax out
            out = emu.reg_read(UC_X86_REG_ESI)
            emu.mem_write(out, struct.pack("<3f", *(self.listener if address == LISTENER_POSITION else self.source)))
            emu.reg_write(UC_X86_REG_EAX, out)
        emu.reg_write(UC_X86_REG_ESP, sp + 4)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def site(self, at, stop, regs, frame=b"", changed=lambda sp: {}):
        """Runs a lohook site to `stop`: the registers come back as given but those its replayed span
        changes (`changed`, from the site's ESP: {register: value}, ESP among them when it moves)."""
        sp = STACK - 0x8000
        self.emu.mem_write(sp, frame + bytes(0x100))
        given = dict(zip(REGS, (0x0A0A0A0A, 0x0B0B0B0B, 0x0C0C0C0C, 0x0D0D0D0D, 0x05050505, 0x0E0E0E0E,
                                0x0019FF00)))
        given.update(regs)
        for reg, value in given.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.emu_start(at, stop, count=5_000_000)
        expect = {**given, UC_X86_REG_ESP: sp, **changed(sp)}
        return {r: self.emu.reg_read(r) for r in expect} == expect

    def item_room(self, tag):
        """The cook's item site on an item of `tag` (None: no sound_environment): the room byte it keeps."""
        item = {"filename": "x", "enabled": 1} if tag is None else {"filename": "x", "sound_environment": tag}
        sp = STACK - 0x8000
        # the span: mov esi,[esp+24h] (0 here) / lea ecx,[esp+44h]
        kept = self.site(ITEM_READ, ITEM_READ + ITEM_READ_SPAN, {UC_X86_REG_EBX: self.cfg(item)},
                         bytes(0x55) + b"\xEE", lambda sp: {UC_X86_REG_ESI: 0, UC_X86_REG_ECX: sp + 0x44})
        return kept, self.emu.mem_read(sp + 0x55, 1)[0]

    def emit(self, rooms, source, hud=False):
        """emit_sound_propagators on a composite of items with these room bytes: which items played."""
        self.source = source
        self.emu.mem_write(PROXY, bytes(0x100))
        self.word(PROXY + 0x68, HEAP + 0x3E000)
        self.word(PROXY + 0x80, 3 if hud else 0)
        for i, room in enumerate(rooms):
            self.emu.mem_write(ITEMS + 0x18 * i, struct.pack("<IIIffBBH", SOUNDS_OF[i], 0, 0, 0.0, 2000.0, 1,
                                                              room, 0))
        self.emu.mem_write(COMPOSITE, struct.pack("<IHHIII", 0, 0xFFFF, 0, ITEMS, ITEMS + 0x18 * len(rooms), 1))
        sp = STACK - 0x8000
        self.emu.mem_write(sp, struct.pack("<IIIII", DONE, PROXY, 0, 0, 0))
        self.emu.reg_write(UC_X86_REG_ECX, COMPOSITE)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.plays = []
        self.emu.emu_start(EMIT, DONE, count=5_000_000)
        assert self.emu.reg_read(UC_X86_REG_ESP) == sp + 0x14, "ret 10h"
        return self.plays

    def insert(self, root):
        self.word(BINARY + 0x108, self.cfg(root))
        # the span: push ebp / mov ebp,esp / and esp,-8
        return self.site(INSERT, INSERT + INSERT_SPAN, {UC_X86_REG_ECX: PROJECT}, struct.pack("<II", DONE, PHYSICS),
                         lambda sp: {UC_X86_REG_EBP: sp - 4, UC_X86_REG_ESP: (sp - 4) & ~7})

    def frame(self, camera):
        self.emu.mem_write(GAME_WORLD + 0x3C, struct.pack("<3f", *camera))
        self.queries = []
        # the span: lea ecx,[esi+1Ch] / lea edx,[esi+2Ch]
        kept = self.site(LISTENER, LISTENER + LISTENER_SPAN, {UC_X86_REG_ESI: GAME_WORLD}, b"",
                         lambda sp: {UC_X86_REG_ECX: GAME_WORLD + 0x1C, UC_X86_REG_EDX: GAME_WORLD + 0x2C})
        return kept, self.queries


# each item's sound: an object whose vtable's +1Ch gives its emitter (itself), whose
# vtable's first slot plays it
SOUNDS_OF = [SOUNDS + 0x40 * i for i in range(4)]


def sounds(game):
    for sound in SOUNDS_OF:
        game.word(sound, sound + 0x20)
        game.word(sound + 0x20, PLAY)
        game.word(sound + 0x20 + 0x1C, GET_EMITTER)


def zones(version=1):
    box = {"kind": 1, "x": 0.0, "y": 5.0, "z": 0.0, "hx": 4.0, "hy": 1.5, "hz": 4.0,
           "r00": 1.0, "r01": 0.0, "r02": 0.0, "r10": 0.0, "r11": 1.0, "r12": 0.0, "r20": 0.0, "r21": 0.0, "r22": 1.0}
    hall = dict(box, x=20.0)
    return {"srp_sound_environments": {
        "version": version,
        "environments": {
            "class": {"room": "school_class", "ambient_sound": "scapes/rooms_069d/test_class", "fade_in": 250,
                      "fade_out": 300, "meshes": {"m0": box}},
            "hall": {"room": "school_hall", "ambient_sound": "scapes/rooms_069d/test_hall", "fade_in": 200,
                     "fade_out": 200, "meshes": {"m0": hall}},
        }}}


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    old, new = Game(original), Game(bytes(built))
    for game in (old, new):
        sounds(game)

    print("the cook's items:")
    kept, class_byte = new.item_room("school_class")
    check(kept, "the item site returns with the registers as they were")
    check(class_byte == 2, f"school_class numbered 2 (got {class_byte})")
    check(new.item_room("school_class")[1] == 2, "the same name, the same number")
    check(new.item_room("default")[1] == 1, "default is 1, the outdoors'")
    check(new.item_room(None)[1] == 0, "an item without sound_environment: 0 (written over the stack's byte)")
    check(new.item_room("")[1] == 0, "an empty tag: none")
    hall_byte = new.item_room("school_hall")[1]
    check(hall_byte == 3, "school_hall numbered 3")

    rooms = [0, class_byte, 1]                 # untagged, school_class, default
    inside, outside, below = (1.0, 4.5, 1.0), (30.0, 0.0, 30.0), (1.0, 0.0, 1.0)
    print("the composite filter:")
    check(old.emit(rooms, inside) == [0, 1, 2], "the original plays every item")
    check(new.emit(rooms, inside) == [0, 2], "no map's zones: untagged and default (today)")
    check(new.insert({"name": "x"}), "a project without zones inserted")
    check(new.emit(rooms, inside) == [0, 2], "still none")
    check(new.insert(zones()), "the map's project inserted, its registers kept")
    check(new.emit(rooms, inside) == [0, 1], "a placed sound in the classroom: untagged and school_class")
    check(new.emit(rooms, outside) == [0, 2], "one outside every zone: untagged and default")
    check(new.emit(rooms, below) == [0, 1], "one under the box: the ray meets its floor, the classroom")
    check(new.emit(rooms, inside, hud=True) == [0, 2], "a hud sound: the listener's room, outdoors yet")

    print("the listener and the beds:")
    kept, queries = new.frame((30.0, 1.7, 30.0))
    check(kept, "the listener site returns with the registers as they were")
    check(queries == [("sound_scene", 38, 1)] * 4, f"the first frame asks the beds' four scenes ({queries})")
    kept, queries = new.frame((1.0, 4.0, 1.0))
    check(queries == [("scapes/rooms_069d/test_class", 256, 1)], f"into the classroom: its bed asked ({queries})")
    check(new.emit(rooms, outside, hud=True) == [0, 1], "a hud sound now plays the classroom's")
    check(new.frame((1.0, 4.0, 2.0))[1] == [], "within the room: nothing asked")
    check(new.frame((20.0, 4.0, 0.0))[1] == [("scapes/rooms_069d/test_hall", 256, 1)], "the hall: its bed asked")
    check(new.frame((30.0, 1.7, 30.0))[1] == [], "outdoors: nothing asked")

    print("the project's removal:")
    # the span: pop edi / pop ebx / ret (remove_static's epilogue)
    check(new.site(REMOVED, DONE, {UC_X86_REG_ESI: PROJECT}, struct.pack("<III", 0x0E0E0E0E, 0x0B0B0B0B, DONE),
                   lambda sp: {UC_X86_REG_ESP: sp + 12}), "the removal site returns through its epilogue")
    check(new.emit(rooms, inside) == [0, 2], "no zones: the classroom's item silent again")
    new.logged = b""
    check(new.insert(zones(version=2)), "a project of another version inserted")
    check(b"room acoustics" in new.logged and new.emit(rooms, inside) == [0, 2],
          "its data refused and logged; no zones")

    print("all checks passed" if not failed else f"{failed} check(s) FAILED")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
