#!/usr/bin/env python3
"""Patch 195 (use_prompt): a use's progress bar ends with the use
(docs/notes/client_patch.md "The use prompt after a use"), the default build
beside the original.

    python check_use_prompt_026g.py <original survarium.exe>

Requires Python `unicorn`. The game's own game_world_ui::set_using_info_message
(0x23A560: EAX the prompt's text key, the ui on the stack) and
set_using_progress_message (0x23A430: the ui and the progress) run as
player::st_draw calls them each frame, on a made-up hud. Stand-ins:
Scaleform's invoke (0x511600) keeps the call's name and its first two values;
the translator (0x1FF0F0) gives the key itself; the Value array's
constructor, assignment and destructor (0x11100, 0x2AF7B0, 0x2A890) keep the
text and nothing else.

What a game shows:
- the prompt's text, then a use's progress, then the same text again (a death
  or a finished use with nothing usable in view, when the use began with that
  text shown: "" for nothing usable): the original gives the prompt nothing
  and keeps the bar; the build gives the text again, which replaces the bar;
- the take prompt, a use, nothing usable: both give "" (the text changed);
- a use's progress of 0 (its first frame) gives nothing in both; the build
  still forgets the text;
- the prompt's text unchanged and no use between: nothing given, in both.
The function's bytes but the hook's six are the original's.
"""
import os
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDI, UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_EIP

if len(sys.argv) > 1:
    os.environ.setdefault("SRP_026G_EXE", sys.argv[1])    # a span hook's bytes (patch_026g.RUST_HOOKS)
import patch_026g as base

INFO, PROGRESS = 0x23A560, 0x23A430
SITE, PROGRESS_END = 0x23A430, 0x23A559
INVOKE, TRANSLATE, VALUES_NEW, VALUE_TEXT, VALUES_FREE = 0x511600, 0x1FF0F0, 0x11100, 0x2AF7B0, 0x2A890
NOTHING, TAKE = 0x9DB708, 0xA16BCC      # "" and st_use_artefact_container, as use_info gives them
PROMPT_TEXT = 0x334
THREAD_SLOT = 0x870118
HEAP, STUBS, STACK = 0x30000000, 0x38000000, 0x39000000
UI, HOLDER, MOVIE_HOLDER, WORLD = HEAP, HEAP + 0x1000, HEAP + 0x2000, HEAP + 0x3000
RETURN = STUBS + 0xF00

failed = 0


def check(ok, what):
    global failed
    failed += not ok
    print(f"  {'ok' if ok else 'FAIL'}  {what}")


