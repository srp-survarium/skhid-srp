#!/usr/bin/env python3
"""Forest Protection's glow (patch 110, rust/patches/src/forest_glow.rs;
docs/notes/artefact-skills.md, "Forest Protection") run in the built exe.

    python check_forest_glow_026g.py <original survarium.exe>

Requires Python `unicorn`. On the medical checks' match fixture (the game
world, its network client and places, players with profiles and kits), the
whole after_move chain (game_world.after_move 0x1F7C3C, every patch's
handler) is run as the game runs it each frame, with the game's own
query_resources stubbed (recorded), its game effect player's add 0x2DE090
and remove 0x2DE350 stubbed (recorded), and the rest native: the boost
function bind, its invoker (the query's callback), get_active_player, the
modifier getter, game_effect_resource::emit (its emitter stubbed). It checks:

- the first frame asks for the effect once: forest_protection, class 109,
  one request, a boost function bound to the glow's callback and tag 1;
  until it arrives nothing plays; when it arrives one reference is taken;
- the trigger on both kits, on the local player and another place: the glow
  is played (emitted from the kept resource, at the player's time, kind 2,
  the held clock) exactly when the player is alive, inside a damage zone
  (+2E4h 1, 0 none) and its current kit's modifier 41 is a rank or more
  (-0.05; 0 none; the other kit's ranks never count); an effect it already
  plays is not played again; when the condition ends (out of the zone,
  ranks 0, dead) the effect it plays is removed through the player's effect
  player (eax player+358h, ecx the effect), and nothing else is;
- the held clock never ends (interval 0, time 0: the artefact's frozen
  glow), and lets other clocks through to Medic's and the native;
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
from check_artefacts_026g import PLAYER, KITS
from check_medical_skills_026g import SoundGame, bits

INVOKER = 0x3AAA20          # the bound cdecl function's invoker (vtable 0x9DB9E0 slot 1)
FUNCTION_VTABLE = 0x9DB9E0
ADD, REMOVE = 0x2DE090, 0x2DE350
CAPACITY = 0x2EC81E
MODIFIER_41 = 0xF4 + 41 * 4  # a kit's modifier 41 in its profile (kit n at +704h x n)


class Glow(SoundGame):
    def __init__(self, data):
        super().__init__(data, [0, 1])
        self.other = self.player(1, 1, (3, 0, 0))
        self.removed = []
        self.played = []
        self.emu.hook_add(UC_HOOK_CODE, self.on_remove, begin=REMOVE, end=REMOVE)

    def effect(self, emu, address, size, _):
        esp = emu.reg_read(UC_X86_REG_ESP)
        self.played.append((emu.reg_read(UC_X86_REG_EDI) - 0x358, self.read(self.read(esp + 4)),
                            self.read(esp + 8), self.read(esp + 12), self.read(esp + 16), self.read(esp + 20)))
        super().effect(emu, address, size, _)

    def on_remove(self, emu, address, size, _):
        esp = emu.reg_read(UC_X86_REG_ESP)
        self.removed.append((emu.reg_read(UC_X86_REG_EAX) - 0x358, emu.reg_read(UC_X86_REG_ECX)))
        emu.reg_write(UC_X86_REG_EIP, self.read(esp))
        emu.reg_write(UC_X86_REG_ESP, esp + 4)

    def profile(self, at):
        return self.read(at + 0x6B8)

    def state(self, at, kit, ranks, zones, alive=True, other_kit_ranks=1):
        """The player's current kit and its modifier 41 (the other kits' a rank
        unless said), its damage zones and life."""
        self.emu.mem_write(at + 0x115, bytes([kit]))
        for k in range(3):
            self.word(self.profile(at) + MODIFIER_41 + k * 0x704, bits(-0.05 * (ranks if k == kit else other_kit_ranks)))
        self.emu.mem_write(at + 0x2E4, bytes([zones]))
        self.emu.mem_write(at + 0x33C, bytes([int(alive)]))

    def playing(self, at, effect, listener):
        """The player's effect list: one node playing `effect` with `listener`
        (or none)."""
        if effect is None:
            self.word(at + 0x358 + 0x2C, 0)
            return
        node = self.block(0x50)
        self.word(node, effect)
        self.word(node + 0x30, listener)
        self.word(at + 0x358 + 0x2C, node)

    def frame(self):
        self.played, self.removed = [], []
        self.publish()
        return self.played, self.removed


def check(data):
    g = Glow(data)
    for at in (PLAYER, g.other):
        g.state(at, 0, 1, 1)
        g.word(at + 0x324, 5000 if at == PLAYER else 7000)       # the player's simulation time

    # The first frame asks for the effect; nothing plays before it arrives.
    played, removed = g.frame()
    assert len(g.queries) == 1, g.queries
    path, cls, count, function = g.queries[0]
    vtable, entry, word = struct.unpack_from("<I4xII", function)
    assert (path, cls, count, vtable, word) == ("forest_protection", 109, 1, FUNCTION_VTABLE | 1, 1), g.queries
    assert played == [] and removed == []
    g.frame()
    assert len(g.queries) == 1 and g.played == [], "asked once"

    resource = g.effect_resource()
    g.word(resource + 0xD0, 1)
    results = g.block(0x400)
    g.word(results + 0x38, 1)
    g.word(results + 0x12C, resource)
    at_function = g.block(0x20)
    g.emu.mem_write(at_function, function)
    g.call(INVOKER, (at_function + 8, results))
    assert g.read(resource + 0xD0) == 2, "the arrived resource kept: one reference of its own"
    print("Forest glow: the effect asked once at the first frame (forest_protection, class 109, tag 1), "
          "kept when it arrives: PASS")

    # The trigger: both kits, both places, ranks 0 and 1, zones 0 and 1.
    clock = None
    cases = 0
    for at in (PLAYER, g.other):
        for kit in (0, 1):
            for ranks in (0, 1):
                for zones in (0, 1):
                    for alive in (True, False):
                        on = ranks > 0 and zones > 0 and alive
                        g.state(g.other if at == PLAYER else PLAYER, kit, 0, 0)  # the other player never glows
                        g.state(at, kit, ranks, zones, alive)
                        g.playing(at, None, 0)
                        played, removed = g.frame()
                        assert removed == [], (hex(at), kit, ranks, zones, alive, removed)
                        if not on:
                            assert played == [], (hex(at), kit, ranks, zones, alive, played)
                            cases += 1
                            continue
                        assert len(played) == 1, (hex(at), kit, ranks, zones, played)
                        target, effect, time, third, fourth, listener = played[0]
                        assert target == at and time == g.read(at + 0x324) and (third, fourth) == (0, 2)
                        assert g.read(effect + 0x10) == resource, "emitted from the kept resource"
                        assert g.read(listener + 4) == 0xFFFFFFFF and listener not in (at + 0x394,)
                        clock = clock or listener
                        assert listener == clock, "one held clock"
                        # Already playing: not again, not removed.
                        g.playing(at, effect, listener)
                        assert g.frame() == ([], []), (hex(at), kit)
                        # Another effect of the player (Medic's or the game's) is never ours.
                        g.playing(at, effect, at + 0x394)
                        played, removed = g.frame()
                        assert removed == [] and len(played) == 1
                        # The condition ends: out of the zone, ranks 0, dead.
                        for end in ((ranks, 0, True), (0, zones, True), (ranks, zones, False)):
                            g.state(at, kit, *end)
                            g.playing(at, effect, listener)
                            assert g.frame() == ([], [(at, effect)]), (hex(at), kit, end)
                            g.playing(at, effect, at + 0x394)
                            assert g.frame() == ([], []), "only its own effect is removed"
                        cases += 1
    assert cases == 32
    print("Forest glow: played exactly while alive, in a damage zone and a rank in the current kit "
          "(both kits, the local player and another place, ranks 0/1, zones 0/1, dead), once, from the "
          "player's time with the held clock; removed when out of the zone, without ranks or dead; "
          "other effects untouched: PASS")

    # The held clock never ends; other clocks go on to Medic's and the native.
    node, out = g.block(0x50), g.block(12)
    effect = g.emitted[0]
    g.word(node, effect)
    for start, now in ((5000, 5000), (5000, 5000 + 3_600_000), (0xFFFFFFF0, 0x10)):
        g.word(node + 0x18, start)
        g.call(g.read(g.read(clock)), (out, node, start, now), ecx=clock)
        assert (g.read(out), g.float(out + 4), g.emu.mem_read(out + 8, 1)) == (0, 0.0, b"\0")
        assert g.emu.reg_read(UC_X86_REG_EAX) == out
    print("Forest glow: the held clock never ends (interval 0, time 0): PASS")

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
