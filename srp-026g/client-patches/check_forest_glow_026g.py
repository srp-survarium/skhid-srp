#!/usr/bin/env python3
"""The legs' glows (patch 111, rust/patches/src/forest_glow.rs;
docs/notes/artefact-skills.md, "Forest Protection" and "Leg quarters") run in
the built exe.

    python check_forest_glow_026g.py <original survarium.exe>

Requires Python `unicorn`. On the medical checks' player fixture (players
with profiles and kits, their world core; for the quarters the artefact
checks' match around it, with containers and Resonance carriers), the
player's simulation tick (`battery.carrier_tick` 0x2DAAD3, the whole chain)
and the default clock (0x2E5D80, Medic's handler, then the glow's) are run as
the game runs them, with the game's own query_resources stubbed (recorded)
and its game effect player's add 0x2DE090 stubbed (recorded); the rest
native: the boost function bind, its invoker (the query's callback), the
modifier getter, game_effect_resource::emit (its emitter stubbed). It checks:

- the first tick asks for the thirteen effects once, in one query
  (forest_protection, then artefact_leg_<spring, lifebone, onyx>_<1..4>,
  class 109 each, no user data, tag 1); until they arrive nothing plays;
  when they arrive one reference is taken on each;
- Forest Protection's glow on both kits, on two places: a tick plays it
  (emitted from the kept resource, at the player's time, kind 2, the
  player's own calculator +394h) exactly when the player is alive, inside a
  damage zone (+2E4h 1, 0 none) and its current kit's modifier 41 is a rank
  or more; a player that already plays it is not given another; the
  calculator ends it when the condition ends; any other effect of that
  calculator goes on through the native calculation unchanged;
- the leg quarters: own passives 0 to 4, an active Onyx (no passive), own
  and Resonance's shared copies of a second kind, an enemy's (none) and the
  cap of 4: a tick lights each kind's quarters 1..copies; the count dropping
  (shared copies leaving, the carrier dying, an own copy going) and the
  player's death end the quarters above it through the calculator;
- the effect cook gives each effect 20 instances (and still
  artefact_lifebone 20, any other effect its own count);
- a place's third-person presenter (made by its constructor 0x297570) on its
  current kit: player::initialize (0x1F2540, its own code up to its call of
  set_effect_presenter) gives it only to an object whose effects were never
  given anywhere, so a respawn in a kit the place had before leaves it on
  the other kit's object; the tick of the place's current kit's player gives
  it back through the game's set_effect_presenter 0x1F3740 (its effects
  presented again, game_effect_player 0x2DE560 run natively), once; the
  place's other kit's object, a player whose effects go to the first-person
  presenter and one whose effects were never given are left as they are;
  after the hand-back (which cleared the presenter), a stop of an effect it
  no longer holds goes to patch 117: the sound and particle presenters'
  on_event (0x2A9EF0, 0x2AC400) return, the presenter unchanged.

Says nothing about how the glow looks; the renders and the owner's test do.
"""
import struct
import sys

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_ESP, UC_X86_REG_EIP, UC_X86_REG_ESI, \
    UC_X86_REG_EDI

import patch_026g as base
from check_artefacts_026g import PLAYER
from check_medical_skills_026g import MedicalGame, bits

TICK = 0x2DAAD3              # battery.carrier_tick: EBX the player, a 5-byte span
QUERY = 0xE5B60              # query_resources (cdecl: eax requests, ecx count; function, allocator, user, parent, flag)
INVOKER = 0x3AAA20           # the bound cdecl function's invoker (vtable 0x9DB9E0 slot 1)
FUNCTION_VTABLE = 0x9DB9E0
DEFAULT_CLOCK_VT = 0xA11398  # default_game_effect_time_calculator (+0 calculate 0x2E5D80)
CAPACITY = 0x2EC81E
MODIFIER_41 = 0xF4 + 41 * 4  # a kit's modifier 41 in its profile (kit n at +704h x n)


NAMES = ["forest_protection"] + [f"artefact_leg_{kind}_{n}" for kind in ("spring", "lifebone", "onyx")
                                  for n in (1, 2, 3, 4)]


