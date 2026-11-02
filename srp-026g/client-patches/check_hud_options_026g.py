#!/usr/bin/env python3
"""Run patch 83 (rust/patches/src/hud_options.rs, its lohooks of
hud_options.toml: the Settings' "Hide hit markers" and "Hide player scores", kept in
user.cfg) in the default build on made-up game objects, each from the game's
own code before its site to past it, the original beside it.

    python check_hud_options_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them; check_language_026g.py's harness (check_hide_ui_026g.py's:
Scaleform's Invoke 0x511600 a stub that records each call; translate_text
0x1FF0F0 the key's English text; game::save_user_config 0x1E9030 counted):

- on_configs_loaded's start (0x1EA210 to 0x1EA216, the chain: patch 79's
  hook, then this one): the console variables `srp_hide_hit_markers` and
  `srp_hide_scores` registered once, after patch 79's `localization`, with
  the game's own cc_value<bool>::cc_value (a cc_bool, saved, kind 2: user.cfg's
  lines set them), off; the game's own cc_bool::execute sets each from a
  user.cfg line's value; registers and stack as the original's;
- the Settings' call (game_options::scaleform_callback 0x1FF5D0 to its
  return, through patch 79's hook at its start): a setting's name with "1" or
  "0" sets that variable alone and writes user.cfg once, with another value
  or none sets nothing, and each answers root.srp_check_option(name, label,
  "1" or "0") and returns as the function does (ret 10h); srp_language still
  reaches patch 79; the original answers nothing;
- each frame's player icons update (update_ui's `push edi / call 0x23AA10`
  at 0x23B206 to update_player_icons' prologue's end 0x23AA16, through patch
  65's hook): the hud movie told root.srp_hud_options(hide hit markers, hide
  scores), after patch 65's root.force_interface_hidden, exactly at a change
  of the hud movie or of a setting, and again at the next frame while the
  movie refuses it (Invoke answering 0), and again after a match's or a
  replay's options read (a new match's hud can be made at the last one's
  address; process_static_match_info 0x27B92D, a replay's chunk 1 0x1F0971,
  after patch 65's hooks); nothing without a hud movie;
  registers and stack as the original's, which invokes nothing.
"""
import struct
import sys

from unicorn import UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EBP, UC_X86_REG_EDI, UC_X86_REG_ESI, \
    UC_X86_REG_ESP

import patch_026g as base
import check_hide_ui_026g as hide
import check_blackout_026g as blackout
import check_language_026g as language

EXECUTE_BOOL, CC_BOOL_VTABLE = 0xECA30, 0x9E7C4C
SETTINGS = {"srp_hide_hit_markers": "Hide hit markers", "srp_hide_scores": "Hide player scores",
            "srp_hide_crosshair": "Hidden crosshair"}
# the group's own texts, in root.srp_hud_texts' order
TEXTS = {"st_srp_options_hud": "HUD:", "st_srp_hide_quests_option": "Hide quests", "st_srp_crosshair_dynamic": "Dynamic crosshair",
         "st_srp_crosshair_static": "Static crosshair", "st_srp_crosshair_hidden": "Hidden crosshair",
         "st_srp_static_minimap_option": "Static minimap"}
language.ENGLISH.update({key: text.encode() for key, text in TEXTS.items()})
language.ENGLISH.update({"st_srp_hide_hit_markers_option": b"Hide hit markers", "st_srp_hide_scores_option": b"Hide player scores"})
OTHER_HUD = 0x20076800                  # another hud movie (a new game world's)
# a match's options and the message they are read from (check_blackout_026g.py's OPTIONS and WORK
# lie in check_language_026g.py's DATA: these are their own)
MATCH_OPTIONS, MATCH_WORK = 0x31000000, 0x31100000


