#!/usr/bin/env python3
"""Patch 197's kick (camera_trail.rs, "The weapon's kick is the gun's"): the
drawn view takes 40 % of the lift the weapon's kick gives the eye.

    python check_kick_view_026g.py <original survarium.exe>

update_camera (0x1F3480) is run in the original and in the built exe on
check_head_clearance_026g's rig (its player, handler and stand-ins), to the
handler's view (0x1F3650), with a client weapon in the player's hand:
- no kick: the original's view, bit for bit;
- a kick of 0.05 over a look pitch of 0.1 (3 degrees of the eye): the view
  1.8 degrees under the original's, turned about its own right, from the same
  place; across the level (67.5 degrees a unit below it, 60 above) the same
  share of the whole lift;
- the scope's own view on, and a held item that is no client weapon: the
  original's view;
- the registers the original's in every case.
"""
import math
import struct
import sys

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EIP

import check_head_clearance_026g as rig

WEAPON = rig.HEAP + 0x2C000
WEAPON_VTABLE, WEAPON_CORE_VTABLE = 0xA04174, 0xA11094
KICK_SHOWN = 0.4
failed = 0


def check(ok, what):
    global failed
    failed += not ok
    print(("  ok  " if ok else "  FAIL  ") + what)


def eye_pitch(look):
    look = max(-1.0, min(1.0, look))
    return math.radians(look * (67.5 if look < 0 else 60.0))


class Held:
    """A game with something in the player's hand, put there as update_camera starts."""

    def __init__(self, data):
        self.game = rig.Game(data)
        self.hand = None
        self.ms = 0
        self.game.emu.hook_add(UC_HOOK_CODE, self.hold, begin=rig.UPDATE_CAMERA, end=rig.UPDATE_CAMERA)

    def hold(self, emu, *_):
        if self.hand is None:
            return
        vtable, look, kick, scope = self.hand
        emu.mem_write(rig.PLAYER + 0x128, struct.pack("<I", WEAPON))
        emu.mem_write(rig.PLAYER + 0x48C, struct.pack("<f", look))
        emu.mem_write(WEAPON, struct.pack("<I", vtable))
        emu.mem_write(WEAPON + 0x478, struct.pack("<f", kick))
        emu.mem_write(WEAPON + 0x181E, bytes([scope]))

    def view(self, vtable=WEAPON_VTABLE, look=0.0, kick=0.0, scope=0):
        self.hand = (vtable, look, kick, scope)
        # a second after the frame before: the view's follow of the gun's turn (the same patch) starts anew, at rest
        self.ms += 1000
        view, regs, _ = self.game.update_camera(0, ms=self.ms)
        return view, regs


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(rig.base.default_bin("section"), "rb").read()
    built, _ = rig.base.build(original, "section", code, None, open(rig.base.late_join_bin(), "rb").read())
    stock, ours = Held(original), Held(bytes(built))

    for what, hand in (("no kick", dict(look=0.1)),
                       ("the scope's own view on", dict(look=0.1, kick=0.05, scope=1)),
                       ("a held item that is no client weapon", dict(vtable=WEAPON_CORE_VTABLE, look=0.1, kick=0.05))):
        s_view, s_regs = stock.view(**hand)
        o_view, o_regs = ours.view(**hand)
        check(s_regs[UC_X86_REG_EIP] == rig.VIEWED and o_view == s_view and o_regs == s_regs,
              f"{what}: the original's view and registers")

    for what, look, kick in (("a kick of 0.05 over a look pitch of 0.1", 0.1, 0.05),
                             ("a kick of 0.2 across the level from -0.1", -0.1, 0.2),
                             ("a kick past the look's end (0.95 + 0.2)", 0.95, 0.2)):
        lift = eye_pitch(look + kick) - eye_pitch(look)
        under = (1 - KICK_SHOWN) * lift
        s_view, s_regs = stock.view(look=look, kick=kick)
        o_view, o_regs = ours.view(look=look, kick=kick)
        dot = lambda a, b: sum(x * y for x, y in zip(a, b))
        turned = math.acos(max(-1.0, min(1.0, dot(o_view[8:11], s_view[8:11]))))
        check(o_regs == s_regs and o_view[12:15] == s_view[12:15], f"{what}: the registers and the eye's place the original's")
        check(abs(turned - under) < 2e-5 and o_view[9] < s_view[9],
              f"{what}: the view {math.degrees(turned):.3f} degrees under the original's "
              f"({100 * (1 - KICK_SHOWN):.0f} % of the kick's {math.degrees(lift):.2f})")
        check(math.dist(o_view[0:3], s_view[0:3]) < 2e-5, f"{what}: turned about the view's own right")
    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
