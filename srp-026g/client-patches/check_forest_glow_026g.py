#!/usr/bin/env python3
"""Forest Protection's glow (patch 111, rust/patches/src/forest_glow.rs;
docs/notes/artefact-skills.md, "Forest Protection") run in the built exe.

    python check_forest_glow_026g.py <original survarium.exe>

Requires Python `unicorn`. On the medical checks' player fixture (players
with profiles and kits, their world core), the player's simulation tick
(`battery.carrier_tick` 0x2DAAD3, the whole chain: the battery carrier's
handler, then the glow's) and the default clock (0x2E5D80, Medic's handler,
then the glow's) are run as the game runs them, with the game's own
query_resources stubbed (recorded) and its game effect player's add
0x2DE090 stubbed (recorded); the rest native: the boost function bind, its
invoker (the query's callback), the modifier getter, game_effect_resource::
emit (its emitter stubbed). It checks:

- the first tick asks for the effect once: forest_protection, class 109, one
  request, a boost function bound to the glow's callback and tag 1; until it
  arrives nothing plays; when it arrives one reference is taken;
- the trigger on both kits, on two places: a tick plays the glow (emitted
  from the kept resource, at the player's time, kind 2, the player's own
  calculator +394h) exactly when the player is alive, inside a damage zone
  (+2E4h 1, 0 none) and its current kit's modifier 41 is a rank or more
  (-0.05; 0 none; the other kits' ranks never count); a player that already
  plays it is not given another;
- the clock: an effect of the kept resource with the player's calculator
  goes on while the player shows the glow and ends when it is out of the
  zone, without ranks or dead; any other effect of that calculator goes on
  through the native calculation unchanged;
- the effect cook gives forest_protection 20 instances (and still
  artefact_lifebone 20, any other effect its own count).

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


class Glow(MedicalGame):
    def __init__(self, data):
        super().__init__(data)
        self.other = self.player(1, 1, (3, 0, 0))
        self.queries, self.played = [], []
        self.emu.hook_add(UC_HOOK_CODE, self.on_query, begin=QUERY, end=QUERY)
        for at in (PLAYER, self.other):
            self.word(at + 0x394, DEFAULT_CLOCK_VT)            # the player's own calculator

    def on_query(self, emu, address, size, _):
        esp = emu.reg_read(UC_X86_REG_ESP)
        requests = emu.reg_read(UC_X86_REG_EAX)
        path = bytes(emu.mem_read(self.read(requests), 64)).split(b"\0")[0].decode()
        self.queries.append((path, self.read(requests + 4), emu.reg_read(UC_X86_REG_ECX),
                             bytes(emu.mem_read(self.read(esp + 4), 0x20))))
        emu.reg_write(UC_X86_REG_EIP, self.read(esp))
        emu.reg_write(UC_X86_REG_ESP, esp + 4)

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

    # The first tick asks for the effect; nothing plays before it arrives.
    assert g.tick(PLAYER) == []
    assert len(g.queries) == 1, g.queries
    path, cls, count, function = g.queries[0]
    vtable, _entry, word = struct.unpack_from("<I4xII", function)
    assert (path, cls, count, vtable, word) == ("forest_protection", 109, 1, FUNCTION_VTABLE | 1, 1), g.queries
    assert g.tick(g.other) == [] and len(g.queries) == 1, "asked once"

    resource = g.effect_resource()
    g.word(resource + 0xD0, 1)
    results = g.block(0x400)
    g.word(results + 0x38, 1)
    g.word(results + 0x12C, resource)
    at_function = g.block(0x20)
    g.emu.mem_write(at_function, function)
    g.call(INVOKER, (at_function + 8, results))
    assert g.read(resource + 0xD0) == 2, "the arrived resource kept: one reference of its own"
    print("Forest glow: the effect asked once at the first tick (forest_protection, class 109, tag 1), "
          "kept when it arrives: PASS")

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
                                    ("artefact_lifebone", 1, 20), ("forest_protection_x", 1, 1),
                                    ("forest", 1, 1), ("artefact_onyx", 1, 1)):
        g.emu.mem_write(fallback, name.encode() + b"\0")
        g.step(CAPACITY, 5, ebx=query, esi=initial)
        assert g.emu.reg_read(UC_X86_REG_ESI) == expected, (name, initial)
    print("Forest glow: the cook gives forest_protection 20 instances, Larkspur still 20, others their own: PASS")


def main():
    original = open(sys.argv[1], "rb").read()
    patched, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None,
                            open(base.late_join_bin(), "rb").read())
    check(patched)


if __name__ == "__main__":
    main()
