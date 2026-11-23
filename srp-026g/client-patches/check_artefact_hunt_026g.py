#!/usr/bin/env python3
"""Run patch 171 (Artifact Hunt, mode 13: artefact_hunt.rs over
crates/artefact-hunt-rule) in the default build on made-up games, worlds and
players (docs/notes/artefact_hunt.md, "The exe").

    python check_artefact_hunt_026g.py <original survarium.exe>

Requires Python `unicorn` (and the research-mode scripts' bcw.py, for a mode
config). check_research_026g.py's harness: the built exe's sections loaded as
they are, its code run as machine code, the game's own code where the patch
calls it (register_cook 0x102D10, game_match_rule_base 0x313900, the config's
look-ups 0xF6B10 / 0xF6C00, the field writers 0x1F3DE0 / 0x1F48A0,
blackboard::set<bool> 0x316630, apply_modifier 0x2D5AE0) and the game's own
callers of every slot (the translate's calls, the core's loops over its
rules, the tick list's runner, rules_on_event). Intercepted: the allocator
(0xE4F90), GetCurrentThreadId, a buffer's resize (0x7AB30), the statistics'
listener (0x2F5D40), and where a hooked function goes on into the game past
the site (query_base_resources 0x1EC176, apply_new_input 0x2D86E6,
can_sprint 0x2DBF4B, and the presentation's sites). The presentation's calls
into the game are answered here and kept: the query (0xE5B60, its callback
then called with made-up results), the scene's model and particle calls
(0x21CD0, 0x21F40, 0x22280, 0x22660, 0x229D0, 0x22B30), the hud sound's emit
and the voice's play and free (0x65630, 0x5ABA0, 0x5B390), the translator
(0x1FF0F0); the clock and log imports are stubs.
The rule's state is read back as the world has it:
its serialize's bytes (artefact_hunt_rule::wire's layout). What each case
checks is printed.
"""
import os
import struct
import sys

from unicorn import UcError, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EBX, UC_X86_REG_EDX, UC_X86_REG_ESP, \
    UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_XMM0

import patch_026g as base
import check_research_026g as research

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "scripts", "research-mode"))
import bcw  # noqa: E402

ARENA = base.arena()
SYMBOLS = {name: va for name, va in ARENA.symbols.items() if name.startswith("artefact_hunt")}
CLASS = 481
STOP, STACK, CORE, CONFIG = research.STOP, research.STACK, research.CORE, research.CONFIG
COOKS = research.COOKS
# where a hooked function goes on into the game after our site's replay
QUERY_ON, INPUT_ON, SPRINT_ON = 0x1EC176, 0x2D86E6, 0x2DBF4B
AFTER_MOVE_ON, DEACTIVATE_ON, ICONS_ON, MARK_ON = 0x1F7C41, 0x1F9525, 0x23AA16, 0x1F2D92
# the game's functions the presentation calls: answered here, each call kept
QUERY_ONE, ADD_MODEL, REMOVE_MODEL, UPDATE_MODEL = 0xE5B60, 0x21CD0, 0x21F40, 0x22280
PLAY_PARTICLE, REMOVE_PARTICLE, EMIT_HUD, PLAY_VOICE, FREE_VOICE = 0x22660, 0x229D0, 0x65630, 0x5ABA0, 0x5B390
TRANSLATE, MOVE_PARTICLE = 0x1FF0F0, 0x22B30
# the core's copy of the time state: 3 in play, 4 a round about to start (vostok::round_countdown)
TIME_STATE = 0x18FE4
IN_SCENE = 0x10D                                 # a model instance's in-scene byte
CLEANUPS = {ADD_MODEL: 16, REMOVE_MODEL: 8, UPDATE_MODEL: 16, PLAY_PARTICLE: 28, REMOVE_PARTICLE: 12,
            MOVE_PARTICLE: 20, PLAY_VOICE: 12, FREE_VOICE: 0}
# the rule object (fields_026g.tsv's survarium::hunt_rule)
R_BLACKBOARD, R_TICK, R_CORE, R_SIZE = 0x118, 0x120, 0x138, 0x140
BLACKBOARD = 0x188B0
EVENTS_AT, LISTENER = research.EVENTS_AT, research.LISTENER
TIME = 0x18FC0
START = 20000
TEAM0, TEAM1 = (0, 1, 2, 3), (10, 11, 12, 13)
LINES = ("art_spawned", "art_is_our", "enemy_got_art", "we_lost_art", "enemy_loose_art", "art_on_our_base",
         "art_on_enemy_base")
SPAWNERS = [(10.0, 0.0, 0.0), (40.0, 0.0, 0.0), (60.0, 0.0, 0.0), (90.0, 0.0, 0.0)]


def config_bytes():
    """A mode config as the pack makes them ("Data"): 0.46c2's parameters,
    four spawners on the line between two bases 100 m apart."""
    def crate(x):
        return {"type": 1, "position": (x, 0.5, 0.0), "axis_x": (1.0, 0.0, 0.0), "axis_y": (0.0, 1.0, 0.0),
                "axis_z": (0.0, 0.0, 1.0), "half": (0.5, 0.5, 0.5)}
    return bcw.dump({"mode": {
        "parameters": {"match_time": 900, "wait_time": 5, "countdown_time": 20, "wait_player_percent": 0.6,
                       "respawn_time": 15, "time_needed_to_continue_match": 90, "draw_additional_time": 180,
                       "time_before_new_round": 3, "round_countdown_time": 5, "score": 5,
                       "time_needed_to_respawn": 7, "vi_spawn_time": 60, "vi_spawn_variation": 10,
                       "vi_not_picked_ttl": 120, "vi_dropped_ttl": 60},
        "artefact_hunt": {
            "spawners": [{"position": p, "rotation": (0.0, 0.0, 0.0)} for p in SPAWNERS],
            "bases": [{"team": 0, "position": (0.0, 0.0, 0.0), "shapes": [crate(0.0)]},
                      {"team": 1, "position": (100.0, 0.0, 0.0), "shapes": [crate(100.0)]}],
            "item": {"model": "item/artefacts/artefact_shining_forest", "particle_idle": "a",
                     "particle_spawn": "b", "particle_destroy": "c", "carrier_particle": "g",
                     "carrier_particle_fp": "gf",
                     "movement_speed_modifier": -0.2, "health_regeneration_speed_modifier": 0.05,
                     "usable_radius": 0.3, "drop_lift": 0.13}},
        "sound_notifications": dict([("base_path", "vo")] + [(key, [258, f"artifact_hunt/{line}"]) for key, line in zip(
            ("artefact_spawned", "artefact_picked_up", "artefact_picked_up_by_enemy", "artefact_dropped",
             "artefact_dropped_by_enemy", "artefact_brought", "artefact_brought_by_enemy"), LINES)]),
        "rules": [93, 91, CLASS, 94], "mode": 13}})


