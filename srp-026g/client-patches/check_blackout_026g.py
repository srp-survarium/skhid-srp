#!/usr/bin/env python3
"""Run patch 65 (blackout_026g.asm and .c: Blackout, a battery match with no
interface and a team's band on every character) in the default build on
made-up game objects.

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
  each static info: after a Blackout one the reset, then
  root.hide_interface(true) to the hud and the icons (the interface forced
  hidden), and F11 (on_keyboard_action's case 19h, 0x25CED1) then invokes
  nothing and goes on at 0x25CF0D, or at 0x25CEDB with a replay's movie; after
  any other the reset, then root.hide_interface(false), and F11 hides as
  patch 62 (the next match is free again);
- the character: profile_skin_visual_cook::on_configs_loaded at 0x2056F0
  (the parts and the head done) on a visual the game's own constructor
  (0x127C30) made, with four part records in use, a frame with the visual at
  [esp+1Ch], the name's buffer at [esp+348h] and the profile at [ebp+10h],
  until 0x2056F7. The band is the character's own profile's (its protection
  field, srp's: 11 blue, 12 red): every way a character is built (a match's
  start, before and after the world has its match; a late joiner's own copy
  of his profile; a respawn; a replay, whose flag was never read) gives the
  same record 4: base_model ported/blackout/<the torso, '/' as '_'>_blue or
  _red, part_name band, the part 2, the records 5, and `srpbo0_` or
  `srpbo1_` after the name; 10 (the lobby, a match without Blackout), 13,
  an unknown torso or none, a visual of 20 records: nothing changed. Always
  edx [esp+58h], esi [ebp+0Ch], ebx edi
  ebp and the stack kept; the original
  changes nothing.
"""
import struct
import sys

from unicorn import UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ESP, UC_X86_REG_EBP, \
    UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EDX

import patch_026g as base
import check_hide_ui_026g as hide
from check_team_damage_026g import empty_options, srpx, numbers

DESERIALIZE = 0x1F0E70
VISUAL_SITE, VISUAL_BACK = 0x2056F0, 0x2056F7
VISUAL_CONSTRUCTOR = 0x127C30          # skeleton_combined_cook_data's (esi the data, a byte pushed; ret 4)
OPTIONS, WORK, VISUAL, LOBBY_PROFILE = 0x30000000, 0x30100000, 0x30200000, 0x30300000
GAME, MATCH = 0x30400000, 0x30480000           # a made-up game (its game_world at +18h) and match
VISUAL_COOK_GAME, WORLD_MATCH = 0x9262320, 0x18 + 0x4D4C
LATE_PROFILE = LOBBY_PROFILE + 0x8000          # a late joiner's profile: its context's own copy
PROFILE_BAND = 0x45F4                          # the profile's protection: srp's band (11 blue, 12 red)
PROFILE_LOCAL = 0x45F0                         # the profile's is_local byte: the game's own player
OPTIONS_BLACKOUT, MAGIC = 0x57870, b"SRBO"
PROFILE_SIZE, PROFILE_TEAM = 0x45FC, 0x45EC
RECORDS, RECORD_SIZE, COUNT = 0x234, 0xC0C, 0xF324
FRAME = 0x500                          # on_configs_loaded's frame below ebp (its esp is ebp - 5A4h and more)
TORSO = b"character/human/edge/edge_hunter_01_torso_s1"
ICONS_SITE, ICONS_BACK, PLAYER_ICONS = 0x23B206, 0x23B20C, 0x23AA10
TAB_SITE, TAB_BACK, SHOW_PLAYER_LIST = 0x25CD34, 0x25CD39, 0x2384C0
INPUT_SITE, INPUT_MASK = 0x2421B0, 0x33C
# the input's bits (srp's drive.rs::key)
FORWARD, BACK, LEFT, RIGHT, JUMP, FIRE, RELOAD, AIM = 0x1, 0x2, 0x4, 0x8, 0x10, 0x20, 0x40, 0x80
CROUCH, SPRINT, WEAPON_1, GRENADE, MELEE, SPOT = 0x100, 0x200, 0x1000, 0x800000, 0x8000000, 0x10000000
QUICK_1, USE = 0x4000, 0x200000