def on_query(g, emu):
    """query_resources stubbed: its requests (path, class), its count, its function and user data."""
    esp = emu.reg_read(UC_X86_REG_ESP)
    requests, count = emu.reg_read(UC_X86_REG_EAX), emu.reg_read(UC_X86_REG_ECX)
    paths = [(bytes(emu.mem_read(g.read(requests + 8 * i), 64)).split(b"\0")[0].decode(), g.read(requests + 8 * i + 4))
             for i in range(count)]
    g.queries.append((paths, count, bytes(emu.mem_read(g.read(esp + 4), 0x20)), g.read(esp + 12)))
    emu.reg_write(UC_X86_REG_EIP, g.read(esp))
    emu.reg_write(UC_X86_REG_ESP, esp + 4)


def asked_and_kept(g):
    """The one query: the thirteen effects, class 109 each, tag 1, no user data; its callback
    through the game's invoker with a resource each (results 2E0h apart): one reference kept on
    each. The resources, in the effects' order."""
    assert len(g.queries) == 1, g.queries
    paths, count, function, user = g.queries[0]
    vtable, _entry, word = struct.unpack_from("<I4xII", function)
    assert paths == [(name, 109) for name in NAMES] and count == 13 and user == 0, g.queries
    assert (vtable, word) == (FUNCTION_VTABLE | 1, 1)
    resources = [g.effect_resource() for _ in NAMES]
    results = g.block(0x12C + 0x2E0 * len(NAMES))
    g.word(results + 0x38, len(NAMES))
    for i, resource in enumerate(resources):
        g.word(resource + 0xD0, 1)
        g.word(results + 0x12C + 0x2E0 * i, resource)
    at_function = g.block(0x20)
    g.emu.mem_write(at_function, function)
    g.call(INVOKER, (at_function + 8, results))
    assert all(g.read(resource + 0xD0) == 2 for resource in resources), "each kept: one reference of its own"
    return resources


class Glow(MedicalGame):
    def __init__(self, data):
        super().__init__(data)
        self.other = self.player(1, 1, (3, 0, 0))
        self.queries, self.played = [], []
        self.emu.hook_add(UC_HOOK_CODE, self.on_query, begin=QUERY, end=QUERY)
        for at in (PLAYER, self.other):
            self.word(at + 0x394, DEFAULT_CLOCK_VT)            # the player's own calculator

    def on_query(self, emu, address, size, _):
        on_query(self, emu)

    def effect(self, emu, address, size, _):
        esp = emu.reg_read(UC_X86_REG_ESP)
        self.played.append((emu.reg_read(UC_X86_REG_EDI) - 0x358, self.read(self.read(esp + 4)),
                            self.read(esp + 8), self.read(esp + 12), self.read(esp + 16), self.read(esp + 20)))
        super().effect(emu, address, size, _)

    def profile(self, at):
        return self.read(at + 0x6B8)

    def state(self, at, kit, ranks, zones, alive=True):
        """The player's current kit and its modifier 41 (every other kit a
        rank), its damage zones and life."""
        self.emu.mem_write(at + 0x115, bytes([kit]))
        for k in range(3):
            self.word(self.profile(at) + MODIFIER_41 + k * 0x704, bits(-0.05 * (ranks if k == kit else 1)))
        self.emu.mem_write(at + 0x2E4, bytes([zones]))
        self.emu.mem_write(at + 0x33C, bytes([int(alive)]))

    def playing(self, at, effect):
        """The player's effect list: one node playing `effect` (or none)."""
        if effect is None:
            self.word(at + 0x358 + 0x2C, 0)
            return
        node = self.block(0x50)
        self.word(node, effect)
        self.word(at + 0x358 + 0x2C, node)

    def tick(self, at):
        self.played = []
        self.step(TICK, 5, stack=[0] * 8, ebx=at)
        return self.played

    def clock(self, at, effect, start=1000, now=5000):
        """The player's calculator on a node of `effect`: (interval, time, ended)."""
        node, out = self.block(0x50), self.block(12)
        self.word(node, effect)
        self.word(node + 0x18, start)
        self.call(self.read(DEFAULT_CLOCK_VT), (out, node, start, now), ecx=at + 0x394)
        assert self.emu.reg_read(UC_X86_REG_EAX) == out
        return self.read(out), self.float(out + 4), self.emu.mem_read(out + 8, 1)[0]


