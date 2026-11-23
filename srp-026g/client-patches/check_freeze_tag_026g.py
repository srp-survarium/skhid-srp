#!/usr/bin/env python3
"""Unicorn: installed Freeze Tag hooks, native lifecycle, wall rays and world state.
Run through tools/check freeze-native, with the original 0.26g executable.
"""
import struct
import sys
from unicorn import UC_HOOK_CODE, UcError
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI, UC_X86_REG_EIP,
                                UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_EBP)
import patch_026g as base
import check_free_for_all_026g as ffa
import check_blackout_026g as blackout
import check_hide_ui_026g as hide
import check_kit_change_026g as kits

FREEZE = blackout.empty_options(blackout.srpx((6, b"\x01")))
PLAIN = blackout.empty_options(b"")
LENGTH = 149
STATS = ffa.DATA + 0x60000
CAMERA_INPUT = ffa.DATA + 0x96000
CAMERA_STACK = 0x36000000
# The frozen glow's particle query (a frame's draw asks it): the game's query_resources stubbed and
# recorded; one test answers it and checks what the scene renderer is given (its play, move and
# remove stubbed: pops 28, 20, 12).
QUERY = 0xE5B60
PLAY_PARTICLE, MOVE_PARTICLE, REMOVE_PARTICLE = 0x22660, 0x22B30, 0x229D0
GLOW = 0x38000000            # the test's renderer (its scene renderer at +200064h) and particle systems
WEAPON_VTABLE = 0xA04174   # the client weapon's (fields_026g.tsv survarium::image weapon_vtable)
# A world player of its own size (the harness's players are 1000h), for player::insert and
# player::remove themselves: their fields run to +0F540h, the player effect.
WHOLE = 0x37000000
INSERT, REMOVE, BASE_INSERT, BASE_REMOVE, CLIENT_REMOVE = 0x1F2720, 0x1F2870, 0x2DA850, 0x2DA790, 0x1F3670
NULL_EFFECT = 0x2DE36D   # game_effect_player::stop's read of the effect it did not find


