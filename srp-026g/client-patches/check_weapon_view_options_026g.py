#!/usr/bin/env python3
"""Patch 218 (the Rust patch weapon_view_options, rust/patches/src/weapon_view_options.rs):
the player's weapon view kept in user.cfg (../docs/notes/weapon-view-settings.md).

    python check_weapon_view_options_026g.py <original survarium.exe>

Requires Python `unicorn`. check_language_026g.py's harness (the exe's
sections and the arena loaded as they are; Scaleform's Invoke recorded,
translate_text the key's English text, game::save_user_config counted):

- on_configs_loaded's start (0x1EA210 to 0x1EA216): `srp_weapon_fov` (65),
  `srp_weapon_fov_aimed` (32.5), `srp_weapon_fov_scope` (32.5),
  `srp_sight_distance` (0), saved floats of kind 2 with their ranges, and
  `srp_weapon_fov_world` (a saved bool, off), registered once; the original
  registers none;
- the Settings' call (0x1FF5D0): each slider with its steps within its range
  saves it (user.cfg written once), a value out of the range, not digits or
  none nothing; each answers root.srp_slider_option(name, label, saved,
  least, most, steps a unit) in its steps (1, 1/10, 1/100, whole percent);
  the checkbox with "1"/"0" root.srp_check_option(name,
  label, saved); `srp_weapon_view_texts` root.srp_weapon_view_texts(title, note);
  the original answers none of them.
The lens and the eye the values make: check_attachments_026g.py.
"""
import struct
import sys

import patch_026g as base
import check_language_026g as language
from check_mouse_pitch_026g import CC_BOOL_VTABLE, CC_FLOAT_VTABLE, Run

LABELS = {"st_srp_weapon_fov_option": "Weapon FOV at the hip",
          "st_srp_weapon_fov_aimed_option": "Weapon FOV over sights",
          "st_srp_weapon_fov_scope_option": "Weapon FOV in scopes",
          "st_srp_sight_distance_option": "Sight distance",
          "st_srp_sight_distance_hint": "0 % = current, 100 % = classic",
          "st_srp_weapon_fov_world_option": "Weapon at the world FOV",
          "st_srp_weapon_view_heading": "Weapon view"}
language.ENGLISH.update({key: text.encode() for key, text in LABELS.items()})

# name: (label key, default, least, most) in the variable's units, its steps to a unit
SLIDERS = {"srp_weapon_fov": ("st_srp_weapon_fov_option", 65.0, 40.0, 100.0, 1),
           "srp_weapon_fov_aimed": ("st_srp_weapon_fov_aimed_option", 32.5, 20.0, 65.0, 10),
           "srp_weapon_fov_scope": ("st_srp_weapon_fov_scope_option", 32.5, 16.25, 32.5, 100),
           "srp_sight_distance": ("st_srp_sight_distance_option", 0.0, 0.0, 100.0, 1)}
WORLD = "srp_weapon_fov_world"


def bits(value):
    return struct.pack("<f", value)


def steps(value, unit):
    return str(int(round(value * unit)))


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    built, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None,
                          open(base.late_join_bin(), "rb").read())
    old, new = Run(original), Run(bytes(built))
    expect = lambda ok, text: print("ok", text) if ok else sys.exit("FAIL " + text)

    old.start()
    new.start()
    ours = new.variables()
    for name, (_, default, least, most, _) in SLIDERS.items():
        got = ours.get(name)
        expect(got is not None and got[:3] == (CC_FLOAT_VTABLE, 2, 1)
               and bytes(new.emu.mem_read(got[3], 4)) == bits(default),
               f"configs.start: {name} a saved float of kind 2, {default:g}")
    world = ours.get(WORLD)
    expect(world is not None and world[:3] == (CC_BOOL_VTABLE, 2, 1) and new.read(world[3], 1) == b"\x00",
           f"configs.start: {WORLD} a saved bool of kind 2, off")
    expect(not any(name.startswith(("srp_weapon_fov", "srp_sight")) for name in old.variables()),
           "the original registers none")
    new.start()
    expect(list(new.variables()) == list(ours), "configs.start again: each registered once")

    def value(name):
        return struct.unpack("<f", new.emu.mem_read(ours[name][3], 4))[0]

    for name, (label, default, least, most, unit) in SLIDERS.items():
        good = steps((least + most) / 2, unit)
        cases = [(good, float(good) / unit, 1), (steps(least, unit), least, 1), (steps(most, unit), most, 1),
                 (str(int(round(least * unit)) - 1) if least > 0 else "abc", None, 0),
                 (str(int(round(most * unit)) + 1), None, 0),
                 ("1.5", None, 0), (None, None, 0), (steps(default, unit), default, 1)]
        for argument, want, saves in cases:
            before = value(name)
            invokes, count = new.callback(name, argument)
            now = value(name)
            want = before if want is None or saves == 0 else want
            answer = (name, LABELS[label], steps(now, unit), steps(least, unit), steps(most, unit), str(unit))
            expect(invokes == [(language.MOVIE, "root.srp_slider_option", answer)] and count == saves
                   and bits(now) == bits(want),
                   f"{name} with {argument!r}: {now:g} ({steps(now, unit)} steps of 1/{unit}), user.cfg written "
                   f"{saves} time(s), answered with {steps(least, unit)} to {steps(most, unit)}")
    for argument, want, saves in (("1", True, 1), ("0", False, 1), ("on", False, 0), (None, False, 0)):
        invokes, count = new.callback(WORLD, argument)
        expect(invokes == [(language.MOVIE, "root.srp_check_option",
                            (WORLD, LABELS["st_srp_weapon_fov_world_option"], "1" if want else "0"))]
               and count == saves and (new.read(world[3], 1) == b"\x01") == want,
               f"{WORLD} with {argument!r}: {int(want)}, user.cfg written {saves} time(s), answered")
    invokes, count = new.callback("srp_weapon_view_texts")
    expect(invokes == [(language.MOVIE, "root.srp_weapon_view_texts",
                        (LABELS["st_srp_weapon_view_heading"], LABELS["st_srp_sight_distance_hint"]))]
           and count == 0, "srp_weapon_view_texts: the title and the sight distance's note, nothing saved")
    for name in list(SLIDERS) + [WORLD, "srp_weapon_view_texts"]:
        invokes, count = old.callback(name, "1")
        expect(invokes == [] and count == 0, f"the original ignores {name}")
    print("all checks passed")


if __name__ == "__main__":
    main()