def check(data):
    g = Glow(data)
    for at, time in ((PLAYER, 5000), (g.other, 7000)):
        g.state(at, 0, 1, 1)
        g.word(at + 0x324, time)                                # the player's simulation time

    # The first tick asks for the effects; nothing plays before they arrive.
    assert g.tick(PLAYER) == []
    assert g.tick(g.other) == [] and len(g.queries) == 1, "asked once"
    resource = asked_and_kept(g)[0]
    print("Forest glow: the thirteen effects asked once at the first tick (forest_protection and the "
          "leg quarters, class 109, tag 1), each kept when they arrive: PASS")

    # The trigger and the clock: both kits, two places, ranks 0 and 1, zones 0 and 1, alive or dead.
    cases = 0
    ours = None
    for at in (PLAYER, g.other):
        for kit in (0, 1):
            for ranks in (0, 1):
                for zones in (0, 1):
                    for alive in (True, False):
                        on = ranks > 0 and zones > 0 and alive
                        g.state(at, kit, ranks, zones, alive)
                        g.playing(at, None)
                        played = g.tick(at)
                        if on:
                            assert len(played) == 1, (hex(at), kit, ranks, zones, played)
                            target, effect, time, third, fourth, listener = played[0]
                            assert (target, time, third, fourth, listener) == (at, g.read(at + 0x324), 0, 2, at + 0x394)
                            assert g.read(effect + 0x10) == resource, "emitted from the kept resource"
                            ours = effect
                            g.playing(at, effect)
                            assert g.tick(at) == [], "a player playing it gets no other"
                        else:
                            assert played == [], (hex(at), kit, ranks, zones, alive, played)
                        if ours is not None:
                            assert g.clock(at, ours) == (0, 0.0, int(not on)), (hex(at), kit, ranks, zones, alive)
                        cases += 1
    assert cases == 32 and ours is not None
    # Another effect of the player's calculator: the native calculation, as before.
    other = g.block(0x60)
    interval = g.block(8)
    g.word(interval + 4, bits(1))
    node, out = g.block(0x50), g.block(12)
    g.word(node, other)
    g.word(node + 0x10, 1)
    g.word(node + 0x14, interval)
    g.call(g.read(DEFAULT_CLOCK_VT), (out, node, 1000, 3000), ecx=PLAYER + 0x394)
    native = (g.read(out), g.float(out + 4), g.emu.mem_read(out + 8, 1)[0])
    assert native != (0, 0.0, 0) and native != (0, 0.0, 1), native
    print("Forest glow: played at a tick exactly while alive, in a damage zone and a rank in the current kit "
          "(both kits, two places, ranks 0/1, zones 0/1, dead), once, from the player's time with its own "
          "calculator; the calculator ends it when out of the zone, without ranks or dead; other effects of "
          "it the native's: PASS")

    # The effect cook's instances.
    query, fallback = g.block(0x120), g.block(64)
    g.word(query + 0xF8, fallback)
    g.word(query + 0xFC, 0)
    for name, initial, expected in (("forest_protection", 1, 20), ("forest_protection", 30, 30),
                                    ("artefact_lifebone", 1, 20), ("artefact_leg_onyx_3", 1, 20), ("forest_protection_x", 1, 1),
                                    ("forest", 1, 1), ("artefact_onyx", 1, 1)):
        g.emu.mem_write(fallback, name.encode() + b"\0")
        g.step(CAPACITY, 5, ebx=query, esi=initial)
        assert g.emu.reg_read(UC_X86_REG_ESI) == expected, (name, initial)
    print("Forest glow: the cook gives forest_protection and each quarter 20 instances, Larkspur still 20, others their own: PASS")