class Run(research.Run):
    def __init__(self, data):
        super().__init__(data)
        # the match core is a pvp_match's (+1F8h): its page below CORE
        self.emu.mem_map(CORE - 0x1000, 0x1000)
        # the frame steps' other handlers (ragdoll's 36 KiB frame) below the span's stack
        self.emu.mem_map(STACK - 0x40000, 0x40000)
        # QueryPerformanceCounter and -Frequency (the breadcrumbs' clock): 1 to the out-pointer
        clock = self.allocate(0x40)
        for n, iat in enumerate((0x870158, 0x870150)):
            self.word(iat, clock + 0x20 * n)
            self.emu.mem_write(clock + 0x20 * n, bytes.fromhex("8b442404c70001000000c7400400000000b801000000c20400"))
        # the patches' log (another handler of a chain may write a line): GetCurrentProcessId,
        # CreateFileA, WriteFile, CloseHandle; GetEnvironmentVariableA (the handlers' SKHID_HUNT_LOG): unset
        files = self.allocate(0x50)
        for n, (iat, code) in enumerate(((0x8701B8, "b82a000000c3"), (0x870048, "b844000000c21c00"),
                                         (0x870108, "b801000000c21400"), (0x870040, "b801000000c20400"),
                                         (0x8701B4, "31c0c20c00"))):
            self.word(iat, files + 0x10 * n)
            self.emu.mem_write(files + 0x10 * n, bytes.fromhex(code))
        for address in (QUERY_ON, INPUT_ON, SPRINT_ON, AFTER_MOVE_ON, DEACTIVATE_ON, ICONS_ON,
                        MARK_ON):
            self.emu.hook_add(UC_HOOK_CODE, self.past, begin=address, end=address)
        self.drawn, self.queries, self.voices = [], [], []
        self.proxy = None
        for address in (QUERY_ONE, ADD_MODEL, REMOVE_MODEL, UPDATE_MODEL, PLAY_PARTICLE, REMOVE_PARTICLE, EMIT_HUD,
                        PLAY_VOICE, FREE_VOICE, TRANSLATE, MOVE_PARTICLE):
            self.emu.hook_add(UC_HOOK_CODE, self.present, begin=address, end=address)

    def present(self, emu, address, size, user):
        sp = emu.reg_read(UC_X86_REG_ESP)
        arg = lambda n: self.read(sp + 4 * n)
        if address == QUERY_ONE:
            requests, count = emu.reg_read(UC_X86_REG_EAX), emu.reg_read(UC_X86_REG_ECX)
            self.queries.append([(self.string(self.read(requests + 8 * i)), self.read(requests + 8 * i + 4))
                                 for i in range(count)])
            self.ret(0, 0)
        elif address in (ADD_MODEL, UPDATE_MODEL):
            matrix = arg(3)
            at = struct.unpack("<3f", emu.mem_read(matrix + 48, 12))
            reference = emu.reg_read(UC_X86_REG_EAX)
            emu.mem_write(self.read(reference) + IN_SCENE, b"\x01")
            self.drawn.append(("model+", arg(1), reference, arg(2), at))
            self.ret(0, CLEANUPS[address])
        elif address == REMOVE_MODEL:
            reference = emu.reg_read(UC_X86_REG_EAX)
            emu.mem_write(self.read(reference) + IN_SCENE, b"\x00")
            self.drawn.append(("model-", arg(1), reference, arg(2)))
            self.ret(0, CLEANUPS[address])
        elif address == PLAY_PARTICLE:
            at = struct.unpack("<3f", emu.mem_read(arg(4) + 48, 12))
            self.drawn.append(("particle+", arg(1), arg(2), arg(3), at))
            self.ret(0, CLEANUPS[address])
        elif address == REMOVE_PARTICLE:
            self.drawn.append(("particle-", arg(1), arg(2), self.read(arg(3))))
            self.ret(0, CLEANUPS[address])
        elif address == EMIT_HUD:
            proxy = self.proxy = self.allocate(0x40)
            self.word(proxy + 0x28, 1)                          # its one reference (the Voice's)
            out = emu.reg_read(UC_X86_REG_EDI)
            self.word(out, proxy)
            self.voices.append(("emit", emu.reg_read(UC_X86_REG_ESI), self.read(emu.reg_read(UC_X86_REG_EAX)),
                                emu.reg_read(UC_X86_REG_ECX)))
            self.ret(out, 0)
        elif address == PLAY_VOICE:
            self.voices.append(("play", emu.reg_read(UC_X86_REG_ECX), arg(1)))
            self.ret(0, CLEANUPS[address])
        elif address == FREE_VOICE:
            self.voices.append(("free", emu.reg_read(UC_X86_REG_ECX)))
            self.ret(0, 0)
        elif address == TRANSLATE:
            emu.mem_write(arg(2), self.string(arg(1)).encode() + b"\0")
            self.ret(0, 8)
        elif address == MOVE_PARTICLE:
            at = struct.unpack("<3f", emu.mem_read(arg(4) + 48, 12))
            self.drawn.append(("particle~", arg(1), arg(2), self.read(arg(3)), at))
            self.ret(0, CLEANUPS[address])

    def intercept(self, emu, address, size, user):
        # the constructor's call goes on into our hook at 0x1EC170, the icons' update into our hud's
        if address not in (research.CTOR_CALL, research.PLAYER_ICONS):
            super().intercept(emu, address, size, user)

    def past(self, emu, address, size, user):
        self.stopped = ("past", address)
        emu.emu_stop()


