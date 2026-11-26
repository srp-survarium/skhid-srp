#!/usr/bin/env python3
"""Patch 197's gun behind a turn (camera_trail.rs, "The gun has weight on a turn").

    python check_gun_sway_026g.py <original survarium.exe>

On check_head_clearance_026g's rig, in the original and in the built exe:
- process_first_person_mode's end (0x2421B0, `input_builder.end`: its three
  pops and mov esp,ebp) with a living local player holding a client weapon
  from the hip: the first frame's turn is the game's; a frame 16 ms on, of a
  turn of 2 degrees to the right, the input's turn holds the gun's share by
  this file's own arithmetic, the registers the original's;
- update_camera after it: the view is the eyes turned to the right by what is
  pending (the view is the mouse's), its place the eyes';
- a still mouse a frame later: the gun takes a share of what is pending; fully
  aimed a larger one;
- a look pitch at its upper end leaves no pitch pending;
- a dead player, no local player and the scope's own view: the frame's turn
  and all that was pending, at once, and the view the original's;
- a turn no frame took for 300 ms: the view the original's.
"""
import math
import struct
import sys

from unicorn import UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ESP, UC_X86_REG_EIP

import check_head_clearance_026g as rig

SITE, NEXT = 0x2421B0, 0x2421B5
SPAN = bytes.fromhex("5f5e5b8be5")                        # pop edi / pop esi / pop ebx / mov esp,ebp
WEAPON_VTABLE = 0xA04174
CHAIN = 0x33000000
WORLD, GAME, CLIENT, WEAPON = CHAIN, CHAIN + 0x1000, CHAIN + 0x7000, CHAIN + 0x8000
RATE, EASY, AIMED = 7.0, math.radians(12.0), 5.0
UNIT_UP = math.radians(60.0)
failed = 0


def check(ok, what):
    global failed
    failed += not ok
    print(("  ok  " if ok else "  FAIL  ") + what)


def half(value):
    return struct.unpack("<H", struct.pack("<e", value))[0]


def of_half(bits):
    return struct.unpack("<e", struct.pack("<H", bits))[0]


def taken(pending, dt, raised):
    """The gun's share of a pending (yaw, pitch), as halves' values."""
    angle = math.hypot(pending[0], pending[1] * UNIT_UP)
    share = min(RATE * (1 + (angle / EASY) ** 2) * (1 + AIMED * raised) * dt, 1.0) if angle else 1.0
    return tuple(of_half(half(p * share)) for p in pending)