def check_quarters(data):
    """The leg quarters: a kind's passive copies counting for the player (its own carried ones
    giving their passive, Resonance's shared ones, at most 4) light that many of the kind's
    quarters, from the first; the player's calculator ends a quarter when the count drops below
    it, and every one at death."""
    from check_artefact_skills_026g import Match, RESONANCE, SPRING_ACTIVE, ONYX_ACTIVE_
    m = Match(data)
    g = m.g
    g.queries = []
    g.emu.hook_add(UC_HOOK_CODE, lambda emu, *_: on_query(g, emu), begin=QUERY, end=QUERY)
    g.word(PLAYER + 0x394, DEFAULT_CLOCK_VT)
    g.word(PLAYER + 0x324, 9000)
    g.emu.mem_write(PLAYER + 0x2E4, b"\0")                     # no zone: no Forest Protection
    tick = lambda who=PLAYER: (g.step(TICK, 5, stack=[0] * 8, ebx=who), [g.read(e + 0x10) for e in g.effect_instances])
    g.effect_instances.clear()
    tick()
    resources = asked_and_kept(g)
    quarter = {(kind, n): resources[1 + 4 * k + n - 1]
               for k, kind in enumerate(("spring", "lifebone", "onyx")) for n in (1, 2, 3, 4)}

    def lit(expected, who=PLAYER):
        """A tick with no effect playing lights exactly `expected` {kind: copies}; each quarter's
        clock goes on for those and ends for the others."""
        g.word(who + 0x358 + 0x2C, 0)
        g.effect_instances.clear()
        _, played = tick(who)
        want = sorted(quarter[(kind, n)] for kind, copies in expected.items() for n in range(1, copies + 1))
        assert sorted(played) == want, (expected, played)
        for (kind, n), resource in quarter.items():
            effect, node, out = g.block(0x60), g.block(0x50), g.block(12)
            g.word(effect + 0x10, resource)
            g.word(node, effect)
            g.call(g.read(DEFAULT_CLOCK_VT), (out, node, 1000, 5000), ecx=who + 0x394)
            ended = g.emu.mem_read(out + 8, 1)[0]
            assert ended == int(n > expected.get(kind, 0)), (expected, kind, n, ended)

    def slot_of(player, item):
        inv = m.inventory(player)
        return next(inv + 0x10C + s * 4 for s in range(13, 19) if g.read(inv + 0x10C + s * 4) == item)

    # Own Onyx passives, 0 to 4; one active Onyx gives no passive (its active instead).
    onyx = []
    lit({})
    for n in range(1, 5):
        onyx.append(m.artefact(PLAYER, ONYX_ACTIVE_, 3))
        lit({"onyx": n})
    g.word(onyx[3] + 0x280, 4)
    lit({"onyx": 3})
    g.word(onyx[3] + 0x280, 3)
    # A second kind: one own Spring and two a Resonance teammate 3 m away carries.
    own_spring = m.artefact(PLAYER, SPRING_ACTIVE, 3)
    carrier = m.player(1, 0, (3.0, 0.0, 0.0), RESONANCE)
    shared = [m.artefact(carrier, SPRING_ACTIVE, 3) for _ in range(2)]
    lit({"onyx": 4, "spring": 3})
    # An enemy's Springs count nothing.
    enemy = m.player(2, 1, (2.0, 0.0, 0.0), RESONANCE)
    m.artefact(enemy, SPRING_ACTIVE, 3)
    lit({"onyx": 4, "spring": 3})
    # Past 4: two more shared stop at the cap (the sharing's own room).
    shared += [m.artefact(carrier, SPRING_ACTIVE, 3) for _ in range(2)]
    lit({"onyx": 4, "spring": 4})
    # The count drops: the carrier's copies leave, then the carrier dies, then an own Onyx goes.
    for item in shared[2:]:
        g.word(slot_of(carrier, item), 0)
    lit({"onyx": 4, "spring": 3})
    g.emu.mem_write(carrier + 0x33C, b"\0")
    lit({"onyx": 4, "spring": 1})
    g.word(slot_of(PLAYER, onyx[0]), 0)
    lit({"onyx": 3, "spring": 1})
    # A player without a world core (the lobby's character): nothing lit, no lookup (core_active_player
    # reads through the core unchecked, 0x2D0B12).
    core = g.read(PLAYER + 0x120)
    g.word(PLAYER + 0x120, 0)
    lit({})
    g.word(PLAYER + 0x120, core)
    lit({"onyx": 3, "spring": 1})
    # The gate: the local player (the game world's game's network client's +4) must have Artifact
    # Sight in its current kit (513: bit 12 of modifier 40) for this game to show any quarters, its
    # own too; Knowledge of Artifacts (modifier 39) no longer does; no local player shows all.
    world, game, client = g.block(0x100), g.block(0x5400), g.block(0x100)
    g.word(world + 0x9C, game)
    g.word(game + 0x53AC, client)
    for who in (PLAYER, carrier):
        g.word(who + 0xF474, world)
    g.word(carrier + 0x394, DEFAULT_CLOCK_VT)
    g.word(carrier + 0x324, 9000)
    modifier = lambda at, kit, index: g.read(at + 0x6B8) + 0xF4 + kit * 0x704 + index * 4
    g.word(client + 4, PLAYER)
    g.emu.mem_write(PLAYER + 0x115, bytes([0]))
    g.word(modifier(PLAYER, 0, 40), bits(4096.0))            # kit 0: Artifact Sight
    g.word(modifier(PLAYER, 0, 39), bits(0.0))
    g.word(modifier(PLAYER, 1, 40), bits(0.0))               # kit 1: Knowledge of Artifacts 1, no Sight
    g.word(modifier(PLAYER, 1, 39), bits(0.05))
    g.emu.mem_write(carrier + 0x33C, b"\1")
    carried = m.artefact(carrier, ONYX_ACTIVE_, 3)
    lit({"onyx": 4, "spring": 3}, PLAYER)                    # with the skill: its own (Onyx 3 + 1 shared, Spring 1 + 2)
    lit({"spring": 2, "onyx": 1}, carrier)                   # and another player's
    g.emu.mem_write(PLAYER + 0x115, bytes([1]))              # a kit without it: none, its own neither
    lit({}, PLAYER)
    lit({}, carrier)
    g.emu.mem_write(PLAYER + 0x115, bytes([0]))              # back to the kit with it: all again
    lit({"spring": 2, "onyx": 1}, carrier)
    g.word(client + 4, 0)                                    # no local player (a spectator, a replay): all
    g.emu.mem_write(PLAYER + 0x115, bytes([1]))
    lit({"onyx": 4, "spring": 3}, PLAYER)
    lit({"spring": 2, "onyx": 1}, carrier)
    g.word(client + 4, PLAYER)
    g.emu.mem_write(PLAYER + 0x115, bytes([0]))
    # A kit change: each kit of a place is a player object of its own (its kit byte fixed), the
    # place's current kit (+0Ch) names the one in play, and the network client's local player may
    # stay the first kit's object. Kit 0 (the local player's object) without Artifact Sight, the
    # place's current kit 1 with it: the game shows the quarters (the current kit decides).
    entry = g.places
    second = g.player(0, 0, (0.0, 0.0, 0.0), profile=g.read(PLAYER + 0x6B8))
    g.word(entry, PLAYER)
    g.word(entry + 4, second)
    g.emu.mem_write(second + 0x115, bytes([1]))
    g.word(second + 0xF474, world)
    g.word(modifier(PLAYER, 0, 40), bits(0.0))
    g.word(modifier(PLAYER, 1, 40), bits(4096.0))
    g.emu.mem_write(entry + 0xC, bytes([1]))
    lit({"spring": 2, "onyx": 1}, carrier)
    g.emu.mem_write(entry + 0xC, bytes([0]))                 # back to kit 0, no Sight: none
    lit({}, carrier)
    g.word(modifier(PLAYER, 0, 40), bits(4096.0))
    g.word(modifier(PLAYER, 1, 40), bits(0.0))
    g.word(slot_of(carrier, carried), 0)
    g.emu.mem_write(carrier + 0x33C, b"\0")
    # Death: nothing lit, every quarter ended.
    g.emu.mem_write(PLAYER + 0x33C, b"\0")
    lit({})
    assert own_spring
    print("Leg quarters: a kind's passive copies (own 0..4, own and Resonance's shared, two kinds, an active "
          "Onyx, an enemy's and the cap of 4) light that many of its quarters from the first; the count "
          "dropping and death end them through the player's calculator; a player without a world core lights none; "
          "a game shows them (every player's, its own too) only when its local player's current kit has Artifact "
          "Sight (modifier 40 bit 12; Knowledge of Artifacts alone shows none) in its place's current kit object (a kit "
          "change keeps or ends them), a kit flip, and all of "
          "them without a local player: PASS")