def state(run, rule):
    """The rule's state as its serialize writes it (slot 15, the game's
    writers): the bytes."""
    writer, buffer = run.allocate(0x20), run.allocate(0x10)
    run.word(writer + 0x14, buffer)
    run.word(buffer + 4, run.allocate(0x200))
    run.word(buffer + 8, 0x200)
    run.word(buffer + 0xC, 0)
    _, cleanup = run.thiscall(research.vtable(run, rule, 15), rule, writer)
    assert cleanup == 4, cleanup
    return bytes(run.emu.mem_read(run.read(buffer + 4), run.read(buffer + 0xC)))


def decoded(blob):
    """The fields a case looks at (artefact_hunt_rule's wire table)."""
    out = {"score": (blob[4], blob[5]), "round_starts": struct.unpack_from("<I", blob, 6)[0], "stage": blob[18]}
    if blob[18] == 1:
        out["start"] = struct.unpack_from("<I", blob, 19)[0]
        out["round"] = blob[23]
        if blob[23] in (2, 3, 4, 5, 7, 8):
            out["clock"] = blob[24]
        if blob[23] == 3:
            out["spawner"] = blob[25]
        if blob[23] == 4:
            out["carrier"] = blob[25]
        if blob[23] == 6:
            out["until"] = struct.unpack_from("<I", blob, 24)[0]
    assert blob[-1] == len(blob) - 1, "m_size counts the bytes before it"
    return out


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    run = Run(bytes(built))
    results = []

    def check(what, ok, detail=""):
        results.append(ok)
        print(f"  {'ok' if ok else 'FAIL'}  {what}" + (f": {detail}" if detail else ""))

    # 1. The game's constructor (through Research's registration, which
    # tail-jumps to query_base_resources) registers the cook at 481.
    table = run.allocate(0x206 * 4)
    run.word(COOKS, table)
    run.word(COOKS + 4, table + 0x206 * 4)
    game = run.allocate(0x100)
    run.call(0x1E9B4C, regs={UC_X86_REG_EBX: game}, until=STOP)
    cook = run.read(table + 4 * CLASS)
    check("the constructor registers the cook at class 481 after Research's, and query_base_resources goes on",
          run.stopped == ("past", QUERY_ON) and cook != 0 and run.read(cook) == SYMBOLS["artefact_hunt_cook_vtable"] + 4
          and run.read(cook + 8) == CLASS and run.read(cook + 0x10) == 0xFFFFFFFF and run.read(cook + 0x14) == 0x77
          and run.read(cook + 0x18) == 8 and run.read(cook + 0x20) == game
          and run.read(table + 4 * base.RESEARCH_CLASS) != 0, f"stopped {run.stopped}, cook {cook:#x}")
    run.word(table + 4 * CLASS, 0)
    run.call(0x1E9B4C, regs={UC_X86_REG_EBX: game}, until=STOP)
    check("a second construction registers nothing", run.read(table + 4 * CLASS) == 0)
    run.word(table + 4 * CLASS, cook)
    taken = Run(bytes(built))
    other = taken.allocate(0x206 * 4)
    taken.word(COOKS, other)
    taken.word(COOKS + 4, other + 0x206 * 4)
    taken.word(other + 4 * CLASS, 0x12345678)
    taken.call(0x1E9B4C, regs={UC_X86_REG_EBX: game}, until=STOP)
    check("a taken class is left to its cook", taken.read(other + 4 * CLASS) == 0x12345678
          and taken.stopped == ("past", QUERY_ON))

    # 2. The cook makes the rule: through the rule cooks' translate, its
    # create and size (0x2F42B8..0x2F42CF).
    run.emu.mem_write(CONFIG, research.loaded(config_bytes(), CONFIG))
    run.word(0x1F1D9C4, run.allocate(0x100))                # the game's allocator (set before any cook)
    query = run.allocate(0x10)
    options = run.allocate(0x60000)
    run.word(options + 0x577B0, 7)                          # the match's seed
    run.word(query, options)
    run.word(query + 0xC, CONFIG)
    run.emu.reg_write(UC_X86_REG_ESI, 0)
    eip, moved = run.span(0x2F42B8, 0x2F42CF, {UC_X86_REG_EBX: cook, UC_X86_REG_EDI: query})
    rule, size = run.emu.reg_read(UC_X86_REG_ESI), run.emu.reg_read(UC_X86_REG_EAX)
    check("driven: the cook's create and size through translate's calls: a rule of type 6, our vtable, tdm's resolver",
          eip == 0x2F42CF and moved == 0 and size == R_SIZE and rule != 0
          and run.read(rule) == SYMBOLS["artefact_hunt_rule_vtable"] + 4 and run.read(rule + 0x108) == 0xA06D10
          and run.read(rule + 0x10C) == 6 and run.read(rule + R_CORE) == 0 and run.read(rule + 0x13C) == game,
          f"esp moved {moved}, size {size:#x}, rule {rule:#x}")
    blob = state(run, rule)
    check("a new rule is idle: 100 bytes, its rng the match's seed", len(blob) == 100 and blob[18] == 0
          and struct.unpack_from("<I", blob, 0)[0] == 7, blob.hex())

    # 3. The world: slot 11 through the core's loop links the tick at the
    # list's end in its order; slot 12 (tdm's, kept) the blackboard.
    players, statistics = research.world(run)
    for i, player in enumerate(players):
        run.word(run.read(player + 0x6B8) + 0x45EC, 0 if i in TEAM0 else 1 if i in TEAM1 else 0)
        run.emu.mem_write(player + 0x114, bytes([i, 0]))
        run.word(player + 0x120, CORE)                       # its world core
    # the match being played: the engine world's game's game world's match
    engine, client_game = run.allocate(0x400), run.allocate(0x6000)
    run.word(0x1F1D310, engine)
    run.word(engine + 0x2D0, client_game)
    run.word(client_game + 0x18 + 0x4D4C, CORE - 0x1F8)
    run.word(statistics + LISTENER, 1)
    # the match's places only are in the world's active places
    run.word(CORE + research.ACTIVE_PLACES, sum(1 << p for p in TEAM0 + TEAM1))
    run.emu.mem_write(CORE + BLACKBOARD + 8, b"\x01")
    run.emu.mem_write(CORE + BLACKBOARD + 0x38, b"\x01")
    time_rule = run.allocate(0x200)
    # the time rule only answers the loops over the rules: its 7 pops 0Ch, its 8 and 9 pop 8
    stubs, methods = run.allocate(0x10), run.allocate(0x50)
    run.emu.mem_write(stubs, b"\xC2\x0C\x00\xC2\x08\x00")
    for slot in range(20):
        run.word(methods + 4 * slot, stubs + (3 if slot in (8, 9) else 0))
    run.word(time_rule, methods)
    run.word(time_rule + 0x10C, 2)
    run.word(time_rule + 0x12C, START)
    first = run.allocate(0x18)
    run.word(first, time_rule)                              # the time rule's delegate: ret 0Ch
    run.word(first + 4, stubs)
    run.emu.mem_write(first + 0x14, b"\x01")
    run.word(CORE + research.TICK_LIST, 1)
    run.word(CORE + research.TICK_LIST + 4, first)
    run.word(CORE + research.TICK_LIST + 8, first)
    rules = run.allocate(8)
    run.word(rules, time_rule)
    run.word(rules + 4, rule)
    run.word(CORE + research.RULES, rules)
    run.word(CORE + research.RULES + 4, rules + 8)
    # the core's loop gives each rule the core (the time rule's slot 11 is
    # not run here: only ours is in the loop's span)
    run.word(rules, rule)
    run.word(CORE + research.RULES + 4, rules + 4)
    eip, moved = run.span(0x2D3B2B, 0x2D3B51, {UC_X86_REG_EBP: CORE})
    tick = rule + R_TICK
    check("driven: slot 11 through the world's loop: the core kept, the tick after the time rule's, in order",
          eip == 0x2D3B51 and moved == 0 and run.read(rule + R_CORE) == CORE
          and run.read(CORE + research.TICK_LIST) == 2 and run.read(CORE + research.TICK_LIST + 8) == tick
          and run.read(first + 0xC) == tick and run.read(tick + 8) == first and run.read(tick) == rule
          and run.read(tick + 4) == SYMBOLS["artefact_hunt_on_tick"] and run.read(tick + 0x10) == 7
          and run.read(tick + 0x14, 1) == 1, f"esp moved {moved}")
    eip, moved = run.span(0x2D58F0, 0x2D5917, {UC_X86_REG_EDI: CORE})
    check("driven: slot 12 (tdm's own) through the world's loop: the blackboard at +118h",
          eip == 0x2D5917 and moved == 0 and run.read(rule + R_BLACKBOARD) == CORE + BLACKBOARD)
    run.word(rules, time_rule)
    run.word(rules + 4, rule)
    run.word(CORE + research.RULES + 4, rules + 8)

    # The hud state's points from the match's first frame (before its start): a last match's 7:3 is
    # written over with the hunt's 0:0 (the owner, 2026-11-22: "make sure arthunt also resets it").
    first_hud = run.allocate(0x100)
    run.emu.mem_write(first_hud + 0x90, struct.pack("<2H", 7, 3))
    run.call(0x2D36B0, args=(CORE, first_hud, 0), until=0x2D36E2)
    check("driven: slot 17 through fill_match_hud_state's loop before the match's start: 0 / 0 over a "
          "last match's 7 / 3", run.read(first_hud + 0x90, 2) == 0 and run.read(first_hud + 0x92, 2) == 0,
          f"{run.read(first_hud + 0x90, 2)} / {run.read(first_hud + 0x92, 2)}")

    # 4. Who is in: slot 8 through the world's loop. As in the game (the
    # 2026-11-21 test: nobody was ever in the match), team 1's players have
    # no team yet when slot 8 comes: the tick takes them in.
    ok = True
    profiles = {place: run.read(players[place] + 0x6B8) + 0x45EC for place in TEAM1}
    for place in TEAM1:
        run.word(profiles[place], 7)
    for place in TEAM0 + TEAM1:
        eip, moved = run.span(0x2D1FFF, 0x2D202A, {UC_X86_REG_EDI: CORE}, [0] * 8 + [place, 0])
        ok = ok and eip == 0x2D202A and moved == 0
    for place in TEAM1:
        run.word(profiles[place], 1)
    blob = state(run, rule)
    sides = struct.unpack_from("<II", blob, 10)
    check("driven: slot 8 through the world's loop: each place of its profile's team; one without a team "
          "of the two not yet", ok and sides == (0x0F, 0), f"sides {sides[0]:#x} {sides[1]:#x}")

    # 5. The start: rules_on_event(0) as the time rule's countdown end sends
    # it; the match time from the time rule's +12Ch.
    run.word(CORE + TIME, START)
    run.call(0x2D1EA0, args=(0, 0), regs={UC_X86_REG_EDI: CORE})
    blob = state(run, rule)
    got = decoded(blob)
    check("driven: slot 7 event 0 through rules_on_event: play from the time rule's start, the artefact waited for",
          got.get("start") == START and got.get("round") == 2 and run.emu.reg_read(UC_X86_REG_ESP) == STACK + 0x8000,
          str(got))
    check("the statistics' tail: 0 / 0, a draw", run.read(statistics + 0x31D4) == 0
          and run.read(statistics + 0x31E4, 1) == 2)

    # 6. Ticks through the tick list's runner until the artefact lies.
    now = START
    now += 50
    run.word(CORE + TIME, now)
    run.call(0x2D1D20, args=(CORE + research.TICK_LIST, 50, now, 1))
    sides = struct.unpack_from("<II", state(run, rule), 10)
    check("the first tick takes in the players of the world's places slot 8 left out (their team known now)",
          sides == (0x0F, 0x3C00), f"sides {sides[0]:#x} {sides[1]:#x}")
    while decoded(state(run, rule)).get("round") == 2 and now < START + 80000:
        now += 50
        run.word(CORE + TIME, now)
        run.call(0x2D1D20, args=(CORE + research.TICK_LIST, 50, now, 1))
    got = decoded(state(run, rule))
    check("driven: the tick through the list's runner: the artefact at a spawner 50 to 70 s after the start",
          got.get("round") == 3 and 50000 <= now - START < 70000 and got.get("spawner", 9) < 4,
          f"{got} at {now - START} ms")
    check("the blackboard's flags are left true outside sudden death",
          run.read(CORE + BLACKBOARD + 8, 1) == 1 and run.read(CORE + BLACKBOARD + 0x38, 1) == 1)
    at = SPAWNERS[got.get("spawner", 0)]

    # 7. The use key through apply_new_input's entry (patch 78's carry.input):
    # place 10 looks at it from 1.5 m; the take is the hunt's.
    taker = players[10]
    run.word(taker + 0x10C, 0)                              # no inventory
    run.word(taker + 0x350, 0)                              # no physics world: no level ray

    def look(player, eye, forward):
        matrix = [1.0, 0, 0, 0, 0, 1.0, 0, 0, *forward, 0, *eye, 1.0]
        run.emu.mem_write(player + 0x44C, struct.pack("<16f", *matrix))
        run.emu.mem_write(player + 0x43C, struct.pack("<3f", eye[0], eye[1] - 1.5, eye[2]))

    def press(player, actions, time):
        packet = run.allocate(8)
        run.word(packet, 0)
        run.word(packet + 4, actions)
        run.call(0x2D86E0, args=(time, packet), regs={UC_X86_REG_ECX: player}, until=STOP)
        return run.stopped, run.emu.reg_read(UC_X86_REG_ESP)

    look(taker, (at[0] - 1.5, 1.5, 0.0), (1.0, -1.37 / 1.5, 0.0))
    look(taker, (at[0] - 1.5, 1.5, 0.0), (0.738, -0.674, 0.0))
    stopped, _ = press(taker, 1 << 21, now)
    got = decoded(state(run, rule))
    check("the use key's press on the lying artefact within 2 m: place 10 carries it; the input goes on into "
          "the game (patch 78's site)", got.get("round") == 4 and got.get("carrier") == 10
          and stopped == ("past", INPUT_ON), f"{got}, {stopped}")

    # 8. The carrier: no sprint (can_sprint's refusal), his speed and
    # regeneration terms through the game's own apply_modifier.
    run.call(0x2DBF40, args=(taker,), until=STOP)
    refused = run.emu.reg_read(UC_X86_REG_EAX) & 0xFF == 0 and run.stopped is None
    run.call(0x2DBF40, args=(players[11],), until=STOP)
    check("can_sprint refuses the carrier (its own refusal: xor eax / pop ebx / ret 4); another goes on",
          refused and run.stopped == ("past", SPRINT_ON))

    def modifier(player, index):
        profile = run.read(player + 0x6B8)
        container = profile + 0xF4                              # kit 0
        run.call(0x2D5AE0, args=(struct.unpack("<I", struct.pack("<f", 10.0))[0],
                                 struct.unpack("<I", struct.pack("<f", 1.0))[0]),
                 regs={UC_X86_REG_EAX: container, UC_X86_REG_ESI: index}, until=STOP)
        xmm = run.emu.reg_read(UC_X86_REG_XMM0) & 0xFFFFFFFF
        return struct.unpack("<f", struct.pack("<I", xmm))[0]
    try:
        speed, other, regen = modifier(taker, 4), modifier(players[11], 4), modifier(taker, 2)
        check("apply_modifier: the carrier's movement 10 x (1 - 0.2), regeneration 10 x 1.05; another's 10",
              abs(speed - 8.0) < 1e-4 and abs(regen - 10.5) < 1e-4 and abs(other - 10.0) < 1e-4,
              f"{speed}, {regen}, {other}")
    except UcError as error:
        check("apply_modifier runs over the made-up kit", False, str(error))

    # 9. The delivery: place 10 at team 1's crate (x 100), the use key let go
    # and pressed again.
    look(taker, (98.8, 1.5, 0.0), (0.8, -0.6, 0.0))
    press(taker, 0, now)
    stopped, _ = press(taker, 1 << 21, now)
    blob = state(run, rule)
    got = decoded(blob)
    put = run.read(statistics + EVENTS_AT + 0x1F8 * 10 + 20 * 6, 2)
    check("the use key at his own team's base: 0 / 1, the round's 3 s; his put (event 20) counted, the "
          "scoreboard told", got.get("score") == (0, 1) and got.get("round") == 6 and got.get("until") == now + 3000
          and put == 1 and (statistics + LISTENER, 10, 20, 1) in run.notified, f"{got}, put {put}")
    check("the statistics' tail: team 1 leads", run.read(statistics + 0x31D6, 2) == 1
          and run.read(statistics + 0x31E4, 1) == 1)

    # 10. The respawn rule's tick in the 3 s holds a place due now; at the
    # fade's end everyone of the match is due.
    respawn = run.allocate(0x200)
    run.word(respawn + 0x134, CORE)
    run.word(respawn + 0x114, 15000)
    run.emu.mem_write(respawn + 0x138, b"\xff" * 80)

    def respawn_tick(time):
        frame = STACK + 0x7000
        run.word(frame + 8, 50)
        run.word(frame + 0xC, time)
        return run.span(0x316826, 0x31682B, {UC_X86_REG_ECX: respawn, UC_X86_REG_EBP: frame})

    def times():
        return [run.read(respawn + 0x138 + 4 * p) for p in range(20)]
    run.word(respawn + 0x138 + 4 * 2, now + 1000)
    eip, _ = respawn_tick(now + 1000)
    check("in a round's 3 s the respawn rule's tick: a place due now is due no more (-1)",
          eip == 0x31682B and times()[2] == 0xFFFFFFFF)
    fade_end = None
    while fade_end is None and now < START + 200000:
        now += 50
        run.word(CORE + TIME, now)
        respawn_tick(now)
        if times()[0] == now:
            fade_end = now
        run.call(0x2D1D20, args=(CORE + research.TICK_LIST, 50, now, 1))
    t = times()
    got = decoded(state(run, rule))
    check("at the fade's end (3 s + 1 s) every place of the match is due at that step, alive or dead; the "
          "countdown begins", fade_end is not None and got.get("round") == 8
          and all(t[p] == fade_end for p in TEAM0 + TEAM1) and t[5] == 0xFFFFFFFF,
          f"{got}, fade end {fade_end}")

    # 11. The countdown is the match start's (vostok::round_countdown): the
    # core's time state 4, which the client time rule's camera test reads as
    # the countdown's 2 (the warmup camera, patch 31's kits); the warmup holds
    # the input, which the hunt no longer drops itself.
    def camera_state(core):
        time_rule = run.allocate(0x200)
        run.word(time_rule + 0x128, core)
        run.word(time_rule + 0x148, 3)
        eip, _ = run.span(0x267AE6, 0x267AEC, {UC_X86_REG_EDI: time_rule}, [0] * 8)
        return eip, run.emu.reg_read(UC_X86_REG_EDI)
    check("in the countdown the core's time state is 4 and the client's camera test sees the countdown (2)",
          run.read(CORE + TIME_STATE) == 4 and camera_state(CORE) == (0x267AEC, 2),
          f"{run.read(CORE + TIME_STATE)}, {camera_state(CORE)}")
    stopped, _ = press(players[0], 1, now)
    check("in the countdown an input goes on into the game (the warmup holds it, the hunt does not)",
          stopped == ("past", INPUT_ON), str(stopped))
    while decoded(state(run, rule)).get("round") == 8:
        now += 50
        run.word(CORE + TIME, now)
        run.call(0x2D1D20, args=(CORE + research.TICK_LIST, 50, now, 1))
    got = decoded(state(run, rule))
    check("after the 5 s the next round waits for its artefact; one round start counted (the fade's end)",
          got.get("round") == 2 and now - fade_end == 5000 and got.get("round_starts") == 1,
          str(got))
    stopped, _ = press(players[0], 1, now)
    check("then an input goes on into the game; the core's time state is 3 again and the camera test the time "
          "rule's own", stopped == ("past", INPUT_ON) and run.read(CORE + TIME_STATE) == 3
          and camera_state(CORE) == (0x267AEC, 3))

    # 12. A death's delay: the wave after death + 7 s, from the match's start.
    def delay(at_time, native):
        sp = STACK + 0x6000
        run.word(sp + 0x20, at_time)
        run.emu.reg_write(UC_X86_REG_ESP, sp)
        for reg, value in ((UC_X86_REG_EAX, native), (UC_X86_REG_EDI, respawn), (UC_X86_REG_EBX, players[1])):
            run.emu.reg_write(reg, value)
        run.emu.emu_start(0x316B6B, 0x316B71, count=5_000_000)
        return run.emu.reg_read(UC_X86_REG_EAX)
    death = now
    wave = START + ((death + 7000 - START + 14999) // 15000) * 15000
    got_delay = delay(death, 15000)
    shifted = delay(death, 12000)
    check("a death's delay is the wave's: the first 15 s from the start at or after death + 7 s; a -20 % "
          "modifier 3 s sooner, never under the 7 s", got_delay == wave - death
          and shifted == max(wave - 3000, death + 7000) - death, f"{got_delay}, {shifted}, wave {wave - death}")

    # 13. Slot 17 through the hud's loop: the points, the clock's seconds.
    hud = run.allocate(0x100)
    run.call(0x2D36B0, args=(CORE, hud, now), until=0x2D36E2)
    seconds = (900000 - (now - START)) // 1000
    check("driven: slot 17 through fill_match_hud_state's loop: 0 / 1 and the match's seconds left",
          run.read(hud + 0x90, 2) == 0 and run.read(hud + 0x92, 2) == 1 and run.read(hud + 4) == seconds,
          f"{run.read(hud + 4)} against {seconds}")

    # 14. Slot 16: a world's bytes read back, the reader past them; a
    # late joiner's rule computes the same.
    blob = state(run, rule)
    source = run.allocate(0x200)
    run.emu.mem_write(source, blob + b"\xAA" * 8)
    reader = run.allocate(0x10)
    run.word(reader, source)
    run.word(reader + 4, source)
    run.word(reader + 8, len(blob) + 8)
    _, cleanup = run.thiscall(research.vtable(run, rule, 16), rule, reader, 0x5A5A5A5A)
    check("deserialize(reader, dword) reads exactly its bytes and pops both (ret 8)",
          cleanup == 8 and run.read(reader + 4) == source + len(blob) and state(run, rule) == blob)
    check("slot 18, completed: not before the end", run.thiscall(research.vtable(run, rule, 18), rule, now)[0]
          & 0xFF == 0)

    # 14b. Slots 15, 16 and 18 through the game's own loops over the world's
    # rules (game_world_core's serialize 0x2D2661..0x2D2688, deserialize
    # 0x2D2B1A..0x2D2B5F, rule_completed's 0x2D271D..), ours the one rule.
    run.word(rules, rule)
    run.word(CORE + research.RULES + 4, rules + 4)
    snapshot = research.FAKE
    loop_writer, loop_buffer = snapshot + 0xB7EF0, run.allocate(0x10)
    run.emu.mem_write(loop_writer, bytes(0x20))
    run.word(loop_writer + 0x14, loop_buffer)
    run.word(loop_buffer + 4, run.allocate(0x200))
    run.word(loop_buffer + 8, 0x200)
    run.word(loop_buffer + 0xC, 0)
    eip, moved = run.span(0x2D2661, 0x2D2688, {UC_X86_REG_EBP: CORE, UC_X86_REG_ESI: snapshot})
    looped = bytes(run.emu.mem_read(run.read(loop_buffer + 4), run.read(loop_buffer + 0xC)))
    check("driven: slot 15, serialize through the snapshot's rules loop 0x2D2661: esp as before, the rule's bytes",
          eip == 0x2D2688 and moved == 0 and looped == blob, f"esp moved {moved}, {len(looped)} bytes")
    source = run.allocate(0x200)
    run.emu.mem_write(source, blob)
    run.word(snapshot + 0xB7EE4, source)
    run.word(snapshot + 0xB7EEC, source + len(blob))
    eip, moved = run.span(0x2D2B1A, 0x2D2B5F, {UC_X86_REG_EBP: CORE, UC_X86_REG_EBX: snapshot},
                          [0] * 0x21 + [0x7E57])           # [esp+84h]: the dword the loop passes on
    check("driven: slot 16, deserialize through the snapshot's rules loop 0x2D2B42: esp as before, the cursor "
          "past its bytes, the same state", eip == 0x2D2B5F and moved == 0
          and run.read(STACK + 0x6000 + 0x58) == source + len(blob) and state(run, rule) == blob,
          f"esp moved {moved}, cursor {run.read(STACK + 0x6000 + 0x58):#x} of {source:#x}")
    run.word(CORE + 0x18FC0, now)
    eip, moved = run.span(0x2D271D, 0x2D274B, {UC_X86_REG_EBP: CORE, UC_X86_REG_ESI: run.allocate(0xC0C00)})
    check("driven: slot 18, completed through the world's loop 0x2D2730: not before the end, the loop goes on",
          eip == 0x2D274B and moved == 0, f"{eip:#x}, esp moved {moved}")
    run.word(rules, time_rule)
    run.word(rules + 4, rule)
    run.word(CORE + research.RULES + 4, rules + 8)

    # 15. The artefact in the world. The match's game world (the game's +18h),
    # its renderer's scene renderer and its scene; the pack's assets asked by
    # a query of the patch's own (model 15, particles 61) at the first frame,
    # drawn at the next frames from the rule's state.
    game_world = client_game + 0x18
    run.word(game_world + 0x9C, client_game)
    renderer, scene_renderer = run.allocate(0x200100), run.allocate(0x100)
    run.word(client_game + 0x5360, renderer)
    run.word(renderer + 0x200064, scene_renderer)
    sound_world, user, sound_scene = run.allocate(0x200), run.allocate(0x40), run.allocate(0x40)
    run.word(client_game + 0x5350, sound_world)
    run.word(sound_world + 0xB0, user)
    run.word(game_world + 0x94, sound_scene)

    def frame():
        run.drawn = []
        run.span(0x1F7C3C, AFTER_MOVE_ON, {UC_X86_REG_EBX: game_world}, [0] * 8)
        return run.drawn

    while decoded(state(run, rule)).get("round") == 2:
        now += 50
        run.word(CORE + TIME, now)
        run.call(0x2D1D20, args=(CORE + research.TICK_LIST, 50, now, 1))
    run.queries = []
    frame()
    item = [("item/artefacts/artefact_shining_forest", 15), ("a", 61), ("b", 61), ("c", 61), ("g", 61), ("gf", 61)]
    check("the first frame of a hunt asks the item's model, its idle, spawn and destroy particles and the carrier's "
          "glow (on his body, at his own camera) in one query", run.queries == [item], str(run.queries))
    # the query's callback (the game's: cdecl (queries_result, its word)) with
    # the six results 2E0h apart at +12Ch: a model and five particle systems
    answer = run.allocate(0x12C + 6 * 0x2E0)
    run.word(answer + 0x38, 6)
    model, particles = run.allocate(0x200), [run.allocate(0x100) for _ in range(5)]
    instance = run.allocate(0x200)
    run.word(model + 0x108, instance)
    for i, resource in enumerate([model] + particles):
        run.word(answer + 0x12C + 0x2E0 * i, resource)
    run.call(SYMBOLS["artefact_hunt_on_loaded"], args=(answer, 3))
    check("the assets' callback keeps one reference of each (the model's and every particle's count +1)",
          run.read(model + 0xD0) == 1 and all(run.read(p + 0xD0) == 1 for p in particles))
    drawn = frame()
    at = SPAWNERS[decoded(state(run, rule)).get("spawner", 0)]
    scene = game_world + 4
    added = [d for d in drawn if d[0] == "model+"]
    played = [d for d in drawn if d[0] == "particle+"]
    check("lying at a spawner: the model added to the world's scene at its point, the idle and the spawn particle "
          "played there (each its reference given)", added == [("model+", scene_renderer, model + 0x108, scene, at)]
          and [(p[3], p[4]) for p in played] == [(particles[0], at), (particles[1], at)]
          and run.read(particles[0] + 0xD0) == 2, str(drawn))
    check("an unchanged frame draws nothing again", frame() == [])
    look(taker, (at[0] - 1.5, 1.5, 0.0), (0.738, -0.674, 0.0))
    press(taker, 0, now)
    press(taker, 1 << 21, now)
    drawn = frame()
    feet = tuple(struct.unpack("<3f", run.emu.mem_read(taker + 0x43C, 12)))
    check("taken: the model out of the scene and the idle particle removed, no destroy particle; the carrier's "
          "glow played on his body (each game draws it itself, never in the simulation)",
          decoded(state(run, rule)).get("carrier") == 10
          and ("model-", scene_renderer, model + 0x108, scene) in drawn
          and ("particle-", scene_renderer, scene, particles[0]) in drawn
          and [d[3:] for d in drawn if d[0] == "particle+"] == [(particles[3], feet)], str(drawn))
    check("the next frame moves the glow with him", [d[3] for d in frame() if d[0] == "particle~"] == [particles[3]])
    run.emu.mem_write(taker + 0x43C, struct.pack("<3f", 30.0, 0.0, 0.0))
    press(taker, 0, now)
    press(taker, 1 << 22, now)
    drawn = frame()
    lies = (30.0, 0.13, 0.0)
    got = [(d[0], d[-1]) for d in drawn]
    check("the Drop key: it lies 0.13 m over his feet (no ground ray without a physics world), the model and the "
          "idle there, no spawn particle", decoded(state(run, rule)).get("round") == 5
          and ("model+", lies) in [(d[0], tuple(round(v, 3) for v in d[-1])) for d in drawn if d[0] == "model+"]
          and sum(d[0] == "particle+" for d in drawn) == 1
          and ("particle-", scene_renderer, scene, particles[3]) in drawn, str(got))
    while decoded(state(run, rule)).get("round") == 5:
        now += 50
        run.word(CORE + TIME, now)
        run.call(0x2D1D20, args=(CORE + research.TICK_LIST, 50, now, 1))
    drawn = frame()
    check("removed by its 60 s: the model and the idle out, the destroy particle played where it lay",
          [d[3] for d in drawn if d[0] == "particle+"] == [particles[2]]
          and ("model-", scene_renderer, model + 0x108, scene) in drawn, str(drawn))

    # 16. A new artefact taken again, for the mark below.
    while decoded(state(run, rule)).get("round") == 2:
        now += 50
        run.word(CORE + TIME, now)
        run.call(0x2D1D20, args=(CORE + research.TICK_LIST, 50, now, 1))
    got = decoded(state(run, rule))
    look(taker, (SPAWNERS[got["spawner"]][0] - 1.5, 1.5, 0.0), (0.738, -0.674, 0.0))
    press(taker, 0, now)
    press(taker, 1 << 21, now)
    check("taken again: he carries it", decoded(state(run, rule)).get("carrier") == 10, str(decoded(state(run, rule))))

    # 17. The icon's carrier mark: the carrier's record gets the mark (CL 1,
    # EAX place x 3 at the record's compare); another's the game's own test.
    def mark(player):
        run.stopped = None
        run.span(0x1F2D8B, 0x1F2D98, {UC_X86_REG_ESI: player, UC_X86_REG_EAX: run.read(player + 0x114, 1),
                                      UC_X86_REG_EDX: inventory, UC_X86_REG_ECX: 0x77777700}, [0] * 8)
        return run.emu.reg_read(UC_X86_REG_EIP), run.emu.reg_read(UC_X86_REG_EAX), run.emu.reg_read(UC_X86_REG_ECX)
    inventory = run.allocate(0x200)                         # carries nothing (+178h 0)
    eip, eax, ecx = mark(taker)
    check("the carrier's icon is marked: at the record's compare with EAX place x 3 and CL 1",
          eip == 0x1F2D98 and eax == 30 and ecx == 0x77777701, f"{eip:#x} {eax} {ecx:#x}")
    run.span(0x1F2D8B, 0x1F2D98, {UC_X86_REG_ESI: players[11], UC_X86_REG_EAX: 11, UC_X86_REG_EDX: inventory},
             [0] * 8)
    check("another player's icon takes the game's own test (its compare of the inventory's carried item)",
          run.stopped == ("past", MARK_ON), str(run.stopped))

    # 18. The announcer: its seven lines (sound collections, class 258) asked
    # at a hunt's first hud frame in the game's language's voice; a cue's line
    # played once as a hud sound of the game world's scene, a running one
    # freed first.
    ui, holder, view, movie = run.allocate(0x400), run.allocate(0x200), run.allocate(0x10), run.allocate(0x40)
    run.word(ui + 0x18, game_world)
    run.word(ui + 8, holder)
    run.word(holder + 0x108, view)
    run.word(view + 4, movie)
    run.emu.mem_write(ui + 0x24, bytes([0]))
    run.emu.mem_write(0x21E75EC, b"english\0")
    run.word(0x21E75E0, 0x21E75EC)

    def hud():
        run.stopped = None
        run.voices = []
        run.span(0x23AA10, ICONS_ON, {}, [0, ui] + [0] * 8)
        return run.voices
    run.queries = []
    hud()
    check("the first hud frame asks the seven lines, sound collections in the english folder, as the pack has "
          "them (sounds/collections/vo/english/artifact_hunt/<line>)",
          run.queries == [[(f"vo/english/artifact_hunt/{n}", 258)] for n in LINES], str(run.queries[:2]))
    emitters = []
    for index in range(7):
        result, emitter = run.allocate(0x12C + 0x2E0), run.allocate(0x100)
        run.word(result + 0x38, 1)
        run.word(result + 0x12C, emitter)
        run.call(SYMBOLS["artefact_hunt_on_loaded"], args=(result, 4 + index))
        emitters.append(emitter)
    # place 0 (team 0) views; place 10 (team 1) drops it: the enemy's drop line
    look(taker, (30.0, 1.5, 0.0), (1.0, 0.0, 0.0))
    press(taker, 0, now)
    press(taker, 1 << 22, now)
    said = hud()
    check("an enemy's drop: its line (enemy_loose_art) emitted in the game world's scene with the sound world's "
          "user, played once (mode 0)", said[:2] == [("emit", emitters[4], sound_scene, user),
                                                    ("play", run.proxy, 0)], str(said))
    press(taker, 0, now)
    look(taker, (30.0, 1.6, 0.0), (0.0, -1.0, 0.0))
    press(taker, 1 << 21, now)
    said = hud()
    check("the next line frees the one playing first, then plays (enemy_got_art)",
          said and said[0][0] == "free" and ("emit", emitters[2], sound_scene, user) in said, str(said))

    # 19. Slot 9 through the world's loop (0x2D2119..0x2D214A): place 11
    # left, out of his team's side.
    before = struct.unpack_from("<II", state(run, rule), 10)
    eip, moved = run.span(0x2D2119, 0x2D214A, {UC_X86_REG_EBP: CORE}, [0] * 8 + [11, now])
    sides = struct.unpack_from("<II", state(run, rule), 10)
    check("driven: slot 9, left through the world's loop 0x2D2130: place 11 out of team 1's side",
          eip == 0x2D214A and moved == 0 and before[1] & 1 << 11 and sides == (before[0], before[1] & ~(1 << 11)),
          f"esp moved {moved}, sides {before} -> {sides}")

    # 20. The match's scene left (it drops the hunt, #2376: last).
    run.drawn = []
    run.span(0x1F9520, DEACTIVATE_ON, {UC_X86_REG_ECX: game_world}, [0] * 8)
    removed = sorted(d[3] for d in run.drawn if d[0] == "particle-")
    check("the match's scene left: the one-shot particles played are removed, nothing of ours stays",
          removed == sorted([particles[1], particles[2]]), str(run.drawn))
    print(f"{sum(results)}/{len(results)} checks passed")
    sys.exit(0 if all(results) else 1)


if __name__ == "__main__":
    main()
