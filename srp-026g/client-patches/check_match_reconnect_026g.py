#!/usr/bin/env python3
"""Patch 132: a receive aborted by the connect that replaced the match link's
socket is no lost link (client_patch.md section 3.78).

    python check_match_reconnect_026g.py <original survarium.exe>

The events run in the order the network thread makes them, each a run of the
game's own code in the original and in the built exe, on a made-up
udp_match_client:

- the tail of udp_match_client::connect (0x3F29EA..its `ret 10h`): the socket
  open, it closes it (0x3F3580) and, with its receive still pending, leaves
  connect_impl (0x3F2610) in the client's boost::function at +1280h (0x3F2A8B,
  the game's own bind and assign) for the last completion;
- send_packet_impl (0x3F30E0) up to its `inc [ebx+1CFCh]`: a send issued on
  the closed socket (start_send_to_op's check of the socket, 0x3F767E, posts
  it completed with WSAEBADF: that is the "file handle" line of the failure);
- handle_receive (0x3F2050) with 995, the aborted read: its first in the
  completion queue, the close posted it;
- original_packet_send_handler (0x3F3070) with WSAEBADF, the send.

Stubbed and written down: the socket's close, instant_disconnect (0x3FDF30),
connect_impl, start_receiving, the packet's send handler, the log's filter.

The original takes the abort for a lost link (instant_disconnect(1)); the
build leaves it, and the send's completion runs the connect exactly once.
Every other order and error is the game's, in both: no send outstanding (the
read's completion connects), a send completed before the read, a live link
losing its read (995, 10061), another error (the read goes on), a dead link.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_ECX, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_EAX

import patch_026g as base

CONNECT_TAIL, SEND_IMPL, SEND_COUNTED = 0x3F29EA, 0x3F30E0, 0x3F3101
HANDLE_RECEIVE, SEND_HANDLER = 0x3F2050, 0x3F3070
CLOSE, DISCONNECT, CONNECT_IMPL, START_RECEIVING, PACKET_HANDLER = 0x3F3580, 0x3FDF30, 0x3F2610, 0x3F2590, 0x3FE260
FILTERS, LOG, LOG_FILTERED = 0x111860, 0x111ED0, 0x1F1D60C
ABORTED, REFUSED, BAD_DESCRIPTOR, OTHER = 995, 10061, 10009, 10054
STATE, WAITING, SOCKET, SENDS, RECEIVING = 0x15C, 0x1280, 0x12A4, 0x1CFC, 0x1D04
CONNECTED, DISCONNECTING, DISCONNECTED = 0, 1, 3

CLIENT, SCRATCH, STACK = 0x42000000, 0x41000000, 0x41100000
ERROR_CODE, RETURN, PACKET, TIME = SCRATCH, SCRATCH + 0x1000, 0x51000000, 0x12345678
TOP = STACK + 0x8000


class Client:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(CLIENT, 0x4000, UC_PROT_ALL)
        emu.mem_map(SCRATCH, 0x2000, UC_PROT_ALL)
        emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
        self.word(LOG_FILTERED, 1)                             # the filter refuses every log line
        self.stubs = {
            CLOSE: (0, self.on_close),
            DISCONNECT: (4, self.on_disconnect),
            CONNECT_IMPL: (8, lambda arg: ("connect", self.emu.reg_read(UC_X86_REG_ECX), arg(0), arg(1))),
            START_RECEIVING: (0, lambda arg: ("start_receiving",)),
            PACKET_HANDLER: (8, None),
            FILTERS: (None, None),
        }
        emu.hook_add(UC_HOOK_CODE, self.stub)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def byte(self, at):
        return self.emu.mem_read(at, 1)[0]

    def on_close(self, arg):
        assert self.emu.reg_read(UC_X86_REG_EAX) == CLIENT + SOCKET - 4, "not the client's socket"
        self.word(CLIENT + SOCKET, 0xFFFFFFFF)
        return ("close",)

    def on_disconnect(self, arg):
        assert self.emu.reg_read(UC_X86_REG_ESI) == CLIENT + 8, "not the client's connection"
        self.word(CLIENT + STATE, DISCONNECTED)                # as the real one's first write, 0x3FDF33
        return ("instant_disconnect", arg(0))

    def stub(self, emu, address, size, _):
        if address == LOG:
            raise AssertionError("the log should be filtered")
        if address not in self.stubs:
            return
        pops, record = self.stubs[address]
        sp = emu.reg_read(UC_X86_REG_ESP)
        if record is not None:
            call = record(lambda n: self.read(sp + 4 + 4 * n))
            self.calls.append(call)
        emu.reg_write(UC_X86_REG_EAX, 0)                       # 0x111860: filtered out
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + (pops or 0))

    def reset(self, *, state=DISCONNECTING, sends=0, receiving=1):
        self.emu.mem_write(CLIENT, bytes(0x4000))
        self.word(CLIENT + STATE, state)
        self.word(CLIENT + SOCKET, 0x100)                      # open
        self.word(CLIENT + SENDS, sends)
        self.emu.mem_write(CLIENT + RECEIVING, bytes([receiving]))
        self.calls = []

    def run(self, start, stack, regs, stop=RETURN, pops=0):
        emu = self.emu
        sp = TOP - 0x4000
        for i, value in enumerate([RETURN] + stack):
            self.word(sp + 4 * i, value)
        for reg, value in regs.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, sp)
        emu.emu_start(start, stop, count=50000)
        assert emu.reg_read(UC_X86_REG_EIP) == stop, hex(emu.reg_read(UC_X86_REG_EIP))
        if stop == RETURN:
            assert emu.reg_read(UC_X86_REG_ESP) == sp + 4 + pops, "not the function's own return"

    def connect(self):
        """udp_match_client::connect from its test of the socket; the frame
        its prologue made (0x54 of locals, four saved registers), the four
        arguments above."""
        emu, sp = self.emu, TOP - 0x4000
        frame = sp - 0x64
        for i, value in enumerate([RETURN, 0, 0, PACKET, TIME]):
            self.word(sp + 4 * i, value)
        emu.reg_write(UC_X86_REG_ESI, CLIENT)
        emu.reg_write(UC_X86_REG_ESP, frame)
        emu.emu_start(CONNECT_TAIL, RETURN, count=50000)
        assert emu.reg_read(UC_X86_REG_EIP) == RETURN and emu.reg_read(UC_X86_REG_ESP) == sp + 4 + 0x10

    def send_issued(self):
        """send_packet_impl to its count of the send (what the rest does: the
        start_send_to_op of the docstring)."""
        self.run(SEND_IMPL, [PACKET, 0, 0, 0], {UC_X86_REG_ECX: CLIENT}, stop=SEND_COUNTED)

    def read_done(self, error, bytes_transferred=0):
        self.word(ERROR_CODE, error)
        self.word(ERROR_CODE + 4, 0)
        self.run(HANDLE_RECEIVE, [ERROR_CODE, bytes_transferred], {UC_X86_REG_ECX: CLIENT}, pops=8)

    def send_done(self, error):
        self.word(ERROR_CODE, error)
        self.run(SEND_HANDLER, [ERROR_CODE, 0, ERROR_CODE], {UC_X86_REG_ECX: CLIENT}, pops=0xC)


def connected_once(client, calls):
    assert calls.count(("connect", CLIENT, PACKET, TIME)) == 1, calls
    assert client.read(CLIENT + WAITING) == 0, "the deferred connect not cleared"


def failure_order(client, build):
    """The failure's order: close, a send on the closed socket, the abort, the send."""
    client.reset()
    client.connect()
    assert client.calls == [("close",)], client.calls
    assert client.read(CLIENT + WAITING) != 0, "connect did not wait for the completions"
    client.send_issued()
    assert client.read(CLIENT + SENDS) == 1
    client.read_done(ABORTED)
    assert client.byte(CLIENT + RECEIVING) == 0, "the read is done"
    if build:
        assert client.calls == [("close",)], ("a lost link", client.calls)
        assert client.read(CLIENT + STATE) == DISCONNECTING, "the old connection's state touched"
        assert client.read(CLIENT + WAITING) != 0, "the connect still waits for the send"
    else:
        assert client.calls == [("close",), ("instant_disconnect", 1)], client.calls
    client.send_done(BAD_DESCRIPTOR)
    assert client.read(CLIENT + SENDS) == 0
    connected_once(client, client.calls)
    return client.calls


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    site = base.file_offset(0x3F21DE)
    assert original[site:site + 7] == bytes.fromhex("83BB5C01000003"), original[site:site + 7].hex()
    assert built[site] == 0xE9, "the site is not hooked"
    assert built[site + 5:site + 7] == bytes.fromhex("9090"), built[site + 5:site + 7].hex()

    stock, patched = Client(original), Client(bytes(built))
    assert failure_order(stock, False) == [("close",), ("instant_disconnect", 1), ("connect", CLIENT, PACKET, TIME)]
    assert failure_order(patched, True) == [("close",), ("connect", CLIENT, PACKET, TIME)]

    for client in (stock, patched):
        # no send outstanding: the read's completion connects (a good cycle)
        client.reset()
        client.connect()
        client.read_done(ABORTED)
        assert client.calls == [("close",), ("connect", CLIENT, PACKET, TIME)], client.calls
        assert client.read(CLIENT + WAITING) == 0

        # a send issued before the close, its completion first
        client.reset(sends=1)
        client.connect()
        client.send_done(0)
        assert client.calls == [("close",)], client.calls
        client.read_done(ABORTED)
        assert client.calls == [("close",), ("connect", CLIENT, PACKET, TIME)], client.calls

        # a live link's read fails: the game's handling, unchanged
        for error, want in ((ABORTED, [("instant_disconnect", 1)]), (REFUSED, [("instant_disconnect", 1)]),
                            (OTHER, [("start_receiving",)])):
            client.reset(state=CONNECTED)
            client.read_done(error)
            assert client.calls == want, (hex(error), client.calls)

        # a dead link: nothing
        client.reset(state=DISCONNECTED)
        client.read_done(ABORTED)
        assert client.calls == [], client.calls

    # the build: a send outstanding past the abort, then another; the last connects
    patched.reset()
    patched.connect()
    patched.send_issued()
    patched.send_issued()
    patched.read_done(ABORTED)
    patched.send_done(BAD_DESCRIPTOR)
    assert patched.calls == [("close",)], patched.calls
    patched.send_done(BAD_DESCRIPTOR)
    connected_once(patched, patched.calls)
    print("patch 132: the failure's order (close, a send on the closed socket, the read's 995, the send's WSAEBADF) "
          "disconnects the original (instant_disconnect(1)) and not the build, whose send completion connects once; "
          "good cycles, a send completed first, live links (995, 10061, another error) and dead links are the "
          "original's in both")


if __name__ == "__main__":
    main()
