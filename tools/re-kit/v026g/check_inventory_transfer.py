#!/usr/bin/env python3
"""Exercise the original inventory ABI before implementing held artefact transfer.

Usage: python check_inventory_transfer.py <original survarium.exe>
Requires Unicorn. This is research validation, not a feature implementation.
Native inventory code runs; item lifecycle callbacks are recording stubs.
"""
from pathlib import Path
import struct
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "srp-026g/client-patches"))
from check_artefacts_026g import Game
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_ECX, UC_X86_REG_EAX


class InventoryGame(Game):
    def __init__(self, data):
        super().__init__(data)
        self.events = []
        self.vtable = self.block(0x100)
        self.set_owner = self.block(0x30)
        # thiscall(inventory, slot), exactly the item virtual +28h ABI.
        self.emu.mem_write(self.set_owner, bytes.fromhex(
            "8b442404 898150020000 8b442408 898154020000 c20800"))
        self.taken, self.removed, self.not_grenade = [self.block(0x10) for _ in range(3)]
        self.emu.mem_write(self.taken, b"\xc3")
        self.emu.mem_write(self.removed, b"\xc3")
        self.emu.mem_write(self.not_grenade, bytes.fromhex("31c0 c3"))
        for offset, callback in [(0x28, self.set_owner), (0x30, self.taken),
                                 (0x34, self.removed), (0x58, self.not_grenade)]:
            self.word(self.vtable + offset, callback)
        self.emu.hook_add(UC_HOOK_CODE, self.record,
                          begin=self.taken, end=self.removed)

    def record(self, emu, address, _size, _data):
        if address in (self.taken, self.removed):
            self.events.append(("taken" if address == self.taken else "removed",
                                emu.reg_read(UC_X86_REG_ECX)))

    def item(self):
        item = self.block(0x340)
        self.word(item, self.vtable)
        # Anomaly ownership plus this fixture's retained reference.
        self.word(item + 0xD0, 2)
        self.word(item + 0x24C, 1)
        return item

    def slots(self, items, before=False):
        external = self.block(max(4, len(items) * 4)) if before else None
        inventory = self.block(0x190)
        external = external or self.block(max(4, len(items) * 4))
        self.word(inventory + 0x168, external)
        self.emu.mem_write(inventory + 0x16C, bytes([len(items)]))
        for ordinal, item in enumerate(items):
            index = self.encoded_slot(inventory, ordinal)
            if item:
                reference = self.block(4)
                self.word(reference, item)
                self.call(0x2FC620, (reference,), esi=inventory, edi=index)
        self.events.clear()
        return inventory

    def encoded_slot(self, inventory, ordinal):
        relative = (self.read(inventory + 0x168) - inventory - 0x10C) & 0xFFFFFFFF
        signed = struct.unpack("<i", struct.pack("<I", relative))[0]
        return ((signed >> 2) + ordinal) & 0xFFFFFFFF

    def contents(self, inventory, count):
        first = self.read(inventory + 0x168)
        return [self.read(first + 4 * index) for index in range(count)]

    def take(self, inventory, item):
        reference = self.block(4)
        self.word(reference, item)
        self.call(0x2FC850, (reference,), eax=inventory)


def run(data):
    g = InventoryGame(data)
    a, b, incoming = [g.item() for _ in range(3)]
    inventory = g.slots([a, 0, b])
    # Empty artefact containers occupy a quick-slot ordinal too.
    for quick, expected in enumerate([g.encoded_slot(inventory, i) for i in range(3)] + [23]):
        g.call(0x2FE7F0, (inventory, quick))
        assert g.emu.reg_read(UC_X86_REG_EAX) == expected
    g.take(inventory, incoming)
    assert g.contents(inventory, 3) == [a, incoming, b]
    assert g.events == [("taken", incoming)]
    assert g.read(incoming + 0x250) == inventory
    assert g.read(incoming + 0x254) == g.encoded_slot(inventory, 1)
    assert g.read(incoming + 0xD0) == 3

    g = InventoryGame(data)
    a, b, c, incoming = [g.item() for _ in range(4)]
    inventory = g.slots([a, b, c])
    g.take(inventory, incoming)
    assert g.contents(inventory, 3) == [b, c, incoming]
    assert g.events == [("removed", a), ("taken", incoming)]
    assert g.read(a + 0x250) == 0 and g.read(a + 0x254) == 23
    assert g.read(a + 0xD0) == 2
    assert all(g.read(item + 0xD0) == 3 for item in [b, c, incoming])

    g = InventoryGame(data)
    item = g.item()
    donor, recipient = g.slots([item]), g.slots([0])
    # Retain before removal, as a future owned transfer guard must do.
    g.word(item + 0xD0, g.read(item + 0xD0) + 1)
    g.call(0x2FC680, esi=donor, eax=g.encoded_slot(donor, 0))
    assert g.contents(donor, 1) == [0]
    g.take(recipient, item)
    g.word(item + 0xD0, g.read(item + 0xD0) - 1)
    assert g.contents(recipient, 1) == [item]
    assert g.read(item + 0x250) == recipient
    assert g.read(item + 0xD0) == 3
    assert g.events == [("removed", item), ("taken", item)]

    # A container allocation below the inventory creates a wrapped negative
    # native slot. Quick lookup, remove and take must all reconstruct it.
    g = InventoryGame(data)
    item = g.item()
    donor = g.slots([item], before=True)
    slot = g.encoded_slot(donor, 0)
    assert slot > 0xF0000000
    g.call(0x2FE7F0, (donor, 0))
    assert g.emu.reg_read(UC_X86_REG_EAX) == slot
    g.call(0x2FC680, esi=donor, eax=slot)
    assert g.contents(donor, 1) == [0]
    g.take(donor, item)
    assert g.contents(donor, 1) == [item]
    assert g.read(item + 0x254) == slot

    g = InventoryGame(data)
    world = g.block(0x19040)
    begin = world + 0x18B58
    g.word(world + 0x18B4C, begin)
    g.word(world + 0x18B50, begin)
    g.word(world + 0x18B54, begin + 255 * 4)
    for index in range(2):
        # Weapon's inventory subobject follows its active-object prefix.
        active = g.block(0x600)
        g.call(0x2E30F0, (world,), ecx=active + 0x10)
        assert g.read(begin + 4 * index) == active
        assert bytes(g.emu.mem_read(active + 0x0C, 1)) == bytes([index])
        assert g.read(active + 4) == world
        assert g.read(active + 0x10 + 0x248) == world
        assert g.read(world + 0x18B50) == begin + (index + 1) * 4
    print("original inventory: quick-slot mapping, empty pickup, full eviction, "
          "retained transfer and active-object registration passed")


if __name__ == "__main__":
    run(Path(sys.argv[1]).read_bytes())
