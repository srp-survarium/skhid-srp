#!/usr/bin/env python3
"""Run patch 79's entries (netcode_026g.asm, rust/netcode: the per-tick
protocol's client) in the default build on made-up messages, senders and
send passes.

    python check_netcode_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are; the
game's own code runs from the hooked sites: the dispatch
on_match_packet_received (0x27A300, called as the game calls it), the
length getter 0x279670, the two infos 0x279A60 and 0x279B10, the connect's
send site 0x27A247, the two senders' new_packet sites 0x27EB21 and
0x27ECAC, the send pass's synchronize site 0x27CD1F. Stubbed: the dispatch's
table jump (0x27A330: each message that reaches the stock handlers is
written down, consumed, and the dispatch goes on at its end 0x27A6FC),
synchronize (0x27E6D0), new_packet (0x279FE0), buffer_writer::w (0x7AC10),
enqueue (0x27A2A0), the connect's send (0x379090) and the log's kernel32
calls. The client, its match core and link are made-up memory at the
offsets game.rs reads.

Checked:
- the tables: 0xE2's sent info is 0xDE's (reliable, plain, again every 50
  ms); 0xF7's received info is 0xF6's with 500 ms to live and 50 ms
  resends; both are variable (0xF6's case of the length getter); every
  stock id's info and length as the original's;
- 0xD8's capability byte gets bit 1 and the send goes on as the original's;
- the static info: with the trailer, the stock handlers get the message
  without it; without, unchanged; any other id reaches them as it came;
- 0xF7: with the stock protocol nothing reaches the handlers; with the
  per-tick one the stock messages of its changes do, in order (outcomes,
  leaves, entries, others' inputs and kit choices, the clock), each as
  the stock server would have sent it (computed here, not by the Rust
  code); the reader consumed, the dispatch returns as it does;
- the own input and kit choice: taken (on to the enqueue's end, the stack
  as it was) only on a match of the per-tick protocol and for the own
  place; else the stock new_packet, as the original;
- the send pass: synchronize called with the original's arguments, then
  one 0xE2 a tick with the inputs taken (its bytes computed here), its
  packet not joined, AL 1 when one went; nothing with the stock protocol;
- the clock: late updates make one 0xE7 of the steering's size;
- every entry keeps EBX ESI EDI EBP (and the frame), and enters Rust with
  the stack aligned to 16.
"""
import struct
import sys

from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_EFLAGS

import patch_026g as base

DISPATCH, TABLE_JUMP, DISPATCH_END = 0x27A300, 0x27A330, 0x27A6FC
LENGTH, SEND_INFO, RECEIVED_INFO = 0x279670, 0x279A60, 0x279B10
CONNECT_SITE, CONNECT_SEND = 0x27A247, 0x379090
INPUT_SITE, INPUT_ON, INPUT_SKIP = 0x27EB21, 0x27EB2A, 0x27EB4D
PROFILE_SITE, PROFILE_ON, PROFILE_SKIP = 0x27ECAC, 0x27ECB4, 0x27ECD2
SYNC_SITE, SYNCHRONIZE = 0x27CD1F, 0x27E6D0
NEW_PACKET, WRITE, ENQUEUE = 0x279FE0, 0x07AC10, 0x27A2A0
IAT = {0x870048: "CreateFileA", 0x870108: "WriteFile", 0x870040: "CloseHandle"}
JOURNAL = 0xA8394C                      # the journal's recorder (0: none)

