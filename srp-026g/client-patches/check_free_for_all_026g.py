#!/usr/bin/env python3
"""Patch 142 (free_for_all): each player of a free-for-all his own side
(docs/notes/ffa.md). srp gives each a team of his own (4 + his place).

    python check_free_for_all_026g.py <original survarium.exe>

Requires Python `unicorn`. Runs the installed Rust hooks in the patched exe on
made-up objects, with the game's own code around them (check_blackout_026g.py's
harness: Scaleform's Invoke 0x511600 a stub that records each call):

- the respawn tick's batching (0x3168CE, to the lea esi after the replayed
  lea edx / mov eax,[ebp-10h]): teams 0 and 1 go into their own vector as
  before; a team of 2 or more into the first (EDX 0), not past the two;
- the whole select_respawn_points (0x3171D0, ret 18h) for a vector of
  free-for-all players, with the game's own clear test (0x2F4800, its sqrt,
  the players' transforms through their vtables) and push_back (0x3174C0):
  each due player chooses among all points that pass the clear test and
  have no point already given at this tick inside their enemy radius, using
  a deterministic integer mix of time and place; with none he gets
  one of the five best-separated points; every due player gets a point. Hand-made cases, and
  300 seeded random ones against this file's own model of the rule. The
  callee-saved registers and the stack as the game's ret 18h leaves them. A
  vector of teams 0 and 1 takes the game's own path;
- resupply_zone_core::can_use (0x2F8080) and use_hint (0x2F80C0): no
  ammunition refill in a free-for-all (the owner, 2026-11-12): a player of
  his own side is refused at a zone of every team value (0, 1, 2, 3, his own
  side's number, another's, 0xFFFFFFFF), by the game's own "no" (0x2F80BB,
  0x2F80DC) before its test of the team, the inventory test never reached;
  a player of team 0 or 1 is served exactly as by the original exe (its
  team's zone and team 2's), result, inventory test and path alike;
- the options read (process_static_match_info 0x27B92D, a replay's chunk 1
  0x1F0971) with srp's block: tag 3, two bytes, is the kill target of a
  free-for-all; the hud movie is then told root.set_ffa_progress(own, top,
  target) at update_player_icons' start (0x23AA10) from the hud state (the
  viewed place +24h, the kd records +BCh): at each change, again after
  initialize_match's start (0x234880), again while the movie refuses, never
  from a place past the match's, and nothing in a match without the tag;
- the welcome (update_ui 0x23B32D..0x23B371): mode 8 has no text in the
  game (nor in patch 53's stub), the hook shows the dictionary's
  st_start_match_welcome_message_8 with the target in it, through
  root.set_parameterized_message(text, 21h, 14h, 6000); modes 2 to 5 as
  before;
- the end: the live roster, the serialized KD counters frozen by the
  native death-event handler at the target or exact time limit, and the
  viewed place's victory/defeat text only after actual completion; restoring
  the records through the native deserializer restores scoring too.

The original exe runs the same sites and invokes nothing; its mode switch
passes mode 8 by (0x23B336..0x23B36A, no message).
"""
import random
import struct
import sys
from unicorn import UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import *
import patch_026g as base
import check_hide_ui_026g as hide
import check_blackout_026g as blackout

DUE, DUE_AFTER = 0x3168CE, 0x3168D4            # lea edx,[eax+eax*2] (3) / mov eax,[ebp-10h] (3)
SELECT = 0x3171D0                              # thiscall (team, due, living, a, time, chosen); ret 18h
CAN_USE, USE_HINT = 0x2F8080, 0x2F80C0         # thiscall (&player); ret 4
TEAM_TEST = {CAN_USE: 0x2F808F, USE_HINT: 0x2F80CF}    # the zone's own test of the team
ALLOWED = {CAN_USE: 0x2F809C, USE_HINT: 0x2F80E4}      # its allowed path: patch 85's pickup sites
REFUSED = {CAN_USE: 0x2F80BB, USE_HINT: 0x2F80DC}      # its own "no": xor eax,eax / mov eax,9DB708h; ret 4
INVENTORY_TEST = 0x2FDF50                      # (inventory, profile) on the stack, AL the kit; pops 8, 0 = may
NOT_HERE = 0x9DB708                            # use_hint's "no" string
INITIALIZE, INITIALIZE_AFTER = 0x234880, 0x234886
WELCOME_START, WELCOME_SITE, WELCOME_PAST = 0x23B32D, 0x23B36A, 0x23B371
SHOW_MESSAGE, TRANSLATE = 0x2375C0, 0x1FF0F0   # (key in ECX; ui, 21h, 14h, ms), ret 10h; thiscall (key, out), ret 8
PROGRESS, MESSAGE = "root.set_ffa_progress", "root.set_parameterized_message"
TEXTS = {
    b"st_start_match_welcome_message_8": b"Free for all. First to %1$s kills wins",
    b"st_finish_match_mode_08_winner_message": b"Victory",
    b"st_finish_match_mode_08_loser_message": b"Defeat",
}

