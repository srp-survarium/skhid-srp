#!/usr/bin/env python3
"""Run patch 65 (rust/patches/src/blackout.rs, its lohooks of blackout.toml:
Blackout, a battery match with no interface) in the default build on made-up
game objects, each from the game's own code before its site to past it.

    python check_blackout_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them (the Rust crate's code and its generated entries);
check_hide_ui_026g.py's harness: Scaleform's Invoke 0x511600 is a stub that
records each call. The original runs the same way and must do what it did:

- the options read: process_static_match_info from 0x27B917 (its pushes,
  the game's own match_options::deserialize 0x1F0E70 on made-up options,
  the reader at ebp-2Ch) to 0x27B932, and a replay's chunk 1 from 0x1F094C
  (the reader made at esp+14h, the options at [esp+40h]) to its `ret 8`:
  the options' +57870h holds "SRBO" for srp's block with tag 2 and one byte
  1 (alone, after another entry, after an unknown tag), zeros for none,
  another value or length, a short block or another magic; the original
  leaves it; esi, edi, ebp and the stack as the game's code leaves them;
- the markers' feed: update_ui's `push edi / call 0x23AA10` (0x23B206) to
  update_player_icons' prologue's end (0x23AA16): the hud movie told
  root.force_interface_hidden and the icons movie root.force_interface_hidden
  and root.force_interface_tab exactly at a change of the movie or the state
  (and again at the next frame while the movie refuses, Invoke answering 0);
  the original invokes nothing;
- Tab: on_keyboard_action from 0x25CD26 (bl Tab down) past show_player_list
  (a stub given the byte) and the game's Tab test: in a Blackout match the
  key goes on (0x25CD41, patch 58's kit keys), in another, and in the
  original, to the end (0x25CF0D) while Tab is down;
- names: the native Alt/crosshair result through its HUD cache; Tab forces
  same-team names only in Blackout, preserving other decisions, registers and
  stack; a new match resets the held key;
- the input: process_first_person_mode's epilogue 0x2421B0 to its `ret 8`:
  while Tab is held in a Blackout match the action mask has no move and no
  act, an act held through Tab stays out until let go; the original and a
  match without send what is pressed.
"""
import struct
import sys

from unicorn import UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI

import patch_026g as base
import check_hide_ui_026g as hide

OPTIONS, WORK = 0x30000000, 0x30100000
OPTIONS_BLACKOUT, MAGIC = 0x57870, b"SRBO"
STATIC_START, STATIC_PAST = 0x27B917, 0x27B932       # process_static_match_info: before the pushes, past the site
REPLAY_START, REPLAY_RET = 0x1F094C, 0x1F0976        # the replay's chunk 1: its reader made, its `ret 8`
ICONS_SITE, ICONS_PAST, PLAYER_ICONS = 0x23B206, 0x23AA16, 0x23AA10
# The same site's other hooks' Invokes, which their own checks cover (patch 83, after this
# patch's in the chain: check_hud_options_026g.py).
OTHER_PATCHES_INVOKES = ("root.srp_hud_options",)
TAB_START, SHOW_PLAYER_LIST = 0x25CD26, 0x2384C0
KEY_ON, KEY_END = 0x25CD41, 0x25CF0D   # on_keyboard_action after its Tab test: the key's way (patch 58's), its end
INPUT_SITE, INPUT_MASK = 0x2421B0, 0x33C
OTHER_ICONS = 0x20076000               # another icons' movie (a new game world's)
NAME_SITE, NAME_PAST = 0x1F2F10, 0x1F2F33
PLACE_COUNT = 4
# the input's bits
FORWARD, BACK, LEFT, RIGHT, JUMP, FIRE, RELOAD, AIM = 0x1, 0x2, 0x4, 0x8, 0x10, 0x20, 0x40, 0x80
CROUCH, SPRINT, WEAPON_1, GRENADE, MELEE, SPOT = 0x100, 0x200, 0x1000, 0x800000, 0x8000000, 0x10000000
QUICK_1, USE = 0x4000, 0x200000


