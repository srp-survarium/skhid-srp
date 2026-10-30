#!/usr/bin/env python3
"""Run patch 82 (rust/patches/src/map_names.rs, its lohook of map_names.toml:
the names on the Tab map while Alt is held) in the default build on made-up
game objects, from game_world::on_keyboard_action's start (0x25CD00) to past
its show_player_list (0x25CD39).

    python check_map_names_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them; check_hide_ui_026g.py's harness: Scaleform's Invoke 0x511600
is a stub that records each call, show_player_list 0x2384C0 a stub that
records its byte. The game's own code reads the keyboard: the input world's
vtable+24h is the game's getter (0x79E40, [this+24h]), the keyboard's vtable
the platform keyboard's (0xA25478, slot 0 is_key_down 0x3B8D00 reading the
state at +4 + 4 x key), so Tab is asked as in the game. Checked, each key in
both builds:

- the original invokes nothing; the patched build stops at 0x25CD39 with
  every register, the stack and the frame as the original's, show_player_list
  given the same Tab byte;
- the hud movie ([[[handler+4]+8]+108h]+4) is told root.show_map_names(held),
  held = left or right Alt down (bit 80h of its state; the other bits are not
  it), at the first key and at each change, not again while it holds; Tab
  held changes nothing of it;
- a movie that refuses (Invoke answers 0: no root yet) is told again at the
  next key until it takes it; another hud movie (a new game world's) is told
  at once; without a hud movie nothing; a keyboard of another vtable is not
  read (nothing told) and the game's run is the original's.
"""
import struct
import sys

from unicorn import UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import patch_026g as base
import check_hide_ui_026g as hide

START, PAST = 0x25CD00, 0x25CD39            # on_keyboard_action's start; after show_player_list (test bl,bl)
SHOW_PLAYER_LIST = 0x2384C0
GETTER = 0x79E40                            # the input world's vtable+24h: mov eax,[ecx+24h] / ret
KEYBOARD_VTABLE = 0xA25478                  # vostok::input::platform::keyboard (slot 0 is_key_down 0x3B8D00)
WORK = 0x30000000
INPUT_WORLD, INPUT_VTABLE, KEYBOARD, OTHER_VTABLE = WORK, WORK + 0x100, WORK + 0x1000, WORK + 0x2000
OTHER_MOVIE = 0x20076000
TAB, LEFT_ALT, RIGHT_ALT, W = 0x0F, 0x38, 0xB8, 0x11
DOWN, HOLD = 1, 3


