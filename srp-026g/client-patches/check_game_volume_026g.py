#!/usr/bin/env python3
"""Run patch 144 (rust/patches/src/game_volume.rs, its hooks of
game_volume.toml: the Settings' Game slider, docs/notes/game-volume.md) in
the default build on made-up game objects, with patch 88's harness
(check_music_026g.py) for the music's bus.

    python check_game_volume_026g.py <original survarium.exe>

Requires Python `unicorn`. Run as the game: sound_world::tick's general pass
from its read of s_general_volume (0x59AD7) to its end (0x59B4B), the
console's lookup and float setter, add_scene_to_active (0x59FB0); a submix
voice is a made-up object whose vtable's SetVolume (+30h) records its
arguments. Checked:

- the three hooks in place (`jmp` to their entries; the general pass's
  `cmp byte [edi+48F0h],al` in the original);
- `s_ingame_volume` registered at on_configs_loaded's start, last of the
  chain, found by the console's lookup (the stock Game option's): a saved
  kind-2 cc_float of 0 to 100 at 100; the game's setter takes 40;
- Game at 100 is stock: the same scenes through the original exe's pass and
  this one's give the same factors and the same SetVolume calls;
- Game at 40, General at 80: the bus at General's 0.8 alone, every other
  scene at 0.8 x 0.4, each submix set once to its volume x its factor, the
  world's byte 80; EBX, EBP, ESI, EDI and ESP as the stock pass leaves them;
- a pass with nothing changed sets nothing; Game at 0: the game's scenes
  silent, the bus not touched; General at 30 with Game at 0: the bus alone set;
- a scene made active (add_scene_to_active gives General's factor alone) is
  given Game's at the next pass;
- the main menu's sound tab no longer greys the Game slider out.
"""
import pathlib
import struct
import sys

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_ESI, UC_X86_REG_EDI, \
    UC_X86_REG_EBP, UC_X86_REG_ESP

import check_music_026g as music
import patch_026g as base

GENERAL_READ, GENERAL_SITE, GENERAL_DONE = 0x59AD7, 0x59ADF, 0x59B4B
ADD_ACTIVE = 0x59FB0
GENERAL_VALUE = 0xA7AF90                  # s_general_volume's value (its cc_float 0xA7D3C8, +40h)
SCENES = music.HEAP + 0x60000             # the game's scenes, 0x400 apart
SUBMIXES = music.HEAP + 0x70000           # their submix voices, 0x10 apart
SUBMIX_VTABLE = music.HEAP + 0x78000
SET_VOLUME = music.STUBS + 0x700          # IXAudio2Voice::SetVolume (stdcall this, volume, operation set)
KEPT = (UC_X86_REG_EBX, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI)   # live at 0x59B4B (EAX, ECX, EDX written first)

failed = 0


def check(ok, what):
    global failed
    failed += not ok
    print(f"  {'ok' if ok else 'FAIL'}  {what}")


def f32(x):
    return struct.unpack("<f", struct.pack("<f", x))[0]