TP_PRESENTER = 0x297570       # third_person_game_effects_presenter's constructor: eax = this, edx = the world
INITIALIZE, INITIALIZE_GIVES, INITIALIZE_ON = 0x1F2540, 0x1F2580, 0x1F2585   # player::initialize: its call of
                              # set_effect_presenter, the code after it
SOUND_EVENT, PARTICLE_EVENT = 0x2A9EF0, 0x2AC400   # the presenters' on_event (patch 117's sites)
TICK_COUNT_SLOT = 0x8701B8    # GetTickCount's import slot (the refusals' clock)


def check_presenter(data):
    """A place's third-person presenter follows the place's current kit."""
    g = Glow(data)
    world = g.block(0x6000)
    presenters = g.block(0x22CC + 20 * 0x220)
    holder = presenters + 0x22CC + 0 * 0x220                  # place 0's holder
    presenter = g.block(0x300)
    g.call(TP_PRESENTER, edx=world, eax=presenter)
    g.word(holder, presenter)
    first_person = g.block(0x10)                              # another holder (the camera's player's)
    g.word(first_person, presenter)
    second = g.player(0, 0, (0.0, 0.0, 0.0), profile=g.profile(PLAYER))
    g.emu.mem_write(second + 0x115, bytes([1]))
    for at in (PLAYER, second):
        g.word(at + 0xF478, presenters)
        g.word(at + 0x394, DEFAULT_CLOCK_VT)
        g.word(at + 0x358 + 0x2C, 0)                          # nothing playing
        g.word(at + 0x358 + 0x38, holder)                     # its effects given to its place's presenter
        g.word(at + 0x3A0, holder)
    entry = g.places
    g.word(entry, PLAYER)
    g.word(entry + 4, second)

    def initialize_gives(at):
        """Whether player::initialize, run on `at`, gives it its place's presenter."""
        gave = []

        def on_give(emu, address, size, _):
            gave.append(True)
            emu.reg_write(UC_X86_REG_EIP, INITIALIZE_ON)
        hook = g.emu.hook_add(UC_HOOK_CODE, on_give, begin=INITIALIZE_GIVES, end=INITIALIZE_GIVES)
        g.emu.reg_write(UC_X86_REG_ESP, 0x21007000)
        g.emu.reg_write(UC_X86_REG_ECX, at)
        g.emu.emu_start(INITIALIZE, INITIALIZE_ON, count=10_000)
        g.emu.hook_del(hook)
        return bool(gave)

    # The stock rule: an object whose effects already went to the presenter is not given it again.
    fresh = g.block(0x500)
    g.word(fresh + 0x390, fresh + 0x3A8)
    g.word(fresh + 0xF478, presenters)
    assert initialize_gives(fresh) and not initialize_gives(PLAYER)
    # Kit 1 spawned last (the presenter plays on its object); the place respawns in kit 0.
    g.word(presenter + 0x10, second)
    g.emu.mem_write(entry + 0xC, bytes([0]))
    g.tick(second)                                            # not the current kit's: left
    assert g.read(presenter + 0x10) == second
    g.tick(PLAYER)
    assert g.read(presenter + 0x10) == PLAYER and g.read(presenter + 0xC0) == PLAYER, \
        "the current kit's tick takes its place's presenter back"
    assert g.read(PLAYER + 0x3A0) == holder and g.read(PLAYER + 0x390) == holder
    # Patch 117 beside the hand-back: its represent cleared the presenter, so a damage zone's stop of
    # an effect played before it (game_effect_player::stop, the tick after) finds no entry in either
    # presenter; the build returns from both on_event, the presenter as the hand-back left it.
    tick_count = g.block(0x10)
    g.emu.mem_write(tick_count, b"\x31\xc0\xc3")                # GetTickCount: xor eax,eax; ret
    g.word(TICK_COUNT_SLOT, tick_count)
    effect, event = g.block(0x60), g.block(0x20)
    g.word(event + 4, 1)                                      # the event's kind: a stop
    before = bytes(g.emu.mem_read(presenter, 0x300))
    g.call(SOUND_EVENT, (presenter + 0xB4, effect, event))
    g.call(PARTICLE_EVENT, (presenter + 0x8, effect), edx=event)
    assert bytes(g.emu.mem_read(presenter, 0x300)) == before, "a stop it holds no entry of changes nothing"
    g.word(presenter + 0x10, PLAYER)
    g.tick(PLAYER)
    assert g.read(presenter + 0x10) == PLAYER
    # Back in kit 1: its tick takes it.
    g.emu.mem_write(entry + 0xC, bytes([1]))
    g.tick(second)
    assert g.read(presenter + 0x10) == second
    # The camera's player (its effects to the first-person presenter) and one never given: left.
    g.word(second + 0x358 + 0x38, first_person)
    g.word(presenter + 0x10, PLAYER)
    g.tick(second)
    assert g.read(presenter + 0x10) == PLAYER
    g.word(second + 0x358 + 0x38, second + 0x3A8)
    g.tick(second)
    assert g.read(presenter + 0x10) == PLAYER
    print("Presenter: player::initialize gives a place's third-person presenter only to an object whose effects "
          "were never given; the tick of the place's current kit's object takes it back from another kit's (once, "
          "its effects presented again natively); another kit's object, the camera's player and one never given "
          "left; after the hand-back a stop of an effect neither presenter holds left to patch 117 (both return, the presenter unchanged): PASS")


def main():
    original = open(sys.argv[1], "rb").read()
    patched, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None,
                            open(base.late_join_bin(), "rb").read())
    check(patched)
    check_quarters(patched)
    check_presenter(patched)


if __name__ == "__main__":
    main()
