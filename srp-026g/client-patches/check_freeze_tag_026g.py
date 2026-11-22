#!/usr/bin/env python3
"""Unicorn: installed Freeze Tag hooks, native lifecycle, wall rays and world state.
Run through tools/check freeze-native, with the original 0.26g executable.
"""
import struct
import sys
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI, UC_X86_REG_ESP, UC_X86_REG_EBP
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
# recorded, never answered, so nothing of the glow is drawn here (it is presentation only).
QUERY = 0xE5B60


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

    def on_query(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        requests, count = emu.reg_read(UC_X86_REG_EAX), emu.reg_read(UC_X86_REG_ECX)
        paths = [(self.text(self.read(requests + 8 * i)).decode(), self.read(requests + 8 * i + 4)) for i in range(count)]
        self.queries.append((paths, bytes(emu.mem_read(self.read(sp + 4), 0x20)), self.read(sp + 12)))
        self.leave(emu, 0)

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
    assert r.player_tick(2, 4150) and r.input(2)  # Winners also wait.
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
    print("ok frozen camera reuses native warmup orbit/zoom and limits; thaw, rollback and leaving play restore "
          "ownership; a round's countdown leaves it to the native camera")


if __name__ == "__main__":
    main()
