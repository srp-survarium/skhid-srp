#!/usr/bin/env python3
"""Run patch 59 (the Rattle, artefact type 8: rattle_026g.asm, rattle_026g.c)
in the default build on made-up cooks, inventories, players, weapons, sound
proxies and propagators, beside the original exe where they differ.

    python check_rattle_026g.py <original survarium.exe> [--pack <rattle-3 folder>]

Requires Python `unicorn`. The built exe's sections are loaded as they are
and its code (the arena's block rattle.code, linked by link_026g.py) runs as
machine code; the game's own code runs where the patch
calls it or is called from it: the artefact cook's construct and size
(0x25E660, 0x25E770), Rattle's constructor 0x25FA10 (artefact_base's
0x312FD0, the config's look-ups 0xF6B10/0xF6C00), set_game_world, the
inventory slot and added_to_inventory slots, artefact_base::action 0x313300
(patch 54's hook: one press fires every ready Rattle), artefact_base::tick
0x313200 with decrement_amount, the modifiers' getter 0x2D5AE0, the hud's
info 0x313880, serialize and deserialize 0x313670/0x313760 with the field
writer 0x1F48A0, on_player_sound_event 0x2A6470, 0x657C0 to its point and
hud proxies, play_sound_callback 0x297E20 to its point proxy,
calculate_hdr_audio_frame's two stores (0x5CA37, 0x5C8BA). Intercepted
(answered here): the allocator, the proxies' makers 0x64F80, 0x65670 and
0x65100, a sound's and a scene's
vtables, on_landing's step 0x23FBE0. With --pack (or SRP_RATTLE_PACK) the
cook reads the content pack's own artefact_rattle.options. Each slot of
ours is reached through the game's own caller too (the cases `driven: ...`,
which patch_026g.py's INSTALLED_VTABLES names): the hud's quick slot
(0x23A25B), artefact_base::action and tick, decrement_amount, and
generic_anomaly_core's serialize and deserialize loops. Last, what a
listener hears of a carrier (SoundWorld, a fresh Run of the build and of
the original): his animation's foot event, his landing and his fire state's
sound run through the game's own code to the point proxy it plays, and a
listener's frame (the propagator the game makes of that proxy, 0x5C7A0, the
voice's volume) gives the volume; his own game in first person plays them
as hud proxies, quieted the same; another player's own sounds are not.
What each case checks is printed.
"""
import argparse
import math
import os
import struct
import sys

from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI, \
    UC_X86_REG_EBX, UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_XMM0, UC_X86_REG_FPSW
from unicorn import UC_HOOK_CODE

import patch_026g as base
from check_sponge_a_026g import Run, Game, pe_sections, loaded, f32, bcw, CORE, CONFIG, STACK

HERE = os.path.dirname(os.path.abspath(__file__))
# The arena's link (link_026g.py): the Rattle's entries, C functions and
# data, and rt_g, the steps' pending quiet (the block rattle.state).
ARENA = base.arena()
SYMBOLS = {name: va for name, va in ARENA.symbols.items() if name.startswith(("rt_", "rattle_"))}
SPONGE_SYMBOLS = {name: va for name, va in ARENA.symbols.items() if name.startswith("sp_")}
STATE = SYMBOLS["rt_g"]
# the content pack (release/content.tsv rattle@3), in the main checkout's private folder
PACK = os.path.join(HERE, "..", "..", "..", "skhid-srp", "private", "content-packs", "rattle-3")
# struct rattle (rattle_026g.c's _Static_asserts)
R_DURATION, R_LEFT, R_PASSIVE_STEPS, R_ACTIVE_STEPS, R_ACTIVE_SHOTS, R_SIZE = 0x360, 0x364, 0x368, 0x36C, 0x370, 0x378
STATE_AT, COOLDOWN, ACTIVATED = 0x280, 0x28C, 0x2A0
QUIET = 0x3A                                       # a proxy's quiet (u16)
HUD_KIND = 0x80                                    # a proxy's kind: 3 a hud proxy (0x5A840), 0 a point one (0x5A6B0)


def rattle_options():
    """The pack's gameplay/items/artefacts/artefact_rattle.options, value for
    value (scripts/rattle/make_pack.py: 0.20e's made 0.26g's)."""
    return {
        "parameters": {"weight": f32(0.2)},
        "ui_desc": {"icon": [21, 1], "props_list": {},
                    "text_descriptions": {"name": "st_artefact_rattle_name",
                                          "description": "st_artefact_rattle_description"}},
        "object": {"model": "item/artefacts/artefact_pogremushka",
                   "particle_on_spawn": "fx/artifacts/emergence_zhivokost_single",
                   "sound_on_spawn": [257, "artefacts/artefact_appear_default"]},
        "data": {"activation_hint": "st_rattle_activation_hint", "pickup_hint": "st_rattle_pickup_hint",
                 "active": {"duration": 10, "shots_sound_volume_3rd_person": 0.5,
                            "steps_sound_volume_3rd_person": 0},
                 "passive": {"steps_sound_volume_3rd_person": f32(0.7)},
                 "amount": -1, "cooldown_sec": 40, "effect": "artefact_rattle", "spawn_sec": 1, "type": 8}}


def quiet_of(gain):
    """rattle_026g.c's quiet() in float32 steps."""
    gain = min(max(f32(gain), 0.0), 1.0)
    q = f32(f32(f32(1.0 - gain) * 65535.0) + 0.5)
    return 0xFFFF if q >= 65535.0 else int(q)