class Input:
    def __init__(self, data):
        self.game = rig.Game(data)
        self.game.emu.mem_map(CHAIN, 0x10000, UC_PROT_ALL)

    def take(self, turn, ms, alive=1, local=True, raised=0.0, scope=0, look_pitch=0.0):
        """The frame's turn (yaw rad, pitch units) at `ms`: (the input's turn after the builder's end, the registers)."""
        game = self.game
        emu, u32 = game.emu, game.u32
        game.ms = ms
        emu.mem_write(CHAIN, bytes(0x10000))
        u32(rig.HANDLER + 0x334, WORLD)
        u32(WORLD + 0x9C, GAME)
        u32(GAME + 0x53AC, CLIENT)
        u32(CLIENT + 4, rig.PLAYER if local else 0)
        emu.mem_write(rig.PLAYER + 0x33C, bytes([alive]))
        emu.mem_write(rig.PLAYER + 0x48C, struct.pack("<f", look_pitch))
        u32(rig.PLAYER + 0x128, WEAPON)
        u32(WEAPON, WEAPON_VTABLE)
        emu.mem_write(WEAPON + 0x5FC, struct.pack("<f", raised))
        emu.mem_write(WEAPON + 0x181E, bytes([scope]))
        emu.mem_write(rig.HANDLER + 0x338, struct.pack("<2H", half(turn[0]), half(turn[1])))
        for reg, value in rig.STEADY.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_EBX, rig.HANDLER)
        emu.reg_write(UC_X86_REG_ESP, rig.STACK + 0x6000)
        emu.reg_write(UC_X86_REG_EBP, rig.STACK + 0x6100)
        emu.mem_write(rig.STACK + 0x6000, struct.pack("<3I", 0x0E0E0E01, 0x05050501, 0x0B0B0B01))
        emu.emu_start(SITE, NEXT, count=200000)
        regs = {r: emu.reg_read(r) for r in rig.REGS + (UC_X86_REG_EIP,)}
        return struct.unpack("<2H", emu.mem_read(rig.HANDLER + 0x338, 4)), regs

    def view(self, ms):
        view, regs, _ = self.game.update_camera(0, ms=ms)
        return view, regs


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(rig.base.default_bin("section"), "rb").read()
    built, _ = rig.base.build(original, "section", code, None, open(rig.base.late_join_bin(), "rb").read())
    built = bytes(built)
    linked = rig.base.arena()
    va, old, symbol, opcode = {h[0]: h for h in rig.base.rust_hooks(original)}[SITE]
    at = rig.base.file_offset(va)
    new = rig.base.hook_bytes(linked.symbols[symbol], va, old, 0, opcode)
    check(opcode == 0xE9 and old == SPAN and original[at:at + len(SPAN)] == SPAN and built[at:at + len(new)] == new,
          f"{va:#x}: {old.hex()} in the original, `jmp {symbol}` here")

    stock, ours = Input(original), Input(built)
    close = lambda got, want: all(abs(g - half(w)) <= 1 for g, w in zip(got, want))

    def both(turn, ms, **player):
        (s_turn, s_regs), (o_turn, o_regs) = stock.take(turn, ms, **player), ours.take(turn, ms, **player)
        check(s_turn == (half(turn[0]), half(turn[1])) and s_regs[UC_X86_REG_EIP] == NEXT and s_regs[UC_X86_REG_ESP] == rig.STACK + 0x6100,
              "  the original leaves the turn")
        check(o_regs == s_regs, "  the registers the original's")
        return o_turn, (of_half(o_turn[0]), of_half(o_turn[1]))

    first = (of_half(half(-0.02)), of_half(half(0.01)))
    print("the first frame")
    o_turn, _ = both(first, 1000)
    check(o_turn == (half(first[0]), half(first[1])), "the turn is the game's")
    s_view, s_regs = stock.view(1004)
    o_view, o_regs = ours.view(1004)
    check(o_view == s_view and o_regs == s_regs, "nothing pending: the view and the registers the original's")

    print("16 ms on, 2 degrees to the right")
    frame = (of_half(half(-math.radians(2.0))), 0.0)
    want = taken(frame, 0.016, 0.0)
    o_turn, took = both(frame, 1016)
    check(close(o_turn, want) and abs(took[0]) < 0.2 * abs(frame[0]), f"the gun takes {math.degrees(-took[0]):.3f} of the 2 degrees")
    pending = (frame[0] - took[0], frame[1] - took[1])
    s_view, s_regs = stock.view(1020)
    o_view, o_regs = ours.view(1020)
    yaw = math.atan2(s_view[8], s_view[10]) - pending[0]
    check(max(abs(a - b) for a, b in zip(o_view[8:11], (math.sin(yaw), 0.0, math.cos(yaw)))) < 2e-6 and o_view[12:] == s_view[12:]
          and abs(o_view[1]) < 1e-6 and o_regs == s_regs,
          f"the view is the eyes turned {math.degrees(-pending[0]):.3f} degrees on to the right, level, at the eyes' place")

    print("a still mouse, 16 ms on")
    want = taken(pending, 0.016, 0.0)
    o_turn, took = both((0.0, 0.0), 1032)
    check(close(o_turn, want) and took[0] < 0.0 and o_turn[1] == 0, f"the gun takes {math.degrees(-took[0]):.3f} degrees of what is pending")
    pending = (pending[0] - took[0], 0.0)
    want = taken(pending, 0.016, 1.0)
    o_turn, aimed = both((0.0, 0.0), 1048, raised=1.0)
    check(close(o_turn, want) and aimed[0] < 3.0 * took[0], f"fully aimed it takes {math.degrees(-aimed[0]):.3f}")
    pending = (pending[0] - aimed[0], 0.0)

    print("a look pitch at its upper end")
    up = (0.0, of_half(half(0.05)))
    o_turn, took = both(up, 1064, look_pitch=1.0)
    check(o_turn[1] == 0 and close(o_turn[:1], taken(pending, 0.016, 0.0)[:1]), "no pitch is taken and none stays pending")
    pending = (pending[0] - took[0], 0.0)
    o_turn, took = both((0.0, 0.0), 1080)
    check(o_turn[1] == 0, "the next frame has no pitch to take")
    pending = (pending[0] - took[0], 0.0)

    for what, player in (("a dead player", dict(alive=0)), ("no local player", dict(local=False)), ("the scope's own view", dict(scope=1, raised=1.0))):
        print(what)
        ours.take((0.0, 0.0), 2000)
        ours.take(frame, 2016)                              # something pending again
        left = frame[0] - taken(frame, 0.016, 0.0)[0]
        o_turn, took = both(first, 2032, **player)
        check(abs(o_turn[0] - half(left + first[0])) <= 1 and o_turn[1] == half(first[1]), "the frame's turn and all that was pending, at once")
        s_view, s_regs = stock.view(2036)
        o_view, o_regs = ours.view(2036)
        check(o_view == s_view and o_regs == s_regs, "the view and the registers the original's")

    print("a turn no frame took for 300 ms")
    ours.take((0.0, 0.0), 3000)
    ours.take(frame, 3016)
    s_view, s_regs = stock.view(3316)
    o_view, o_regs = ours.view(3316)
    check(o_view == s_view and o_regs == s_regs, "the view and the registers the original's")
    o_turn, _ = both(first, 3332)
    check(o_turn == (half(first[0]), half(first[1])), "and the next frame's turn is the game's")
    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
