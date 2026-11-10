#!/usr/bin/env python3
"""Run patch 76's head shot of a corpse (rust/patches/src/head_shot.rs, its
lohooks head_shot.toml and its callers in ragdoll.rs and artefact_carry.rs;
../docs/notes/ragdoll.md) in the built exe, with Unicorn.

    python check_head_shot_026g.py <original survarium.exe>

A shot into a corpse's head puts that corpse's player in the dark: the
shooter's client says so in its next input free of quick-use keys
(weapon_pickup::Command::HeadShot, the victim's place), every client reads it
where an input is applied, and the one whose own player is that place's, and
dead, shows the hud movie's black view and turns every sound off.

The harness is check_ragdoll_026g.py's Game (the made-up match, the step at
game_world.after_move, the bullet's ray, the bones), one machine for the
whole flow: the patch's state is the build's own statics. What runs is the
build's code from each site to the end of its span, every hook of the site's
chain with it (patches 65, 78, 83, 85, 88, 81 and 76's others):

- game_world.after_move 0x1F7C3C (the step) and check_collision 0x32D627
  (a bullet's ray), as check_ragdoll_026g.py runs them;
- an input applied as base_player::set_input does: carry.input 0x2D86E0
  (ECX the player, the packet on the stack: patch 78 reads it), the copy of
  its actions to the player's +330h, then carry.filter 0x2D876F (EDI the
  player);
- input_builder.end 0x2421B0 (EBX the input handler, its actions at +33Ch)
  and carry.key 0x23FE42 (patch 78's key down and up, for its own command);
- player_icons.update_start 0x23AA10 ([esp+4] the ui), sound_world.tick
  0x59AB1 (EDI the sound world) and game.clear_resources 0x1ED030 (ECX the
  game).

Fixtures: the players, their skeleton and the level are check_ragdoll's (a
player's world core, +120h, a zeroed block: patch 78's key of it); the ui
(its hud movie [[+8]+108h]+4), the input handler, the input's packet, the
sound world (its mastering voice at +0C4h) and the unloading game are zeroed
blocks. No applied input has Use (patch 78's take would look for a donor).
Stubs: Scaleform's Movie::Invoke (0x511600: records the movie, the method
and its arguments, answers 1, or 0 for a movie made to refuse) and the
mastering voice's IXAudio2Voice::SetVolume (its vtable's +30h, stdcall: this,
the volume, the operation set), which records its arguments.

Each of these sites also runs in the original exe from the same registers,
stack and memory: the hook must leave every register and ESP as the
original's instructions of the span do.

Checked:
- the three sites of head_shot.toml are jmps to chains that hold its handler;
- a HeadShot applied while the local player lives, or for another place,
  does nothing; one for the dead local player's place makes the next step
  dark: root.set_dead_black(true) once to the hud movie (again while the
  movie refuses, again to another movie, never to a movie that was never
  black), SetVolume(master, 0.0, 0) once;
- the dark ends when the player lives again (set_dead_black(false) once,
  SetVolume(master, 1.0, 0) once), with another death of the place, and at
  the game's unload;
- the filter leaves the applied actions what patch 78's filter makes of a
  pattern it does not know (the marker and the quick bits out), and an
  input without the marker as it was;
- a sound world with no mastering voice does not stop the sound tick
  (nothing is called; a voice that appears while dark is not silenced:
  not checked, head_shot.rs keeps what it wished, not what it did);
- the sender: the local player's bullet in the head of a ragdoll half a
  second old or more is told in the first input with no quick bit and no
  marker, once (an input with a quick key down, or patch 78's carry
  command, goes out as it is and the notice waits); a ragdoll younger, a
  shot in the torso, another shooter's bullet, a bullet with no shooter and
  a victim that lives again before the input tell nothing; the told input,
  applied on the victim's client, is what makes it dark.
"""
import math
import struct
import sys

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base
from check_ragdoll_026g import BULLET, COOKED, HEAP, SHOOTER, STACK, STUBS, Game, check

