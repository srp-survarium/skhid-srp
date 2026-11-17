#!/usr/bin/env python3
"""Patch 157 (the Rust patch console_paste, rust/patches/src/console_paste.rs):
Ctrl+V pastes the clipboard's first line into the console's line
(docs/notes/console-paste.md, client_patch.md section 3.83).

    python check_console_paste_026g.py <original survarium.exe>

Requires Python `unicorn`. The original exe and the build, each with its
sections and arena as they are. The game's own code makes a text edit
(ui_world::create_text_edit 0x346610: the constructor, its key bindings,
init_internals 0x34A5E0) and takes every key through its keyboard action
(0x348A10: the bindings, the modifiers, a typed character's delete and insert,
set_text 0x3479D0, undo). Stubbed: the allocator (a bump allocator), the
keyboard's key names (none: a letter types its binding's own character), the
timer of a key's repeat (0x106A00: 0.0), the log line of the engine's stubs
(log_message 0x111ED0: counted), and Windows: GetModuleHandleA,
GetProcAddress, user32's OpenClipboard, GetClipboardData, CloseClipboard,
kernel32's GlobalLock, GlobalUnlock and GlobalSize over a made-up clipboard.

Checked, the same keys in both exes:
- typing (W H E R E, Shift, Ctrl+A, Ctrl+Z, Ctrl+Shift+V, Ctrl+C, Ctrl+X,
  plain V) does the same in both; Ctrl+V and Shift+Insert reach
  paste_from_clipboard in both, the only references to it;
- the original's Ctrl+V logs "not implemented" and leaves the line;
- the build's pastes the clipboard's text at the caret as typed: one
  set_text (one text-changed event, one undo step), the caret after it, the
  selection replaced (deleted first, as a typed character's); the first line
  alone (CR LF, LF, a tab, U+2028 end it), characters other than printable
  ASCII left out; cut to the line's max length (0FFh, or a smaller one set),
  the text already there kept; no text (none on the clipboard, another
  program holding it, an empty first line, no room): nothing changes, the
  selection stays; the clipboard closed and its memory unlocked each time,
  read no further than GlobalSize says.
"""
import struct
import sys

from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI, UC_X86_REG_EIP,
                               UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_EBP)

import patch_026g as base

CREATE_TEXT_EDIT, KEYBOARD_ACTION, PASTE = 0x346610, 0x348A10, 0x3482C0
TEXT_CHANGED = 0x283D00                 # the base's event 5 handler (xor al,al / ret 0Ch): one call a set_text
TIMER = 0x106A00                        # the repeat's clock (fs:2Ch): stubbed fldz / ret
LOG_MESSAGE = 0x111ED0                  # cdecl
LOCALE_GUARD, LOCALE = 0xA2644B4, 0x2260CA4   # the character binding's _create_locale(LC_ALL, ""), never made
GET_MODULE, GET_PROC = 0x87003C, 0x870084
OPEN_CLIP, GET_CLIP, CLOSE_CLIP = 0xAEFEDC, 0xAEFED8, 0xAEFED4
GLOBAL_LOCK, GLOBAL_UNLOCK = 0x870078, 0x87007C
CF_UNICODETEXT = 13

STACK, HEAP, OBJECTS, STUBS, CLIP, RETURN = 0x20000000, 0x21000000, 0x22000000, 0x23000000, 0x24000000, 0x25000000
WORLD, ALLOCATOR, ALLOCATOR_VT, KEYS, KEYS_VT, NAMES, NAMES_VT = (OBJECTS + 0x100 * i for i in range(7))
KERNEL32, HANDLE = 0x7C800000, 0x00C11B00
KEPT = {UC_X86_REG_EBX: 0x0B0B0B0B, UC_X86_REG_ESI: 0x05050505, UC_X86_REG_EDI: 0x0E0E0E0E,
        UC_X86_REG_EBP: 0x0019FF00}

