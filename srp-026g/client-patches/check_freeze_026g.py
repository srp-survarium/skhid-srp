#!/usr/bin/env python3
"""Run patch 71 (freeze_026g.asm and freeze_026g.c: the freeze detector) in
the default build on made-up game objects.

    python check_freeze_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, the
arena with them. QueryPerformanceCounter and QueryPerformanceFrequency (their
import slots 0x870158, 0x870150) are stubs that give a scripted clock (10 MHz);
`rdtsc` in the arena's code gives a scripted time stamp counter (3 GHz); the
game's logger (0x111ED0) is a stub that records each line (the format must be
"%s", the channel "game", the level 4).

- every hook: each of the 23 entries runs from its site, the original and
  the build alike, with made-up registers and flags and a made-up caller's
  stack, until the function's own code past the site's bytes (FREEZE_BACKS):
  the registers, the flags and the stack must be the original's there (a
  timed function's return address, alone, the leave's), and the call counted.
  Then the function's `ret n` is played (made-up result registers and flags,
  n bytes popped): the build must come back to the caller with exactly those
  registers and flags, the stack the original's (n popped), the slot free and
  the time added to the function's total ((ticks x frz_tsc_mult) >> 32). A
  timed function entered three times before it returns (two slots taken, the
  third counted and not timed) returns to each caller in turn;
- the state: logic_tick's leave names it from made-up games (none, loading,
  the world with a network client in waiting, countdown, play or out of a
  match, a replay's client, no client, the lobby, the login);
- the frames: the present's leave measures each frame with the clock; 101
  frames of 10 ms (the clock's second: frz_tsc_mult), then frames with
  known calls and times in them: a long frame's line must be the expected
  text with what each counter grew by in that frame alone (a frame after it
  has its own, all 0 when nothing ran), among them ticks to times already
  ticked to after a rollback (resimulated, by atomic_tick's edi) and the
  bullets fired inside them (replayed), against a reference; a frame under srp_freeze_ms or under
  three times the median is not logged; srp_freeze_ms 0 logs none;
- a match: frames in the lobby, loading, waiting, countdown and play with
  known times, back to the lobby: the summary's lines (the world's time and
  frames, p50, p95, p99 and the longest, the long frames by state, the three
  worst with their counters) as a reference computes them; at most 100 lines
  of long frames (20 while loading) and then one that says so;
- the variable: game::game's call of the game world's constructor (0x1E90A3)
  runs with the constructor a stub (ret 8) and the game's own cc_value<u32>
  constructor (0x1DCD0); the call must reach the stub with the world and the
  game (eax the world), come back with its registers and both arguments
  popped, and put one cc_u32 (0x9E7CCC) named srp_freeze_ms on frz_limit_ms
  (40), 0 to 100000, saved, kind 2, at the head of the commands' list; the
  game's lookup (0xEB7C0) finds it and its execute (0xEB980, a user.cfg line:
  filter 1, mask 2; the CRT's sscanf 0x422B46 a stub) sets the value.
"""
import struct
import sys

from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_EFLAGS

import patch_026g as base

QPC_SLOT, QPF_SLOT = 0x870158, 0x870150
LOG, SSCANF = 0x111ED0, 0x422B46
GAME_WORLD_SITE, GAME_WORLD_BACK, GAME_WORLD = 0x1E90A3, 0x1E90A8, 0x1F67B0
LOOKUP, EXECUTE, COMMANDS, CC_U32 = 0xEB7C0, 0xEB980, 0x1F1D66C, 0x9E7CCC
CLIENT_VTABLE, CLIENT_STATE = 0x9FFB14, 0x1F1C60
STUBS, STACK, SENTINEL, OBJECTS, TEXT = 0x30000000, 0x21000000, 0x22000000, 0x24000000, 0x23000000
QPC, QPF, CREATE_FILE, WRITE_FILE, LOCAL_TIME = STUBS, STUBS + 0x10, STUBS + 0x20, STUBS + 0x30, STUBS + 0x40
CREATE_SLOT, WRITE_SLOT, TIME_SLOT, USER_DATA = 0x870048, 0x870108, 0x870184, 0x18BD0
NOW = (2026, 10, 0, 25, 17, 3, 12, 345)       # GetLocalTime's: 2026-10-25 17:03:12.345
MATCH_CLIENT = OBJECTS + 0x100000             # a network client's match client (+1F1C48h), its id +577BCh
CRC, SORT = 0x42050, 0x16A530
FOLDER = b"c:\\users\\owner\\documents\\survarium"
HANDLE = 0x4444
REGS = (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP)
GAME = dict(zip(REGS, (0x0A0A0A0A, 0x0C0C0C0C, 0x0D0D0D0D, 0x0B0B0B0B, 0x05050505, 0x0E0E0E0E, 0x0019FF00)))
FLAGS = 0x8D7                          # CF PF AF ZF SF OF set, IF clear, DF clear
FLAG_BITS = 0xCD5                      # CF PF AF ZF SF DF OF
FREQUENCY, TSC_PER_US = 10_000_000, 3000
# (name, site, index, timed, the function's `ret n`): FREEZE_HOOKS' entries
HOOKS = (
    ("logic", 0x01A030, 0, True, 4), ("present", 0x3CF7F0, 1, True, 4), ("sim", 0x2D2E40, 2, True, 4),
    ("tick", 0x2D1E20, 3, False, 8), ("rollback", 0x2D2770, 4, True, 12), ("message", 0x27A300, 5, True, 8),
    ("datagram", 0x3F3BB0, 6, False, 20), ("bake", 0x12E510, 7, True, 24), ("baked load", 0x12DB40, 8, True, 16),
    ("sync load", 0x0E5C20, 9, True, 0), ("cook", 0x1096C0, 10, False, 4), ("callbacks", 0x0E5AB0, 11, True, 0),
    ("particle", 0x022660, 12, False, 28), ("invoke", 0x511600, 13, False, 16), ("advance", 0x51F1C0, 14, True, 12),
    ("flash render", 0x13CED0, 15, True, 8), ("shot", 0x322830, 16, False, 32), ("hit", 0x32EBD0, 17, False, 16),
    ("decal", 0x321FA0, 18, False, 32), ("impact", 0x3223E0, 19, False, 20), ("sound", 0x05ABA0, 20, False, 12),
    ("kill", 0x2DA170, 23, False, 20), ("kill row", 0x237C70, 24, False, 20),
    ("particle gone", 0x0229D0, 25, False, 12), ("tick events", 0x2D21E0, 34, True, 12),
    ("render", 0x13D010, 35, True, 28), ("particle tick", 0x35EEB0, 36, True, 12), ("sprites", 0x1669D0, 37, True, 36),
    ("streaming", 0x155980, 39, True, 0),
)
# The calls made calls of a wrapper: (name, site, index, the callee, its `ret n`)
CALLS = (("crc", 0x12F340, 32, CRC, 4), ("crc", 0x3C0FF0, 32, CRC, 4), ("crc", 0x10A10E, 32, CRC, 4),
         ("crc", 0x10FD36, 32, CRC, 4), ("crc", 0x3DD515, 32, CRC, 4), ("sort", 0x178093, 38, SORT, 12),
         ("sort", 0x1964D1, 38, SORT, 12), ("refused", 0x27D908, 40, LOG, 0), ("refused", 0x27DA7F, 40, LOG, 0),
         ("refused", 0x27DB54, 40, LOG, 0), ("refused", 0x27E2E8, 40, LOG, 0), ("refused", 0x27E45F, 40, LOG, 0),
         ("refused", 0x27E534, 40, LOG, 0))
