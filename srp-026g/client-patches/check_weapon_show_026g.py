#!/usr/bin/env python3
"""Run weapon::on_show and the next weapon::draw's models of the original
and of patch 91 (WEAPON_SHOW) on made-up weapons, with Unicorn (2.1.4).

    python check_weapon_show_026g.py <original survarium.exe>

The exe is built as patch_026g.py builds it (with the late join), without
the late join and over on_hash_mismatch; the original is run the same way.
The game's own code runs: weapon::on_show (0x243DD0, ECX the weapon, `ret
4`, run to its return), weapon::draw from the copy of the scope's locator
(0x244E99) to the end of its models (0x245017), add_aimed_rifle_scope_to_scene
(0x244B60), remove_aimed_rifle_scope_from_scene (0x244C50) and the
first-person test (0x243940: the user's +F4E3h, the game's network client's
local player's place). Stubs: the scene renderer's add_model (0x21CD0),
remove_model (0x21F40), update_model (0x22280), update_skeleton (0x22490)
and set_model_visible (0x253E0), which keep each model's in-scene byte
(instance +10Dh) as the renderer does and write down every call (an add of a
model in the scene, a remove of one out of it: the renderer's assertion);
update_pfx_transform (0x244570), set_foreground (0x243D40) and
set_movable_static (0x243CB0), which return.

Each case shows a weapon (its "aim" byte +181Eh as a world gives it), draws
it once with that byte, then once with it cleared (the aim released), for
the local player in first person and for another player, with no optic,
an optic that keeps the gun when aimed (hide_weapon_on_aim, +11Ch, 0) and
one that hides it (1):

- the original, first person, aimed, +11Ch 0: on_show adds the aimed
  sight and not the body, and after the release only the idle sight is in
  the scene (the bug);
- the build: on_show adds the body and the idle sight in every case and
  leaves +181Dh 0; the aimed draw then aims (the aimed sight in, the idle
  one out, the body out and the hands hidden for +11Ch 1 alone, +181Dh 1),
  the release puts the idle sight and the body back; no add or remove of a
  model already in or out of the scene;
- every other case: the build's scene, +181Dh and the hands' calls after
  each draw are the original's;
- +181Eh is never written by on_show or draw; the stack and EBX ESI EDI
  EBP are as the original leaves them.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX,
                               UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

ON_SHOW, DRAW_FROM, DRAW_TO = 0x243DD0, 0x244E99, 0x245017
ADD_MODEL, REMOVE_MODEL, UPDATE_MODEL, UPDATE_SKELETON = 0x21CD0, 0x21F40, 0x22280, 0x22490
SET_MODEL_VISIBLE, UPDATE_PFX, SET_FOREGROUND, SET_MOVABLE_STATIC = 0x253E0, 0x244570, 0x243D40, 0x243CB0
# weapon: user, transform, scope transform, optics, world, body, in the scene, aim
USER, TRANSFORM, OPTICS, WORLD_OF, BODY_OF, SCOPE_AIMED, AIM = 0x8, 0x9BC, 0x1808, 0x180C, 0x1810, 0x181D, 0x181E
IDLE_OF, AIM_OF, HIDE = 0x108, 0x10C, 0x11C          # rifle_scope
GAME_OF, SCENE_OF, RENDERER_OF, NETWORK_OF, SCENE_RENDERER_OF = 0x9C, 0x4, 0x5360, 0x53AC, 0x200064
PLACE, DEMO, GAME_OF_PLAYER, PLAYER_MODEL, SHOWN = 0x114, 0xF4E3, 0xF478, 0xF290, 0xF544
RENDER_REF, IN_SCENE = 0x108, 0x10D                  # model holder's render reference; instance's byte

WEAPON, PLAYER, OTHER, WORLD = 0x41000000, 0x41010000, 0x41020000, 0x41030000
GAME, NET, SCENE, OPTIC = 0x41040000, 0x41050000, 0x41060000, 0x41070000
MATRICES, STACK, RETURN = 0x41080000, 0x41098000, 0x410A0000
RENDERER, SCENE_RENDERER = 0x41100000, 0x41400000
MODELS = {"body": 0x41500000, "idle": 0x41510000, "aim": 0x41520000, "hands": 0x41530000}
INSTANCE = 0x8000                                    # each holder's instance, after it
KEPT = {UC_X86_REG_EBX: 0x11111111, UC_X86_REG_ESI: 0x22222222, UC_X86_REG_EDI: 0x33333333,
        UC_X86_REG_EBP: 0x44444444}


class Weapon:
    def __init__(self, data, first_person, hide, aim):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at in (WEAPON, PLAYER, OTHER, WORLD, GAME, NET, SCENE, OPTIC, MATRICES, STACK - 0x8000, RETURN,
                   *MODELS.values()):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        emu.mem_map(RENDERER, 0x210000, UC_PROT_ALL)
        self.names = {}
        for name, holder in MODELS.items():
            self.word(holder + RENDER_REF, holder + INSTANCE)
            self.names[holder + RENDER_REF] = name
        self.word(WEAPON + USER, PLAYER)
        self.word(WEAPON + WORLD_OF, WORLD)
        self.word(WEAPON + BODY_OF, MODELS["body"])
        self.word(WEAPON + OPTICS, 0 if hide is None else OPTIC)
        emu.mem_write(WEAPON + AIM, bytes([aim]))
        self.word(OPTIC + IDLE_OF, MODELS["idle"])
        self.word(OPTIC + AIM_OF, MODELS["aim"])
        emu.mem_write(OPTIC + HIDE, bytes([hide or 0]))
        self.word(WORLD + SCENE_OF, SCENE)
        self.word(WORLD + GAME_OF, GAME)
        self.word(SCENE + 0xD0, 1)                   # the world's reference: on_show and draw add and drop one
        self.word(GAME + RENDERER_OF, RENDERER)
        self.word(RENDERER + SCENE_RENDERER_OF, SCENE_RENDERER)
        self.word(GAME + NETWORK_OF, NET)
        self.word(NET + 4, PLAYER if first_person else OTHER)
        self.word(PLAYER + GAME_OF_PLAYER, GAME)
        self.word(PLAYER + PLAYER_MODEL, MODELS["hands"])
        emu.mem_write(PLAYER + PLACE, b"\x03")
        emu.mem_write(OTHER + PLACE, b"\x05")
        self.calls = []
        emu.hook_add(UC_HOOK_CODE, self.stub)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def byte(self, at):
        return self.emu.mem_read(at, 1)[0]

    def returns(self, pop):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pop)

    def in_scene(self, name):
        return self.byte(MODELS[name] + INSTANCE + IN_SCENE)

    def scene(self):
        return {name for name in ("body", "idle", "aim") if self.in_scene(name)}

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        arg = lambda n: self.read(sp + 4 + 4 * n)
        model = lambda: self.names[emu.reg_read(UC_X86_REG_EAX)]
        if address == ADD_MODEL:                     # EAX the model; (renderer, &scene, matrix, matrix), ret 10h
            assert arg(0) == SCENE_RENDERER and self.read(arg(1)) == SCENE, "add_model's renderer and scene"
            name = model()
            # The renderer asserts and adds nothing when the model is in the scene (0x21CFB).
            self.calls.append(("add", name) if not self.in_scene(name) else ("add again", name))
            emu.mem_write(MODELS[name] + INSTANCE + IN_SCENE, b"\x01")
            self.returns(16)
        elif address == REMOVE_MODEL:                # EAX the model; (renderer, &scene), ret 8
            assert arg(0) == SCENE_RENDERER and self.read(arg(1)) == SCENE, "remove_model's renderer and scene"
            name = model()
            self.calls.append(("remove", name) if self.in_scene(name) else ("remove again", name))
            emu.mem_write(MODELS[name] + INSTANCE + IN_SCENE, b"\x00")
            self.returns(8)
        elif address == UPDATE_MODEL:                # EAX the model; ret 10h
            self.returns(16)
        elif address == SET_MODEL_VISIBLE:           # (renderer, the model, surface, value), ret 10h
            assert arg(0) == SCENE_RENDERER
            self.calls.append(("visible", self.names[arg(1)], arg(2), arg(3)))
            self.returns(16)
        elif address == UPDATE_SKELETON:             # EDI the renderer; four stack arguments, ret 10h
            self.returns(16)
        elif address in (UPDATE_PFX, SET_FOREGROUND, SET_MOVABLE_STATIC):
            self.returns(0)

    def start(self, at, until, sp, registers):
        emu = self.emu
        for reg, value in {**KEPT, **registers}.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, sp)
        emu.emu_start(at, until, count=200000)
        assert emu.reg_read(UC_X86_REG_EIP) == until, f"stopped at {emu.reg_read(UC_X86_REG_EIP):#x}"

    def show(self):
        aim = self.byte(WEAPON + AIM)
        sp = STACK - 8
        self.word(sp, RETURN)
        self.word(sp + 4, 1)                         # shown: on_show's argument
        self.start(ON_SHOW, RETURN, sp, {UC_X86_REG_ECX: WEAPON})
        assert self.emu.reg_read(UC_X86_REG_ESP) == sp + 8, "on_show pops its argument (ret 4)"
        assert all(self.emu.reg_read(reg) == value for reg, value in KEPT.items()), "on_show keeps ebx esi edi ebp"
        assert self.byte(WEAPON + AIM) == aim, "on_show never writes +181Eh"
        return self.step()

    def draw(self):
        aim = self.byte(WEAPON + AIM)
        frame = STACK - 0x40                         # draw's ebp: (transform, matrices, count, ...)
        self.word(frame + 8, WEAPON + TRANSFORM)
        self.word(frame + 12, MATRICES)
        self.word(frame + 16, 1)
        sp = frame - 0xB0
        self.start(DRAW_FROM, DRAW_TO, sp, {UC_X86_REG_EBX: WEAPON, UC_X86_REG_ESI: MATRICES,
                                             UC_X86_REG_EBP: frame})
        assert self.emu.reg_read(UC_X86_REG_ESP) == sp and self.emu.reg_read(UC_X86_REG_EBX) == WEAPON \
            and self.emu.reg_read(UC_X86_REG_EBP) == frame, "draw's frame as it was"
        assert self.byte(WEAPON + AIM) == aim, "draw never writes +181Eh"
        return self.step()

    def step(self):
        calls, self.calls = self.calls, []
        return frozenset(self.scene()), self.byte(WEAPON + SCOPE_AIMED), tuple(calls)

    def release(self):
        # The aim let go: the world's byte, as weapon::deserialize or the
        # aim's end gives it (the game's write, not the patch's).
        self.emu.mem_write(WEAPON + AIM, b"\x00")


CASES = [(first_person, hide, aim) for first_person in (True, False) for hide in (None, 0, 1) for aim in (0, 1)]


def run(data):
    out = {}
    for case in CASES:
        weapon = Weapon(data, *case)
        shown = weapon.show()
        aimed = weapon.draw()
        weapon.release()
        released = weapon.draw()
        out[case] = (shown, aimed, released)
    return out


def label(case):
    first_person, hide, aim = case
    optic = "no optic" if hide is None else f"+11Ch {hide}"
    return f"{'first person' if first_person else 'another player'}, {optic}, +181Eh {aim}"


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    patched, _ = base.build(original, "section", code, None, late_join)
    before, _ = base.build(original, "section", code, None, None)
    mismatch, _ = base.build(original, "hash-mismatch", open(base.default_bin("hash-mismatch"), "rb").read())
    va, old, new = base.WEAPON_SHOW
    at = base.file_offset(va)
    assert original[at:at + len(old)] == old and all(image[at:at + len(new)] == new
                                                     for image in (patched, before, mismatch))
    stock, built = run(original), run(patched)
    assert run(before) == built and run(mismatch) == built, "the other builds do not show a weapon as this one"

    bug = (True, 0, 1)
    shown, aimed, released = stock[bug]
    assert shown[0] == {"aim"} and shown[1] == 1 and released[0] == {"idle"}, \
        f"the original, {label(bug)}: the body never comes back ({stock[bug]})"

    for case in CASES:
        first_person, hide, aim = case
        (shown, shown_aimed, shown_calls), (scene, scope_aimed, calls), released = built[case]
        optic = set() if hide is None else {"idle"}
        assert shown == {"body"} | optic and shown_aimed == 0, f"{label(case)}: on_show adds the body and the " \
            f"idle sight, +181Dh 0 ({built[case]})"
        assert all(not c[0].endswith("again") for step in built[case] for c in step[2]), \
            f"{label(case)}: no model added twice or removed twice ({built[case]})"
        if first_person and aim and hide is not None:
            body = set() if hide else {"body"}
            hands = [("visible", "hands", 1, 2)] if hide else []
            assert scene == {"aim"} | body and scope_aimed == 1 \
                and [c for c in calls if c[0] == "visible"] == hands, \
                f"{label(case)}: the next draw aims ({built[case]})"
            hands = [("visible", "hands", 1, 3)] if hide else []
            assert released[0] == {"body", "idle"} and released[1] == 0 \
                and [c for c in released[2] if c[0] == "visible"] == hands, \
                f"{label(case)}: the release puts the body and the idle sight back ({built[case]})"
        else:
            assert (scene, scope_aimed) == (shown, 0) and released[:2] == (shown, 0), \
                f"{label(case)}: nothing to aim ({built[case]})"
        if case != bug:
            for (s_scene, s_aimed, s_calls), (b_scene, b_aimed, b_calls) in zip(stock[case][1:], built[case][1:]):
                s_hands = [c for c in s_calls if c[0] == "visible"]
                b_hands = [c for c in b_calls if c[0] == "visible"]
                assert (s_scene, s_aimed) == (b_scene, b_aimed) and (s_hands == b_hands or hide == 1), \
                    f"{label(case)}: after each draw the scene is the original's ({stock[case]} / {built[case]})"

    print("patch 91: on_show of a weapon whose world says aimed, for the local player in first person with an "
          "optic that keeps the gun (+11Ch 0): the original adds the aimed sight and not the body, and after the "
          "release only the idle sight is drawn; the build adds the body and the idle sight on every show, the next "
          "draw aims (the body out and the hands hidden for +11Ch 1 alone) and the release puts both back; every "
          "other case after each draw as the original; no model added or removed twice; +181Eh never written; "
          "the three builds the same; the stack and the kept registers as the original leaves them")


if __name__ == "__main__":
    main()