def empty_options(block=b""):
    """Twenty empty profiles, map 61, four places, the rest, then `block`."""
    out = bytearray()
    for _ in range(20):
        out += bytes(1) + bytes(1) + bytes(1) + struct.pack("<I", 10) + bytes(2)
    out += bytes([61, PLACE_COUNT]) + struct.pack("<IH", 0x80001234, 3)
    out += struct.pack("<28f", *[float(i) for i in range(28)]) + bytes(range(20))
    return bytes(out) + block


def srpx(*entries):
    return b"SRPX" + b"".join(bytes([tag, len(body)]) + body for tag, body in entries)


class Run(hide.Run):
    def __init__(self, data):
        super().__init__(data)
        for at, size in ((OPTIONS, 0x60000), (WORK, 0x10000), (0x30200000, 0x1000)):
            self.emu.mem_map(at, size, UC_PROT_ALL)
        self.refused, self.refusing = (), ()
        self.ticks = 0
        for slot, stub in ((0x870158, 0x30200000), (0x870150, 0x30200010)):
            self.word(slot, stub)
            self.emu.mem_write(stub, b"\xc2\x04\x00")
            self.emu.hook_add(hide.UC_HOOK_CODE, self.clock, begin=stub, end=stub)

    def clock(self, emu, address, *_):
        out = self.read(emu.reg_read(UC_X86_REG_ESP) + 4)
        emu.mem_write(out, struct.pack("<Q", self.ticks if address == 0x30200000 else 1000))
        emu.reg_write(UC_X86_REG_EAX, 1)


    def name_visible(self, local_team, player_team, normal, cached=0):
        """The original Alt/crosshair decision through the name's cached flag."""
        player, profile, place = WORK + 0x2000, WORK + 0x4000, 3
        self.word(player + 0x6B8, profile)
        self.word(profile + 0x45EC, player_team)
        self.emu.mem_write(player + 0x114, bytes([place]))
        row = hide.HUD + place * 24
        self.emu.mem_write(row + 0x387, bytes([cached, 0, 0]))
        sp = hide.STACK + 0x8000
        self.word(sp + 0x14, local_team)
        given = {**hide.GAME, UC_X86_REG_EAX: 0x12345600 | normal,
                 UC_X86_REG_ESI: player, UC_X86_REG_EDI: hide.HUD}
        regs, esp = self.run(NAME_SITE, (NAME_PAST,), sp, given)
        kept = esp == sp and self.stopped == NAME_PAST and regs[UC_X86_REG_ECX] == row \
            and regs[UC_X86_REG_EAX] & 0xFFFFFF00 == 0x12345600 \
            and all(regs[r] == v for r, v in given.items() if r not in (UC_X86_REG_EAX, UC_X86_REG_ECX))
        return self.read(row + 0x387) & 0xFF, self.read(row + 0x389) & 0xFF, kept

    # the harness's stubs; a movie that refuses a method (no root yet): its Invoke answers 0
    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        refused = address == hide.INVOKE and self.text(self.read(sp + 4)) in getattr(self, "refused", ()) \
            and emu.reg_read(UC_X86_REG_ECX) in getattr(self, "refusing", ())
        super().stub(emu, address, size, _)
        if refused:
            emu.reg_write(UC_X86_REG_EAX, 0)

    def options_payload(self, payload, stale):
        self.emu.mem_write(OPTIONS, bytes(0x578A0))
        self.emu.mem_write(OPTIONS + OPTIONS_BLACKOUT, stale)
        self.emu.mem_write(WORK, bytes(0x10000))
        self.emu.mem_write(WORK, payload)

    # process_static_match_info from its pushes of deserialize's arguments to past the site.
    def static_info(self, payload, stale=b"\x77" * 4):
        self.options_payload(payload, stale)
        world = hide.WORLD
        self.emu.mem_write(world, bytes(0x10000))
        self.word(world, world + 0x8000)                  # [edi]: a vtable
        self.word(world + 0x14, world + 0x1000)           # [edi+14h]+53A4h: the dictionary (none)
        ebp = hide.STACK + 0x9000
        for i, value in enumerate((WORK, WORK, len(payload))):
            self.word(ebp - 0x2C + 4 * i, value)
        sp = ebp - 0x100
        regs, esp = self.run(STATIC_START, (STATIC_PAST,), sp, {**hide.GAME, UC_X86_REG_EDI: world,
                                                               UC_X86_REG_ESI: OPTIONS - 0xC, UC_X86_REG_EBP: ebp})
        kept = regs[UC_X86_REG_ESI] == OPTIONS and regs[UC_X86_REG_EDI] == world and regs[UC_X86_REG_EBP] == ebp \
            and regs[UC_X86_REG_EDX] == world + 0x8000 and esp == sp
        return self.stopped == STATIC_PAST and kept, bytes(self.emu.mem_read(OPTIONS + OPTIONS_BLACKOUT, 4))

    # a replay's chunk 1 from its reader's making to its `ret 8`.
    def replay(self, payload, stale=b"\x77" * 4):
        self.options_payload(payload, stale)
        world = hide.WORLD
        self.emu.mem_write(world, bytes(0x10000))
        sp = hide.STACK + 0x8000
        self.emu.mem_write(sp, bytes(0x100))
        self.word(sp + 0x40, OPTIONS)
        self.word(sp + 0x44, 0)
        self.word(sp, 0x55555555)                         # the epilogue's saved esi and ebx
        self.word(sp + 4, 0x66666666)
        regs, esp = self.run(REPLAY_START, (REPLAY_RET,), sp, {**hide.GAME, UC_X86_REG_EAX: WORK,
                                                              UC_X86_REG_ECX: len(payload), UC_X86_REG_EDI: world})
        kept = esp == sp + 0x3C and regs[UC_X86_REG_ESI] == 0x55555555 and regs[UC_X86_REG_EBX] == 0x66666666 \
            and regs[UC_X86_REG_EDI] == world
        return self.stopped == REPLAY_RET and kept, bytes(self.emu.mem_read(OPTIONS + OPTIONS_BLACKOUT, 4))

    # update_ui's `push edi / call` of update_player_icons (0x23B206), the hud in edi, its icons'
    # movie `icons` (0: none), to past update_player_icons' prologue: the Invokes.
    def icons_fed(self, icons=hide.ICONS_MOVIE, others=False):
        """The site run as the game runs it: whether it went on as the game's,
        and the Invokes (without the other patches' of the same site unless
        `others`: patch 83's root.srp_hud_options, check_hud_options_026g.py's)."""
        self.movies(hide.HUD, 1, icons != 0)
        self.word(hide.ICONS_UI + 4, icons or hide.ICONS_MOVIE)
        sp = hide.STACK + 0x8000
        regs, esp = self.run(ICONS_SITE, (ICONS_PAST,), sp, {**hide.GAME, UC_X86_REG_EDI: hide.HUD})
        # past the prologue: [ebp] the caller's ebp, [ebp+4] the return, [ebp+8] the ui
        frame = regs[UC_X86_REG_EBP]
        back = self.read(frame + 4)
        direct = back == ICONS_SITE + 6
        # Research wraps this call, forwarding the same UI. Check both
        # frames and the inner call's target; its return is covered by the
        # Research check, which continues past the wrapper to update_ui.
        call = bytes(self.emu.mem_read(back - 5, 5))
        wrapped = call[0] == 0xE8 and back + struct.unpack("<i", call[1:])[0] == PLAYER_ICONS \
            and self.read(frame + 12) == ICONS_SITE + 6 and self.read(frame + 16) == hide.HUD
        kept = self.read(frame) == hide.GAME[UC_X86_REG_EBP] and (direct or wrapped) \
            and self.read(frame + 8) == hide.HUD and esp & 7 == 0 and esp <= frame \
            and all(regs[r] == v for r, v in {**hide.GAME, UC_X86_REG_EDI: hide.HUD}.items() if r != UC_X86_REG_EBP)
        invoked = [c for c in self.invokes() if others or c[1] not in OTHER_PATCHES_INVOKES]
        return self.stopped == ICONS_PAST and kept, invoked

    # on_keyboard_action from its Tab byte (al, 0x25CD26) through show_player_list (a stub) and the
    # game's Tab test: where the key goes, or None when the run was not as the game's.
    def tab(self, down):
        given = []

        def stub(emu, address, size, _):
            sp = emu.reg_read(UC_X86_REG_ESP)
            given.append(self.read(sp + 4) & 0xFF)
            emu.reg_write(UC_X86_REG_ESP, sp + 8)
            emu.reg_write(hide.UC_X86_REG_EIP, self.read(sp))
        handle = self.emu.hook_add(hide.UC_HOOK_CODE, stub, begin=SHOW_PLAYER_LIST, end=SHOW_PLAYER_LIST)
        sp = hide.STACK + 0x8000
        self.emu.mem_write(sp, bytes(0x40))
        regs, esp = self.run(TAB_START, (KEY_ON, KEY_END), sp, {**hide.GAME, UC_X86_REG_EAX: down,
                                                               UC_X86_REG_EDI: hide.HANDLER})
        self.emu.hook_del(handle)
        ok = esp == sp and given == [down] and regs[UC_X86_REG_ESI] == hide.HANDLER + 4 \
            and self.read(sp + 0x14) & 0xFF == down \
            and all(regs[r] == hide.GAME[r] for r in (UC_X86_REG_EBP,)) and regs[UC_X86_REG_EDI] == hide.HANDLER
        return self.stopped if ok else None

    # process_first_person_mode's epilogue (0x2421B0) on a handler whose mask is `mask`: the
    # mask after it, and whether it returned as the function (`ret 8`, edi esi ebx ebp popped).
    def input(self, mask, held=None):
        handler = hide.HANDLER
        self.emu.mem_write(handler, bytes(0x400))
        self.word(handler + INPUT_MASK, mask)
        # `held`: the state of the action `artefact` (23h, patch 78's carry key,
        # default H) this tick, pushed into the handler's event buffer (+98h)
        # by the game's own push_back (0x242580), as on_keyboard_action does.
        if held is not None:
            pair, sp = WORK + 0x6000, hide.STACK + 0x8000
            self.word(pair, 0x23)
            self.word(pair + 4, held)
            self.word(sp, hide.SENTINEL)
            self.run(0x242580, (hide.SENTINEL,), sp, {**hide.GAME, UC_X86_REG_ECX: handler + 0x98,
                                                      UC_X86_REG_EDX: pair})
            # The carry takes H's events where on_keyboard_action has its
            # action (site carry.key 0x23FE42: EDI the action, the key's state
            # at ebp+10h as input::enum_keyboard_action, 1 down / 2 up / 3 hold,
            # one more than the pushed state), down and up alike.
            frame = hide.STACK + 0xA000
            self.word(frame + 0x10, held + 1)
            self.run(0x23FE42, (0x23FE48,), sp, {**hide.GAME, UC_X86_REG_EDI: 0x23, UC_X86_REG_EBP: frame})
            assert self.stopped == 0x23FE48, hex(self.stopped)
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
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    on = srpx((2, b"\x01"))
    print("the options read (process_static_match_info 0x27B92D, a replay's chunk 1 0x1F0971)")
    cases = [
        ("no block", b"", False),
        ("Blackout", on, True),
        ("another entry (team damage's), then Blackout", srpx((1, bytes(12)), (2, b"\x01")), True),
        ("an unknown tag, then Blackout", srpx((9, b"xyz"), (2, b"\x01")), True),
        ("another entry alone", srpx((1, bytes(12))), False),
        ("Blackout 0", srpx((2, b"\x00")), False),
        ("Blackout of two bytes", srpx((2, b"\x01\x01")), False),
        ("a short block", on[:-1], False),
        ("another magic", b"SRPY" + bytes([2, 1, 1]), False),
    ]
    for name, block, want in cases:
        payload = empty_options(block)
        for where, run in (("0xE4", lambda r: r.static_info(payload)), ("a replay", lambda r: r.replay(payload))):
            a, b = run(stock), run(patched)
            ok = a == (True, b"\x77" * 4) and b == (True, MAGIC if want else bytes(4))
            report(ok, f"{name} ({where}): +57870h {b[1]!r}; the original's left; the game's registers and stack "
                       "as its code leaves them")

    print("the markers' feed (update_player_icons' start 0x23AA10)")
    told = lambda movie, flag, tab: [(movie, "root.force_interface_hidden", (flag,)),
                                     (movie, "root.force_interface_tab", (tab,))]
    hud = lambda flag: [(hide.HUD_MOVIE, "root.force_interface_hidden", (flag,))]
    M, OTHER = hide.ICONS_MOVIE, OTHER_ICONS
    frames = [
        # (what, the static info read first (None: as it is), Tab down (1), up (0) or as it is (None), the
        # icons' movie, it refuses, the Invokes wanted)
        ("a match without, its first frame", b"", None, M, False, hud(0) + told(M, 0, 0)),
        ("its next frame", None, None, M, False, []),
        ("Tab down in it", None, 1, M, False, told(M, 0, 1)),
        ("Tab up", None, 0, M, False, told(M, 0, 0)),
        ("a Blackout match, its first frame: the hud forced, no players' markers", on, None, M, False,
         hud(1) + told(M, 1, 0)),
        ("its next frame", None, None, M, False, []),
        ("Tab down: the players' markers", None, 1, M, False, told(M, 1, 1)),
        ("Tab still held", None, 1, M, False, []),
        ("Tab up: none again", None, 0, M, False, told(M, 1, 0)),
        ("another Blackout match reusing both movie addresses", on, None, M, False,
         hud(1) + told(M, 1, 0)),
        ("the hud holding another icons' movie (a new game world's)", None, None, OTHER, False, told(OTHER, 1, 0)),
        ("its next frame", None, None, OTHER, False, []),
        ("a Blackout match again, its icons movie without a root yet (Invoke answers 0)", on, None, M, True,
         hud(1) + told(M, 1, 0)[:1]),
        ("the next frame, the movie taking it", None, None, M, False, told(M, 1, 0)),
        ("the frame after", None, None, M, False, []),
        ("no icons' movie", None, None, 0, False, []),
        ("the next match without", b"", None, M, False, hud(0) + told(M, 0, 0)),
    ]
    for name, block, tab, icons, refuses, want in frames:
        if block is not None:
            patched.static_info(empty_options(block))
        if tab is not None:
            patched.tab(tab)
        patched.refused, patched.refusing = ((b"root.force_interface_hidden",), (icons,)) if refuses else ((), ())
        back, invoked = patched.icons_fed(icons)
        report(back and invoked == want,
               f"{name}: " + (", ".join(f"the {'hud' if w[0] == hide.HUD_MOVIE else 'icons'}' {w[1]}({w[2][0]})"
                                        for w in want) if want else "nothing invoked")
               + ", on into update_player_icons as the game's")
    patched.refused = ()
    back, invoked = stock.icons_fed()
    report(back and invoked == [], "the original: nothing invoked, on into update_player_icons")

    print("Tab (on_keyboard_action 0x25CD2C, the input builder's end 0x2421B0)")
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
        run.static_info(empty_options(on))
    for name, down, mask, want in steps:
        a_tab, b_tab = stock.tab(down), patched.tab(down)
        stock.ticks += 200
        patched.ticks += 200
        a, b = stock.input(mask), patched.input(mask)
        ok = a_tab == (KEY_END if down else KEY_ON) and b_tab == KEY_ON and a == (mask, True) and b == (want, True)
        report(ok, f"{name}: {b[0]:#x} sent (pressed {mask:#x}); the original sends what is pressed; "
                   "show_player_list given the byte; the key goes on (0x25cd41: the kits' keys 1 to 3)"
                   + (", the original's to the end (0x25cf0d: its Tab list)" if down else ""))
    patched.tab(1)
    patched.ticks += 200
    blocked = FORWARD | BACK | LEFT | RIGHT | SPRINT | JUMP | FIRE | RELOAD | AIM | 0xFC000 | 0x100000 \
        | USE | 0x400000 | GRENADE | MELEE
    b = patched.input(0xFFFFFFFF)
    blocked |= 1 << 29  # the carry chord's marker is produced only by a valid chord
    report(b == (0xFFFFFFFF & ~blocked, True), f"Tab down, every bit pressed: {b[0]:#x} sent, all but the moves "
           "and acts")
    patched.tab(0)
    patched.input(0)
    patched.static_info(empty_options(b""))
    where = patched.tab(1)
    b = patched.input(moving)
    report(b == (moving, True) and where == KEY_END, f"a match without Blackout, Tab down: {b[0]:#x} sent, all "
           "that is pressed; the key to the end (0x25cf0d) as the original's")
    patched.static_info(empty_options(on))
    b = patched.input(moving)
    report(b == (moving, True), "a new Blackout match's static info forgets a Tab of before: all that is pressed")

    print("Tab movement grace: immediate names, 200 ms before stopping movement")
    patched.static_info(empty_options(on))
    patched.ticks = 1000
    patched.tab(1)
    for now in (1000, 1199, 1200, 1250):
        patched.ticks = now
        patched.tab(1)
        visible, _, kept = patched.name_visible(0, 0, 0)
        expected = moving & ~(FIRE | AIM) if now < 1200 else CROUCH | WEAPON_1
        report(patched.input(moving) == (expected, True) and visible == 1 and kept,
               f"Tab at {now - 1000} ms: movement {'kept' if now < 1200 else 'blocked'}, names shown")
    patched.tab(0)
    report(patched.input(FORWARD | FIRE) == (FORWARD, True), "release restores movement; held fire stays suppressed")
    patched.ticks = 1300
    patched.tab(1)
    patched.ticks = 1399
    patched.tab(0)
    report(patched.input(FORWARD) == (FORWARD, True), "a short Tab tap never stops movement")
    patched.tab(1)
    patched.static_info(empty_options(on))
    patched.ticks += 500
    report(patched.input(moving) == (moving, True), "new match clears the grace and held actions")

    print("teammate names: the Alt/crosshair result, after visibility and projection")
    for blackout in (False, True):
        patched.static_info(empty_options(on if blackout else b""))
        for tab in (0, 1):
            patched.tab(tab)
            for local, team in ((0, 0), (1, 1), (0, 1), (1, 0), (2, 0), (2, 2), (3, 3)):
                for normal in (0, 1):
                    wanted = int(bool(normal or (blackout and tab and local in (0, 1) and local == team)))
                    for cached in (0, 1):
                        a = stock.name_visible(local, team, normal, cached)
                        b = patched.name_visible(local, team, normal, cached)
                        report(a == (normal, 0x20 if normal != cached else 0, True)
                               and b == (wanted, 0x20 if wanted != cached else 0, True),
                               f"Blackout={blackout}, Tab={tab}, teams={local}/{team}, native={normal}, "
                               f"cached={cached}: name={b[0]}, dirty={b[1]:#x}; registers/stack preserved")
    patched.static_info(empty_options(on))
    report(patched.name_visible(0, 0, 0, 1) == (0, 0x20, True),
           "a new match resets Tab: no stale forced teammate name")

    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
