#!/usr/bin/env python3
"""Run patch 94 (rust/patches/src/scarcity.rs, its lohooks of scarcity.toml:
Scarcity, a battery match with no ammo crates) in the default build on
made-up game objects, each from the game's own code before its site to past
it.

    python check_scarcity_026g.py <original survarium.exe>

Requires Python `unicorn`. check_blackout_026g.py's harness reads the match's
options (process_static_match_info from 0x27B917, a replay's chunk 1 from
0x1F094C) with srp's block; then project_cooker_simple::create_game_objects
runs from its entry (0x22C010) to past its first three instructions
(0x22C016) on a made-up client_project, relocated as the loader leaves one,
the game's own config lookups (0xF6B10, 0xF6C00) reading it. The original runs
the same way and must leave the config as it is:

- a match without srp's block, with Blackout's entry (tag 2, one byte 1) or
  with another value: the config unchanged, byte for byte;
- a Scarcity match (tag 2, one byte 2), read from the 0xE4 or a replay: the
  crates out of it: each resupply_zone and every client object of its folder,
  every render visual and collision object of item/quest/container_closed;
  the rest in their order, and the lamps' visual indexes renumbered;
- the next match without it: unchanged again;
- the frame as create_game_objects' own first three instructions leave it
  (ebp the entry's esp - 4, esp aligned to 16 below it) and the callee-saved
  registers kept.
"""
import importlib.util
from pathlib import Path
import struct
import sys

from unicorn import UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP

import patch_026g as base
import check_hide_ui_026g as hide
import check_blackout_026g as blackout

spec = importlib.util.spec_from_file_location(
    "content_edit", Path(__file__).resolve().parents[1] / "scripts/content-edit/content_edit.py")
edit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(edit)

COOK, PAST = 0x22C010, 0x22C016
CONFIG = blackout.WORK + 0x8000          # the binary config object: its root record at +108h
TREE = 0x30300000                         # the relocated records, names and texts
KEPT = (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI)
CRATE = "models/v069d_rudnya/item/quest/container_closed"
CRATE_COLLISION = "resources/models/v069d_rudnya/item/quest/container_closed.model/collision#[1.12][1.12][1.12]"


def items(*values):
    return dict(enumerate(values))


def obj(kind, name):
    return {"game_object_type": kind, "full_name": name}


def project():
    """Two crates (folders logic/resupply/team1 and logic/fill/team2/resupply)
    among other objects, models and lamps."""
    return {
        "client_objects": items(
            obj("light", "lights/lamp_1"),
            obj("resupply_zone", "logic/resupply/team1/resupply_zone"),
            obj("collision_geometry", "logic/resupply/team1/collision_geometry"),
            obj("decal", "logic/resupply/team1x/decal"),            # another folder: kept
            obj("decal", "logic/resupply/team1/decal"),
            obj("collision_geometry", "logic/fill/team2/resupply/geometry"),
            obj("resupply_zone", "logic/fill/team2/resupply/resupply_zone_team1"),
            obj("decal", "logic/fill/team2/decal"),                   # the folder above: kept
            obj("sky", "sky")),
        "render_visuals": items(*({"lib_name": name} for name in (
            "models/house", CRATE, "models/lamp_on", CRATE, "models/lamp_off", "models/fence"))),
        "collision_objects": items(*({"lib_name": name} for name in (
            CRATE_COLLISION, "resources/models/house.model/collision#[1][1][1]",
            CRATE_COLLISION.replace("/collision#", "/walker_collision#")))),
        "srp_lamps": {"version": 3, "lamps": items(
            {"visuals": {"intact": {"index": 2, "state": 0}, "broken": {"index": 4, "state": 1}}},
            {"visuals": {"intact": {"index": 0, "state": 0}, "broken": {"index": 5, "state": 1}}})},
    }


def relocated(tree):
    """The config as the loader leaves it at TREE: every table's children and
    every text an address."""
    b = bytearray(24)
    b[:24] = edit.value_record(b, 0, "", tree)
    records = []

    def visit(off):
        d, name, crc, t, count = edit.rec(b, off)
        records.append((off, d, name, crc, t, count))
        if t in (3, 4):
            for kid in edit.children(b, off):
                visit(kid)
    visit(0)
    for off, d, name, crc, t, count in records:
        edit.REC.pack_into(b, off, d + TREE if t >= 3 and d else d, name + TREE if name else 0, crc, t, count)
    return bytes(b)