COUNTERS = 41
REFUSED = 40
TIMED = {0, 1, 2, 4, 5, 7, 8, 9, 11, 14, 15, 32, 34, 35, 36, 37, 38, 39}
SPLITS, RELAYS, ASKED, WORLDS, MOVES, DISCARDS, CRC_BYTES = 26, 27, 28, 29, 30, 31, 33
COLUMNS = ("logic", "present", "sim", "ticks", "rollbacks", "messages", "datagrams", "bakes", "baked_loads",
           "sync_loads", "cooks", "callbacks", "particles", "invokes", "advance", "flash_render", "shots", "hits",
           "decals", "impacts", "sounds", "resimulated", "replayed", "kills", "feed_rows", "particles_gone",
           "splits", "relays", "worlds_asked", "worlds", "moves", "discards", "crc", "crc_bytes", "tick_events",
           "render", "emitter_ticks", "sprites", "sorts", "streaming", "refused")
RESIM_TICKS, RESIM_SHOTS = 21, 22
S_MENU, S_LOBBY, S_LOADING, S_WAITING, S_COUNTDOWN, S_PLAY, S_REPLAY = range(7)
STATE_NAMES = ("menu", "lobby", "loading", "waiting", "countdown", "play", "replay")
# The frame line's counters, in its order: (text, index, how: "timed", "count" or "time"; a "count"
# text ending in "(" is the count before it in brackets)
FIELDS = (("logic", 0, "timed"), ("tick events", 34, "timed"), ("present", 1, "time"), ("render", 35, "timed"),
          ("sim", 2, "timed"), ("ticks", 3, "count"), ("(resimulated", 21, "count"), ("+split", 26, "count"),
          ("rollbacks", 4, "count"), ("shots", 16, "count"),
          ("(replayed", 22, "count"), ("hits", 17, "count"), ("decals", 18, "count"), ("impacts", 19, "count"),
          ("sounds", 20, "count"), ("kills", 23, "count"), ("feed rows", 24, "count"),
          ("messages", 5, "timed"), ("(relays", 27, "count"), ("+worlds", 29, "count"), ("+moves", 30, "count"),
          ("+discards", 31, "count"), ("+refused", 40, "count"), ("+worlds asked", 28, "count"), ("datagrams", 6, "count"),
          ("bakes", 7, "timed"), ("baked loads", 8, "timed"), ("crc", 32, "timed"), ("(bytes", 33, "count"),
          ("sync loads", 9, "timed"), ("cooks", 10, "count"),
          ("callbacks", 11, "timed"), ("streaming", 39, "timed"), ("particles", 12, "count"), ("(alive", -1, "count"),
          ("emitter ticks", 36, "timed"), ("sprites", 37, "timed"), ("sorts", 38, "timed"), ("invokes", 13, "count"),
          ("advance", 14, "timed"), ("flash render", 15, "time"))
# The made-up engine world and game (freeze_026g.c's offsets)
WORLD, GAME_OBJ, CLIENT = OBJECTS, OBJECTS + 0x1000, OBJECTS + 0x10000
ENGINE_GAME, GAME_WORLD_AT, GAME_LOADING, GAME_LOBBY, GAME_SCENE, GAME_CLIENT = \
    0x2D0, 0x18, 0x4E38, 0x5390, 0x539C, 0x53AC
LOBBY_MENU, LOGIN_MENU = OBJECTS + 0x300000, OBJECTS + 0x301000
ARGUMENTS = OBJECTS + 0x200000         # the hooked functions' made-up arguments: zeroed (a world without a game)
T0 = 1_000_000_000 + 7_654_321         # the counter at the first frame's end (the present's case below)
PATTERN = 0xA5                         # what the stack holds before each case