# the sites (sites_026g.tsv): the VA and the span a lohook replays
CARRY_INPUT, FILTER, BUILDER_END, CARRY_KEY = (0x2D86E0, 6), (0x2D876F, 5), (0x2421B0, 5), (0x23FE42, 6)
ICONS, SOUND_TICK, UNLOAD = (0x23AA10, 6), (0x59AB1, 6), (0x1ED030, 10)
INVOKE = 0x511600                       # Scaleform's Movie::Invoke (thiscall: name, result, arguments, count; ret 10h)
SET_VOLUME = STUBS + 0x200              # the mastering voice's SetVolume (stdcall: this, volume, operation set)
# made-up objects (past check_ragdoll's own in its heap)
UI, HUD_HOLDER, HUD_UI, HUD_MOVIE, OTHER_UI, OTHER_MOVIE, HANDLER, MASTER, MASTER_VTABLE = \
    (HEAP + 0xD00000 + 0x1000 * i for i in range(9))
PACKET, KEY_FRAME = HEAP + 0xD09000, HEAP + 0xD0A000
ARTEFACT_ACTION, KEY_DOWN, KEY_UP = 0x23, 1, 2           # patch 78's carry key (H) at on_keyboard_action
SOUND_WORLD, UNLOADING_GAME, PLAYER_WORLD = HEAP + 0xD10000, HEAP + 0xD20000, HEAP + 0xD40000
ACTIONS, HANDLER_ACTIONS, WORLD_CORE, MASTER_VOICE = 0x330, 0x33C, 0x120, 0xC4
DEAD_BLACK = "root.set_dead_black"

# weapon_pickup::Command (crates/weapon-pickup/src/lib.rs): the marker and, in
# the six quick bits, a pattern of two or more bits; HeadShot(place) is the
# pattern at ordinal 30 + place
CHORD, QUICK = 1 << 29, 63 << 14
PATTERNS = [v for v in range(64) if v.bit_count() >= 2]
FORWARD, FIRE, AIM, QUICK_1 = 0x1, 0x20, 0x80, 1 << 14
HEAD, SPIN = ([n for n, _ in COOKED].index(name) for name in ("Head", "Spin"))
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP)
MARKS = dict(zip(REGS, (0x0A0A0A0A, 0x0B0B0B0B, 0x0C0C0C0C, 0x0D0D0D0D, 0x05050505, 0x0E0E0E0E, STACK - 0x800)))


def head_shot(place):
    return CHORD | PATTERNS[30 + place] << 14


