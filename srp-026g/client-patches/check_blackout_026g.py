#!/usr/bin/env python3
"""Run patch 65 (blackout_026g.asm and .c: Blackout, a battery match with no
interface but the players' markers) in the default build on made-up game
objects.

    python check_blackout_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them; each part runs from the game's own site, the original exe
the same way (check_hide_ui_026g.py's harness: Scaleform's Invoke 0x511600
is a stub that records each call and answers root.interface_hidden):

- the static info: the game's match_options::deserialize (0x1F0E70) reads
  made-up options (check_team_damage_026g.py's) with and without srp's block.
  The options' +57870h must hold "SRBO" for a block with tag 2 and one byte 1
  (alone, after team damage's entry, after an unknown tag), zeros for none,
  another value or length, a short block or another magic; the original
  leaves it as it was; every field the original reads read alike, `ret 8`;
- a match's reset: initialize_match's Invoke of root.reset (0x234AE8) after
  each static info: after a Blackout one the reset, then the hud's
  root.force_interface_hidden(true) and the icons' root.hide_interface(false)
  (the interface forced hidden, the icons shown as every match starts), and
  F11 (on_keyboard_action's case 19h, 0x25CED1) then invokes nothing and goes
  on at 0x25CF0D, or at 0x25CEDB with a replay's movie; after
  any other the reset, then the hud's root.force_interface_hidden(false) and
  the icons' root.hide_interface(false), and F11 hides as patch 62 (the next
  match is free again);
- the markers' feed: update_ui's call of update_player_icons (0x23B207, a
  stub) runs once a frame in any match; before it the icons' movie is told
  the forced state (root.force_interface_hidden: 1 in a Blackout match, its
  players' markers kept and its objects' hidden, 0 in another) at the first
  frame after a reset, after the hud holds another icons' movie, and again
  at the next frame while the movie does not take it (Invoke answering 0);
  else nothing; no movie, nothing; the original feeds alike and invokes
  nothing;
- Tab: show_player_list's call (0x25CD34) keeps Tab's state, and the input
  builder's end (0x2421B0) takes the moves and acts out of the actions mask
  while Tab is held in a Blackout match, an act held through it until let go.
"""
import sys

from unicorn import UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ESP, UC_X86_REG_EBP, \
    UC_X86_REG_ESI, UC_X86_REG_EDI

import patch_026g as base
import check_hide_ui_026g as hide
from check_team_damage_026g import empty_options, srpx, numbers

DESERIALIZE = 0x1F0E70
OPTIONS, WORK = 0x30000000, 0x30100000
OPTIONS_BLACKOUT, MAGIC = 0x57870, b"SRBO"
PROFILE_SIZE, PROFILE_TEAM = 0x45FC, 0x45EC
OTHER_ICONS = 0x20076000               # another icons' movie (a new game world's)
ICONS_SITE, ICONS_BACK, PLAYER_ICONS = 0x23B206, 0x23B20C, 0x23AA10
TAB_SITE, SHOW_PLAYER_LIST = 0x25CD34, 0x2384C0
KEY_ON, KEY_END = 0x25CD41, 0x25CF0D   # on_keyboard_action after its Tab test: the key's way (patch 58's kit keys), its end
INPUT_SITE, INPUT_MASK = 0x2421B0, 0x33C
# the input's bits (srp's drive.rs::key)
FORWARD, BACK, LEFT, RIGHT, JUMP, FIRE, RELOAD, AIM = 0x1, 0x2, 0x4, 0x8, 0x10, 0x20, 0x40, 0x80
CROUCH, SPRINT, WEAPON_1, GRENADE, MELEE, SPOT = 0x100, 0x200, 0x1000, 0x800000, 0x8000000, 0x10000000
QUICK_1, USE = 0x4000, 0x200000


