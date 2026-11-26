#!/usr/bin/env python3
"""Run patch 211, the world of a tick (rust/patches/src/exact_world.rs,
vostok/exact_world.rs; ../docs/notes/desync_capture.md), in the default build
beside the original exe.

    python check_exact_world_026g.py <original survarium.exe>

Requires Python `unicorn`. Its two sites run from their entry, on a made-up
history item, hash() frame, network client, options holder, packet pool and
message reader, with GetCurrentThreadId a stub and every other import a
failure; the packet calls (match_client's begin 0x279FE0 and send 0x27A2A0,
the buffer write 0x7AC10) are stubs that record what is sent:

- `world_hash.serialized` (hash() after its serialize, 0x3251DC): for worlds
  of 20 000 and 0xF000 bytes, the registers after the span and every byte of
  the made-up memory are the original's;
- `upload_request.case` (the dispatcher's case 0xF6, 0x27A4F4):
  - a request of one byte (srp's 0), of 1 and a tick the ring does not hold,
    of 1 and a held tick with the packet pool too full for its chunks, of
    two bytes not 1, and of a held tick in a connection state other than 2:
    the run reaches the snapshot upload's request hook (0x27A505) with the
    registers and the made-up memory the original's, and nothing is sent;
  - of 1 and a held tick: the run leaves at the case's end (0x27A5AB) with the
    callee-saved registers kept, and the chunks sent are that tick's world
    byte for byte, as the snapshot upload sends it (u32 time, u16 offset,
    u16 total, 480 bytes at most; each packet's flag 0x40 cleared);
  - a tick hashed again is its newest world.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

ENGINE_TSV = "engine_026g.tsv"
HASHED, REQUEST = (0x3251DC, 5), (0x27A4F4, 7)
REQUEST_HOOK, DONE = 0x27A505, 0x27A5AB
BEGIN, SEND, APPEND = 0x279FE0, 0x27A2A0, 0x7AC10
THREAD_SLOT = 0x870118
CLIENT, HOLDER, POOL, PACKET = 0x40000000, 0x40400000, 0x40500000, 0x40600000
ITEM, BODY, READER, STACK, STUBS, GAME = 0x41000000, 0x42000000, 0x43000000, 0x44000000, 0x45000000, 0x46000000
FORBIDDEN, THREAD = STUBS, STUBS + 0x10
SP = STACK + 0x10000
FRAME = SP + 0x100                      # hash()'s EBP
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP, UC_X86_REG_ESP)
KEPT = (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP)
MAPPED = ((CLIENT, 0x200000), (HOLDER, 0x60000), (POOL, 0x1000), (PACKET, 0x1000), (ITEM, 0xC1000),
          (BODY, 0x10000), (READER, 0x1000), (STACK, 0x20000), (GAME, 0x1000))
# compared after a site: the made-up objects and the stack from the entry's ESP up
MEMORY = ((CLIENT + 0x1F1C00, 0x100), (HOLDER + 0x578A0, 0x20), (POOL, 0x40), (ITEM + 0xC0BA0, 0x20),
          (BODY, 0x10000), (READER, 0x100), (SP, 0x10000))
CHUNK, MAX_BODY = 480, 0xF000


def world(time, size):
    return bytes((time // 50 + i * 7) & 0xFF for i in range(size))


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in MAPPED + ((STUBS, 0x1000),):
            emu.mem_map(at, size, UC_PROT_ALL)
        emu.mem_write(THREAD, bytes.fromhex("b801000000c3"))     # GetCurrentThreadId: mov eax,1 / ret
        self.forbidden, self.sent, self.pending = [], [], None
        for line in open(ENGINE_TSV, encoding="utf-8"):
            cells = line.rstrip("\n").split("\t")
            if len(cells) > 4 and cells[4] == "import":
                self.word(int(cells[1], 16), FORBIDDEN)
        self.word(THREAD_SLOT, THREAD)
        emu.hook_add(UC_HOOK_CODE, self.code, begin=FORBIDDEN, end=FORBIDDEN)
        for address in (BEGIN, SEND, APPEND):
            emu.hook_add(UC_HOOK_CODE, self.packet, begin=address, end=address)
        self.word(CLIENT, 0x9FFB14)                              # a live match's network_client
        self.word(CLIENT + 0x1F1C40, GAME)                       # its match
        self.word(CLIENT + 0x1F1C48, HOLDER)
        self.word(CLIENT + 0x1F1C64, 2)                          # the live match's transport
        self.word(HOLDER, 0xA08864)                              # match_client's vtable
        self.word(HOLDER + 0x577BC, 0x81234567)                  # the match's seed
        self.word(HOLDER + 0x578B0, POOL)
        self.word(POOL + 0x34, 256)                              # the pool's capacity

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def read(self, at):
        return struct.unpack("<I", bytes(self.emu.mem_read(at, 4)))[0]

    def code(self, emu, address, size, _):
        self.forbidden.append(hex(self.read(emu.reg_read(UC_X86_REG_ESP))))
        emu.emu_stop()

    def packet(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        args = [self.read(sp + 4 * i) for i in range(1, 4)]
        if address == BEGIN:                                     # thiscall (holder, id), ret 4
            assert emu.reg_read(UC_X86_REG_ECX) == HOLDER and args[0] == 0xE0, (hex(args[0]),)
            self.word(PACKET + 0x98, 0x60)
            self.word(POOL + 0x30, self.read(POOL + 0x30) + 1)
            pops, result = 4, PACKET
        elif address == APPEND:                                  # (writer, bytes) edi = count, ret 8
            assert args[0] == PACKET + 0x2A8
            self.pending = bytes(emu.mem_read(args[1], emu.reg_read(UC_X86_REG_EDI)))
            pops, result = 8, 0
        else:                                                    # thiscall (holder, packet), ret 4
            assert emu.reg_read(UC_X86_REG_ECX) == HOLDER and args[0] == PACKET
            self.sent.append((self.read(PACKET + 0x98) & 0x40, self.pending))
            pops, result = 4, 0
        emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def run(self, begin, stops, registers):
        """From `begin` to the first of `stops`: where, the registers, the
        made-up memory."""
        emu = self.emu
        self.sent, self.forbidden = [], []
        for reg in REGS:
            emu.reg_write(reg, 0x5A5A5A5A)
        for reg, value in registers.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, SP)
        handles = [emu.hook_add(UC_HOOK_CODE, lambda e, *_: e.emu_stop(), begin=stop, end=stop) for stop in stops]
        try:
            emu.emu_start(begin, 0, count=5_000_000)
        finally:
            for handle in handles:
                emu.hook_del(handle)
        assert not self.forbidden, f"an import called: {self.forbidden}"
        after = {reg: emu.reg_read(reg) for reg in REGS}
        memory = b"".join(bytes(emu.mem_read(at, size)) for at, size in MEMORY)
        return emu.reg_read(UC_X86_REG_EIP), after, memory

    def hashed(self, time, body):
        """hash() of an item of `time`, its serialize having written `body`."""
        self.word(ITEM + 0xC0BB0, time)
        self.emu.mem_write(BODY, body.ljust(0x10000, b"\0"))
        self.word(FRAME - 0x14, 0)                               # mutable_buffer {0, data, capacity, size}
        self.word(FRAME - 0x10, BODY)
        self.word(FRAME - 0x0C, MAX_BODY)
        self.word(FRAME - 0x08, len(body))
        begin, span = HASHED
        return self.run(begin, (begin + span,), {UC_X86_REG_ESI: ITEM, UC_X86_REG_EBP: FRAME})

    def request(self, payload, state=2, used=0):
        self.word(CLIENT + 0x1F1C64, state)
        self.word(POOL + 0x30, used)
        self.emu.mem_write(READER + 0x80, payload)
        self.word(READER, READER + 0x7F)                         # {begin, cursor, size}: the id, then the payload
        self.word(READER + 4, READER + 0x80)
        self.word(READER + 8, 1 + len(payload))
        return self.run(REQUEST[0], (REQUEST_HOOK, DONE), {UC_X86_REG_ESI: CLIENT, UC_X86_REG_EBX: READER})


def chunks(sent):
    """The world the chunks carry: its time and bytes, if they are whole."""
    data, time, total = bytearray(), None, None
    for index, (flag, carrier) in enumerate(sent):
        if flag or len(carrier) < 8:
            return None
        at, offset, size = struct.unpack("<IHH", carrier[:8])
        if offset != index * CHUNK or (time is not None and (at, size) != (time, total)):
            return None
        time, total = at, size
        data += carrier[8:]
    return (time, bytes(data)) if total == len(data) else None


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    failed = 0

    def case(what, ok, detail=""):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {what}" + (f"\n      {detail}" if not ok and detail else ""))

    stock, patched = Run(original), Run(bytes(built))
    t1, t2, t3 = 270_700, 270_750, 270_800
    worlds = {t1: world(t1, 20_000), t2: world(t2, MAX_BODY), t3: world(t3, 19_123)}
    for time, body in worlds.items():
        want, got = stock.hashed(time, body), patched.hashed(time, body)
        case(f"hash() of {time} ({len(body)} bytes): registers and memory as the original's", want == got)
    # 270700 hashed again (a rollback changed it): its newest world is the one held.
    again = world(t1 + 1, 20_001)
    case("a tick hashed again: as the original's", stock.hashed(t1, again) == patched.hashed(t1, again))
    worlds[t1] = again

    def tick(time):
        return (time // 50) & 0xFF

    falls = [("srp's one byte", bytes([0]), 2, 0),
             ("a tick not held", bytes([1, tick(t1 + 5_000)]), 2, 0),
             ("a held tick, the pool too full", bytes([1, tick(t2)]), 2, 256 - 16 - 10),
             ("two bytes not 1", bytes([2, tick(t2)]), 2, 0),
             ("a held tick, state 1", bytes([1, tick(t2)]), 1, 0)]
    for what, payload, state, used in falls:
        want, got = stock.request(payload, state, used), patched.request(payload, state, used)
        case(f"{what}: on to the snapshot upload (0x27A505), registers and memory as the original's",
             want == got and got[0] == REQUEST_HOOK, (hex(got[0]),))
        case(f"{what}: nothing sent", not patched.sent, len(patched.sent))
    for time in (t1, t2, t3):
        before = stock.request(bytes([0]))
        where, after, _ = patched.request(bytes([1, tick(time)]))
        carried = chunks(patched.sent)
        case(f"the tick {time}: the case done (0x27A5AB)", where == DONE, hex(where))
        case(f"the tick {time}: callee-saved registers kept",
             all(after[reg] == before[1][reg] for reg in KEPT if reg != UC_X86_REG_ESP)
             and after[UC_X86_REG_ESP] == SP)
        case(f"the tick {time}: {len(patched.sent)} chunks, its world byte for byte",
             carried == (time, worlds[time]) and len(patched.sent) == -(-len(worlds[time]) // CHUNK),
             (carried and (carried[0], len(carried[1])), len(patched.sent)))
    print("all cases passed" if not failed else f"{failed} FAILED")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
