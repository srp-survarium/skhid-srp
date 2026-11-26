#!/usr/bin/env python3
"""Run patch 210, the sync log (rust/patches/src/sync_log.rs, vostok/desync.rs;
../docs/notes/sync-log.md), in the default build beside the original exe.

    python check_sync_log_026g.py <original survarium.exe>

Requires Python `unicorn`. Its three sites run from their entry to the end of
their span (the hook, then the game's own displaced instructions), on a
made-up player (place 2), world core and history item, with kernel32's
GetEnvironmentVariableA, CreateFileA, WriteFile and CloseHandle stubs that
record the switch's reads and the lines, every other import a failure:

- `carry.input` (apply_new_input's entry 0x2D86E0, called by player::set_input),
  `sync.animations` (base_player::tick_animations' entry 0x2D8EE0) and
  `sync.world_load` (game_world_core::deserialize's entry 0x2D2770): after
  each, the registers and every byte of the made-up memory are the
  original's, the log on or off (it only reads);
- SKHID_SYNC_LOG unset: the switch is read once, and nothing is written;
- SKHID_SYNC_LOG=on: an input applied at 334697 ms is a line (its place, time,
  the player's last tick, set_input's caller, live or replay); one at 334700
  is not; tick_animations from apply_new_input at 334697 is a line, at 334700
  not; from base_player::tick to 334697 (a frame's part) not, from 334697 to
  334700 (a step that went on from a part) a line, naming the rollback load
  of 334600 before it; the server's world (0x2D0FB9's call) is a line with
  its time; the minimap's test of an enemy (0x239505) and its listing (0x23951B)
  write nothing at once (the map line is a second's, at the frame); a second of game time writes 20 lines at most, and says how
  many it held back.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

ENVIRONMENT, CREATE_FILE, WRITE_FILE, CLOSE_HANDLE, THREAD_SLOT = 0x8701B4, 0x870048, 0x870108, 0x870040, 0x870118
ENGINE_TSV = "engine_026g.tsv"
INPUT, ANIMATIONS, LOAD = (0x2D86E0, 6), (0x2D8EE0, 6), (0x2D2770, 8)
MINIMAP_TEST, MINIMAP_LISTED = (0x239505, 6), (0x23951B, 6)
SET_INPUT_CALLED, SET_INPUT_CALLER = 0x1F3731, 0x2D1F2A
APPLY_CALLED, TICK_CALLED = 0x2D8A06, 0x2DAAE0
ROLLBACK, SERVER = 0x2D2FF5, 0x2D0FBE
DATA, CORE, ITEM, STACK, STUBS = 0x30000000, 0x32000000, 0x34000000, 0x21000000, 0x31000000
PLAYER, PACKET = DATA, DATA + 0x8000
IMPORT, FORBIDDEN, THREAD = STUBS, STUBS + 0x40, STUBS + 0x50
SP = STACK + 0x8000
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP, UC_X86_REG_ESP)
MAPPED = ((DATA, 0x10000), (CORE, 0x20000), (ITEM, 0xC1000), (STACK, 0x10000))
# compared after a site: the made-up objects and the stack from the entry's ESP up (below it the
# hook's own frame)
MEMORY = ((DATA, 0x10000), (CORE, 0x20000), (ITEM, 0xC1000), (SP, 0x8000))


class Run:
    def __init__(self, data, switch=None):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in MAPPED + ((STUBS, 0x1000),):
            emu.mem_map(at, size, UC_PROT_ALL)
        emu.mem_write(THREAD, bytes.fromhex("b801000000c3"))     # GetCurrentThreadId: mov eax,1 / ret
        self.switch, self.environment, self.lines, self.forbidden = switch, 0, [], []
        for line in open(ENGINE_TSV, encoding="utf-8"):
            cells = line.rstrip("\n").split("\t")
            if len(cells) > 4 and cells[4] == "import":
                self.word(int(cells[1], 16), FORBIDDEN)
        for slot, stub in ((ENVIRONMENT, IMPORT), (CREATE_FILE, IMPORT + 4), (WRITE_FILE, IMPORT + 8),
                           (CLOSE_HANDLE, IMPORT + 12), (THREAD_SLOT, THREAD)):
            self.word(slot, stub)
        emu.hook_add(UC_HOOK_CODE, self.code, begin=STUBS, end=STUBS + 0x4F)
        self.word(PLAYER + 0x114, 2)                             # place 2 (a byte; the rest zero)
        self.word(PLAYER + 0x120, CORE)                          # its world core
        self.emu.mem_write(PLAYER + 0x33C, b"\x01")              # alive
        self.word(CORE + 0x18FBC, 50)                            # the tick
        self.word(PLAYER + 0x348, 0)                             # not spotted
        self.word(PLAYER + 0x10C, DATA + 0x2000)                 # an inventory (the listing reads its +178h)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", bytes(self.emu.mem_read(at, 4)))[0]

    def code(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 * i) for i in range(1, 6)]
        if address == IMPORT:                                    # GetEnvironmentVariableA(name, buffer, size)
            name = bytes(emu.mem_read(args[0], 32)).split(b"\0")[0]
            self.environment += name == b"SKHID_SYNC_LOG"
            value = self.switch if name == b"SKHID_SYNC_LOG" else None
            if value is None:
                self.ret(12, 0)
            else:
                emu.mem_write(args[1], value + b"\0")
                self.ret(12, len(value))
        elif address == IMPORT + 4:                              # CreateFileA: 7 arguments, a handle
            self.ret(28, 0x1234)
        elif address == IMPORT + 8:                              # WriteFile(file, bytes, size, &written, 0)
            self.lines.append(bytes(emu.mem_read(args[1], args[2])).decode())
            self.word(args[3], args[2])
            self.ret(20, 1)
        elif address == IMPORT + 12:                             # CloseHandle
            self.ret(4, 1)
        elif address == FORBIDDEN:
            self.forbidden.append(hex(self.read(sp)))
            emu.emu_stop()

    def ret(self, pops, eax):
        emu = self.emu
        sp = emu.reg_read(UC_X86_REG_ESP)
        emu.reg_write(UC_X86_REG_EAX, eax)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def site(self, site, stack, registers):
        """The code from the site's entry to the end of its span: the registers after it and
        the made-up memory, and the lines it wrote."""
        begin, span = site
        emu = self.emu
        self.lines = []
        emu.mem_write(SP, b"".join(struct.pack("<I", word) for word in stack))
        for reg in REGS:
            emu.reg_write(reg, 0x5A5A5A5A)
        emu.reg_write(UC_X86_REG_EBP, 0x0019FF00)
        for reg, value in registers.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, SP)
        emu.emu_start(begin, begin + span, count=2_000_000)
        assert not self.forbidden, f"an import called: {self.forbidden}"
        assert emu.reg_read(UC_X86_REG_EIP) == begin + span, hex(emu.reg_read(UC_X86_REG_EIP))
        after = {reg: emu.reg_read(reg) for reg in REGS}
        memory = b"".join(bytes(emu.mem_read(at, size)) for at, size in MEMORY)
        return after, memory, [line.rstrip("\r\n") for line in self.lines]

    def input(self, time, previous, newest):
        self.word(PLAYER + 0x324, previous)
        self.word(CORE + 0x18FC4, newest)
        return self.site(INPUT, [SET_INPUT_CALLED, time, PACKET, 0x0E5E5E5E, SET_INPUT_CALLER],
                         {UC_X86_REG_ECX: PLAYER})

    def tick(self, caller, previous, now, newest):
        self.word(CORE + 0x18FC4, newest)
        return self.site(ANIMATIONS, [caller, previous, now], {UC_X86_REG_EAX: PLAYER})

    def load(self, caller, time):
        self.word(ITEM + 0xC0BB0, time)
        return self.site(LOAD, [caller, CORE, ITEM, 8], {})


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    failed = 0

    def case(what, ok, detail=""):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {what}" + (f"\n      {detail}" if not ok and detail else ""))

    # Every call, the same in the original, the build with the log off and with it on.
    calls = [("an input at 334697", lambda r: r.input(334697, 334650, 334650)),
             ("an input at 334700", lambda r: r.input(334700, 334650, 334650)),
             ("a rollback's load of 334600", lambda r: r.load(ROLLBACK, 334600)),
             ("an input's tick at 334697", lambda r: r.tick(APPLY_CALLED, 334650, 334697, 334650)),
             ("an input's tick at 334700", lambda r: r.tick(APPLY_CALLED, 334650, 334700, 334650)),
             ("a step to a frame's part, 334697", lambda r: r.tick(TICK_CALLED, 334650, 334697, 334650)),
             ("a step from 334697 to 334700, replayed", lambda r: r.tick(TICK_CALLED, 334697, 334700, 334750)),
             ("the server's world of 92250", lambda r: r.load(SERVER, 92250)),
             ("the minimap's test of an enemy", lambda r: r.site(MINIMAP_TEST, [], {UC_X86_REG_EAX: PLAYER, UC_X86_REG_ESI: 0})),
             ("the minimap's listing", lambda r: r.site(MINIMAP_LISTED, [], {UC_X86_REG_EAX: PLAYER}))]
    stock, off, on = Run(original), Run(bytes(built)), Run(bytes(built), b"on")
    written = []
    for what, call in calls:
        (want, want_memory, _), (got_off, off_memory, off_lines), (got_on, on_memory, on_lines) = \
            call(stock), call(off), call(on)
        case(f"{what}: registers and memory as the original's, off and on",
             want == got_off == got_on and want_memory == off_memory == on_memory)
        case(f"{what}: off, nothing written", not off_lines, off_lines)
        written.append(on_lines)
    case("off: the switch read once", off.environment == 1, off.environment)
    case("on: the switch read once", on.environment == 1, on.environment)

    def lines(i):
        return [line.split("srp: ", 1)[-1] for line in written[i]]

    expect = [
        ["sync log: on (SKHID_SYNC_LOG)",
         "sync: input p2 t334697 prev 334650 via set_input from 0x002d1f2a live"],
        [],
        [],
        ["sync: tick p2 t334697 prev 334650 by input live"],
        [],
        [],
        ["sync: tick p2 t334700 prev 334697 by step replay (newest 334750) after a load of 334600 by rollback"],
        ["sync: the server's world loaded (a join or a resync), its time 92250"],
        [],
        [],
    ]
    for (what, _), want, i in zip(calls, expect, range(len(calls))):
        case(f"on: {what}: {len(want)} line(s)", lines(i) == want, (written[i], want))

    # The budget: 25 inputs in second 400 write 20; the next second's first says what was held.
    for i in range(25):
        on.input(400_001 + i, 400_000, 400_000)
    _, _, first = on.input(401_003, 401_000, 401_000)
    first = [line.split("srp: ", 1)[-1] for line in first]
    case("on: a second writes 20 lines, the next says 5 held back",
         first[:1] == ["sync: 5 lines held back in second 400"] and first[1].startswith("sync: input p2 t401003"),
         first)
    print("all cases passed" if not failed else f"{failed} FAILED")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
