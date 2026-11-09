#!/usr/bin/env python3
"""Run the original and patch 30's lobby code for taking an upgrade off an
item: the upgrade window's drop_upgrade_item (0x29A9D0, which writes `46
02`) and the answers `83 46 02` and `83 46 01` (process_succeeded_
inventory_action 0x275520, with process_detach_upgrade 0x275220), on
made-up inventories where one upgrade is on several items.

    python check_shared_upgrades_026g.py <original survarium.exe>

Requires Python `unicorn` (2.1.4). The functions run as machine code from
their first instruction, with the game's own searches and vector code
(0x277AA0, 0x277CF0, 0x2775B0, 0x277520, 0x276EB0, 0x26FC60); the lobby ui,
the lobby client and its inventory are made up, and the packet functions
(new_packet 0x37A0E0, mutable_buffer::resize 0x7AB30, enqueue 0x379640),
update_inventory_item (0x287CC0) and the log filter (0x111860) are stubs
that write down how they were called. The original sends `46 02 <kit>
<upgrade>` and takes an upgrade off the first item holding it; the default
build of patch_026g.py must send `46 02 <kit> <upgrade> <item>` and take
it off the item an answer names, leaving it on the others. Says nothing
about the Flash side or the server (their tests do).
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EIP, \
    UC_X86_REG_ESP, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP

import patch_026g as base

NEW_PACKET, RESIZE, ENQUEUE = 0x37A0E0, 0x7AB30, 0x379640
UPDATE_INVENTORY_ITEM, LOG_FILTER, LOG_ON = 0x287CC0, 0x111860, 0x1F1D60C
ITEMS, ITEM_SIZE, RECORD_SIZE, RECORDS = 0x1EA978, 0xB4, 0x18, 3

UI, OWNER, OWNER2, VTABLE, FRAME_OBJECT = 0x20000000, 0x20001000, 0x20002000, 0x20003000, 0x20004000
ARGS, VALUES = 0x20005000, 0x20005100
CLIENT = 0x30000000                           # the lobby client (inventory at +1EA978h)
MENU_HOLDER, MENU = 0x20006000, 0x20007000    # client+1E56C0h -> +5390h: the lobby menu
INVENTORY = 0x21100000                        # the items, then their upgrade records
PACKET, BUFFER, DATA = 0x21200000, 0x21201000, 0x21202000
READER, MESSAGE = 0x21300000, 0x21300100
STACK, STOP, STUB = 0x21008000, 0x22000000, 0x22000100
REGISTERS = (UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI)
KIT = 0x51


class Lobby:
    def __init__(self, data, items=()):
        """items: [(item id, dict id, [(upgrade id, slot)])]"""
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        headers = base.Headers(data)
        for _, rva, vsize, raw, rsize, _ in headers.sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((UI, 0x10000), (0x21000000, 0x10000), (STOP, 0x10000), (CLIENT, 0x1F0000),
                         (INVENTORY, 0x10000), (PACKET, 0x10000), (READER, 0x1000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        for at, value in ((UI + 0x9C, OWNER), (OWNER + 0x53AC, OWNER2), (OWNER2, VTABLE), (VTABLE + 0x3C, STUB),
                          (UI + 0x6A0, FRAME_OBJECT), (FRAME_OBJECT + 0x4620, KIT),
                          (CLIENT + 0x1E56C0, MENU_HOLDER), (MENU_HOLDER + 0x5390, MENU),
                          (PACKET + 0x2BC, BUFFER), (BUFFER + 4, DATA), (BUFFER + 0xC, 0),
                          (LOG_ON, 1)):
            self.word(at, value)
        self.stubs = {STUB: "client", NEW_PACKET: "new_packet", RESIZE: "resize", ENQUEUE: "enqueue",
                      UPDATE_INVENTORY_ITEM: "update_inventory_item", LOG_FILTER: "log_filter"}
        self.calls, self.sent = [], []
        self.items = [item_id for item_id, _, _ in items]
        records = INVENTORY + ITEM_SIZE * len(items)
        for index, (item_id, dict_id, upgrades) in enumerate(items):
            at = INVENTORY + ITEM_SIZE * index
            self.word(at, item_id)
            emu.mem_write(at + 4, struct.pack("<H", dict_id))
            first = records + RECORD_SIZE * RECORDS * index
            for slot_index, (upgrade_id, slot) in enumerate(upgrades):
                record = first + RECORD_SIZE * slot_index
                emu.mem_write(record, struct.pack("<IIQB", upgrade_id, self.dict_of(items, upgrade_id),
                                                  0x1000 + upgrade_id, slot))
            for offset, value in ((0x60, first), (0x64, first + RECORD_SIZE * len(upgrades)),
                                  (0x68, first + RECORD_SIZE * RECORDS)):
                self.word(at + offset, value)
        self.word(CLIENT + ITEMS, INVENTORY)
        self.word(CLIENT + ITEMS + 4, INVENTORY + ITEM_SIZE * len(items))
        emu.hook_add(UC_HOOK_CODE, self.stub)

    @staticmethod
    def dict_of(items, item_id):
        return next((dict_id for id_, dict_id, _ in items if id_ == item_id), 0)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def returns(self, value, pop=0):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        self.emu.reg_write(UC_X86_REG_EAX, value)
        self.emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pop)

    def stub(self, emu, address, size, _):
        name = self.stubs.get(address)
        if name is None:
            return
        sp = emu.reg_read(UC_X86_REG_ESP)
        arg = lambda n: self.read(sp + 4 + 4 * n)
        eax = emu.reg_read(UC_X86_REG_EAX)
        # What a call may change: make sure the code does not count on it.
        emu.reg_write(UC_X86_REG_ECX, 0xBADC0DE1)
        emu.reg_write(UC_X86_REG_EDX, 0xBADC0DE2)
        if name == "client":                                  # thiscall, no argument
            self.returns(CLIENT)
        elif name == "new_packet":
            assert eax == CLIENT + 8, hex(eax)
            self.calls.append((name, arg(0)))
            self.word(BUFFER + 0xC, 0)
            self.returns(PACKET, 4)
        elif name == "resize":
            assert eax == BUFFER, hex(eax)
            self.calls.append((name, arg(0)))
            self.word(BUFFER + 0xC, arg(0))
            self.returns(eax, 4)
        elif name == "enqueue":
            assert (arg(0), arg(1)) == (CLIENT + 8, PACKET), (hex(arg(0)), hex(arg(1)))
            self.sent.append(bytes(emu.mem_read(DATA, self.read(BUFFER + 0xC))))
            self.returns(0, 8)
        elif name == "update_inventory_item":
            assert arg(0) == MENU
            item = emu.reg_read(UC_X86_REG_ESI)
            self.calls.append((name, self.read(item), arg(1)))
            self.returns(0, 8)
        elif name == "log_filter":                            # cdecl: no line is written
            self.returns(0)

    def start(self, entry, args, ecx=0, eax=0):
        emu = self.emu
        self.word(STACK, STOP)
        for index, value in enumerate(args):
            self.word(STACK + 4 + 4 * index, value)
        emu.reg_write(UC_X86_REG_ESP, STACK)
        emu.reg_write(UC_X86_REG_ECX, ecx)
        emu.reg_write(UC_X86_REG_EAX, eax)
        for register in REGISTERS:
            emu.reg_write(register, 0x12345678)
        emu.emu_start(entry, STOP, count=200000)
        assert emu.reg_read(UC_X86_REG_EIP) == STOP and emu.reg_read(UC_X86_REG_ESP) == STACK + 4 + 4 * len(args), \
            "it returns with `ret 4`"
        assert all(emu.reg_read(register) == 0x12345678 for register in REGISTERS), "callee-saved registers"

    def drop(self, host, upgrade):
        """Flash drop_upgrade_item(host, upgrade): the delegate's arguments,
        two values 18h bytes apart, the number at +8 of each."""
        self.word(ARGS + 8, VALUES)
        self.word(VALUES + 8, host)
        self.word(VALUES + 0x20, upgrade)
        self.start(base.DROP_UPGRADE_HANDLER, [ARGS], ecx=UI)
        assert self.emu.reg_read(UC_X86_REG_EAX) & 0xFF == 1, "it returns true"
        return self.sent

    def answer(self, message):
        """A `83 46 ..` answer from its sub id on, as the lobby client reads it."""
        self.emu.mem_write(MESSAGE, message)
        self.word(READER + 4, MESSAGE)
        self.start(base.INVENTORY_ACTION, [CLIENT], eax=READER)
        return self

    def upgrades(self):
        """{item id: [(upgrade id, slot)]}, the records as the client keeps them."""
        out = {}
        for index, item_id in enumerate(self.items):
            at = INVENTORY + ITEM_SIZE * index
            begin, end = self.read(at + 0x60), self.read(at + 0x64)
            out[item_id] = [(self.read(record), bytes(self.emu.mem_read(record + 0x10, 1))[0])
                            for record in range(begin, end, RECORD_SIZE)]
        return out

    def updated(self):
        return [call[1:] for call in self.calls if call[0] == "update_inventory_item"]


# Two vests (1, 2) wearing camouflage 9; vest 2 also wears decal 7 in slot 0;
# trousers 3 wear nothing.
VEST, VEST2, TROUSERS, CAMO, DECAL = 1, 2, 3, 9, 7
CAMO_DICT, DECAL_DICT = 0x2001, 0x2002
WORN = [(VEST, 0x1001, [(CAMO, 3)]), (VEST2, 0x1001, [(CAMO, 3), (DECAL, 0)]), (TROUSERS, 0x1002, []),
        (CAMO, CAMO_DICT, []), (DECAL, DECAL_DICT, [])]


def detach(upgrade, *host):
    return bytes([2]) + struct.pack("<I", upgrade) + b"".join(struct.pack("<I", h) for h in host)


def attach(host, upgrade, slot, detach_first):
    return bytes([1]) + struct.pack("<IIBQB", host, upgrade, slot, 0x55, detach_first)


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late_join = True
    patched, _ = base.build(original, "section", code, None, late_join)
    checks = 0

    # The drop: the original names the kit and the upgrade, the build the item too.
    for host, upgrade in ((VEST, CAMO), (VEST2, DECAL), (0, CAMO), (0xFFFFFFFE, 0x7FFFFFFF)):
        sent = Lobby(original, WORN).drop(host, upgrade)
        assert sent == [bytes([2]) + struct.pack("<II", KIT, upgrade)], sent
        lobby = Lobby(patched, WORN)
        sent = lobby.drop(host, upgrade)
        assert sent == [bytes([2]) + struct.pack("<III", KIT, upgrade, host)], sent
        assert [call for call in lobby.calls if call[0] == "new_packet"] == [("new_packet", 0x46)]
        checks += 1

    # 83 46 02: the original takes the upgrade off the first item holding it.
    lobby = Lobby(original, WORN).answer(detach(CAMO))
    assert lobby.upgrades()[VEST] == [] and lobby.upgrades()[VEST2] == [(CAMO, 3), (DECAL, 0)], lobby.upgrades()
    assert lobby.updated() == [(CAMO, 0), (VEST, 0)], lobby.updated()
    checks += 1
    # The build: off the item named, and off no other.
    for host, left in ((VEST2, {VEST: [(CAMO, 3)], VEST2: [(DECAL, 0)]}),
                       (VEST, {VEST: [], VEST2: [(CAMO, 3), (DECAL, 0)]})):
        lobby = Lobby(patched, WORN).answer(detach(CAMO, host))
        upgrades = lobby.upgrades()
        assert {item: upgrades[item] for item in left} == left and upgrades[TROUSERS] == [], upgrades
        assert lobby.updated() == [(CAMO, 0), (host, 0)], lobby.updated()
        checks += 1
    lobby = Lobby(patched, WORN).answer(detach(DECAL, VEST2))
    assert lobby.upgrades()[VEST2] == [(CAMO, 3)] and lobby.upgrades()[VEST] == [(CAMO, 3)], lobby.upgrades()
    checks += 1
    # An item that does not hold it, or none of that id: nothing comes off.
    for host, updated in ((TROUSERS, [(CAMO, 0), (TROUSERS, 0)]), (77, [(CAMO, 0)])):
        lobby = Lobby(patched, WORN).answer(detach(CAMO, host))
        assert lobby.upgrades()[VEST] == [(CAMO, 3)] and lobby.upgrades()[VEST2] == [(CAMO, 3), (DECAL, 0)]
        assert lobby.updated() == updated, (host, lobby.updated())
        checks += 1
    # Item 0: the original's search.
    lobby = Lobby(patched, WORN).answer(detach(CAMO, 0))
    assert lobby.upgrades()[VEST] == [] and lobby.upgrades()[VEST2] == [(CAMO, 3), (DECAL, 0)]
    checks += 1
    # The message is read to its end: the reader is past the item.
    lobby = Lobby(patched, WORN).answer(detach(CAMO, VEST2))
    assert lobby.read(READER + 4) == MESSAGE + 9, hex(lobby.read(READER + 4))
    lobby = Lobby(original, WORN).answer(detach(CAMO))
    assert lobby.read(READER + 4) == MESSAGE + 5, hex(lobby.read(READER + 4))
    checks += 1

    # 83 46 01 without detach_first: the upgrade goes on one more item, in both.
    for data in (original, patched):
        lobby = Lobby(data, WORN).answer(attach(TROUSERS, CAMO, 3, 0))
        assert lobby.upgrades() == {VEST: [(CAMO, 3)], VEST2: [(CAMO, 3), (DECAL, 0)], TROUSERS: [(CAMO, 3)],
                                    CAMO: [], DECAL: []}, lobby.upgrades()
        assert lobby.updated()[-2:] == [(CAMO, TROUSERS), (TROUSERS, 0)], lobby.updated()
        checks += 1
    # With detach_first: the original takes it off the first item holding it
    # (vest 1), the build off the item it is put on (it was in another slot).
    lobby = Lobby(original, WORN).answer(attach(VEST2, CAMO, 3, 1))
    assert lobby.upgrades()[VEST] == [] and sorted(lobby.upgrades()[VEST2]) == [(DECAL, 0), (CAMO, 3)]
    lobby = Lobby(patched, WORN).answer(attach(VEST2, CAMO, 3, 1))
    assert lobby.upgrades()[VEST] == [(CAMO, 3)] and sorted(lobby.upgrades()[VEST2]) == [(DECAL, 0), (CAMO, 3)]
    checks += 1
    # A decal moved from slot 0 to slot 1 of the same item, with vest 1
    # wearing it too: the build leaves vest 1's.
    worn = [(VEST, 0x1001, [(DECAL, 0)]), (VEST2, 0x1001, [(DECAL, 0)]), (DECAL, DECAL_DICT, [])]
    lobby = Lobby(original, worn).answer(attach(VEST2, DECAL, 1, 1))
    assert lobby.upgrades()[VEST] == [] and lobby.upgrades()[VEST2] == [(DECAL, 0), (DECAL, 1)], lobby.upgrades()
    lobby = Lobby(patched, worn).answer(attach(VEST2, DECAL, 1, 1))
    assert lobby.upgrades()[VEST] == [(DECAL, 0)] and lobby.upgrades()[VEST2] == [(DECAL, 1)], lobby.upgrades()
    checks += 1
    print(f"patch 30: {checks} drops and answers as expected (the original names no item and takes an upgrade "
          f"off the first item holding it; the build names the item and takes it off that one alone)")


if __name__ == "__main__":
    main()