class Run(hide.Run):
    def __init__(self, data):
        super().__init__(data)
        self.linked = None
        for at, size in ((OPTIONS, 0x60000), (WORK, 0x10000)):
            self.emu.mem_map(at, size, UC_PROT_ALL)

    # match_options::deserialize on `payload`, +57870h `stale` before.
    def options(self, payload, stale=b"\x77" * 4):
        emu = self.emu
        emu.mem_write(OPTIONS, bytes(0x578A0))
        emu.mem_write(OPTIONS + OPTIONS_BLACKOUT, stale)
        emu.mem_write(WORK, payload)
        reader = WORK + 0x8000
        for i, value in enumerate((WORK, WORK, len(payload))):
            self.word(reader + 4 * i, value)
        sp = hide.STACK + 0x8000 - 12
        for i, value in enumerate((hide.SENTINEL, OPTIONS, 0)):
            self.word(sp + 4 * i, value)
        regs, esp = self.run(DESERIALIZE, (hide.SENTINEL,), sp, {**hide.GAME, UC_X86_REG_EAX: reader})
        return (self.stopped, esp - (sp + 12), bytes(self.emu.mem_read(OPTIONS + OPTIONS_BLACKOUT, 4)),
                self.read(OPTIONS + 0x577B0, 0x90), regs)

    def team(self, place, team):
        self.word(OPTIONS + PROFILE_SIZE * place + PROFILE_TEAM, team)

    # update_ui's `push edi / call` of update_player_icons (0x23B206), the hud in edi, its icons'
    # movie `icons` (0: none): how many times 0x23AA10 ran (a stub: it pops the ui), back at 0x23B20C,
    # the registers, the Invokes.
    def icons_fed(self, icons=hide.ICONS_MOVIE):
        emu = self.emu
        self.movies(hide.HUD, 1, icons != 0)
        self.word(hide.ICONS_UI + 4, icons or hide.ICONS_MOVIE)
        self.fed = 0
        def stub(emu, address, size, _):
            sp = emu.reg_read(UC_X86_REG_ESP)
            self.fed += 1
            emu.reg_write(UC_X86_REG_ESP, sp + 8)
            emu.reg_write(hide.UC_X86_REG_EIP, self.read(sp))
        handles = [emu.hook_add(hide.UC_HOOK_CODE, stub, begin=PLAYER_ICONS, end=PLAYER_ICONS)]
        # Research's units' markers (rn_markers) after the icons: not this check's (it returns at once).
        markers = self.linked.symbols.get("rn_markers") if self.linked else None
        if markers:
            def none(emu, address, size, _):
                sp = emu.reg_read(UC_X86_REG_ESP)
                emu.reg_write(UC_X86_REG_ESP, sp + 4)
                emu.reg_write(hide.UC_X86_REG_EIP, self.read(sp))
            handles.append(emu.hook_add(hide.UC_HOOK_CODE, none, begin=markers, end=markers))
        sp = hide.STACK + 0x8000
        regs, esp = self.run(ICONS_SITE, (ICONS_BACK,), sp, {**hide.GAME, UC_X86_REG_EDI: hide.HUD})
        for handle in handles:
            emu.hook_del(handle)
        kept = all(regs[r] == v for r, v in hide.GAME.items() if r != UC_X86_REG_EDI) and \
            regs[UC_X86_REG_EDI] == hide.HUD
        return self.fed, self.stopped == ICONS_BACK and esp == sp, kept, self.invokes()

    # game_world::on_keyboard_action's call of show_player_list (0x25CD34), the byte pushed and in
    # bl (0x25CD26), on through the game's Tab test (0x25CD39): where the key goes (0x25CD41 on its
    # way, 0x25CF0D the function's end), whether it got there with the byte popped, show_player_list
    # (a stub) given the byte and the other registers kept.
    def tab(self, down):
        emu = self.emu
        given = []
        def stub(emu, address, size, _):
            sp = emu.reg_read(UC_X86_REG_ESP)
            given.append(self.read(sp + 4) & 0xFF)
            emu.reg_write(UC_X86_REG_ESP, sp + 8)
            emu.reg_write(hide.UC_X86_REG_EIP, self.read(sp))
        handle = emu.hook_add(hide.UC_HOOK_CODE, stub, begin=SHOW_PLAYER_LIST, end=SHOW_PLAYER_LIST)
        sp = hide.STACK + 0x8000 - 4
        self.word(sp, 0xABCDEF00 | down)
        ebx = (hide.GAME[UC_X86_REG_EBX] & ~0xFF) | down
        regs, esp = self.run(TAB_SITE, (KEY_ON, KEY_END), sp, {**hide.GAME, UC_X86_REG_ESI: hide.HUD,
                                                                UC_X86_REG_EBX: ebx})
        emu.hook_del(handle)
        kept = all(regs[r] == v for r, v in hide.GAME.items() if r not in (UC_X86_REG_ESI, UC_X86_REG_EBX)) \
            and regs[UC_X86_REG_EBX] & ~0xFF == ebx & ~0xFF
        return self.stopped if esp == sp + 4 and given == [down] and kept else None

    # process_first_person_mode's epilogue (0x2421B0) on a handler whose mask is `mask`: the
    # mask after it, and whether it returned as the function (`ret 8`, edi esi ebx ebp popped).
    def input(self, mask):
        emu = self.emu
        handler = hide.HANDLER
        emu.mem_write(handler, bytes(0x400))
        self.word(handler + INPUT_MASK, mask)
        ebp = hide.STACK + 0x9000
        for i, value in enumerate((0x0BADEBB0, hide.SENTINEL, handler, 1)):
            self.word(ebp + 4 * i, value)
        sp = ebp - 0x100
        for i, value in enumerate((0xED1ED1ED, 0xE51E51E5, 0xEB1EB1EB)):
            self.word(sp + 4 * i, value)
        regs, esp = self.run(INPUT_SITE, (hide.SENTINEL,), sp, {**hide.GAME, UC_X86_REG_EBX: handler,
                                                                  UC_X86_REG_EBP: ebp})
        back = self.stopped == hide.SENTINEL and esp == ebp + 16 and regs[UC_X86_REG_EDI] == 0xED1ED1ED \
            and regs[UC_X86_REG_ESI] == 0xE51E51E5 and regs[UC_X86_REG_EBX] == 0xEB1EB1EB \
            and regs[UC_X86_REG_EBP] == 0x0BADEBB0
        return self.read(handler + INPUT_MASK), back



