#!/usr/bin/env python3
"""Run patch 103 (the Rust patch `spectator`: a game with no place of its own
watches its match) in the default build beside the original, at its four
sites, on made-up objects.

    python check_spectator_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are. A
network client (its vtable network_client's 0x9FFB14 or the replay client's
0x9FF344, its followed player +4, its +0Ch, its game +14h, its match options'
holder +1F1C48h, its local place +1F1C44h), a game (its game world inside at
+18h, +18h+9Ch the game again, +53ACh the client), a hud (+18h the game
world) and up to four players (+0D0h references, +114h place, +33Ch alive,
+33Fh inserted) are made up. The game's functions the patch calls are run as
stand-ins that keep what they were given: get_active_player (0x27A9C0, the
place's player, a reference added), set_followed_player (0x242B90, `ret 4`)
and switch_to_spectator_camera (0x1F8360, EDX).

- A player's game (a local place, or a replay's client): at every site the
  build reaches the instruction after the hooked span with every register,
  the flags and the made-up objects as the original has them there, and calls
  nothing.
- A spectator's (local place 0xFF, network_client's vtable):
  - after a world (0x27C116): its +0Ch 1; with no player followed, or one not
    in the match, set_followed_player of the first player in the match
    (alive first, else the first inserted), then switch_to_spectator_camera
    with the game's game world, and on at 0x27C1B9 (the join after the
    game's camera steps); with one in the match, nothing called, at 0x27C1B9;
    with nobody in the match, nothing called, at 0x27C1B9;
  - a key (0x25CDBD): next_player (17h) and prev_player (18h) the next and
    the previous living player of any team, the one followed left out, then
    0x25CEAE (the key's return, AL 0); any other action as the original;
  - a match line (0x208D77): [esp+48h] 0, on at 0x208DE2 (the plain line);
  - the hud's life byte (0x23B7B9): AL 1 for a dead player, the read
    skipped, at 0x23B7BF.
"""
import os
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI,
                               UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP,
                               UC_X86_REG_EFLAGS)

if len(sys.argv) > 1:
    os.environ.setdefault("SRP_026G_EXE", sys.argv[1])    # a span hook's bytes (patch_026g.RUST_HOOKS)
import patch_026g as base

NC_VTABLE, REPLAY_VTABLE = 0x9FFB14, 0x9FF344
GET_ACTIVE, FOLLOW, CAMERA, CLOSE, DETACH = 0x27A9C0, 0x242B90, 0x1F8360, 0x27A790, 0x242CC0
THREAD_SLOT = 0x870118
NC, GAME, OBJECTS, STUBS, STACK = 0x40000000, 0x48000000, 0x50000000, 0x51000000, 0x52000000
HOLDER, UI = OBJECTS + 0x0, OBJECTS + 0x60000
PLAYERS = [OBJECTS + 0x70000 + 0x1000 * i for i in range(4)]
SP = STACK + 0x8000
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP)
# where each site's span ends, where the patch's exit goes
SITES = {
    "world": (0x27C116, bytes.fromhex("8b470485c0"), 0x27C11B, 0x27C1B9),
    "key": (0x25CDBD, bytes.fromhex("8bb3ac530000"), 0x25CDC3, 0x25CEAE),
    "chat": (0x208D77, bytes.fromhex("0fb686441c1f00"), 0x208D7E, 0x208DE2),
    "hud": (0x23B7B9, bytes.fromhex("8a853c030000"), 0x23B7BF, None),
    "left": (0x27C72C, bytes.fromhex("8a86441c1f003cff"), 0x27C734, 0x27C79F),
}

failed = 0


def check(ok, what):
    global failed
    failed += not ok
    print(f"  {'ok' if ok else 'FAIL'}  {what}")