class SoundWorld:
    """A made-up match around the game's own sound code, for one exe: a
    carrier (place 3; the Game's inventory is his) and another player (place
    4, an inventory of his own, empty), each with his equipment's sound
    effect and a weapon's fire state, the world they play in (its sound
    scene, step manager and client with the camera), and the sound thread's
    side (a propagator of a played proxy, the listener's frame, the voice).
    Answered here: the scene's pool of proxies (0x65450, a fresh block), the
    command that hands a played proxy to the sound thread (0x5CEA0,
    recorded), the foot's ray, the step sound of a material (0x306D50), the
    weapon's record of the sound events it played (0x245430, none yet), the
    voice maker (0x59E70) and the sounds' own vtables. Everything between
    runs as the game's code: the animation event to the step's proxy, the
    shot's scheduled callback to its proxy, the point and hud proxies' makers
    (0x64F80, 0x65100) and constructors, their position and play, the
    propagator's constructor 0x5CB00, the listener's frame 0x5C7A0 and the
    voice's volume (notify_listener2 0x5C710..0x5C748, 0x633F0)."""
    DISTANCE = 8.0                                  # metres: amplitude 2 / 8 = 0.25
    VOLUME = 0.8                                    # the sound's own volume: a voice at 0.25 x 0.8 = 0.2

    def __init__(self, r, game):
        self.r = r
        self.game = game
        self.made, self.played, self.voices = [], [], []
        # the world (+94h the sound scene, +9Ch the client; slot 13 the step manager, slot 7 the
        # materials), the client (+53ACh the camera, +5350h the sound world's holder), the scene's pool
        self.world = world = r.allocate(0x5000)
        client, scene, pool = r.allocate(0x6000), r.allocate(0x200), r.allocate(0x40)
        self.camera, holder, sound_world = r.allocate(0x10), r.allocate(0x10), r.allocate(0x200)
        steps = r.allocate(0x40)
        r.word(world, r.vtable({7: (lambda r: 0x7777, 0), 13: (lambda r: steps, 0)}))
        r.word(world + 0x94, scene)
        r.word(world + 0x9C, client)
        r.word(client + 0x53AC, self.camera)
        r.word(client + 0x5350, holder)
        r.word(holder, r.vtable({2: (lambda r: sound_world, 0)}))
        r.word(scene + 0x188, pool)
        r.word(pool + 0x34, 16)                                       # 16 free, none taken
        r.stub(self.take, 0, at=0x65450, name="the scene's pool")
        r.stub(lambda r: self.played.append(r.arg(1)), 8, at=0x5CEA0, name="the sound thread's command")
        # the step manager (+4 the world) and its foot's ray: a hit (+1Ch of the ray's result) on material 7
        hit, collision, middle = r.allocate(0x10), r.allocate(0x10), r.allocate(0x200)
        r.word(steps + 4, world)
        r.word(world + 0x4D4C, middle)
        r.word(middle + 0x130, collision)
        r.word(hit, r.vtable({4: (lambda r: 7, 8)}))
        r.word(collision, r.vtable({18: (lambda r: r.word(r.arg(0) + 0x1C, hit), 0x20)}))
        # a sound (its intrusive count +D0h) and the step sound of the material
        self.sound = r.allocate(0x200)
        r.word(self.sound, r.vtable({7: (lambda r: 0x5555, 0)}))
        r.word(self.sound + 0xD0, 5)
        record = r.allocate(0x80)
        r.word(record + 0x20, self.sound)
        r.stub(lambda r: record, 0, at=0x306D50, name="the step sound of a material")
        # the carrier, and another player with no Rattle
        self.carrier = self.person(game.inventory, game.PLACE)
        self.player, self.effect, self.shot = self.carrier
        self.other = self.person(r.allocate(0x200), game.PLACE + 1)
        # the sound thread: the sound's interface for its propagators (slot 1 the envelope, +110h its
        # length; slot 2 the description: +4 dB at 1 m, +8 its volume), the voice maker
        self.description = r.allocate(0x40)
        r.float(self.description + 8, self.VOLUME)
        envelope, envelope_vt, holder_ = r.allocate(0x200), r.allocate(0x40), r.allocate(0x10)
        loud = r.allocate(0x20)                                       # `fld 1.0 / ret 8`: full loudness
        r.float(loud + 0x10, 1.0)
        r.emu.mem_write(loud, b"\xD9\x05" + struct.pack("<I", loud + 0x10) + b"\xC2\x08\x00")
        r.word(envelope_vt + 0x1C, loud)
        r.word(envelope, envelope_vt)
        r.word(envelope + 0x110, 1000)
        r.word(holder_, envelope)
        self.interface = r.allocate(0x10)
        r.word(self.interface, r.vtable({1: (lambda r: holder_, 4), 2: (lambda r: self.description, 0)}))
        r.stub(self.voice, 0xC, at=0x59E70, name="the voice maker")
        r.stub(lambda r: 0, 0, at=0x245430, name="the weapon's played events")

    def person(self, inventory, place):
        """a player in `place` with `inventory`, his equipment's sound effect and his weapon's fire
        state's: (player, effect, shot)"""
        r, world = self.r, self.world
        # the game's player (+10Ch the inventory, +114h the place, +120h the world's core, +F474h the
        # world, +F470h the view (+358h 0: first person), +118h his transform's holder)
        player = r.allocate(0x10000)
        r.word(player + 0x10C, inventory)
        r.emu.mem_write(player + 0x114, bytes([place, 0]))
        r.word(player + 0x120, CORE)
        r.word(player + 0xF474, world)
        r.word(player + 0xF470, r.allocate(0x400))
        transform = r.allocate(0x80)
        r.word(player + 0x118, r.vtable({10: (lambda r: transform, 0)}))
        r.word(player + 0xD0, 5)                                      # his intrusive count (the camera's hold)
        r.float(player + 0x310, 1000.0)                               # no fall damage at the landing
        r.word(player + 0xF460, r.allocate(0x40))                     # the landing's steps (0x23FBE0): their
        r.floats(player + 0xF464, [1.0, 5.0])                         # sounds' table, speeds for a step and a
        r.word(player + 0xF46C, 0)                                    # hard one, the last one's time
        # his equipment's sound effect (+3Ch the player, +38h his movement, +40h the world, +44h feet set)
        effect = r.allocate(0x80)
        movement, inner = r.allocate(0x40), r.allocate(0x40)
        r.word(effect + 0x3C, player)
        r.word(effect + 0x38, inner)
        r.word(inner + 0x20, movement)
        r.word(effect + 0x40, world)
        r.emu.mem_write(effect + 0x44, b"\x01")
        # his weapon (+8 the user, +594h the user's game object, +650h its position) and its fire state
        # (weapon_core_fire_state 0xA04EE4, its weapon_sound_effect at +168h: +30h the weapon, +34h the
        # world, +0 the first person's sounds and +0Ch the ones of them playing, +18h and +24h the third
        # person's, +3Ah at most)
        weapon = r.allocate(0x800)
        r.word(weapon + 8, player)
        r.word(weapon + 0x594, player + 0x110)
        state = r.allocate(0x400)
        r.word(state, 0xA04EE4)
        shot = state + 0x168
        r.word(shot + 0x30, weapon)
        r.word(shot + 0x34, world)
        sounds = r.allocate(0x10)
        r.word(sounds, self.sound)
        for at in (0, 0x18):
            playing = r.allocate(0x40)
            r.word(shot + at, sounds)
            r.word(shot + at + 4, sounds + 4)
            r.word(shot + at + 0xC, playing)
            r.word(shot + at + 0x10, playing)
            r.word(shot + at + 0x14, playing + 0x40)
        r.emu.mem_write(shot + 0x3A, b"\x08")
        return player, effect, shot

    def take(self, r):
        proxy = r.allocate(0x90, 0xCD)
        self.made.append(proxy)
        return proxy

    def voice(self, r):
        voice, mixer = r.allocate(0x40), r.allocate(0x40)
        r.word(voice + 0x14, mixer)
        self.voices.append(mixer)
        return voice

    def view(self, player):
        """the camera on `player` in first person (his own game), or on no one (None: another's)"""
        self.r.word(self.camera + 4, player or 0)

    def step(self, first_person=False, who=None):
        """an animation's foot event of `who` (the carrier when None; 0x2A6470, event 0), in his own
        game in first person or in another's: the proxies played"""
        r = self.r
        player, effect, _ = who or self.carrier
        self.view(player if first_person else None)
        event = r.allocate(0x40)
        r.word(event, player)
        r.word(event + 0x10, r.read(CORE + 0x18FC4) + 100)           # after the world's time
        before = len(self.played)
        value, cleanup = r.thiscall(0x2A6470, effect, event)
        return self.played[before:], (value, cleanup)

    def landing(self, speed=7.5, first_person=False):
        """the carrier's player::on_landing 0x1F3B30 at `speed`, 60 ms after the last, in his own
        game in first person or in another's: the proxies played"""
        r = self.r
        self.view(self.player if first_person else None)
        r.word(self.player + 0xF46C, 0)
        r.word(self.player + 0x324, 60)
        before = len(self.played)
        r.emu.reg_write(UC_X86_REG_XMM0, struct.unpack("<I", struct.pack("<f", speed))[0])
        _, cleanup = r.call(0x1F3B30, (struct.unpack("<I", struct.pack("<f", speed))[0],),
                            {UC_X86_REG_ECX: self.player})
        return self.played[before:], cleanup

    def fire(self, first_person=False, who=None):
        """the fire state's sound of `who`'s weapon (the carrier's when None), as the timer runs it
        (play_sound_callback 0x297E20 with the effect 0x2981A0 bound, sound 0, played), in his own
        game in first person or in another's: the proxies played"""
        r = self.r
        player, _, shot = who or self.carrier
        self.view(player if first_person else None)
        before = len(self.played)
        for at in (0xC, 0x24):                                        # nothing playing
            r.word(shot + at + 4, r.read(shot + at))
        r.emu.reg_write(UC_X86_REG_EBX, 0)
        _, cleanup = r.call(0x297E20, (0, 0, 0, 1), {UC_X86_REG_ECX: shot})
        return self.played[before:], cleanup

    def heard(self, proxy):
        """a propagator of `proxy` (the game's 0x5CB00) at DISTANCE in a listener's frame (0x5C7A0):
        (its proxy, amplitude, HDR loudness, the voice's volume or None for no voice)"""
        r = self.r
        propagator = r.allocate(0x60)
        r.call(0x5CB00, (0,), {UC_X86_REG_EAX: proxy, UC_X86_REG_ECX: self.interface, UC_X86_REG_ESI: propagator})
        r.word(propagator + 0x24, 10)                                 # started 10 ms ago
        r.word(propagator + 0x2C, 1000)
        r.float(propagator + 0x38, self.DISTANCE)
        listener, span, array = r.allocate(0x400), r.allocate(8), r.allocate(4)
        r.word(array, propagator)
        r.word(span, array)
        r.word(span + 4, array + 4)
        before = len(self.voices)
        r.call(0x5C7A0, (listener, span), {UC_X86_REG_EAX: 16})
        volume = None
        if len(self.voices) > before:
            r.span(0x5C710, 0x5C748, {UC_X86_REG_EDI: propagator}, [0] * 10 + [self.description] + [0] * 8)
            volume = r.readf(self.voices[-1] + 0x2C)
        return r.read(propagator + 8), r.readf(propagator + 0x4C), r.readf(propagator + 0x40), volume