class Run(language.Run):
    def __init__(self, data):
        super().__init__(data)
        self.refused = False
        self.emu.mem_map(MATCH_OPTIONS, 0x60000, UC_PROT_ALL)
        self.emu.mem_map(MATCH_WORK, 0x10000, UC_PROT_ALL)

    def new_match(self, replay):
        """A match's options read, as check_blackout_026g.py's static_info and replay run
        them (process_static_match_info from its pushes to past the site; a replay's chunk 1
        from its reader's making to its `ret 8`), a match without Blackout: whether it went
        on as the game's."""
        payload = blackout.empty_options()
        self.emu.mem_write(MATCH_OPTIONS, bytes(0x578A0))
        self.emu.mem_write(MATCH_WORK, bytes(0x10000))
        self.emu.mem_write(MATCH_WORK, payload)
        world = hide.WORLD
        self.emu.mem_write(world, bytes(0x10000))
        if replay:
            sp = hide.STACK + 0x8000
            self.emu.mem_write(sp, bytes(0x100))
            self.word(sp + 0x40, MATCH_OPTIONS)
            self.word(sp, 0x55555555)
            self.word(sp + 4, 0x66666666)
            regs, esp = self.run(blackout.REPLAY_START, (blackout.REPLAY_RET,), sp,
                                 {**hide.GAME, UC_X86_REG_EAX: MATCH_WORK, UC_X86_REG_ECX: len(payload),
                                  UC_X86_REG_EDI: world})
            return self.stopped == blackout.REPLAY_RET and esp == sp + 0x3C and regs[UC_X86_REG_EDI] == world
        self.word(world, world + 0x8000)                  # [edi]: a vtable
        self.word(world + 0x14, world + 0x1000)           # [edi+14h]+53A4h: the dictionary (none)
        ebp = hide.STACK + 0x9000
        for i, value in enumerate((MATCH_WORK, MATCH_WORK, len(payload))):
            self.word(ebp - 0x2C + 4 * i, value)
        sp = ebp - 0x100
        regs, esp = self.run(blackout.STATIC_START, (blackout.STATIC_PAST,), sp,
                             {**hide.GAME, UC_X86_REG_EDI: world, UC_X86_REG_ESI: MATCH_OPTIONS - 0xC,
                              UC_X86_REG_EBP: ebp})
        return self.stopped == blackout.STATIC_PAST and esp == sp and regs[UC_X86_REG_ESI] == MATCH_OPTIONS

    # a hud movie that refuses root.srp_hud_options (no root yet): its Invoke answers 0
    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        refused = self.refused and address == hide.INVOKE and self.text(self.read(sp + 4)) == b"root.srp_hud_options"
        super().stub(emu, address, size, _)
        if refused:
            emu.reg_write(UC_X86_REG_EAX, 0)

    def frame(self, movie=hide.HUD_MOVIE):
        """update_ui's call of update_player_icons to past its prologue, the hud
        at hide.HUD with `movie` (0: none) and the icons' movie: the registers
        and the stack past the prologue as the game's (check_blackout_026g.py's
        icons_fed: called directly or through Research's wrapper, which
        forwards the same ui), and the Invokes."""
        self.movies(hide.HUD, movie != 0, True)
        self.word(hide.HUD_UI + 4, movie or hide.HUD_MOVIE)
        sp = hide.STACK + 0x8000
        given = {**hide.GAME, UC_X86_REG_EDI: hide.HUD}
        regs, esp = self.run(blackout.ICONS_SITE, (blackout.ICONS_PAST,), sp, given)
        frame = regs[UC_X86_REG_EBP]
        back = self.read(frame + 4)
        call = bytes(self.emu.mem_read(back - 5, 5))
        wrapped = call[0] == 0xE8 and back + struct.unpack("<i", call[1:])[0] == blackout.PLAYER_ICONS \
            and self.read(frame + 12) == blackout.ICONS_SITE + 6 and self.read(frame + 16) == hide.HUD
        kept = self.stopped == blackout.ICONS_PAST and self.read(frame) == hide.GAME[UC_X86_REG_EBP] \
            and (back == blackout.ICONS_SITE + 6 or wrapped) and self.read(frame + 8) == hide.HUD \
            and esp & 7 == 0 and esp <= frame and all(regs[r] == v for r, v in given.items() if r != UC_X86_REG_EBP)
        return kept, self.invokes()


def registered(r, head, before):
    return language.registered(r, head, before)


def console_bool(r, node):
    """The registered variable: (name, kind, saved byte, its value's address), its vtable checked."""
    assert r.read(node) == CC_BOOL_VTABLE, hex(r.read(node))
    return r.text(r.read(node + 0x10)).decode(), r.read(node + 0x14), r.read(node + 0x1C, 2)[1], r.read(node + 0x40)


