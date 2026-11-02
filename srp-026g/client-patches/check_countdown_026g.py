#!/usr/bin/env python3
"""Run patch 95 (rust/patches/src/countdown.rs, its lohooks of countdown.toml:
a match's countdown from its static info) in the default build on made-up
game objects, each from the game's own code before its site to past it.

    python check_countdown_026g.py <original survarium.exe>

Requires Python `unicorn`. check_blackout_026g.py's harness (itself
check_hide_ui_026g.py's). The original runs the same way and must do what it
did:

- the options read: process_static_match_info from 0x27B917 to 0x27B932, and
  a replay's chunk 1 from 0x1F094C to its `ret 8` (patch 65's runs): the
  options' +57874h holds "SRCD" and +57878h the milliseconds for srp's block
  with tag 3 and one byte (alone, after Blackout's entry, after an unknown
  tag; Blackout's mark kept beside it), zeros for none, another length, a
  short block or another magic; the original leaves both words; the game's
  registers and stack as its code leaves them;
- the rule made: timelimit_rule_core's constructor from 0x317EA7 (the mode's
  times stored, ESI the rule, EBP its frame, the cook's data at [ebp+8] and
  the options at its +0) to its `mov eax,esi` on either path of
  wait_player_percent's type (0x317EBA for a float, 0x317ED1 for an int):
  the countdown at +140h is the options' when they keep one, else the mode's
  as stored; the fraction at +144h as the original stores it (the replayed
  compare's flags pick the path); every register as the original leaves it;
- both together: the options read for a match, then its rule made from them.
"""
import struct
import sys

from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EDI, UC_X86_REG_ESI

import patch_026g as base
import check_hide_ui_026g as hide
import check_blackout_026g as blackout

OPTIONS, WORK = blackout.OPTIONS, blackout.WORK
OPTIONS_BLACKOUT, OPTIONS_MARK, OPTIONS_MS = 0x57870, 0x57874, 0x57878
STALE = b"\x77" * 4
SITE, FLOAT_PAST, INT_PAST = 0x317EA7, 0x317EBA, 0x317ED1
RULE, CONFIG, COOK = WORK + 0x4000, WORK + 0x5000, WORK + 0x6000
COUNTDOWN, FRACTION = 0x140, 0x144
MODE_MS = 20_000                       # the mode's countdown_time x 1000, as the constructor stored it


def mark(ms):
    return b"SRCD" + struct.pack("<I", ms)


class Run(blackout.Run):
    def options_payload(self, payload, stale):
        super().options_payload(payload, stale)
        self.emu.mem_write(OPTIONS + OPTIONS_MARK, stale * 2)

    def options_words(self):
        return bytes(self.emu.mem_read(OPTIONS + OPTIONS_BLACKOUT, 12))

    # The constructor from the site to its `mov eax,esi`, the options as they are: the countdown and
    # the fraction stored, and whether every register is as the original's code leaves it.
    def made(self, float_fraction=True):
        self.emu.mem_write(RULE, bytes(0x200))
        self.word(RULE + COUNTDOWN, MODE_MS)
        self.emu.mem_write(CONFIG, bytes(0x40))
        if float_fraction:
            self.emu.mem_write(CONFIG, struct.pack("<f", 0.8))
            self.emu.mem_write(CONFIG + 0x14, struct.pack("<H", 2))
        else:
            self.word(CONFIG, 1)
            self.emu.mem_write(CONFIG + 0x14, struct.pack("<H", 1))
        self.word(COOK, OPTIONS)
        ebp = hide.STACK + 0x9000
        self.word(ebp + 8, COOK)
        sp = ebp - 0x10
        self.word(sp, hide.GAME[UC_X86_REG_EDI])     # the whole path pops edi first (0x317EC8): the caller's
        given = {**hide.GAME, UC_X86_REG_EAX: CONFIG, UC_X86_REG_ESI: RULE, UC_X86_REG_EBP: ebp}
        regs, esp = self.run(SITE, (FLOAT_PAST, INT_PAST), sp, given)
        past = FLOAT_PAST if float_fraction else INT_PAST
        kept = self.stopped == past and esp == sp + (0 if float_fraction else 4) \
            and all(regs[r] == v for r, v in given.items())
        fraction = struct.unpack("<f", bytes(self.emu.mem_read(RULE + FRACTION, 4)))[0]
        return kept, self.read(RULE + COUNTDOWN), round(fraction, 3)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    stock, patched = Run(original), Run(bytes(built))
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    srpx = blackout.srpx
    five = srpx((3, b"\x05"))
    print("the options read (process_static_match_info 0x27B92D, a replay's chunk 1 0x1F0971)")
    cases = [
        ("no block", b"", bytes(4) + bytes(8)),
        ("a countdown of 5 s", five, bytes(4) + mark(5_000)),
        ("Blackout, then a countdown", srpx((2, b"\x01"), (3, b"\x05")), b"SRBO" + mark(5_000)),
        ("an unknown tag, then a countdown of 0 s", srpx((9, b"xyz"), (3, b"\x00")), bytes(4) + mark(0)),
        ("Blackout alone", srpx((2, b"\x01")), b"SRBO" + bytes(8)),
        ("a countdown of two bytes", srpx((3, b"\x05\x00")), bytes(4) + bytes(8)),
        ("a short block", five[:-1], bytes(4) + bytes(8)),
        ("another magic", b"SRPY" + bytes([3, 1, 5]), bytes(4) + bytes(8)),
    ]
    for name, block, want in cases:
        payload = blackout.empty_options(block)
        for where, run in (("0xE4", lambda r: r.static_info(payload)), ("a replay", lambda r: r.replay(payload))):
            a, b = run(stock), run(patched)
            words = (stock.options_words(), patched.options_words())
            ok = a[0] and b[0] and words == (STALE * 3, want)
            report(ok, f"{name} ({where}): +57870h..+5787Bh {words[1].hex()}; the original's left; the game's "
                       "registers and stack as its code leaves them")

    print("the rule made (timelimit_rule_core's constructor 0x317EA7)")
    for name, block, want in [("no countdown in the options", b"", MODE_MS),
                              ("the options' 5 s", five, 5_000),
                              ("the options' 0 s", srpx((3, b"\x00")), 0),
                              ("Blackout alone", srpx((2, b"\x01")), MODE_MS)]:
        for fraction in (True, False):
            patched.static_info(blackout.empty_options(block))
            stock.static_info(blackout.empty_options(block))
            a, b = stock.made(fraction), patched.made(fraction)
            ok = a == (True, MODE_MS, 0.8 if fraction else 1.0) and b == (True, want, a[2])
            report(ok, f"{name}, a {'float' if fraction else 'whole'} fraction: the countdown {b[1]} ms "
                       f"(the original's {a[1]}), the fraction {b[2]}; the registers as the original's")

    print("a stale mark: the options not read through either site keep what they hold")
    patched.static_info(blackout.empty_options(five))
    patched.emu.mem_write(OPTIONS + OPTIONS_MARK, b"SRCX" + struct.pack("<I", 1_000))
    report(patched.made() == (True, MODE_MS, 0.8), "another mark: the mode's countdown")

    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