class Run:
    def __init__(self, data, linked=None):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(STUBS, 0x1000, UC_PROT_ALL)
        emu.mem_write(STUBS, b"\xC3" * 0x1000)
        for at, size in ((STACK, 0x10000), (SENTINEL, 0x10000), (TEXT, 0x10000), (OBJECTS, 0x400000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        self.word(QPC_SLOT, QPC)
        self.word(QPF_SLOT, QPF)
        self.word(CREATE_SLOT, CREATE_FILE)
        self.word(WRITE_SLOT, WRITE_FILE)
        self.word(TIME_SLOT, LOCAL_TIME)
        self.emu.mem_write(TEXT + 0x900, FOLDER + b"\0")
        self.opened, self.written = [], []
        self.qpc = 0
        self.tsc = 0
        self.lines = []
        self.stopped = None
        emu.hook_add(UC_HOOK_CODE, self.stub, begin=STUBS, end=STUBS + 0xFFF)
        for at in (LOG, SSCANF, GAME_WORLD, USER_DATA):
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=at, end=at)
        self.linked = linked
        if linked is not None:
            code = linked.blocks["freeze.code"]
            emu.hook_add(UC_HOOK_CODE, self.rdtsc, begin=code.va, end=code.va + code.size - 1)
            self.sym = linked.symbols
        self.backs = set()

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at, size=4):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0] if size == 4 else bytes(self.emu.mem_read(at, size))

    def text(self, at, most=1024):
        return bytes(self.emu.mem_read(at, most)).split(b"\0")[0]

    def rdtsc(self, emu, address, size, _):
        if size == 2 and bytes(emu.mem_read(address, 2)) == b"\x0f\x31":
            emu.reg_write(UC_X86_REG_EAX, self.tsc & 0xFFFFFFFF)
            emu.reg_write(UC_X86_REG_EDX, self.tsc >> 32)
            emu.reg_write(UC_X86_REG_EIP, address + 2)

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 * i) for i in range(1, 12)]
        if address in (QPC, QPF):                  # stdcall (&a 64-bit integer), ret 4; eax 1
            value = self.qpc if address == QPC else FREQUENCY
            emu.mem_write(args[0], struct.pack("<Q", value))
            pops, result = 4, 1
        elif address == LOG:                       # cdecl (&function, a, b, file, line, function, channel,
            try:
                self.lines.append((self.text(args[8]), self.text(args[6]), args[7], self.text(args[9])))
            except UcError:                        # a call with made-up arguments (the refused discards' case)
                self.lines.append(None)
            pops, result = 0, 0                    #        level, format, the text)
        elif address == SSCANF:                    # cdecl (text, "%d", &value): 1 when a number
            words = self.text(args[0]).split()
            try:
                emu.mem_write(args[2], struct.pack("<i", int(words[0])))
                result = 1
            except (ValueError, IndexError):
                result = 0
            pops = 0
        elif address == CREATE_FILE:               # stdcall (name, access, share, security, how, flags, template)
            self.opened.append((self.text(args[0]), args[1], args[2], args[4], args[5]))
            pops, result = 28, HANDLE
        elif address == WRITE_FILE:                # stdcall (file, bytes, count, &written, overlapped)
            self.written.append((args[0], bytes(emu.mem_read(args[1], args[2]))))
            self.word(args[3], args[2])
            pops, result = 20, 1
        elif address == LOCAL_TIME:                # stdcall (&SYSTEMTIME)
            emu.mem_write(args[0], struct.pack("<8H", *NOW))
            pops, result = 4, 0
        elif address == USER_DATA:                 # get_user_data_directory: eax the folder
            pops, result = 0, TEXT + 0x900
        elif address == GAME_WORLD:                # the world, the game; ret 8
            self.calls.append(("game_world", emu.reg_read(UC_X86_REG_EAX), args[0], args[1]))
            for reg, value in self.stub_regs.items():
                emu.reg_write(reg, value)
            pops, result = 8, self.stub_regs[UC_X86_REG_EAX]
        else:
            return
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def stop(self, emu, address, size, _):
        self.stopped = address
        emu.emu_stop()

    def stop_at(self, address):
        if address not in self.backs:
            self.backs.add(address)
            self.emu.hook_add(UC_HOOK_CODE, self.stop, begin=address, end=address)

    def state(self):
        regs = {reg: self.emu.reg_read(reg) for reg in REGS}
        return regs, self.emu.reg_read(UC_X86_REG_ESP), self.emu.reg_read(UC_X86_REG_EFLAGS) & FLAG_BITS

    def go(self, start, until, regs, sp, flags=FLAGS):
        for reg, value in regs.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.emu.reg_write(UC_X86_REG_EFLAGS, flags)
        self.stopped = None
        self.emu.emu_start(start, until, count=2000000)
        return self.emu.reg_read(UC_X86_REG_EIP)

    # A call of the hooked function at `site`: the caller's stack (the
    # return address, then made-up arguments: zeroed objects), run to its
    # code past the site's bytes.
    def enter(self, site, back, sp, ret=SENTINEL, regs=GAME, flags=FLAGS, first=None):
        self.word(sp, ret)
        for i in range(1, 8):
            self.word(sp + 4 * i, ARGUMENTS + 0x1000 * i)
        if first is not None:
            self.word(sp + 4, first)
        self.stop_at(back)
        self.go(site, 0, regs, sp, flags)
        return self.stopped

    # The function's `ret n`, played: its result registers and flags, then
    # wherever the return address on the stack sends it, until `until`.
    def leave(self, sp_entry, pops, until=SENTINEL, regs=None, flags=0x0C5):
        target = self.read(sp_entry)
        regs = regs or {UC_X86_REG_EAX: 0x12345678, UC_X86_REG_ECX: 0x0CCC0CCC, UC_X86_REG_EDX: 0x0DDD0DDD,
                        UC_X86_REG_EBX: 0x0BBB0BBB, UC_X86_REG_ESI: 0x05550555, UC_X86_REG_EDI: 0x0EEE0EEE,
                        UC_X86_REG_EBP: 0x0019FE00}
        self.go(target, until, regs, sp_entry + 4 + pops, flags)
        return self.emu.reg_read(UC_X86_REG_EIP), regs, flags & FLAG_BITS

    def total(self, index):
        at = self.sym["frz_totals"] + 8 * index
        return self.read(at), self.read(at + 4)

    def slots(self, name):
        name = {"logic": "logic_timed", "message": "message_timed", "crc": "crc_timed"}.get(name, name)
        return self.sym[f"freeze_{name.replace(' ', '_')}_slots"]


def hook_cases(stock, patched, report):
    print("every hook: the function's first bytes and its return")
    backs = dict(zip([site for site, _, _, opcode in base.FREEZE_HOOKS if opcode == 0xE9], base.FREEZE_BACKS))
    window = STACK + 0x7000
    code = patched.linked.blocks["freeze.code"]
    for name, site, index, timed, pops in HOOKS:
        back = backs[site]
        sp = STACK + 0x8000
        stack_of = lambda run: bytes(run.emu.mem_read(window, 0x2000))
        for run in (stock, patched):
            run.emu.mem_write(window, bytes([PATTERN]) * 0x2000)
        stock.enter(site, back, sp)
        a = stock.state(), stack_of(stock), stock.stopped
        count0, us0 = patched.total(index)
        patched.tsc = 1_000_000_000
        patched.enter(site, back, sp)
        b = patched.state(), stack_of(patched), patched.stopped
        count1, _ = patched.total(index)
        at = sp - window
        leave = patched.read(sp)
        # The stack as the original leaves it: the caller's frame (the return
        # address but a timed function's, the arguments) and every byte the
        # original's own instructions wrote below it; what is below and
        # unwritten is no one's.
        kept = [i for i in range(len(a[1])) if (i >= at or a[1][i] != PATTERN) and not (timed and at <= i < at + 4)]
        same = all(b[1][i] == a[1][i] for i in kept)
        ok_ret = code.va <= leave < code.va + code.size if timed else leave == SENTINEL
        ok = a[2] == b[2] == back and a[0] == b[0] and same and ok_ret and count1 == count0 + 1
        why = "" if ok else (f"; stopped {a[2]}, {b[2]}; registers {a[0]} / {b[0]}; stack differs at "
                             f"{[hex(window + i) for i in kept if a[1][i] != b[1][i]][:12]}; "
                             f"return {leave:#x}; count {count0} -> {count1}")
        report(ok, f"{name} ({site:#x}): at {back:#x} with the registers, flags and stack of the original"
               + (", its return address the leave's" if timed else ", the return address the caller's")
               + ", counted" + why)
        # the function's own `ret n` (the present's leave is the first frame's end: the clock starts at T0)
        stock_ret = stock.leave(sp, pops)
        patched.tsc = T0
        eip, regs, flags = patched.leave(sp, pops)
        got = patched.state()
        _, us1 = patched.total(index)
        mult = patched.read(patched.sym["frz_tsc_mult"])
        want_us = (7_654_321 * mult) >> 32
        slot_free = not timed or patched.read(patched.slots(name)) == 0
        report(stock_ret[0] == eip == SENTINEL and got[0] == regs and got[1] == sp + 4 + pops and got[2] == flags
               and slot_free and us1 - us0 == (want_us if timed else 0),
               f"{name}: its `ret {pops}` comes back to the caller with the function's registers and flags, "
               f"{pops} bytes popped" + (f", the slot free, {us1 - us0} us added" if timed else ""))


