#!/usr/bin/env python3
"""Patch 112 (the Rust patch mouse_pitch, rust/patches/src/mouse_pitch.rs): the
mouse look's pitch with "Lock vertical sensitivity" (client_patch.md section
3.70, ../docs/notes/mouse-pitch.md).

    python check_mouse_pitch_026g.py <original survarium.exe>

Requires Python `unicorn`. check_language_026g.py's harness (the exe's
sections and the arena loaded as they are; Scaleform's Invoke recorded,
translate_text the key's English text, game::save_user_config counted):

- on_configs_loaded's start (0x1EA210 to 0x1EA216): `srp_mouse_vertical_locked`
  (a saved bool, on) and `srp_mouse_vertical` (a saved float, 1) registered
  once, of kind 2 (user.cfg's lines set them); the original registers neither;
- the Settings' call (0x1FF5D0): `srp_mouse_vertical_locked` with "1"/"0" and
  `srp_mouse_vertical` with hundredths ("1" to "1000") save it (user.cfg
  written once), another value or none nothing; each answers
  root.srp_check_option(name, label, saved) / root.srp_slider_option(name,
  label, saved hundredths); the original answers nothing;
- player_input_handler::on_mouse_move (0x240200) from its start to the wheel's
  store (0x2403E7) on a made-up handler (+358h its mode, +334h the game world,
  +9Ch the game, +53ACh its network client, +4 the local player, +48Ch its
  look pitch; +5348h the render window: vtable+1Ch stubbed, the window's w and
  h as floats, thiscall, `ret 4`); the half-float pack (0xF65A0) the game's
  own, its argument recorded: the new yaw (the first call) and the new pitch
  (the second) from a turn of 0. At eleven windows (4:3 to 32:9, three exact
  16:9 ones), the look pitch up and down (and no local player), the warmup's
  mode 3, an inverted mouse, sensitivities and an aimed fov factor (+94h):
  - locked (the default): the build bit for bit the original at an exact
    16:9 window, and at every other window the original's 16:9 pitch; the
    yaw the original's float everywhere;
  - unlocked: a count's pitch radians the yaw's to float precision at the
    vertical value (units x 60 degrees up, x 67.5 down: the look clip
    on_site_camera_look; radians in the warmup's orbit), the same at every
    window, the yaw as locked; at the value the greyed slider shows while
    locked (the option x 0.5625, to the slider's 0.01) the first person's
    looking-up rate the locked one's, to that rounding;
  - mode 4 (a spectator's): no turn in either.
Says nothing about the render window's real size; the look clip is decoded
in ../docs/notes/mouse-pitch.md.
"""
import math
import struct
import sys

from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_ESP, UC_X86_REG_EIP

import patch_026g as base
import check_language_026g as language

START, END = 0x240200, 0x2403E7               # on_mouse_move; `mov eax,esi` after the wheel's store
PACK = 0xF65A0                                # float -> half, cdecl (the float on the stack)
SENSITIVITY, INVERT, JOURNAL = 0xA74AEC, 0x1F1D9D3, 0xA8394C
CC_BOOL_VTABLE, CC_FLOAT_VTABLE = 0x9E7C4C, 0x9E7C6C
OBJECTS = 0x40000000
HANDLER, WORLD, GAME, RENDER, RVT = (OBJECTS + 0x1000 * i for i in range(5))
CLIENT, PLAYER = OBJECTS + 0x9000, OBJECTS + 0xA000
SIZE_STUB, RETURN = OBJECTS + 0x8000, OBJECTS + 0x8010
SP = OBJECTS + 0xF000
WINDOWS = (("4:3", 1024, 768), ("4:3", 1600, 1200), ("5:4", 1280, 1024), ("16:10", 1680, 1050),
           ("16:9", 1920, 1080), ("16:9", 1280, 720), ("16:9", 2560, 1440), ("16:9", 1366, 768), ("21:9", 2560, 1080), ("21:9", 3440, 1440),
           ("32:9", 5120, 1440))