class Mixer:
    """A game of check_music's with submix voices that record their SetVolume."""

    def __init__(self, exe):
        self.game = game = music.Game(exe)
        self.sets = []
        game.emu.mem_write(SET_VOLUME, b"\xC2\x0C\x00")
        game.emu.hook_add(UC_HOOK_CODE, self.set_volume, begin=SET_VOLUME, end=SET_VOLUME)
        game.word(SUBMIX_VTABLE + 0x30, SET_VOLUME)

    def set_volume(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.sets.append((self.game.read(sp + 4), self.game.float(sp + 8), self.game.read(sp + 12)))

    def submix(self, i):
        at = SUBMIXES + 0x10 * i
        self.game.word(at, SUBMIX_VTABLE)
        return at

    def scene(self, at, volume, submix):
        """A scene the cook would make, its fade volume and submix given, made active by the game."""
        game = self.game
        game.emu.mem_write(at, bytes(0x400))
        game.word(at + 0xD0, 1)
        game.emu.mem_write(at + 0x2E4, struct.pack("<f", volume))
        game.word(at + 0x2E0, submix)
        game.call(ADD_ACTIVE, {UC_X86_REG_ECX: music.WORLD, UC_X86_REG_EAX: at})
        game.emu.mem_write(at + 0x2F9, b"\x01")
        return at

    def general(self, value):
        self.game.emu.mem_write(GENERAL_VALUE, struct.pack("<f", value))

    def factor(self, scene):
        return self.game.float(scene + 0x2E8)

    def factor_bits(self, scene):
        return self.game.read(scene + 0x2E8)

    def run_pass(self):
        """The general pass, from the read of General to the pass's end: the SetVolume calls,
        the registers and ESP after, those given."""
        sp = music.STACK - 0x200
        given = {**music.STEADY, UC_X86_REG_EDI: music.WORLD, UC_X86_REG_ESI: 0}
        self.sets = []
        after, esp = self.game.go(GENERAL_READ, GENERAL_DONE, given, sp)
        return self.sets, {r: after[r] for r in KEPT}, esp == sp, {r: given[r] for r in KEPT}


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    linked = base.arena()
    reader = base.hooks_026g.original_reader(original)
    ours = [h for h in base.hooks_026g.hooks(exe=reader)
            if any(m["patch"] == "game_volume" for m in h.get("chain", [h]))]
    installs = {h[0]: h for h in base.rust_hooks(original)}
    for h in ours:
        va, old, symbol, opcode = installs[h["site"]]
        at = base.file_offset(va)
        new = base.hook_bytes(linked.symbols[symbol], va, old, 0, opcode)
        check(opcode == 0xE9 and original[at:at + len(old)] == old and built[at:at + len(new)] == new,
              f"{va:#x}: {old.hex()} in the original, `jmp {symbol}` here")
    general_site = [installs[h["site"]] for h in ours if installs[h["site"]][0] == GENERAL_SITE]
    check(len(ours) == 3 and general_site and general_site[0][1] == bytes.fromhex("3887f0480000"),
          f"game_volume's three hooks ({len(ours)}), the general pass's at its cmp byte [edi+48F0h],al")

    # Game at 100 is stock: three scenes through the original's pass and this one's (the bus not open:
    # every scene the game's)
    def three(mixer):
        mixer.general(80.0)
        scenes = [mixer.scene(SCENES + 0x400 * i, v, mixer.submix(i)) for i, v in enumerate((1.0, 0.5, 0.25))]
        sets, after, same_sp, given = mixer.run_pass()
        return [mixer.factor_bits(s) for s in scenes], sets, after, same_sp, given

    stock = three(Mixer(original))
    ours_full = three(Mixer(bytes(built)))
    check(stock[0] == ours_full[0] and stock[1] == ours_full[1] and len(stock[1]) == 3
          and stock[2] == ours_full[2] == stock[4] and ours_full[3] and stock[3],
          f"Game at 100 (its default): the original's factors and SetVolume calls ({stock[1]}); EBX, EBP, ESI, "
          "EDI and ESP as the original leaves them")

    mixer = Mixer(bytes(built))
    game = mixer.game

    def hook(name, **named):
        regs = {getattr(music, "UC_X86_REG_" + k): v for k, v in named.items()}
        return game.hook(name, regs)[0]

    game.at(1000)
    hook("configs")
    head = game.read(music.LIST_HEAD)
    name = game.put_text("s_ingame_volume")
    node = game.call(music.FIND, {UC_X86_REG_ESI: name})[UC_X86_REG_EAX]
    value = game.read(node + 0x40) if node else 0
    check(node and node == head and game.read(node) == music.CC_FLOAT_VTABLE
          and game.text(game.read(node + 0x10)) == "s_ingame_volume" and game.read(node + 0x14) == 2
          and game.read(node + 0x1C, 2)[1] == 1 and (game.float(node + 0x44), game.float(node + 0x48)) == (0.0, 100.0)
          and game.float(value) == 100.0,
          "configs.start: s_ingame_volume registered, the chain's last, found by the console's lookup (0xEB7C0, "
          "the Settings' Game option's): a cc_float, saved (user.cfg), kind 2, 0 to 100, 100 at first")
    hook("configs")
    check(game.read(music.LIST_HEAD) == head, "configs.start again: registered once")

    def slider(text):
        game.call(music.SET_FLOAT, {UC_X86_REG_ECX: node}, (game.put_text(text),))
        hook("frame", EBX=music.GAME)
        return game.float(value)

    # the music's bus opened as patch 88 opens it, its submix one of ours
    calls = hook("frame", EBX=music.GAME)
    function = calls[0][4]
    game.emu.mem_write(music.SCENE, bytes(0x400))
    game.word(music.SCENE + 0xD0, 1)
    game.word(music.SCENE + 0x2E0, mixer.submix(9))
    game.emu.mem_write(music.STUBS + 0x400, function)
    game.emu.mem_write(music.RESULTS, bytes(0x400))
    game.word(music.RESULTS + 0x38, 1)
    game.word(music.RESULTS + 0x12C, music.SCENE)
    game.callback(music.STUBS + 0x408, music.RESULTS)
    hook("sound", EDI=music.WORLD, EBX=10)
    bus = music.SCENE
    game.emu.mem_write(bus + 0x2E4, struct.pack("<f", 0.75))       # a fade's volume
    one, two = (mixer.scene(SCENES + 0x400 * i, v, mixer.submix(i)) for i, v in ((0, 1.0), (1, 0.5)))
    check(game.read(music.WORLD + 0x48E4) == bus and game.read(bus + 0x108) == one and game.read(one + 0x108) == two,
          "the active scenes: the bus, then two of the game's")

    check(slider("40") == 40.0, "the game's own setter takes 40")
    mixer.general(80.0)
    sets, after, same_sp, given = mixer.run_pass()
    general, game_factor = f32(80 * f32(0.01)), f32(f32(80 * f32(0.01)) * f32(0.4))
    check(mixer.factor(bus) == general and mixer.factor(one) == game_factor and mixer.factor(two) == game_factor
          and game.read(music.WORLD + 0x48F0, 1) == bytes([80]),
          f"General 80, Game 40: the bus at General's {mixer.factor(bus)} alone, the game's scenes at "
          f"{mixer.factor(one)} (0.8 x 0.4), the world's byte 80")
    expect = [(mixer.submix(9), f32(0.75 * general), 0), (mixer.submix(0), f32(1.0 * game_factor), 0),
              (mixer.submix(1), f32(0.5 * game_factor), 0)]
    check(sets == expect, f"each submix set once, its volume x its factor, at once (operation set 0): {sets}")
    check(after == given and same_sp, "EBX, EBP, ESI, EDI and ESP as the stock pass leaves them")

    sets = mixer.run_pass()[0]
    check(sets == [], f"a pass with nothing changed sets nothing ({sets})")
    slider("0")
    sets = mixer.run_pass()[0]
    check(mixer.factor(one) == 0.0 and mixer.factor(two) == 0.0 and mixer.factor(bus) == general
          and sets == [(mixer.submix(0), 0.0, 0), (mixer.submix(1), 0.0, 0)],
          f"Game 0: the game's scenes silent, the bus untouched ({sets})")
    mixer.general(30.0)
    sets = mixer.run_pass()[0]
    check(mixer.factor(bus) == f32(30 * f32(0.01)) and sets == [(mixer.submix(9), f32(0.75 * f32(30 * f32(0.01))), 0)],
          f"General 30 with Game 0: the bus alone set ({sets})")

    slider("40")
    mixer.sets = []
    three_ = mixer.scene(SCENES + 0x800, 1.0, mixer.submix(2))
    made = list(mixer.sets)
    sets = mixer.run_pass()[0]
    general = f32(30 * f32(0.01))
    game_factor = f32(general * f32(0.4))
    check(made == [(mixer.submix(2), general, 0)] and mixer.factor(three_) == game_factor
          and (mixer.submix(2), game_factor, 0) in sets,
          f"a scene made active: General's factor from add_scene_to_active ({made}), Game's at the next pass")

    tabs = (pathlib.Path(__file__).resolve().parent / ".." / "flash" / "main_menu" / "src" / "main_menu_fla"
            / "settingsTabs_15.as").read_text(encoding="utf-8")
    sound_tab = tabs[tabs.index("internal function frame4()"):].split("}")[0]
    check("this._game_volume." not in sound_tab, "the main menu's sound tab leaves the Game slider enabled")

    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
