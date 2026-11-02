#!/usr/bin/env python3
"""Run patch 94 (rust/patches/src/scarcity.rs, its lohooks of scarcity.toml:
Scarcity, a battery match whose ammo crates refill no one) in the default
build on made-up game objects, each from the game's own code before its site
to past it.

    python check_scarcity_026g.py <original survarium.exe>

Requires Python `unicorn`. check_blackout_026g.py's harness reads the match's
options (process_static_match_info from 0x27B917, a replay's chunk 1 from
0x1F094C) with srp's block; then resupply_zone_core::can_use (0x2F8080) and
use_hint (0x2F80C0) run from their starts to their `ret 4` on a made-up zone
(its team at +54h), interaction record and player (its profile at +6B8h, the
profile's team at +45ECh), inventory::check_resupply (0x2FDF50) a stub that
answers 0 (something to refill). The original runs the same way and must do
what it did:

- a match without srp's block, with Blackout's entry (tag 2, one byte 1) or
  with another value: the native answers, the zone of the player's team or of
  both (2) usable (eax 1, the hint st_use_resupply_zone), another team's not
  (eax 0, the empty key 0x9DB708), check_resupply asked only for a usable one;
- a Scarcity match (tag 2, one byte 2), read from the 0xE4 or a replay: every
  zone as another team's (eax 0, the empty key), check_resupply never asked;
- the next match without it: the native answers again;
- the callee-saved registers and the stack as the game's code leaves them.
"""
import struct
import sys

from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_ESP, \
    UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import patch_026g as base
import check_hide_ui_026g as hide
import check_blackout_026g as blackout

CAN_USE, USE_HINT, CHECK_RESUPPLY = 0x2F8080, 0x2F80C0, 0x2FDF50
EMPTY_KEY, USE_KEY = 0x9DB708, 0xA13210        # "" and "st_use_resupply_zone"
ZONE, RECORD, PLAYER, PROFILE = (blackout.WORK + 0x8000 + 0x800 * i for i in range(4))
KEPT = (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)


class Run(blackout.Run):
    def __init__(self, data):
        super().__init__(data)
        self.asked = 0
        self.emu.hook_add(hide.UC_HOOK_CODE, self.check_resupply, begin=CHECK_RESUPPLY, end=CHECK_RESUPPLY)

    def check_resupply(self, emu, address, *_):
        """inventory::check_resupply: 0, something to refill; `ret 8`."""
        self.asked += 1
        sp = emu.reg_read(UC_X86_REG_ESP)
        emu.reg_write(UC_X86_REG_EAX, 0)
        emu.reg_write(UC_X86_REG_ESP, sp + 12)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def zone(self, start, zone_team, player_team):
        """One call of `start` (thiscall, the record): eax, check_resupply asked,
        and whether it returned as the function does with the registers kept."""
        for at in (ZONE, RECORD, PLAYER, PROFILE):
            self.emu.mem_write(at, bytes(0x800))
        self.word(ZONE + 0x54, zone_team)
        self.word(RECORD, PLAYER)
        self.word(PLAYER + 0x6B8, PROFILE)
        self.word(PROFILE + 0x45EC, player_team)
        sp = hide.STACK + 0x8000
        self.word(sp, hide.SENTINEL)
        self.word(sp + 4, RECORD)
        self.asked = 0
        regs, esp = self.run(start, (hide.SENTINEL,), sp, {**hide.GAME, UC_X86_REG_ECX: ZONE})
        back = self.stopped == hide.SENTINEL and esp == sp + 8 and all(regs[r] == hide.GAME[r] for r in KEPT)
        return regs[UC_X86_REG_EAX], self.asked, back


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

    # (zone team, player team): the player's team's zone, both teams' (2), the other team's.
    zones = ((0, 0), (1, 1), (2, 0), (2, 1), (1, 0), (0, 1))

    def native(zone_team, player_team):
        usable = zone_team in (player_team, 2)
        return (1, 1, True) if usable else (0, 0, True), (USE_KEY, 1, True) if usable else (EMPTY_KEY, 0, True)

    def answers(run):
        return [(run.zone(CAN_USE, z, p), run.zone(USE_HINT, z, p)) for z, p in zones]

    print("the original: the game's own rule")
    report(answers(stock) == [native(z, p) for z, p in zones],
           "own team's and both teams' zones usable (eax 1, st_use_resupply_zone), another team's not (eax 0, the "
           "empty key), check_resupply asked only for a usable one; returned as the function, registers kept")

    refused = [((0, 0, True), (EMPTY_KEY, 0, True)) for _ in zones]
    cases = [
        ("a match without srp's block", "static", b"", False),
        ("a Blackout match (tag 2, one byte 1)", "static", blackout.srpx((2, b"\x01")), False),
        ("a Scarcity match (tag 2, one byte 2)", "static", blackout.srpx((2, b"\x02")), True),
        ("the next match without it", "static", b"", False),
        ("a Scarcity replay", "replay", blackout.srpx((1, bytes(12)), (2, b"\x02")), True),
        ("tag 2 of another value (3)", "static", blackout.srpx((2, b"\x03")), False),
        ("an unknown tag, then Scarcity", "replay", blackout.srpx((9, b"xyz"), (2, b"\x02")), True),
        ("Scarcity's entry of two bytes", "static", blackout.srpx((2, b"\x02\x02")), False),
        ("a short block", "static", blackout.srpx((2, b"\x02"))[:-1], False),
    ]
    print("the crates after the options read (process_static_match_info 0x27B92D, a replay's chunk 1 0x1F0971)")
    for name, where, block, scarce in cases:
        payload = blackout.empty_options(block)
        read = patched.static_info(payload) if where == "static" else patched.replay(payload)
        ok = read[0] and answers(patched) == (refused if scarce else [native(z, p) for z, p in zones])
        report(ok, f"{name} ({'a replay' if where == 'replay' else 'the 0xE4'}): "
                   + ("every zone refused (eax 0, the empty key), check_resupply never asked"
                      if scarce else "the game's own answers") + "; returned as the function, registers kept")

    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