class Run(hide.Run):
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_map(WORK, 0x10000, UC_PROT_ALL)
        self.word(INPUT_WORLD, INPUT_VTABLE)
        self.word(INPUT_VTABLE + 0x24, GETTER)
        self.word(INPUT_WORLD + 0x24, KEYBOARD)
        # another keyboard's vtable: the platform one's is_key_down, at another address
        self.emu.mem_write(OTHER_VTABLE, bytes(self.emu.mem_read(KEYBOARD_VTABLE, 0x40)))
        self.refusing = False
        self.given = []
        self.emu.hook_add(hide.UC_HOOK_CODE, self.show_list, begin=SHOW_PLAYER_LIST, end=SHOW_PLAYER_LIST)

    def show_list(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.given.append(self.read(sp + 4) & 0xFF)
        emu.reg_write(UC_X86_REG_ESP, sp + 8)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def stub(self, emu, address, size, _):
        super().stub(emu, address, size, _)
        if address == hide.INVOKE and self.refusing:
            emu.reg_write(UC_X86_REG_EAX, 0)

    def key(self, down=(), key=W, kind=DOWN, movie=hide.HUD_MOVIE, vtable=KEYBOARD_VTABLE, states=None):
        """One key the game world is given, the keys `down` held (or the raw `states`): where it stopped,
        the registers, the stack and frame bytes, the Tab byte given and the Invokes."""
        self.emu.mem_write(KEYBOARD, bytes(0x404))
        self.word(KEYBOARD, vtable)
        for dik, state in (states or {k: 0x80 for k in down}).items():
            self.emu.mem_write(KEYBOARD + 4 + 4 * dik, bytes([state]))
        self.emu.mem_write(hide.HANDLER, bytes(0x400))
        self.movies(hide.HANDLER + 4, movie != 0, False)
        self.word(hide.HUD_UI + 4, movie or hide.HUD_MOVIE)
        sp = hide.STACK + 0x8000
        self.emu.mem_write(sp - 0x100, bytes(0x200))
        for i, value in enumerate((hide.SENTINEL, INPUT_WORLD, key, kind)):
            self.word(sp + 4 * i, value)
        self.given = []
        regs, esp = self.run(START, (PAST,), sp, {**hide.GAME, UC_X86_REG_ECX: hide.HANDLER})
        frame = self.read(esp, 0x40) + self.read(sp, 0x10)
        return self.stopped, regs, esp, frame, self.given, self.invokes()


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

    told = lambda shown, movie=hide.HUD_MOVIE: [(movie, "root.show_map_names", (shown,))]
    steps = [
        # (what, held keys, the key, its kind, the hud movie, the keyboard's vtable, refusing, raw states, told)
        ("the first key, no Alt", (), W, DOWN, hide.HUD_MOVIE, KEYBOARD_VTABLE, False, None, told(0)),
        ("the next, no Alt", (W,), W, HOLD, hide.HUD_MOVIE, KEYBOARD_VTABLE, False, None, []),
        ("left Alt down", (LEFT_ALT,), LEFT_ALT, DOWN, hide.HUD_MOVIE, KEYBOARD_VTABLE, False, None, told(1)),
        ("left Alt held, the next tick", (LEFT_ALT,), LEFT_ALT, HOLD, hide.HUD_MOVIE, KEYBOARD_VTABLE, False, None, []),
        ("Tab down too", (LEFT_ALT, TAB), TAB, DOWN, hide.HUD_MOVIE, KEYBOARD_VTABLE, False, None, []),
        ("right Alt down too", (LEFT_ALT, RIGHT_ALT, TAB), RIGHT_ALT, DOWN, hide.HUD_MOVIE, KEYBOARD_VTABLE, False,
         None, []),
        ("left Alt up, the right held", (RIGHT_ALT, TAB), LEFT_ALT, 2, hide.HUD_MOVIE, KEYBOARD_VTABLE, False, None,
         []),
        ("right Alt up, Tab held", (TAB,), RIGHT_ALT, 2, hide.HUD_MOVIE, KEYBOARD_VTABLE, False, None, told(0)),
        ("Alt's state without bit 80h", (), TAB, HOLD, hide.HUD_MOVIE, KEYBOARD_VTABLE, False,
         {LEFT_ALT: 0x7F, RIGHT_ALT: 0x01, TAB: 0x80}, []),
        ("right Alt down, the movie refusing", (RIGHT_ALT,), RIGHT_ALT, DOWN, hide.HUD_MOVIE, KEYBOARD_VTABLE, True,
         None, told(1)),
        ("held, still refusing", (RIGHT_ALT,), RIGHT_ALT, HOLD, hide.HUD_MOVIE, KEYBOARD_VTABLE, True, None, told(1)),
        ("held, the movie taking it", (RIGHT_ALT,), RIGHT_ALT, HOLD, hide.HUD_MOVIE, KEYBOARD_VTABLE, False, None,
         told(1)),
        ("held, the next tick", (RIGHT_ALT,), RIGHT_ALT, HOLD, hide.HUD_MOVIE, KEYBOARD_VTABLE, False, None, []),
        ("another hud movie (a new game world's)", (RIGHT_ALT,), RIGHT_ALT, HOLD, OTHER_MOVIE, KEYBOARD_VTABLE, False,
         None, told(1, OTHER_MOVIE)),
        ("no hud movie", (), W, DOWN, 0, KEYBOARD_VTABLE, False, None, []),
        ("a keyboard of another vtable", (), RIGHT_ALT, 2, OTHER_MOVIE, OTHER_VTABLE, False, None, []),
        ("the platform keyboard again, Alt up", (), RIGHT_ALT, 2, OTHER_MOVIE, KEYBOARD_VTABLE, False, None,
         told(0, OTHER_MOVIE)),
    ]
    for name, down, key, kind, movie, vtable, refusing, states, want in steps:
        patched.refusing = refusing
        a = stock.key(down, key, kind, movie, vtable, states)
        b = patched.key(down, key, kind, movie, vtable, states)
        tab = int(TAB in down or (states or {}).get(TAB, 0) & 0x80 != 0)
        same = a[:5] == b[:5] and a[0] == PAST and a[4] == [tab] and a[1][UC_X86_REG_ESI] == hide.HANDLER + 4
        report(same and a[5] == [] and b[5] == want,
               f"{name}: " + (f"show_map_names({want[0][2][0]})" if want else "nothing told")
               + "; the registers, stack, frame and Tab byte as the original's")

    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
