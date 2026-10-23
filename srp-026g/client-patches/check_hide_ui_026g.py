#!/usr/bin/env python3
"""Run patch 62 (hide_ui_026g.asm: one key hides the interface and shows it
again) in the default build on made-up game objects.

    python check_hide_ui_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them; the patch's three entries run from the game's own sites, with
the game's own show_ui (0x1F95F0) and get_binded_action (0x207D50):

- the match: on_keyboard_action's case of action 19h (0x25CED1) runs with a
  made-up input handler at [esp+10h] (+18h the replay's movie, +1Ch the game
  world, +2E0h the cursor shown) until 0x25CEDB (the replay's toggle) or
  0x25CF0D (the key not taken). The scene's show_movie (0x23DC00) and
  hide_movie (0x23DC70) are stubs that record the scene and the movie's place.
  With the replay's movie both builds reach 0x25CEDB with esi the handler and
  nothing shown or hidden. Without it the original reaches 0x25CF0D and does
  nothing; the patch hides the hud (+204h) and the player icons (+20Ch) when
  they are shown and the cursor is not, and shows them again only when this
  key hid them; esp and the stack as the original's;
- the kits: show_profiles_selector's call of show_cursor (0x23845B) runs with
  edi a made-up hud (+18h the game world) and show_cursor (0x23A930) a stub
  that records its byte and hud and pops it. A game world this key hid is
  shown again before the cursor when the byte is 1; anything else only
  reaches show_cursor; both back at 0x238460 with the byte popped and ebx,
  esi, edi, ebp kept;
- the lobby: lobby_menu::on_keyboard_action's binder call (0x26CCD2) runs
  with a made-up key binder (the game's action table's descriptions, a few
  keys bound), edi 2, esi the key and ebx a made-up lobby menu (its movie
  [[+690h]+108h]+4), until 0x26CCD7 (back from the call), 0x26CCAE (the key
  taken) or 0x26CDD2 (the key to the movie, no action). Scaleform's Invoke
  (0x511600) is a stub that records it and answers root.interface_hidden
  with a bool. F11 (bound to toggle_replay_ui) invokes root.toggle_interface
  on the movie (no result, no argument) and is taken; without the movie it is
  not. While the movie answers that its interface is hidden, every other key
  (a window's, F6's Play, Enter) goes to 0x26CDD2; else it comes back at
  0x26CCD7 with the original's answer, toggle place and registers.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import patch_026g as base

CASE, REPLAY_BACK, KEY_OTHER = 0x25CED1, 0x25CEDB, 0x25CF0D
KITS_SITE, KITS_BACK, SHOW_CURSOR = 0x23845B, 0x238460, 0x23A930
LOBBY_SITE, LOBBY_BACK, LOBBY_TAKEN, LOBBY_TO_MOVIE = 0x26CCD2, 0x26CCD7, 0x26CCAE, 0x26CDD2
SHOW_MOVIE, HIDE_MOVIE, INVOKE = 0x23DC00, 0x23DC70, 0x511600
ACTIONS = 0xA74AF0                     # the action table: 20-byte {name, id, kinds, toggle, key name}
HANDLER, WORLD, HUD, LOBBY = 0x20000000, 0x20010000, 0x20020000, 0x20030000
HOLDER, UI, MOVIE, BINDER, KEYS = 0x20040000, 0x20041000, 0x20042000, 0x20050000, 0x20060000
STACK, SENTINEL = 0x21000000, 0x22000000
REGS = (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP)
GAME = dict(zip(REGS, (0x0A0A0A0A, 0x0C0C0C0C, 0x0D0D0D0D, 0x0B0B0B0B, 0x05050505, 0x0E0E0E0E, 0x0019FF00)))
OTHER_WORLD = 0x20070000
# The keys bound (DIK code: action id), as the defaults have them: F11
# toggle_replay_ui (kinds 5), C character (2) and crouch_toggle (5), F friends
# (2) and melee (5), W forward (5), F6 next_player (5; the lobby's Play at
# 0x26CDA2); H and Enter nothing.
F11, C, F, W, H, F6, ENTER = 0x57, 0x2E, 0x21, 0x11, 0x23, 0x40, 0x1C
BOUND = {0x19: F11, 0x40: C, 0x06: C, 0x44: F, 0x21: F, 0x0B: W, 0x17: F6}


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at in (HANDLER, WORLD, HUD, LOBBY, HOLDER, BINDER, KEYS, OTHER_WORLD, STACK, SENTINEL):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        self.calls = []
        self.hidden = 0
        for stub in (SHOW_MOVIE, HIDE_MOVIE, INVOKE, SHOW_CURSOR):
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=stub, end=stub)
        self.stops = ()
        emu.hook_add(UC_HOOK_CODE, self.stop)
        # the binder: 48h bindings {description, key, second key}, the bound ones' keys made up
        emu.mem_write(BINDER, bytes(0x360))
        for i in range((0xA74FA0 - ACTIONS) // 20):
            description = ACTIONS + 20 * i
            action = self.read(description + 4)
            self.word(BINDER + 12 * action, description)
            if action in BOUND:
                key = KEYS + 16 * action
                self.word(key + 4, BOUND[action])
                self.word(BINDER + 12 * action + 4, key)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at, size=4):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0] if size == 4 else bytes(self.emu.mem_read(at, size))

    def text(self, at):
        return bytes(self.emu.mem_read(at, 64)).split(b"\0")[0]

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 * i) for i in range(1, 6)]
        pops = 0
        if address == SHOW_MOVIE:          # eax the scene, esi the movie's place; ret
            self.calls.append(("show", emu.reg_read(UC_X86_REG_EAX), emu.reg_read(UC_X86_REG_ESI)))
        elif address == HIDE_MOVIE:        # edx the scene, eax the movie's place; ret
            self.calls.append(("hide", emu.reg_read(UC_X86_REG_EDX), emu.reg_read(UC_X86_REG_EAX)))
        elif address == SHOW_CURSOR:       # edi the hud, the byte; ret 4
            self.calls.append(("cursor", emu.reg_read(UC_X86_REG_EDI), args[0] & 0xFF))
            pops = 4
        else:                              # Invoke: thiscall (name, result, arguments, count), ret 10h; al 1
            self.calls.append(("invoke", emu.reg_read(UC_X86_REG_ECX), self.text(args[0]), args[1] != 0,
                               args[2], args[3]))
            if args[1] and self.text(args[0]) == b"root.interface_hidden":
                emu.mem_write(args[1] + 4, struct.pack("<II", 2, self.hidden))
            emu.reg_write(UC_X86_REG_EAX, 1)
            pops = 16
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def stop(self, emu, address, size, _):
        if address in self.stops and self.started:
            self.stopped = address
            emu.emu_stop()
        self.started = True

    def run(self, start, stops, sp, regs):
        for reg, value in regs.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.stops, self.stopped, self.started, self.calls = stops, None, False, []
        self.emu.emu_start(start, 0, count=500000)
        return {reg: self.emu.reg_read(reg) for reg in REGS}, self.emu.reg_read(UC_X86_REG_ESP)

    # The match: the case 19h, [esp+10h] the handler.
    def match(self, replay, world, shown, cursor, hidden, hidden_world):
        emu = self.emu
        emu.mem_write(HANDLER, bytes(0x400))
        emu.mem_write(WORLD, bytes(0x400))
        self.word(HANDLER + 0x18, 0x30303030 if replay else 0)
        self.word(HANDLER + 0x1C, WORLD if world else 0)
        emu.mem_write(HANDLER + 0x2E0, bytes([cursor]))
        emu.mem_write(WORLD + 0xA0, bytes([shown]))
        if hidden_world is not None:
            self.word(hidden_world, hidden)
        sp = STACK + 0x8000
        emu.mem_write(STACK, bytes(0x10000))
        for i, value in enumerate((0xE0E0E0E0, 0x50505050, 0xB0B0B0B0, 0x11111111, HANDLER, 0x19FF00, SENTINEL)):
            self.word(sp + 4 * i, value)
        before = self.read(sp, 0x40)
        regs, esp = self.run(CASE, (REPLAY_BACK, KEY_OTHER), sp, {**GAME, UC_X86_REG_EBP: sp + 0x14})
        return (self.stopped, esp, regs[UC_X86_REG_ESI] if self.stopped == REPLAY_BACK else None,
                regs[UC_X86_REG_EBP], self.read(sp, 0x40) == before, list(self.calls), self.read(WORLD + 0xA0, 1)[0],
                self.read(hidden_world) if hidden_world is not None else None)

    # The kits: the site's push done, then its call.
    def kits(self, byte, shown, hidden, hidden_world):
        emu = self.emu
        emu.mem_write(HUD, bytes(0x400))
        emu.mem_write(WORLD, bytes(0x400))
        self.word(HUD + 0x18, WORLD)
        emu.mem_write(WORLD + 0xA0, bytes([shown]))
        if hidden_world is not None:
            self.word(hidden_world, hidden)
        sp = STACK + 0x8000 - 4
        self.word(sp, 0x37373700 | byte)
        regs, esp = self.run(KITS_SITE, (KITS_BACK,), sp, {**GAME, UC_X86_REG_EDI: HUD})
        return (self.stopped, esp - sp, regs, list(self.calls), self.read(WORLD + 0xA0, 1)[0],
                self.read(hidden_world) if hidden_world is not None else None)

    # The lobby: the site's push of the toggle's place done, then its call.
    def lobby(self, key, movie, hidden=0):
        emu = self.emu
        self.hidden = hidden
        emu.mem_write(LOBBY, bytes(0x1000))
        self.word(LOBBY + 0x690, HOLDER if movie else 0)
        self.word(HOLDER + 0x108, UI)
        self.word(UI + 4, MOVIE)
        sp = STACK + 0x8000 - 4
        toggle = STACK + 0x9000
        self.word(toggle, 0x7E7E7E7E)
        self.word(sp, toggle)
        emu.mem_write(STACK + 0x8000, bytes(0x40))
        regs, esp = self.run(LOBBY_SITE, (LOBBY_BACK, LOBBY_TAKEN, LOBBY_TO_MOVIE), sp,
                             {**GAME, UC_X86_REG_EAX: BINDER, UC_X86_REG_EDI: 2, UC_X86_REG_ESI: key,
                              UC_X86_REG_EBX: LOBBY})
        return self.stopped, esp - sp, regs, list(self.calls), self.read(toggle)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    linked = base.arena()
    hidden_world = linked.symbols["hidden_world"]
    stock, patched = Run(original), Run(bytes(built))
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    hide = [("hide", WORLD, WORLD + 0x204), ("hide", WORLD, WORLD + 0x20C)]
    show = [("show", WORLD, WORLD + 0x204), ("show", WORLD, WORLD + 0x20C)]
    print("the match (on_keyboard_action's case 19h, 0x25CED1, to 0x25CEDB or 0x25CF0D)")
    for replay in (0, 1):
        for world in (0, 1):
            for shown in (0, 1):
                for cursor in (0, 1):
                    for hidden in (0, WORLD, OTHER_WORLD):
                        a = stock.match(replay, world, shown, cursor, hidden, None)
                        b = patched.match(replay, world, shown, cursor, hidden, hidden_world)
                        same = a[:5] == b[:5] and a[4] and not a[5]
                        if replay:
                            want, state = [], (shown, hidden)
                            ok = same and b[0] == REPLAY_BACK and b[2] == HANDLER
                        else:
                            if not world:
                                want, state = [], (shown, hidden)
                            elif shown and not cursor:
                                want, state = hide, (0, WORLD)
                            elif not shown and hidden == WORLD:
                                want, state = show, (1, 0)
                            else:
                                want, state = [], (shown, hidden)
                            ok = same and b[0] == KEY_OTHER
                        ok = ok and b[5] == want and b[6:] == state
                        report(ok, f"replay {replay} world {world} shown {shown} cursor {cursor} hidden by the key "
                                   f"{'yes' if hidden == WORLD else 'another' if hidden else 'no'}: "
                                   + ("hides the hud and the icons" if want == hide else "shows them"
                                      if want == show else "nothing") + f", at {b[0]:#x} as the original")

    print("the kits (show_profiles_selector's show_cursor call, 0x23845B)")
    for byte in (0, 1):
        for shown in (0, 1):
            for hidden in (0, WORLD, OTHER_WORLD):
                a = stock.kits(byte, shown, hidden, None)
                b = patched.kits(byte, shown, hidden, hidden_world)
                restore = byte and hidden == WORLD
                want = (show if restore and not shown else []) + [("cursor", HUD, byte)]
                state = (1 if restore else shown, 0 if restore else hidden)
                keep = {reg: GAME[reg] for reg in (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EBP)}
                ok = a[0] == b[0] == KITS_BACK and a[1] == b[1] == 4 and a[3] == [("cursor", HUD, byte)] \
                    and b[3] == want and b[4:] == state and b[2][UC_X86_REG_EDI] == HUD \
                    and all(b[2][reg] == value for reg, value in keep.items())
                report(ok, f"shown {byte}, the ui {shown}, hidden by the key "
                           f"{'yes' if hidden == WORLD else 'another' if hidden else 'no'}: "
                           + ("the ui shown, then " if want != [("cursor", HUD, byte)] else "")
                           + "show_cursor, back at 0x238460 with the byte popped, ebx esi edi ebp kept")

    print("the lobby (lobby_menu::on_keyboard_action's binder call, 0x26CCD2)")
    asked = ("invoke", MOVIE, b"root.interface_hidden", True, 0, 0)
    for key, name in ((F11, "F11"), (C, "C"), (F, "F"), (W, "W"), (H, "H"), (F6, "F6"), (ENTER, "Enter")):
        for movie in (True, False):
            for hidden in (0, 1):
                a = stock.lobby(key, movie, hidden)
                b = patched.lobby(key, movie, hidden)
                keep = {UC_X86_REG_EBP: GAME[UC_X86_REG_EBP], UC_X86_REG_EDI: 2, UC_X86_REG_ESI: key,
                        UC_X86_REG_EBX: LOBBY}
                kept = all(b[2][reg] == value for reg, value in keep.items())
                same = a[0] == LOBBY_BACK and a[1] == b[1] == 4 and kept and not a[3] and a[4] == b[4]
                if key == F11 and movie:
                    ok = same and a[2][UC_X86_REG_EAX] == 0x49 and b[0] == LOBBY_TAKEN \
                        and b[3] == [("invoke", MOVIE, b"root.toggle_interface", False, 0, 0)]
                    what = "root.toggle_interface invoked, the key taken (0x26ccae)"
                elif movie and hidden:
                    ok = same and b[0] == LOBBY_TO_MOVIE and b[3] == [asked]
                    what = "hidden: no action of the game, the key to the movie (0x26cdd2)"
                else:
                    ok = same and b[0] == LOBBY_BACK and b[3] == ([asked] if movie else []) \
                        and a[2][UC_X86_REG_EAX] == b[2][UC_X86_REG_EAX]
                    what = f"action {b[2][UC_X86_REG_EAX]:#x} as the original's"
                report(ok, f"{name}, movie {'yes' if movie else 'no'}, the interface "
                           f"{'hidden' if hidden else 'shown'}: {what}, its toggle place as the original's, "
                           "ebx esi edi ebp kept")

    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
