#!/usr/bin/env python3
"""Run patch 81 (rust/patches/src/lobby_scene.rs, its lohooks of
lobby_scene.toml: players choose the lobby's scene, a setting kept in
user.cfg; docs/notes/lobby-scenes.md) in the default build on made-up game
objects, each from the game's own code before its site to past it, the
original beside it.

    python check_lobby_scene_026g.py <original survarium.exe>

Requires Python `unicorn`. check_language_026g.py's harness (patch 79 shares
two sites with it: configs.start and options.callback, a chain).

- on_configs_loaded's start: `srp_lobby_scene`, a saved string of 32 bytes,
  registered once with the game's own cc_string::cc_string, after patch 79's
  `localization`; the game's cc_string::execute sets it from a user.cfg line;
- lobby_menu::query_resources (0x21E450) from its start to its query
  (0x21E5ED): the fourth request is the original's ({"lobby_scene/day", 40h},
  the lobby menu's variant as its user data) for no choice, the classic or an
  unknown id; for another lobby the lobby's config first
  ({"resources/projects/lobby_scene/<id>/client_project", 1Fh}, no user
  data), the other seven requests and their user data the original's;
- lobby_menu::on_render_scenes_ready (0x220B20): after a probe the patch asks
  the query again (query_resources with the lobby menu) and returns (ret 4)
  without the original's code, then asks the lobby's project, or the
  classic's when the config was not there; a project without its camera
  track is asked again as the classic; a usable project, or the classic's,
  goes on into the original's code (its span replayed);
- the Settings' call (game_options::scaleform_callback): `srp_lobby_scene`
  with a lobby's id saves it (user.cfg written once), with another or none
  saves nothing, and both answer root.srp_lobby_scene(label, note, saved,
  shown, then every lobby's id and name); patch 79's `srp_language` and an
  unknown method are as patch 79's check says (check_language_026g.py).
"""
import struct
import sys

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_EIP

import patch_026g as base
import check_hide_ui_026g as hide
import check_language_026g as language

QUERY, QUERY_CALL = 0x21E450, 0x21E5ED
READY, READY_REPLAYED = 0x220B20, 0x220B26
MENU, RESULTS = 0x30050000, 0x30060000
PROJECT = 0x30070000                  # a project's resource part (the project is at -38h)
CONFIG = 0x30078000                   # a binary config's resource
LOBBIES = [("day", "Classic"), ("stk2_blockpost", "STALKER 2"), ("hangar_046", "Hangar"), ("camp_069", "Camp")]
ENGLISH = {"st_srp_lobby_scene_option": b"Lobby:", "st_srp_lobby_scene_restart": b"Applies after restart.",
           **{f"st_srp_lobby_{lobby}": name.encode() for lobby, name in LOBBIES}}