def nested_case(patched, report):
    print("a timed function entered three times before it returns (callbacks, 0xE5AB0)")
    name, site, index = "callbacks", 0x0E5AB0, 11
    back = site + 6
    sps = (STACK + 0x9000, STACK + 0x8800, STACK + 0x8000)
    rets = (SENTINEL, SENTINEL + 0x100, SENTINEL + 0x200)
    count0, us0 = patched.total(index)
    entered = []
    for i, (sp, ret) in enumerate(zip(sps, rets)):
        patched.tsc = 5_000_000_000 + i * 3000
        patched.enter(site, back, sp, ret)
        entered.append(patched.read(sp))
    slots = patched.slots(name)
    taken = [patched.read(slots), patched.read(slots + 16)]
    out = []
    for i in (2, 1, 0):
        patched.tsc = 5_000_000_000 + 30000
        eip, regs, flags = patched.leave(sps[i], 0, rets[i])
        out.append(eip)
    count1, us1 = patched.total(index)
    mult = patched.read(patched.sym["frz_tsc_mult"])
    want = ((30000 - 3000) * mult >> 32) + (30000 * mult >> 32)
    report(entered[2] == rets[2] and entered[0] != rets[0] and entered[1] != rets[1] and taken == [1, 1]
           and out == [rets[2], rets[1], rets[0]] and count1 - count0 == 3 and us1 - us0 == want
           and patched.read(slots) == patched.read(slots + 16) == 0,
           "the first two take the two slots, the third is counted and not timed; each returns to its own "
           f"caller, both slots free after, {us1 - us0} us added (the two timed)")


class Game:
    """Made-up engine world, game and network client for the state."""

    def __init__(self, run):
        self.run = run
        self.on_tick = None

    def set(self, game=True, loading=0, scene=None, client="match", time_state=3, match=None):
        run = self.run
        run.emu.mem_write(WORLD, bytes(0x1000))
        run.emu.mem_write(GAME_OBJ, bytes(0x6000))
        run.emu.mem_write(CLIENT, bytes(16))
        run.word(CLIENT + CLIENT_STATE, time_state)
        run.word(CLIENT + 0x1F1C48, MATCH_CLIENT if match is not None else 0)
        run.word(MATCH_CLIENT + 0x577BC, match if match is not None else 0)
        run.word(WORLD + ENGINE_GAME, GAME_OBJ if game else 0)
        run.emu.mem_write(GAME_OBJ + GAME_LOADING, bytes([loading]))
        run.word(GAME_OBJ + GAME_LOBBY, LOBBY_MENU)
        run.word(GAME_OBJ + GAME_SCENE, {"world": GAME_OBJ + GAME_WORLD_AT, "lobby": LOBBY_MENU,
                                         "login": LOGIN_MENU, None: 0}[scene])
        run.word(GAME_OBJ + GAME_CLIENT, {"match": CLIENT, "replay": CLIENT, None: 0}[client])
        run.word(CLIENT, CLIENT_VTABLE if client == "match" else 0xA074BC)

    def tick(self):
        """One logic tick: logic_tick's entry with the world, its return."""
        run = self.run
        sp = STACK + 0x8000
        run.word(sp + 4, WORLD)
        run.word(sp, SENTINEL)
        run.stop_at(0x1A036)
        run.go(0x1A030, 0, GAME, sp)
        run.leave(sp, 4)
        state = run.read(run.sym["frz_state"])
        if self.on_tick:
            self.on_tick(state)
        return state


def state_cases(patched, report):
    print("the state (logic_tick's leave)")
    game = Game(patched)
    cases = (({"game": False}, S_MENU), ({"loading": 1, "scene": "lobby"}, S_LOADING),
             ({"loading": 1, "scene": "world"}, S_LOADING), ({"scene": "world", "time_state": 1}, S_WAITING),
             ({"scene": "world", "time_state": 2}, S_COUNTDOWN), ({"scene": "world", "time_state": 3}, S_PLAY),
             ({"scene": "world", "time_state": 0}, S_WAITING), ({"scene": "world", "client": "replay"}, S_REPLAY),
             ({"scene": "world", "client": None}, S_REPLAY), ({"scene": "lobby"}, S_LOBBY),
             ({"scene": "login"}, S_MENU), ({"scene": None}, S_MENU))
    for setting, want in cases:
        game.set(**setting)
        got = game.tick()
        report(got == want and patched.read(patched.sym["frz_world"]) == WORLD,
               f"{setting}: {STATE_NAMES[got] if got < len(STATE_NAMES) else got}")


def ms(us):
    return f"{us // 1000}.{us // 100 % 10}"


def frame_text(us, state, limit, counts, times, number=0, alive=0):
    out = f"#{number} {ms(us)} ms in {STATE_NAMES[state]} (limit {ms(limit)} ms): "
    parts = []
    fields = list(FIELDS)
    for k, (text, index, how) in enumerate(fields):
        value = alive if index == -1 else counts[index]
        closes = k + 1 == len(fields) or not fields[k + 1][0].startswith("+")
        if text.startswith("("):
            parts[-1] += f" {text} {value}" + (")" if closes else "")
        elif text.startswith("+"):
            parts[-1] += f", {text[1:]} {value}" + (")" if closes else "")
        elif how == "timed":
            parts.append(f"{text} {counts[index]} in {ms(times[index])} ms")
        elif how == "count":
            parts.append(f"{text} {counts[index]}")
        else:
            parts.append(f"{text} {ms(times[index])} ms")
    return out + ", ".join(parts)