class Hud:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(HEAP, 0x10000, UC_PROT_ALL)
        for at in (STUBS, STACK):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        self.u32(THREAD_SLOT, STUBS + 0x80)
        emu.mem_write(STUBS + 0x80, b"\xB8\x01\x00\x00\x00\xC3")    # GetCurrentThreadId: mov eax,1 / ret
        self.u32(UI + 8, HOLDER)
        self.u32(HOLDER + 0x108, MOVIE_HOLDER)
        self.u32(MOVIE_HOLDER + 4, STUBS + 0x400)
        self.u32(UI + 0x18, WORLD)
        self.u32(WORLD + 0x9C, WORLD + 0x100)
        self.given = []
        for at, stub in ((INVOKE, self.invoke), (TRANSLATE, self.translate), (VALUES_NEW, self.values),
                         (VALUE_TEXT, self.text), (VALUES_FREE, self.values)):
            emu.hook_add(UC_HOOK_CODE, stub, begin=at, end=at)

    def u32(self, at, value=None):
        if value is None:
            return struct.unpack("<I", self.emu.mem_read(at, 4))[0]
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def ret(self, pops):
        esp = self.emu.reg_read(UC_X86_REG_ESP)
        self.emu.reg_write(UC_X86_REG_ESP, esp + 4 + pops)
        self.emu.reg_write(UC_X86_REG_EIP, self.u32(esp))

    def string(self, at):
        return bytes(self.emu.mem_read(at, 64)).split(b"\0")[0].decode()

    def invoke(self, *_):
        """GFx movie invoke (thiscall, `ret 10h`): the name and its values'
        text and number (each value 18h bytes, its data at +8)."""
        esp = self.emu.reg_read(UC_X86_REG_ESP)
        name, values = self.string(self.u32(esp + 4)), self.u32(esp + 0xC)
        self.given.append((name, self.string(self.u32(values + 8)), self.u32(values + 0x18 + 8)))
        self.ret(0x10)

    def translate(self, *_):
        """text_translator::translate_text (thiscall, `ret 8`): the key, into the buffer."""
        esp = self.emu.reg_read(UC_X86_REG_ESP)
        key, out = self.u32(esp + 4), self.u32(esp + 8)
        self.emu.mem_write(out, self.string(key).encode() + b"\0")
        self.ret(8)

    def values(self, emu, address, *_):
        """The Value array's constructor (the values empty) or destructor
        iterator (`ret 10h`: the array, the size, the count, the function)."""
        esp = emu.reg_read(UC_X86_REG_ESP)
        if address == VALUES_NEW:
            emu.mem_write(self.u32(esp + 4), bytes(self.u32(esp + 8) * self.u32(esp + 0xC)))
        self.ret(0x10)

    def text(self, *_):
        """A Value given a string (EDI the text, ESI the value): its data its text."""
        self.u32(self.emu.reg_read(UC_X86_REG_ESI) + 8, self.emu.reg_read(UC_X86_REG_EDI))
        self.ret(0)

    def call(self, va, args, eax=0):
        sp = STACK + 0x8000 - 4 * (len(args) + 1)
        self.u32(sp, RETURN)
        for n, value in enumerate(args):
            self.u32(sp + 4 + 4 * n, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.reg_write(UC_X86_REG_EAX, eax)
        self.emu.reg_write(UC_X86_REG_ECX, 0)
        self.emu.emu_start(va, RETURN, count=200_000)
        assert self.emu.reg_read(UC_X86_REG_ESP) == STACK + 0x8000, "the call's own pops"

    def info(self, text):
        """st_draw's prompt with nothing in use (0x246FFB): what was given."""
        self.given = []
        self.call(INFO, (UI,), eax=text)
        return [(name, shown) for name, shown, _ in self.given]

    def progress(self, value):
        """st_draw's progress of a use (0x246FC4): what was given."""
        self.given = []
        self.call(PROGRESS, (UI, struct.unpack("<I", struct.pack("<f", value))[0]))
        return [(name, shown) for name, shown, _ in self.given]


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    built = bytes(built)
    installs = {h[0]: h for h in base.rust_hooks(original)}
    at = base.file_offset(SITE)
    check(SITE in installs and built[at] == 0xE9 and original[at] != 0xE9,
          f"use_prompt.progress ({SITE:#x}): a jump to the build's hook")
    a, b = base.file_offset(SITE + 6), base.file_offset(PROGRESS_END)
    check(built[a:b] == original[a:b], f"{SITE + 6:#x}..{PROGRESS_END:#x}: the original's bytes")
    a, b = base.file_offset(INFO), base.file_offset(INFO + 0xFD)
    check(built[a:b] == original[a:b], f"{INFO:#x}..{INFO + 0xFD:#x}: the original's bytes")

    stock, ours = Hud(original), Hud(built)
    progress = ("root.set_context", "st_using_progress_message")
    nothing = [("root.set_context", "")]

    for hud in (stock, ours):
        hud.info(NOTHING)
    shown = [hud.progress(0.3) for hud in (stock, ours)]
    check(shown == [[progress], [progress]], "a use's progress: the bar, in both")
    check(ours.u32(UI + PROMPT_TEXT) == 0 and stock.u32(UI + PROMPT_TEXT) == NOTHING,
          "the build forgets the prompt's text at the progress; the original keeps it")
    after = [hud.info(NOTHING) for hud in (stock, ours)]
    check(after == [[], nothing], "the use ended with the same text: the original gives nothing (the bar stays), "
                                  "the build gives \"\" (the bar goes)")
    check([hud.info(NOTHING) for hud in (stock, ours)] == [[], []], "the text unchanged after that: nothing in both")

    for hud in (stock, ours):
        hud.info(TAKE)
        hud.progress(0.5)
    check([hud.info(NOTHING) for hud in (stock, ours)] == [nothing, nothing],
          "the take prompt, a use, then nothing usable: \"\" in both")

    for hud in (stock, ours):
        hud.info(NOTHING)
    check([hud.progress(0.0) for hud in (stock, ours)] == [[], []], "a progress of 0: nothing given in both")
    check([hud.info(NOTHING) for hud in (stock, ours)] == [[], nothing],
          "after it, the same text: the build gives it again")

    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