def hold(emu, game, states):
    """the Game's inventory's artefact slots 13..18: a Rattle cooked by `emu`'s cook in each state"""
    for slot in range(13, 19):
        emu.word(game.inventory + 0x10C + 4 * slot, 0)
    for slot, state in zip(range(13, 19), states):
        memory = emu.allocate(R_SIZE, 0xAB)
        emu.thiscall(0x25E660, game.cook, memory, game.resource_ptr, emu.allocate(4), game.scene)
        emu.word(memory + 0x25C, game.dict_item)
        emu.thiscall(emu.read(emu.read(memory) + 4 * 28), memory, CORE)
        emu.word(memory + STATE_AT, state)
        emu.word(game.inventory + 0x10C + 4 * slot, memory)


def real_path(check, built, original, options):
    """The sounds others hear of a carrier, from the game's own event to the
    voice, in the build and the original (SoundWorld)."""
    worlds = {}
    for name, data in (("build", built), ("original", original)):
        emu = Run(pe_sections(data))
        game = Game(emu, options)
        worlds[name] = (emu, game, SoundWorld(emu, game))
    r, game, w = worlds["build"]
    stock, stock_game, s = worlds["original"]
    full = SoundWorld.VOLUME * 2.0 / SoundWorld.DISTANCE              # 0.2: what everyone hears unquieted
    near = lambda a, b: a is not None and b is not None and abs(a - b) < 1e-4

    def one(world, played):
        """the one proxy played, made by the scene's pool, as a listener hears it: (the propagator's
        proxy is it, its quiet, amplitude, HDR loudness, the voice's volume, its kind +80h: 0 a point
        proxy, 3 a hud one)"""
        if len(played) != 1 or played[0] != world.made[-1]:
            return None
        proxy, amplitude, loudness, volume = world.heard(played[0])
        return (proxy == played[0], world.r.read(played[0] + QUIET, 2), amplitude, loudness, volume,
                world.r.read(played[0] + HUD_KIND))

    heard = {}
    for states in ((), (3,), (4,)):
        hold(r, game, states)
        played, (value, cleanup) = w.step()
        heard[states] = one(w, played), value, cleanup, r.read(STATE)
    none, passive, active = (heard[k][0] for k in ((), (3,), (4,)))
    hold(stock, stock_game, (4,))
    stock_step = one(s, s.step()[0])
    check("a step others hear, from the carrier's animation event (on_player_sound_event 0x2A6470, a foot: "
          "on_foot_step, the step manager's on_step 0x248AD0, play_sound 0x657C0, the pool's point proxy "
          "0x64F80 and its constructor, its position and play) to the voice a listener's frame makes of it "
          "(the propagator 0x5CB00 of the proxy played, calculate_hdr_audio_frame 0x5C7A0, the voice's volume "
          "0x633F0) at 8 m: no Rattle 0.2 as the original's (an active Rattle changes nothing there), a "
          "passive Rattle 0.14 (x0.7), an active one no voice (amplitude 0); eax 0, ret 4, nothing pending "
          "after",
          none and passive and active and stock_step and none[0] and passive[0] and active[0]
          and [none[1], passive[1], active[1]] == [0, quiet_of(0.7), 0xFFFF]
          and near(none[4], full) and near(stock_step[4], full) and stock_step[1] == 0
          and near(passive[4], full * 0.7) and near(passive[2], 0.25 * 0.7)
          and active[4] is None and active[2] == 0.0
          and abs(passive[3] - (none[3] + 20 * math.log10(0.7))) < 1e-3
          and all(v[1:] == (0, 4, 0) for v in heard.values()),
          f"volumes {none and none[4]}, {passive and passive[4]}, {active and active[4]}; the original "
          f"{stock_step and stock_step[4]}")

    hold(r, game, (3, 3))
    played, cleanup = w.landing()
    landing = one(w, played)
    check("a landing others hear, from player::on_landing 0x1F3B30 (its step 0x23FBE0, on_step, 0x657C0) to "
          "the voice: two passive Rattles 0.098 (x0.49); ret 4, nothing pending after",
          landing and landing[0] and landing[1] == quiet_of(0.49) and near(landing[4], full * 0.49)
          and cleanup == 4 and r.read(STATE) == 0, str(landing))

    shots = {}
    for states in ((), (3,), (4, 4)):
        hold(r, game, states)
        played, cleanup = w.fire()
        shots[states] = one(w, played), cleanup
    hold(stock, stock_game, (4, 4))
    stock_shot = one(s, s.fire()[0])
    bare, passive_shot, two_active = (shots[k][0] for k in ((), (3,), (4, 4)))
    check("a shot others hear, from the fire state's sound as its timer runs it (play_sound_callback 0x297E20 "
          "with the weapon_sound_effect at +168h of weapon_core_fire_state, emit_point_sound 0x65670, the "
          "pool's proxy, its position and play) to the voice: no Rattle 0.2 as the original's, a passive one "
          "0.2 (the passive leaves shots), two active 0.05 (x0.25); ret 10h",
          bare and passive_shot and two_active and stock_shot and two_active[0]
          and near(bare[4], full) and near(passive_shot[4], full) and near(stock_shot[4], full)
          and two_active[1] == quiet_of(0.25) and near(two_active[4], full * 0.25)
          and all(v[1] == 0x10 for v in shots.values()),
          f"volumes {bare and bare[4]}, {passive_shot and passive_shot[4]}, {two_active and two_active[4]}; "
          f"the original {stock_shot and stock_shot[4]}")

    # The carrier's own game (its camera his, in first person): his steps and shots are hud proxies
    # (0x65100, kind 3), quieted as the others hear him (the owner, 2026-10-24: the same for himself).
    own = {}
    for states in ((), (3,), (4,)):
        hold(r, game, states)
        played, (value, cleanup) = w.step(first_person=True)
        own[states] = one(w, played), value, cleanup, r.read(STATE)
    none, passive, active = (own[k][0] for k in ((), (3,), (4,)))
    hold(stock, stock_game, (4,))
    stock_own = one(s, s.step(first_person=True)[0])
    check("the carrier's own step in his own game (in first person: on_step's play_sound 0x657C0, the hud "
          "proxy 0x65100 at 0x65803, kind 3) to its voice: no Rattle 0.2 as the original's (an active Rattle "
          "changes nothing there), a passive Rattle 0.14 (x0.7), an active one no voice: as the others hear "
          "him; eax 0, ret 4, nothing pending after",
          none and passive and active and stock_own and none[0] and passive[0] and active[0]
          and [none[5], passive[5], active[5], stock_own[5]] == [3, 3, 3, 3]
          and [none[1], passive[1], active[1], stock_own[1]] == [0, quiet_of(0.7), 0xFFFF, 0]
          and near(none[4], full) and near(stock_own[4], full) and near(passive[4], full * 0.7)
          and active[4] is None and all(v[1:] == (0, 4, 0) for v in own.values()),
          f"volumes {none and none[4]}, {passive and passive[4]}, {active and active[4]}; the original "
          f"{stock_own and stock_own[4]}")

    hold(r, game, (3, 3))
    played, cleanup = w.landing(first_person=True)
    landing = one(w, played)
    check("the carrier's own landing in his own game (on_landing's step, on_step, the hud proxy): two passive "
          "Rattles 0.098 (x0.49); ret 4, nothing pending after",
          landing and landing[0] and landing[5] == 3 and landing[1] == quiet_of(0.49)
          and near(landing[4], full * 0.49) and cleanup == 4 and r.read(STATE) == 0, str(landing))

    shots = {}
    for states in ((), (3,), (4, 4)):
        hold(r, game, states)
        played, cleanup = w.fire(first_person=True)
        shots[states] = one(w, played), cleanup
    hold(stock, stock_game, (4, 4))
    stock_shot = one(s, s.fire(first_person=True)[0])
    bare, passive_shot, two_active = (shots[k][0] for k in ((), (3,), (4, 4)))
    check("the carrier's own shot in his own game (play_sound_callback's first person: the hud proxy 0x65100 "
          "at 0x297F5E, kind 3) to its voice: no Rattle 0.2 as the original's, a passive one 0.2 (the passive "
          "leaves shots), two active 0.05 (x0.25): as the others hear him; ret 10h",
          bare and passive_shot and two_active and stock_shot and two_active[0]
          and [bare[5], passive_shot[5], two_active[5], stock_shot[5]] == [3, 3, 3, 3]
          and near(bare[4], full) and near(passive_shot[4], full) and near(stock_shot[4], full)
          and two_active[1] == quiet_of(0.25) and near(two_active[4], full * 0.25)
          and all(v[1] == 0x10 for v in shots.values()),
          f"volumes {bare and bare[4]}, {passive_shot and passive_shot[4]}, {two_active and two_active[4]}; "
          f"the original {stock_shot and stock_shot[4]}")

    # Another player's own game while the carrier's two Rattles are active: his own sounds as they were.
    hold(r, game, (4, 4))
    step_played, (value, cleanup) = w.step(first_person=True, who=w.other)
    his_step = one(w, step_played)
    shot_played, shot_cleanup = w.fire(first_person=True, who=w.other)
    his_shot = one(w, shot_played)
    check("another player's own step and shot in his own game (hud proxies), the carrier's two Rattles "
          "active: not marked, 0.2 each; eax 0, ret 4 and 10h, nothing pending after",
          his_step and his_shot and his_step[0] and his_shot[0] and [his_step[5], his_shot[5]] == [3, 3]
          and [his_step[1], his_shot[1]] == [0, 0] and near(his_step[4], full) and near(his_shot[4], full)
          and (value, cleanup, shot_cleanup, r.read(STATE)) == (0, 4, 0x10, 0),
          f"volumes {his_step and his_step[4]}, {his_shot and his_shot[4]}")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("original")
    parser.add_argument("--pack", default=os.environ.get("SRP_RATTLE_PACK", PACK),
                        help="the content pack's folder (rattle@3: private/content-packs/rattle-3)")
    args = parser.parse_args()
    original = open(args.original, "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    options = bcw.dump(rattle_options())
    results = []

    def check(what, ok, detail=""):
        results.append(bool(ok))
        print(f"  {'ok' if ok else 'FAIL'}  {what}" + (f": {detail}" if detail else ""))

    run, stock = Run(pe_sections(built)), Run(pe_sections(original))
    game, stock_game = Game(run, options), Game(stock, options)
    r = run
    vt = lambda emu, obj, slot: emu.read(emu.read(obj) + 4 * slot)
    pending = lambda: r.read(STATE)
    # Each slot of ours, as entered: its name and the return address on the stack (who called it).
    entered = []
    for name in ("rt_ui_info", "rt_serialize", "rt_deserialize", "rt_tick_impl", "rt_enable_active",
                 "rt_disable_active", "rt_active_effect_ended"):
        r.emu.hook_add(UC_HOOK_CODE, lambda emu, address, size, name: entered.append(
            (name, r.read(emu.reg_read(UC_X86_REG_ESP)))), name, begin=SYMBOLS[name], end=SYMBOLS[name])
    called_from = lambda name, start, end: [ret for n, ret in entered if n == name and start <= ret < end]
    fire_kind = ARENA.blocks["artefacts.code"]

    # 1. The cook: the size and the object.
    size, cleanup = r.thiscall(0x25E770, game.cook, game.resource_ptr)
    stock_size, _ = stock.thiscall(0x25E770, stock_game.cook, stock_game.resource_ptr)
    check("the artefact cook's size for type 8 is the Rattle's (378h; the original 360h), ret 4",
          size == R_SIZE and cleanup == 4 and stock_size == 0x360, f"{size:#x}, the original {stock_size:#x}")

    def make(emu, fixture, slot=None):
        memory = emu.allocate(R_SIZE, 0xAB)
        made, cleanup = emu.thiscall(0x25E660, fixture.cook, memory, fixture.resource_ptr, emu.allocate(4),
                                     fixture.scene)
        emu.word(memory + 0x25C, fixture.dict_item)
        emu.thiscall(vt(emu, memory, 28), memory, CORE)
        if slot is not None:
            emu.emu.mem_write(memory + 0x260, struct.pack("<H", 1))
            emu.word(memory + 0xD0, 2)
            emu.word(fixture.inventory + 0x10C + 4 * slot, memory)
            emu.thiscall(vt(emu, memory, 10), memory, fixture.inventory, slot)
            emu.thiscall(vt(emu, memory, 12), memory)
        return memory, made, cleanup

    rattle, made, cleanup = make(r, game)
    stock_rattle, _, _ = make(stock, stock_game)
    check("construct for type 8: Rattle's object (its drawable's vtable 0xA061B8, the scene) under the Rattle's "
          "vtable; the original's vtable 0xA06124; ret 10h",
          made == rattle and cleanup == 0x10 and r.read(rattle) == SYMBOLS["rt_vtable"] + 4
          and r.read(rattle + 0x340) == 0xA061B8 and r.read(rattle + 0x34C) == game.scene
          and stock.read(stock_rattle) == 0xA06124 and r.read(rattle + 0x27C, 2) == 0xFFFF
          and r.read(rattle + 0x274) == 40000 and r.read(rattle + 0x278) == 1000,
          f"vtable {r.read(rattle):#x}, amount {r.read(rattle + 0x27C, 2):#x}, cooldown {r.read(rattle + 0x274)}, "
          f"spawn {r.read(rattle + 0x278)}")
    check("the config's numbers: duration 10000 ms (data/active), passive steps 0.7, active steps 0, active "
          "shots 0.5, nothing left of an active",
          r.read(rattle + R_DURATION) == 10000 and r.readf(rattle + R_PASSIVE_STEPS) == f32(0.7)
          and r.readf(rattle + R_ACTIVE_STEPS) == 0.0 and r.readf(rattle + R_ACTIVE_SHOTS) == 0.5
          and r.read(rattle + R_LEFT) == 0)
    from check_sponge_a_026g import config_resource, sponge_options
    pointer = config_resource(r, CONFIG + 0x8000, sponge_options())
    memory = r.allocate(0x700, 0xAB)
    made, _ = r.thiscall(0x25E660, game.cook, memory, pointer, r.allocate(4), game.scene)
    check("type 12 is still the Sponge's (the Sponge's switch before the type 8 case)",
          made == memory and r.read(memory) == SPONGE_SYMBOLS["sp_vtable"] + 4, f"{r.read(memory):#x}")

    # 2. In an inventory, the press, the active's ticks; the original beside it.
    first, _, _ = make(r, game, 13)
    second, _, _ = make(r, game, 14)
    stock_one, _, _ = make(stock, stock_game, 13)
    check("picked up: state 3 (passive)", r.read(first + STATE_AT) == 3 and r.read(second + STATE_AT) == 3
          and stock.read(stock_one + STATE_AT) == 3)
    game.modifier(39, 0.2)
    entered.clear()
    _, cleanup = r.thiscall(vt(r, first, 11), first, 1, 100000)
    check("driven: slot 32, enable_active(time), through artefact_base::action 0x313300 (patch 54's press of "
          "every ready one of the kind, its block artefacts.code): both Rattles active from it (state 4, +2A0h "
          "the time), each for its duration as the holder's artefact_duration modifier makes it (10000 x 1.2 = "
          "12000 ms); ret 8",
          cleanup == 8 and len(called_from("rt_enable_active", fire_kind.va, fire_kind.va + fire_kind.size)) == 2 and [r.read(x + STATE_AT) for x in (first, second)] == [4, 4]
          and [r.read(x + ACTIVATED) for x in (first, second)] == [100000, 100000]
          and [r.read(x + R_LEFT) for x in (first, second)] == [12000, 12000],
          str([(r.read(x + STATE_AT), r.read(x + R_LEFT)) for x in (first, second)]))
    game.modifier(39, 0.0)
    stock.thiscall(vt(stock, stock_one, 11), stock_one, 1, 100000)
    stock.thiscall(vt(stock, stock_one, 29), stock_one, 50, 100050)
    check("the original's Rattle: active (state 4) at the press, back to its passive at the first tick with its "
          "cooldown (40000 ms) running: 0.20e's stub active_effect_ended is true at once",
          stock.read(stock_one + STATE_AT) == 3 and stock.read(stock_one + COOLDOWN) == 40000,
          f"state {stock.read(stock_one + STATE_AT)}, cooldown {stock.read(stock_one + COOLDOWN)}")
    info = r.allocate(8)
    r.thiscall(vt(r, first, 18), first, info)
    check("the hud's info while active: artefact_base's (the dict id, the count) and the seconds left, ceil(12000 / "
          "1000) = 12", r.read(info + 2, 2) == 12 and r.read(info + 4, 2) == 1, r.bytes(info, 8).hex())
    time, ticks = 100000, 0
    entered.clear()
    while r.read(first + STATE_AT) == 4 and ticks < 1000:
        time += 50
        r.thiscall(vt(r, first, 29), first, 50, time)
        ticks += 1
        if ticks == 239:
            left_before_end = r.read(first + R_LEFT)
    check("ticked every 50 ms: active for 240 ticks (12000 ms; 50 left at the 239th), then its passive (state 3) "
          "with the cooldown cooldown_sec x 1000 (40000)",
          ticks == 240 and left_before_end == 50 and r.read(first + STATE_AT) == 3 and r.read(first + COOLDOWN) == 40000
          and r.read(first + R_LEFT) == 0, f"{ticks} ticks, state {r.read(first + STATE_AT)}, "
          f"cooldown {r.read(first + COOLDOWN)}")
    check("driven: slot 30, tick_impl(dt, time), through artefact_base::tick 0x313200: called by it at each of "
          "the 240 ticks, the time left taken down by dt (50), not by the time",
          len(called_from("rt_tick_impl", 0x313200, 0x313300)) == 240 and left_before_end == 50)
    check("driven: slot 35, active_effect_ended, through artefact_base::tick 0x313200: asked at each tick, true "
          "at the 240th (the cooldown started then, not before)",
          len(called_from("rt_active_effect_ended", 0x313200, 0x313300)) == 240 and r.read(first + COOLDOWN) == 40000)
    check("driven: slot 34, disable_active, through artefact_base::decrement_amount 0x313530: once, at the "
          "active's end (amount -1: the Rattle is not used up)",
          len(called_from("rt_disable_active", 0x313530, 0x313600)) == 1 and len(entered) == 240 * 2 + 1
          and r.read(first + 0x27C, 2) == 0xFFFF, str(sorted(set((n, hex(a)) for n, a in entered))))
    r.thiscall(vt(r, first, 18), first, info)
    check("the hud's timer at 0 after it", r.read(info + 2, 2) == 0)
    _, cleanup = r.thiscall(vt(r, first, 11), first, 1, time)
    check("a press while cooling fires the ready one (the second, still active, is not): nothing new",
          cleanup == 8 and r.read(first + STATE_AT) == 3 and r.read(second + STATE_AT) == 4)

    # 3. Serialization: the time left in state 4, nothing more in the others.
    def write(emu, obj):
        writer, buffer = emu.allocate(0x20), emu.allocate(0x10)
        emu.word(writer + 0x14, buffer)
        emu.word(buffer + 4, emu.allocate(0x10))
        emu.word(buffer + 8, 0x10)
        emu.emu.mem_write(0x1F1D32C, b"\x00\x00")
        _, cleanup = emu.thiscall(vt(emu, obj, 26), obj, writer, 0)
        return emu.bytes(emu.read(buffer + 4), emu.read(buffer + 0xC)), cleanup

    def load(emu, obj, blob):
        reader, source = emu.allocate(8), emu.allocate(len(blob) + 16)
        emu.emu.mem_write(source, blob)
        emu.word(reader + 4, source)
        _, cleanup = emu.thiscall(vt(emu, obj, 27), obj, reader, 0, 0)
        return emu.read(reader + 4) - source, cleanup

    def resize(emu):                                   # a writer's buffer grows (the Sponge's check's)
        buffer, size = emu.emu.reg_read(UC_X86_REG_EAX), emu.arg(0)
        if size > emu.read(buffer + 8):
            grown = emu.allocate(size + 0x400)
            emu.emu.mem_write(grown, emu.bytes(emu.read(buffer + 4), emu.read(buffer + 0xC)))
            emu.word(buffer + 4, grown)
            emu.word(buffer + 8, size + 0x400)
        emu.word(buffer + 0xC, size)
    for emu in (r, stock):
        emu.stub(resize, 4, at=0x7AB30)
    r.word(second + R_LEFT, 4321)
    active_blob, cleanup = write(r, second)
    passive_blob, _ = write(r, first)
    stock_blob, _ = write(stock, stock_one)
    fresh, _, _ = make(r, game)
    read, read_cleanup = load(r, fresh, active_blob)
    check("serialize in state 4: artefact_base's then the u32 time left (4321); deserialize reads it all back "
          "(state 4, the time, +2A0h); ret 8 and 0Ch",
          cleanup == 8 and read_cleanup == 0xC and read == len(active_blob) and active_blob.endswith(struct.pack("<I", 4321))
          and r.read(fresh + STATE_AT) == 4 and r.read(fresh + R_LEFT) == 4321 and r.read(fresh + ACTIVATED) == 100000,
          f"{len(active_blob)} bytes, read {read}")
    again, _ = write(r, fresh)
    r.word(fresh + R_LEFT, 777)
    read, _ = load(r, fresh, passive_blob)
    check("in state 3 artefact_base's alone (the original's Rattle's bytes, its length); loaded, nothing left of "
          "an active", again == active_blob and len(passive_blob) == len(stock_blob) and read == len(passive_blob)
          and r.read(fresh + STATE_AT) == 3 and r.read(fresh + R_LEFT) == 0,
          f"{len(passive_blob)} and the original {len(stock_blob)} bytes")

    # 4. The quiet of a player's steps and shots.
    rt_steps, rt_shot = SYMBOLS["rt_steps"], SYMBOLS["rt_shot"]
    slots = [game.inventory + 0x10C + 4 * s for s in range(13, 19)]

    def holding(states):
        """the player's artefact slots: a Rattle in each given state"""
        for at in slots:
            r.word(at, 0)
        for at, state in zip(slots, states):
            obj, _, _ = make(r, game)
            r.word(obj + STATE_AT, state)
            r.word(at, obj)

    def steps_quiet(player):
        _, cleanup = r.call(rt_steps, (player,))
        return pending(), cleanup

    weapon, handler, pistol, other = r.allocate(0x600), r.allocate(0x400), r.allocate(0x400), r.allocate(0x400)
    r.word(weapon + 8, game.player)
    for obj, vtable, at in ((handler, 0xA04EE4, 0x168), (pistol, 0xA05248, 0x170), (other, 0xA04E00, 0x168)):
        r.word(obj, vtable)
        r.word(obj + at + 0x30, weapon)

    def shot_quiet(effect):
        proxy, out = r.allocate(0x90), r.allocate(4)
        r.word(out, proxy)
        _, cleanup = r.call(rt_shot, (effect, out))
        return r.read(proxy + QUIET, 2), cleanup

    cases = []
    for states, steps, shots in (((), 1.0, 1.0), ((3,), 0.7, 1.0), ((3, 3), 0.49, 1.0), ((3, 3, 3), 0.343, 1.0),
                                 ((4,), 0.0, 0.5), ((4, 4), 0.0, 0.25), ((4, 4, 4, 4), 0.0, 0.0625),
                                 ((4, 3), 0.0, 0.5), ((5, 5), 1.0, 1.0)):
        holding(states)
        step_q, step_cleanup = steps_quiet(game.player)
        shot_q, shot_cleanup = shot_quiet(handler + 0x168)
        g_steps = 1.0
        for s in states:
            g_steps *= {3: f32(0.7), 4: 0.0}.get(s, 1.0)
        cases.append((states, step_q, quiet_of(g_steps), shot_q, quiet_of(shots), step_cleanup, shot_cleanup))
        check(f"Rattles in states {states or 'none'}: steps x{steps:g} (quiet {step_q}), shots x{shots:g} "
              f"(quiet {shot_q}); stdcall, 4 and 8",
              step_q == quiet_of(g_steps) and shot_q == quiet_of(shots) and step_cleanup == 4 and shot_cleanup == 8,
              f"expected {quiet_of(g_steps)}, {quiet_of(shots)}")
    holding((4,))
    check("a pistol's fire state (its effect at +170h, 0xA05248) is a shot too; another handler state's sound "
          "(reload, show, ...) and a weapon without a user are not quieted",
          shot_quiet(pistol + 0x170)[0] == quiet_of(0.5) and shot_quiet(other + 0x168)[0] == 0
          and (r.word(weapon + 8, 0) or shot_quiet(handler + 0x168)[0] == 0))
    r.word(weapon + 8, game.player)
    check("percent for others (gain, from the quiet): 70 / 49 / 34.3 % steps, 50 / 25 / 6.25 % shots",
          [round(1 - c[1] / 65535, 4) for c in cases[1:4]] == [0.7, 0.49, 0.343]
          and [round(1 - c[3] / 65535, 4) for c in cases[4:7]] == [0.5, 0.25, 0.0625])

    # 5. The hooks in the game's code.
    holding((3,))
    # on_player_sound_event: the steps' quiet pending while it runs, none after
    seen = []
    r.emu.hook_add(UC_HOOK_CODE, lambda emu, address, size, user: seen.append(r.read(STATE)),
                   begin=0x2A6475, end=0x2A6475)
    sound_effect, event, someone = r.allocate(0x80), r.allocate(0x40), r.allocate(0x200)
    r.word(sound_effect + 0x3C, game.player)
    r.word(game.player + 0x120, 0)
    r.word(event, someone)
    value, cleanup = r.thiscall(0x2A6470, sound_effect, event)
    check("on_player_sound_event (0x2A6470): runs with the player's steps' quiet pending (quiet of 70 %), its own "
          "code after the entry's two instructions; nothing pending after it; eax 0, ret 4",
          seen == [quiet_of(0.7)] and pending() == 0 and value == 0 and cleanup == 4, f"{seen}, {pending()}")

    # 0x657C0's point proxy, marked with what is pending
    made_proxies = []

    def new_point(emu):
        proxy = emu.allocate(0x90)
        made_proxies.append(proxy)
        emu.word(emu.arg(0), proxy)
        return emu.arg(0)
    r.stub(new_point, 0x10, at=0x64F80, name="new_point_sound_instance_proxy")
    scene_ptr, scene, sound, position = r.allocate(4), r.allocate(0x100), r.allocate(0x200), r.allocate(16)
    r.word(scene_ptr, scene)
    r.word(sound, r.vtable({7: (lambda emu: 0, 0)}))

    def point(quiet):
        r.word(STATE, quiet)
        eip, _ = r.span(0x657C0, 0x65879, {UC_X86_REG_ECX: scene_ptr, UC_X86_REG_EAX: 0},
                        stack=(0xDEAD, sound, position, 0))
        return eip, r.read(made_proxies[-1] + QUIET, 2)
    marked, unmarked = point(quiet_of(0.49)), point(0)
    r.word(STATE, 0)
    check("0x657C0 (a step's or a rustle's sound for another player): its point proxy marked with the pending "
          "quiet; none pending, none marked",
          marked == (0x65879, quiet_of(0.49)) and unmarked == (0x65879, 0), f"{marked}, {unmarked}")

    # 0x657C0's hud proxy (its flag set: one's own step in first person), the same
    hud_scenes = []

    def new_hud(emu):
        hud_scenes.append(emu.emu.reg_read(UC_X86_REG_EAX))
        return new_point(emu)
    r.stub(new_hud, 0x10, at=0x65100, name="new_hud_sound_instance_proxy")

    def hud(quiet):
        r.word(STATE, quiet)
        eip, _ = r.span(0x657C0, 0x65808, {UC_X86_REG_ECX: scene_ptr, UC_X86_REG_EAX: 0},
                        stack=(0xDEAD, sound, position, 1))
        out = r.emu.reg_read(UC_X86_REG_EAX)
        return eip, r.read(out) == made_proxies[-1], r.read(made_proxies[-1] + QUIET, 2)
    marked, unmarked = hud(quiet_of(0.49)), hud(0)
    r.word(STATE, 0)
    check("0x657C0 with its hud flag (one's own step or rustle in first person): its hud proxy (0x65100 at "
          "0x65803, eax the scene) marked with the pending quiet; none pending, none marked; eax &the proxy",
          marked == (0x65808, True, quiet_of(0.49)) and unmarked == (0x65808, True, 0)
          and hud_scenes == [scene, scene], f"{marked}, {unmarked}")

    # play_sound_callback 0x297E20 to its point proxy: a fire state's effect, the weapon's user's shot quiet
    holding((4, 4))

    def emit_point(emu):
        proxy = emu.allocate(0x90)
        made_proxies.append(proxy)
        emu.word(emu.arg(0), proxy)
        return emu.arg(0)
    r.stub(emit_point, 8, at=0x65670, name="emit_point_sound")
    world, client, camera, holder_ = r.allocate(0x200), r.allocate(0x6000), r.allocate(0x10), r.allocate(0x10)
    r.word(world + 0x9C, client)
    r.word(client + 0x53AC, camera)
    r.word(camera + 4, 0)                                            # no player in first person here
    r.word(client + 0x5350, holder_)
    r.word(holder_, r.vtable({2: (lambda emu: 0x1234, 0)}))
    owner = r.allocate(0x10)
    r.word(weapon + 0x594, owner)
    effect = handler + 0x168
    sounds = r.allocate(0x10)
    r.word(sounds, sound)
    r.word(effect + 0x18, sounds)
    r.word(effect + 0x34, world)

    def callback(effect_at):
        r.word(effect_at + 0x18, sounds)
        r.word(effect_at + 0x34, world)
        r.emu.reg_write(UC_X86_REG_EBX, 0)
        r.call(0x297E20, (0, 0, 0, 1), {UC_X86_REG_ECX: effect_at}, until=0x297F7E)
        out = r.emu.reg_read(UC_X86_REG_EAX)
        return r.read(out) == made_proxies[-1], r.read(made_proxies[-1] + QUIET, 2)
    got = callback(effect)
    r.word(other + 0x168 + 0x30, weapon)
    reload = callback(other + 0x168)
    check("play_sound_callback (0x297E20) for another player's weapon: its point proxy (0x65670 at 0x297F79, the "
          "effect at [esp+2Ch] there) marked with the user's shot quiet (two active: 25 %); another state's "
          "sound not; eax as 0x65670 left it (&the proxy)",
          got == (True, quiet_of(0.25)) and reload == (True, 0),
          f"{got}, {reload}")

    # on_landing's step: everything 0x23FBE0 takes passed on, the quiet pending only during it
    landing = []

    def landing_step(emu):
        landing.append((emu.emu.reg_read(UC_X86_REG_ECX) & 0xFF, emu.emu.reg_read(UC_X86_REG_EDX),
                        emu.emu.reg_read(UC_X86_REG_XMM0) & 0xFFFFFFFF, [emu.arg(i) for i in range(4)], emu.read(STATE)))
        return 0
    r.stub(landing_step, 0x10, at=0x23FBE0, name="landing step")
    holding((3, 3))
    r.emu.reg_write(UC_X86_REG_XMM0, struct.unpack("<I", struct.pack("<f", 7.5))[0])
    _, cleanup = r.call(SYMBOLS["rattle_landing"], (11, 22, 33, 44),
                        {UC_X86_REG_ECX: 0x1, UC_X86_REG_EDX: 123456, UC_X86_REG_EDI: game.player})
    check("on_landing's step (0x1F3BB6): 0x23FBE0 gets cl, edx, xmm0 and its four arguments as they were, with "
          "the player's steps' quiet (49 %) pending; nothing pending after; ret 10h",
          landing == [(1, 123456, struct.unpack("<I", struct.pack("<f", 7.5))[0], [11, 22, 33, 44], quiet_of(0.49))]
          and pending() == 0 and cleanup == 0x10, str(landing))

    # 6. The sound thread: a propagator's amplitude and HDR loudness by its proxy's quiet.
    propagator, proxy = r.allocate(0x50), r.allocate(0x90)
    r.word(propagator + 8, proxy)

    def sound_store(site, until, field, value, quiet):
        r.emu.mem_write(proxy + QUIET, struct.pack("<H", quiet))
        r.emu.reg_write(UC_X86_REG_XMM0, struct.unpack("<I", struct.pack("<f", value))[0])
        top = (r.emu.reg_read(UC_X86_REG_FPSW) >> 11) & 7
        eip, moved = r.span(site, until, {UC_X86_REG_ESI: propagator})
        return eip, moved, r.readf(propagator + field), top == (r.emu.reg_read(UC_X86_REG_FPSW) >> 11) & 7

    amps = [sound_store(0x5CA37, 0x5CA3C, 0x4C, 0.8, quiet) for quiet in (0, quiet_of(0.7), quiet_of(0.25), 0xFFFF)]
    check("0x5CA37 (calculate_hdr_audio_frame, sound thread): the amplitude stored x the proxy's gain: 0.8 as it "
          "was, x0.7, x0.25, 0 silent; on to 0x5CA3C, the stack and the x87 stack as they were",
          all(a[0] == 0x5CA3C and a[1] == 0 and a[3] for a in amps)
          and all(abs(a[2] - w) < 2e-5 for a, w in zip(amps, (0.8, 0.56, 0.2, 0.0))) and amps[0][2] == f32(0.8)
          and amps[3][2] == 0.0, str([round(a[2], 6) for a in amps]))
    louds = [sound_store(0x5C8BA, 0x5C8BF, 0x40, -20.0, quiet) for quiet in (0, quiet_of(0.7), quiet_of(0.5), 0xFFFF)]
    want = [-20.0, -20.0 + 20 * math.log10(0.7), -20.0 + 20 * math.log10(0.5), -120.0]
    check("0x5C8BA: the HDR loudness stored + 20 log10(gain): -20 dB as it was, -23.1, -26.0, -120 silent; on to "
          "0x5C8BF, the stacks as they were",
          all(a[0] == 0x5C8BF and a[1] == 0 and a[3] for a in louds)
          and all(abs(a[2] - w) < 1e-3 for a, w in zip(louds, want)), str([round(a[2], 4) for a in louds]))

    # 7. Each slot of ours that no case above reaches through the game's own caller.
    print("  through the game's own call sites")
    shown = r.allocate(4)
    r.word(second + R_LEFT, 4321)
    r.word(shown, second)
    entered.clear()
    eip, moved = r.span(0x23A25B, 0x23A267, {UC_X86_REG_ECX: second})
    info_at = STACK + 0x8000 + 8                       # esp+8 at the site (Run.span: esp at STACK+8000h)
    check("driven: slot 18, fill_ui_info(info), through the hud's quick slot 0x23A25B (`mov edx,[ecx] / mov "
          "edx,[edx+48h] / lea eax,[esp+8] / push eax / call edx`): artefact_base's then ceil(4321 / 1000) = 5 "
          "seconds at +2, the stack as it was",
          eip == 0x23A267 and moved == 0 and r.read(info_at + 2, 2) == 5 and r.read(info_at + 4, 2) == 1
          and called_from("rt_ui_info", 0x23A267, 0x23A268), r.bytes(info_at, 8).hex())
    holder = r.allocate(0x200)                         # an anomaly core's +118h: its artefacts at +98h, count +9Ch

    def artefacts_of(*objs):
        array = r.allocate(4 * len(objs))
        for i, obj in enumerate(objs):
            r.word(array + 4 * i, obj)
        r.word(holder + 0x98, array)
        r.word(holder + 0x9C, len(objs))
    direct, _ = write(r, second)
    writer, buffer = r.allocate(0x20), r.allocate(0x10)
    r.word(writer + 0x14, buffer)
    r.word(buffer + 4, r.allocate(0x10))
    r.word(buffer + 8, 0x10)
    artefacts_of(second)
    eip, moved = r.span(0x31B074, 0x31B089, {UC_X86_REG_EBX: holder, UC_X86_REG_ESI: 0, UC_X86_REG_EDI: writer,
                                              UC_X86_REG_EBP: r.allocate(0x20)})
    through = r.bytes(r.read(buffer + 4), r.read(buffer + 0xC))
    check("driven: slot 26, serialize(writer, dumper), through generic_anomaly_core::serialize's loop 0x31B074 (a "
          "Rattle in an anomaly's list, active): the same bytes as written directly, the time left last",
          eip == 0x31B089 and moved == 0 and through == direct and through.endswith(struct.pack("<I", 4321)),
          f"{len(through)} / {len(direct)} bytes")
    fresh, _, _ = make(r, game)
    reader, source = r.allocate(8), r.allocate(len(direct) + 16)
    r.emu.mem_write(source, direct)
    r.word(reader + 4, source)
    artefacts_of(fresh)
    eip, moved = r.span(0x31B1A0, 0x31B1BB, {UC_X86_REG_EDI: holder, UC_X86_REG_ESI: 0, UC_X86_REG_EBX: reader},
                        [0] * 8)
    again, _ = write(r, fresh)
    check("driven: slot 27, deserialize(reader, a, b), through generic_anomaly_core::deserialize's loop 0x31B1A0: "
          "every byte read, the same state written again (state 4, 4321 ms left)",
          eip == 0x31B1BB and moved == 0 and r.read(reader + 4) - source == len(direct) and again == direct
          and r.read(fresh + STATE_AT) == 4 and r.read(fresh + R_LEFT) == 4321)

    # 8. The content pack's own config.
    pack_config = os.path.join(args.pack, "files", "gameplay", "items", "artefacts", "artefact_rattle.options")
    if os.path.isfile(pack_config):
        data = open(pack_config, "rb").read()
        check("the pack's artefact_rattle.options is the config these cases ran on, value for value",
              bcw.loads(data) == bcw.loads(options))
    else:
        print(f"  --  the pack's config not read (no {pack_config})")

    # 9. The real path: what a listener hears, from the game's own event to the voice.
    print("  from the carrier's event to the voice a listener hears")
    real_path(check, built, original, options)

    failed = results.count(False)
    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
