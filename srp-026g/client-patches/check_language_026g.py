#!/usr/bin/env python3
"""Run patch 79 (rust/patches/src/language.rs, its lohooks of language.toml:
the game language is a setting kept in user.cfg) in the default build on
made-up game objects, each from the game's own code before its site to past
it, the original beside it.

    python check_language_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them; check_hide_ui_026g.py's harness (Scaleform's Invoke
0x511600 a stub that records each call). Stubbed besides: the CRT's
setlocale (0x426AE1, recorded), kernel32's GetUserDefaultLCID (its import
slot 0x87027C: the case's user locale, the calls counted), translate_text
(0x1FF0F0: the key's English text) and game::save_user_config (0x1E9030,
counted):

- on_configs_loaded's start (0x1EA210 to 0x1EA216): the patch registers the
  console variable `localization` once, with the game's own
  cc_string::cc_string (a saved string of 256 bytes at the head of the
  console's list); the game's own cc_string::execute sets it from a user.cfg
  line's value and its status gives it back; registers and stack as the
  original's, which registers nothing;
- before the dictionary loads (0x1EA306 to 0x1EA30C): the language string
  (the engine's buffer_string 0x21E75E0, as its initializer 0x84E3E0 leaves
  it, or as -localization= sets it) and the saved value give: a registered
  flag wins (unchanged, no locale call), else the saved choice, else the
  language of Windows' user locale by its primary language (asked only
  then), else English; a changed language gets the
  locale sequence of core::preinitialize (C, .ACP, its code page if any, C);
  the saved value is the language after; eax the original's;
- the Settings' call (game_options::scaleform_callback 0x1FF5D0 to its
  return): `srp_language` with a registered name saves it (user.cfg written
  once), with another or none saves nothing, and both answer
  root.srp_language(label, note, saved, running, then every registered
  language's name and own name) and return as the function does (ret 10h); an
  unknown method name goes on to the original's dispatch and its return, no
  Invoke, in both builds.
"""
import struct
import sys

from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_EDI, \
    UC_X86_REG_EIP

import patch_026g as base
import check_hide_ui_026g as hide
from localization_026g import LANGUAGES

START, START_END = 0x1EA210, 0x1EA216
TEXTS, TEXTS_END = 0x1EA306, 0x1EA30C
CALLBACK = 0x1FF5D0
LIST_HEAD, CC_STRING_VTABLE, EXECUTE = 0x1F1D66C, 0x9E7C2C, 0xEC8C0
LOCALIZATION, INITIALIZER = 0x21E75E0, 0x84E3E0
SETLOCALE, TRANSLATE, SAVE = 0x426AE1, 0x1FF0F0, 0x1E9030
USER_LCID_SLOT = 0x87027C   # GetUserDefaultLCID's import
DATA, OPTIONS, GAME, ARGS, TEXT = 0x30000000, 0x30001000, 0x30010000, 0x30020000, 0x30021000
MOVIE, HOLDER, VIEW = 0x30030000, 0x30031000, 0x30032000
# the callback's first argument in the game: a static object, not the menu's
# movie (the first test's crash, 2026-10-29: Invoke on it read [[+0Ch]+8])
CALLER = 0xB07018
USER_LCID = 0x3003F000      # its stub's code
ENGLISH = {"st_srp_language_option": b"Language", "st_srp_language_restart": b"Applies after restart."}