# (mode, x, y, invert, option sensitivity in hundredths, the handler's +94h, the look pitch: None no local player)
MOVES = ((0, 1, 1, 0, 100, 1.0, 0.4), (0, 1, 1, 0, 100, 1.0, -0.4), (0, 7, -3, 0, 100, 1.0, 0.0),
         (0, -40, 25, 1, 100, 1.0, -0.9), (0, 13, 9, 0, 37, 1.0, 0.7), (0, 5, -11, 0, 100, 0.25, -0.2),
         (3, 12, 6, 0, 100, 1.0, 0.3), (0, 3, 8, 0, 1000, 0.8333333, -1.0), (0, 2, 5, 0, 250, 1.0, None))
VERTICALS = (1, 50, 200, 735, 1000)          # other vertical values, in hundredths
UP, DOWN = math.pi / 3, 3 * math.pi / 8       # radians of view a look-pitch unit, up and down (on_site_camera_look)


def unit(mode, look):
    """A look-pitch unit's radians: the warmup's orbit a radian, the first person's by the look pitch's side."""
    return 1.0 if mode == 3 else DOWN if look is not None and look < 0 else UP


LABELS = {"st_srp_mouse_lock_option": "Lock vertical sensitivity",
          "st_srp_mouse_vertical_option": "Vertical sensitivity"}
language.ENGLISH.update({key: text.encode() for key, text in LABELS.items()})


def hundredths(n):
    """A slider's value n x 0.01 (a double) as the exe's float."""
    return struct.unpack("<f", struct.pack("<f", n * 0.01))[0]


def bits(value):
    return struct.pack("<f", value)