DATA = 0x31000000
SIZE = 0x100000
RULE, DUE_V, LIVING_V, CHOSEN_V = DATA, DATA + 0x1000, DATA + 0x1100, DATA + 0x1200
DUE_A, LIVING_A, CHOSEN_A = DATA + 0x1300, DATA + 0x1400, DATA + 0x1500
PLAYERS = DATA + 0x10000                       # 40 players, 1000h each
PROFILES = DATA + 0x40000                      # their profiles' teams (+45ECh), 10h apart
GETTER = DATA + 0x3000                         # the players' transform vtable: +28h the getter
CODE = DATA + 0x4000                           # per player: mov eax, <its transform>; ret
ZONE, PLAYER_REF, RETURN = DATA + 0x5000, DATA + 0x5100, DATA + 0x5200
UI_WORLD, UI_GAME = DATA + 0x6000, DATA + 0x7000   # the hud's game world (+18h) and its game (+9Ch)
POINT_SIZE = 0x38
HUD = hide.HUD
PVP, CORE = DATA + 0x70000, DATA + 0x701F8
TIME_RULE, KD_RULE, RULE_LIST, EVENT = DATA + 0xB000, DATA + 0xA000, DATA + 0xB800, DATA + 0xB900
CLIENT, CLIENT_TABLE, STALE_OPTIONS = 0x33000000, DATA + 0x9000, 0x35000000
PLACES = blackout.PLACE_COUNT                  # empty_options' places


def player(i):
    return PLAYERS + 0x1000 * i


def profile(i):
    return PROFILES - 0x45EC + 0x10 * i