def main():
    original = open(sys.argv[1], "rb").read()
    patched, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None,
                            open(base.late_join_bin(), "rb").read())
    stock, ours = Run(original), Run(patched)
    expect = lambda ok, text: print("ok", text) if ok else sys.exit("FAIL " + text)

    # on_configs_loaded's start: patch 79's variable, then these, once
    head_before = ours.read(language.LIST_HEAD)
    a_regs, a_sp, a_head = stock.start()
    b_regs, b_sp, b_head = ours.start()
    # patch 81's `srp_lobby` and `srp_room_rain`, a later member of the site's chain, register after these
    lobby_scene = (b"srp_lobby", b"srp_room_rain")
    nodes = [node for node in registered(ours, b_head, head_before) if ours.text(ours.read(node + 0x10)) not in lobby_scene]
    names = [ours.text(ours.read(node + 0x10)).decode() for node in nodes]
    variables = {name: console_bool(ours, node) for name, node in zip(names, nodes) if name in SETTINGS}
    expect(a_regs == b_regs and a_sp == b_sp and a_head == head_before
           and names == ["s_music_volume", "srp_hide_crosshair", "srp_hide_scores", "srp_hide_hit_markers", "localization"]
           and all((kind, saved) == (2, 1) and ours.read(value, 1) == b"\0" for _, kind, saved, value in variables.values()),
           "configs.start: registers and stack as the original's; `srp_hide_hit_markers` and `srp_hide_scores`, "
           "saved bools of kind 2, off, registered after patch 79's `localization` (the original registers nothing)")
    _, _, again = ours.start()
    again_nodes = [node for node in registered(ours, again, head_before) if ours.text(ours.read(node + 0x10)) not in lobby_scene]
    expect(again_nodes == nodes, "configs.start again: each registered once")
    value = {name: variables[name][3] for name in SETTINGS}
    for name, node in zip(names, nodes):
        if name not in SETTINGS:
            continue
        for line, want in ((b"on", b"\x01"), (b"off", b"\0")):
            ours.emu.mem_write(language.TEXT, line + b"\0")
            ours.call_at(EXECUTE_BOOL, (language.TEXT,), {UC_X86_REG_ECX: node})
            expect(ours.read(value[name], 1) == want, f"user.cfg's line `{name} {line.decode()}` sets it "
                   "(cc_bool::execute 0xECA30)")

    # the Settings' call
    state = lambda: {name: ours.read(value[name], 1) == b"\x01" for name in SETTINGS}
    for name, label in SETTINGS.items():
        others = [n for n in SETTINGS if n != name]
        for case, argument, on, saves in [
            ("\"1\"", "1", True, 1),
            ("\"0\"", "0", False, 1),
            ("\"1\" again", "1", True, 1),
            ("another value", "on", True, 0),
            ("no argument", None, True, 0),
        ]:
            before = state()
            invokes, count = ours.callback(name, argument)
            after = state()
            expect(invokes == [(language.MOVIE, "root.srp_check_option", (name, label, "1" if on else "0"))]
                   and count == saves and after[name] == on and all(after[n] == before[n] for n in others),
                   f"{name} with {case}: it is {int(on)} (the others unchanged), user.cfg written {saves} time(s), "
                   f"the movie answered with its name, label and {int(on)}")
            stock_invokes, stock_saves = stock.callback(name, argument)
            expect(stock_invokes == [] and stock_saves == 0, f"the original ignores {name} ({case})")
        ours.callback(name, "0")
    before = state()
    invokes, count = ours.callback("srp_hud_texts")
    expect(invokes == [(language.MOVIE, "root.srp_hud_texts", tuple(TEXTS.values()))] and count == 0 and state() == before,
           "srp_hud_texts: the group's texts (heading, Hide quests, the crosshair list's three choices, Static minimap), "
           "nothing set or saved")
    stock_invokes, _ = stock.callback("srp_hud_texts")
    expect(stock_invokes == [], "the original ignores srp_hud_texts")
    invokes, count = ours.callback("srp_language")
    expect([m for _, m, _ in invokes] == ["root.srp_language"] and count == 0,
           "srp_language still reaches patch 79 (its hook before this one)")
    for r, which in ((stock, "the original"), (ours, "the build")):
        invokes, count = r.callback("srp_hide")
        expect(invokes == [] and count == 0, f"{which}: another method returns, nothing invoked or saved")

    # each frame: the hud told the settings at a change
    told = lambda movie, hits, scores, crosshair=0: [(movie, "root.srp_hud_options", (hits, scores, crosshair))]
    frames = [
        # (what, the Settings' calls made first (None: a new match's options read, "replay" a replay's),
        # the hud movie (0 none), it refuses, the hud's Invokes wanted)
        ("the first frame, both off", [], hide.HUD_MOVIE, False, told(hide.HUD_MOVIE, 0, 0)),
        ("the next frame", [], hide.HUD_MOVIE, False, []),
        ("Hide hit markers checked", [("srp_hide_hit_markers", "1")], hide.HUD_MOVIE, False,
         told(hide.HUD_MOVIE, 1, 0)),
        ("the next frame", [], hide.HUD_MOVIE, False, []),
        ("Hide player scores checked too", [("srp_hide_scores", "1")], hide.HUD_MOVIE, False, told(hide.HUD_MOVIE, 1, 1)),
        ("a Settings answer that changes nothing", [("srp_hide_scores", None)], hide.HUD_MOVIE, False, []),
        ("Hide hit markers unchecked", [("srp_hide_hit_markers", "0")], hide.HUD_MOVIE, False,
         told(hide.HUD_MOVIE, 0, 1)),
        ("another hud movie (a new game world's)", [], OTHER_HUD, False, told(OTHER_HUD, 0, 1)),
        ("its next frame", [], OTHER_HUD, False, []),
        ("no hud movie", [("srp_hide_scores", "0")], 0, False, []),
        ("Hidden crosshair chosen (another hud movie)", [("srp_hide_crosshair", "1")], OTHER_HUD, False,
         told(OTHER_HUD, 0, 0, 1)),
        ("Static crosshair chosen again", [("srp_hide_crosshair", "0")], OTHER_HUD, False, told(OTHER_HUD, 0, 0, 0)),
        ("the hud back, refusing (no root yet)", [], hide.HUD_MOVIE, True, told(hide.HUD_MOVIE, 0, 0)),
        ("the next frame, the movie taking it", [], hide.HUD_MOVIE, False, told(hide.HUD_MOVIE, 0, 0)),
        ("the frame after", [], hide.HUD_MOVIE, False, []),
        ("Hide hit markers checked", [("srp_hide_hit_markers", "1")], hide.HUD_MOVIE, False,
         told(hide.HUD_MOVIE, 1, 0)),
        ("a new match, its hud at the last one's address (a new root)", [None], hide.HUD_MOVIE, False,
         told(hide.HUD_MOVIE, 1, 0)),
        ("its next frame", [], hide.HUD_MOVIE, False, []),
        ("a replay, its hud at the same address", ["replay"], hide.HUD_MOVIE, False, told(hide.HUD_MOVIE, 1, 0)),
        ("its next frame", [], hide.HUD_MOVIE, False, []),
    ]
    for case, calls, movie, refuses, want in frames:
        for call in calls:
            if call is None or call == "replay":
                for r in (stock, ours):
                    expect(r.new_match(call == "replay"), f"{case}: the options read goes on as the game's")
                continue
            ours.callback(*call)
        ours.refused = refuses
        a_kept, a_invoked = stock.frame(movie)
        b_kept, b_invoked = ours.frame(movie)
        mine = [c for c in b_invoked if c[1] == "root.srp_hud_options"]
        order = [c[1] for c in b_invoked if c[0] == (movie or -1)]
        expect(a_kept and b_kept and a_invoked == [] and mine == want
               and (not mine or order.index("root.srp_hud_options") == len(order) - 1),
               f"{case}: " + (f"the hud told {want[0][2]}" if want else "nothing told")
               + ", after patch 65's Invokes; on into update_player_icons as the game's")
    print("all cases passed")


if __name__ == "__main__":
    main()