class Run(hide.Run):
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_map(DATA, 0x40000, UC_PROT_ALL)
        self.locales, self.saves = [], 0
        self.windows, self.windows_calls = None, []
        for stub in (SETLOCALE, TRANSLATE, SAVE):
            self.emu.hook_add(UC_HOOK_CODE, self.engine, begin=stub, end=stub)
        self.word(USER_LCID_SLOT, USER_LCID)
        self.emu.hook_add(UC_HOOK_CODE, self.user_lcid, begin=USER_LCID, end=USER_LCID)
        # the language string as its initializer leaves it
        self.call_at(INITIALIZER, ())

    def call_at(self, start, args, regs=None):
        sp = hide.STACK + 0x8000
        self.word(sp, hide.SENTINEL)
        for i, value in enumerate(args):
            self.word(sp + 4 + 4 * i, value)
        after, end = self.run(start, (hide.SENTINEL,), sp, {**hide.GAME, **(regs or {})})
        assert self.stopped == hide.SENTINEL, hex(start)
        return after, end - sp

    def engine(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        if address == SETLOCALE:                       # cdecl (category, locale)
            self.locales.append((self.read(sp + 4), self.text(self.read(sp + 8)).decode()))
            emu.reg_write(UC_X86_REG_EAX, self.read(sp + 8))
            pops = 0
        elif address == TRANSLATE:                     # thiscall (translator, key, out), ret 8
            assert emu.reg_read(UC_X86_REG_ECX) == GAME + 0x52A8
            key, out = self.text(self.read(sp + 4)).decode(), self.read(sp + 8)
            emu.mem_write(out, ENGLISH.get(key, key.encode()) + b"\0")
            pops = 8
        else:                                          # save_user_config, no arguments
            self.saves += 1
            pops = 0
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def user_lcid(self, emu, address, size, _):
        """GetUserDefaultLCID(), stdcall, no arguments: the case's LCID."""
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.windows_calls.append("GetUserDefaultLCID")
        emu.reg_write(UC_X86_REG_EAX, self.windows)
        emu.reg_write(UC_X86_REG_ESP, sp + 4)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    # The engine's language string: {begin, end, capacity end} over 16 bytes.
    def language(self):
        return self.text(self.read(LOCALIZATION)).decode()

    def flag(self, name):
        buffer = self.read(LOCALIZATION)
        self.emu.mem_write(buffer, name.encode() + b"\0")
        self.word(LOCALIZATION + 4, buffer + len(name))

    def start(self):
        self.locales = []
        sp = hide.STACK + 0x9000
        after, end = self.run(START, (START_END,), sp, hide.GAME)
        assert self.stopped == START_END
        return after, end, self.read(LIST_HEAD)

    def texts(self, windows=0x0409):
        self.locales, self.windows, self.windows_calls = [], windows, []
        sp = hide.STACK + 0x9000
        regs = {**hide.GAME, UC_X86_REG_EDI: GAME}
        after, end = self.run(TEXTS, (TEXTS_END,), sp, regs)
        assert self.stopped == TEXTS_END and end == sp
        assert after[UC_X86_REG_EAX] == GAME + 0x52A8 and after[UC_X86_REG_EDI] == GAME
        return self.language(), list(self.locales), list(self.windows_calls)

    def callback(self, method, argument=None, managed=False, menu=True):
        """scaleform_callback(caller, method, arguments, count) on the
        options' interface, whose menu holder's view has the movie
        ([[[+8]+108h]+4], as the options' own Invoke 0x1FF98F); the movie's
        Invokes."""
        self.emu.mem_write(OPTIONS, bytes(0x100))
        self.word(OPTIONS + 0x30, GAME)
        self.word(OPTIONS + 0x08, HOLDER if menu else 0)
        self.word(HOLDER + 0x108, VIEW)
        self.word(VIEW + 0x04, MOVIE)
        self.emu.mem_write(TEXT, method.encode() + b"\0")
        count = 0
        if argument is not None:
            text = TEXT + 0x100
            self.emu.mem_write(text, argument.encode() + b"\0")
            value = text
            if managed:
                self.word(TEXT + 0x200, text)
                value = TEXT + 0x200
            self.emu.mem_write(ARGS, struct.pack("<IIIIII", 0, 6 | (0x40 if managed else 0), value, 0, 0, 0))
            count = 1
        self.saves = 0
        after, popped = self.call_at(CALLBACK, (CALLER, TEXT, ARGS, count), {UC_X86_REG_ECX: OPTIONS})
        assert popped == 4 + 0x10, popped
        return self.invokes(), self.saves


def console_variable(r, head):
    """The registered variable: (name, buffer, size, saved byte), its vtable checked."""
    assert r.read(head) == CC_STRING_VTABLE
    name, buffer, size = r.text(r.read(head + 0x10)), r.read(head + 0x40), r.read(head + 0x44)
    return name.decode(), buffer, size, r.read(head + 0x1C, 2)[1]


def main():
    original = open(sys.argv[1], "rb").read()
    patched, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None,
                            open(base.late_join_bin(), "rb").read())
    stock, ours = Run(original), Run(patched)
    names = [name for name, *_ in LANGUAGES]
    expect = lambda ok, text: print("ok", text) if ok else sys.exit("FAIL " + text)

    # on_configs_loaded's start: the variable, once
    head_before = ours.read(LIST_HEAD)
    expect(stock.language() == "original" and ours.language() == "original",
           "the initializer leaves the language string \"original\"")
    a_regs, a_sp, a_head = stock.start()
    b_regs, b_sp, b_top = ours.start()
    # the site's chain (hooks_026g's shared lohook sites): patch 79's handler, then
    # patch 81's, which puts its own variable at the head after this one
    lobby = console_variable(ours, b_top)
    b_head = ours.read(b_top + 0x0C)
    variable = console_variable(ours, b_head)
    expect(a_regs == b_regs and a_sp == b_sp and a_head == head_before and b_head != head_before
           and variable[0] == "localization" and variable[2] == 256 and variable[3] == 1
           and ours.read(b_head + 0x0C) == head_before and lobby[0] == "srp_lobby",
           "configs.start: registers and stack as the original's; `localization`, a saved string of 256 "
           "bytes, in the console's list before the old head, patch 81's `srp_lobby` registered after "
           "it (the original registers nothing)")
    _, _, again = ours.start()
    expect(again == b_top and ours.read(b_head + 0x0C) == head_before, "configs.start again: registered once")
    buffer = variable[1]
    ours.emu.mem_write(TEXT, b"hungarian\0")
    ours.call_at(EXECUTE, (TEXT,), {UC_X86_REG_ECX: b_head})
    expect(ours.text(buffer) == b"hungarian", "user.cfg's line `localization hungarian` sets it "
           "(cc_string::execute 0xEC8C0)")

    def saved(value):
        ours.emu.mem_write(buffer, value.encode() + b"\0")

    # before the dictionary loads
    c, acp, c_numeric = (0, "C"), (1, ".ACP"), (4, "C")
    page = lambda name: [c, acp, (0, name), c_numeric]
    asked = ["GetUserDefaultLCID"]
    # (label, flag, saved choice, Windows' user LCID, the language, locale calls, kernel32 calls)
    cases = [
        ("flag czech, saved german", "czech", "german", 0x0419, "czech", [], []),
        ("flag english, saved polish", "english", "polish", 0x0419, "english", [], []),
        ("no flag, saved german", None, "german", 0x0419, "german", [c, acp, c_numeric], []),
        ("no flag, saved hungarian", None, "hungarian", 0x0419, "hungarian", [c, acp, c_numeric], []),
        ("no flag, no choice, Windows 0x0419", None, "", 0x0419, "russian", page(".1251"), asked),
        ("no flag, no choice, Windows 0x0415", None, "", 0x0415, "polish", page(".1250"), asked),
        ("no flag, no choice, Windows 0x0405", None, "", 0x0405, "czech", [c, acp, c_numeric], asked),
        ("no flag, no choice, Windows 0x0411", None, "", 0x0411, "english", page(".1252"), asked),
        ("no flag, unknown choice, Windows 0x101A (Croatian, Bosnia)", None, "klingon", 0x101A, "serbian",
         [c, acp, c_numeric], asked),
        ("no flag, no choice, Windows 0x0C07 (German, Austria)", None, "", 0x0C07, "german",
         [c, acp, c_numeric], asked),
        ("no flag, no choice, Windows 0x10407 (German, phone book sort)", None, "", 0x10407, "german",
         [c, acp, c_numeric], asked),
        ("no flag, no choice, Windows 0x0C00 (a custom locale)", None, "", 0x0C00, "english", page(".1252"), asked),
    ]
    for label, flag, choice, windows, language, locales, kernel_calls in cases:
        for r in (stock, ours):
            r.call_at(INITIALIZER, ())
            if flag:
                r.flag(flag)
        saved(choice)
        before = stock.language()
        got = ours.texts(windows)
        stock_got = stock.texts(windows)
        expect(got == (language, locales, kernel_calls) and ours.text(buffer).decode() == language
               and stock_got == (before, [], []),
               f"{label}: the language {language}, locale calls {locales}, "
               f"Windows {'asked' if kernel_calls else 'not asked'}; saved {language}; the original keeps {before}, "
               f"no calls")

    # the Settings' call
    ours.flag("english")
    saved("english")
    answer = ["Language", "Applies after restart."]
    listing = [text for name, native, *_ in LANGUAGES for text in (name, native)]
    for label, argument, managed, saved_after, saves in [
        ("a registered name", "czech", False, "czech", 1),
        ("a managed string", "finnish", True, "finnish", 1),
        ("an unregistered name", "klingon", False, "finnish", 0),
        ("no argument", None, False, "finnish", 0),
    ]:
        invokes, count = ours.callback("srp_language", argument, managed)
        expect(invokes == [(MOVIE, "root.srp_language", tuple(answer + [saved_after, "english"] + listing))]
               and count == saves and ours.text(buffer).decode() == saved_after,
               f"srp_language with {label}: saved {saved_after}, user.cfg written {saves} time(s), the movie "
               f"answered with the texts, {saved_after}, english and the {len(names)} languages")
        stock_invokes, stock_saves = stock.callback("srp_language", argument, managed)
        expect(stock_invokes == [] and stock_saves == 0, f"the original ignores srp_language ({label})")
    invokes, count = ours.callback("srp_language", "german", menu=False)
    expect(invokes == [] and count == 1 and ours.text(buffer).decode() == "german",
           "srp_language without the menu's movie: german saved, nothing invoked")
    for r, which in ((stock, "the original"), (ours, "the build")):
        invokes, count = r.callback("no_such_method")
        expect(invokes == [] and count == 0, f"{which}: an unknown method returns, nothing invoked or saved")
    print("all cases passed")


if __name__ == "__main__":
    main()