class Frames:
    """Frames through the present's leave, with the clock and the counters scripted."""

    def __init__(self, run):
        self.run = run
        self.game = Game(run)
        self.hooks = {name: (site, index, timed, pops) for name, site, index, timed, pops in HOOKS}
        self.backs = dict(zip([site for site, _, _, opcode in base.FREEZE_HOOKS if opcode == 0xE9],
                              base.FREEZE_BACKS))
        self.n = 0
        # The reference: what each counter must grow by in the frame being made.
        self.time = 0                    # the next tick's time when a case names none
        self.latest, self.again = 0, False
        self.counts, self.times = [0] * COUNTERS, [0] * COUNTERS
        self.game.on_tick = self.logic_ticked
        # particle systems alive: started less removed, from the counts the cases before left
        self.alive = (run.total(12)[0] - run.total(25)[0]) & 0xFFFFFFFF

    def logic_ticked(self, state):
        self.counts[0] += 1
        if state < S_WAITING:
            self.latest = 0

    def event(self, name, us=0, time=None):
        """One call of a hooked function, taking `us` microseconds (timed ones); a tick to `time`, a
        message of the id `time`."""
        run = self.run
        site, index, timed, pops = self.hooks[name]
        sp = STACK + 0x8000
        regs = dict(GAME)
        first = None
        self.counts[index] += 1
        if name == "particle":
            self.alive += 1
        if name == "particle gone":
            self.alive -= 1
        if name == "message" and time is not None:
            first = time
            kind = {0xE8: WORLDS, 0xF6: ASKED, 0xEF: MOVES, 0xF0: MOVES, 0xF4: MOVES, 0xF1: DISCARDS,
                    0xF5: DISCARDS, 0xEB: RELAYS, 0xEC: RELAYS, 0xED: RELAYS, 0xEE: RELAYS, 0xF3: RELAYS}.get(time)
            if kind is not None:
                self.counts[kind] += 1
        if name == "tick":
            if time is None:
                self.time = max(self.time, self.latest) + 50
                time = self.time
            regs[UC_X86_REG_EDI] = time
            if time % 50:
                self.counts[SPLITS] += 1
            self.again = time <= self.latest
            if self.again:
                self.counts[RESIM_TICKS] += 1
            else:
                self.latest = time
        if name == "shot" and self.again:
            self.counts[RESIM_SHOTS] += 1
        run.enter(site, self.backs[site], sp, regs=regs, first=first)
        if timed:
            self.times[index] += us
            run.tsc += us * TSC_PER_US
            run.leave(sp, pops)

    def frame(self, us, events=(), present_us=0):
        """A frame of `us` microseconds by the clock (from the last present's end to this one's); the
        counter runs with the clock (T0 + 300 x QPC), the calls in it at its start, the present at its end."""
        run = self.run
        run.tsc = T0 + 300 * run.qpc
        for name, n, each, *time in events:
            for _ in range(n):
                self.event(name, each, *time)
        end = run.qpc + us * FREQUENCY // 1000000
        assert run.tsc <= T0 + 300 * end - present_us * TSC_PER_US
        run.tsc = T0 + 300 * end - present_us * TSC_PER_US
        site, index, timed, pops = self.hooks["present"]
        sp = STACK + 0x8000
        run.enter(site, self.backs[site], sp, regs={**GAME, UC_X86_REG_EAX: OBJECTS + 0x200000})
        run.tsc, run.qpc = T0 + 300 * end, end
        eip, _, _ = run.leave(sp, pops)
        assert eip == SENTINEL, hex(eip)
        self.counts[1] += 1
        self.times[1] += present_us
        self.n += 1
        self.last = self.counts, self.times, self.n, self.alive
        self.counts, self.times = [0] * COUNTERS, [0] * COUNTERS
        out, run.lines = run.lines, []
        return out

    def settle(self, us):
        """Frames of `us` until the median (made every 64 frames) is of them alone."""
        out = []
        for _ in range(64):
            out += self.frame(us)
        while self.n % 64:
            out += self.frame(us)
        return out


def frame_cases(patched, report):
    print("the frames (the present's leave)")
    frames = Frames(patched)
    game = frames.game
    game.set(scene="lobby")
    game.tick()
    logged = []                                  # the present's case above was the first frame: the clock's start
    for _ in range(101):
        logged += frames.frame(10000)
    mult = patched.read(patched.sym["frz_tsc_mult"])
    report(not logged and mult == round(2 ** 32 / TSC_PER_US),
           f"101 frames of 10 ms in the lobby: nothing logged; after the clock's second frz_tsc_mult is {mult} "
           f"(2^32 / {TSC_PER_US}, rounded)")
    logged += frames.settle(10000)
    game.set(scene="world", time_state=3)
    game.tick()
    # Many shooters: ticks to 1050 and 1100 with shots, a rollback, the two ticks again with the
    # shots replayed, one tick ahead; their hits, decals, impacts, sounds, a kill and its row.
    events = (("tick", 1, 0, 1050), ("shot", 2, 0), ("tick", 1, 0, 1100), ("shot", 3, 0), ("rollback", 1, 0),
              ("tick", 1, 0, 1050), ("shot", 2, 0), ("tick", 1, 0, 1100), ("shot", 3, 0), ("tick", 1, 0, 1150),
              ("shot", 1, 0), ("hit", 6, 0), ("decal", 4, 0), ("impact", 5, 0), ("sound", 9, 0), ("kill", 1, 0),
              ("kill row", 1, 0), ("tick", 1, 0, 1175), ("sim", 1, 2100), ("message", 2, 100, 0xEB),
              ("message", 1, 0, 0xEF), ("message", 1, 0, 0xE8), ("message", 1, 0, 0x40), ("datagram", 4, 0),
              ("bake", 1, 60000), ("cook", 2, 0), ("callbacks", 1, 6100), ("particle", 4, 0),
              ("particle gone", 1, 0), ("tick events", 1, 3000), ("render", 1, 4000), ("particle tick", 3, 200),
              ("sprites", 4, 100), ("streaming", 1, 300),
              ("invoke", 12, 0), ("advance", 2, 600), ("flash render", 1, 800))
    lines = frames.frame(83400, events, present_us=300)
    counts, times, number, alive = frames.last
    want = "srp freeze: " + frame_text(83400, S_PLAY, 40000, *frames.last)
    ok = lines == [(b"%s", b"game", 4, want.encode())] and counts[3] == 6 and counts[RESIM_TICKS] == 2 \
        and counts[16] == 11 and counts[RESIM_SHOTS] == 5 and counts[0] == 1 and counts[SPLITS] == 1 \
        and counts[RELAYS] == 2 and counts[MOVES] == 1 and counts[WORLDS] == 1
    report(ok, f"a frame of 83.4 ms in play: one line, {want!r}" + ("" if ok else f"; got {lines!r}"))
    lines = frames.frame(45000)
    want = "srp freeze: " + frame_text(45000, S_PLAY, 40000, *frames.last)
    report(lines == [(b"%s", b"game", 4, want.encode())],
           "the next frame, 45 ms with nothing in it: its own counters, all 0 but the present"
           + ("" if lines == [(b"%s", b"game", 4, want.encode())] else f"; got {lines!r}"))
    report(not frames.frame(39900) and not frames.frame(10000), "39.9 ms and 10 ms: not logged")
    patched.word(patched.sym["frz_limit_ms"], 0)
    report(not frames.frame(500000), "srp_freeze_ms 0: a frame of 500 ms is not logged")
    patched.word(patched.sym["frz_limit_ms"], 40)
    # three times the median: frames of 20 ms make the median 20 ms, the limit 60 ms
    frames.settle(20000)
    quiet = frames.frame(59000)
    loud = frames.frame(60000)
    want = "srp freeze: " + frame_text(60000, S_PLAY, 60000, *frames.last)
    report(not quiet and loud == [(b"%s", b"game", 4, want.encode())],
           "with a median of 20 ms the limit is 60 ms: 59 ms not logged, 60 ms logged")
    frames.settle(10000)
    game.set(scene="lobby")
    game.tick()
    frames.frame(10000)                           # the match ends: its summary, not checked here
    return frames