DOWN, UP = 1, 2
LCTRL, LSHIFT, INSERT, HOME, END = 0x1D, 0x2A, 0xD2, 0xC7, 0xCF
A, C, V, X, Z = 0x1E, 0x2E, 0x2F, 0x2D, 0x2C
WHERE = (0x11, 0x23, 0x12, 0x13, 0x12)


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((STACK, 0x20000), (HEAP, 0x400000), (OBJECTS, 0x1000), (STUBS, 0x1000),
                         (CLIP, 0x20000), (RETURN, 0x1000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        emu.mem_write(TIMER, b"\xD9\xEE\xC3")
        self.word(LOCALE_GUARD, self.read(LOCALE_GUARD) | 1)
        self.word(LOCALE, 0)
        self.heap, self.sizes, self.logged, self.changed = HEAP, {}, 0, 0
        self.clipboard, self.openable, self.extra = None, True, b""
        self.calls, self.open, self.locked = [], False, False
        stubs = {}

        def stub(name, pops, action):
            at = STUBS + 0x10 * len(stubs)
            stubs[at] = (name, pops, action)
            return at

        self.word(WORLD + 0x10, ALLOCATOR)
        self.word(WORLD + 4, KEYS)
        self.word(ALLOCATOR, ALLOCATOR_VT)
        for slot in range(0, 0x40, 4):
            self.word(ALLOCATOR_VT + slot, stub(f"allocator+{slot:#x}", 0, None))
        self.word(ALLOCATOR_VT + 0x10, stub("malloc", 0x14, self.malloc))
        self.word(ALLOCATOR_VT + 0x14, stub("realloc", 0x18, self.realloc))
        self.word(ALLOCATOR_VT + 0x18, stub("free", 0x10, lambda args: 0))
        self.word(KEYS, KEYS_VT)
        self.word(KEYS_VT + 0x24, stub("keyboard", 0, lambda args: NAMES))
        self.word(NAMES, NAMES_VT)
        self.word(NAMES_VT + 4, stub("key name", 0xC, lambda args: 0))
        for slot, name, pops, action in (
                (GET_MODULE, "GetModuleHandleA", 4, self.module),
                (GET_PROC, "GetProcAddress", 8, self.proc),
                (OPEN_CLIP, "OpenClipboard", 4, self.open_clipboard),
                (GET_CLIP, "GetClipboardData", 4, self.clipboard_data),
                (CLOSE_CLIP, "CloseClipboard", 0, self.close_clipboard),
                (GLOBAL_LOCK, "GlobalLock", 4, self.lock),
                (GLOBAL_UNLOCK, "GlobalUnlock", 4, self.unlock)):
            self.word(slot, stub(name, pops, action))
        self.global_size = stub("GlobalSize", 4, self.size)
        self.stubs = stubs
        emu.hook_add(UC_HOOK_CODE, self.on_stub, begin=STUBS, end=STUBS + 0xFFF)
        emu.hook_add(UC_HOOK_CODE, self.on_log, begin=LOG_MESSAGE, end=LOG_MESSAGE)
        emu.hook_add(UC_HOOK_CODE, self.on_changed, begin=TEXT_CHANGED, end=TEXT_CHANGED)
        self.edit = self.call(CREATE_TEXT_EDIT, [], ecx=WORLD, edi=WORLD)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at, size=4):
        return struct.unpack({4: "<I", 2: "<H"}[size], self.emu.mem_read(at, size))[0]

    def text(self, at):
        return bytes(self.emu.mem_read(at, 128)).split(b"\0")[0]

    def on_stub(self, emu, address, size, _):
        name, pops, action = self.stubs[address]
        sp = emu.reg_read(UC_X86_REG_ESP)
        if action is None:
            raise AssertionError(f"unexpected call of {name} from {self.read(sp):#x}")
        args = [self.read(sp + 4 + 4 * i) for i in range(pops // 4)]
        self.calls.append((name, *args))
        emu.reg_write(UC_X86_REG_EAX, action(args))
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def on_log(self, emu, *_):
        self.logged += 1
        sp = emu.reg_read(UC_X86_REG_ESP)
        emu.reg_write(UC_X86_REG_ESP, sp + 4)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def on_changed(self, emu, *_):
        self.changed += 1

    def malloc(self, args):
        at, self.heap = self.heap, (self.heap + args[0] + 0x1F) & ~0xF
        assert self.heap < HEAP + 0x400000, "the bump allocator's room"
        self.emu.mem_write(at, bytes(args[0]))
        self.sizes[at] = args[0]
        return at

    def realloc(self, args):
        old, size = args[0], args[1]
        at = self.malloc([size])
        if old:
            self.emu.mem_write(at, bytes(self.emu.mem_read(old, min(size, self.sizes[old]))))
        return at

    def module(self, args):
        return KERNEL32 if self.text(args[0]).lower() == b"kernel32.dll" else 0

    def proc(self, args):
        assert args[0] == KERNEL32, "GetProcAddress of kernel32"
        return self.global_size if self.text(args[1]) == b"GlobalSize" else 0

    def open_clipboard(self, args):
        assert not self.open and args[0] == 0, "opened once, for the task"
        self.open = self.openable
        return int(self.openable)

    def clipboard_data(self, args):
        assert self.open
        return HANDLE if args[0] == CF_UNICODETEXT and self.clipboard is not None else 0

    def close_clipboard(self, args):
        assert self.open and not self.locked, "closed after the memory is unlocked"
        self.open = False
        return 1

    def size(self, args):
        assert self.open and args[0] == HANDLE
        return len(self.memory())

    def lock(self, args):
        assert self.open and args[0] == HANDLE and not self.locked
        self.locked = True
        memory = self.memory()
        self.emu.mem_write(CLIP, memory + b"\xEE\xEE" * 64)   # past GlobalSize: never read
        return CLIP

    def unlock(self, args):
        assert self.locked and args[0] == HANDLE
        self.locked = False
        return 0

    def windows(self):
        """The Windows functions called since the last key, in order."""
        return [c[0] for c in self.calls if c[0] not in ("malloc", "realloc", "free", "keyboard", "key name")]

    def memory(self):
        return self.clipboard.encode("utf-16-le", "surrogatepass") + self.extra

    def call(self, function, args, ecx=0, edi=None):
        sp = STACK + 0x10000
        self.emu.mem_write(sp, struct.pack(f"<{1 + len(args)}I", RETURN, *args))
        for reg, value in KEPT.items():
            self.emu.reg_write(reg, value)
        if edi is not None:
            self.emu.reg_write(UC_X86_REG_EDI, edi)
        self.emu.reg_write(UC_X86_REG_ECX, ecx)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        try:
            self.emu.emu_start(function, RETURN, count=3_000_000)
        except UcError as error:
            raise AssertionError(f"{function:#x}: {error} at {self.emu.reg_read(UC_X86_REG_EIP):#x}")
        assert self.emu.reg_read(UC_X86_REG_EIP) == RETURN, f"{function:#x} did not return"
        assert self.emu.reg_read(UC_X86_REG_ESP) == sp + 4 + 4 * len(args), f"{function:#x}: its pops"
        for reg, value in KEPT.items():
            if reg != UC_X86_REG_EDI or edi is None:
                assert self.emu.reg_read(reg) == value, f"{function:#x} kept {reg}"
        return self.emu.reg_read(UC_X86_REG_EAX)

    def key(self, code, kind=None):
        """A key's action through the text edit (down then up unless a kind is given)."""
        for k in ((DOWN, UP) if kind is None else (kind,)):
            self.call(KEYBOARD_ACTION, [0, code, k], ecx=self.edit)

    def chord(self, *codes):
        """The modifiers held down, the last key pressed, the modifiers let go."""
        *held, last = codes
        for code in held:
            self.key(code, DOWN)
        self.key(last)
        for code in reversed(held):
            self.key(code, UP)

    def state(self):
        """(the line, the caret, the selection)."""
        begin, end = self.read(self.edit + 0x48), self.read(self.edit + 0x4C)
        line = bytes(self.emu.mem_read(begin, end - begin)).decode("ascii")
        assert self.read(end, 2) & 0xFF == 0, "its zero"
        caret, a, b = (self.read(self.edit + o, 2) for o in (0x2A6, 0x2A8, 0x2AA))
        return line, caret, (a, b)

    def paste(self, text, keys=(LCTRL, V)):
        """Ctrl+V (or other keys) with `text` on the clipboard: (the line's state, text-changed
        events, log lines); the clipboard closed again."""
        self.clipboard, self.changed, self.logged, self.calls = text, 0, 0, []
        self.chord(*keys)
        assert not self.open and not self.locked, "the clipboard closed, its memory unlocked"
        return self.state(), self.changed, self.logged


def references(data, va):
    """Every place in .text whose four bytes are `va` (an immediate or a pointer)."""
    needle, found, at = struct.pack("<I", va), [], 0
    text = bytes(data[base.TEXT_FILE:base.TEXT_FILE + base.TEXT_VSIZE])
    while (at := text.find(needle, at)) >= 0:
        found.append(base.TEXT_VA + at)
        at += 1
    return found


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    built = bytes(built)

    assert references(original, PASTE) == [0x34AA5D], "the only use: init_internals's mov ebx,3482C0h"
    old, new = Run(original), Run(built)

    def both(step, *args):
        a, b = step(old, *args), step(new, *args)
        assert a == b, (step.__name__, args, a, b)
        return a

    def type_where(r):
        for code in WHERE:
            r.key(code)
        return r.state()

    assert both(type_where) == ("where", 5, (5, 5))
    print("ok the same typing in both: W H E R E typed through the text edit's own bindings")

    assert old.paste("collision walk") == (("where", 5, (5, 5)), 0, 1)
    assert new.paste("collision walk") == (("wherecollision walk", 19, (19, 19)), 1, 0)
    assert new.windows() == ["GetModuleHandleA", "GetProcAddress", "OpenClipboard", "GetClipboardData",
                             "GlobalSize", "GlobalLock", "GlobalUnlock", "CloseClipboard"], new.calls
    assert old.windows() == []
    print("ok Ctrl+V: the original logs \"not implemented\"; the build pastes at the caret, one set_text, "
          "the caret after it; the clipboard opened, read, unlocked and closed")

    def undo(r):
        r.chord(LCTRL, Z)
        return r.state()

    # an undo step keeps the caret set_text found (a typed character's: before it)
    assert undo(new) == ("where", 4, (4, 4)), "the paste is one undo step"
    assert undo(old) == ("wher", 3, (3, 3))
    assert undo(new) == ("wher", 3, (3, 3))
    print("ok Ctrl+Z: the paste is one undo step, then the typing's own")

    def end_e(r):
        r.key(END)
        r.key(0x12)
        return r.state()

    assert both(end_e) == ("where", 5, (5, 5))

    def plain(r, *keys):
        r.calls, r.logged = [], 0
        r.chord(*keys)
        return r.state(), r.logged, r.windows()

    assert both(plain, V) == (("wherev", 6, (6, 6)), 0, [])
    assert both(plain, LSHIFT, V) == (("wherevV", 7, (7, 7)), 0, [])
    assert both(plain, LCTRL, LSHIFT, V) == (("wherevV", 7, (7, 7)), 0, [])
    assert both(plain, LCTRL, C) == (("wherevV", 7, (7, 7)), 1, [])
    print("ok V types v and Shift+V V in both; Ctrl+Shift+V does nothing in both; Ctrl+C logs its stub in both")

    assert both(plain, LCTRL, A) == (("wherevV", 7, (0, 7)), 0, [])
    assert both(plain, LCTRL, X) == (("wherevV", 7, (0, 7)), 1, []), "the cut stub: nothing cut"
    assert old.paste("where") == (("wherevV", 7, (0, 7)), 0, 1)
    assert new.paste("where") == (("where", 5, (5, 5)), 2, 0), "the selection deleted, then the paste"
    print("ok Ctrl+A and Ctrl+X the same in both; Ctrl+V over a selection replaces it in the build "
          "(deleted first, as a typed character's)")

    # from here the build alone: a fresh line
    r = Run(built)
    assert r.paste("where\r\ncollision") == (("where", 5, (5, 5)), 1, 0)
    assert r.paste(" walk\nx")[0] == ("where walk", 10, (10, 10))
    assert r.paste("\tx\ty")[0][0] == "where walk"
    assert r.paste("!\u2028y")[0][0] == "where walk!"
    assert r.paste("\r\nwhere") == (("where walk!", 11, (11, 11)), 0, 0)
    print("ok the first line alone: CR LF, LF, a tab, U+2028 end it; an empty first line pastes nothing")

    r = Run(built)
    assert r.paste("say привет ok \u00e9\U0001F600!")[0] == ("say  ok !", 9, (9, 9))
    assert r.paste("\ud800x")[0][0] == "say  ok !x"
    print("ok characters other than printable ASCII are left out (Cyrillic, é, an emoji, a lone surrogate)")

    r = Run(built)
    for code in WHERE:
        r.key(code)
    r.key(HOME)
    assert r.state() == ("where", 0, (0, 0))
    assert r.paste("x" * 300) == (("x" * 250 + "where", 250, (250, 250)), 1, 0), "cut to 255, the line kept"
    assert r.paste("yy") == (("x" * 250 + "where", 250, (250, 250)), 0, 0), "no room: nothing"
    r = Run(built)
    r.emu.mem_write(r.edit + 0x2A4, struct.pack("<H", 10))
    assert r.paste("collision walk")[0] == ("collision ", 10, (10, 10)), "a smaller max length is the line's"
    print("ok cut to the line's max length (0FFh, or 10 when set), the text already there kept; no room: nothing")

    r = Run(built)
    for code in WHERE:
        r.key(code)
    r.chord(LCTRL, A)
    assert r.paste(None) == (("where", 5, (0, 5)), 0, 0)
    r.openable = False
    assert r.paste("x") == (("where", 5, (0, 5)), 0, 0)
    assert r.windows() == ["GetModuleHandleA", "GetProcAddress", "OpenClipboard"]
    r.openable = True
    r.extra = "ab".encode("utf-16-le")                      # no zero: GlobalSize ends it
    assert r.paste("x" * 3)[0] == ("xxxab", 5, (5, 5))
    r.extra = "\0zz".encode("utf-16-le")
    assert r.paste("w")[0] == ("xxxabw", 6, (6, 6))
    print("ok no text on the clipboard, or the clipboard held elsewhere: nothing, the selection stays; "
          "the text read to its zero or to GlobalSize's end")

    r = Run(built)
    assert r.paste("where", keys=(LSHIFT, INSERT))[0] == ("where", 5, (5, 5))
    print("ok Shift+Insert, the same binding, pastes too")
    print("patch 157: all checks passed")


if __name__ == "__main__":
    main()