class Run(ffa.Run):
    def __init__(self, data):
        super().__init__(data)
        # The shared after_move hook also steps ragdolls. Match that native
        # harness's 1 MiB stack, rather than the small rule-call fixture's.
        self.emu.mem_map(CAMERA_STACK, 0x100000)
        self.blocked = False
        self.thaws = []
        self.camera_calls = []
        self.emu.hook_add(UC_HOOK_CODE, self.ray, begin=0x75E20, end=0x75E20)
        self.emu.hook_add(UC_HOOK_CODE, self.reset_health, begin=0x2E5290, end=0x2E5290)
        self.emu.hook_add(UC_HOOK_CODE, self.camera_director, begin=0x243510, end=0x243510)
        self.queries = []
        self.emu.hook_add(UC_HOOK_CODE, self.on_query, begin=QUERY, end=QUERY)
        self.particles = []
        for at in (PLAY_PARTICLE, MOVE_PARTICLE, REMOVE_PARTICLE):
            self.emu.hook_add(UC_HOOK_CODE, self.on_particle, begin=at, end=at)
        self.emu.mem_map(WHOLE, 0x20000)
        self.pairs = None
        for at in (BASE_INSERT, BASE_REMOVE, CLIENT_REMOVE):
            self.emu.hook_add(UC_HOOK_CODE, self.base_pair, begin=at, end=at)

    def base_pair(self, emu, address, *_):
        """Only inside whole_player's calls: base_player::insert/remove keep +33Fh and nothing
        else; the client player's model removal (0x1F3670) is counted, not run."""
        if self.pairs is None:
            return
        this = emu.reg_read(UC_X86_REG_ECX) if address != CLIENT_REMOVE else WHOLE
        if address == BASE_INSERT:
            self.emu.mem_write(this + 0x33F, b"\x01")
        elif address == BASE_REMOVE:
            self.emu.mem_write(this + 0x33F, b"\x00")
        self.pairs.append(address)
        self.leave(emu, 4)

    def whole_player(self, call, link):
        """The game's own player::insert or player::remove (thiscall, ret 4) on WHOLE with
        `link`: the address it stopped at (its return, or a read it faulted on) and the base
        halves and model removal it called."""
        sp = hide.STACK + 0x8000
        self.emu.mem_write(sp, struct.pack("<2I", ffa.RETURN, link))
        self.pairs = []
        try:
            self.run(call, (ffa.RETURN,), sp, {**hide.GAME, UC_X86_REG_ECX: WHOLE})
            stopped = self.stopped
        except UcError:
            stopped = self.emu.reg_read(UC_X86_REG_EIP)
        calls, self.pairs = self.pairs, None
        return stopped, calls

    def on_query(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        requests, count = emu.reg_read(UC_X86_REG_EAX), emu.reg_read(UC_X86_REG_ECX)
        paths = [(self.text(self.read(requests + 8 * i)).decode(), self.read(requests + 8 * i + 4)) for i in range(count)]
        self.queries.append((paths, bytes(emu.mem_read(self.read(sp + 4), 0x20)), self.read(sp + 12)))
        self.leave(emu, 0)

    def on_particle(self, emu, address, *_):
        """The scene renderer's play (its particle, the matrix's translation and the two attachment
        slots' values: the animated object and the skeleton model instance), move and remove."""
        sp = emu.reg_read(UC_X86_REG_ESP)
        arg = lambda n: self.read(sp + 4 * n)
        if address == PLAY_PARTICLE:
            at = struct.unpack("<3f", emu.mem_read(arg(4) + 48, 12))
            self.particles.append(("play", arg(3), at, self.read(arg(6)), self.read(arg(7))))
            self.leave(emu, 28)
        elif address == MOVE_PARTICLE:
            self.particles.append(("move", self.read(arg(3))))
            self.leave(emu, 20)
        else:
            self.particles.append(("remove", self.read(arg(3))))
            self.leave(emu, 12)

    def camera_director(self, emu, *_):
        # Execute both real game_world camera switches; only their external
        # director is replaced. Its chosen interface is the native input+4.
        self.camera_calls.append(emu.reg_read(UC_X86_REG_EDI))
        self.leave(emu, 0)

    def camera(self):
        self.word(ffa.UI_WORLD + 0x4D58, CAMERA_INPUT)
        self.word(ffa.CLIENT + 4, ffa.player(0))
        # Other after_move subscribers have no presentation roster here.
        self.emu.mem_write(ffa.STALE_OPTIONS + 0x577B5, b"\0")
        sp = CAMERA_STACK + 0xFF000
        self.emu.mem_write(sp, bytes(0x40))
        self.run(0x1F7C3C, (0x1F7C41,), sp, {**hide.GAME, UC_X86_REG_EBX: ffa.UI_WORLD})
        return self.read(CAMERA_INPUT + 0x358)

    def ray(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        assert self.read(sp + 20) == 0x24 and self.read(sp + 24) == 2
        self.emu.mem_write(self.read(sp + 4), bytes(40))
        self.word(self.read(sp + 4) + 28, 123 if self.blocked else 0)
        self.leave(emu, 32)

    def reset_health(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.thaws.append((emu.reg_read(UC_X86_REG_EDI), self.read(sp + 4)))
        self.leave(emu, 4)

    def start(self, payload=FREEZE):
        assert self.static_info(payload)[0]
        self.rule([])
        self.word(ffa.RULE + 0x134, ffa.CORE)
        self.word(ffa.CORE + 0x1845C, ffa.CORE + 0x18464 + 3 * 48)
        self.word(ffa.CORE + 0x18FD8, 7)
        self.word(ffa.CORE + 0x18FE4, 3)
        self.word(ffa.CORE + 0x18FA8, STATS)
        for i, team in enumerate([0, 0, 1]):
            p = ffa.player(i)
            self.make_player(i, team, i, (float(i), 0.0, 0.0))
            self.word(p + 0x120, ffa.CORE)
            self.word(p + 0x350, ffa.DATA + 0x1000)
            self.word(p + 0x2E0, ffa.DATA + 0xC000 + 0x100 * i)
            self.emu.mem_write(p + 0x33C, b"\x01\x00\x00\x01")
        self.thaws.clear()

    def out_of_world(self, place):
        """The place in the match, its player out of the world: it entered (or came back) and
        has not spawned yet."""
        self.emu.mem_write(ffa.player(place) + 0x33F, b"\x00")

    def countdown_view(self, local):
        """The client time rule's camera test (time_rule.camera_state, 0x267AE6) with the local
        player at place `local` (None: no local player): the state the camera is set for, 2 the
        countdown's (the warmup camera, patch 31's kits), 3 play's."""
        rule = CAMERA_STACK + 0x10000
        self.emu.mem_write(rule, bytes(0x200))
        self.word(rule + 0x128, ffa.CORE)
        self.word(rule + 0x148, 3)
        kept = self.read(ffa.CLIENT + 4)
        self.word(ffa.CLIENT + 4, 0 if local is None else ffa.player(local))
        regs, _ = self.run(0x267AE6, (0x267AEC,), CAMERA_STACK + 0xF0000,
                           {**hide.GAME, UC_X86_REG_EDI: rule, UC_X86_REG_ESI: ffa.CLIENT})
        self.word(ffa.CLIENT + 4, kept)
        assert self.stopped == 0x267AEC
        return regs[UC_X86_REG_EDI]

    def lethal(self, place):
        self.emu.mem_write(ffa.player(place) + 0x33D, b"\x01")
        # The native death event already queued an ordinary respawn.
        self.word(ffa.RULE + 0x138 + 4 * place, 8000)

    def tick(self, time, elapsed=50):
        sp = hide.STACK + 0x8000
        self.emu.mem_write(sp, struct.pack("<4I", ffa.RETURN, elapsed, time, 0))
        self.run(0x316826, (0x31682B,), sp - 0x38, {**hide.GAME, UC_X86_REG_ECX: ffa.RULE, UC_X86_REG_EBP: sp - 4})
        assert self.stopped == 0x31682B

    def player_tick(self, place, time):
        sp = hide.STACK + 0x8000
        self.emu.mem_write(sp, struct.pack("<2I", ffa.RETURN, time))
        self.run(0x2DAA60, (0x2DAA65, ffa.RETURN), sp,
                 {**hide.GAME, UC_X86_REG_ECX: ffa.player(place)})
        return self.stopped == ffa.RETURN

    def input(self, place, time=0):
        # set_input's entry (patch 78's carry.input): held is its own ret 8
        # back to the caller; otherwise the chain goes on to patch 78 and
        # the game's bytes, past the site.
        sp = hide.STACK + 0x8000
        packet = sp + 0x100
        self.emu.mem_write(packet, bytes(8))
        self.emu.mem_write(sp, struct.pack("<3I", ffa.RETURN, time, packet))
        _, after = self.run(0x2D86E0, (0x2D86E6, ffa.RETURN), sp,
                            {**hide.GAME, UC_X86_REG_ECX: ffa.player(place)})
        held = self.stopped == ffa.RETURN
        assert not held or after == sp + 12, "ret 8 gives back both arguments"
        return held

    def state(self):
        return self.serialize()[-LENGTH:]

    def freeze_hud(self, viewed):
        self.movies(ffa.HUD, 1, 1)
        self.hud_state(viewed, [0, 0, 0], 0, 300)
        back, invoked = self.icons_fed(others=True)
        assert back
        return [(name, values) for movie, name, values in invoked
                if movie == hide.HUD_MOVIE and name.startswith("root.freeze_tag_")]


def main():
    original = open(sys.argv[1], "rb").read()
    patched, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None,
                            open(base.late_join_bin(), "rb").read())
    r = Run(bytes(patched))
    r.start(PLAIN)
    plain = r.serialize()
    r.lethal(0)
    assert not r.player_tick(0, 50) and not r.input(0)
    print("ok ordinary matches keep native death and input handling")

    r.start()
    assert len(r.serialize()) == len(plain) + LENGTH
    r.lethal(0)
    assert r.player_tick(0, 1000) and r.input(0)
    assert r.read(ffa.player(0) + 0x324) == 1000
    assert bytes(r.emu.mem_read(ffa.player(0) + 0x33C, 2)) == b"\x01\x01"
    for time in range(50, 4000, 50):
        r.tick(time)
    before = r.serialize()
    state = before[-LENGTH:]
    assert state[:2] == b"\x02\x00"
    assert state[7] == 3 and struct.unpack_from("<H", state, 8)[0] == 3950
    assert r.freeze_hud(0) == [("root.freeze_tag_score", (0, 0, 5)),
                               ("root.freeze_tag_update", (1, 3950, 0, 0, 4000, 116050)),
                               ("root.freeze_tag_rows", (1,))]
    assert r.freeze_hud(1) == [("root.freeze_tag_score", (0, 0, 5)),
                               ("root.freeze_tag_update", (0, 3950, 0, 0, 4000, 0)),
                               ("root.freeze_tag_rows", (1,))]
    assert r.read(ffa.RULE + 0x138) == 0xFFFFFFFF
    # Only a frozen body's respawn is cancelled: another place's native
    # schedule (a late arrival's first spawn) is left to run.
    r.word(ffa.RULE + 0x138 + 4, 5000)
    r.tick(4000)
    assert r.read(ffa.RULE + 0x138 + 4) == 5000
    assert len(r.thaws) == 1 and r.thaws[0][1] == 4000
    assert r.state()[7:10] == b"\x01\x00\x00"
    assert not r.player_tick(0, 4050) and not r.input(0)
    print("ok frozen body stays alive and immobile; native respawn cancelled; thaw at four seconds")

    # Real respawn serialization/deserialization restores rule progress.
    assert r.deserialize(before) == len(before)
    r.lethal(0)  # The enclosing native world's player load restores this.
    r.blocked = True
    r.tick(4000)
    assert r.state()[7:10] == b"\x03\x00\x00"
    r.blocked = False
    r.tick(4050)
    assert struct.unpack_from("<H", r.state(), 8)[0] == 50
    print("ok rollback restores thaw progress and a wall interrupts it")

    r.lethal(1)
    r.tick(4100)
    state = r.state()
    assert state[1:4] == b"\x01\x00\x01"  # Between rounds, 0:1.
    assert bytes(r.emu.mem_read(STATS + 0x31E4, 1)) == b"\x02"
    assert struct.unpack("<2H", r.emu.mem_read(STATS + 0x31D4, 4)) == (0, 1)
    assert r.freeze_hud(1) == [("root.freeze_tag_score", (0, 1, 5)),
                               ("root.freeze_tag_update", (1, 0, 5, 5000, 4000, 119950)),
                               ("root.freeze_tag_rows", (3,))]
    assert r.freeze_hud(2) == [("root.freeze_tag_score", (1, 0, 5)),
                               ("root.freeze_tag_update", (0, 0, 4, 5000, 4000, 0)),
                               ("root.freeze_tag_rows", (3,))]
    # Only the frozen stop in the pause (the owner, 2026-11-22: "only frozen people do"): the
    # winner's tick and input are his, and he keeps his camera; the frozen are held and see the
    # round's countdown (the warmup camera and the kits).
    # The score bars' native source is the hud state's +90h/+92h, which no Freeze Tag rule fills
    # and nothing clears at a new match: the last match's 7:3 is written over with the rule's.
    r.emu.mem_write(ffa.HUD + 0xB4, struct.pack("<2H", 7, 3))
    r.freeze_hud(1)
    assert struct.unpack("<2H", r.emu.mem_read(ffa.HUD + 0xB4, 4)) == (0, 1)
    print("ok the hud state's team points are the rule's (the last match's 7:3 written over)")
    assert not r.player_tick(2, 4150) and not r.input(2)
    assert r.player_tick(0, 4150) and r.input(0) and r.player_tick(1, 4150) and r.input(1)
    assert r.read(ffa.CORE + 0x18FE4) == 4
    assert r.countdown_view(2) == 3 and r.countdown_view(0) == 2 and r.countdown_view(None) == 2
    print("ok in the pause the winner moves on with his camera; the frozen are held and see the countdown")
    # Between rounds no place keeps a time (a death's respawn, a late
    # arrival's entry): the restart spawns everyone; the core's time state
    # is the round's countdown (4).
    r.word(ffa.RULE + 0x138 + 4, 9050)
    r.word(ffa.RULE + 0x138 + 8, 10000)
    r.tick(9050, 4950)
    assert [r.read(ffa.RULE + 0x138 + 4*i) for i in (1, 2)] == [0xFFFFFFFF, 0xFFFFFFFF]
    assert r.read(ffa.CORE + 0x18FE4) == 4
    before_restart = r.serialize()
    assert struct.unpack_from("<H", r.state(), 4)[0] == 50
    r.tick(9100)
    assert r.state()[1] == 2
    assert r.state()[-2:] == b"\x01\x00"  # One round start after the first.
    assert [r.read(ffa.RULE + 0x138 + 4*i) for i in range(3)] == [9100]*3
    # Rollback repeats the same spawn scheduling, exactly once.
    assert r.deserialize(before_restart) == len(before_restart)
    r.tick(9100)
    assert r.state()[-2:] == b"\x01\x00"  # Counted once, as the spawns.
    assert [r.read(ffa.RULE + 0x138 + 4*i) for i in range(3)] == [9100]*3
    # The native respawn path clears pending death. Its full remove/spawn
    # execution is covered by the existing world-roundtrip harness.
    for i in range(3):
        r.emu.mem_write(ffa.player(i) + 0x33D, b"\0")
    assert r.player_tick(2, 9100) and r.input(2)
    assert bytes(r.emu.mem_read(ffa.player(2) + 0x33D, 1)) == b"\0"
    r.tick(9150)
    assert r.state()[1:4] == b"\x00\x00\x01"
    assert not r.player_tick(2, 9200) and not r.input(2)
    for score in (2, 3, 4, 5):
        r.lethal(0)
        r.lethal(1)
        r.tick(9200)
        assert r.state()[3] == score
        if score < 5:
            r.tick(14200, 5000)
            for i in range(3):
                r.emu.mem_write(ffa.player(i) + 0x33D, b"\0")
            r.tick(14250)
    assert r.state()[1] == 3  # Match finished at five round wins.
    assert r.state()[-2:] == b"\x04\x00"  # The last round is not restarted.
    assert bytes(r.emu.mem_read(STATS + 0x31E4, 1)) == b"\x01"
    saved = r.serialize()
    r.tick(19000, 5000)
    assert r.serialize() == saved
    r.word(ffa.RULE + 0x138 + 8, 19050)
    r.tick(19050)
    assert r.read(ffa.RULE + 0x138 + 8) == 0xFFFFFFFF  # No spawn after the match.
    assert r.read(ffa.CORE + 0x18FE4) == 3  # No countdown after the match.
    print("ok best of nine: no time stands in the pause (its countdown state 4), the restart makes everyone due, "
          "rollback, first to five, then no countdown and no spawn")

    r.start()
    for place in range(3):
        r.lethal(place)
    r.tick(50)
    assert r.state()[1:4] == b"\x01\0\0" and r.state()[6] == 3
    r.tick(5050, 5000)
    assert r.state()[1] == 2 and r.state()[-2:] == b"\x01\x00"  # The replay is a round start.
    r.start(PLAIN)
    assert len(r.serialize()) == len(plain)
    print("ok simultaneous wipes draw and the replay counts a round start; a subsequent ordinary match has no Freeze Tag bytes")

    r.start()
    r.word(CAMERA_INPUT + 0x358, 0)
    r.tick(50)
    assert r.camera() == 0 and not r.camera_calls
    r.lethal(0)
    r.tick(100)
    r.word(CAMERA_INPUT + 0x338, 123)
    r.word(CAMERA_INPUT + 0x33C, 456)
    assert r.camera() == 3 and r.camera_calls == [CAMERA_INPUT + 4]
    assert r.read(CAMERA_INPUT + 0x338) == r.read(CAMERA_INPUT + 0x33C) == 0
    r.emu.mem_write(CAMERA_INPUT + 0x34C, struct.pack("<3f", .7, .2, 3.5))
    assert r.camera() == 3 and len(r.camera_calls) == 1
    assert struct.unpack("<3f", r.emu.mem_read(CAMERA_INPUT + 0x34C, 12))[2] == 3.5
    saved = r.serialize()
    for time in range(150, 4100, 50):
        r.tick(time)
    assert r.camera() == 0 and len(r.camera_calls) == 2
    r.deserialize(saved)
    assert r.camera() == 3 and len(r.camera_calls) == 3
    r.word(ffa.CORE + 0x18FE4, 1)  # Out of play: the orbit relinquished.
    assert r.camera() == 0 and len(r.camera_calls) == 4
    r.start()
    r.word(CAMERA_INPUT + 0x358, 3)  # An existing pregame camera is not ours.
    assert r.camera() == 3 and len(r.camera_calls) == 4
    r.word(CAMERA_INPUT + 0x358, 4)  # Nor is a spectator's camera.
    r.lethal(0)
    r.tick(100)
    assert r.camera() == 4 and len(r.camera_calls) == 4
    r.start()
    r.word(CAMERA_INPUT + 0x358, 2)
    r.lethal(0)
    r.tick(100)
    snapshot = r.serialize()
    assert r.camera() == 3 and r.serialize() == snapshot
    r.word(ffa.CORE + 0x18FE4, 1)
    assert r.camera() == 2
    r.start()
    r.word(CAMERA_INPUT + 0x358, 3)
    r.lethal(0)
    r.tick(100)
    assert r.camera() == 3  # Joining with an already-active warmup camera.
    r.word(ffa.CORE + 0x18FE4, 1)
    assert r.camera() == 0
    r.start()
    r.word(CAMERA_INPUT + 0x358, 2)
    r.lethal(0)
    r.tick(100)
    assert r.camera() == 3
    calls = len(r.camera_calls)
    r.word(ffa.CORE + 0x18FE4, 4)  # A round's countdown is the native camera's: ours leaves it.
    assert r.camera() == 3 and len(r.camera_calls) == calls
    assert kits.check_limits(bytes(patched), True) == 7

    # The frozen glow on the frozen body's bones (the owner, 2026-11-23: "just make the glow cover the
    # whole body, not just legs"): played as the effects' presenter plays a player's effect (0x2AC290),
    # with his hit model (+498h) and his model (+F290h), so its body-shaped emitters follow his
    # skeleton instead of all starting at his feet.
    r.emu.mem_map(GLOW, 0x300000)
    renderer, scene_renderer, systems = GLOW, GLOW + 0x200100, [GLOW + 0x210000 + 0x100 * i for i in range(20)]
    r.word(ffa.UI_GAME + 0x5360, renderer)
    r.word(renderer + 0x200064, scene_renderer)
    hit_model, model = 0x0BAD0498, 0x0BADF290
    r.start()
    r.word(ffa.player(0) + 0x498, hit_model)
    r.word(ffa.player(0) + 0xF290, model)
    r.word(CAMERA_INPUT + 0x358, 2)
    # asked once, at the first frame of a Freeze Tag match above, never answered until now
    assert [paths for paths, _, _ in r.queries] == [[("fx/artifacts/human_frozen", 61)] * 20], r.queries
    tag = 2                              # the particles' query word (resources.rs PARTICLES_WORD)
    answer = GLOW + 0x220000
    r.word(answer + 0x38, 20)
    for i, system in enumerate(systems):
        r.word(answer + 0x12C + 0x2E0 * i, system)
    sp = CAMERA_STACK + 0xFE000          # the query's callback: cdecl (queries_result, its word)
    r.emu.mem_write(sp, struct.pack("<3I", ffa.RETURN, answer, tag))
    r.run(base.arena().symbols["freeze_tag_on_loaded"], (ffa.RETURN,), sp, hide.GAME)
    assert r.stopped == ffa.RETURN
    assert all(r.read(system + 0xD0) == 1 for system in systems), "each system kept once"
    r.particles.clear()
    r.camera()
    assert r.particles == [], "nobody frozen: no glow"
    r.lethal(0)
    r.tick(100)
    r.camera()
    feet = struct.unpack("<3f", r.emu.mem_read(ffa.player(0) + 0x43C, 12))
    assert r.particles == [("play", systems[0], feet, hit_model, model)], r.particles
    assert r.read(systems[0] + 0xD0) == 2, "the play takes its reference"
    r.particles.clear()
    r.camera()
    assert r.particles == [("move", systems[0])], r.particles
    r.particles.clear()
    r.word(ffa.CORE + 0x18FE4, 1)  # out of play: the glow leaves the scene
    r.camera()
    assert r.particles == [("remove", systems[0])], r.particles
    print("ok the frozen glow is played on the frozen body's hit model and model (his bones), moved with him, "
          "removed out of play")

    # The gun in hand keeps the clock while held: its first tick after a thaw no longer steps the
    # whole freeze at once (the owner, 2026-11-22: "after unfreezing there are problems with mouse").
    r.start()
    gun = CAMERA_STACK + 0x20000
    r.emu.mem_write(gun, bytes(0x800))
    r.word(gun, WEAPON_VTABLE)
    r.word(gun + 0x5A4, 950)
    r.word(ffa.player(0) + 0x128, gun)
    r.lethal(0)
    assert r.player_tick(0, 1000) and r.read(gun + 0x5A4) == 1000
    assert r.player_tick(0, 1050) and r.read(gun + 0x5A4) == 1050
    r.word(gun, 0x00ABCDEF)                    # another item in hand: its fields are not a gun's
    r.word(gun + 0x5A4, 7)
    assert r.player_tick(0, 1100) and r.read(gun + 0x5A4) == 7
    r.word(ffa.player(0) + 0x128, 0)
    print("ok a held player's gun keeps its clock (its +5A4h the hold's time); another item is not touched")

    # Why: the game's own recoil spring (weapon_recoil_calculator::tick 0x331A50) on a gun frozen
    # 63 ms after a shot (the state of a frozen player's gun in the 2026-11-22 dump: its clock at
    # 52250, the match at 147000): one step of the whole freeze pins the kick's y at -1 for seconds;
    # with the clock kept the kick settles within a quarter of a second.
    def kick_after(first):
        r.emu.mem_write(gun, bytes(0x800))
        r.emu.mem_write(gun + 0x464, bytes.fromhex("32dcab3d"))
        recoil = gun + 0x468
        r.word(recoil, gun)
        r.word(recoil + 8, 52187)
        r.emu.mem_write(recoil + 0xC, struct.pack("<6f", -0.0045, 0.0288, 0.7218, 0.0512, 0.1725, 1.6604))
        r.emu.mem_write(recoil + 0x48, struct.pack("<2f", 5.0, 10.0))
        ys = []
        for i, dt in enumerate([first] + [50] * 39):
            sp = hide.STACK + 0x8000
            r.emu.mem_write(sp, struct.pack("<2I", ffa.RETURN, 147000 + 50 * i))
            r.run(0x331A50, (ffa.RETURN,), sp, {**hide.GAME, UC_X86_REG_ESI: recoil, UC_X86_REG_EAX: dt})
            assert r.stopped == ffa.RETURN
            ys.append(struct.unpack("<f", r.emu.mem_read(recoil + 0x10, 4))[0])
        return ys
    stepped, kept = kick_after(147000 - 52250), kick_after(50)
    assert all(y == -1.0 for y in stepped[:20]), stepped[:20]
    assert all(abs(y) < 0.03 for y in kept) and abs(kept[5]) < 0.01, kept
    print("ok the game's recoil spring: one step of the freeze pins the kick at -1 (1 s and more), a kept clock "
          "settles it")

    # A new match: the rule's 0:0 from its first frame, before anyone has entered.
    r.start()
    r.emu.mem_write(ffa.HUD + 0xB4, struct.pack("<2H", 7, 3))
    assert r.freeze_hud(0) == []                   # nobody entered yet: no Freeze Tag call
    assert struct.unpack("<2H", r.emu.mem_read(ffa.HUD + 0xB4, 4)) == (0, 0)
    print("ok the hud state's team points are the rule's 0:0 from a new match's first frame, nobody entered yet")

    # No place out of the world (freeze-tag.md): an entering place's spawn is never cancelled.
    r.start()
    r.lethal(2)                    # the second team's only player frozen: the pause
    r.tick(50)
    assert r.state()[1] == 1
    r.out_of_world(1)              # place 1 came back in the pause, not spawned yet
    r.word(ffa.RULE + 0x138 + 4, 100)
    r.word(ffa.RULE + 0x138, 100)  # place 0's body: a time of its, cancelled
    r.tick(100)
    assert r.read(ffa.RULE + 0x138 + 4) == 100, "the entry spawn runs in the pause"
    assert r.read(ffa.RULE + 0x138) == 0xFFFFFFFF
    print("ok a place that enters in the pause spawns at its entry (a body's time is still cancelled)")

    r.start()
    r.lethal(0)
    r.tick(50)                     # place 0 frozen, place 1 fighting: the round in play
    assert r.state()[1] == 0 and r.read(ffa.RULE + 0x138) == 0xFFFFFFFF
    r.out_of_world(0)              # it left and came back: frozen in the rule, out of the world
    r.tick(100)
    assert r.state()[7] == 3, "absent, it stays frozen"
    r.word(ffa.RULE + 0x138, 150)  # its entry
    r.tick(150)
    assert r.read(ffa.RULE + 0x138) == 150, "the frozen rejoiner's entry spawn runs"
    r.emu.mem_write(ffa.player(0) + 0x33C, b"\x01\x00\x00\x01")  # spawned: in the world, not lethal
    assert r.player_tick(0, 150)   # held: frozen in the rule, the hold makes it lethal
    assert bytes(r.emu.mem_read(ffa.player(0) + 0x33D, 1)) == b"\x01"
    r.tick(200)
    assert r.state()[7] == 3, "back in the world, frozen there"
    print("ok a frozen player who comes back spawns at his entry and stays frozen")

    # Why: what the game does to a player put in with link 0 that was out of the world (a world
    # load's 0x2D2D51) when it is next taken out with 1 (the restart's 0x3169A0, the leave handler's
    # 0x316C12): player::insert and player::remove themselves, base_player's halves kept to +33Fh.
    r.emu.mem_write(WHOLE, bytes(0x10000))
    assert r.whole_player(INSERT, 0) == (ffa.RETURN, [BASE_INSERT])
    assert r.read(WHOLE + 0x33F, 1)[0] == 1 and r.read(WHOLE + 0xF540) == 0, "in, no player effect"
    # Out with 1: its models out (the native's remove_model of a model not in the scene asserts
    # there), then the effect stopped: the read of the null effect, 2026-11-22's.
    assert r.whole_player(REMOVE, 1) == (NULL_EFFECT, [BASE_REMOVE, CLIENT_REMOVE])
    print("ok a player put in with link 0 from out of the world reads a null effect at 0x2DE36D when taken out "
          "with 1 (the crash an entry held out of the world leads to)")
    print("ok frozen camera reuses native warmup orbit/zoom and limits; thaw, rollback and leaving play restore "
          "ownership; a round's countdown leaves it to the native camera")


if __name__ == "__main__":
    main()