def percentile(frames, p):
    """The reference: the bucket edge (0.1 ms to 100 ms, 1 ms to 1 s) where p per mille are at or below."""
    def bucket(us):
        return us // 100 if us < 100000 else 1000 + (us - 100000) // 1000 if us < 1000000 else 1900
    def edge(b, most):
        us = (b + 1) * 100 if b < 1000 else 100000 + (b - 999) * 1000 if b < 1900 else most
        return min(us, most)
    counts = [0] * 1901
    for us in frames:
        counts[bucket(us)] += 1
    n = len(frames)
    want = n // 1000 * p + (n % 1000 * p + 999) // 1000
    seen = 0
    for b, c in enumerate(counts):
        seen += c
        if seen >= want and seen:
            return edge(b, max(frames))
    return max(frames)


def match_cases(patched, frames, report):
    print("a match: the summary at the lobby")
    game = frames.game
    game.set(scene="lobby")
    game.tick()
    frames.settle(10000)
    world, lines, longs = [], [], {}
    worst = []

    kept = {}

    def frame(us, state, events=()):
        nonlocal lines
        out = frames.frame(us, events)
        kept[us] = frames.last
        lines += out
        if state >= S_WAITING:
            world.append(us)
        if us >= 40000:
            longs[state] = longs.get(state, 0) + 1
        return out

    game.set(scene="lobby", loading=1)
    game.tick()
    for us in (900000, 250000, 10000, 1500000):
        frame(us, S_LOADING)
    game.set(scene="world", time_state=1)
    game.tick()
    for _ in range(30):
        frame(10000, S_WAITING)
    game.set(scene="world", time_state=2)
    game.tick()
    for i in range(200):
        frame(11000 + i * 10, S_COUNTDOWN)
    frame(55000, S_COUNTDOWN, (("bake", 1, 50000),))
    game.set(scene="world", time_state=3)
    game.tick()
    for i in range(2000):
        frame(9000 + (i * 37) % 4000, S_PLAY)
    frame(140200, S_PLAY, (("sync load", 2, 60000), ("cook", 3, 0)))
    frame(97000, S_PLAY, (("rollback", 1, 0), ("tick", 9, 0, 100), ("shot", 30, 0), ("sim", 1, 90000)))
    frame(41000, S_PLAY)
    frame(48000, S_PLAY, (("baked load", 1, 30000),))
    game.set(scene="lobby")
    game.tick()
    summary = frames.frame(10000)
    world_ms = sum(world) // 1000
    first = (f"srp freeze summary: {world_ms // 1000}.{world_ms // 100 % 10} s in the world, {len(world)} frames: "
             f"p50 {ms(percentile(world, 500))} ms, p95 {ms(percentile(world, 950))} ms, "
             f"p99 {ms(percentile(world, 990))} ms, max {ms(max(world))} ms")
    second = (f"srp freeze summary: {sum(longs.values())} long frames (srp_freeze_ms 40): loading "
              f"{longs.get(S_LOADING, 0)}, waiting {longs.get(S_WAITING, 0)}, countdown {longs.get(S_COUNTDOWN, 0)}, "
              f"play {longs.get(S_PLAY, 0)}, replay {longs.get(S_REPLAY, 0)}")
    got = [line[3].decode() for line in summary]
    report(len(got) == 5 and got[0] == first and got[1] == second,
           f"the summary's first lines: {first!r}, {second!r}" + ("" if got[:2] == [first, second] else f"; got {got!r}"))

    wants = [f"srp freeze summary: worst {i + 1}: {frame_text(us, state, 40000, *kept[us])}"
             for i, (us, state) in enumerate(((140200, S_PLAY), (97000, S_PLAY), (55000, S_COUNTDOWN)))]
    report(got[2:] == wants, "the three worst frames in the world with their counters, longest first"
           + ("" if got[2:] == wants else f"; got {got[2:]!r} want {wants!r}"))
    loading_lines = [l for l in lines if b" in loading " in l[3]]
    report(len(loading_lines) == 3 and len(lines) == sum(longs.values()),
           f"every long frame had its line ({len(lines)}), the loading's three too; the 10 ms of loading none")
    # The lines' limits: 100 a match (20 while loading), then one line that says so. Two
    # frames of 10 ms before each of 50 ms keep the median at 10 ms.
    game.set(scene="lobby", loading=1)
    game.tick()
    out = []
    for _ in range(25):
        out += frames.frame(10000) + frames.frame(10000) + frames.frame(50000)
    game.set(scene="world", time_state=3)
    game.tick()
    for _ in range(105):
        out += frames.frame(10000) + frames.frame(10000) + frames.frame(50000)
    texts = [l[3] for l in out]
    told = b"srp freeze: more long frames than are logged; the summary counts them all"
    report(sum(b" in loading " in t for t in texts) == 20 and sum(b" in play " in t for t in texts) == 100
           and texts.count(told) == 1 and len(texts) == 121,
           "25 long frames loading and 105 in play: 20 and 100 lines, then one line that says so")
    game.set(scene="lobby")
    game.tick()
    summary = frames.frame(10000)
    report(len(summary) >= 2 and summary[1][3].startswith(b"srp freeze summary: 130 long frames (srp_freeze_ms 40): "
                                                          b"loading 25, waiting 0, countdown 0, play 105"),
           "its summary counts all 130")