class Run(blackout.Run):
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_map(TREE, 0x10000, UC_PROT_ALL)

    def cook(self, tree):
        """create_game_objects from its entry to 0x22C016 on `tree`: whether
        it got there with the frame and registers kept, the bytes before and
        after."""
        before = relocated(tree)
        self.emu.mem_write(TREE, bytes(0x10000))
        self.emu.mem_write(TREE, before)
        self.word(CONFIG + 0x108, TREE)
        sp = hide.STACK + 0x8000
        for i, value in enumerate((hide.SENTINEL, 0x40404040, CONFIG, 0x41414141, 0x42424242)):
            self.word(sp + 4 * i, value)
        regs, esp = self.run(COOK, (PAST,), sp, {**hide.GAME})
        ok = (self.stopped == PAST and regs[UC_X86_REG_EBP] == sp - 4 and esp == (sp - 4) & ~0xF
              and self.read(sp - 4) == hide.GAME[UC_X86_REG_EBP] and all(regs[r] == hide.GAME[r] for r in KEPT))
        return ok, before, bytes(self.emu.mem_read(TREE, len(before)))

    def record(self, at):
        """A record's children (or data), name and count."""
        data, name = struct.unpack("<II", self.emu.mem_read(at, 8)[:4] + self.emu.mem_read(at + 8, 4))
        count = struct.unpack("<H", self.emu.mem_read(at + 0x16, 2))[0]
        return data, name, count

    def node(self, at, path):
        """The record at `path` (names, or an int for an item) from the record `at`."""
        for step in path:
            first, _, count = self.record(at)
            records = [first + 24 * i for i in range(count)]
            at = records[step] if isinstance(step, int) else \
                next(r for r in records if self.text(self.record(r)[1]) == step.encode())
        return at

    def entries(self, path, key):
        """Each item's `key` text of the list at `path`."""
        count = self.record(self.node(TREE, path))[2]
        return [self.text(self.record(self.node(TREE, path + [i, key]))[0]).decode() for i in range(count)]


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

    print("the original: the cook's first three instructions")
    ok, before, after = stock.cook(project())
    report(ok and after == before, "push ebp / mov ebp,esp / and esp,-10h: the config unchanged, the frame and "
                                   "registers as the instructions leave them")

    def without_crates(run):
        return (run.entries(["client_objects"], "full_name") == [
                    "lights/lamp_1", "logic/resupply/team1x/decal", "logic/fill/team2/decal", "sky"]
                and run.entries(["render_visuals"], "lib_name") == ["models/house", "models/lamp_on",
                                                                    "models/lamp_off", "models/fence"]
                and run.entries(["collision_objects"], "lib_name") == [
                    "resources/models/house.model/collision#[1][1][1]"]
                and [run.read(run.node(TREE, ["srp_lamps", "lamps", lamp, "visuals", view, "index"]))
                     for lamp in (0, 1) for view in ("intact", "broken")] == [1, 2, 0, 3])

    cases = [
        ("a match without srp's block", "static", b"", False),
        ("a Blackout match (tag 2, one byte 1)", "static", blackout.srpx((2, b"\x01")), False),
        ("a Scarcity match (tag 2, one byte 2)", "static", blackout.srpx((2, b"\x02")), True),
        ("the next match without it", "static", b"", False),
        ("a Scarcity replay", "replay", blackout.srpx((1, bytes(12)), (2, b"\x02")), True),
        ("tag 2 of another value (3)", "static", blackout.srpx((2, b"\x03")), False),
        ("an unknown tag, then Scarcity", "replay", blackout.srpx((9, b"xyz"), (2, b"\x02")), True),
        ("Scarcity's entry of two bytes", "static", blackout.srpx((2, b"\x02\x02")), False),
        ("a short block", "static", blackout.srpx((2, b"\x02"))[:-1], False),
    ]
    print("the project's cook after the options read (process_static_match_info 0x27B92D, a replay's chunk 1 "
          "0x1F0971)")
    for name, where, block, scarce in cases:
        payload = blackout.empty_options(block)
        read = patched.static_info(payload) if where == "static" else patched.replay(payload)
        ok, before, after = patched.cook(project())
        kept = without_crates(patched) if scarce else after == before
        report(read[0] and ok and kept,
               f"{name} ({'a replay' if where == 'replay' else 'the 0xE4'}): "
               + ("both crates out (2 zones and their folders' 3 objects, 2 boxes, 2 box collisions), the rest "
                  "in order, the lamps' indexes 2, 4, 0, 5 now 1, 2, 0, 3" if scarce
                  else "the config unchanged") + "; the frame and registers as the original's")
    patched.static_info(blackout.empty_options(blackout.srpx((2, b"\x02"))))
    ok, before, after = patched.cook({"render_visuals": items({"lib_name": "models/house"}),
                                      "client_objects": items(obj("sky", "sky"))})
    report(ok and after == before, "a Scarcity match's project without crates (a lobby's): unchanged")

    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
