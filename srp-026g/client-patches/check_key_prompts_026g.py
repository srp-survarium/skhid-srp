#!/usr/bin/env python3
"""Patch 212: keys in texts (README "Keys in texts").

python check_key_prompts_026g.py ORIGINAL [PATCHED]

Runs the real text_translator::translate_text (0x1FF0F0) and its localizer
expand_text (0x1FEF90) in both exes, the built one's through the Rust lohook
`text_translator.expand_name` (key_prompts.rs), on a made-up game: a
translator whose dictionary has the texts below and the key names' texts, and
a key binder whose bindings are every row of the exe's own action table
(each row in the binding of its id, as key_binder::key_binder does; the
build's table is patch 149's, read from action_name_to_ptr's immediates) and
whose keys are rows of the exe's own key names' table (0xA74FA0).

The original: {USE} is Use's key, any other name nothing, an unbound Use a
fault (translate_text of NULL, 0x1FF09D). The build: every action's name, in
any case, is its key's text (the first key, else the second), an action with
no key "unbound" (st_srp_key_unbound), any other name nothing, "{{" a brace.
Every {NAME} the dictionaries (localization/*.tsv) hold is an action of the
build's table.
"""
import re
import struct
import sys
import zlib
from pathlib import Path
from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32
from unicorn.x86_const import UC_X86_REG_ECX, UC_X86_REG_ESP, UC_X86_REG_EIP
import patch_026g as base

TRANSLATE, EXPAND_JNE, EXPAND_NAME = 0x1FF0F0, 0x1FF037, 0x1FF0AB
SECTION = 0x9FC84C                                 # the dictionary's name translate_text looks up
ACTION_TABLE_REF, ACTION_TABLE_END = 0x207AE8, 0x207B03   # action_name_to_ptr: [esi+table], cmp esi,end
KEYS, KEY_STRIDE = 0xA74FA0, 0x88
HEAP, STACK = 0x30000000, 0x30200000
GAME, OWNER, ROOT, NODES, STRINGS, BINDER, OUT = (HEAP + i * 0x10000 for i in range(7))
SENTINEL = 0x30300000
BINDINGS = 0x60
DICTIONARY = {
    b"st_pick": b"Pick up ({USE})", b"st_lower": b"Use: {use}", b"st_sight": b"Switch ({SCOPE_NEXT})",
    b"st_ui": b"Controls: {HIDE_INTERFACE}", b"st_unknown": b"for {s} s", b"st_braces": b"{{x}",
    b"st_fire": b"Fire ({WPN_FIRE})", b"st_srp_key_unbound": b"unbound",
    b"kE": b"E", b"kF": b"F", b"kY": b"Y", b"kF9": b"F9", b"mouse1": b"LMB",
}


class Game:
    def __init__(self, data):
        self.u = u = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vs, raw, size, _ in base.Headers(data).sections(data):
            u.mem_map(base.IMAGE_BASE + rva, (max(vs, size) + 4095) & ~4095)
            u.mem_write(base.IMAGE_BASE + rva, bytes(data[raw:raw + min(size, vs or size)]))
        u.mem_map(HEAP, 0x80000)
        u.mem_map(STACK, 0x10000)
        u.mem_map(SENTINEL, 0x1000)
        self.text = STRINGS
        section = bytes(u.mem_read(SECTION, 64)).split(b"\0")[0]
        # binary_config nodes as check_match_stat_map_026g.py makes them: 24 bytes, +0 children (a
        # value's text), +8 the name, +10h ~CRC-32 of the name (children sorted by it), +16h the count.
        words = sorted(DICTIONARY.items(), key=lambda kv: zlib.crc32(kv[0]))
        entries = NODES + 0x100
        for i, (key, value) in enumerate(words):
            self.node(entries + i * 24, self.string(value), key, 0)
        self.node(NODES, entries, section, len(words))
        self.node(ROOT, NODES, b"", 1)
        self.word(OWNER + 0x108, ROOT)
        # The translator in its game (+52A8h): +0 the dictionary's owner, +8 the game (0x1E90FC).
        # The game's +B4h is its world's game (+18h, +9Ch: itself), +5344h the key binder.
        self.translator = GAME + 0x52A8
        for at, value in ((self.translator, OWNER), (self.translator + 8, GAME), (GAME + 0xB4, GAME),
                          (GAME + 0x5344, BINDER)):
            self.word(at, value)
        self.keys = {}
        for i in range(0x200):
            name_at, code = struct.unpack("<II", bytes(u.mem_read(KEYS + i * KEY_STRIDE, 8)))
            if name_at == 0:
                break
            self.keys.setdefault(self.c_string(name_at), KEYS + i * KEY_STRIDE)
        table = self.dword(ACTION_TABLE_REF)
        end = self.dword(ACTION_TABLE_END)
        self.actions = {}
        for row in range(table, table + end, 20):
            name, ident = self.c_string(self.dword(row)), self.dword(row + 4)
            assert ident < BINDINGS and name not in self.actions, (name, hex(ident))
            self.actions[name] = ident
            self.word(BINDER + ident * 12, row)      # the binding's description: its row

    def dword(self, at): return struct.unpack("<I", bytes(self.u.mem_read(at, 4)))[0]

    def word(self, at, value): self.u.mem_write(at, struct.pack("<I", value))

    def c_string(self, at): return bytes(self.u.mem_read(at, 0x80)).split(b"\0")[0].decode()

    def string(self, text):
        at, self.text = self.text, self.text + (len(text) + 16 & ~15)
        self.u.mem_write(at, text + b"\0")
        return at

    def node(self, at, children, name, count):
        self.u.mem_write(at, struct.pack("<IIIIIHH", children, 0, self.string(name), 0,
                                         zlib.crc32(name), 0, count))

    def bind(self, action, first=None, second=None):
        """Binding of `action`: its keys by name (None: none)."""
        at = BINDER + self.actions[action] * 12
        self.word(at + 4, self.keys[first] if first else 0)
        self.word(at + 8, self.keys[second] if second else 0)

    def translate(self, key):
        """translate_text(key) into a buffer, or the address the game faults at."""
        sp = STACK + 0x8000
        self.u.mem_write(OUT, b"\xAA" * 0x200)
        self.u.mem_write(sp, struct.pack("<III", SENTINEL, self.string(key), OUT))
        self.u.reg_write(UC_X86_REG_ESP, sp)
        self.u.reg_write(UC_X86_REG_ECX, self.translator)
        try:
            self.u.emu_start(TRANSLATE, SENTINEL, count=2_000_000)
        except UcError:
            return self.u.reg_read(UC_X86_REG_EIP)
        assert self.u.reg_read(UC_X86_REG_EIP) == SENTINEL
        assert self.u.reg_read(UC_X86_REG_ESP) == sp + 12, "translate_text pops its two arguments"
        return bytes(self.u.mem_read(OUT, 0x200)).split(b"\0")[0].decode()


