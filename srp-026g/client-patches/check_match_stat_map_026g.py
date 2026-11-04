#!/usr/bin/env python3
"""The results screen's map name in a game that has loaded no match.

python check_match_stat_map_026g.py ORIGINAL [PATCHED]

Runs lobby_menu::fill_match_statistic from its map-name read (0x29ABE1)
through the real text_translator::translate_text (0x1FF0F0) and its
binary_config lookups (find 0xF6B10, value_exists 0xF6C00, the CRC table
0x1C210, the search 0xF6D50) and the copy into the screen's buffer
(0x1FEF90), on a made-up translator config with one dictionary. Nothing is
stubbed. lobby_menu+313Ch NULL (no match loaded): the stock game faults
reading address 0 at 0xF6C47 (the live crash), the patched one shows "".
A map key, translated or not, gives the same text in both.
"""
import struct
import sys
import zlib
from pathlib import Path
from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32
from unicorn.x86_const import (UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_ESI, UC_X86_REG_EDI,
    UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP)
import patch_026g as base

SITE, AFTER = 0x29ABE1, 0x29AC09                   # the map name's read, after translate_text
FAULT = 0xF6C47                                    # value_exists' strlen of the key
SECTION = 0x9FC84C                                 # the dictionary's name translate_text looks up
HEAP, STACK = 0x30000000, 0x30100000
MENU, HOLDER, OWNER, ROOT, NODES, STRINGS = (HEAP + i * 0x10000 for i in range(6))
TRANSLATOR = HOLDER + 0x52A8
DICTIONARY = {b"mp_bath": b"Bath", b"mp_rudnya": b"Rudnya"}


class Game:
    def __init__(self, data):
        self.u = u = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vs, raw, size, _ in base.Headers(data).sections(data):
            u.mem_map(base.IMAGE_BASE + rva, (max(vs, size) + 4095) & ~4095)
            u.mem_write(base.IMAGE_BASE + rva, bytes(data[raw:raw + min(size, vs or size)]))
        u.mem_map(HEAP, 0x60000)
        u.mem_map(STACK, 0x10000)
        self.text = STRINGS
        section = bytes(u.mem_read(SECTION, 64)).split(b"\0")[0]
        assert section, "no dictionary name at 0x9FC84C"
        # binary_config nodes: 24 bytes, +0 children (or a string value's
        # text), +8 the name, +10h ~CRC-32 of the name (the search's key,
        # children sorted by it), +16h the children's count.
        words = sorted(DICTIONARY.items(), key=lambda kv: zlib.crc32(kv[0]))
        entries = NODES + 0x100
        for i, (key, value) in enumerate(words):
            self.node(entries + i * 24, self.string(value), key, 0)
        self.node(NODES, entries, section, len(words))
        self.node(ROOT, NODES, b"", 1)
        self.word(OWNER + 0x108, ROOT)
        self.word(TRANSLATOR, OWNER)
        self.word(MENU + 0x9C, HOLDER)

    def word(self, at, value): self.u.mem_write(at, struct.pack("<I", value))

    def string(self, text):
        at, self.text = self.text, self.text + (len(text) + 16 & ~15)
        self.u.mem_write(at, text + b"\0")
        return at

    def node(self, at, children, name, count):
        self.u.mem_write(at, struct.pack("<IIIIIHH", children, 0, self.string(name), 0,
                                         zlib.crc32(name), 0, count))

    def results(self, name):
        """The text fill_match_statistic gets for lobby_menu+313Ch = name
        (None: NULL), or the address the game faults at."""
        self.word(MENU + 0x313C, 0 if name is None else self.string(name))
        sp = STACK + 0x8000
        out = sp + 0x740
        self.u.mem_write(out, b"\xAA" * 0x200)
        saved = {UC_X86_REG_EBX: 0, UC_X86_REG_ESI: MENU, UC_X86_REG_EDI: 0x13570001, UC_X86_REG_EBP: 0x13570002}
        for reg, value in saved.items(): self.u.reg_write(reg, value)
        self.u.reg_write(UC_X86_REG_ESP, sp)
        self.u.reg_write(UC_X86_REG_ECX, 0x2468ACE0)
        try:
            self.u.emu_start(SITE, AFTER, count=100000)
        except UcError:
            return self.u.reg_read(UC_X86_REG_EIP)
        assert self.u.reg_read(UC_X86_REG_EIP) == AFTER
        assert self.u.reg_read(UC_X86_REG_ESP) == sp, "translate_text pops its two arguments"
        assert {reg: self.u.reg_read(reg) for reg in saved} == saved
        return bytes(self.u.mem_read(out, 0x200)).split(b"\0")[0].decode()


def checks(original, built):
    stock, fixed = Game(original), Game(built)
    jump = bytes(fixed.u.mem_read(SITE, 6))
    target = SITE + 5 + struct.unpack("<i", jump[1:5])[0]
    assert jump[0] == 0xE9 and jump[5] == 0x90 and target >= base.ARENA_VA, jump.hex()
    assert bytes(stock.u.mem_read(SITE, 6)) == bytes.fromhex("8b8e3c310000")
    assert stock.results(None) == FAULT, "the stock game faults in value_exists' strlen"
    assert fixed.results(None) == "", "the patched game shows no map name"
    for name, text in ((b"mp_bath", "Bath"), (b"mp_rudnya", "Rudnya"), (b"mp_unknown", "mp_unknown")):
        assert stock.results(name) == fixed.results(name) == text, name
    assert fixed.results(None) == "", "and again after a named map"
    print("ok: stock faults at 0xF6C47 on a NULL map name; patched shows \"\", map names unchanged")


if __name__ == "__main__":
    original = Path(sys.argv[1]).read_bytes()
    if len(sys.argv) > 2:
        built = Path(sys.argv[2]).read_bytes()
    else:
        built, _ = base.build(original, "section", Path(base.default_bin("section")).read_bytes(), None,
                              Path(base.late_join_bin()).read_bytes())
    checks(original, built)
