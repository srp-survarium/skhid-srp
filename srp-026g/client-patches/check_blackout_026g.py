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
  until 0x2056F7. In a Blackout match a player of the options (places 0 to
  19) gets record 4: base_model ported/blackout/band_blue (team 0) or
  band_green (team 1), part_name band, the part 2 (the torso's), the records 5, and the
  name `srpbo0_` or `srpbo1_` more; a profile outside the options (the
  lobby's), inside them but not at a place's start, of team 2, a visual of
  20 records or a match without Blackout: nothing changed. Always edx
  [esp+58h], esi [ebp+0Ch], ebx edi ebp and the stack kept; the original
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
OPTIONS_BLACKOUT, MAGIC = 0x57870, b"SRBO"
PROFILE_SIZE, PROFILE_TEAM = 0x45FC, 0x45EC
RECORDS, RECORD_SIZE, COUNT = 0x234, 0xC0C, 0xF324
FRAME = 0x500                          # on_configs_loaded's frame below ebp (its esp is ebp - 5A4h and more)


class Run(hide.Run):
    def __init__(self, data):
        super().__init__(data)
        for at, size in ((OPTIONS, 0x60000), (WORK, 0x10000), (VISUAL, 0x10000), (LOBBY_PROFILE, 0x10000)):
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

    def string(self, at):
        begin, end, cap = struct.unpack("<III", self.emu.mem_read(at, 12))
        return bytes(self.emu.mem_read(begin, end - begin)), begin == at + 12, cap == begin + 0x104

    # on_configs_loaded at 0x2056F0: its esp frame and ebp frame made up.
    def configs_loaded(self, profile, count=4):
        emu = self.emu
        self.visual(count)
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
    asked = (hide.HUD_MOVIE, "root.interface_hidden", ())
    for name, block, blackout in (("a Blackout match", srpx((2, b"\x01")), True),
                                  ("the next, without", b"", False),
                                  ("a Blackout match again", srpx(team_damage, (2, b"\x01")), True)):
        patched.options(empty_options(block))
        r = patched.reset(1, 1)
        ok = r[0] == hide.RESET_BACK and r[1] == 16 and r[3] == [reset] + hides(1 if blackout else 0) \
            and r[2][UC_X86_REG_EBP] == hide.HUD and (r[2][UC_X86_REG_EAX] & 0xFF) == 1
        report(ok, f"{name}: root.reset, then root.hide_interface({int(blackout)}) to the hud and the icons, "
                   "back at 0x234aed with the arguments popped")
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

    print("the character (on_configs_loaded at 0x2056F0)")
    blue = (b"ported/blackout/band_blue", True, True)
    green = (b"ported/blackout/band_green", True, True)
    for name, block, place_team, profile, want in (
            ("place 0, team 0, Blackout", srpx((2, b"\x01")), (0, 0), OPTIONS, (blue, b"srpbo0_")),
            ("place 7, team 1, Blackout", srpx((2, b"\x01")), (7, 1), OPTIONS + 7 * PROFILE_SIZE,
             (green, b"srpbo1_")),
            ("place 19, team 0, Blackout", srpx((2, b"\x01")), (19, 0), OPTIONS + 19 * PROFILE_SIZE,
             (blue, b"srpbo0_")),
            ("the lobby's profile, Blackout", srpx((2, b"\x01")), (0, 0), LOBBY_PROFILE, None),
            ("inside the options but not a place", srpx((2, b"\x01")), (0, 0), OPTIONS + 0x100, None),
            ("place 3, team 2, Blackout", srpx((2, b"\x01")), (3, 2), OPTIONS + 3 * PROFILE_SIZE, None),
            ("place 0, no Blackout", srpx(team_damage), (0, 0), OPTIONS, None)):
        for run in (stock, patched):
            run.options(empty_options(block))
            run.team(*place_team)
        a, b = stock.configs_loaded(profile), patched.configs_loaded(profile)
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
    patched.options(empty_options(srpx((2, b"\x01"))))
    patched.team(0, 1)
    b = patched.configs_loaded(OPTIONS, count=20)
    report(b["at"] == VISUAL_BACK and b["records"] == 20 and b["name"] == b"12_0_7_",
           "a visual of 20 records in a Blackout match: nothing added")

    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