def placeholders(catalog):
    """Every {NAME} of the dictionaries' texts, with a key that has it."""
    found = {}
    for path in sorted(Path(catalog).glob("*.tsv")):
        for line in path.read_text(encoding="utf-8").splitlines()[1:]:
            key, _, text = line.partition("\t")
            for name in re.findall(r"(?<!\{)\{([^{}]+)\}", text):
                found.setdefault(name, f"{path.stem}:{key}")
    return found


def checks(original, built):
    stock, fixed = Game(original), Game(built)
    assert bytes(stock.u.mem_read(EXPAND_JNE, 2)) == bytes.fromhex("7572")
    assert bytes(fixed.u.mem_read(EXPAND_JNE, 2)) == bytes.fromhex("eb72"), "every name to 0x1FF0AB"
    assert bytes(stock.u.mem_read(EXPAND_NAME, 2)) == bytes.fromhex("32c0")
    jump = bytes(fixed.u.mem_read(EXPAND_NAME, 5))
    assert jump[0] == 0xE9 and EXPAND_NAME + 5 + struct.unpack("<i", jump[1:])[0] >= base.ARENA_VA, jump.hex()
    assert {"use", "scope_next", "hide_interface", "wpn_fire"} <= fixed.actions.keys(), sorted(fixed.actions)
    for game in (stock, fixed):
        game.bind("use", "kE")
        game.bind("wpn_fire", "mouse1")
        if "scope_next" in game.actions:
            game.bind("scope_next", "kY")
        game.bind("hide_interface" if "hide_interface" in game.actions else "toggle_replay_ui", "kF9")

    # Default keys: Use is E in both; the build names every action's key.
    assert stock.translate(b"st_pick") == fixed.translate(b"st_pick") == "Pick up (E)"
    assert fixed.translate(b"st_lower") == "Use: E", "a name in any case"
    assert fixed.translate(b"st_sight") == "Switch (Y)"
    assert fixed.translate(b"st_ui") == "Controls: F9"
    assert fixed.translate(b"st_fire") == "Fire (LMB)"
    assert stock.translate(b"st_sight") == "Switch ()", "the original names Use alone"
    for game in (stock, fixed):
        assert game.translate(b"st_unknown") == "for  s", "a name of no action is nothing, as before"
        assert game.translate(b"st_braces") == "{x", "{{ is a brace"
    # Rebound.
    for game in (stock, fixed):
        game.bind("use", "kF")
    assert stock.translate(b"st_pick") == fixed.translate(b"st_pick") == "Pick up (F)"
    fixed.bind("scope_next", None, "kE")
    assert fixed.translate(b"st_sight") == "Switch (E)", "the second key when there is no first"
    # Unbound.
    for game in (stock, fixed):
        game.bind("use")
    assert isinstance(stock.translate(b"st_pick"), int), "the original translates NULL for an unbound {USE}"
    assert fixed.translate(b"st_pick") == "Pick up (unbound)"
    fixed.bind("scope_next")
    assert fixed.translate(b"st_sight") == "Switch (unbound)"
    # A text with no dictionary entry is its key, expanded the same.
    assert fixed.translate(b"Hold {WPN_FIRE}") == "Hold LMB"

    names = placeholders(Path(__file__).resolve().parent / "localization")
    actions = {name.lower() for name in fixed.actions}
    unknown = {name: where for name, where in names.items() if name.lower() not in actions}
    assert not unknown, f"placeholders that name no action of the build's table: {unknown}"
    print(f"ok: expand_text names every action's key (default, rebound, second key, unbound, any case), "
          f"nothing for other names; the original Use alone and faults unbound; the dictionaries' "
          f"{len(names)} placeholders ({', '.join(sorted(names))}) are actions of the build")


if __name__ == "__main__":
    original = Path(sys.argv[1]).read_bytes()
    if len(sys.argv) > 2:
        built = Path(sys.argv[2]).read_bytes()
    else:
        built, _ = base.build(original, "section", Path(base.default_bin("section")).read_bytes(), None,
                              Path(base.late_join_bin()).read_bytes())
    checks(original, built)
