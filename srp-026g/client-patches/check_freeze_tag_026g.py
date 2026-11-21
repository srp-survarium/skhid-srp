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
LENGTH = 69
STATS = ffa.DATA + 0x60000
CAMERA_INPUT = ffa.DATA + 0x96000
CAMERA_STACK = 0x36000000
# The frozen glow (pack freeze-tag-glow): the game's query_resources stubbed (recorded), its
# callback through the game's invoker, game_effect_resource::emit native (its emitter stubbed), the
# effect player's add 0x2DE090 stubbed (recorded), the default clock native.
QUERY = 0xE5B60
INVOKER = 0x3AAA20
FUNCTION_VTABLE = 0x9DB9E0
DEFAULT_CLOCK_VT = 0xA11398
ADD_EFFECT = 0x2DE090
GLOW = ffa.DATA + 0xC0000


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
        self.queries, self.played, self.emitted = [], [], []
        self.next = GLOW + 0x100
        self.emu.mem_write(GLOW, b"\xc2\x0c\x00")        # the emitter's emit: ret 0Ch
        self.emu.mem_write(GLOW + 0x20, b"\xc2\x04\x00")  # its other slot: ret 4
        self.emu.hook_add(UC_HOOK_CODE, self.on_query, begin=QUERY, end=QUERY)
        self.emu.hook_add(UC_HOOK_CODE, self.on_emit, begin=GLOW, end=GLOW)
        self.emu.hook_add(UC_HOOK_CODE, self.on_add, begin=ADD_EFFECT, end=ADD_EFFECT)

    def block(self, size):
        at = self.next
        self.next += (size + 15) & ~15
        assert self.next < ffa.DATA + ffa.SIZE
        self.emu.mem_write(at, bytes(size))
        return at

    def on_query(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        requests, count = emu.reg_read(UC_X86_REG_EAX), emu.reg_read(UC_X86_REG_ECX)
        paths = [(self.text(self.read(requests + 8 * i)).decode(), self.read(requests + 8 * i + 4)) for i in range(count)]
        self.queries.append((paths, bytes(emu.mem_read(self.read(sp + 4), 0x20)), self.read(sp + 12)))
        self.leave(emu, 0)

    def on_emit(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        effect = self.block(0x60)
        self.word(effect + 0x1C, 1)
        self.word(self.read(sp + 4), effect)
        self.emitted.append(effect)

    def on_add(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.played.append((emu.reg_read(UC_X86_REG_EDI) - 0x358, self.read(self.read(sp + 4)), self.read(sp + 8),
                            self.read(sp + 20)))
        effect = self.read(self.read(sp + 4))
        self.word(effect + 0x1C, self.read(effect + 0x1C) + 1)   # the native add takes its own reference
        self.leave(emu, 20)

    def call_native(self, va, args, regs=()):
        sp = hide.STACK + 0x8000
        self.emu.mem_write(sp, struct.pack("<%dI" % (len(args) + 1), ffa.RETURN, *args))
        self.run(va, (ffa.RETURN,), sp, dict(regs))
        assert self.stopped == ffa.RETURN, hex(va)

    def deliver(self):
        """The glow's query answered through the game's invoker with an effect resource (its
        emitter stubbed): the patch keeps one reference of its own. The resource."""
        paths, function, user = self.queries[0]
        vtable, _entry, word = struct.unpack_from("<I4xII", function)
        assert paths == [("freeze_tag_frozen", 109)] and user == 0 and (vtable, word) == (FUNCTION_VTABLE | 1, 1), self.queries
        resource, emitter, table = self.block(0x120), self.block(4), self.block(0x30)
        self.word(emitter, table)
        self.word(table + 0x20, GLOW)
        self.word(table + 0x28, GLOW + 0x20)
        self.word(resource + 0x108, emitter)
        self.word(resource + 0x114, struct.unpack("<I", struct.pack("<f", 1.0))[0])
        self.word(resource + 0xD0, 1)
        results = self.block(0x12C + 0x2E0)
        self.word(results + 0x38, 1)
        self.word(results + 0x12C, resource)
        at = self.block(0x20)
        self.emu.mem_write(at, function)
        self.call_native(INVOKER, (at + 8, results))
        assert self.read(resource + 0xD0) == 2, "kept: one reference of its own"
        return resource

    def playing(self, place, effect):
        """The body's effect list: one node playing `effect`, or none."""
        node = 0
        if effect is not None:
            node = self.block(0x50)
            self.word(node, effect)
        self.word(ffa.player(place) + 0x358 + 0x2C, node)

    def effect_clock(self, place, effect, start=100, now=5000):
        """The body's own calculator on a node of `effect`: whether it says the effect ended."""
        node, out = self.block(0x50), self.block(12)
        self.word(node, effect)
        self.word(node + 0x18, start)
        self.call_native(self.read(DEFAULT_CLOCK_VT), (out, node, start, now), {UC_X86_REG_ECX: ffa.player(place) + 0x394})
        return self.emu.mem_read(out + 8, 1)[0]

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
            self.word(p + 0x394, DEFAULT_CLOCK_VT)             # the body's own calculator
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
    for time in range(50, 3000, 50):
        r.tick(time)
    before = r.serialize()
    state = before[-LENGTH:]
    assert state[:2] == b"\x01\x00"
    assert state[7] == 3 and struct.unpack_from("<H", state, 8)[0] == 2950
    assert r.freeze_hud(0) == [("root.freeze_tag_score", (0, 0)),
                               ("root.freeze_tag_update", (1, 2950, 0, 0))]
    assert r.freeze_hud(1) == [("root.freeze_tag_score", (0, 0)),
                               ("root.freeze_tag_update", (0, 2950, 0, 0))]
    assert r.read(ffa.RULE + 0x138) == 0xFFFFFFFF
    # Only a frozen body's respawn is cancelled: another place's native
    # schedule (a late arrival's first spawn) is left to run.
    r.word(ffa.RULE + 0x138 + 4, 5000)
    r.tick(3000)
    assert r.read(ffa.RULE + 0x138 + 4) == 5000
    assert len(r.thaws) == 1 and r.thaws[0][1] == 3000
    assert r.state()[7:10] == b"\x01\x00\x00"
    assert not r.player_tick(0, 3050) and not r.input(0)
    print("ok frozen body stays alive and immobile; native respawn cancelled; thaw at three seconds")

    # Real respawn serialization/deserialization restores rule progress.
    assert r.deserialize(before) == len(before)
    r.lethal(0)  # The enclosing native world's player load restores this.
    r.blocked = True
    r.tick(3000)
    assert r.state()[7:10] == b"\x03\x00\x00"
    r.blocked = False
    r.tick(3050)
    assert struct.unpack_from("<H", r.state(), 8)[0] == 50
    print("ok rollback restores thaw progress and a wall interrupts it")

    r.lethal(1)
    r.tick(3100)
    state = r.state()
    assert state[1:4] == b"\x01\x00\x01"  # Between rounds, 0:1.
    assert bytes(r.emu.mem_read(STATS + 0x31E4, 1)) == b"\x02"
    assert struct.unpack("<2H", r.emu.mem_read(STATS + 0x31D4, 4)) == (0, 1)
    assert r.freeze_hud(1) == [("root.freeze_tag_score", (0, 1)),
                               ("root.freeze_tag_update", (1, 0, 5, 5000))]
    assert r.freeze_hud(2) == [("root.freeze_tag_score", (1, 0)),
                               ("root.freeze_tag_update", (0, 0, 4, 5000))]
    assert r.player_tick(2, 3150) and r.input(2)  # Winners also wait.
    # Between rounds whatever falls due waits for the restart (a late
    # arrival enters with the next round); a later time is left alone.
    r.word(ffa.RULE + 0x138 + 4, 8050)
    r.word(ffa.RULE + 0x138 + 8, 9000)
    r.tick(8050, 4950)
    assert [r.read(ffa.RULE + 0x138 + 4*i) for i in (1, 2)] == [0xFFFFFFFF, 9000]
    before_restart = r.serialize()
    assert struct.unpack_from("<H", r.state(), 4)[0] == 50
    r.tick(8100)
    assert r.state()[1] == 2
    assert r.state()[-2:] == b"\x01\x00"  # One round start after the first.
    assert [r.read(ffa.RULE + 0x138 + 4*i) for i in range(3)] == [8100]*3
    # Rollback repeats the same spawn scheduling, exactly once.
    assert r.deserialize(before_restart) == len(before_restart)
    r.tick(8100)
    assert r.state()[-2:] == b"\x01\x00"  # Counted once, as the spawns.
    assert [r.read(ffa.RULE + 0x138 + 4*i) for i in range(3)] == [8100]*3
    # The native respawn path clears pending death. Its full remove/spawn
    # execution is covered by the existing world-roundtrip harness.
    for i in range(3):
        r.emu.mem_write(ffa.player(i) + 0x33D, b"\0")
    assert r.player_tick(2, 8100) and r.input(2)
    assert bytes(r.emu.mem_read(ffa.player(2) + 0x33D, 1)) == b"\0"
    r.tick(8150)
    assert r.state()[1:4] == b"\x00\x00\x01"
    assert not r.player_tick(2, 8200) and not r.input(2)
    for score in (2, 3):
        r.lethal(0)
        r.lethal(1)
        r.tick(8200)
        assert r.state()[3] == score
        if score == 2:
            r.tick(13200, 5000)
            for i in range(3):
                r.emu.mem_write(ffa.player(i) + 0x33D, b"\0")
            r.tick(13250)
    assert r.state()[1] == 3  # Match finished at three round wins.
    assert r.state()[-2:] == b"\x02\x00"  # The last round is not restarted.
    assert bytes(r.emu.mem_read(STATS + 0x31E4, 1)) == b"\x01"
    saved = r.serialize()
    r.tick(18000, 5000)
    assert r.serialize() == saved
    r.word(ffa.RULE + 0x138 + 8, 18050)
    r.tick(18050)
    assert r.read(ffa.RULE + 0x138 + 8) == 0xFFFFFFFF  # No spawn after the match.
    print("ok best of five: pause holds what falls due, the restart makes everyone due, rollback and first to three")

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

    # The frozen glow: asked once, at the first tick of a Freeze Tag match; nothing plays until it
    # arrives. Then a frozen body's tick plays it once, from the body's time with its own
    # calculator, which ends it at the thaw, at the round's restart and at the match's end.
    assert len(r.queries) == 1, r.queries
    resource = r.deliver()
    r.start()
    r.played.clear()
    assert not r.player_tick(1, 100) and r.played == [], "a fighting body: none"
    r.lethal(0)
    r.tick(100)
    assert r.player_tick(0, 150)
    assert len(r.played) == 1, r.played
    body, effect, time, calculator = r.played[0]
    assert (body, time, calculator) == (ffa.player(0), 150, ffa.player(0) + 0x394), r.played
    assert r.read(effect + 0x10) == resource, "emitted from the kept resource"
    r.playing(0, effect)
    r.played.clear()
    assert r.player_tick(0, 200) and r.played == [], "a body playing it gets no other"
    assert r.effect_clock(0, effect) == 0, "frozen: it goes on"
    for time in range(150, 3200, 50):
        r.tick(time)
    assert r.thaws and r.effect_clock(0, effect) == 1, "thawed: it ends"
    r.lethal(0)
    r.lethal(1)
    r.tick(3250)
    assert r.state()[1] == 1 and r.effect_clock(0, effect) == 0, "frozen between rounds: it goes on"
    r.tick(8250, 5000)
    for i in range(3):
        r.emu.mem_write(ffa.player(i) + 0x33D, b"\0")
    r.tick(8300)
    assert r.state()[1] == 0 and r.effect_clock(0, effect) == 1, "the round restarted: it ends"
    r.lethal(0)
    r.tick(8350)
    assert r.effect_clock(0, effect) == 0
    r.word(ffa.CORE + 0x18FE4, 4)
    assert r.effect_clock(0, effect) == 1, "the match ended: it ends"
    r.start(PLAIN)
    assert r.effect_clock(0, effect) == 1, "another match: it ends"
    print("ok frozen glow asked once, played on a frozen body from its time with its own calculator, ended at the thaw, the restart and the match's end")

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
    for time in range(150, 3100, 50):
        r.tick(time)
    assert r.camera() == 0 and len(r.camera_calls) == 2
    r.deserialize(saved)
    assert r.camera() == 3 and len(r.camera_calls) == 3
    r.word(ffa.CORE + 0x18FE4, 4)  # Native match end relinquishes the orbit.
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
    r.word(ffa.CORE + 0x18FE4, 4)
    assert r.camera() == 2
    r.start()
    r.word(CAMERA_INPUT + 0x358, 3)
    r.lethal(0)
    r.tick(100)
    assert r.camera() == 3  # Joining with an already-active warmup camera.
    r.word(ffa.CORE + 0x18FE4, 4)
    assert r.camera() == 0
    assert kits.check_limits(bytes(patched), True) == 7
    print("ok frozen camera reuses native warmup orbit/zoom and limits; thaw, rollback and match end restore ownership")


if __name__ == "__main__":
    main()