class Run(language.Run):
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_map(OBJECTS, 0x10000, UC_PROT_ALL)
        self.emu.hook_add(UC_HOOK_CODE, self.size, begin=SIZE_STUB, end=SIZE_STUB)
        self.emu.hook_add(UC_HOOK_CODE, self.pack, begin=PACK, end=PACK)
        self.window, self.packed, self.sizes = (1024.0, 768.0), [], 0

    def size(self, emu, address, _size, _user):
        sp = emu.reg_read(UC_X86_REG_ESP)
        assert emu.reg_read(UC_X86_REG_ECX) == RENDER
        out = self.read(sp + 4)
        emu.mem_write(out, struct.pack("<ff", *self.window))
        self.sizes += 1
        emu.reg_write(UC_X86_REG_EAX, out)
        emu.reg_write(UC_X86_REG_ESP, sp + 8)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def pack(self, emu, address, _size, _user):
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.packed.append(struct.unpack("<f", emu.mem_read(sp + 4, 4))[0])

    def variables(self):
        """The console's list from its head: {name: (vtable, kind, saved, value's address)}."""
        out, node = {}, self.read(language.LIST_HEAD)
        while node:
            out[self.text(self.read(node + 0x10)).decode()] = (self.read(node), self.read(node + 0x14),
                                                               self.read(node + 0x1C, 2)[1], self.read(node + 0x40))
            node = self.read(node + 0x0C)
        return out

    def move(self, window, mode, x, y, invert, sensitivity, factor, look):
        """One move: the pack's floats (the new yaw, the new pitch), the window
        size's calls and where it stopped."""
        emu = self.emu
        emu.mem_write(OBJECTS, bytes(0xE000))
        emu.mem_write(SIZE_STUB, b"\xcc")
        emu.mem_write(RETURN, b"\xcc")
        self.word(HANDLER + 0x358, mode)
        emu.mem_write(HANDLER + 0x94, struct.pack("<f", factor))
        self.word(HANDLER + 0x334, WORLD)
        self.word(WORLD + 0x9C, GAME)
        self.word(GAME + 0x53AC, CLIENT)
        self.word(CLIENT + 4, PLAYER if look is not None else 0)
        emu.mem_write(PLAYER + 0x48C, struct.pack("<f", look or 0.0))
        self.word(GAME + 0x5348, RENDER)
        self.word(RENDER, RVT)
        self.word(RVT + 0x1C, SIZE_STUB)
        emu.mem_write(INVERT, bytes([invert]))
        emu.mem_write(SENSITIVITY, bits(hundredths(sensitivity)))
        self.word(JOURNAL, 0)                     # no input journal: the mouse's own step
        self.word(SP, RETURN)                     # the return address, then the input world, x, y, z
        for i, value in enumerate((0, x, y, 0)):
            self.word(SP + 4 + 4 * i, value)
        self.window, self.packed, self.sizes = window, [], 0
        self.run(START, (END, RETURN), SP, {UC_X86_REG_ECX: HANDLER})
        return list(self.packed), self.sizes, self.stopped


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    built, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None,
                          open(base.late_join_bin(), "rb").read())
    old, new = Run(original), Run(bytes(built))
    expect = lambda ok, text: print("ok", text) if ok else sys.exit("FAIL " + text)
    LOCK = "srp_mouse_vertical_locked"

    # on_configs_loaded's start
    old.start()
    new.start()
    ours = new.variables()
    locked, vertical = ours.get(LOCK), ours.get("srp_mouse_vertical")
    expect(not any(name.startswith("srp_mouse") for name in old.variables())
           and locked is not None and locked[:3] == (CC_BOOL_VTABLE, 2, 1) and new.read(locked[3], 1) == b"\x01"
           and vertical is not None and vertical[:3] == (CC_FLOAT_VTABLE, 2, 1)
           and bytes(new.emu.mem_read(vertical[3], 4)) == bits(1.0),
           f"configs.start: {LOCK} (a saved bool of kind 2, on) and srp_mouse_vertical (a saved float, 1); "
           "the original registers neither")
    new.start()
    expect(list(new.variables()) == list(ours), "configs.start again: each registered once")
    on = lambda: new.read(locked[3], 1) == b"\x01"
    value = lambda: struct.unpack("<f", new.emu.mem_read(vertical[3], 4))[0]

    # the Settings' calls
    for argument, want, saves in (("0", False, 1), ("1", True, 1), ("0", False, 1), ("on", False, 0), (None, False, 0)):
        invokes, count = new.callback(LOCK, argument)
        expect(invokes == [(language.MOVIE, "root.srp_check_option",
                            (LOCK, LABELS["st_srp_mouse_lock_option"], "1" if want else "0"))]
               and count == saves and on() == want,
               f"{LOCK} with {argument!r}: {int(want)}, user.cfg written {saves} time(s), answered")
    for argument, want, saves in (("123", 123, 1), ("1000", 1000, 1), ("1", 1, 1), ("0", 1, 0), ("1001", 1, 0),
                                  ("1.5", 1, 0), ("abc", 1, 0), (None, 1, 0), ("100", 100, 1)):
        invokes, count = new.callback("srp_mouse_vertical", argument)
        expect(invokes == [(language.MOVIE, "root.srp_slider_option",
                            ("srp_mouse_vertical", LABELS["st_srp_mouse_vertical_option"], str(want)))]
               and count == saves and bits(value()) == bits(hundredths(want)),
               f"srp_mouse_vertical with {argument!r}: {want} hundredths ({value()!r}), user.cfg written {saves} "
               "time(s), answered")
    for name in (LOCK, "srp_mouse_vertical"):
        invokes, count = old.callback(name, "1")
        expect(invokes == [] and count == 0, f"the original ignores {name}")
    new.callback(LOCK, "1")

    def run(r, window, *move):
        packed, sizes, stopped = r.move(window, *move)
        assert stopped == END and sizes == 2 and len(packed) == 2, (window, move, packed, sizes, stopped)
        return packed

    def at(name_w_h):
        return float(name_w_h[1]), float(name_w_h[2])

    exact = lambda w, h: w * 9 == h * 16

    # locked: the original at 16:9, bit for bit, at every window
    for move in MOVES:
        mode, x, y, invert, sensitivity, factor, look = move
        reference = run(old, (1920.0, 1080.0), *move)
        for window in WINDOWS:
            name, w, h = window
            (o_yaw, o_pitch), (n_yaw, n_pitch) = run(old, at(window), *move), run(new, at(window), *move)
            case = (name, w, h, move)
            assert bits(o_yaw) == bits(n_yaw) and bits(n_pitch) == bits(reference[1]), case
            if exact(w, h):
                assert bits(o_pitch) == bits(n_pitch), case
            if (x, y) == (1, 1):
                yaw_rad = abs(n_yaw / x)
                print(f"   {name:>5} {w}x{h}, look pitch {look:+.1f}: pitch / yaw, original "
                      f"{abs(o_pitch / y) * unit(mode, look) / yaw_rad:.4f}, build locked {abs(n_pitch / y) * unit(mode, look) / yaw_rad:.4f}")
        print(f"ok locked: mode {mode}, move ({x}, {y}), invert {invert}, sensitivity {sensitivity / 100}, factor {factor}, "
              f"look pitch {look}: the original bit for bit at the exact 16:9 windows, its 16:9 pitch at all {len(WINDOWS)}")

    # unlocked: 1:1 at the vertical value; at the greyed slider's value the locked looking-up rate
    for move in MOVES:
        mode, x, y, invert, sensitivity, factor, look = move
        new.callback(LOCK, "1")
        locked_runs = {window: run(new, at(window), *move) for window in WINDOWS}
        new.callback(LOCK, "0")
        shown = int(sensitivity * 0.5625 + 0.5)       # the greyed slider: the option x 0.5625, to 0.01 (Math.round)
        for other in (shown,) + VERTICALS:
            new.callback("srp_mouse_vertical", str(other))
            unlocked = {window: run(new, at(window), *move) for window in WINDOWS}
            for window in WINDOWS:
                yaw, pitch = unlocked[window]
                assert bits(yaw) == bits(locked_runs[window][0]) and bits(pitch) == bits(unlocked[WINDOWS[0]][1]), (window, move, other)
                yaw_rad = abs(yaw / x)
                rate = abs(pitch / y) * unit(mode, look)            # the pitch's radians a count
                assert abs(rate / (yaw_rad * hundredths(other) / hundredths(sensitivity)) - 1) < 1e-6, (window, move, other)
                assert (pitch < 0) == ((y > 0) != bool(invert)), (window, move, other)
            if other == shown and mode == 0 and (look is None or look >= 0):
                locked_rate = abs(locked_runs[WINDOWS[0]][1] / y) * UP
                bound = 0.005 / (hundredths(sensitivity) * 0.5625) + 1e-6
                assert abs(rate / locked_rate - 1) <= bound, (move, rate, locked_rate)
        print(f"ok unlocked: mode {mode}, move ({x}, {y}), sensitivity {sensitivity / 100}, factor {factor}, look pitch {look}: "
              f"a count's pitch radians the yaw's x vertical / option at {shown / 100} (the greyed slider's) and "
              f"{len(VERTICALS)} others, at all {len(WINDOWS)} windows"
              + ("; at the greyed value the locked looking-up rate" if mode == 0 and (look is None or look >= 0) else ""))
    new.callback(LOCK, "1")

    for r, which in ((old, "the original"), (new, "the build")):
        packed, sizes, stopped = r.move((1920.0, 1080.0), 4, 9, 9, 0, 100, 1.0, 0.0)
        expect(packed == [] and sizes == 0 and stopped == RETURN, f"{which}: mode 4 (a spectator's) turns nothing")


if __name__ == "__main__":
    main()