class Run(hide.Run):
    def __init__(self, data):
        super().__init__(data)
        self.linked = None
        for at, size in ((OPTIONS, 0x60000), (WORK, 0x10000), (VISUAL, 0x10000), (LOBBY_PROFILE, 0x10000),
                         (GAME, 0x10000), (MATCH, 0x1000)):
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

    # A visual made by the game's constructor, `count` records in use.
    def visual(self, count):
        emu = self.emu
        emu.mem_write(VISUAL, bytes(0x10000))
        sp = hide.STACK + 0x8000 - 8
        self.word(sp, hide.SENTINEL)
        self.word(sp + 4, 0)
        self.run(VISUAL_CONSTRUCTOR, (hide.SENTINEL,), sp, {**hide.GAME, UC_X86_REG_ESI: VISUAL})
        emu.mem_write(VISUAL + COUNT, bytes([count]))

    # update_ui's `push edi / call` of update_player_icons (0x23B206), a made-up ui in edi:
    # how many times 0x23AA10 ran (a stub: it pops the ui), back at 0x23B20C, the registers.
    def icons_fed(self):
        emu = self.emu
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
        return self.fed, self.stopped == ICONS_BACK and esp == sp, kept

    # game_world::on_keyboard_action's call of show_player_list (0x25CD34), the byte pushed:
    # back at 0x25CD39 with it popped, show_player_list (a stub) given the byte.
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
        regs, esp = self.run(TAB_SITE, (TAB_BACK,), sp, {**hide.GAME, UC_X86_REG_ESI: hide.HUD})
        emu.hook_del(handle)
        return self.stopped == TAB_BACK and esp == sp + 4 and given == [down] \
            and all(regs[r] == v for r, v in hide.GAME.items() if r != UC_X86_REG_ESI)

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

    def string(self, at):
        begin, end, cap = struct.unpack("<III", self.emu.mem_read(at, 12))
        return bytes(self.emu.mem_read(begin, end - begin)), begin == at + 12, cap == begin + 0x104

    # on_configs_loaded at 0x2056F0: its esp frame and ebp frame made up.
    # Record `index` of the visual made the part `kind` of model `model` (its text in its own buffer).
    def record(self, index, kind, model):
        at = VISUAL + RECORDS + index * RECORD_SIZE
        begin = self.read(at)
        self.emu.mem_write(begin, model + b"\0")
        self.word(at + 4, begin + len(model))
        self.word(at + 0xC08, kind)

    # The game the visual cook holds (+20h, 0x9262320), its game world's match or none (the lobby).
    def in_match(self, match):
        self.word(VISUAL_COOK_GAME, GAME)
        self.word(GAME + WORLD_MATCH, MATCH if match else 0)

    def configs_loaded(self, profile, count=4, torso=TORSO):
        emu = self.emu
        self.visual(count)
        # record 1 the gloves, record 2 the torso (as the slot table's order puts them)
        self.record(1, 1, b"character/human/edge/edge_hunter_01_hand")
        if torso is not None:
            self.record(2, 2, torso)
        ebp = hide.STACK + 0x9000
        sp = ebp - FRAME - 0x5A4 + 0x100
        emu.mem_write(sp, bytes(0x1000))
        emu.mem_write(ebp, bytes(0x40))
        self.word(sp + 0x1C, VISUAL)
        self.word(sp + 0x58, 0x58585858)
        name = sp + 0x354
        emu.mem_write(name, b"12_0_7_\0")
        for i, value in enumerate((name, name + 7, sp + 0x458)):
            self.word(sp + 0x348 + 4 * i, value)
        self.word(ebp + 0x0C, 0x0C0C0C0C)
        self.word(ebp + 0x10, profile)
        before = self.read(sp, 0x340) + self.read(sp + 0x400, 0x400)
        regs, esp = self.run(VISUAL_SITE, (VISUAL_BACK,), sp, {**hide.GAME, UC_X86_REG_EBP: ebp})
        records = self.read(VISUAL + COUNT, 1)[0]
        last = VISUAL + RECORDS + (records - 1) * RECORD_SIZE
        out = {
            "at": self.stopped, "esp": esp == sp, "records": records,
            "model": self.string(last), "part": self.string(last + 0x114),
            "kind": self.read(last + 0xC08),
            "name": bytes(emu.mem_read(name, 32)).split(b"\0")[0], "cursor": self.read(sp + 0x34C) - name,
            "edx": regs[UC_X86_REG_EDX], "esi": regs[UC_X86_REG_ESI],
            "kept": all(regs[r] == hide.GAME[r] for r in (UC_X86_REG_EBX, UC_X86_REG_EDI))
            and regs[UC_X86_REG_EBP] == ebp,
            "stack": self.read(sp, 0x340) + self.read(sp + 0x400, 0x400) == before,
        }
        return out


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
                           (hide.ICONS_MOVIE, "root.hide_interface", (flag,))]
    asked = (hide.HUD_MOVIE, "root.interface_hidden", ())
    for name, block, blackout in (("a Blackout match", srpx((2, b"\x01")), True),
                                  ("the next, without", b"", False),
                                  ("a Blackout match again", srpx(team_damage, (2, b"\x01")), True)):
        patched.options(empty_options(block))
        r = patched.reset(1, 1)
        ok = r[0] == hide.RESET_BACK and r[1] == 16 and r[3] == [reset] + forces(1 if blackout else 0) \
            and r[2][UC_X86_REG_EBP] == hide.HUD and (r[2][UC_X86_REG_EAX] & 0xFF) == 1
        report(ok, f"{name}: root.reset, then the hud's root.force_interface_hidden({int(blackout)}) (its "
                   f"countdown kept, HUD.as) and the icons' root.hide_interface({int(blackout)}), back at "
                   "0x234aed with the arguments popped")
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

    print("the character (on_configs_loaded at 0x2056F0): the band is the profile's own field")
    band = lambda colour: (b"ported/blackout/character_human_edge_edge_hunter_01_torso_s1_" + colour, True, True)
    exo = b"ported/duty_026/duty_03_torso_s1"
    on, off = srpx((2, b"\x01")), b""
    blue, red, none = 11, 12, 10
    # (what, the static info read last, the game world's match, where the profile is, its field, the torso,
    # the band and name wanted). Every way a character is built drives its own profile through the same
    # hook, whatever the flag or the world: so they cannot differ.
    paths = (
        ("a match's start: an options' profile, blue", on, 1, OPTIONS, blue, TORSO, (band(b"blue"), b"srpbo0_")),
        ("a match's start, before the world has its match (the owner's round 5 build failed here)", on, 0,
         OPTIONS + 7 * PROFILE_SIZE, red, TORSO, (band(b"red"), b"srpbo1_")),
        ("a late joiner seen by the others: his context's own copy of his profile, red", on, 1, LATE_PROFILE, red,
         TORSO, (band(b"red"), b"srpbo1_")),
        ("a respawn: the same profile built again", on, 1, LATE_PROFILE, red, TORSO, (band(b"red"), b"srpbo1_")),
        ("a replay: its options' profile, the flag never read", off, 0, OPTIONS + 19 * PROFILE_SIZE, blue, exo,
         ((b"ported/blackout/ported_duty_026_duty_03_torso_s1_blue", True, True), b"srpbo0_")),
        ("the lobby: its profile says 10", on, 0, LOBBY_PROFILE, none, TORSO, None),
        ("a match without Blackout: 10", off, 1, OPTIONS, none, TORSO, None),
        ("another value (13)", on, 1, OPTIONS, 13, TORSO, None),
        ("a torso the bands pack has none for", on, 1, OPTIONS, blue, b"character/human/edge/unknown_torso_s1",
         None),
        ("no torso record", on, 1, OPTIONS, blue, None, None),
        # the game's own player (is_local, set by the static info's reader): no band of his own, seen in
        # first person it is a patch at the camera; the others' games draw it (their copy is not local)
        ("the game's own player (is_local), blue", on, 1, OPTIONS + 3 * PROFILE_SIZE, blue, TORSO, None),
    )
    for name, block, match, profile, field, torso, want in paths:
        for run in (stock, patched):
            run.options(empty_options(block))
            run.word(profile + PROFILE_BAND, field)
            run.emu.mem_write(profile + PROFILE_LOCAL, bytes([1 if "own player" in name else 0]))
            run.in_match(match)
        a, b = stock.configs_loaded(profile, torso=torso), patched.configs_loaded(profile, torso=torso)
        same = a["at"] == b["at"] == VISUAL_BACK and a["esp"] and b["esp"] and a["kept"] and b["kept"] \
            and a["stack"] and b["stack"] and a["edx"] == b["edx"] == 0x58585858 \
            and a["esi"] == b["esi"] == 0x0C0C0C0C and a["records"] == 4 and a["name"] == b"12_0_7_"
        if want is None:
            ok = same and b["records"] == 4 and b["name"] == b"12_0_7_" and b["cursor"] == 7
            what = "nothing changed"
        else:
            ok = same and b["records"] == 5 and b["model"] == want[0] and b["part"] == (b"band", True, True) \
                and b["kind"] == 2 and b["name"] == b"12_0_7_" + want[1] and b["cursor"] == 7 + len(want[1])
            what = f"record 4 {b['model'][0].decode()} / band, part 2, the name {b['name'].decode()}"
        report(ok, f"{name}: {what}; edx esi as the two instructions leave them, ebx edi ebp and the frame kept; "
                   "the original changes nothing")
    patched.word(OPTIONS + PROFILE_BAND, red)
    b = patched.configs_loaded(OPTIONS, count=20)
    report(b["at"] == VISUAL_BACK and b["records"] == 20 and b["name"] == b"12_0_7_",
           "a visual of 20 records with a band: nothing added")

    print("the markers' movie's feed (update_ui's update_player_icons, 0x23B207)")
    for name, block, fed in (("a Blackout match", on, 0), ("a match without", b"", 1)):
        for run, want in ((stock, 1), (patched, fed)):
            run.options(empty_options(block))
            calls, back, kept = run.icons_fed()
            report(back and kept and calls == want,
                   f"{name}, {'the build' if run is patched else 'the original'}: update_player_icons run "
                   f"{calls} time(s) (wanted {want}), back at 0x23b20c with the ui popped, the registers kept")

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
        ok = a_tab and b_tab and a == (mask, True) and b == (want, True)
        report(ok, f"{name}: {b[0]:#x} sent (pressed {mask:#x}); the original sends what is pressed; "
                   "show_player_list given the byte, the builder returns as itself")
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
    patched.tab(1)
    b = patched.input(moving)
    report(b == (moving, True), f"a match without Blackout, Tab down: {b[0]:#x} sent, all that is pressed")
    patched.options(empty_options(srpx((2, b"\x01"))))
    b = patched.input(moving)
    report(b == (moving, True), "a new Blackout match's static info forgets a Tab of before: all that is pressed")

    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