class Run(blackout.Run):
    def __init__(self, data):
        super().__init__(data)
        self.emu.mem_map(DATA, SIZE, UC_PROT_ALL)
        self.tested, self.native_messages, self.passed = [], [], set()
        self.emu.hook_add(UC_HOOK_CODE, self.inventory_test, begin=INVENTORY_TEST, end=INVENTORY_TEST)
        self.emu.hook_add(UC_HOOK_CODE, self.show_message, begin=SHOW_MESSAGE, end=SHOW_MESSAGE)
        self.emu.hook_add(UC_HOOK_CODE, self.translate, begin=TRANSLATE, end=TRANSLATE)
        for at in (*TEAM_TEST.values(), *ALLOWED.values(), *REFUSED.values()):
            self.emu.hook_add(UC_HOOK_CODE, lambda emu, address, *_: self.passed.add(address), begin=at, end=at)
        self.word(HUD + 0x18, UI_WORLD)
        self.word(UI_WORLD + 0x9C, UI_GAME)
        self.emu.mem_map(CLIENT, 0x200000)
        self.word(UI_GAME + 0x53AC, CLIENT)
        self.word(CLIENT, CLIENT_TABLE)
        self.word(CLIENT_TABLE + 0x34, 0x219990)
        self.emu.mem_map(STALE_OPTIONS, 0x60000)
        self.emu.mem_write(STALE_OPTIONS + 0x577B5, bytes([PLACES]))
        self.word(CLIENT + 0x1F1C48, STALE_OPTIONS - 12)
        self.word(PVP + 0x124, blackout.OPTIONS)
        self.word(UI_WORLD + 0x4D4C, PVP)
        self.word(CORE + 0x18824, RULE_LIST)
        self.word(CORE + 0x18828, RULE_LIST + 4)
        self.word(RULE_LIST, TIME_RULE)
        self.word(TIME_RULE + 0x10C, 2)
        self.word(TIME_RULE + 0x12C, 20_000)
        self.word(TIME_RULE + 0x138, 900_000)
        self.word(KD_RULE + 0x1B4, CORE)
        self.word(CORE + 0x18458, CORE + 0x18464)
        for i in range(20):
            self.make_player(i, 4 + i, 0, (0, 0, 0))
            self.word(CORE + 0x18464 + 48 * i, player(i))

    def run(self, start, stops, sp, regs, count=40_000_000):
        for reg, value in regs.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.stops, self.stopped, self.started, self.calls, self.invoked = stops, None, False, [], []
        self.native_messages = []
        self.emu.emu_start(start, 0, count=count)
        return {reg: self.emu.reg_read(reg) for reg in hide.REGS}, self.emu.reg_read(UC_X86_REG_ESP)

    def leave(self, emu, pops):
        sp = emu.reg_read(UC_X86_REG_ESP)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)

    def inventory_test(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.tested.append((self.read(sp + 4), self.read(sp + 8)))
        emu.reg_write(UC_X86_REG_EAX, 0)
        self.leave(emu, 8)

    def show_message(self, emu, *_):
        """The game's show_parametrized_message: its key and numbers."""
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.native_messages.append((self.text(emu.reg_read(UC_X86_REG_ECX)),
                                     *(self.read(sp + 4 * i) for i in range(1, 5))))
        self.leave(emu, 16)

    def translate(self, emu, *_):
        """The game's translate_text: the dictionary's text (the key itself without one)."""
        sp = emu.reg_read(UC_X86_REG_ESP)
        key = self.text(self.read(sp + 4))
        assert emu.reg_read(UC_X86_REG_ECX) == UI_GAME + 0x52A8, hex(emu.reg_read(UC_X86_REG_ECX))
        emu.mem_write(self.read(sp + 8), TEXTS.get(key, key) + b"\0")
        self.leave(emu, 8)

    def due(self, team):
        sp = hide.STACK + 0x9000
        frame = sp + 0x100
        self.word(frame - 0x10, 0x7EC70000)
        regs = {**hide.GAME, UC_X86_REG_EAX: team, UC_X86_REG_EBP: frame}
        after, sp_after = self.run(DUE, (DUE_AFTER,), sp, regs)
        assert self.stopped == DUE_AFTER and sp_after == sp, hex(self.stopped or 0)
        return after[UC_X86_REG_EDX], after[UC_X86_REG_EAX]

    def make_player(self, i, team, place, position):
        """Player i: its place, its profile's team, its simulated transform
        (+40Ch, which the getter at [+118h]'s vtable +28h returns, as the
        game's 0x1F17C0; the position its +30h, the player's +43Ch)."""
        at = player(i)
        self.emu.mem_write(at, bytes(0x1000))
        self.emu.mem_write(at + 0x114, bytes([place]))
        self.word(at + 0x6B8, profile(i))
        self.word(profile(i) + 0x45EC, team)
        self.word(at + 0x118, GETTER + 0x40 * i)
        self.word(GETTER + 0x40 * i + 0x28, CODE + 0x10 * i)
        self.emu.mem_write(CODE + 0x10 * i, b"\xB8" + struct.pack("<I", at + 0x40C) + b"\xC3")
        self.emu.mem_write(at + 0x43C, struct.pack("<3f", *position))

    def vector(self, head, array, items, capacity=40):
        for i, item in enumerate(items):
            self.word(array + 4 * i, item)
        self.word(head, array)
        self.word(head + 4, array + 4 * len(items))
        self.word(head + 8, array + 4 * capacity)

    def select(self, points, due, living, time, team=0):
        """points: (position, enemy radius) of the team-2 group (ally radius
        1); due, living: player indices. The chosen points' indices."""
        self.emu.mem_write(RULE, bytes(0x1000))
        first = RULE + 0x200
        for k, (position, radius) in enumerate(points):
            p = first + POINT_SIZE * k
            self.emu.mem_write(p + 8, struct.pack("<3f", *position))
            self.emu.mem_write(p + 0x30, struct.pack("<2f", 1.0, radius))
        self.word(RULE + 0x190, first if points else 0)
        self.word(RULE + 0x19C, len(points))
        self.vector(DUE_V, DUE_A, [player(i) for i in due])
        self.vector(LIVING_V, LIVING_A, [player(i) for i in living])
        self.vector(CHOSEN_V, CHOSEN_A, [])
        sp = hide.STACK + 0x9000 - 0x1C
        self.emu.mem_write(sp, struct.pack("<7I", RETURN, team, DUE_V, LIVING_V, 0x12345, time, CHOSEN_V))
        regs = {**hide.GAME, UC_X86_REG_ECX: RULE}
        after, sp_after = self.run(SELECT, (RETURN,), sp, regs)
        assert self.stopped == RETURN, hex(self.stopped or 0)
        assert sp_after == sp + 0x1C, hex(sp_after - sp)
        for reg in (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP):
            assert after[reg] == hide.GAME[reg], reg
        begin, end = self.read(CHOSEN_V), self.read(CHOSEN_V + 4)
        return [(self.read(begin + 4 * i) - first) // POINT_SIZE for i in range((end - begin) // 4)]

    def zone(self, function, zone_team, team):
        """The zone's function for a player of `team`: its result, what the
        inventory test was given, and which of the zone's own team test and
        its allowed path's start ran."""
        self.make_player(0, team, 0, (0, 0, 0))
        self.word(PLAYER_REF, player(0))
        self.word(ZONE + 0x54, zone_team)
        sp = hide.STACK + 0x9000 - 8
        self.emu.mem_write(sp, struct.pack("<II", RETURN, PLAYER_REF))
        self.tested, self.passed = [], set()
        after, sp_after = self.run(function, (RETURN,), sp, {**hide.GAME, UC_X86_REG_ECX: ZONE})
        assert self.stopped == RETURN and sp_after == sp + 8
        return after[UC_X86_REG_EAX], self.tested, self.passed

    def hud_state(self, viewed, kills, pregame, seconds, remaining_ms=None):
        """The hud state as the rules fill it: the viewed place (None: none),
        each place's kills, the time rule's pregame flag and seconds left."""
        self.emu.mem_write(HUD + 0x24, bytes([0xFF if viewed is None else viewed]))
        self.word(HUD + 0x28, seconds)
        self.word(TIME_RULE + 0x148, 2 if pregame else 3)
        left = seconds * 1000 if remaining_ms is None else remaining_ms
        self.word(CORE + 0x18FC0, 920_000 - left)
        self.emu.mem_write(HUD + 0x15C, bytes([pregame]))
        for place in range(20):
            count = kills[place] if place < len(kills) else 0
            self.emu.mem_write(HUD + 0xBC + 8 * place, struct.pack("<HH", count, 0))

    def kill(self, killer, victim):
        self.emu.mem_write(EVENT, bytes([killer, victim]))
        sp = hide.STACK + 0x8000
        self.emu.mem_write(sp, struct.pack("<4I", RETURN, 1, EVENT, self.read(CORE + 0x18FC0)))
        given = {**hide.GAME, UC_X86_REG_ECX: KD_RULE}
        regs, after = self.run(0x3237E0, (RETURN,), sp, given)
        assert self.stopped == RETURN and after == sp + 16
        assert all(regs[r] == given[r] for r in (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP))

    def restore_kd(self, records):
        # The real kd_stats_rule::deserialize, including its record
        # count and the packed five-byte wire records. This is the path
        # a loaded world takes during rollback, late join and replay.
        count = self.emu.mem_read(KD_RULE + 0x1B0, 1)[0]
        wire = b''.join(records[i * 8:i * 8 + 5] for i in range(count))
        reader, buffer = EVENT + 0x100, EVENT + 0x200
        self.emu.mem_write(buffer, wire)
        for offset, value in ((0, buffer), (4, buffer), (8, len(wire))):
            self.word(reader + offset, value)
        sp = hide.STACK + 0x8000
        self.emu.mem_write(sp, struct.pack('<3I', RETURN, reader, self.read(CORE + 0x18FC0)))
        self.run(0x323910, (RETURN,), sp, {**hide.GAME, UC_X86_REG_ECX: KD_RULE})
        assert self.stopped == RETURN and self.read(reader + 4) == buffer + len(wire)

    def frame(self):
        """update_player_icons' start: whether it went on as the game's, and
        this patch's Invokes of the hud movie."""
        back, invoked = self.icons_fed(others=True)
        return back, [(name, values) for movie, name, values in invoked
                      if name in (PROGRESS, MESSAGE) and movie == hide.HUD_MOVIE]

    def initialize(self):
        """initialize_match's start, to past its replayed `sub esp,5D4h`."""
        sp = hide.STACK + 0x8000
        self.emu.mem_write(sp, struct.pack("<3I", RETURN, HUD, 0x0F0F0F0F))
        regs, esp = self.run(INITIALIZE, (INITIALIZE_AFTER,), sp, dict(hide.GAME))
        return self.stopped == INITIALIZE_AFTER and esp == sp - 0x5D4 \
            and all(regs[r] == v for r, v in hide.GAME.items())

    def welcome(self, mode):
        """update_ui from its test of the welcome's mark to past the mark, the
        hud's mode `mode`: whether it went as the game's, the game's own
        messages (show_parametrized_message's) and ours."""
        self.movies(HUD, 1, 1)
        self.word(HUD + 0x20, mode)
        self.emu.mem_write(HUD + 0x370, b"\0")
        sp = hide.STACK + 0x8000
        given = {**hide.GAME, UC_X86_REG_EDI: HUD}
        regs, esp = self.run(WELCOME_START, (WELCOME_PAST,), sp, given)
        kept = self.stopped == WELCOME_PAST and esp == sp and self.read(HUD + 0x370) & 0xFF == 1 \
            and all(regs[r] == v for r, v in given.items() if r not in (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX))
        ours = [(name, values) for movie, name, values in self.invokes() if name == MESSAGE]
        return kept, self.native_messages, ours


def model(points, due, living, time):
    """This file's own statement of the rule: for each due (place) in order,
    the point he gets. points: (position, radius); living: positions. The
    coordinates are multiples of 0.25 m, so every square and sum is exact."""
    def d2(a, b):
        return sum((x - y) ** 2 for x, y in zip(a, b))
    chosen = []
    for place in due:
        others = living + [points[k][0] for k in chosen]
        rank = [min((d2(p, o) for o in others), default=float("inf")) for p, _ in points]
        passing = [j for j, (p, r) in enumerate(points)
                   if j not in chosen and all(d2(p, o) >= r * r for o in others)]
        pool = passing or sorted(range(len(points)), key=lambda j: (-rank[j], j))[:5]
        mixed = (time + place * 0x9e3779b9) & 0xffffffff
        mixed = ((mixed ^ (mixed >> 16)) * 0x7feb352d) & 0xffffffff
        mixed = ((mixed ^ (mixed >> 15)) * 0x846ca68b) & 0xffffffff
        chosen.append(pool[(mixed ^ (mixed >> 16)) % len(pool)])
    return chosen


def check_protection(patched):
    """Run the original seconds conversion and the installed optional-ms hook."""
    r = Run(bytes(patched))
    point, config, legacy, override = DATA + 0xC0000, DATA + 0xC1000, DATA + 0xC2000, DATA + 0xC3000
    r.word(config + 0x14, 3)  # relocated named table, no iterated children
    r.word(legacy, 2)
    current = [None]

    def has(emu, *_):
        assert emu.reg_read(UC_X86_REG_EAX) == config
        assert r.text(emu.reg_read(UC_X86_REG_ESI)) == b'invulnerability_time_ms'
        emu.reg_write(UC_X86_REG_EAX, int(current[0] is not None))
        r.leave(emu, 0)

    def child(emu, *_):
        assert emu.reg_read(UC_X86_REG_EAX) == config
        key = r.text(emu.reg_read(UC_X86_REG_EDI))
        assert key in (b'invulnerability_time', b'invulnerability_time_ms'), key
        emu.reg_write(UC_X86_REG_EAX, legacy if key == b'invulnerability_time' else override)
        r.leave(emu, 0)

    r.emu.hook_add(UC_HOOK_CODE, has, begin=0xF6C00, end=0xF6C00)
    r.emu.hook_add(UC_HOOK_CODE, child, begin=0xF6B10, end=0xF6B10)
    for value, expected in [(None, 2000), ((1, 500), 500), ((1, 0), 0),
                            ((1, 2000), 2000), ((1, 0xFFFFFFFF), 2000),
                            ((2, 0x3F000000), 2000)]:
        current[0] = value
        if value is not None:
            kind, data = value
            r.word(override, data)
            r.word(override + 0x14, kind)
        r.word(point + 0x20, 0x12345678)
        sp = hide.STACK + 0x9000
        regs = {**hide.GAME, UC_X86_REG_EBP: point, UC_X86_REG_EBX: config,
                UC_X86_REG_ESI: 0xA12CC4}
        after, end_sp = r.run(0x2F461F, (0x2F4638,), sp, regs)
        assert r.stopped == 0x2F4638 and end_sp == sp
        assert r.read(point + 0x20) == expected, (value, r.read(point + 0x20))
        assert after[UC_X86_REG_EBX] == config and after[UC_X86_REG_EBP] == point
        assert after[UC_X86_REG_ESI] == 0xA12CC4 and after[UC_X86_REG_EDI] == 0x9E1C18
    print('ok native seconds load preserved; optional 500 ms and zero exact; invalid overrides ignored')


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], 'rb').read()
    patched, _ = base.build(original, 'section', open(base.default_bin('section'), 'rb').read(), None,
                            open(base.late_join_bin(), 'rb').read())
    check_protection(patched)
    stock, r = Run(original), Run(bytes(patched))

    assert r.due(0) == (0, 0x7EC70000) and r.due(1) == (3, 0x7EC70000)
    print('ok teams 0 and 1 go into their own vectors as before')
    for team in (2, 4, 13, 23, 0xFFFFFFFF):
        assert r.due(team) == (0, 0x7EC70000), team
    print('ok a team of 2 or more goes into the first vector, never past the two')

    for i in range(10):
        r.make_player(i, 4 + i, i, (1000.0 + 100 * i, 0, 0))
    line = lambda *xs: [((x, 0, 0), 30) for x in xs]
    # A living player at 0: the ranks rise with x; the point at 10 is inside his radius.
    points = line(10, 40, 100, 70, 200)
    r.make_player(20, 30, 10, (0, 0, 0))
    seen = set()
    for time in range(0, 5_000, 50):
        got = r.select(points, [0], [20], time)
        assert got == model(points, [0], [(0, 0, 0)], time)
        seen.update(got)
    assert seen == {1, 2, 3, 4}, seen
    print('ok every safe point is eligible, including the nearest safe point formerly excluded')
    for time in (0, 50, 100, 0xffffffff):
        points = line(10, 40, 100)
        assert r.select(points, [0], [20], time) == model(points, [0], [(0, 0, 0)], time)
        points = line(0, 10, 40, 100, 170)
        got = r.select(points, [0, 1, 2], [], time)
        assert got == model(points, [0, 1, 2], [], time)
        assert all(abs(points[a][0][0] - points[b][0][0]) >= 30
                   for i, a in enumerate(got) for b in got[:i])
    print('ok safe selection at clock wrap; simultaneous spawns keep their separation')
    points = [((0, 0, 40), 30)] + [((x, 0, z), 30) for x in (-100, 100) for z in (-100, 100)]
    visits = [0] * len(points)
    for event in range(500):
        got = r.select(points, [0], [20], event * 5_000)
        assert got == model(points, [0], [(0, 0, 0)], event * 5_000)
        visits[got[0]] += 1
    assert all(60 <= n <= 140 for n in visits), visits
    print('ok regular 5-second respawns use the safe middle and all corners:', visits)
    assert r.select(line(5, 25, 15), [0], [20], 0) == [1]
    points = line(5, 25)
    assert r.select(points, [0, 1, 2], [20], 0) == model(points, [0, 1, 2], [(0, 0, 0)], 0)
    points = [((x, 0, 0), 20) for x in (1, 19, 3, 17, 5, 15, 7, 13, 9, 11)]
    visits = [0] * len(points)
    for event in range(500):
        got = r.select(points, [0], [20], event * 5_000)
        assert got == model(points, [0], [(0, 0, 0)], event * 5_000)
        visits[got[0]] += 1
    assert all(n == 0 if i % 2 == 0 else 60 <= n <= 140 for i, n in enumerate(visits)), visits
    print('ok crowded fallback uses all five best-separated points, excludes nearer ones:', visits)
    r.make_player(21, 4, 11, (5.0, 0, 0))
    assert {r.select(line(0, 500), [0], [21], time)[0] for time in range(0, 500, 50)} == {0, 1}
    print('ok the clear test is of the due player\'s own side (a player of his team: the ally radius, 1 m)')
    assert r.select([], [0], [], 0) == []
    print('ok a map without team-2 points: nobody spawns')

    # The final content uses 20 m: a nearby 19.75 m point is rejected,
    # while a 20.25 m point is eligible even when a much farther one exists.
    r.make_player(20, 30, 10, (0, 0, 0))
    points = [((x, 0, 0), 20) for x in (19.75, 20.25, 100)]
    seen = {r.select(points, [0], [20], time)[0] for time in range(0, 5_000, 50)}
    assert seen == {1, 2}, seen
    print('ok 20 m exclusion: 19.75 m refused, 20.25 m eligible alongside far points')

    rng = random.Random(140)
    grid = lambda: (rng.randrange(-800, 801) / 4, rng.randrange(0, 41) / 4, rng.randrange(-800, 801) / 4)
    for case in range(300):
        points = [(grid(), rng.choice((20.0, 30.0))) for _ in range(rng.randrange(1, 34))]
        living = [grid() for _ in range(rng.randrange(0, 20))]
        due = rng.sample(range(20), rng.randrange(1, 21 - len(living)))
        for i, position in enumerate(living):
            r.make_player(20 + i, 100 + i, 0, position)
        for place in due:
            r.make_player(place, 4 + place, place, grid())
        time = rng.randrange(0, 900_000)
        got = r.select(points, due, list(range(20, 20 + len(living))), time)
        assert got == model(points, due, living, time), (case, got, model(points, due, living, time))
    print('ok 300 random matches (1 to 33 points, up to 19 living, up to 20 due): the points this file\'s model gives')

    for i in range(4):
        r.make_player(i, i % 2, i, (0, 0, 0))
    assert r.select(line(0), [0, 2], [], 0, team=0) == []
    print('ok teams 0 and 1: the game\'s own selection (its group of team 0 empty here: nothing chosen)')

    zone_teams = (0, 1, 2, 3, 4, 7, 13, 23, 0xFFFFFFFF)
    for function in (CAN_USE, USE_HINT):
        yes = 1 if function == CAN_USE else None
        # A team's player: served by his team's zone and team 2's, exactly as the original exe serves him.
        for team in (0, 1):
            for zone_team in zone_teams:
                served = zone_team in (team, 2)
                ours, theirs = r.zone(function, zone_team, team), stock.zone(function, zone_team, team)
                assert ours == theirs, (hex(function), zone_team, team, ours, theirs)
                result, tested, passed = ours
                assert TEAM_TEST[function] in passed and REFUSED[function] not in passed or not served
                if served:
                    assert tested == [(0, profile(0))], (zone_team, team, tested)
                    assert (result == 1) if yes else (result != NOT_HERE), (hex(function), zone_team, team, result)
                    assert ALLOWED[function] in passed
                else:
                    assert tested == [] and result == (0 if yes else NOT_HERE), (zone_team, team, result)
        # A free-for-all's player (4 + his place): no zone, of any team, serves him; the original
        # would have served him at team 2's zone and at one numbered as his side.
        for team in (4, 7, 13, 23):
            for zone_team in zone_teams:
                result, tested, passed = r.zone(function, zone_team, team)
                assert tested == [] and result == (0 if yes else NOT_HERE), (hex(function), zone_team, team, result)
                assert passed == {REFUSED[function]}, (hex(function), zone_team, team, sorted(map(hex, passed)))
            assert stock.zone(function, 2, team)[1] == [(0, profile(0))]
            assert stock.zone(function, team, team)[1] == [(0, profile(0))]
    for at in (0x2F8086, 0x2F80C6, 0x2F809C, 0x2F80E4):
        jump = bytes(patched[hide.file_at(patched, at):][:5])
        assert jump[0] == 0xE9 and at + 5 + struct.unpack("<i", jump[1:])[0] >= base.ARENA_VA, (hex(at), jump.hex())
    print('ok no ammunition refill in a free-for-all: a player of his own side refused at a zone of every team '
          'value by the game\'s own "no", the inventory test never reached; a team\'s player served exactly as '
          'by the original (patch 85\'s sites still hooked)')

    target = lambda n: blackout.srpx((3, struct.pack("<H", n)))
    options = lambda block: blackout.empty_options(block)
    progress = lambda own, top, goal=30: (PROGRESS, (own, top, goal))
    message = lambda text: (MESSAGE, (text, 0x21, 0x14, 6000))

    def frames(name, run, steps):
        for what, state, want in steps:
            run.hud_state(*state)
            back, invoked = run.frame()
            assert back and invoked == want, (name, what, invoked, want)

    for where, read in (("the static info", lambda run, block: run.static_info(options(block))),
                        ("a replay's chunk 1", lambda run, block: run.replay(options(block)))):
        assert read(r, target(30))[0]
        r.movies(HUD, 1, 1)
        frames(where, r, [
            ("the first frame", (1, [3, 5, 7, 2, 0, 0, 99], 1, 0), [progress(5, 7)]),
            ("nothing changed", (1, [3, 5, 7, 2, 0, 0, 99], 1, 0), []),
            ("a kill of the viewed place", (1, [3, 6, 7, 2, 0, 0, 99], 1, 0), [progress(6, 7)]),
            ("a kill of another", (1, [3, 6, 7, 8, 0, 0, 99], 1, 0), [progress(6, 8)]),
            ("another place viewed (a replay, a spectator)", (3, [3, 6, 7, 8, 0, 0, 99], 1, 0), [progress(8, 8)]),
            ("no place viewed", (None, [3, 6, 7, 8, 0, 0, 99], 1, 0), [progress(0, 8)]),
            ("a place past the match's viewed", (6, [3, 6, 7, 8, 0, 0, 99], 1, 0), []),
        ])
        assert r.initialize()
        frames(where, r, [("after initialize_match", (None, [3, 6, 7, 8, 0, 0, 99], 1, 0), [progress(0, 8)]),
                          ("the frame after", (None, [3, 6, 7, 8, 0, 0, 99], 1, 0), [])])
        print(f'ok {where} with tag 3: the hud movie told root.set_ffa_progress(own, top, 30) at each change of '
              'the viewed place\'s kills or the most kills, and again after initialize_match; the kills of a '
              'place past the match\'s four (an earlier match\'s) never counted')
    assert r.static_info(options(blackout.srpx((2, b"\x01"), (3, struct.pack("<H", 300)))))[1] == blackout.MAGIC
    frames("with Blackout", r, [("the first frame", (0, [4], 1, 0), [progress(4, 4, 300)])])
    print('ok tag 3 after patch 65\'s tag 2: both read (the Blackout mark, a target of 300)')

    assert r.static_info(options(target(30)))[0]
    r.refused, r.refusing = (PROGRESS.encode(), MESSAGE.encode()), (hide.HUD_MOVIE,)
    frames("a movie without a root", r, [("refused", (0, [1, 2], 1, 0), [progress(1, 2)]),
                                        ("refused again", (0, [1, 2], 1, 0), [progress(1, 2)])])
    r.refused, r.refusing = (), ()
    frames("a movie without a root", r, [("taken", (0, [1, 2], 1, 0), [progress(1, 2)]),
                                        ("told", (0, [1, 2], 1, 0), [])])
    print('ok a movie that does not take it (no root yet) is told again at the next frame')

    for block, name in ((b"", "no block"), (blackout.srpx((2, b"\x01")), "Blackout alone"),
                        (blackout.srpx((3, b"\x1e")), "tag 3 of one byte"), (target(0), "a target of 0"),
                        (target(30)[:-1], "a cut entry")):
        assert r.static_info(options(block))[0] and r.initialize()
        frames(name, r, [("a frame", (1, [30, 30, 30, 30], 0, 0), []), ("the next", (1, [0, 0, 0, 0], 0, 500), [])])
        assert r.welcome(8) == (True, [], []), name
    print('ok a match without the entry (no block, another tag, another length, a target of 0, a cut one): '
          'nothing told, no welcome of ours')

    won, lost = message("Victory"), message("Defeat")

    # The native installer grows the options in place, without reading
    # static info again; a replay can watch the added place too. The
    # client's other options object deliberately keeps its original count.
    assert r.static_info(options(target(30)))[0]
    r.hud_state(0, [3, 5, 7, 2], 0, 400)
    assert r.frame() == (True, [progress(3, 7)])
    r.emu.mem_write(blackout.OPTIONS + 0x577B5, bytes([5]))
    r.hud_state(4, [3, 5, 7, 2, 30], 0, 399)
    assert r.frame() == (True, [progress(30, 30), won])
    assert r.initialize()
    assert r.frame() == (True, [progress(30, 30)])
    print('ok a late-installed leader and a replay viewing him use the live roster')

    # Whole displayed seconds stay at zero through the last fraction.
    assert r.static_info(options(target(30)))[0]
    r.hud_state(0, [12, 11], 0, 1)
    assert r.frame() == (True, [progress(12, 12)])
    r.hud_state(0, [12, 11], 0, 0, remaining_ms=999)
    assert r.frame() == (True, [])
    r.hud_state(0, [12, 13], 0, 0, remaining_ms=1)
    assert r.frame() == (True, [progress(12, 13)])
    r.hud_state(0, [12, 13], 0, 0, remaining_ms=0)
    assert r.frame() == (True, [lost])
    assert r.frame() == (True, [])
    print('ok the last fractional second can change the winner; verdict only at the exact deadline')

    # Execute the game's real KD event handler, including enemy checks.
    # Both players are at 29; the first 30th kill fixes the serialized
    # records, including that event's victim death, through later events.
    r.emu.mem_write(KD_RULE + 0x1B0, bytes([20]))  # growing-roster constructor, patch 026g
    records = bytearray(160)
    struct.pack_into('<HH', records, 0, 29, 29)
    struct.pack_into('<HH', records, 8, 29, 29)
    r.emu.mem_write(KD_RULE + 0x110, bytes(records))
    r.hud_state(0, [29, 29], 0, 400)
    r.kill(0, 1)
    terminal = bytes(r.emu.mem_read(KD_RULE + 0x110, 160))
    assert struct.unpack_from('<HH', terminal, 0) == (30, 29)
    assert struct.unpack_from('<HH', terminal, 8) == (29, 30)
    for _ in range(3):
        r.kill(1, 0)
    assert bytes(r.emu.mem_read(KD_RULE + 0x110, 160)) == terminal
    # The KD state restored from a world is all the hook consults. No
    # process-global end flag survives a rollback/seek before the kill.
    r.restore_kd(records)
    r.kill(1, 0)
    assert struct.unpack('<HH', r.emu.mem_read(KD_RULE + 0x110, 4)) == (29, 30)
    assert struct.unpack('<HH', r.emu.mem_read(KD_RULE + 0x118, 4)) == (30, 29)
    # Loading the terminal world again keeps its original winner,
    # regardless of the different winner just simulated on this machine.
    r.restore_kd(terminal)
    r.kill(1, 0)
    assert bytes(r.emu.mem_read(KD_RULE + 0x110, 160)) == terminal
    # A fifth player's goal is also terminal: the rule already owns
    # all twenty records, independently of the initial options roster.
    struct.pack_into('<H', records, 4 * 8, 30)
    r.emu.mem_write(KD_RULE + 0x110, bytes(records))
    r.kill(0, 1)
    assert bytes(r.emu.mem_read(KD_RULE + 0x110, 160)) == bytes(records)
    # At the time limit, even sub-target records stop changing. Just
    # before it, a kill still counts despite the HUD displaying zero.
    records = bytes(160)
    r.emu.mem_write(KD_RULE + 0x110, records)
    r.hud_state(0, [], 0, 0, remaining_ms=1)
    r.kill(0, 1)
    terminal = bytes(r.emu.mem_read(KD_RULE + 0x110, 160))
    assert struct.unpack_from('<H', terminal)[0] == 1
    r.hud_state(0, [], 0, 0, remaining_ms=0)
    r.kill(1, 0)
    assert bytes(r.emu.mem_read(KD_RULE + 0x110, 160)) == terminal
    # The same handler in another mode retains native behavior.
    assert r.static_info(options(b''))[0]
    r.kill(1, 0)
    assert struct.unpack('<H', r.emu.mem_read(KD_RULE + 0x118, 2))[0] == 1
    print('ok native KD events freeze the winner and deaths at the goal/deadline, restore across rollback, and pass other modes')
    ends = [
        ("the target reached by the viewed place", [
            ("before the start", (1, [0, 0, 0, 0], 1, 0), [progress(0, 0)]),
            ("in play", (1, [0, 29, 12, 0], 0, 400), [progress(29, 29)]),
            ("the 30th kill", (1, [0, 30, 12, 0], 0, 400), [progress(30, 30), won]),
            ("after it", (1, [0, 30, 12, 0], 0, 399), [])]),
        ("the target reached by another", [
            ("in play", (1, [0, 29, 29, 0], 0, 400), [progress(29, 29)]),
            ("his 30th kill", (1, [0, 29, 30, 0], 0, 400), [progress(29, 30), lost]),
            ("after it", (1, [0, 29, 30, 0], 0, 400), [])]),
        ("the time over, the viewed place with the most kills", [
            ("in play", (2, [3, 9, 9, 1], 0, 1), [progress(9, 9)]),
            ("no second left", (2, [3, 9, 9, 1], 0, 0), [won]),
            ("after it", (2, [3, 9, 9, 1], 0, 0), [])]),
        ("the time over, another with more", [
            ("in play", (0, [3, 9, 9, 1], 0, 1), [progress(3, 9)]),
            ("no second left", (0, [3, 9, 9, 1], 0, 0), [lost])]),
        ("the time over without a kill", [
            ("in play", (0, [0, 0, 0, 0], 0, 1), [progress(0, 0)]),
            ("no second left", (0, [0, 0, 0, 0], 0, 0), [lost])]),
        ("no place viewed at the end", [
            ("in play", (None, [3, 9, 9, 1], 0, 1), [progress(0, 9)]),
            ("no second left", (None, [3, 9, 9, 1], 0, 0), [])]),
        ("a game that comes when the match is over (a hud state not seen in play)", [
            ("its first frame", (1, [0, 30, 12, 0], 0, 0), [progress(30, 30)]),
            ("the next", (1, [0, 30, 12, 0], 0, 0), [])]),
    ]
    for name, steps in ends:
        assert r.static_info(options(target(30)))[0]
        frames(name, r, steps)
    print('ok the end, once: the victory text to the viewed place with the most kills (at least one), the defeat '
          'text to another, at the target or exact deadline; none to a game that never saw the match in play')

    assert r.static_info(options(target(30)))[0]
    frames("a message refused", r, [("in play", (1, [0, 29], 0, 400), [progress(29, 29)])])
    r.refused, r.refusing = (MESSAGE.encode(),), (hide.HUD_MOVIE,)
    frames("a message refused", r, [("refused", (1, [0, 30], 0, 400), [progress(30, 30), won])])
    r.refused, r.refusing = (), ()
    frames("a message refused", r, [("taken", (1, [0, 30], 0, 400), [won]), ("after", (1, [0, 30], 0, 400), [])])
    print('ok an end\'s message the movie does not take is shown at the next frame')

    assert r.static_info(options(target(30)))[0]
    assert r.welcome(8) == (True, [], [message("Free for all. First to 30 kills wins")])
    assert r.replay(options(target(125)))[0]
    assert r.welcome(8) == (True, [], [message("Free for all. First to 125 kills wins")])
    print('ok the welcome of a free-for-all (mode 8): the dictionary\'s text with the target, by '
          'root.set_parameterized_message(text, 21h, 14h, 6000); the game\'s own shows nothing')
    keys = {2: b"st_start_match_welcome_message", 3: b"st_start_match_welcome_message_2",
            4: b"st_start_match_welcome_message_3", 5: b"st_start_match_welcome_message_5"}
    assert r.static_info(options(b""))[0]
    for mode in (2, 3, 4, 5, 6, 8, 0xFF):
        kept, native, ours = r.welcome(mode)
        assert kept and ours == [] and native == ([(keys[mode], HUD, 0x21, 0x14, 6000)] if mode in keys else []), mode
        kept, native, ours = stock.welcome(mode)
        assert kept and ours == [] and native == ([(keys[mode], HUD, 0x21, 0x14, 6000)] if mode in (2, 3, 4) else [])
    print('ok the other modes\' welcome as before (2 to 4 the game\'s, 5 patch 53\'s); the original passes mode 8 by')

    for run in (stock,):
        assert run.static_info(options(target(30)))[0] and run.initialize()
        frames("the original", run, [("a frame", (1, [0, 30, 12, 0], 0, 400), []),
                                     ("the end", (1, [0, 30, 12, 0], 0, 0), [])])
    print('ok the original invokes nothing')


if __name__ == '__main__':
    main()
