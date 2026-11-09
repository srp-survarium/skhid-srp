#!/usr/bin/env python3
"""Patch 102 (MOUSE_BUTTONS): a side mouse button is no left click in the
menus (client_patch.md section 3.63).

    python check_mouse_buttons_026g.py <original survarium.exe>

Each of the five handlers that give a mouse key to their movie run, in the
original and in the built exe, from the key's mapping (EAX the key) to the
handler's `ret`, the movie's mouse call (0x2AF610, `ret 0Ch`) stubbed to
record its button:
- keys 151h, 152h, 153h (left, right, middle): the movie gets 0, 1, 2 in both;
- keys 154h..158h (mouse4..mouse8): the original gives the movie 0 (a left
  click); the built exe gives it nothing;
- every run ends at the handler's ret with AL 1 and the same ESP in both.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP

import patch_026g as base

MOVIE_MOUSE = 0x2AF610
# (name, start: the mapping's first instruction, EAX the key; the handler's ret)
HANDLERS = (
    ("swf_input_translator::process_mouse_btn", 0x1FDD30, 0x1FDD6A),
    ("game_options::on_mouse_key_action", 0x201752, 0x20178F),
    ("game_world::on_mouse_key_action", 0x25D08A, 0x25D0C9),
    ("lobby_menu::on_mouse_key_action", 0x26D042, 0x26D080),
    ("login_menu::on_mouse_key_action", 0x28DCD8, 0x28DD13),
)
STACK = 0x21000000
SP = STACK + 0x8000


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
        emu.hook_add(UC_HOOK_CODE, self.movie, begin=MOVIE_MOUSE, end=MOVIE_MOUSE)
        emu.hook_add(UC_HOOK_CODE, self.end)
        self.ret = None

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def movie(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.calls.append(self.read(sp + 8))    # this, then (?, button, ...): push edx/esi, push ecx, push float
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + 12)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def end(self, emu, address, size, _):
        if address == self.ret:
            emu.emu_stop()

    def key(self, start, ret, key):
        self.emu.mem_write(STACK, bytes(0x10000))
        self.emu.reg_write(UC_X86_REG_EAX, key)
        self.emu.reg_write(UC_X86_REG_ESP, SP)
        self.emu.reg_write(UC_X86_REG_EBP, SP + 0x40)    # game_options ends with mov esp,ebp / pop ebp
        self.calls, self.ret = [], ret
        self.emu.emu_start(start, 0, count=10000)
        assert self.emu.reg_read(UC_X86_REG_EIP) == ret, hex(self.emu.reg_read(UC_X86_REG_EIP))
        return self.calls, self.emu.reg_read(UC_X86_REG_EAX) & 0xFF, self.emu.reg_read(UC_X86_REG_ESP)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, True)
    old, new = Run(original), Run(bytes(built))
    for name, start, ret in HANDLERS:
        for key in range(0x151, 0x159):
            o_calls, o_al, o_sp = old.key(start, ret, key)
            n_calls, n_al, n_sp = new.key(start, ret, key)
            assert o_al == n_al == 1 and o_sp == n_sp, (name, hex(key), o_al, n_al, hex(o_sp), hex(n_sp))
            if key <= 0x153:
                assert o_calls == n_calls == [key - 0x151], (name, hex(key), o_calls, n_calls)
            else:
                assert o_calls == [0] and n_calls == [], (name, hex(key), o_calls, n_calls)
        print(f"ok {name}: left/right/middle as before; mouse4..mouse8 no longer a left click")


if __name__ == "__main__":
    main()