class Match(Game):
    """check_ragdoll's game with the head shot's other sites: the same
    made-up match in the build and (`stock`) in the original exe."""

    def __init__(self, data, stock=None):
        super().__init__(data)
        self.stock = stock
        self.invoked, self.volumes, self.refusing, self.now = [], [], (), 1000
        emu = self.emu
        emu.hook_add(UC_HOOK_CODE, self.invoke, begin=INVOKE, end=INVOKE)
        emu.mem_write(SET_VOLUME, b"\xC2\x0C\x00")
        emu.hook_add(UC_HOOK_CODE, self.set_volume, begin=SET_VOLUME, end=SET_VOLUME)
        self.u32(MASTER, MASTER_VTABLE)
        self.u32(MASTER_VTABLE + 0x30, SET_VOLUME)
        self.u32(SOUND_WORLD + MASTER_VOICE, MASTER)
        self.hud_movie(HUD_UI, HUD_MOVIE)
        for place, x in enumerate((0.0, 5.0, 10.0)):
            player = self.add_player(place, x, 0.0)
            self.u32(player + WORLD_CORE, PLAYER_WORLD)      # patch 78's key of a player: its world core

    def both(self, at, value):
        """A word of the made-up memory, in the original's machine too."""
        self.u32(at, value)
        if self.stock is not None:
            self.stock.u32(at, value)

    def hud_movie(self, ui, movie):
        """The ui's hud movie: [[+8]+108h]+4 (0: the hud has none)."""
        self.both(UI + 8, HUD_HOLDER if movie else 0)
        self.both(HUD_HOLDER + 0x108, ui)
        self.both(ui + 4, movie)

    # -- stubs ---------------------------------------------------------------
    def invoke(self, emu, *_):
        esp = emu.reg_read(UC_X86_REG_ESP)
        ret, name, _result, arguments, count = (self.u32(esp + 4 * i) for i in range(5))
        movie = emu.reg_read(UC_X86_REG_ECX)
        values = [struct.unpack("<II", emu.mem_read(arguments + 24 * i + 4, 8)) for i in range(count)]
        self.invoked.append((movie, bytes(emu.mem_read(name, 64)).split(b"\0")[0].decode(),
                             tuple(bool(v & 0xFF) if kind == 2 else v for kind, v in values)))
        emu.reg_write(UC_X86_REG_EAX, 0 if movie in self.refusing else 1)
        emu.reg_write(UC_X86_REG_ESP, esp + 4 + 0x10)
        emu.reg_write(UC_X86_REG_EIP, ret)

    def set_volume(self, emu, *_):
        esp = emu.reg_read(UC_X86_REG_ESP)
        voice, operation_set = self.u32(esp + 4), self.u32(esp + 12)
        self.volumes.append((voice, struct.unpack("<f", emu.mem_read(esp + 8, 4))[0], operation_set))
        emu.reg_write(UC_X86_REG_EAX, 0)

    # -- the sites -------------------------------------------------------------
    def span(self, site, regs, stack=b""):
        """The site's span: the registers and ESP's move after it."""
        (start, size), emu = site, self.emu
        esp = STACK - 0x1000
        emu.mem_write(esp - 0x40, bytes(0x40) + stack.ljust(0x40, b"\0"))
        emu.reg_write(UC_X86_REG_ESP, esp)
        for reg, value in {**MARKS, **regs}.items():
            emu.reg_write(reg, value)
        try:
            emu.emu_start(start, start + size, count=40_000_000)
        except Exception as error:
            sys.exit(f"FAIL: {start:#x}: {error} at {emu.reg_read(UC_X86_REG_EIP):#x} "
                     f"(esp {emu.reg_read(UC_X86_REG_ESP):#x})")
        assert emu.reg_read(UC_X86_REG_EIP) == start + size, f"{start:#x}: stopped at {emu.reg_read(UC_X86_REG_EIP):#x}"
        return [emu.reg_read(r) for r in REGS], emu.reg_read(UC_X86_REG_ESP) - esp

    def hooked(self, site, regs, stack=b""):
        """The build's hook at the site, which must leave the registers and
        ESP as the original exe's instructions of the span do."""
        mine, theirs = self.span(site, regs, stack), self.stock.span(site, regs, stack)
        assert mine == theirs, (f"{site[0]:#x}: the hook left {[hex(r) for r in mine[0]]} esp {mine[1]:+d}, the "
                                f"original's span {[hex(r) for r in theirs[0]]} esp {theirs[1]:+d}")

    def apply(self, player, actions):
        """An input applied to `player` as base_player::set_input does: its
        entry (patch 78's carry.input reads the packet), its copy of the
        actions to +330h, the apply point. The actions the game's tick then
        reads."""
        self.now += 16
        self.both(PACKET + 4, actions)
        self.hooked(CARRY_INPUT, {UC_X86_REG_ECX: player}, struct.pack("<III", 0x2D0000, self.now, PACKET))
        assert self.u32(PACKET + 4) == actions, "the shared packet was changed"
        self.both(player + ACTIONS, actions)
        self.hooked(FILTER, {UC_X86_REG_EDI: player, UC_X86_REG_EAX: player + 0x32C})
        return self.u32(player + ACTIONS)

    def build(self, actions):
        """The input builder's end: the actions the input carries."""
        self.both(HANDLER + HANDLER_ACTIONS, actions)
        self.hooked(BUILDER_END, {UC_X86_REG_EBX: HANDLER})
        return self.u32(HANDLER + HANDLER_ACTIONS)

    def carry_key(self, state):
        """Patch 78's carry key down or up, where on_keyboard_action has its action."""
        self.both(KEY_FRAME + 0x10, state)
        self.hooked(CARRY_KEY, {UC_X86_REG_EDI: ARTEFACT_ACTION, UC_X86_REG_EBP: KEY_FRAME})

    def icons(self):
        """A frame of update_player_icons: what the hud movie's black view was told."""
        self.invoked = []
        self.hooked(ICONS, {}, struct.pack("<II", 0x23B20C, UI))
        return [(movie, values) for movie, method, values in self.invoked if method == DEAD_BLACK]

    def sound(self):
        """A tick of the sound thread: the SetVolume calls."""
        self.volumes = []
        self.hooked(SOUND_TICK, {UC_X86_REG_EDI: SOUND_WORLD, UC_X86_REG_EBX: 16})
        return self.volumes

    def unload(self):
        self.hooked(UNLOAD, {UC_X86_REG_ECX: UNLOADING_GAME})

    def frames(self, count, *corpses):
        for _ in range(count):
            self.step()
            for corpse in corpses:
                self.draw(corpse)

    # -- shots (the victim stands at the origin: its model space is the world's) --
    def shot(self, victim, bullet, part, shooter=1):
        """A bullet (its id, its shooter's place; None: no shooter) through the
        corpse's head (from beyond it, away from the hip) or down into its
        torso: whether the corpse was hit (the body-hit particle played; a
        bullet with no shooter plays none)."""
        drawn, _ = self.draw(victim)
        if part == "head":
            head, hip = drawn[HEAD][12:15], drawn[1][12:15]
            away = (head[0] - hip[0], 0.0, head[2] - hip[2])
            size = math.hypot(away[0], away[2])
            away = (away[0] / size, 0.0, away[2] / size) if size > 0.05 else (1.0, 0.0, 0.0)
            origin, direction = tuple(h + 1.5 * a for h, a in zip(head, away)), tuple(-a for a in away)
        else:
            spin = drawn[SPIN][12:15]
            origin, direction = (spin[0], spin[1] + 1.5, spin[2]), (0.0, -1.0, 0.0)
        self.u32(BULLET + 0x4, bullet)
        self.u32(BULLET + 0x8, 0 if shooter is None else SHOOTER)
        self.u8(SHOOTER + 0x4, shooter or 0)
        played = len(self.particles)
        self.shoot(origin, direction, 3.0)
        return len(self.particles) - played


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    linked = base.arena()
    block = linked.blocks["patches.code"]
    entries = {h["entry"] for h in base.hooks_026g.hooks(exe=base.hooks_026g.original_reader(original))
               if any(m["patch"] == "head_shot" for m in h.get("chain", [h]))}
    hooks = [h for h in base.rust_hooks(original) if h[2] in entries]
    check(len(hooks) == 3 and {h[0] for h in hooks} == {ICONS[0], SOUND_TICK[0], UNLOAD[0]},
          "the Rust patch head_shot's three lohooks (head_shot.toml, sites_026g.tsv)")
    for va, old, symbol, opcode in hooks:
        at = base.file_offset(va)
        target = linked.symbols[symbol]
        new = base.hook_bytes(target, va, old, 0, opcode)
        check(opcode == 0xE9 and original[at:at + len(old)] == old and built[at:at + len(new)] == new
              and block.va <= target < block.va + block.size,
              f"{va:#x}: {old.hex()} in the original, a jmp to its chain's entry ({symbol}) here")

    game = Match(built, Match(original))
    a, b, c = game.players
    dark, light = [(HUD_MOVIE, (True,))], [(HUD_MOVIE, (False,))]
    silent, full = [(MASTER, 0.0, 0)], [(MASTER, 1.0, 0)]
    quiet = lambda: (game.icons(), game.sound()) == ([], [])

    # -- all alive: nothing is told, and a head shot blinds no living player
    game.set_local(a)
    game.frames(2)
    check(quiet(), "a match's first frames: the hud movie, never black, is told nothing, and no volume is set")
    unknown = CHORD | PATTERNS[52] << 14
    plain = FORWARD | FIRE | AIM
    mask = head_shot(0) | FORWARD | FIRE | AIM
    applied = [game.apply(b, m) for m in (mask, unknown | FORWARD | FIRE | AIM, plain, CHORD | QUICK_1 | FORWARD)]
    check(applied[0] == applied[1] == FORWARD | FIRE | AIM and applied[2] == plain,
          f"carry.filter: a head shot's input is applied as patch 78's filter leaves a pattern it does not know "
          f"(the marker and the quick bits out: {mask:#x} -> {applied[0]:#x}), an input without the marker as it "
          f"was; the registers and ESP the original span's")
    game.frames(1)
    check(quiet(), "a head shot for the local player's place while it lives: nothing")

    # -- the sender: b (place 1) is the local player, a (place 0) the corpse
    game.set_local(b)
    game.set_alive(a, False, 2000)
    game.frames(12, a)
    check(game.shot(a, 100, "head") == 1, "the local player's bullet meets the head of a ragdoll 0.2 s old")
    game.frames(1, a)
    check(game.build(FORWARD) == FORWARD, "a head shot in the ragdoll's first half second is not told")
    game.frames(60, a)
    check(game.shot(a, 101, "torso") == 1, "the local player's bullet meets the torso of the ragdoll, over a second old")
    game.frames(1, a)
    check(game.build(FORWARD) == FORWARD, "a shot in the torso is not told")
    check(game.shot(a, 102, "head", shooter=2) == 1, "another player's bullet meets its head")
    game.frames(1, a)
    check(game.build(FORWARD) == FORWARD, "another shooter's bullet is not told")
    game.shot(a, 103, "head", shooter=None)
    game.frames(1, a)
    check(game.build(FORWARD) == FORWARD, "a bullet with no shooter is not told")
    check(game.shot(a, 104, "head") == 1, "the local player's bullet meets its head")
    game.frames(1, a)
    busy = FORWARD | QUICK_1
    check(game.build(busy) == busy, "an input with a quick-use key down is left as it is")
    game.carry_key(KEY_DOWN)
    check(game.build(busy) == busy | CHORD, "nor does it take the place of patch 78's carry command (H with the key)")
    game.carry_key(KEY_UP)
    sent = game.build(FORWARD | FIRE | AIM)
    check(sent == head_shot(0) | FORWARD | FIRE | AIM,
          f"the next input with no quick bit carries HeadShot(0), its own actions kept ({sent:#x}); the registers "
          f"and ESP the original epilogue's")
    check(game.build(FORWARD) == FORWARD and game.build(0) == 0, "told once: the inputs after it are unchanged")
    game.frames(3, a)
    check(game.build(FORWARD) == FORWARD, "the steps after it tell nothing more")
    check(quiet(), "the shooter's own client stays as it was: no black view, no volume set")
    # the victim lives again before an input could carry the notice
    check(game.shot(a, 105, "head") == 1, "the local player's bullet meets its head again")
    game.frames(1, a)
    check(game.build(busy) == busy, "the notice waits while the quick-use key is down")
    game.set_alive(a, True, 3000)
    game.frames(1)
    check(game.build(FORWARD) == FORWARD, "the victim lives again before a free input: the notice is dropped")

    # -- the receiver: a (place 0) is the local player, dead; b's inputs are applied
    game.set_local(a)
    game.set_alive(a, False, 3000)
    game.frames(5, a)
    check(quiet(), "the local player dead, no head shot: nothing")
    game.apply(b, head_shot(1) | FORWARD)
    game.apply(c, head_shot(2))
    game.frames(1, a)
    check(quiet(), "head shots of other places' corpses: nothing")
    check(game.apply(b, sent) == FORWARD | FIRE | AIM, "the sender's input applied on the victim's client")
    check(game.icons() == [], "applied, before the step: nothing yet")
    game.frames(1, a)
    game.refusing = (HUD_MOVIE,)
    check(game.icons() == dark and game.icons() == dark,
          "after the step the hud movie is told root.set_dead_black(true), again each frame while it refuses")
    game.refusing = ()
    check(game.icons() == dark and game.icons() == [] and game.icons() == [],
          "told once when the movie takes it, not again on later frames")
    check(game.sound() == silent and game.sound() == [] and game.sound() == [],
          "the sound tick calls SetVolume(master, 0.0, 0) once")
    game.frames(30, a)
    game.apply(b, head_shot(0))
    game.frames(2, a)
    check(quiet(), "dark while dead of that death (another head shot too): nothing more is told")
    game.hud_movie(OTHER_UI, OTHER_MOVIE)
    check(game.icons() == [(OTHER_MOVIE, (True,))] and game.icons() == [], "another hud movie is told the dark once")
    game.hud_movie(HUD_UI, 0)
    check(game.icons() == [], "a hud without a movie is told nothing")
    game.hud_movie(HUD_UI, HUD_MOVIE)
    check(game.icons() == dark and game.icons() == [], "the first movie, back, is told again")

    # -- it ends: the player lives again
    game.set_alive(a, True, 4000)
    game.frames(1)
    check(game.icons() == light and game.icons() == [], "alive again: root.set_dead_black(false) once")
    check(game.sound() == full and game.sound() == [], "alive again: SetVolume(master, 1.0, 0) once")
    game.frames(3)
    check(quiet(), "the frames after: nothing")

    def darken(life):
        game.set_alive(a, False, life)
        game.frames(4, a)
        game.apply(c, head_shot(0))
        game.frames(1, a)
        return game.icons() == dark and game.sound() == silent

    # another death of the place with no step that saw it alive (a rollback's re-simulation)
    check(darken(4000), "dead and shot in the head again: dark, silent")
    game.set_alive(a, False, 5000)
    game.frames(1, a)
    check(game.icons() == light and game.sound() == full, "another death of the place (a new life): the dark ends")
    game.set_alive(a, True, 5000)
    game.frames(1)

    # -- the game unloads its match while dark
    check(darken(5000), "dark again")
    game.unload()
    check(game.sound() == full and game.sound() == [],
          "game.clear_resources while dark (the registers and ESP the original span's): the next sound tick "
          "restores SetVolume(master, 1.0, 0)")
    check(game.icons() == light and game.icons() == [], "and a hud movie still there is told the black view is off")
    game.frames(2, a)
    check(quiet(), "the steps after the unload, the player still dead of that death: not dark again")
    game.set_alive(a, True, 6000)
    game.frames(1)

    # -- a sound world with no mastering voice (no device)
    game.both(SOUND_WORLD + MASTER_VOICE, 0)
    check(darken(6000) is False and game.volumes == [], "no mastering voice: dark, the sound tick runs and calls nothing")
    check(game.sound() == [], "its next tick too")
    game.set_alive(a, True, 7000)
    game.frames(1)
    check(game.icons() == light and game.sound() == [], "alive again with no mastering voice: the tick runs, nothing called")
    game.both(SOUND_WORLD + MASTER_VOICE, MASTER)
    check(darken(7000), "the voice back, the next dark: silent")
    game.set_alive(a, True, 8000)
    game.frames(1)
    check(game.icons() == light and game.sound() == full, "and full again")
    check(not game.files and not game.written, "the build's diagnostics are off: no file opened, nothing written")
    print("all checks passed")


if __name__ == "__main__":
    main()