def call_cases(stock, patched, report):
    print("the calls made calls of a wrapper (the CRC-32 step, sort_particles)")
    data = bytes(range(256)) * 16 + b"freeze"
    for run in (stock, patched):                        # the game's own CRC table (0x1C210, as the sites call it)
        run.word(STACK + 0x7FF0, SENTINEL)
        run.go(0x1C210, SENTINEL, GAME, STACK + 0x7FF0)
    want_crc = 0
    for name, site, index, callee, pops in CALLS:
        sp = STACK + 0x8000
        outs = []
        for run in (stock, patched):
            run.emu.mem_write(TEXT + 0x2000, data)
            run.word(TEXT + 0x1F00, 0xFFFFFFFF)            # the CRC so far
            run.word(sp, len(data))                         # the site's pushed count (or argument)
            for i in range(1, 4):
                run.word(sp + 4 * i, ARGUMENTS + 0x1000 * i)
            regs = {**GAME, UC_X86_REG_EAX: TEXT + 0x1F00, UC_X86_REG_EDX: TEXT + 0x2000}
            if callee == SORT:                              # played: its `ret 12`, its registers
                run.sort_at = sp - 4
                run.stop_at(SORT)
                run.go(site, 0, regs, sp)
                assert run.stopped == SORT
                run.leave(sp - 4, pops, until=site + 5)
            else:
                run.go(site, site + 5, regs, sp)
            outs.append((run.state(), run.read(TEXT + 0x1F00), run.emu.reg_read(UC_X86_REG_EIP)))
        a, b = outs
        if callee == CRC:
            import zlib
            want_crc = zlib.crc32(data) ^ 0xFFFFFFFF
        if callee == LOG:
            a = (a[0], 0, a[2])                             # the logger's stub: the same in both
            b = (b[0], 0, b[2])
        report(a == b and a[2] == site + 5 and (callee != CRC or a[1] == want_crc),
               f"{name} at {site:#x}: back at {site + 5:#x} with the original's registers, flags, stack"
               + (f" and CRC {a[1]:#010x}" if callee == CRC else "") + ("" if a == b else f"; {a} / {b}"))
    report(patched.total(REFUSED)[0] == 6, "the six \"cannot discard\" lines counted as refused discards")
    count, _ = patched.total(32)
    report(count >= 5 and patched.total(CRC_BYTES)[0] >= 5 * len(data),
           f"the CRC-32 step counted ({count} calls) with its bytes ({patched.total(CRC_BYTES)[0]})")


def trace_text(q):
    """The trace's t of a QueryPerformanceCounter q (10 MHz, the first frame at 0): seconds, 6 places."""
    us = q // 10
    return f"{us // 1000000}.{us % 1000000:06d}"


def trace_line(number, q, us, state, flags, alive, counts, times):
    fields = [str(number), trace_text(q), str(us), str(state), flags, str(alive) if alive else ""]
    for i in range(COUNTERS):
        fields.append(str(counts[i]) if counts[i] else "")
        if i in TIMED:
            fields.append(str(times[i]) if times[i] else "")
    return ",".join(fields).encode()


def trace_cases(patched, frames, report):
    print("the trace file (Documents\\Survarium\\srp_frames.csv): every frame")
    run = patched
    text = b"".join(data for _, data in run.written)
    opened = run.opened
    path = FOLDER + b"\\srp_frames.csv"
    report(len(opened) == 1 and opened[0] == (path, 4, 3, 4, 0x80) and all(h == HANDLE for h, _ in run.written),
           f"opened once, {path.decode()!r}, to append (FILE_APPEND_DATA, OPEN_ALWAYS), shared")
    rows = text.split(b"\r\n")
    revision = run.read(0x380866, 1)[0]
    header = b"frame,t,frame_us,state,flags,alive," + b",".join(
        (f"{c}_n,{c}_us" if k in TIMED else f"{c}_n").encode() for k, c in enumerate(COLUMNS))
    first = f"# srp_frames 1; revision {revision}; started 2026-10-25 17:03:12.345 local; qpc 10000000 Hz; " \
            "srp_freeze_ms 40"
    report(rows[0] == first.encode() and rows[1].startswith(b"# state: 0 menu, 1 lobby")
           and rows[2].startswith(b"# flags: L long") and rows[3].startswith(b"# t: seconds") and rows[4] == header
           and text.count(b"# srp_frames") == 1,
           f"the header once: {first!r}, the states, the flags, the units, the columns")
    body = [r for r in rows[5:] if r and not r.startswith(b"#")]
    numbers = [int(r.split(b",")[0]) for r in body]
    width = 6 + COUNTERS + len(TIMED)
    report(numbers == list(range(1, len(numbers) + 1)) and all(len(r.split(b",")) == width for r in body),
           f"every frame, in order, each of {width} columns ({len(numbers)} so far)")
    # a known stretch: a match (id 4660) in play, shooting and a rollback, a refused discard, a world asked
    run.written.clear()
    frames.game.set(scene="world", time_state=3, match=0x80001234)
    frames.game.tick()
    start = frames.n
    lines = {}
    events = (("tick", 1, 0, 5050), ("shot", 3, 0), ("rollback", 1, 2000), ("tick", 1, 0, 5050), ("shot", 3, 0),
              ("message", 1, 0, 0xF6), ("kill", 1, 0), ("bake", 1, 20000))
    for k in range(12):
        us = 45000 if k == 5 else 10000
        frames.frame(us, events if k == 5 else ())
        counts, times, number, alive = frames.last
        flags = "LRBUK" if k == 5 else ""
        lines[number] = trace_line(number, run.qpc, us, S_PLAY, flags, alive, counts, times)
    frames.game.set(scene="lobby")
    frames.game.tick()
    for _ in range(120):                                # a second and more: written
        frames.frame(10000)
    rows = b"".join(d for _, d in run.written).split(b"\r\n")
    got = {int(r.split(b",")[0]): r for r in rows if r and not r.startswith(b"#")}
    match_lines = [r for r in rows if r.startswith(b"# match ")]
    report(all(got.get(n) == line for n, line in lines.items()),
           "twelve frames of a match, each line as the reference makes it (the long one: flags LRBUK, "
           "shots and replayed, the rollback's time, the bake's)"
           + ("" if all(got.get(n) == line for n, line in lines.items())
              else f"; {[(got.get(n), line) for n, line in lines.items() if got.get(n) != line][:1]}"))
    report(len(match_lines) == 2
           and match_lines[0].startswith(f"# match 4660 frame {start + 1} t ".encode())
           and match_lines[0].endswith(b" at 2026-10-25 17:03:12.345")
           and match_lines[1].startswith(f"# match 0 frame {start + 13} t ".encode()),
           f"the match's id at its first frame and 0 when it is over: {match_lines}")
    # the writes once a second
    run.written.clear()
    for _ in range(50):
        frames.frame(10000)
    early = len(run.written)
    for _ in range(60):
        frames.frame(10000)
    report(early <= 1 and len(run.written) >= 1, "written at most once a second, never more")
    run.word(run.sym["frz_trace_ms"], 0)
    run.written.clear()
    for _ in range(250):
        frames.frame(30000)
    report(not run.written, "srp_trace 0: nothing written")
    run.word(run.sym["frz_trace_ms"], 1)