CLIENT, CORE, LINK, PACKET = 0x30000000, 0x30400000, 0x30500000, 0x30600000
READER, MESSAGE, SENDING = 0x30700000, 0x30701000, 0x30800000
STACK, SENTINEL, STUBS = 0x31000000, 0x32000000, 0x33000000
STATE, MATCH_CORE, LINK_FIELD = 0x1F1C64, 0x1F1C40, 0x1F1C48
WORLD = 0x1F8
NOW, RESYNCED, OWNER, FROZEN = WORLD + 0x18FC0, WORLD + 0x18FD4, WORLD + 0x18FDC, WORLD + 0x18FDF
POOL_IN_USE, POOL_CAPACITY = 0x578B0 + 0x30, 0x578B0 + 0x34
WRITER, FLAGS = 0x2A8, 0x98
KEPT = {UC_X86_REG_EBX: 0x11110000, UC_X86_REG_ESI: 0x22220000, UC_X86_REG_EDI: 0x33330000,
        UC_X86_REG_EBP: 0x44440000}
TICK = 50
TRAILER = bytes([TICK, 1]) + b"SRPT"
OWN = 1


def u32(value):
    return struct.pack("<I", value & 0xFFFFFFFF)


class Game:
    def __init__(self, data, rust_entries=()):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((CLIENT, 0x200000), (CORE, 0x20000), (LINK, 0x60000), (PACKET, 0x1000),
                         (READER, 0x2000), (SENDING, 0x1000), (STACK, 0x10000), (SENTINEL, 0x1000),
                         (STUBS, 0x1000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        self.w32(JOURNAL, 0)
        self.stubs = {TABLE_JUMP: self.table_jump, CONNECT_SEND: self.connect_send,
                      SYNCHRONIZE: self.synchronize, NEW_PACKET: self.new_packet, WRITE: self.write,
                      ENQUEUE: self.enqueue}
        for n, (slot, name) in enumerate(IAT.items()):
            self.w32(slot, STUBS + 0x10 * n)
            self.stubs[STUBS + 0x10 * n] = getattr(self, name)
        self.stops = set()
        self.entries = set(rust_entries)
        self.aligned = []
        self.reset()
        emu.hook_add(UC_HOOK_CODE, self.hook)

    def reset(self):
        self.handled, self.sent, self.log, self.connects, self.syncs = [], [], [], [], []
        self.stopped = None
        self.packet_id, self.packet_bytes = None, bytearray()

    # memory
    def w32(self, at, value):
        self.emu.mem_write(at, u32(value))

    def r32(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def reg(self, r):
        return self.emu.reg_read(r)

    def arg(self, n):
        return self.r32(self.reg(UC_X86_REG_ESP) + 4 * n)

    def ret(self, pops, eax=None):
        sp = self.reg(UC_X86_REG_ESP)
        if eax is not None:
            self.emu.reg_write(UC_X86_REG_EAX, eax & 0xFFFFFFFF)
        self.emu.reg_write(UC_X86_REG_EIP, self.r32(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)

    def hook(self, emu, address, size, _):
        if address in self.entries:
            self.aligned.append((address, (self.reg(UC_X86_REG_ESP) + 4) % 16))
        if address in self.stops:
            self.stopped = address
            emu.emu_stop()
            return
        stub = self.stubs.get(address)
        if stub:
            stub()

    # the stubs
    def table_jump(self):
        """The dispatch's table: a stock handler takes the message."""
        reader = self.reg(UC_X86_REG_EBX)
        base_, pos, size = self.r32(reader), self.r32(reader + 4), self.r32(reader + 8)
        payload = bytes(self.emu.mem_read(pos, base_ + size - pos))
        self.handled.append(((self.reg(UC_X86_REG_EAX) + 0xE3) & 0xFF, payload))
        self.w32(reader + 4, base_ + size)
        self.emu.reg_write(UC_X86_REG_EIP, DISPATCH_END)

    def connect_send(self):
        packet, offset = self.reg(UC_X86_REG_ESI), self.reg(UC_X86_REG_EDI)
        data = self.r32(self.r32(packet + 0x2BC) + 4)
        self.connects.append((self.emu.mem_read(data + offset, 1)[0], self.r32(self.reg(UC_X86_REG_ESP))))
        self.ret(0)

    def synchronize(self):
        self.syncs.append((self.arg(1), self.arg(2), self.arg(3), self.reg(UC_X86_REG_EAX),
                           self.reg(UC_X86_REG_EBX)))
        self.ret(12, 0)

    def new_packet(self):
        assert self.reg(UC_X86_REG_ECX) == LINK, "new_packet on another object"
        self.packet_id = self.arg(1)
        self.packet_bytes = bytearray()
        self.emu.mem_write(PACKET, b"\xFF" * 0x400)
        self.ret(4, PACKET)

    def write(self):
        writer, source, n = self.arg(1), self.arg(2), self.reg(UC_X86_REG_EDI)
        assert writer == PACKET + WRITER, "buffer_writer::w on another writer"
        self.packet_bytes += bytes(self.emu.mem_read(source, n))
        self.ret(8)

    def enqueue(self):
        assert self.reg(UC_X86_REG_ECX) == LINK and self.arg(1) == PACKET, "enqueue of another packet"
        self.sent.append((self.packet_id, bytes(self.packet_bytes), self.emu.mem_read(PACKET + FLAGS, 1)[0]))
        self.ret(4)

    def CreateFileA(self):
        self.ret(28, 7)

    def WriteFile(self):
        self.log.append(bytes(self.emu.mem_read(self.arg(2), self.arg(3))).decode())
        self.ret(20, 1)

    def CloseHandle(self):
        self.ret(4, 1)


    # the game's state
    def world(self, state=2, core=True, now=1200, resynced=0, owner=OWN, frozen=0):
        self.w32(CLIENT + STATE, state)
        self.w32(CLIENT + MATCH_CORE, CORE if core else 0)
        self.w32(CLIENT + LINK_FIELD, LINK)
        self.w32(CORE + NOW, now)
        self.w32(CORE + RESYNCED, resynced)
        self.emu.mem_write(CORE + OWNER, bytes([owner]))
        self.emu.mem_write(CORE + FROZEN, bytes([frozen]))
        self.w32(LINK + POOL_CAPACITY, 256)
        self.w32(LINK + POOL_IN_USE, 3)

    def run(self, start, sp, regs, stops=()):
        self.stops = set(stops)
        self.stopped = None
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        for r, v in {**KEPT, **regs}.items():
            self.emu.reg_write(r, v)
        self.emu.emu_start(start, SENTINEL, count=2_000_000)

    def kept(self, sp):
        return all(self.reg(r) == v for r, v in KEPT.items()) and self.reg(UC_X86_REG_ESP) == sp

    # what the game does
    def receive(self, id, payload):
        """on_match_packet_received(id, reader) as the match client calls it."""
        self.handled = []
        self.emu.mem_write(MESSAGE, payload)
        self.w32(READER, MESSAGE)
        self.w32(READER + 4, MESSAGE)
        self.w32(READER + 8, len(payload))
        sp = STACK + 0x8000
        self.w32(sp, SENTINEL)
        self.w32(sp + 4, id)
        self.w32(sp + 8, READER)
        self.run(DISPATCH, sp, {UC_X86_REG_ECX: CLIENT})
        consumed = self.r32(READER + 4) == MESSAGE + len(payload) - (len(TRAILER) if id == 0xE4 and
                                                                      payload.endswith(b"SRPT") else 0)
        return self.handled, consumed and self.kept(sp + 12)

    def length(self, id, left):
        sp = STACK + 0x8000
        # (id, the bytes left, the area's start and end: left = end - start
        # + left; the message's bytes, zeros, at both)
        self.emu.mem_write(MESSAGE, bytes(0x100))
        for i, v in enumerate((SENTINEL, id, left, MESSAGE, MESSAGE, 0, STACK + 0x9000)):
            self.w32(sp + 4 * i, v)
        try:
            self.run(LENGTH, sp, {UC_X86_REG_ECX: LINK})
        except UcError as error:            # an id whose length reads the roster (0xED, 0xEE)
            return "fault", str(error)
        return self.reg(UC_X86_REG_EAX), self.emu.mem_read(STACK + 0x9000, 1)[0]

    def info(self, function, id):
        sp = STACK + 0x8000
        self.w32(sp, SENTINEL)
        out = STACK + 0x9000
        self.emu.mem_write(out, b"\xEE" * 8)
        self.run(function, sp, {UC_X86_REG_ECX: id, UC_X86_REG_EAX: out})
        return struct.unpack("<IHB", self.emu.mem_read(out, 7))

    def connect(self):
        self.connects = []
        buffer = PACKET + 0x400
        self.w32(PACKET + 0x2BC, buffer)
        self.w32(buffer + 4, buffer + 0x100)
        self.emu.mem_write(buffer + 0x100, bytes([0xD8, 1, 2, 3, 4, 0x80]))
        sp = STACK + 0x8000
        self.w32(sp, 0x5A5A5A5A)
        self.run(CONNECT_SITE, sp, {UC_X86_REG_ESI: PACKET, UC_X86_REG_EDI: 5}, stops={CONNECT_SITE + 5})
        kept = {**KEPT, UC_X86_REG_ESI: PACKET, UC_X86_REG_EDI: 5}
        return self.connects, all(self.reg(r) == v for r, v in kept.items()) and self.reg(UC_X86_REG_ESP) == sp

    def sender(self, site, on, skip, slots, stock_push):
        """A sender at its new_packet: its frame's slots (offset: value),
        EBP its context (+4 the world core), EDX the link's vtable, EBX 1.
        Its outcome ("stock": on to new_packet, "taken": on at the
        enqueue's end) and whether the frame and registers are as that
        path has them (the stock one: EAX new_packet and the id pushed or
        in EBX)."""
        sp = STACK + 0x8000
        for offset, value in slots.items():
            self.emu.mem_write(sp + offset, value)
        self.w32(SENDING + 4, CORE + WORLD)
        vtable = STUBS + 0x800
        self.w32(vtable + 0x18, NEW_PACKET)
        regs = {UC_X86_REG_EBP: SENDING, UC_X86_REG_EDX: vtable, UC_X86_REG_EBX: 1}
        self.run(site, sp, regs, stops={on, skip})
        others = self.reg(UC_X86_REG_EBP) == SENDING and self.reg(UC_X86_REG_ESI) == KEPT[UC_X86_REG_ESI] \
            and self.reg(UC_X86_REG_EDI) == KEPT[UC_X86_REG_EDI] and self.reg(UC_X86_REG_EDX) == vtable
        esp, ebx = self.reg(UC_X86_REG_ESP), self.reg(UC_X86_REG_EBX)
        if self.stopped == on:
            if stock_push:
                frame = esp == sp - 4 and self.r32(esp) == stock_push and ebx == 1
            else:
                frame = esp == sp and ebx == 0xDD
            return "stock", others and frame and self.reg(UC_X86_REG_EAX) == NEW_PACKET
        return ("taken" if self.stopped == skip else "lost"), others and esp == sp and ebx == 1

    def own_input(self, time, rotation, actions, place, flag):
        slots = {0x20: u32(rotation), 0x24: u32(actions), 0x28: u32(time), 0x2C: bytes([place, flag, 0x77, 0x77])}
        return self.sender(INPUT_SITE, INPUT_ON, INPUT_SKIP, slots, None)

    def own_profile(self, time, value, place):
        slots = {0x1C: u32(value | 0x5A5A5A00), 0x20: u32(time), 0x24: u32(place | 0x5A5A5A00)}
        return self.sender(PROFILE_SITE, PROFILE_ON, PROFILE_SKIP, slots, 0xDE)

    def send_pass(self):
        """send_match_messages at its synchronize: [esp] 0x27CD24 after the
        call, its three arguments pushed."""
        self.sent, self.syncs = [], []
        sp = STACK + 0x8000
        for i, v in enumerate((0xC0DE0001, 0xC0DE0002, 0xC0DE0003)):
            self.w32(sp + 4 * i, v)
        regs = {UC_X86_REG_EBX: CLIENT, UC_X86_REG_EAX: 0xC0DE0004}
        kept = {**KEPT, UC_X86_REG_EBX: CLIENT}
        self.run(SYNC_SITE, sp, {**regs}, stops={SYNC_SITE + 5})
        ok = all(self.reg(r) == v for r, v in kept.items() if r != UC_X86_REG_EBX) \
            and self.reg(UC_X86_REG_EBX) == CLIENT and self.reg(UC_X86_REG_ESP) == sp + 12
        return self.sent, self.syncs, self.reg(UC_X86_REG_EAX) & 0xFF, ok


# The messages as the stock server sent them (match.md section 1.2).
def tick_byte(time):
    return (time // TICK) & 0xFF


def at(time):
    """A time: its tick byte, and the ms into it if it is off the grid."""
    return bytes([tick_byte(time)]) + (bytes([time % TICK]) if time % TICK else b"")


def relay(place, time, rotation, actions):
    return (0xEC if time % TICK else 0xEB, at(time) + bytes([place]) + u32(rotation) + u32(actions))


def moved(source, to):
    precise = source % TICK or to % TICK
    pick = (lambda t: bytes([tick_byte(t), t % TICK])) if precise else (lambda t: bytes([tick_byte(t)]))
    return (0xF0 if precise else 0xEF, pick(source) + pick(to))


def server_update(tick, entered=(), left=(), arrival=None, inputs=(), profiles=(), outcomes=()):
    """An 0xF7 (sync_core::wire::ServerUpdate), written here from its layout."""
    mask = lambda places: sum(1 << p for p in places)
    out = bytes([1]) + u32(tick) + u32(mask(entered)) + u32(mask(left))
    out += b"\0" if arrival is None else b"\1" + struct.pack("<h", arrival)
    out += bytes([len(inputs), len(profiles), len(outcomes)])
    for place, back, rotation, actions in inputs:
        out += bytes([place, back]) + u32(rotation) + u32(actions)
    for place, value in profiles:
        out += bytes([place, value])
    for update, kind, to in outcomes:
        out += u32(update) + bytes([kind]) + (u32(to) if kind == 1 else b"")
    return out


def client_update(tick, inputs=(), profiles=()):
    out = bytes([1]) + u32(tick) + bytes([len(inputs), len(profiles)])
    for back, rotation, actions in inputs:
        out += bytes([back]) + u32(rotation) + u32(actions)
    return out + bytes(profiles)


ROT, ROT2 = 0x3F803C00, 0x00003C00          # sane rotations (halves under 1024)
INSANE = 0x7C003C00                         # a half's exponent at 1024 and over


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    linked = base.arena()
    rust = {linked.symbols[s] for s in ("netcode_update", "netcode_static_info", "netcode_own_input_hook",
                                        "netcode_own_profile_hook", "netcode_send")}
    stock, game = Game(original), Game(bytes(built), rust)
    failed = 0

    def check(ok, what):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {what}")

    # The tables.
    infos = lambda g, f, ids: {i: g.info(f, i) for i in ids}
    sent_before, sent = infos(stock, SEND_INFO, range(0xD8, 0xE2)), infos(game, SEND_INFO, range(0xD8, 0xE3))
    got_before, got = infos(stock, RECEIVED_INFO, range(0xE3, 0xF7)), infos(game, RECEIVED_INFO, range(0xE3, 0xF8))
    check(all(sent[i] == sent_before[i] for i in sent_before) and all(got[i] == got_before[i] for i in got_before),
          "every stock id's sent and received info is the original's")
    check(sent[0xE2] == sent_before[0xDE] and sent_before[0xDE][1] == 50,
          f"0xE2's sent info is 0xDE's (lifetime {sent[0xE2][0]:#x}, resend {sent[0xE2][1]}, flags {sent[0xE2][2]:#x})")
    check(got[0xF7] == (500, 50, got_before[0xF6][2]) and got_before[0xE8][0] == 1750,
          f"0xF7's received info: 500 ms to live (0xE8's 1750 is the same field), 50 ms resends, 0xF6's flags "
          f"{got_before[0xF6][2]:#x}")
    lengths = {i: game.length(i, 7) for i in (0xE2, 0xF7)}
    check(lengths[0xE2] == lengths[0xF7] == stock.length(0xF6, 7) == (0xFFFF, 1)
          and game.length(0xF7, 0)[1] == stock.length(0xF6, 0)[1] == 0
          and all(game.length(i, 7) == stock.length(i, 7) for i in range(0xD8, 0xF7) if i != 0xE2),
          "0xE2 and 0xF7 are variable (0xF6's case: 0xFFFF, a byte at least; the original's table has no 0xE2: "
          "a jump to nowhere); every other id's length the original's")

    # 0xD8.
    connects, kept = game.connect()
    connects_before, kept_before = stock.connect()
    check(connects == [(0x82, CONNECT_SITE + 5)] and connects_before == [(0x80, CONNECT_SITE + 5)] and kept
          and kept_before,
          "0xD8's capability byte 0x80 goes as 0x82 (bit 1: the per-tick protocol), the send called as the original's")

    # The static info, and the routing of the others.
    for g in (stock, game):
        g.world(state=1)
    info = bytes([3]) + bytes(range(40))
    seen, ok = game.receive(0xE4, info)
    seen_before, _ = stock.receive(0xE4, info)
    check(seen == seen_before == [(0xE4, info)] and ok, "a static info without the trailer reaches the handlers whole")
    seen, ok = game.receive(0xE4, info + TRAILER)
    check(seen == [(0xE4, info)] and ok, "with the trailer: the handlers get it without")
    game.world()
    for id, payload in ((0xE9, bytes([24, 3])), (0xE7, struct.pack("<h", -20)), (0xEB, bytes(10)), (0xE5, bytes(7))):
        seen, ok = game.receive(id, payload)
        check(seen == [(id, payload)] and ok, f"{id:#x} reaches its handler as it came")

    # 0xF7 with the per-tick protocol.
    update = server_update(1200, entered=[4], left=[2],
                           inputs=[(0, 5, ROT, 7), (OWN, 0, ROT, 8), (3, 0, INSANE, 9), (5, 0, ROT2, 1 << 29)],
                           profiles=[(3, 1), (OWN, 2)])
    seen, ok = game.receive(0xF7, update)
    want = [(0xEA, bytes([24, 2])), (0xE9, bytes([24, 4])), relay(0, 1195, ROT, 7), (0xF3, bytes([24, 3, 1]))]
    check(seen == want and ok,
          "0xF7: a leave, an entry, another's input (back 5: 0xEC), a kit choice; the own place's and the insane "
          "inputs (a rotation's half at 1024, an unused action bit) and the own kit choice left out; consumed")
    game.world(state=1)
    seen, ok = game.receive(0xF7, update)
    check(seen == [] and ok, "0xF7 before the static info's state (1): nothing, consumed")
    game.world(frozen=1)
    seen, ok = game.receive(0xF7, server_update(1250, left=[6]))
    game.world(now=1250)
    seen2, _ = game.receive(0xF7, server_update(1300, entered=[6]))
    check(seen == [] and seen2 == [(0xEA, bytes([25, 6])), (0xE9, bytes([26, 6]))],
          "a leave before the world is loaded waits for it (then before the update's entries)")
    seen, ok = game.receive(0xF7, b"\x02" + bytes(20))
    check(seen == [] and ok and game.log and "netcode" in game.log[-1],
          f"an update that is none: nothing, consumed, a log line ({game.log[-1].strip() if game.log else None!r})")

    # The own inputs: captured, sent, their outcomes.
    game.reset()
    game.world(now=1200)
    sent, syncs, al, ok = game.send_pass()
    check(sent == [] and al == 0 and ok and syncs == [(0xC0DE0001, 0xC0DE0002, 0xC0DE0003, 0xC0DE0004, CLIENT)],
          "the first send pass: synchronize with the original's arguments, then no update (it starts at its tick)")
    taken = game.own_input(1210, ROT, 0x21, OWN, 1)
    other = game.own_input(1220, ROT, 0x22, 3, 0)
    profile = game.own_profile(1250, 2, OWN)
    check(taken[0] == "taken" and taken[1] and other[0] == "stock" and other[1] and profile[0] == "taken"
          and profile[1],
          "the own input and kit choice are taken (on at 0x27EB4D, 0x27ECD2, the frame as it was); another place's "
          "goes on to the stock new_packet")
    game.world(now=1260)
    sent, syncs, al, ok = game.send_pass()
    want = [(0xE2, client_update(1250, [(40, ROT, 0x80000021)], [2]), 0)]
    check([(i, b, f & 0x40) for i, b, f in sent] == want and al == 1 and ok and len(syncs) == 1,
          "the next send pass: tick 1250's update with the input (back 40, the flag as bit 31) and the kit choice, "
          "its packet not joined; AL 1")
    game.world(now=1400)
    sent, _, al, ok = game.send_pass()
    check([b for _, b, _ in sent] == [client_update(t) for t in (1300, 1350, 1400)] and al == 1 and ok,
          "a pass 150 ms later: three empty updates, one a tick")
    seen, ok = game.receive(0xF7, server_update(1450, outcomes=[(1250, 1, 1300), (1300, 0, 0), (1350, 2, 0)]))
    check(seen == [moved(1210, 1260), (0xF4, bytes([25, 26]))] and ok,
          "outcomes: 1250 moved to 1300 moves its input (1210 to 1260: 0xF0) and its kit choice (0xF4); "
          "confirmed and empty updates nothing")
    game.world(now=1500)
    game.w32(LINK + POOL_IN_USE, 250)
    sent, _, al, _ = game.send_pass()
    check(sent == [] and al == 0, "no update while the packet pool is nearly full")

    # The clock.
    orders = []
    for n in range(12):
        game.world(now=1500 + 50 * n)
        seen, _ = game.receive(0xF7, server_update(1500 + 50 * n, arrival=-100))
        orders += [(n, p) for i, p in seen if i == 0xE7]
    check(orders == [(9, struct.pack("<h", 105))],
          "late updates (100 ms): the tenth makes one 0xE7 of 105 ms (5 - -100), none while it corrects")

    # The stock protocol: nothing taken, nothing sent, 0xF7 nothing.
    stockgame = Game(bytes(built), rust)
    stockgame.world(state=1)
    stockgame.receive(0xE4, info)
    stockgame.world(now=1200)
    a = stockgame.own_input(1210, ROT, 0x21, OWN, 1)
    b = stock.own_input(1210, ROT, 0x21, OWN, 1)
    stockgame.send_pass()
    stockgame.world(now=1300)
    sent, syncs, al, ok = stockgame.send_pass()
    seen, ok2 = stockgame.receive(0xF7, update)
    check(a == b and a[0] == "stock" and sent == [] and al == 0 and ok and seen == [] and ok2,
          "a match of the stock protocol: the own input goes to the stock new_packet as in the original, no update, "
          "0xF7 nothing")

    aligned = game.aligned + stockgame.aligned
    entries = sorted({address for address, _ in aligned})
    check(len(entries) == len(rust) and all(residue == 0 for _, residue in aligned),
          f"each of the {len(rust)} Rust entries ran, every time with the stack aligned to 16 at its call")
    if failed:
        sys.exit(f"patch 79: {failed} check(s) FAILED")
    print("patch 79: all checks passed")


if __name__ == "__main__":
    main()