class Run(language.Run):
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_map(MENU, 0x40000)
        self.asked_again = []
        self.stub_query = False
        self.emu.hook_add(UC_HOOK_CODE, self.query_stub, begin=QUERY, end=QUERY)

    def query_stub(self, emu, address, size, _):
        """lobby_menu::query_resources (eax the lobby menu; ret) while the
        results are taken: recorded, not run."""
        if not self.stub_query:
            return
        self.asked_again.append(emu.reg_read(UC_X86_REG_EAX))
        sp = emu.reg_read(UC_X86_REG_ESP)
        emu.reg_write(UC_X86_REG_ESP, sp + 4)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def query(self):
        """query_resources from its start to its call of the query: the
        eight {name, class} requests and their user data pointers."""
        sp = hide.STACK + 0x9000
        self.word(sp, hide.SENTINEL)
        _, _ = self.run(QUERY, (QUERY_CALL,), sp, {**hide.GAME, UC_X86_REG_EAX: MENU})
        assert self.stopped == QUERY_CALL, hex(QUERY)
        ebp = self.emu.reg_read(UC_X86_REG_EBP)
        requests = [(self.text(self.read(ebp - 0x80 + 8 * i)).decode(), self.read(ebp - 0x7C + 8 * i))
                    for i in range(8)]
        data = [self.read(ebp - 0x40 + 4 * i) for i in range(8)]
        return requests, data, ebp

    def ready(self, scene):
        """on_render_scenes_ready(results) with result 3's resource `scene`
        (0 a failed query): "replayed" (on into the original's code, its
        span replayed) or "returned" (ret 4), and the lobby menus asked again."""
        self.emu.mem_write(RESULTS, bytes(0x2E0 * 8 + 0x200))
        self.word(RESULTS + 0x12C + 3 * 0x2E0, scene)
        sp = hide.STACK + 0x9000
        self.word(sp, hide.SENTINEL)
        self.word(sp + 4, RESULTS)
        self.asked_again, self.stub_query = [], True
        try:
            _, end = self.run(READY, (READY_REPLAYED, hide.SENTINEL), sp, {**hide.GAME, UC_X86_REG_ECX: MENU})
        finally:
            self.stub_query = False
        if self.stopped == READY_REPLAYED:
            return "replayed", self.asked_again
        assert self.stopped == hide.SENTINEL and end == sp + 8, (hex(self.stopped or 0), hex(end), hex(sp))
        return "returned", self.asked_again

    def engine(self, emu, address, size, _):
        if address == language.TRANSLATE:              # thiscall (translator, key, out), ret 8
            sp = emu.reg_read(UC_X86_REG_ESP)
            key, out = self.text(self.read(sp + 4)).decode(), self.read(sp + 8)
            if key in ENGLISH:
                emu.mem_write(out, ENGLISH[key] + b"\0")
                emu.reg_write(UC_X86_REG_ESP, sp + 12)
                emu.reg_write(UC_X86_REG_EIP, self.read(sp))
                return
        super().engine(emu, address, size, _)


def project(r, track):
    """A project's resource part at PROJECT, its track (project+1E4h) given."""
    r.emu.mem_write(PROJECT - 0x38, bytes(0x400))
    r.word(PROJECT + 0x1AC, track)
    return PROJECT