def variable_cases(patched, report):
    print("the variable (game::game's call of the game world's constructor, 0x1E90A3)")
    patched.emu.mem_write(TEXT + 0x800, b"minimap_position\0")
    patched.emu.mem_write(OBJECTS + 0x380000, bytes(0x4C))
    patched.word(OBJECTS + 0x380000 + 0x10, TEXT + 0x800)
    patched.word(COMMANDS, OBJECTS + 0x380000)
    head = patched.read(COMMANDS)
    patched.stub_regs = {UC_X86_REG_EAX: 0x0A1A1A1A, UC_X86_REG_ECX: 0x0C1C1C1C, UC_X86_REG_EDX: 0x0D1D1D1D}
    patched.stop_at(GAME_WORLD_BACK)
    runs = []
    for _ in range(2):
        sp = STACK + 0x8000 - 12
        patched.word(sp, GAME_WORLD_BACK)                  # the call's own return; the site's two pushes
        patched.word(sp + 4, OBJECTS + 0x1018)
        patched.word(sp + 8, OBJECTS + 0x1000)
        patched.calls = []
        patched.go(GAME_WORLD_SITE, 0, {**GAME, UC_X86_REG_EAX: OBJECTS + 0x1018, UC_X86_REG_EBX: OBJECTS + 0x1000},
                   sp + 4)
        runs.append((patched.state(), list(patched.calls), patched.read(COMMANDS), patched.stopped))
    variable, value = patched.sym["frz_variable"], patched.sym["frz_limit_ms"]
    want_regs = {**GAME, UC_X86_REG_EBX: OBJECTS + 0x1000, **patched.stub_regs}
    report(all(r[3] == GAME_WORLD_BACK and r[0][0] == want_regs and r[0][1] == STACK + 0x8000 - 12 + 12
               and r[1] == [("game_world", OBJECTS + 0x1018, OBJECTS + 0x1018, OBJECTS + 0x1000)] for r in runs),
           "both calls reach the constructor with the world (eax and the first argument) and the game, and come "
           "back at 0x1e90a8 with its registers and both arguments popped")
    fields = patched.read(variable, 0x4C)
    word = lambda at: struct.unpack_from("<I", fields, at)[0]
    trace, trace_value = patched.sym["frz_trace_variable"], patched.sym["frz_trace_ms"]
    second = patched.read(trace, 0x4C)
    word2 = lambda at: struct.unpack_from("<I", second, at)[0]
    report(runs[0][2] == runs[1][2] == trace and word2(0xC) == variable and word(0) == CC_U32 and word(0xC) == head
           and patched.text(word(0x10)) == b"srp_freeze_ms" and word(0x40) == value and word(0x44) == 0
           and word(0x48) == 100000 and word(0x14) == 2 and fields[0x1D] == 1 and patched.read(value) == 40
           and word2(0) == CC_U32 and patched.text(word2(0x10)) == b"srp_trace" and word2(0x40) == trace_value
           and word2(0x44) == 0 and word2(0x48) == 1 and word2(0x14) == 2 and second[0x1D] == 1
           and patched.read(trace_value) == 1,
           f"two cc_u32 head the commands' list, once over two calls (the old head after them): srp_freeze_ms on "
           f"{value:#x} (40, 0 to 100000) and srp_trace on {trace_value:#x} (1, 0 to 1), saved, kind 2")

    def lookup(name):
        patched.emu.mem_write(TEXT, name + b"\0")
        sp = STACK + 0x8000 - 4
        patched.word(sp, SENTINEL)
        patched.go(LOOKUP, SENTINEL, {**GAME, UC_X86_REG_ESI: TEXT}, sp)
        return patched.emu.reg_read(UC_X86_REG_EAX)

    def execute(line):
        patched.emu.mem_write(TEXT, line + b"\0")
        sp = STACK + 0x8000 - 16
        for i, v in enumerate((SENTINEL, TEXT, 1, 2)):
            patched.word(sp + 4 * i, v)
        patched.go(EXECUTE, SENTINEL, GAME, sp)
        return patched.read(value)

    report(lookup(b"srp_freeze_ms") == variable and lookup(b"srp_trace") == patched.sym["frz_trace_variable"]
           and lookup(b"minimap_position") == head,
           "the game's lookup (0xEB7C0) finds it, and the command before it as before")
    report(execute(b"srp_freeze_ms 25") == 25 and execute(b"srp_freeze_ms 0") == 0
           and execute(b"srp_freeze_ms 100000") == 100000 and execute(b"srp_freeze_ms 40") == 40,
           "the game's execute (0xEB980, a user.cfg line) sets it: 25, 0, 100000, 40")


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    linked = base.arena()
    stock, patched = Run(original), Run(bytes(built), linked)
    stock.calls = patched.calls = []
    stock.stub_regs = patched.stub_regs = {}
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    report(len(HOOKS) == 29 and [site for _, site, _, _, _ in HOOKS]
           == [site for site, _, _, opcode in base.FREEZE_HOOKS if opcode == 0xE9],
           "the 29 entries of FREEZE_HOOKS, in order")
    # The clock's multiplier as the frames make it (frz_tsc_mult); the hooks' cases use it.
    patched.word(linked.symbols["frz_tsc_mult"], round(2 ** 32 / TSC_PER_US))
    hook_cases(stock, patched, report)
    patched.word(linked.symbols["frz_sim_time"], 0)            # the tick's case ticked to 0x0E0E0E0E
    patched.word(linked.symbols["frz_resim"], 0)
    nested_case(patched, report)
    patched.word(linked.symbols["frz_tsc_mult"], 0)
    state_cases(patched, report)
    frames = frame_cases(patched, report)
    match_cases(patched, frames, report)
    trace_cases(patched, frames, report)
    call_cases(stock, patched, report)
    variable_cases(patched, report)
    if failed:
        sys.exit(f"{failed} case(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