def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    stock, patched = Run(original), Run(bytes(built))
    patched.linked = base.arena()
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    print("the static info (match_options::deserialize 0x1F0E70)")
    team_damage = (1, numbers(3.0, 0.67, 2.0))
    cases = [
        ("no block", b"", False),
        ("Blackout", srpx((2, b"\x01")), True),
        ("team damage, then Blackout", srpx(team_damage, (2, b"\x01")), True),
        ("an unknown tag, then Blackout", srpx((9, b"xyz"), (2, b"\x01")), True),
        ("team damage alone", srpx(team_damage), False),
        ("Blackout 0", srpx((2, b"\x00")), False),
        ("Blackout of two bytes", srpx((2, b"\x01\x01")), False),
        ("a short block", srpx((2, b"\x01"))[:-1], False),
        ("another magic", b"SRPY" + bytes([2, 1, 1]), False),
    ]
    for name, block, want in cases:
        payload = empty_options(block)
        a, b = stock.options(payload), patched.options(payload)
        ok = a[0] == b[0] == hide.SENTINEL and a[1] == b[1] == 0 and a[3] == b[3] and a[2] == b"\x77" * 4 \
            and b[2] == (MAGIC if want else bytes(4)) \
            and all(a[4][r] == b[4][r] for r in (UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP))
        report(ok, f"{name}: +57870h {b[2]!r}; the original's fields read alike and its +57870h left, `ret 8`, "
                   "esi edi ebp kept")

    print("a match's reset (0x234AE8), then F11 (0x25CED1)")
    reset = (hide.HUD_MOVIE, "root.reset", ())
    hides = lambda flag: [(hide.HUD_MOVIE, "root.hide_interface", (flag,)),
                          (hide.ICONS_MOVIE, "root.hide_interface", (flag,))]
    forces = lambda flag: [(hide.HUD_MOVIE, "root.force_interface_hidden", (flag,)),
                           (hide.ICONS_MOVIE, "root.hide_interface", (0,))]
    asked = (hide.HUD_MOVIE, "root.interface_hidden", ())
    for name, block, blackout in (("a Blackout match", srpx((2, b"\x01")), True),
                                  ("the next, without", b"", False),
                                  ("a Blackout match again", srpx(team_damage, (2, b"\x01")), True)):
        patched.options(empty_options(block))
        r = patched.reset(1, 1)
        ok = r[0] == hide.RESET_BACK and r[1] == 16 and r[3] == [reset] + forces(1 if blackout else 0) \
            and r[2][UC_X86_REG_EBP] == hide.HUD and (r[2][UC_X86_REG_EAX] & 0xFF) == 1
        report(ok, f"{name}: root.reset, then the hud's root.force_interface_hidden({int(blackout)}) (its "
                   "countdown kept, HUD.as) and the icons' root.hide_interface(0) (shown: told the forced "
                   "state at their next frame), back at 0x234aed with the arguments popped")
        for hidden in (0, 1):
            m = patched.match(0, 1, 1, hidden)
            want = [] if blackout else [asked] + [h for h in hides(1 - hidden)]
            ok = m[0] == hide.KEY_OTHER and m[4] and m[5] == want
            report(ok, f"{name}, the movie answering {'hidden' if hidden else 'shown'}: F11 "
                       + ("does nothing" if blackout else f"gives root.hide_interface({1 - hidden})")
                       + ", at 0x25cf0d")
        m = patched.match(1, 1, 1, 0)
        report(m[0] == hide.REPLAY_BACK and m[2] == hide.HANDLER and not m[5],
               f"{name}, a replay: F11 toggles the replay's controls (0x25cedb), nothing invoked")

    print("the markers' movie's feed (update_ui's update_player_icons, 0x23B207)")
    on = srpx((2, b"\x01"))
    told = lambda movie, flag: [(movie, "root.force_interface_hidden", (flag,))]
    frames = [
        # (what, a reset after this static info first or not, the icons' movie, it refuses, the Invokes wanted)
        ("a match without, its first frame after the reset", b"", True, hide.ICONS_MOVIE, False,
         told(hide.ICONS_MOVIE, 0)),
        ("its next frame", None, False, hide.ICONS_MOVIE, False, []),
        ("a Blackout match, its first frame after the reset", on, True, hide.ICONS_MOVIE, False,
         told(hide.ICONS_MOVIE, 1)),
        ("its next frame", None, False, hide.ICONS_MOVIE, False, []),
        ("the hud holding another icons' movie (a new game world's)", None, False, OTHER_ICONS, False,
         told(OTHER_ICONS, 1)),
        ("its next frame", None, False, OTHER_ICONS, False, []),
        ("a Blackout match again, its movie without a root yet (Invoke answers 0)", on, True, hide.ICONS_MOVIE,
         True, told(hide.ICONS_MOVIE, 1)),
        ("the next frame, the movie taking it", None, False, hide.ICONS_MOVIE, False, told(hide.ICONS_MOVIE, 1)),
        ("the frame after", None, False, hide.ICONS_MOVIE, False, []),
        ("no icons' movie", None, False, 0, False, []),
        ("the next match without, after its reset", b"", True, hide.ICONS_MOVIE, False, told(hide.ICONS_MOVIE, 0)),
    ]
    for name, block, reset, icons, refuses, want in frames:
        if block is not None:
            patched.options(empty_options(block))
            stock.options(empty_options(block))
        if reset:
            patched.reset(1, 1)
        patched.refused, patched.refusing = ((b"root.force_interface_hidden",), (icons,)) if refuses else ((), ())
        calls, back, kept, invoked = patched.icons_fed(icons)
        report(back and kept and calls == 1 and invoked == want,
               f"{name}: update_player_icons run {calls} time(s); "
               + (f"the icons' root.force_interface_hidden({want[0][2][0]})" if want else "nothing invoked")
               + ", back at 0x23b20c with the ui popped, the registers kept")
    patched.refused = ()
    calls, back, kept, invoked = stock.icons_fed()
    report(back and kept and calls == 1 and invoked == [],
           "the original: update_player_icons run once, nothing invoked, back at 0x23b20c")

    print("Tab in a Blackout match (show_player_list's call 0x25CD34, the input builder's end 0x2421B0)")
    moving = FORWARD | SPRINT | FIRE | AIM | CROUCH | WEAPON_1
    steps = [
        ("Tab up: moving, sprinting, firing and aiming, crouched", 0, moving, moving),
        ("Tab down: no move, no fire, no aim; the crouch and the weapon key stay", 1, moving,
         CROUCH | WEAPON_1),
        ("Tab held, fire still held: a grenade, melee, use, quick use 1, jump, reload pressed: none of them; "
         "spot stays", 1, FIRE | GRENADE | MELEE | USE | QUICK_1 | JUMP | RELOAD | SPOT, SPOT),
        ("Tab up, fire and the grenade still held: they stay out, the moves come back", 0,
         FORWARD | LEFT | FIRE | GRENADE, FORWARD | LEFT),
        ("fire let go, the grenade still held", 0, FORWARD | GRENADE, FORWARD),
        ("fire pressed again: it fires; the grenade let go", 0, FORWARD | FIRE, FORWARD | FIRE),
        ("the grenade pressed again", 0, GRENADE, GRENADE),
    ]
    for run in (stock, patched):
        run.options(empty_options(srpx((2, b"\x01"))))
    for name, down, mask, want in steps:
        a_tab, b_tab = stock.tab(down), patched.tab(down)
        a, b = stock.input(mask), patched.input(mask)
        ok = a_tab == (KEY_END if down else KEY_ON) and b_tab == KEY_ON and a == (mask, True) and b == (want, True)
        report(ok, f"{name}: {b[0]:#x} sent (pressed {mask:#x}); the original sends what is pressed; "
                   "show_player_list given the byte, the builder returns as itself; the key goes on (0x25cd41: "
                   "the kits' keys 1 to 3)" + (", the original's to the end (0x25cf0d: its Tab list)" if down else ""))
    # Only the moves and acts: every other bit (crouch, the weapon keys, the wheel, fire mode, ammo type,
    # spot, the suicide key, the unused ones) goes as pressed; the Esc menu is no input bit at all.
    patched.tab(1)
    blocked = FORWARD | BACK | LEFT | RIGHT | SPRINT | JUMP | FIRE | RELOAD | AIM | 0xFC000 | 0x100000 \
        | USE | 0x400000 | GRENADE | MELEE
    b = patched.input(0xFFFFFFFF)
    report(b == (0xFFFFFFFF & ~blocked, True), f"Tab down, every bit pressed: {b[0]:#x} sent, all but the moves and "
           "acts")
    patched.tab(0)
    patched.input(0)
    for run in (stock, patched):
        run.options(empty_options(b""))
    where = patched.tab(1)
    b = patched.input(moving)
    report(b == (moving, True) and where == KEY_END, f"a match without Blackout, Tab down: {b[0]:#x} sent, all that "
           "is pressed; the key to the end (0x25cf0d) as the original's (its Tab list)")
    patched.options(empty_options(srpx((2, b"\x01"))))
    b = patched.input(moving)
    report(b == (moving, True), "a new Blackout match's static info forgets a Tab of before: all that is pressed")

    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