def main():
    original = open(sys.argv[1], "rb").read()
    patched, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None,
                            open(base.late_join_bin(), "rb").read())
    stock, ours = Run(original), Run(patched)
    expect = lambda ok, text: print("ok", text) if ok else sys.exit("FAIL " + text)

    # on_configs_loaded's start: the variable, once, after patch 79's
    head_before = ours.read(language.LIST_HEAD)
    _, _, head = ours.start()
    variable = language.console_variable(ours, head)
    localization = language.console_variable(ours, ours.read(head + 0x0C))
    expect(variable[0] == "srp_lobby_scene" and variable[2] == 32 and variable[3] == 1
           and localization[0] == "localization" and ours.read(ours.read(head + 0x0C) + 0x0C) == head_before,
           "configs.start: `srp_lobby_scene`, a saved string of 32 bytes, at the head of the console's list, "
           "after patch 79's `localization` (the site's chain)")
    _, _, again = ours.start()
    expect(again == head, "configs.start again: registered once")
    buffer = variable[1]
    ours.emu.mem_write(language.TEXT, b"camp_069\0")
    ours.call_at(language.EXECUTE, (language.TEXT,), {UC_X86_REG_ECX: head})
    expect(ours.text(buffer) == b"camp_069", "user.cfg's line `srp_lobby_scene camp_069` sets it "
           "(cc_string::execute 0xEC8C0)")

    def saved(value):
        ours.emu.mem_write(buffer, value.encode() + b"\0")

    # the lobby's query and its results, a fresh scene for each case
    stock_requests, stock_data, _ = stock.query()
    stock_ready = stock.ready(project(stock, 1))
    expect(stock_requests[3] == ("lobby_scene/day", 0x40) and stock_data[3] != 0 and stock_ready == ("replayed", []),
           "the original asks lobby_scene/day (class 40h, the lobby menu's variant) and takes its results")
    config = lambda lobby: f"resources/projects/lobby_scene/{lobby}/client_project"
    # (label, saved value, [(result 3 for the query before it, the request asked, its user data kept)])
    cases = [
        ("no choice", "", [(project, ("lobby_scene/day", 0x40), True, "replayed")]),
        ("the classic", "day", [(project, ("lobby_scene/day", 0x40), True, "replayed")]),
        ("an unknown id", "klingon", [(project, ("lobby_scene/day", 0x40), True, "replayed")]),
        ("hangar_046, there", "hangar_046", [(CONFIG, (config("hangar_046"), 0x1F), False, "returned"),
                                             (project, ("lobby_scene/hangar_046", 0x40), True, "replayed")]),
        ("camp_069, missing", "camp_069", [(0, (config("camp_069"), 0x1F), False, "returned"),
                                           (project, ("lobby_scene/day", 0x40), True, "replayed")]),
        ("stk2_blockpost without its track", "stk2_blockpost", [
            (CONFIG, (config("stk2_blockpost"), 0x1F), False, "returned"),
            ("no track", ("lobby_scene/stk2_blockpost", 0x40), True, "returned"),
            (project, ("lobby_scene/day", 0x40), True, "replayed")]),
    ]
    for label, value, steps in cases:
        fresh = Run(patched)
        _, _, fresh_head = fresh.start()
        fresh.emu.mem_write(fresh.read(fresh_head + 0x40), value.encode() + b"\0")
        for i, (result, asked, keeps_data, flow) in enumerate(steps):
            requests, data, _ = fresh.query()
            others_same = requests[:3] + requests[4:] == stock_requests[:3] + stock_requests[4:] \
                and data[:3] + data[4:] == stock_data[:3] + stock_data[4:]
            scene = project(fresh, 1) if result is project else project(fresh, 0) if result == "no track" else result
            got = fresh.ready(scene)
            again = [MENU] if flow == "returned" else []
            expect(requests[3] == asked and (data[3] != 0) == keeps_data and others_same and got == (flow, again),
                   f"{label}, query {i + 1}: asks {asked[0]} (class {asked[1]:#x}, user data "
                   f"{'kept' if keeps_data else 'none'}), the other requests the original's; its results "
                   f"{'taken' if flow == 'replayed' else 'let go, the query asked again'}")

    # the Settings' call
    ours = Run(patched)
    _, _, head = ours.start()
    buffer = ours.read(head + 0x40)
    ids = [text for lobby, name in LOBBIES for text in (lobby, name)]
    answer = ["Lobby:", "Applies after restart."]
    ours.query()                                  # the scene asked as the classic (no choice yet)
    ours.ready(project(ours, 1))
    for label, argument, managed, saved_after, saves in [
        ("a lobby's id", "camp_069", False, "camp_069", 1),
        ("a managed string", "hangar_046", True, "hangar_046", 1),
        ("an unknown id", "klingon", False, "hangar_046", 0),
        ("no argument", None, False, "hangar_046", 0),
    ]:
        invokes, count = ours.callback("srp_lobby_scene", argument, managed)
        expect(invokes == [(language.MOVIE, "root.srp_lobby_scene", tuple(answer + [saved_after, "day"] + ids))]
               and count == saves and ours.text(buffer).decode() == saved_after,
               f"srp_lobby_scene with {label}: saved {saved_after}, user.cfg written {saves} time(s), the movie "
               f"answered with the texts, {saved_after}, the classic shown and the four lobbies")
        stock_invokes, stock_saves = stock.callback("srp_lobby_scene", argument, managed)
        expect(stock_invokes == [] and stock_saves == 0, f"the original ignores srp_lobby_scene ({label})")
    print("all cases passed")


if __name__ == "__main__":
    main()