class Game:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(NC, 0x200000, UC_PROT_ALL)
        emu.mem_map(GAME, 0x10000, UC_PROT_ALL)
        emu.mem_map(OBJECTS, 0x80000, UC_PROT_ALL)
        for at in (STUBS, STACK):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        self.word(THREAD_SLOT, STUBS + 0x80)
        emu.mem_write(STUBS + 0x80, b"\xB8\x01\x00\x00\x00\xC3")
        self.calls = []
        for va, how in ((GET_ACTIVE, self.get_active), (FOLLOW, self.follow), (CAMERA, self.camera),
                        (CLOSE, self.close), (DETACH, self.detach)):
            emu.hook_add(UC_HOOK_CODE, how, begin=va, end=va)

    def word(self, at, value, size=4):
        self.emu.mem_write(at, (value & (1 << 8 * size) - 1).to_bytes(size, "little"))

    def read(self, at, size=4):
        return int.from_bytes(self.emu.mem_read(at, size), "little")

    def ret(self, pop):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pop)

    def get_active(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        out, place = self.read(sp + 4), self.read(sp + 8) & 0xFF
        player = self.players.get(place, 0)
        if player:
            self.word(player + 0xD0, self.read(player + 0xD0) + 1)
        self.word(out, player)
        self.calls.append(("get_active_player", emu.reg_read(UC_X86_REG_ECX), place))
        emu.reg_write(UC_X86_REG_EAX, out)
        self.ret(8)

    def follow(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        player = self.read(sp + 4)
        self.word(player + 0xD0, self.read(player + 0xD0) - 1)     # it releases its argument
        nc = emu.reg_read(UC_X86_REG_ECX)
        self.calls.append(("set_followed_player", nc, player))
        self.word(nc + 4, player)               # the camera's player and the last followed (0x242BDA, 0x242C1A)
        self.word(nc + 8, player)
        self.ret(4)

    def detach(self, emu, *_):
        nc = emu.reg_read(UC_X86_REG_ECX)
        self.calls.append(("detach_from_player", nc))
        self.word(nc + 4, 0)                    # +4 let go (+8 kept), then the free-fly camera
        self.ret(0)

    def close(self, emu, *_):
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.calls.append(("close_current_match", emu.reg_read(UC_X86_REG_ECX), self.read(sp + 4)))
        self.ret(4)

    def camera(self, emu, *_):
        self.calls.append(("switch_to_spectator_camera", emu.reg_read(UC_X86_REG_EDX)))
        self.ret(0)

    def setup(self, vtable, local, followed, players):
        """players: {place: (alive, inserted)}; followed: a place or None."""
        emu = self.emu
        emu.mem_write(NC, bytes(0x200000))
        emu.mem_write(GAME, bytes(0x10000))
        emu.mem_write(OBJECTS, bytes(0x80000))
        self.word(NC, vtable)
        self.word(NC + 0x14, GAME)
        self.word(NC + 0x1F1C48, HOLDER)
        self.word(HOLDER + 0xC + 0x577B5, len(PLAYERS), 1)
        self.word(NC + 0x1F1C44, local, 1)
        self.word(GAME + 0x18 + 0x9C, GAME)
        self.word(GAME + 0x53AC, NC)
        self.word(UI + 0x18, GAME + 0x18)
        self.players = {}
        for place, (alive, inserted) in players.items():
            p = PLAYERS[place]
            self.players[place] = p
            self.word(p + 0xD0, 1)
            self.word(p + 0x114, place, 1)
            self.word(p + 0x33C, alive, 1)
            self.word(p + 0x33F, inserted, 1)
        if followed is not None:
            self.word(NC + 4, PLAYERS[followed])
        self.calls = []

    def run(self, site, regs, stack=b""):
        start, _, past, exit_ = SITES[site]
        emu = self.emu
        emu.mem_write(STACK, bytes(0x10000))
        emu.mem_write(SP, stack)
        for reg, value in regs.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, SP)
        emu.reg_write(UC_X86_REG_EFLAGS, 0x202)
        stops = {past, exit_}
        hook = emu.hook_add(UC_HOOK_CODE, lambda e, a, *_: e.emu_stop() if a in stops else None)
        emu.emu_start(start, 0, count=500000)
        emu.hook_del(hook)
        state = {r: emu.reg_read(r) for r in REGS + (UC_X86_REG_ESP, UC_X86_REG_EIP)}
        state["flags"] = emu.reg_read(UC_X86_REG_EFLAGS) & 0x8D5
        objects = (bytes(emu.mem_read(NC, 0x20)), bytes(emu.mem_read(NC + 0x1F1C40, 0x30)),
                   bytes(emu.mem_read(GAME, 0x10000)), bytes(emu.mem_read(OBJECTS, 0x80000)),
                   bytes(emu.mem_read(SP, STACK + 0x10000 - SP)))       # the entries' own room is below the site's ESP
        return state, objects, list(self.calls)


STEADY = dict(zip(REGS, (0x0A0A0A0A, 0x0B0B0B0B, 0x0C0C0C0C, 0x0D0D0D0D, 0x05050505, 0x0E0E0E0E, 0x0019FF00)))
MATCH = {0: (1, 0), 1: (1, 1), 2: (0, 1), 3: (1, 1)}   # place: (alive, inserted)


def site_regs(site, action=0x17):
    regs = dict(STEADY)
    if site == "world":
        regs[UC_X86_REG_EDI] = NC
    elif site == "key":
        regs[UC_X86_REG_EBX] = GAME
        regs[UC_X86_REG_EDI] = action
    elif site == "chat":
        regs[UC_X86_REG_ESI] = NC
    elif site == "left":
        regs[UC_X86_REG_ESI] = NC
        regs[UC_X86_REG_EBX] = 0x0B0B0B00 | action      # BL the place that left, the rest the tick's
    else:
        regs[UC_X86_REG_ESI] = UI
        regs[UC_X86_REG_EBP] = PLAYERS[2]
    return regs


def both(stock, ours, site, case, regs, stack=b"", **setup):
    stock.setup(**setup)
    ours.setup(**setup)
    s = stock.run(site, regs, stack)
    o = ours.run(site, regs, stack)
    past = SITES[site][2]
    check(s[0][UC_X86_REG_EIP] == past and s == o,
          f"{site} {case}: at {past:#x} as the original, registers, flags and objects the same, nothing called")


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    built = bytes(built)
    linked = base.arena()
    block = linked.blocks["patches.code"]
    installs = {h[0]: h for h in base.rust_hooks(original)}
    for site, (start, span, _, _) in SITES.items():
        va, old, symbol, opcode = installs[start]
        at = base.file_offset(va)
        new = base.hook_bytes(linked.symbols[symbol], va, old, 0, opcode)
        check(opcode == 0xE9 and old == span and original[at:at + len(span)] == span
              and built[at:at + len(new)] == new and block.va <= linked.symbols[symbol] < block.va + block.size,
              f"{site}: {va:#x} {old.hex()} in the original, `jmp {symbol}` (patches.code) here")
    epilogue = base.file_offset(0x25CEAE)
    check(original[epilogue:epilogue + 9] == bytes.fromhex("32c05f5e5b8be55dc2"),
          "0x25CEAE is on_keyboard_action's `xor al,al` and epilogue (its next/prev player's return)")

    stock, ours = Game(original), Game(built)
    # A player's game, and a replay's client, run the game's code at each site.
    for vtable, local, name in ((NC_VTABLE, 2, "a player (local place 2)"),
                                (REPLAY_VTABLE, 0xFF, "a replay's client")):
        for site in SITES:
            values = {"key": (0x17, 0x18, 0x12, 0x11), "left": (2, 0xFF, 1, 3)}.get(site, (0x17,))
            for action in values:
                case = {"key": f"{name}, action {action:#x}", "left": f"{name}, 0xEA of place {action:#x}"}
                both(stock, ours, site, case.get(site, name),
                     site_regs(site, action), struct.pack("<20I", *range(0x100, 0x114)),
                     vtable=vtable, local=local, followed=1, players=MATCH)

    # leave_match (0x2D18A0, before the handler's local test: every game
    # runs it) of place 0xFF, the game's own code on a made-up core whose
    # history has one item at the leave's tick, all twenty places entered:
    # the place's bit is `shl edx,cl` with CL masked to 31 by the CPU, bit
    # 31, which no entered mask has, so it returns 1 at 0x2D1CC6 with
    # nothing written and the leave (0x2D1100) not called; place 3 reaches it.
    core, item = OBJECTS + 0x40000, OBJECTS + 0x3F000
    for place, leaves in ((0xFF, False), (3, True)):
        emu = stock.emu
        emu.mem_write(OBJECTS + 0x3F000, bytes(0x1D000))
        stock.word(core + 0x18FD4, 1000)          # the core's time: the leave's tick is ahead of it
        stock.word(core + 0x18FC4, 1000)
        stock.word(core + 0x18FDD, 1, 1)
        stock.word(core + 0x12284, item)          # the history's first item
        stock.word(item + 0x8, 0xFFFFF)           # its entered mask: places 0..19
        stock.word(item + 0xC, 1050)              # its tick
        emu.mem_write(SP, struct.pack("<3I", STUBS + 0xF00, core, place))
        emu.reg_write(UC_X86_REG_EAX, 1050)
        emu.reg_write(UC_X86_REG_ESP, SP)
        reached = []
        hook = emu.hook_add(UC_HOOK_CODE, lambda e, a, *_: (reached.append(a), e.emu_stop())
                            if a in (0x2D1100, STUBS + 0xF00) else None)
        before = bytes(emu.mem_read(OBJECTS + 0x3F000, 0x1D000))
        emu.emu_start(0x2D18A0, 0, count=100000)
        emu.hook_del(hook)
        after = bytes(emu.mem_read(OBJECTS + 0x3F000, 0x1D000))
        if leaves:
            check(reached == [0x2D1100], "leave_match of place 3 (entered): on to the leave, 0x2D1100")
        else:
            check(reached == [STUBS + 0xF00] and emu.reg_read(UC_X86_REG_EAX) == 1 and before == after
                  and emu.reg_read(UC_X86_REG_ESP) == SP + 12,
                  "leave_match of place 0xFF: returns 1 (ret 8), the core and its history unwritten, no leave")

    # An 0xEA naming 0xFF in a player's game (the server never sends one):
    # past leave_match, not the local place, not the followed player's: the
    # handler's return, nothing called.
    stock.setup(vtable=NC_VTABLE, local=2, followed=1, players=MATCH)
    stock.emu.mem_write(STACK, bytes(0x10000))
    for reg, value in site_regs("left", 0xFF).items():
        stock.emu.reg_write(reg, value)
    stock.emu.reg_write(UC_X86_REG_ESP, SP)
    hook = stock.emu.hook_add(UC_HOOK_CODE, lambda e, a, *_: e.emu_stop() if a == 0x27C79F else None)
    before = bytes(stock.emu.mem_read(OBJECTS, 0x80000)), bytes(stock.emu.mem_read(NC, 0x20))
    stock.emu.emu_start(0x27C72C, 0, count=100000)
    stock.emu.hook_del(hook)
    after = bytes(stock.emu.mem_read(OBJECTS, 0x80000)), bytes(stock.emu.mem_read(NC, 0x20))
    check(stock.emu.reg_read(UC_X86_REG_EIP) == 0x27C79F and not stock.calls and before == after,
          "left, a player, 0xEA of place 0xFF in the original: on to the handler's return, nothing called or written")

    # The same game a player again (a spectator taken in: it reconnects, the
    # network client is the game's one): its first world turns its input on.
    for game in (stock, ours):
        game.setup(vtable=NC_VTABLE, local=2, followed=None, players=MATCH)
        game.word(NC + 0xC, 1, 1)
    s, o = stock.run("world", site_regs("world")), ours.run("world", site_regs("world"))
    check(s[0] == o[0] and s[0][UC_X86_REG_EIP] == 0x27C11B and stock.read(NC + 0xC, 1) == 1
          and ours.read(NC + 0xC, 1) == 0 and not o[2],
          "world, a player whose client watched a match before (+0Ch 1): +0Ch 0, the rest as the original")

    # A spectator's game.
    def spectator(site, followed, players=MATCH, action=0x17, stack=b""):
        ours.setup(vtable=NC_VTABLE, local=0xFF, followed=followed, players=players)
        return ours.run(site, site_regs(site, action), stack)

    world = GAME + 0x18
    for followed, players, case, calls in (
            (None, MATCH, "nobody followed: the first living player in the match (1)",
             [("set_followed_player", NC, PLAYERS[1]), ("switch_to_spectator_camera", world)]),
            (0, MATCH, "one not in the match followed (0): the first (1)",
             [("set_followed_player", NC, PLAYERS[1]), ("switch_to_spectator_camera", world)]),
            (None, {0: (0, 0), 2: (0, 1)}, "nobody alive: the first inserted (2)",
             [("set_followed_player", NC, PLAYERS[2]), ("switch_to_spectator_camera", world)]),
            (3, MATCH, "one in the match followed (3): kept", []),
            (None, {0: (1, 0)}, "nobody in the match: nothing", [])):
        state, objects, done = spectator("world", followed, players)
        made = [c for c in done if c[0] != "get_active_player"]
        refs = [ours.read(p + 0xD0) for p in ours.players.values()]
        check(state[UC_X86_REG_EIP] == 0x27C1B9 and made == calls and ours.read(NC + 0xC, 1) == 1
              and state[UC_X86_REG_EDI] == NC and all(r == 1 for r in refs),
              f"world, a spectator, {case}: +0Ch 1, at 0x27C1B9, references as they were")

    for action, followed, case, to in ((0x17, 1, "next from 1", 3), (0x17, 3, "next from 3, round", 1),
                                       (0x18, 1, "previous from 1, round", 3), (0x18, None, "previous from nobody", 1)):
        state, _, done = spectator("key", followed, action=action)
        made = [c for c in done if c[0] != "get_active_player"]
        check(state[UC_X86_REG_EIP] == 0x25CEAE and state[UC_X86_REG_EBX] == GAME
              and made == [("set_followed_player", NC, PLAYERS[to]), ("switch_to_spectator_camera", world)],
              f"key, a spectator, {case}: player {to} (2 dead, 0 not in the match), then 0x25CEAE")
    state, _, done = spectator("key", 1, players={1: (1, 1)}, action=0x17)
    check(state[UC_X86_REG_EIP] == 0x25CEAE and not [c for c in done if c[0] != "get_active_player"],
          "key, a spectator alone with the one followed: nothing called, the key taken")
    s_state, _, _ = spectator("key", 1, action=0x11)
    stock.setup(vtable=NC_VTABLE, local=0xFF, followed=1, players=MATCH)
    o_state = stock.run("key", site_regs("key", 0x11))[0]
    check(s_state == o_state and s_state[UC_X86_REG_EIP] == 0x25CDC3,
          "key, a spectator, another action (11h): at 0x25CDC3 as the original")

    state, objects, done = spectator("chat", 1, stack=bytes(0x48) + b"\xEF\xBE\xAD\xDE")
    check(state[UC_X86_REG_EIP] == 0x208DE2 and state[UC_X86_REG_ESP] == SP and not done
          and ours.read(SP + 0x48) == 0 and state[UC_X86_REG_ESI] == NC,
          "chat, a spectator: [esp+48h] 0, at 0x208DE2 (a plain line), ESP as at the site, nothing called")

    state, _, done = spectator("left", 1, action=0xFF)
    check(state[UC_X86_REG_EIP] == 0x27C79F and done == [("close_current_match", NC, 0)]
          and state[UC_X86_REG_ESP] == SP and state[UC_X86_REG_ESI] == NC,
          "left, a spectator, 0xEA of place 0xFF: close_current_match(0) (the world ended, no results), "
          "then the handler's return 0x27C79F")
    state, _, done = spectator("left", 1, action=1)
    made = [c for c in done if c[0] != "get_active_player"]
    check(state[UC_X86_REG_EIP] == 0x27C79F
          and made == [("set_followed_player", NC, PLAYERS[3]), ("switch_to_spectator_camera", world)],
          "left, a spectator, 0xEA of the place it follows (1): the next living player (3), then 0x27C79F")
    ours.setup(vtable=NC_VTABLE, local=0xFF, followed=1, players=MATCH)
    stock.setup(vtable=NC_VTABLE, local=0xFF, followed=1, players=MATCH)
    s, o = stock.run("left", site_regs("left", 3)), ours.run("left", site_regs("left", 3))
    check(s == o and s[0][UC_X86_REG_EIP] == 0x27C734,
          "left, a spectator, 0xEA of another place (3): at 0x27C734 as the original (its je: nothing)")

    state, _, done = spectator("hud", 1)
    check(state[UC_X86_REG_EIP] == 0x23B7BF and state[UC_X86_REG_EAX] & 0xFF == 1
          and state[UC_X86_REG_EAX] >> 8 == STEADY[UC_X86_REG_EAX] >> 8 and not done,
          "hud, a spectator, the followed player dead: AL 1 (the rest of EAX kept), at 0x23B7BF")

    # Detach (F2, cam_2): one spectator's game through a flight, the patch's
    # camera state kept between the runs (its arena's).
    def made(done):
        return [c for c in done if c[0] != "get_active_player"]

    ours.setup(vtable=NC_VTABLE, local=0xFF, followed=1, players=MATCH)
    ours.word(NC + 8, PLAYERS[1])                      # the last one followed, set_followed_player's +8
    state, _, done = ours.run("key", site_regs("key", 0x12))
    check(state[UC_X86_REG_EIP] == 0x25CEAE and made(done) == [("detach_from_player", NC)]
          and ours.read(NC + 4) == 0 and ours.read(NC + 0xC, 1) == 1,
          "detach: F2 (12h) lets go (detach_from_player: +4 0, the game's free-fly camera), +0Ch 1, the key's return")
    ours.calls = []
    state, _, done = ours.run("world", site_regs("world"))
    check(state[UC_X86_REG_EIP] == 0x27C1B9 and not made(done) and ours.read(NC + 4) == 0,
          "detach: a world's load keeps the flight (nobody followed, nothing called), at 0x27C1B9")
    ours.calls = []
    state, _, done = ours.run("left", site_regs("left", 1))
    check(state[UC_X86_REG_EIP] == 0x27C734 and not made(done),
          "detach: an 0xEA of the last one followed (1): the original's je (nothing), still flying")
    ours.calls = []
    state, _, done = ours.run("key", site_regs("key", 0x17))
    check(state[UC_X86_REG_EIP] == 0x25CEAE
          and made(done) == [("set_followed_player", NC, PLAYERS[3]), ("switch_to_spectator_camera", world)],
          "detach: F6 follows again, the next from the last one followed (1 -> 3)")
    ours.word(NC + 4, 0)                               # flying again, from 3
    ours.calls = []
    ours.run("key", site_regs("key", 0x12))
    ours.calls = []
    state, _, done = ours.run("key", site_regs("key", 0x18))
    check(made(done) == [("set_followed_player", NC, PLAYERS[1]), ("switch_to_spectator_camera", world)],
          "detach: F5 after a flight from 3: the previous living one (1)")
    # (A new match's options end a flight: `static_info.options_read`, the
    # camera's step NewMatch, its unit test in spectator.rs; the site's chain
    # of patches 65, 86 and 58 needs a real 0xE4's reader and is not run here.)

    # The input while flying: game_world::tick (0x1F780C on, the original's
    # code) builds and sends input only from 0x1F7868; with +4 0 it goes to
    # 0x1F794A at 0x1F784D, and with a player followed and +0Ch 1 at 0x1F7862.
    for followed, observer, to, case in ((None, 1, 0x1F794A, "flying (+4 0)"),
                                         (1, 1, 0x1F794A, "following, +0Ch 1"),
                                         (1, 0, 0x1F7868, "following, +0Ch 0 (a player's)")):
        stock.setup(vtable=NC_VTABLE, local=0xFF, followed=followed, players=MATCH)
        stock.word(NC + 0xC, observer, 1)
        emu = stock.emu
        for reg, value in STEADY.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_EBX, GAME + 0x18)    # the game world
        emu.reg_write(UC_X86_REG_ESP, SP)
        hook = emu.hook_add(UC_HOOK_CODE, lambda e, a, *_: e.emu_stop() if a in (0x1F794A, 0x1F7868) else None)
        emu.emu_start(0x1F780C, 0, count=100000)
        emu.hook_del(hook)
        check(emu.reg_read(UC_X86_REG_EIP) == to,
              f"game_world::tick, {case}: {'no input built' if to == 0x1F794A else 'the input built'} ({to:#x})")
    if failed:
        sys.exit(f"{failed} check(s) failed")
    print("all checks passed")


if __name__ == "__main__":
    main()
