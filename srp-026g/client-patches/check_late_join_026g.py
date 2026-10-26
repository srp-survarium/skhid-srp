#!/usr/bin/env python3
"""Execute build 2777's roster-capacity paths with Unicorn (2.1.4).

Usage: check_late_join_026g.py ORIGINAL --nasm /path/to/nasm
The original instructions run against small fixture objects. Per-player
statistics codecs and the animation bind method are intercepted. Both a
synthetic binder and the native binder with weapon inventories are exercised;
the bounded profile wrapper executes the native constructor and decoder with
only CRT memcpy intercepted. The statistics handler's start_match, on_event
and deserialize run on a made-up handler: a newcomer's scoreboard score
through rollbacks, in the original and in the build. The respawn rule's
spawn runs on a made-up rule and place (its calls intercepted) for two
games, one whose weapons of the new kit were drawn from and one whose are at
the constructor's 1: the build gives both the same random states, the ones
patch_026g.weapon_random_state names, after the player's initialize and
before it is put into the world, and nothing else; the original leaves them
apart; copies with one thing broken fail. The same spawn on a kit one
game played (its weapons as a last life leaves them) and another loaded a
world since: in the build the new kit's weapons have, at the initialize,
-1 as their last tick and, at a change of kit, a new weapon's fire mode,
ammunition, chamber, aim and recoil, alike in both games. The block of a game's first
world (0x2D0FF2..0x2D105E) runs with the game's own allocator on made-up
lists, then the places' own tick (0x321790): in the build the entries are
in time order and the kit choice is the world's, so the next kit after each
tick is the one the games that played the match have; in the original a
choice relayed for later is hidden by the kit in use. These tests establish profile decoding, capacity,
calling convention and ID allocation, not a playable late join.
"""
import argparse
import hashlib
from pathlib import Path
import re
import struct

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
from unicorn.x86_const import (
    UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
    UC_X86_REG_ESI, UC_X86_REG_EIP, UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_XMM0,
)

import late_join_026g as patch

BASE, SLOT, COUNTER, STACK, STOP = 0x1000000, 0x1030000, 0x1031000, 0x2008000, 0x3000000


def srp_section(data):
    """The build's .srp entry (name, rva, size, raw, raw size, flags), found
    by its name: sections of later patches (patch 46's .srpc) may follow .srpd."""
    return next(section for section in patch.base.Headers(data).sections(data)
                if section[0] == patch.base.SECTION_NAME)


class Machine:
    def __init__(self, data):
        self.uc = Uc(UC_ARCH_X86, UC_MODE_32)
        # Map only executable sections; unrelated runtime globals are not used.
        for name, rva, size, raw, raw_size, flags in patch.base.Headers(data).sections(data):
            if flags & 0x20000000 or name.startswith(b".srpd"):
                self.uc.mem_map(patch.base.IMAGE_BASE + rva, (max(size, raw_size) + 4095) & ~4095)
                if raw_size:
                    self.uc.mem_write(patch.base.IMAGE_BASE + rva, bytes(data[raw:raw + raw_size]))
        self.uc.mem_map(BASE, 0x40000)
        self.uc.mem_map(0x2000000, 0x10000)
        self.uc.mem_map(STOP, 4096)

    def read(self, address):
        return struct.unpack("<I", self.uc.mem_read(address, 4))[0]

    def write(self, address, value):
        self.uc.mem_write(address, struct.pack("<I", value))

    def start(self, address, end=STOP, args=(), count=10000):
        self.uc.reg_write(UC_X86_REG_ESP, STACK)
        self.uc.mem_write(STACK, struct.pack("<" + "I" * (len(args) + 1), STOP, *args))
        self.uc.emu_start(address, end, count=count)
        assert self.uc.reg_read(UC_X86_REG_EIP) == end, "instruction budget exhausted"


def statistics(data, roster, reading=False):
    machine = Machine(data)
    uc = machine.uc
    uc.mem_write(BASE + 0x31E0, bytes([roster]))
    uc.reg_write(UC_X86_REG_ESI, BASE)
    uc.reg_write(UC_X86_REG_EAX, BASE + 0x3500)
    seen = []
    codec = 0x3358B0 if reading else 0x335750

    def visit(uc, address, size, _):
        if address != codec:
            return
        esp = uc.reg_read(UC_X86_REG_ESP)
        seen.append((machine.read(esp + 4) - BASE) // 0x1F8)
        # Both native callers pass the statistics record on the stack.
        uc.reg_write(UC_X86_REG_EIP, machine.read(esp))
        uc.reg_write(UC_X86_REG_ESP, esp + (8 if reading else 12))

    uc.hook_add(UC_HOOK_CODE, visit)
    machine.start(0x2F5BE0 if reading else 0x2F5AC0,
                  0x2F5C0B if reading else 0x2F5AFC)
    assert uc.mem_read(BASE + 0x31E0, 1)[0] == roster
    return seen


def active_objects(machine, core, count=0):
    begin = core + 0x18B58
    machine.write(core + 0x18B4C, begin)
    machine.write(core + 0x18B50, begin + count * 4)
    machine.write(core + 0x18B54, begin + 255 * 4)
    return begin


def binding(data, place, consumed, old_counter):
    machine = Machine(data)
    uc = machine.uc
    active_objects(machine, BASE)
    machine.write(BASE + 0x18FB0, COUNTER)
    machine.write(COUNTER, old_counter)
    uc.mem_write(SLOT + 0x2C, bytes([place]))
    uc.reg_write(UC_X86_REG_EAX, BASE)
    uc.reg_write(UC_X86_REG_EBX, 0x12345678)
    uc.reg_write(UC_X86_REG_ESI, 0x23456789)
    ids = []

    def visit(uc, address, size, _):
        if address != 0x321820:
            return
        esp = uc.reg_read(UC_X86_REG_ESP)
        assert uc.reg_read(UC_X86_REG_EAX) == BASE
        assert machine.read(esp + 4) == SLOT
        counter = machine.read(BASE + 0x18FB0)
        ids.extend(range(machine.read(counter), machine.read(counter) + consumed))
        machine.write(counter, machine.read(counter) + consumed)
        uc.reg_write(UC_X86_REG_EAX, 0xDEADBEEF)  # native volatile register
        uc.reg_write(UC_X86_REG_EIP, machine.read(esp))
        uc.reg_write(UC_X86_REG_ESP, esp + 8)

    uc.hook_add(UC_HOOK_CODE, visit)
    displacement = struct.unpack_from("<i", data, patch.base.file_offset(0x2D3880) + 1)[0]
    machine.start(0x2D3885 + displacement, args=(SLOT,))
    assert machine.read(BASE + 0x18FB0) == COUNTER
    assert machine.read(COUNTER) == old_counter
    assert uc.reg_read(UC_X86_REG_ESP) == STACK + 8
    assert uc.reg_read(UC_X86_REG_EBX) == 0x12345678
    assert uc.reg_read(UC_X86_REG_ESI) == 0x23456789
    assert ids == list(range((place + 1) << 16, ((place + 1) << 16) + consumed))


def native_binding(data, place, kits, weapons=False, world_objects=0):
    """Run the actual slot/player/inventory/weapon binders.

    Only the animation object's virtual bind method is replaced by a ret 4.
    In particular the native code, not the test, assigns the callback IDs.
    """
    machine = Machine(data)
    uc = machine.uc
    begin = active_objects(machine, BASE, world_objects)
    uc.mem_write(begin + 120 * 4, b"\xa5" * (135 * 4))
    machine.write(BASE + 0x18FB0, COUNTER)
    machine.write(COUNTER, 73)
    uc.mem_write(SLOT + 0x2C, bytes([place]))
    vtable, animation_bind = BASE + 0x32000, BASE + 0x32100
    machine.write(vtable, animation_bind)
    uc.mem_map(0x5000000, 0x6000)
    weapon_vtable = BASE + 0x33000
    machine.write(weapon_vtable + 0x70, 0x2E30F0)
    uc.mem_write(animation_bind, b"\xc2\x04\x00")
    for kit in kits:
        player = BASE + 0x34000 + kit * 0x1000
        inventory = player + 0x800
        machine.write(SLOT + kit * 4, player)
        machine.write(player + 0x10C, inventory)
        machine.write(player + 0x3A4, vtable)
        if weapons:
            for index, slot in enumerate((7, 10)):
                weapon = 0x5000000 + kit * 0x2000 + index * 0x1000
                machine.write(weapon + 0x10, weapon_vtable)
                machine.write(inventory + 0x10C + slot * 4, weapon + 0x10)
    uc.reg_write(UC_X86_REG_EAX, BASE)
    displacement = struct.unpack_from("<i", data, patch.base.file_offset(0x2D3880) + 1)[0]
    machine.start(0x2D3885 + displacement, args=(SLOT,))
    first = (place + 1) << 16
    assert machine.read(SLOT + 0x28) == BASE
    assert list(uc.mem_read(SLOT + 0xC, 2)) == [min(kits)] * 2
    for ordinal, kit in enumerate(kits):
        player = machine.read(SLOT + kit * 4)
        assert machine.read(player + 0x120) == BASE
        assert machine.read(player + 0x800 + 0x108) == BASE
        assert machine.read(player + 0x698) == first + ordinal * 2
        assert machine.read(player + 0x6B0) == first + ordinal * 2 + 1
        if weapons:
            for index in range(2):
                weapon = 0x5000000 + kit * 0x2000 + index * 0x1000
                object_id = place * 6 + ordinal * 2 + index
                assert uc.mem_read(weapon + 0xC, 1)[0] == object_id
                assert machine.read(begin + object_id * 4) == weapon
                assert machine.read(weapon + 4) == BASE
    assert machine.read(BASE + 0x18B50) == begin + max(120, world_objects) * 4
    assert uc.mem_read(begin + 120 * 4, 135 * 4) == b"\xa5" * (135 * 4)
    assert machine.read(SLOT + 0x20) == first + len(kits) * 2
    assert machine.read(BASE + 0x18FB0) == COUNTER
    assert machine.read(COUNTER) == 73
    assert uc.reg_read(UC_X86_REG_ESP) == STACK + 8


def saved_list(machine, core):
    """The list the world saves its game objects by (core+0x18914 the first,
    +0x18918 the last), walked from the first with every link checked."""
    nodes, before, node = [], 0, machine.read(core + 0x18914)
    while node:
        assert machine.read(node + 4) == before, "the link back of a saved object"
        assert len(nodes) < 1000, "the list of saved objects does not end"
        nodes.append(node)
        before, node = node, machine.read(node + 8)
    assert machine.read(core + 0x18918) == before, "the last of the saved objects"
    return nodes


def stack_items(machine, inventory, slot, bind, objects, vector):
    """An inventory item that is a stack of world objects (a grenade or a
    trap item: the objects' pointers at +0x268..+0x26C) whose vtable+0x70,
    the bind the inventory calls, is the game's `bind`."""
    item, vtable = vector + 0x100, vector + 0x400
    machine.write(vtable + 0x70, bind)
    machine.write(item, vtable)
    for index, each in enumerate(objects):
        machine.write(vector + index * 4, each)
    machine.write(item + 0x268, vector)
    machine.write(item + 0x26C, vector + len(objects) * 4)
    machine.write(inventory + 0x10C + slot * 4, item)
    return item


def saved_objects(data):
    """The game's own binders (slot 0x321820, kit 0x2D75B0, inventory
    0x2FC7D0, grenade item 0x309EC0 and grenade 0x303820, trap item
    0x3067A0) on made-up kits, through the hooked call: where each slot's
    grenades and traps are in the list the world is saved by, whenever the
    slot is bound. Only the animation object's bind is a `ret 4`."""
    machine = Machine(data)
    uc = machine.uc
    active_objects(machine, BASE)
    machine.write(BASE + 0x18FB0, COUNTER)
    machine.write(COUNTER, 73)
    uc.mem_map(0x5000000, 0x80000)
    free = [0x5000000]

    def room(size):
        at = free[0]
        free[0] += (size + 0xFFF) & ~0xFFF
        return at

    vtable = room(0x200)
    machine.write(vtable, vtable + 0x100)
    uc.mem_write(vtable + 0x100, b"\xc2\x04\x00")
    displacement = struct.unpack_from("<i", data, patch.base.file_offset(0x2D3880) + 1)[0]

    def bind(place, kits):
        """kits: {kit: (grenades, traps)}. Returns the nodes the slot's
        objects have in the list, in the order the game binds them."""
        slot = room(0x40)
        uc.mem_write(slot + 0x2C, bytes([place]))
        expected, ids, next_id = [], [], (place + 1) << 16
        for kit in sorted(kits):
            grenades, traps = kits[kit]
            player = room(0x1000)
            inventory = player + 0x800
            machine.write(slot + kit * 4, player)
            machine.write(player + 0x10C, inventory)
            machine.write(player + 0x3A4, vtable)
            # The inventory binds its slots in their order: the grenades'
            # (14) before the traps' (16).
            objects = [room(0x200) for _ in range(grenades)]
            if objects:
                stack_items(machine, inventory, 14, 0x309EC0, objects, room(0x800))
            for each in objects:
                expected.append(each + 0x10)
                ids.append((each + 0x168, next_id))
                ids.append((each + 0x180, next_id + 1))
                ids.append((each + 0x188, BASE))
                next_id += 2
            objects = [room(0x300) for _ in range(traps)]
            if objects:
                stack_items(machine, inventory, 16, 0x3067A0, objects, room(0x800))
            for each in objects:
                expected.append(each + 0x74)
                ids.append((each + 0x234, next_id))
                ids.append((each + 0x19C, BASE))
                next_id += 1
            ids.append((player + 0x698, next_id))
            ids.append((player + 0x6B0, next_id + 1))
            next_id += 2
        ids.append((slot + 0x20, next_id))
        saved = {UC_X86_REG_EBX: 0x12345678, UC_X86_REG_ESI: 0x23456789,
                 UC_X86_REG_EDI: 0x3456789A, UC_X86_REG_EBP: 0x456789AB}
        for register, value in saved.items():
            uc.reg_write(register, value)
        uc.reg_write(UC_X86_REG_EAX, BASE)
        machine.start(0x2D3885 + displacement, args=(slot,), count=100000)
        assert uc.reg_read(UC_X86_REG_ESP) == STACK + 8
        assert all(uc.reg_read(register) == value for register, value in saved.items())
        assert machine.read(BASE + 0x18FB0) == COUNTER and machine.read(COUNTER) == 73
        for address, value in ids:
            assert machine.read(address) == value, hex(address)
        return expected

    def map_object():
        """As the map's objects come into the list (0x285864..0x28588F):
        at its end."""
        node, last = room(0x40), machine.read(BASE + 0x18918)
        machine.write(node + 4, last)
        machine.write(last + 8 if last else BASE + 0x18914, node)
        machine.write(BASE + 0x18918, node)
        return node

    # A slot with nothing to save, into a list with nothing in it.
    assert bind(0, {0: (0, 0), 1: (0, 0)}) == [] and saved_list(machine, BASE) == []
    # The start of a match: its slots, then the map's objects.
    first = bind(1, {0: (2, 1), 1: (0, 2), 2: (3, 0)})
    assert len(first) == 8 and saved_list(machine, BASE) == first
    second = bind(2, {1: (1, 0)})
    assert saved_list(machine, BASE) == second + first
    of_map = [map_object() for _ in range(3)]
    assert saved_list(machine, BASE) == second + first + of_map
    # Slots bound while the match is played: in front of all of it, as a
    # game that starts with them has them.
    assert bind(3, {0: (0, 0), 2: (0, 0)}) == []
    assert saved_list(machine, BASE) == second + first + of_map
    fourth = bind(4, {0: (2, 0), 2: (2, 2)})
    assert len(fourth) == 6 and saved_list(machine, BASE) == fourth + second + first + of_map
    fifth = bind(19, {1: (0, 1)})
    assert saved_list(machine, BASE) == fifth + fourth + second + first + of_map
    # A list of the map's objects alone, and a slot with grenades after it.
    machine = Machine(data)
    uc = machine.uc
    active_objects(machine, BASE)
    machine.write(BASE + 0x18FB0, COUNTER)
    machine.write(COUNTER, 73)
    uc.mem_map(0x5000000, 0x80000)
    free[0] = 0x5000000
    vtable = room(0x200)
    machine.write(vtable, vtable + 0x100)
    uc.mem_write(vtable + 0x100, b"\xc2\x04\x00")
    of_map = [map_object() for _ in range(2)]
    late = bind(5, {0: (2, 0)})
    assert saved_list(machine, BASE) == late + of_map


def untouched_objects(data):
    """What the install puts into the kept states for each node of a late
    slot is one zero byte. Here the game's own code says so: a grenade and
    a trap as their constructors leave them (grenade_core 0x303640; the
    trap's 0x3043C0 as far as 0x3044E3, where its own fields are set) save
    themselves as that byte (0x303DA0, 0x3052E0) and load it back
    (0x303F80, 0x3053C0) reading nothing more, changed in nothing and
    calling nothing but the grenade's look for its owner. The two
    constructors of the base every game object has (0xF70F0, 0xF17E0)
    are a `ret`; a virtual call would read a vtable that is not mapped."""
    m = Machine(data)
    uc = m.uc
    uc.mem_map(0x1F1D000, 4096)      # the writers' debug flags: off
    uc.mem_map(0x5000000, 0x10000)
    grenade, trap, owner = 0x5000000, 0x5001000, 0x5004000
    stream, buffer, saved = 0x5002000, 0x5002100, 0x5003000
    owner_asked = []

    def visit(uc, address, size, _):
        if address not in (0xF70F0, 0xF17E0, STOP + 0x300):
            return
        esp = uc.reg_read(UC_X86_REG_ESP)
        if address == STOP + 0x300:
            owner_asked.append(uc.reg_read(UC_X86_REG_ECX))
            uc.reg_write(UC_X86_REG_EAX, 0)
        uc.reg_write(UC_X86_REG_EIP, m.read(esp))
        uc.reg_write(UC_X86_REG_ESP, esp + (12 if address == 0xF70F0 else 4))

    uc.hook_add(UC_HOOK_CODE, visit)

    def save(node, function):
        """The bytes the node writes: into a stream as a state has it
        (+0x14 its buffer: an allocator, the bytes, the room, the size)."""
        uc.mem_write(saved, b"\xEE" * 0x100)
        for address, value in ((stream + 0x14, buffer), (buffer, 0), (buffer + 4, saved),
                               (buffer + 8, 0x100), (buffer + 12, 0)):
            m.write(address, value)
        uc.reg_write(UC_X86_REG_ECX, node)
        m.start(function, args=(stream, 0))
        assert uc.reg_read(UC_X86_REG_ESP) == STACK + 12
        written = bytes(uc.mem_read(saved, m.read(buffer + 12)))
        assert bytes(uc.mem_read(saved + len(written), 4)) == b"\xEE" * 4
        return written

    def load(node, function, byte):
        """How many bytes the node reads of `byte` and 0xEE after it."""
        uc.mem_write(saved, bytes([byte]) + b"\xEE" * 0xFF)
        for address, value in ((stream, saved), (stream + 4, saved), (stream + 8, 0x100)):
            m.write(address, value)
        uc.reg_write(UC_X86_REG_ECX, node)
        m.start(function, args=(stream, 0, 0))
        assert uc.reg_read(UC_X86_REG_ESP) == STACK + 16
        return m.read(stream + 4) - saved

    # The grenade: its owner is asked for (the item, +0x250, +0x174, the
    # vtable's +0xC) and has no player.
    uc.reg_write(UC_X86_REG_EDI, grenade)
    m.start(0x303640)
    assert uc.reg_read(UC_X86_REG_EAX) == grenade
    for address, value in ((grenade + 0xC, owner), (owner + 0x250, owner + 0x300),
                           (owner + 0x300 + 0x174, owner + 0x600), (owner + 0x600, owner + 0x700),
                           (owner + 0x700 + 0xC, STOP + 0x300)):
        m.write(address, value)
    whole = bytes(uc.mem_read(grenade, 0x220))
    assert save(grenade + 0x10, 0x303DA0) == b"\0"
    assert load(grenade + 0x10, 0x303F80, 0) == 1 and owner_asked == [owner + 0x600]
    assert bytes(uc.mem_read(grenade, 0x220)) == whole
    # The trap.
    m.start(0x3043C0, end=0x3044E3, args=(trap, 0x1234))
    whole = bytes(uc.mem_read(trap, 0x270))
    assert save(trap + 0x74, 0x3052E0) == b"\0"
    assert load(trap + 0x74, 0x3053C0, 0) == 1 and len(owner_asked) == 1
    assert bytes(uc.mem_read(trap, 0x270)) == whole


def native_histories(data):
    """Construct all four vectors through the original engine functions."""
    machine = Machine(data)
    uc = machine.uc
    # Native pool constructors inspect their initially-zero diagnostic flags.
    uc.mem_map(0x2260000, 4096)
    vectors = []
    for offset, stride, constructor in ((0x2FCC, 0x5A0C, 0x2D41B0),
                                        (0xE3E4, 0x1F4C, 0x2D4330)):
        for bank in range(2):
            vector = BASE + offset + bank * stride
            uc.reg_write(UC_X86_REG_ECX, vector)
            machine.start(constructor)
            assert machine.read(vector) == vector + 12
            assert machine.read(vector + 8) == vector + stride
            vectors.append(vector)
    uc.reg_write(UC_X86_REG_EBP, BASE)
    machine.start(0x2D3906, end=0x2D395F, count=100000)
    for vector in vectors:
        assert machine.read(vector + 4) == machine.read(vector + 8)


def native_profile(data):
    """Validate, then execute the original profile constructor and reader.

    Only CRT memcpy is intercepted; dictionary lookup and inventory vector
    construction/readers run natively, including their inline-pointer setup.
    The wire fixture ends at an unmapped page to detect even one-byte overreads.
    """
    m = Machine(data)
    uc = m.uc
    dictionary, rows, mods, defects = BASE + 0x8000, BASE + 0x9000, BASE + 0xB000, BASE + 0xB100
    m.write(dictionary + 0x130, rows)
    m.write(dictionary + 0x134, 2)
    uc.mem_write(rows, struct.pack("<H", 101))
    uc.mem_write(rows + 0x3A0, struct.pack("<H", 102))
    uc.mem_write(rows + 0x3A0 + 0x22A, b"\x01")
    m.write(dictionary + 0x140, mods)
    m.write(dictionary + 0x144, 1)
    m.write(mods, 1230)
    m.write(dictionary + 0x148, defects)
    m.write(dictionary + 0x14C, 1)
    m.write(defects, 7)
    uc.mem_map(0x2260000, 4096)
    wire_page = 0x4000000
    uc.mem_map(wire_page, 0x4000)
    constructed = []

    def visit(uc, address, size, _):
        if address == 0x2F7C50:
            constructed.append(uc.reg_read(UC_X86_REG_ECX))
        if address == 0x424580:  # memcpy, cdecl(dest, source, size)
            esp = uc.reg_read(UC_X86_REG_ESP)
            dest, source, length = (m.read(esp + i) for i in (4, 8, 12))
            if length:
                uc.mem_write(dest, bytes(uc.mem_read(source, length)))
            uc.reg_write(UC_X86_REG_EAX, dest)
            uc.reg_write(UC_X86_REG_EIP, m.read(esp))
            uc.reg_write(UC_X86_REG_ESP, esp + 4)

    uc.hook_add(UC_HOOK_CODE, visit)
    cave = patch.base.LATE_JOIN_VA
    sentinel = b"\xa5" * 0x45FC

    def run(wire, valid):
        source = wire_page + 0x4000 - len(wire)
        if wire:
            uc.mem_write(source, wire)
        uc.mem_write(BASE, sentinel)
        constructed.clear()
        saved = [(UC_X86_REG_EBX, 0x1234), (UC_X86_REG_ESI, 0x2345),
                 (UC_X86_REG_EDI, 0x3456), (UC_X86_REG_EBP, 0x4567)]
        for reg, value in saved:
            uc.reg_write(reg, value)
        m.start(cave + 192, args=(source, len(wire), BASE, dictionary), count=200000)
        assert uc.reg_read(UC_X86_REG_EAX) == int(valid), (len(wire), wire.hex())
        assert uc.reg_read(UC_X86_REG_ESP) == STACK + 4
        assert all(uc.reg_read(reg) == value for reg, value in saved)
        assert constructed == ([BASE] if valid else [])
        if not valid:
            assert bytes(uc.mem_read(BASE, len(sentinel))) == sentinel

    # Two kits, both item layouts, modification/defect tables and an upgrade.
    wire = bytearray(b"\x01\x03new\x02SR" + struct.pack("<I", 10) + b"\x02\x02")
    kit_offsets = []
    for kit in range(2):
        kit_offsets.append(len(wire))
        wire += bytes([0]) + struct.pack("<I", kit + 1) + b"\x01K"
        wire += struct.pack("<IHHHH", 3, 0, 1, 1, 9)
        wire += struct.pack("<IIIIf", 8, 0, 0, 0, 1.25)  # modifier 67
    item_offsets = []
    for index in range(2):
        item_offsets.append(len(wire))
        wire += struct.pack("<IHI", index + 1, 101 + index, 99)
        if not index:
            wire += struct.pack("<II", 2, 3)
        wire += b"\x02" + struct.pack("<IBIB", 1230, 0, 7, 1)
        wire += b"\x01" + struct.pack("<IIQB", 31, 32, 33, 3)
    wire = bytes(wire)
    run(wire, True)
    assert m.read(BASE + 0x45EC) == 1
    assert bytes(uc.mem_read(BASE + 8, 4)) == b"new\0"
    assert bytes(uc.mem_read(BASE + 0x48, 3)) == b"SR\0"
    assert uc.mem_read(BASE + 0x57, 1)[0] == 2
    for kit in range(2):
        assert m.read(BASE + 0xF4 + kit * 0x704 + 67 * 4) == 0x3FA00000
    for index in range(69):
        entry = BASE + 0x1568 + index * 0xB4
        assert m.read(entry + 0x14) == entry + 0x20
        assert m.read(entry + 0x18) == entry + 0x20 + (16 if index < 2 else 0)
        assert m.read(entry + 0x60) == entry + 0x6C
        assert m.read(entry + 0x64) == entry + 0x6C + (24 if index < 2 else 0)
    for length in range(len(wire)):
        run(wire[:length], False)
    run(wire + b"\0", False)
    for offset, replacement in (
        (0, b"\x02"), (1, b"\x40"), (5, b"\x0f"),
        (12, b"\x04"), (13, b"\x46"),
        (kit_offsets[0] + 7, struct.pack("<I", 1 << 23)),
        (kit_offsets[0] + 11, b"\x02\0"),
        (kit_offsets[0] + 19, struct.pack("<I", 16)),
        (kit_offsets[0] + 23, struct.pack("<I", 1)),
        (kit_offsets[0] + 35, struct.pack("<I", 0x7F800000)),
        (item_offsets[0] + 4, b"\xff\xff"),
        (item_offsets[0] + 18, b"\x09"),
        (item_offsets[0] + 19, struct.pack("<I", 9999)),
        (item_offsets[0] + 23, b"\x02"),
        (item_offsets[0] + 29, b"\x04"),
    ):
        invalid = bytearray(wire)
        invalid[offset:offset + len(replacement)] = replacement
        run(bytes(invalid), False)
    # No usable kit, or an occupied slot referring to an empty item.
    invalid = bytearray(wire)
    for offset in kit_offsets:
        invalid[offset] = 2
    run(bytes(invalid), False)
    invalid = wire[:item_offsets[0] + 4] + b"\0\0" + wire[item_offsets[1]:]
    run(invalid, False)

    maximum = bytearray(b"\0\x3f" + b"N" * 63 + b"\x0e" + b"C" * 14)
    maximum += struct.pack("<I", 10) + b"\x03\x45"
    for kit in range(3):
        maximum += b"\0" + struct.pack("<I", kit + 1) + b"\x3f" + b"K" * 63
        maximum += struct.pack("<I", (1 << 23) - 1)
        for slot in range(23):
            maximum += struct.pack("<HH", kit * 23 + slot, 1)
        maximum += struct.pack("<IIII", 15, 0, 0xFFFFFFFF, 0xFFFFFFFF)
        maximum += struct.pack("<68f", *([1.0] * 68))
    for index in range(69):
        maximum += struct.pack("<IHI", index + 1, 101 + index % 2, 99)
        if index % 2 == 0:
            maximum += struct.pack("<II", 2, 3)
        maximum += b"\x08" + struct.pack("<IBIB", 1230, 0, 7, 1) * 4
        maximum += b"\x03" + struct.pack("<IIQB", 31, 32, 33, 3) * 3
    run(bytes(maximum), True)
    last = BASE + 0x1568 + 68 * 0xB4
    assert m.read(last + 0x18) == last + 0x60
    assert m.read(last + 0x64) == last + 0xB4
    for cut in (1, 17, 52):
        run(bytes(maximum[:-cut]), False)

    # The C validator is independently position-independent. Exercise a
    # copied entry, distant from both its linked address and the native exe.
    moved = 0x5000000
    section = next(s for s in patch.base.Headers(data).sections(data)
                   if patch.base.IMAGE_BASE + s[1] == patch.base.SECTION_VA)
    _, _, _, raw, raw_size, _ = section
    offset = patch.base.LATE_JOIN_OFFSET + patch.ASM_SIZE
    code = bytes(data[raw + offset:raw + raw_size])
    uc.mem_map(moved, (len(code) + 4095) & ~4095)
    uc.mem_write(moved, code)
    for body, valid in ((wire, True), (wire[:-1], False), (bytes(maximum), True)):
        source = wire_page + 0x4000 - len(body)
        uc.mem_write(source, body)
        m.start(moved, args=(source, len(body), dictionary), count=200000)
        assert uc.reg_read(UC_X86_REG_EAX) == int(valid)


def native_query(data):
    """Execute native callback construction and our resource-query adapter.

    Query scheduling and C completion are intercepted at their ABI boundary;
    this checks marshaling, not that the resource engine loads real assets.
    """
    cave = patch.base.LATE_JOIN_VA
    for mask in range(1, 8):
        m = Machine(data)
        uc = m.uc
        uc.mem_map(0x2260000, 4096)
        m.write(0x2260A58, 7)
        uc.mem_map(0x1F1D000, 4096)
        m.write(0x1F1D9C4, 0x12340000)
        m.write(BASE + 44, mask)
        m.write(BASE + 24, 19)
        callbacks = []
        completions = []
        callback = cave + 768
        code = bytes(uc.mem_read(callback, 128))
        call_at = callback + code.index(b"\x50\x56\xe8") + 2
        complete = call_at + 5 + struct.unpack("<i", uc.mem_read(call_at + 1, 4))[0]
        expected_players = [BASE + 0x10000 + i * 0x1000 if mask & (1 << i) else 0 for i in range(3)]

        def visit(uc, address, size, _):
            esp = uc.reg_read(UC_X86_REG_ESP)
            if address == 0xE5B60:
                requests = uc.reg_read(UC_X86_REG_EAX)
                count = uc.reg_read(UC_X86_REG_ECX)
                assert count == mask.bit_count()
                cb, allocator, variants, parent, asynchronous = (m.read(esp + i) for i in (4, 8, 12, 16, 20))
                assert (allocator, parent, asynchronous) == (0x12340000, 0, 1)
                assert m.read(cb) == 0xA09375
                assert m.read(cb + 8) == callback
                assert m.read(cb + 12) == 0
                assert m.read(cb + 16) == BASE
                assert m.read(cb + 20) == 0
                uc.mem_write(BASE + 0x30000, bytes(uc.mem_read(cb + 8, 16)))
                for n, kit in enumerate(i for i in range(3) if mask & (1 << i)):
                    assert m.read(requests + n * 8) == 0xA09F48
                    assert m.read(requests + n * 8 + 4) == 0x54
                    variant = m.read(variants + n * 4)
                    assert m.read(variant) == 0xA09F90
                    assert m.read(variant + 8) == BASE + 60
                    assert m.read(variant + 12) == 19
                    assert m.read(variant + 16) == 0x11110000
                    assert m.read(variant + 20) == 0x22220000
                    assert m.read(variant + 24) == 0x33330000
                    assert m.read(variant + 28) == kit
                    assert m.read(variant + 40) == variant
                    assert m.read(variant + 44) == 7
                callbacks.append(callback)
                uc.reg_write(UC_X86_REG_EAX, 77)
                uc.reg_write(UC_X86_REG_EIP, m.read(esp))
                uc.reg_write(UC_X86_REG_ESP, esp + 4)
            elif address == complete:
                assert m.read(esp + 4) == BASE
                players = m.read(esp + 8)
                assert [m.read(players + 4 * i) for i in range(3)] == expected_players
                completions.append(1)
                uc.reg_write(UC_X86_REG_EIP, m.read(esp))
                uc.reg_write(UC_X86_REG_ESP, esp + 4)

        uc.hook_add(UC_HOOK_CODE, visit)
        saved = [(UC_X86_REG_EBX, 0x1234), (UC_X86_REG_ESI, 0x2345),
                 (UC_X86_REG_EDI, 0x3456), (UC_X86_REG_EBP, 0x4567)]
        for reg, value in saved:
            uc.reg_write(reg, value)
        m.start(cave + 320, args=(BASE, 0x11110000, 0x22220000, 0x33330000))
        assert callbacks == [callback] and uc.reg_read(UC_X86_REG_EAX) == 77
        assert uc.reg_read(UC_X86_REG_ESP) == STACK + 4
        assert all(uc.reg_read(reg) == value for reg, value in saved)
        results = BASE + 0x20000
        for n, player in enumerate(p for p in expected_players if p):
            m.write(results + 0x12C + 0x2E0 * n, player)
        # The original boost invoker selected by vtable A09374, including
        # its member-this adjustment and stdcall callback cleanup.
        m.start(0x202300, args=(BASE + 0x30000, results))
        assert completions == [1]
        assert uc.reg_read(UC_X86_REG_ESP) == STACK + 4
        assert all(uc.reg_read(reg) == value for reg, value in saved)
    print("  driven: query_completed through the game's boost invoker 0x202300 (masks 1..7)")


def animation_table(m, table):
    head = table + 0xE034
    for at, value in ((head, 0), (head + 4, head), (head + 8, head),
                      (table + 0xE044, table + 0xE050),
                      (table + 0xE048, table + 0xE050),
                      (table + 0xE04C, table + 0x10050)):
        m.write(at, value)


def native_animations(data):
    """Native name comparison/append with incremental and repeated kit loads."""
    for order in ((0, 1, 2, 3, 4, 5), (3, 5, 1, 4, 0, 2)):
        m = Machine(data)
        uc = m.uc
        animation_table(m, BASE)
        entries = [BASE + 0x20000 + i * 128 for i in order]
        names = [(b"zeta", b"a"), (b"alpha", b"z"), (b"aardvark", b"a"),
                 (b"beta", b"a"), (b"alpha", b"b"), (b"alpha", b"a")]
        for entry, name in zip(entries, names):
            resource = entry + 32
            m.write(entry, resource)
            m.write(entry + 4, resource + 32)
            m.write(entry + 8, 0xCCCCCCCC)
            m.write(resource, resource + 4)
            uc.mem_write(resource + 4, name[0] + b"\0")
            m.write(resource + 32, resource + 36)
            uc.mem_write(resource + 36, name[1] + b"\0")
        def visit(uc, address, size, _):
            esp = uc.reg_read(UC_X86_REG_ESP)
            if address == 0x2D8100:
                uc.reg_write(UC_X86_REG_EIP, m.read(esp))
                uc.reg_write(UC_X86_REG_ESP, esp + 8)
            elif address == 0xF7200:
                result = uc.reg_read(UC_X86_REG_ECX)
                m.write(result, m.read(uc.reg_read(UC_X86_REG_EAX)))
                uc.reg_write(UC_X86_REG_EAX, result)
                uc.reg_write(UC_X86_REG_EIP, m.read(esp))
                uc.reg_write(UC_X86_REG_ESP, esp + 4)
        uc.hook_add(UC_HOOK_CODE, visit)
        for count, expected in ((2, [1, 0]), (4, [1, 0, 2, 3]), (6, [1, 0, 2, 3, 5, 4]),
                                (6, [1, 0, 2, 3, 5, 4])):
            head = BASE + 0xE034
            nodes = sorted(entry + 12 for entry in entries[:count])
            m.write(head, nodes[0])
            m.write(head + 4, nodes[0])
            m.write(head + 8, nodes[-1])
            for i, node in enumerate(nodes):
                m.write(node, head if i == 0 else nodes[i - 1])
                m.write(node + 4, 0)
                m.write(node + 8, nodes[i + 1] if i + 1 < count else 0)
            cave = patch.base.LATE_JOIN_VA
            m.start(cave + 3072, args=(BASE,), count=100000)
            assert uc.reg_read(UC_X86_REG_ESP) == STACK + 8
            assert m.read(BASE + 0xE048) == BASE + 0xE050 + count * 4
            assert [m.read(BASE + 0xE050 + i * 4) for i in range(count)] == [entries[i] for i in expected]
            assert [m.read(entries[index] + 8) for index in expected] == list(range(count))


def native_preparation(data, grenades=0, traps=0):
    """Exercise the composed ready -> E4 -> query -> install -> ACK -> teardown.

    Native profile reading, callback invocation, slot/player/inventory binding
    and the compiled C lifecycle run. Resource scheduling, allocation, packet
    transport, animation/physics/UI setup and player destruction are intercepted.
    This is an integration fixture, not a live resource-engine test.

    The newcomer's kit has `grenades` grenades and `traps` traps, bound by
    the game's own code; the world has two saved objects already and four
    states to roll back to.
    """
    m = Machine(data)
    uc = m.uc
    cave = patch.base.LATE_JOIN_VA
    state = patch.base.IMAGE_BASE + next(rva for name, rva, _, _, _, _ in patch.base.Headers(data).sections(data)
                                         if name == patch.base.LATE_JOIN_STATE_NAME)
    nc, game, mc, options = 0x7000000, 0x8000000, 0x9000000, 0x6000000
    for address, size in ((nc, 0x1F3000), (game, 0x6000), (mc, 0x59000), (options, 0x60000),
                          (0x2260000, 4096), (0x1F1D000, 4096)):
        uc.mem_map(address, size)
    animations = 0xA000000
    uc.mem_map(animations, 0x11000)
    animation_table(m, animations)
    pvp, core = BASE, BASE + 0x1F8
    active_objects(m, core, 130)
    context, statistics, dictionary = BASE + 0x20000, BASE + 0x25000, BASE + 0x2C000
    reader, wire_at, results = BASE + 0x2D000, BASE + 0x2E000, BASE + 0x2F000
    player, inventory = BASE + 0x34000, BASE + 0x34800
    vtable, animation_bind, packet, pool = BASE + 0x38000, BASE + 0x38100, BASE + 0x3A000, BASE + 0x3B000
    transport_vtable, functor = BASE + 0x3C000, BASE + 0x3D000
    seed = 0x80000009
    begin = core + 0x18464
    slot = begin + 2 * 48
    for address, value in (
        (nc + 0x14, game), (nc + 0x1F1C40, pvp), (nc + 0x1F1C48, mc),
        (nc + 0x1F1C64, 2), (game + 0x53A4, dictionary), (pvp + 0x124, options),
        (pvp + 0xD0, 1), (core + 0x18458, begin), (core + 0x1845C, slot),
        (core + 0x18460, begin + 20 * 48), (core + 0x18FA8, statistics),
        (core + 0x18FB0, COUNTER), (COUNTER, 73), (core + 0x19034, 0xBAD00000),
        (pvp + 0x130, 0x11223344), (pvp + 0x19238, animations),
        (options + 0x577B0, seed), (mc + 0x577BC, seed),
        (mc, transport_vtable), (mc + 0x578B0, pool), (pool + 0x34, 256),
        (transport_vtable + 0x18, STOP + 0x100), (transport_vtable + 0x1C, STOP + 0x200),
        (0x2260A58, 7), (0x1F1D9C4, 0x12340000),
        (player + 0x10C, inventory), (player + 0x3A4, vtable),
        (vtable, animation_bind), (player + 0xD0, 1),
    ):
        m.write(address, value)
    uc.mem_write(animation_bind, b"\xc2\x04\x00")
    # The newcomer's grenades and traps, as stacks in its inventory.
    carried = [BASE + 0x1A000 + index * 0x400 for index in range(grenades + traps)]
    if grenades:
        stack_items(m, inventory, 14, 0x309EC0, carried[:grenades], BASE + 0x35000)
    if traps:
        stack_items(m, inventory, 16, 0x3067A0, carried[grenades:], BASE + 0x36000)
    new_nodes = [each + 0x10 for each in carried[:grenades]] + [each + 0x74 for each in carried[grenades:]]
    # What the list of saved objects holds: an older slot's and the map's.
    older, of_map = BASE + 0x1F000, BASE + 0x1F100
    for address, value in ((core + 0x18914, older), (core + 0x18918, of_map),
                           (older + 8, of_map), (of_map + 4, older)):
        m.write(address, value)
    # The states the game can roll back to, oldest first: the game-objects
    # bytes of each, in a buffer of 0x2000 filled with a pattern. The third
    # is all but full: no late slot's bytes go in.
    states = 0xB000000
    uc.mem_map(states, 0x400000)
    saved = (bytes(range(1, 8)), b"", bytes(index % 251 + 1 for index in range(0x1FFE)),
             bytes([0x44, 0x04, 0x99]))
    items = [states + index * 0x100000 for index in range(len(saved))]
    filler = bytes(0xC0 + index % 61 for index in range(0x2000))
    for index, (item, blob) in enumerate(zip(items, saved)):
        uc.mem_write(item + 0xBBF30, blob + filler[len(blob):])
        for offset, value in ((0xBDF30, 0), (0xBDF34, item + 0xBBF30), (0xBDF38, 0x2000),
                              (0xBDF3C, len(blob)), (0xC0BA8, items[index - 1] if index else 0),
                              (0xC0BAC, items[index + 1] if index + 1 < len(items) else 0)):
            m.write(item + offset, value)
    m.write(core + 0x2FB8, len(items))
    m.write(core + 0x2FBC, items[0])
    m.write(core + 0x2FC0, items[-1])
    uc.mem_write(options + 0x577B5, b"\x02")
    uc.mem_write(statistics + 8 + 0x31E0, b"\x02")
    uc.mem_write(statistics + 0x31E0, b"\x5a")
    histories_at = core + 0x2FCC
    histories = b"\x7d" * (0x18458 - 0x2FCC)
    uc.mem_write(histories_at, histories)
    body = b"\x01\x03new\0" + struct.pack("<I", 10) + b"\x01\0"
    body += b"\0" + struct.pack("<I", 1) + b"\0" + bytes(20)
    payload = b"\xffLJ\x01" + struct.pack("<II", seed, 123) + b"\x02\x07" + body
    uc.mem_write(wire_at, payload)
    for offset, value in ((0, wire_at), (4, wire_at), (8, len(payload))):
        m.write(reader + offset, value)
    calls, messages = [], []

    def finish(pop=0, value=None):
        esp = uc.reg_read(UC_X86_REG_ESP)
        if value is not None:
            uc.reg_write(UC_X86_REG_EAX, value)
        uc.reg_write(UC_X86_REG_EIP, m.read(esp))
        uc.reg_write(UC_X86_REG_ESP, esp + 4 + pop)

    def visit(uc, address, size, _):
        esp = uc.reg_read(UC_X86_REG_ESP)
        if address == 0xE4F90:
            assert uc.reg_read(UC_X86_REG_ESI) == 0x12340000
            assert uc.reg_read(UC_X86_REG_EAX) == 0x463C + len(body)
            calls.append("allocate")
            finish(4, context)
        elif address == 0x1539D0:
            assert uc.reg_read(UC_X86_REG_EAX) == context
            assert uc.reg_read(UC_X86_REG_ECX) == 0x12340000
            m.write(m.read(esp + 4), 0)
            calls.append("free")
            finish()
        elif address == 0x424580:
            dest, source, count = (m.read(esp + i) for i in (4, 8, 12))
            if count:
                uc.mem_write(dest, bytes(uc.mem_read(source, count)))
            finish(value=dest)
        elif address == 0xE5B60:
            assert uc.reg_read(UC_X86_REG_ECX) == 1
            cb, variants = m.read(esp + 4), m.read(esp + 12)
            variant = m.read(variants)
            assert m.read(variant + 8) == context + 60
            assert m.read(variant + 16) == game + 0x18
            assert m.read(variant + 20) == 0x11223344
            assert m.read(variant + 24) == dictionary
            uc.mem_write(functor, bytes(uc.mem_read(cb + 8, 16)))
            calls.append("query")
            finish(value=77)
        elif address == 0x2D8100:
            assert uc.reg_read(UC_X86_REG_EDI) == player
            assert m.read(esp + 4) == animations
            calls.append("animations")
            finish(4)
        elif address == 0x3218A0:
            assert uc.reg_read(UC_X86_REG_EAX) == pvp + 0x138
            assert m.read(esp + 4) == slot
            calls.append("physics")
            finish(4)
        elif address == 0x1F29D0:
            assert uc.reg_read(UC_X86_REG_ESI) == player
            calls.append("ready")
            finish()
        elif address == STOP + 0x100:
            assert uc.reg_read(UC_X86_REG_ECX) == mc and m.read(esp + 4) == 0xE0
            uc.mem_write(packet + 0x98, b"\x60")
            calls.append("new_packet")
            finish(4, packet)
        elif address == 0x7AC10:
            assert uc.reg_read(UC_X86_REG_EDI) == 20
            assert m.read(esp + 4) == packet + 0x2A8
            messages.append(bytes(uc.mem_read(m.read(esp + 8), 20)))
            finish(8)
        elif address == STOP + 0x200:
            assert uc.reg_read(UC_X86_REG_ECX) == mc and m.read(esp + 4) == packet
            assert uc.mem_read(packet + 0x98, 1)[0] == 0x20
            calls.append("enqueue")
            finish(4)
        elif address == 0x1F2100:
            assert uc.reg_read(UC_X86_REG_ECX) == player
            assert bytes(uc.mem_read(context + 60 + 8, 4)) == b"new\0"
            calls.append("destroy")
            finish()

    uc.hook_add(UC_HOOK_CODE, visit)
    uc.reg_write(UC_X86_REG_EBX, nc)
    m.start(0x27AEA1, end=patch.base.SECTION_VA, count=200000)
    assert m.read(state + 40) == nc and m.read(state + 56) == 2
    uc.reg_write(UC_X86_REG_ESI, nc)
    uc.reg_write(UC_X86_REG_EBX, reader)
    m.start(0x27A378, end=0x27A5AB, count=200000)
    assert calls == ["allocate", "query"] and not messages
    assert m.read(pvp + 0xD0) == 2 and m.read(core + 0x1845C) == slot
    assert bytes(uc.mem_read(histories_at, len(histories))) == histories
    m.write(player + 0x6B8, context + 60)
    m.write(results + 0x12C, player)
    m.start(0x202300, args=(functor, results), count=200000)
    assert calls == ["allocate", "query", "animations", "physics", "ready", "new_packet", "enqueue"]
    assert m.read(pvp + 0xD0) == 1 and m.read(player + 0xD0) == 2
    assert m.read(core + 0x1845C) == slot + 48 and m.read(slot) == player
    taken = grenades * 2 + traps
    assert m.read(slot + 0x28) == core and m.read(slot + 0x20) == 0x30002 + taken
    assert m.read(player + 0x698) == 0x30000 + taken and m.read(player + 0x6B0) == 0x30001 + taken
    # The newcomer's objects are in front of what the list held, in the
    # order the game bound them, and each took its ids of the place's.
    assert saved_list(m, core) == new_nodes + [older, of_map]
    for index, each in enumerate(carried[:grenades]):
        assert [m.read(each + offset) for offset in (0x168, 0x180, 0x188)] == [
            0x30000 + index * 2, 0x30001 + index * 2, core]
    for index, each in enumerate(carried[grenades:]):
        assert [m.read(each + offset) for offset in (0x234, 0x19C)] == [0x30000 + grenades * 2 + index, core]
    # Every state it can roll back to has one zero byte for each of them in
    # front of its game-objects bytes; the one with no room is as it was.
    added = len(new_nodes)
    for item, blob in zip(items, saved):
        fits = len(blob) + added <= 0x2000
        now = bytes(added) + blob if fits else blob
        assert m.read(item + 0xBDF3C) == len(now), (hex(item), m.read(item + 0xBDF3C))
        assert bytes(uc.mem_read(item + 0xBBF30, 0x2000)) == now + filler[len(now):]
        assert [m.read(item + offset) for offset in (0xBDF30, 0xBDF34, 0xBDF38)] == [0, item + 0xBBF30, 0x2000]
    assert [m.read(core + offset) for offset in (0x2FB8, 0x2FBC, 0x2FC0)] == [len(items), items[0], items[-1]]
    assert m.read(core + 0x18FB0) == COUNTER and m.read(COUNTER) == 73
    assert bytes(uc.mem_read(histories_at, len(histories))) == histories
    assert m.read(state + 56) == 3 and uc.mem_read(options + 0x577B5, 1)[0] == 3
    assert uc.mem_read(statistics + 8 + 0x31E0, 1)[0] == 3
    assert uc.mem_read(statistics + 0x31E0, 1)[0] == 0x5A
    assert uc.mem_read(options + 0x5782C + 2, 1)[0] == 7
    assert bytes(uc.mem_read(options + 2 * 0x45FC + 8, 4)) == b"new\0"
    expected_ack = b"\xff" * 6 + b"\0\0" + struct.pack("<II", seed, 123) + b"\x02\0\x01\0"
    assert messages == [expected_ack]
    uc.reg_write(UC_X86_REG_ESI, nc)
    uc.reg_write(UC_X86_REG_EBX, reader)
    m.start(0x27A378, end=0x27A5AB, count=200000)
    assert messages == [expected_ack] * 2 and calls.count("query") == 1
    uc.reg_write(UC_X86_REG_EDI, nc)
    m.start(0x27B517, end=0x27B51D, count=200000)
    assert m.read(state + 40) == 0 and "free" not in calls
    uc.reg_write(UC_X86_REG_ECX, player)
    m.start(0x1F2013, end=0x1F2018, count=200000)
    assert calls[-2:] == ["destroy", "free"] and m.read(state + 4) == 0


def native_direct_match(data, session):
    for spectator in (0, 1):
        m = Machine(data)
        m.start(0x27a1d1, end=0x27a1d8)
        nc = BASE + 0x10000
        m.uc.mem_map((nc + 0x1ee7ec) & ~4095, 4096)
        m.write(BASE + 0x53ac, nc)
        saved = {UC_X86_REG_EDI: BASE, UC_X86_REG_EAX: nc,
                 UC_X86_REG_EBX: 0x12345678, UC_X86_REG_EBP: 0x87654321}
        for reg, value in saved.items():
            m.uc.reg_write(reg, value)
        calls = []

        def visit(uc, address, size, _):
            if address not in (0x1ed6e0, 0x21beb0):
                return
            esp = uc.reg_read(UC_X86_REG_ESP)
            count = 3 if address == 0x1ed6e0 else 4
            args = [m.read(esp + 4 + 4 * i) for i in range(count)]
            if count == 3:
                assert args == [BASE + 0x8000, 2249, spectator]
                assert uc.reg_read(UC_X86_REG_EDI) == BASE
                assert uc.reg_read(UC_X86_REG_EAX) == nc
            else:
                assert args == [BASE + 0x8000, 2249, 1, session]
                assert uc.reg_read(UC_X86_REG_ECX) == nc
                assert m.read(nc + 0x1ee7ec) == session
            calls.append(address)
            uc.reg_write(UC_X86_REG_EAX, 0xdeadbeef)
            uc.reg_write(UC_X86_REG_EIP, m.read(esp))
            uc.reg_write(UC_X86_REG_ESP, esp + 4 * (count + 1))

        m.uc.hook_add(UC_HOOK_CODE, visit)
        address = 0x1ec0e1
        assert m.uc.mem_read(address, 1) == b"\xe8"
        target = address + 5 + struct.unpack("<i", m.uc.mem_read(address + 1, 4))[0]
        m.start(target, args=(BASE + 0x8000, 2249, spectator))
        assert calls == ([0x1ed6e0] if spectator else [0x1ed6e0, 0x21beb0])
        assert m.uc.reg_read(UC_X86_REG_ESP) == STACK + 16
        for reg, value in saved.items():
            assert m.uc.reg_read(reg) == value


HANDLER, CORE, OPTIONS, FUNCTION = 0x4000000, 0x4010000, 0x4040000, 0x40B0000
HANDLER_VTABLE = 0xA0755C


def map_constants(machine, data, addresses):
    """Map, from the file, the pages of these addresses outside .text."""
    sections = patch.base.Headers(data).sections(data)
    for address in addresses:
        page = address & ~4095
        for _, rva, size, raw, raw_size, _ in sections:
            start = patch.base.IMAGE_BASE + rva
            if start <= page < start + max(size, raw_size):
                try:
                    machine.uc.mem_map(page, 4096)
                except Exception:  # mapped already (an executable section)
                    break
                at = raw + page - start
                machine.uc.mem_write(page, bytes(data[at:at + min(4096, max(0, raw_size - (page - start)))]))
                break


def scoreboard(data, roster, newcomer, rollbacks):
    """The game's own base_game_statistics_handler (patch_026g.py,
    STATISTICS_COUNT): start_match through its vtable slot +3Ch, a newcomer
    the way lj_native_install adds it (the roster and the shared live count
    raised), one tick in which a starting place and the newcomer each kill,
    told through the shared statistics' callback (0x26DEA0, slot +44h, as
    shared_statistics does after adding the kill to the record), then
    `rollbacks` times: the state before that tick loaded through deserialize
    (slot +34h; the shared statistics' reader intercepted, the records put
    back) and the tick simulated again. The hud is a function that keeps
    what it is told, its cells first holding a score of an earlier match.
    Returns (scores kept, the handler's count, places told at the start,
    the hud's cells)."""
    machine = Machine(data)
    uc = machine.uc
    map_constants(machine, data, (HANDLER_VTABLE, 0xA70884, 0xA2D774, 0x927A44))
    uc.mem_map(HANDLER, 0x4000)
    uc.mem_map(CORE, 0x1A000)
    uc.mem_map(OPTIONS, 0x58000)
    uc.mem_map(FUNCTION, 0x2000)
    invoker = FUNCTION + 0x1000
    uc.mem_write(invoker, b"\xc3")                     # cdecl (functor, place, score): ret
    machine.write(FUNCTION + 4, invoker)              # boost::function's invoker
    machine.write(HANDLER, HANDLER_VTABLE)
    machine.write(HANDLER + 0x3218, FUNCTION)          # the hud's score-changed callback
    uc.mem_write(OPTIONS + 0x577B5, bytes([roster]))
    weights = [0.0] * 28
    weights[0], weights[15] = 10.0, 2.5                 # kill, headshot kill
    uc.mem_write(OPTIONS + 0x577BC, struct.pack("<28f", *weights))
    hud, told = {place: 953 for place in range(20)}, []

    def visit(uc, address, size, _):
        if address == invoker:
            esp = uc.reg_read(UC_X86_REG_ESP)
            assert machine.read(esp + 4) == HANDLER + 0x3220
            place, score = machine.read(esp + 8) & 0xFF, machine.read(esp + 12) & 0xFFFF
            hud[place] = score
            told.append(place)
        elif address == 0x2F5BE0:                       # shared_statistics::deserialize
            assert uc.reg_read(UC_X86_REG_ESI) == HANDLER + 8
            uc.mem_write(HANDLER + 8, saved)
            esp = uc.reg_read(UC_X86_REG_ESP)
            uc.reg_write(UC_X86_REG_EIP, machine.read(esp))
            uc.reg_write(UC_X86_REG_ESP, esp + 4)

    uc.hook_add(UC_HOOK_CODE, visit)
    slot = lambda offset: machine.read(HANDLER_VTABLE + offset)

    def run(address, args):
        uc.reg_write(UC_X86_REG_ECX, HANDLER)
        machine.start(address, args=args, count=200000)
        assert uc.reg_read(UC_X86_REG_ESP) == STACK + 4 + 4 * len(args)

    run(slot(0x3C), (CORE, OPTIONS))                   # start_match
    assert machine.read(CORE + 0x18FA8) == HANDLER
    assert uc.mem_read(HANDLER + 0x31E8, 1)[0] == roster   # the shared live count: the roster
    at_start = list(told)
    # The late join (late_join_install_026g.c): the roster and the shared
    # statistics' live count take the newcomer.
    uc.mem_write(OPTIONS + 0x577B5, bytes([newcomer + 1]))
    uc.mem_write(HANDLER + 8 + 0x31E0, bytes([newcomer + 1]))
    saved = bytes(uc.mem_read(HANDLER + 8, 20 * 0x1F8))

    def tick():
        for place, event in ((0, 0), (newcomer, 0), (newcomer, 15)):
            record = HANDLER + 8 + place * 0x1F8 + event * 6
            count = struct.unpack("<H", uc.mem_read(record, 2))[0]
            uc.mem_write(record, struct.pack("<H", count + 1))
            uc.reg_write(UC_X86_REG_ECX, HANDLER)
            machine.start(0x26DEA0, args=(place, event, 1), count=200000)
            assert uc.reg_read(UC_X86_REG_ESP) == STACK + 16   # on_event: ret 0Ch

    tick()
    for _ in range(rollbacks):
        run(slot(0x34), (0x12345678, 0))               # deserialize: the state before the tick
        tick()
    scores = list(struct.unpack("<20H", uc.mem_read(HANDLER + 0x31F0, 40)))
    # Past the start the hud is told of the two places that score alone.
    assert set(told[len(at_start):]) <= {0, newcomer}, told[len(at_start):]
    return scores, uc.mem_read(HANDLER + 0x3240, 1)[0], at_start, hud


def statistics_scores(original, changed):
    """A newcomer's scoreboard score in the original and in the build."""
    for roster, newcomer in ((5, 5), (1, 1), (8, 9), (19, 19)):
        for rollbacks in (0, 1, 3):
            # A kill (10) and a headshot (2.5) are 13 a tick, rounded by
            # event or once.
            scores, count, at_start, hud = scoreboard(original, roster, newcomer, rollbacks)
            assert count == roster and at_start == list(range(roster)), (count, at_start)
            # The fault: a starting place's score is worked out again at the
            # state loaded, the newcomer's tick is added once more for every
            # rollback, and the cells of places past the roster keep an
            # earlier match's score until they are told one.
            assert scores[0] == 10 and scores[newcomer] == 13 * (rollbacks + 1), (roster, newcomer, scores)
            assert hud[0] == 10 and hud[newcomer] == 13 * (rollbacks + 1)
            assert all(hud[place] == 953 for place in range(roster, 20) if place != newcomer)
            scores, count, at_start, hud = scoreboard(changed, roster, newcomer, rollbacks)
            assert count == 20 and at_start == list(range(20)), (count, at_start)
            # The build: every place worked out again at every state loaded,
            # and every cell told a 0 at the start.
            assert scores[0] == 10 and scores[newcomer] == 13, (roster, newcomer, rollbacks, scores)
            assert hud[0] == 10 and hud[newcomer] == 13
            assert all(hud[place] == 0 and scores[place] == 0 for place in range(1, 20) if place != newcomer)


HUD, GAME, PLAYER = 0x4100000, 0x4110000, 0x4120000
# update_ui's calls between the countdown's test and the players' rows:
# (name, what `ret` takes off the stack).
HUD_CALLS = {0x236AD0: ("show_pregame", 0), 0x236B50: ("set_pregame", 4), 0x236A00: ("set_match_time", 4),
             0x2375C0: ("tip", 16), 0x2364C0: ("set_game_progress", 16), 0x236610: ("set_last_players", 12),
             0x236C30: ("set_respawn_time", 4), 0xE0210: ("destroy_player", 0)}


def hud_update(data, countdown, mode, tip_shown):
    """game_world_ui::update_ui (0x23B200) from its test of the countdown
    flag (0x23B307) to the players' rows (0x23B421), the game's own
    instructions on a made-up hud whose state has changed; the functions it
    calls are taken at their entry, written down with what they were given,
    and returned from as they return. Returns (the calls, esp, ebp and edi
    at the rows, the player's count before and after)."""
    machine = Machine(data)
    uc = machine.uc
    uc.mem_map(HUD, 0x1000)
    uc.mem_map(GAME, 0x10000)
    uc.mem_map(PLAYER, 0x10000)
    uc.mem_write(HUD + 0x15C, bytes([0xFF if countdown else 0]))   # the timelimit rule's flag
    uc.mem_write(HUD + 0x370, bytes([1 if tip_shown else 0]))
    machine.write(HUD + 0x20, mode)
    machine.write(HUD + 0x28, 123000)                   # match time
    machine.write(HUD + 0x2C, 0)                        # respawn time: nobody waits for one
    machine.write(HUD + 0xB0, 15)                       # countdown seconds
    uc.mem_write(HUD + 0xB4, struct.pack("<HH", 150, 149))   # the state's team points
    uc.mem_write(HUD + 0xB8, bytes([5, 4]))             # Team Deathmatch's alive counts
    machine.write(HUD + 0x18, GAME)
    machine.write(GAME + 0x9C, GAME + 0x1000)           # the network client
    machine.write(GAME + 0x1000 + 0x53AC, GAME + 0x2000)
    machine.write(GAME + 0x2004, PLAYER)                # the current player
    machine.write(PLAYER + 0xD0, 1)
    machine.write(PLAYER + 0x6B8, PLAYER + 0x1000)      # its profile
    machine.write(PLAYER + 0x1000 + 0x45EC, 1)          # its team
    calls = []

    def visit(uc, address, size, _):
        if address not in HUD_CALLS:
            return
        name, pops = HUD_CALLS[address]
        esp = uc.reg_read(UC_X86_REG_ESP)
        args = [machine.read(esp + 4 + 4 * i) for i in range(pops // 4)]
        calls.append((name, uc.reg_read(UC_X86_REG_EAX) & (0xFF if name == "show_pregame" else 0xFFFFFFFF), args))
        uc.reg_write(UC_X86_REG_EIP, machine.read(esp))
        uc.reg_write(UC_X86_REG_ESP, esp + 4 + pops)

    uc.hook_add(UC_HOOK_CODE, visit)
    uc.reg_write(UC_X86_REG_EDI, HUD)
    uc.reg_write(UC_X86_REG_EBP, 0x5A5A5A5A)            # the state's hash, stored after the rows
    uc.reg_write(UC_X86_REG_ESP, STACK)
    uc.emu_start(0x23B307, 0x23B421, count=10000)
    assert uc.reg_read(UC_X86_REG_EIP) == 0x23B421
    return (calls, uc.reg_read(UC_X86_REG_ESP), uc.reg_read(UC_X86_REG_EBP), uc.reg_read(UC_X86_REG_EDI),
            machine.read(PLAYER + 0xD0))


def pregame_progress(original, changed):
    """patch_026g.py's PREGAME_PROGRESS: what the hud is told while a match
    counts down, and while it is played, in the original and in the build."""
    def matches(calls, wanted):
        # (name, eax, arguments); None: whatever it was.
        return len(calls) == len(wanted) and all(
            name == w_name and (w_eax is None or eax == w_eax) and (w_args is None or args == w_args)
            for (name, eax, args), (w_name, w_eax, w_args) in zip(calls, wanted))

    for mode in (4, 3, 2):
        for tip_shown in (False, True):
            # The team points of the state and the player's team, the alive
            # counts (Team Deathmatch only) with the team in eax, the respawn
            # time (0) in eax.
            drawn = [("set_game_progress", None, [HUD, 150, 149, 1])]
            if mode == 4:
                drawn.append(("set_last_players", 1, [HUD, 5, 4]))
            drawn.append(("set_respawn_time", 0, [HUD]))
            tip = [] if tip_shown else [("tip", None, None)]
            play = tip + [("show_pregame", 0, []), ("set_match_time", 123000, [HUD])] + drawn
            for data in (original, changed):
                calls, esp, ebp, edi, count = hud_update(data, False, mode, tip_shown)
                assert matches(calls, play), calls
                assert (esp, ebp, edi, count) == (STACK, 0x5A5A5A5A, HUD, 1)
            # The countdown: the original shows it alone; the build then the
            # team points, the last players and the respawn time as play
            # does, without the match time and the tip, with the stack, the
            # hash, the hud and the player's count as play leaves them.
            shown = [("show_pregame", 1, []), ("set_pregame", None, [HUD])]
            calls, esp, ebp, edi, count = hud_update(original, True, mode, tip_shown)
            assert matches(calls, shown), calls
            assert (esp, ebp, edi, count) == (STACK, 0x5A5A5A5A, HUD, 1)
            calls, esp, ebp, edi, count = hud_update(changed, True, mode, tip_shown)
            assert matches(calls, shown + drawn), calls
            assert (esp, ebp, edi, count) == (STACK, 0x5A5A5A5A, HUD, 1)


RULE, SPAWN_CORE, SPAWN_PLACES, SPAWN_PLAYERS, ARMS, STUBS = (
    0x4400000, 0x4410000, 0x4430000, 0x4440000, 0x4460000, 0x4480000)
# The vtables of the client's weapon and of weapon_core (at +0 and at the
# item, +10h), of a plain inventory item, and a made-up one whose +70h is the
# grenade item's bind.
WEAPON_VTABLES = ((0xA04174, 0xA0422C), (0xA11094, 0xA1114C))
PLAIN_ITEM_VTABLE, GRENADE_ITEM_BIND = 0xA1395C, 0x309EC0
SPAWN_STUBS = {"kit_changed": 4, "statistics": 16, "initialize": 16, "damage_model": 0, "activate": 4}


def spawn(data, place, old_kit, new_kit, time, states, protection=0, fields=None, observe=None):
    """The respawn rule's spawn (0x316DA0, the game's own instructions, with
    the hook of the build if it has one) on a made-up rule, world and place:
    three kits, each with a weapon in slots 7 and 10 (the client's class
    for kits 0 and 2, weapon_core's for kit 1) whose random states are
    `states[kit]`, an ammunition item (a plain inventory item) in slot 8 and
    a grenade item in slot 13. The functions it calls (the view's kit change,
    the statistics, the player's initialize and activate, the damage model)
    are written down and returned from. Returns (the weapons' states after
    it, as they were at initialize and at activate, the calls, a check of
    everything else). `fields` {kit: {weapon offset: (size, value)}} is
    written into both weapons of a kit first; with `observe` (a dict) the
    weapons' bytes are put into it before the spawn, at the initialize and
    after the spawn."""
    m = Machine(data)
    uc = m.uc
    map_constants(m, data, (WEAPON_VTABLES[0][1], WEAPON_VTABLES[1][1], PLAIN_ITEM_VTABLE))
    for base, size in ((RULE, 0x1000), (SPAWN_CORE, 0x1A000), (SPAWN_PLACES, 0x1000),
                       (SPAWN_PLAYERS, 0x8000), (ARMS, 0x20000), (STUBS, 0x1000)):
        uc.mem_map(base, size)
    stub = {name: STUBS + 0x100 * i for i, name in enumerate(SPAWN_STUBS)}
    for name, pops in SPAWN_STUBS.items():
        if name == "damage_model":       # returns a pointer to the damage model's pointer
            uc.mem_write(stub[name], b"\xb8" + struct.pack("<I", STUBS + 0x800) + b"\xc3")
            m.write(STUBS + 0x800, STUBS + 0x900)
        else:
            uc.mem_write(stub[name], b"\xc2" + struct.pack("<H", pops))
    rule_vtable, statistics, player_vtable, damage_vtable = STUBS + 0x600, STUBS + 0x680, STUBS + 0x700, STUBS + 0x780
    m.write(rule_vtable + 0x60, stub["kit_changed"])
    m.write(statistics, statistics + 0x10)
    m.write(statistics + 0x10 + 0x2C, stub["statistics"])
    m.write(player_vtable + 0x1C, stub["initialize"])
    m.write(player_vtable + 0x20, stub["activate"])
    m.write(damage_vtable + 8, stub["damage_model"])
    m.write(RULE, rule_vtable)
    m.write(RULE + 0x134, SPAWN_CORE)
    m.write(RULE + 0x138 + place * 4, time)
    m.write(SPAWN_CORE + 0x18458, SPAWN_PLACES)
    m.write(SPAWN_CORE + 0x18FA8, statistics)
    entry = SPAWN_PLACES + place * 0x30
    weapons, others = {}, []
    for kit in range(3):
        player = SPAWN_PLAYERS + kit * 0x2000
        inventory = player + 0x1000
        m.write(entry + kit * 4, player)
        m.write(player, player_vtable)
        m.write(player + 0x108, damage_vtable)
        m.write(player + 0x10C, inventory)
        uc.mem_write(player + 0x114, bytes([place, kit]))
        for index, slot in enumerate((7, 10)):
            weapon = ARMS + kit * 0x4000 + index * 0x1000
            primary, item = WEAPON_VTABLES[kit % 2]
            m.write(weapon, primary)
            m.write(weapon + 0x10, item)
            m.write(weapon + 0x5BC, states[kit][index])
            for offset, (size, value) in (fields or {}).get(kit, {}).items():
                uc.mem_write(weapon + offset, value.to_bytes(size, "little"))
            m.write(inventory + 0x10C + slot * 4, weapon + 0x10)
            weapons[kit, slot] = weapon
        ammunition, grenades = ARMS + 0xC000 + kit * 0x1000, ARMS + 0x18000 + kit * 0x1000
        m.write(ammunition, PLAIN_ITEM_VTABLE)
        m.write(grenades, grenades + 0x800)
        m.write(grenades + 0x800 + 0x70, GRENADE_ITEM_BIND)
        m.write(inventory + 0x10C + 8 * 4, ammunition)
        m.write(inventory + 0x10C + 13 * 4, grenades)
        others += [ammunition, grenades]
    uc.mem_write(entry + 0xC, bytes([old_kit, new_kit]))
    point = STUBS + 0xA00
    uc.mem_write(point + 8, struct.pack("<4f", 1.5, -2.0, 3.25, 0.75))   # position, yaw
    m.write(point + 0x20, protection)
    random_state = lambda: {key: m.read(weapon + 0x5BC) for key, weapon in weapons.items()}
    others_before = [bytes(uc.mem_read(item, 0x1000)) for item in others]
    calls, at_initialize, at_activate = [], {}, {}
    weapon_bytes = lambda: {key: bytes(uc.mem_read(weapon, 0x1000)) for key, weapon in weapons.items()}
    if observe is not None:
        observe["before"] = weapon_bytes()

    def visit(uc, address, size, _):
        for name, at in stub.items():
            if address == at:
                esp = uc.reg_read(UC_X86_REG_ESP)
                args = [m.read(esp + 4 + 4 * i) for i in range(SPAWN_STUBS[name] // 4)]
                calls.append((name, uc.reg_read(UC_X86_REG_ECX), args))
                if name == "initialize":
                    at_initialize.update(random_state())
                    if observe is not None:
                        observe["initialize"] = weapon_bytes()
                elif name == "activate":
                    at_activate.update(random_state())

    uc.hook_add(UC_HOOK_CODE, visit)
    kept = {UC_X86_REG_EBX: 0x11111111, UC_X86_REG_ESI: 0x22222222, UC_X86_REG_EDI: 0x33333333,
            UC_X86_REG_EBP: 0x44444444}
    for register, value in kept.items():
        uc.reg_write(register, value)
    uc.reg_write(UC_X86_REG_ECX, RULE)
    # The rule's tick passes the place as a byte in a register whose upper
    # bytes are what they were (0x316994).
    m.start(0x316DA0, args=(0xABCDEF00 | place, point, time), count=20000)
    new_player = SPAWN_PLAYERS + new_kit * 0x2000
    if observe is not None:
        observe["after"] = weapon_bytes()
    rest = (uc.reg_read(UC_X86_REG_ESP) == STACK + 16
            and all(uc.reg_read(register) == value for register, value in kept.items())
            and list(uc.mem_read(entry + 0xC, 2)) == [new_kit, new_kit]
            and m.read(RULE + 0x138 + place * 4) == 0xFFFFFFFF
            and (not protection or m.read(STUBS + 0x900 + 0x32C) == (time + protection) & 0xFFFFFFFF)
            and [bytes(uc.mem_read(item, 0x1000)) for item in others] == others_before)
    # (name, this, arguments); a byte argument is the low byte of its word.
    byte, word = 0xFF, 0xFFFFFFFF
    wanted = ([("kit_changed", RULE, [(place, byte)]),
               ("statistics", statistics, [(time, word), (place, byte), (old_kit, byte), (new_kit, byte)])]
              if old_kit != new_kit else [])
    wanted.append(("initialize", new_player, [(time, word), (point + 8, word),
                                              (struct.unpack("<I", struct.pack("<f", 0.75))[0], word), (0, word)]))
    if protection:
        wanted.append(("damage_model", new_player + 0x108, []))
    wanted.append(("activate", new_player, [(1, word)]))
    calls_right = len(calls) == len(wanted) and all(
        name == w_name and this == w_this and len(args) == len(w_args)
        and all(arg & mask == value for arg, (value, mask) in zip(args, w_args))
        for (name, this, args), (w_name, w_this, w_args) in zip(calls, wanted))
    return random_state(), at_initialize, at_activate, calls_right, rest


def uniform_draws(data, state, count=4):
    """The game's own uniform_01 on minstd_rand (0x2AB6E0, esi = the
    engine, as the spread's normal distribution 0x2E4850 calls it): the
    first `count` draws from `state`, as the bits of the floats."""
    m = Machine(data)
    map_constants(m, data, (0xA2D9E0, 0xA2D790))
    m.uc.mem_map(STUBS, 0x1000)
    m.write(STUBS, state)
    draws = []
    for _ in range(count):
        m.uc.reg_write(UC_X86_REG_ESI, STUBS)
        m.start(0x2AB6E0, count=2000)
        draws.append(m.uc.reg_read(UC_X86_REG_XMM0) & 0xFFFFFFFF)
    return draws, m.read(STUBS)


def weapon_random(original, changed):
    """patch_026g.py's WEAPON_RANDOM: what the respawn rule's spawn leaves
    in the random states of the new kit's weapons, in the original and in
    the build, for two games of one match: one that played the kit before
    (its weapons drawn from), one that loaded a world since (its weapons at
    the constructor's 1)."""
    played = {0: (0x0BADF00D, 0x1234567), 1: (0x7FFFFFFE, 48271), 2: (0x3C6EF372, 0x2545F491)}
    loaded = {kit: (1, 1) for kit in range(3)}
    for place, old_kit, new_kit, time, protection in ((0, 0, 1, 21750, 0), (3, 1, 1, 148800, 3000),
                                                      (8, 2, 0, 729200, 0), (19, 0, 2, 0xFFFFFFCE, 2000)):
        for data in (original, changed):
            game = {}
            for name, states in (("played", played), ("loaded", loaded)):
                after, at_initialize, at_activate, calls_right, rest = spawn(
                    data, place, old_kit, new_kit, time, states, protection)
                assert calls_right and rest, (name, place, old_kit, new_kit)
                before = {(kit, slot): states[kit][index] for kit in range(3) for index, slot in enumerate((7, 10))}
                # Nothing but the new kit's weapons, and nothing before the
                # player's initialize returned.
                assert at_initialize == before
                assert all(after[key] == before[key] for key in before if key[0] != new_kit)
                game[name] = after
                if data is changed:
                    for slot in (7, 10):
                        state = patch.base.weapon_random_state(place, new_kit, slot, time)
                        assert 1 <= state <= 0x7FFFFFFE
                        assert after[new_kit, slot] == at_activate[new_kit, slot] == state, (name, slot, after)
                else:
                    assert after == before
            mine = [game["played"][new_kit, slot] for slot in (7, 10)]
            theirs = [game["loaded"][new_kit, slot] for slot in (7, 10)]
            draws = [uniform_draws(data, state)[0] for state in mine + theirs]
            if data is changed:
                # The two games agree, and the game's own draws from it too.
                assert mine == theirs and draws[:2] == draws[2:]
                assert mine[0] != mine[1]
            else:
                assert mine != theirs and draws[:2] != draws[2:]
    # The two weapons of a kit, the three kits, the places and the times of
    # spawns give other states.
    states = {patch.base.weapon_random_state(place, kit, slot, time)
              for place in range(20) for kit in range(3) for slot in (7, 10) for time in (0, 50, 100, 21750, 900000)}
    assert len(states) == 20 * 3 * 2 * 5
    # The draws of the engine from a state it gets: minstd_rand's.
    draws, state = uniform_draws(changed, 1, 3)
    assert state == pow(48271, 3, 0x7FFFFFFF)


def rust_function(data, symbol):
    """(file offset, bytes) of a Rust patch's function in the build `data`:
    from its symbol (or VA) in the arena's link to the next symbol."""
    linked = patch.base.arena()
    start = linked.symbols[symbol] if isinstance(symbol, str) else symbol
    end = min(va for _, va, _, _ in linked.every if va > start)
    for _, rva, size, raw, raw_size, _ in patch.base.Headers(data).sections(data):
        if patch.base.IMAGE_BASE + rva <= start < patch.base.IMAGE_BASE + rva + raw_size:
            at = raw + start - patch.base.IMAGE_BASE - rva
            return at, bytes(data[at:at + end - start])
    raise AssertionError(f"{symbol} is in no section of the build")


def rust_callee(data, symbol):
    """The VA the first `call rel32` of a Rust function calls (a function the
    compiler did not inline, such as an iterator's next)."""
    at, code = rust_function(data, symbol)
    start = patch.base.arena().symbols[symbol]
    m = re.search(rb"\xe8(.{4})", code, re.S)
    assert m, f"{symbol} calls nothing"
    return (start + m.end() + struct.unpack("<i", m.group(1))[0]) & 0xFFFFFFFF


def broken_rust(data, symbols, pattern, new):
    """A copy of the build with each match of `pattern` (a regex of bytes)
    in Rust functions (a symbol or VA, or a list of them) made `new` (a
    function of the match's bytes): one thing broken. The pattern must be
    there."""
    copy, found = bytearray(data), 0
    for symbol in symbols if isinstance(symbols, list) else [symbols]:
        at, code = rust_function(data, symbol)
        for m in re.finditer(pattern, code, re.S):
            copy[at + m.start():at + m.end()] = new(m.group(0))
            found += 1
    assert found, f"{pattern!r} is not in {symbols}"
    return bytes(copy)


def rust_code(data, symbol):
    """A Rust shim and the function it calls first, if it calls one (what
    the compiler did not inline into it: the inventory's walk)."""
    try:
        return [symbol, rust_callee(data, symbol)]
    except AssertionError:
        return [symbol]


# The slots' bound in a Rust walk of an inventory: cmp reg,16h (the last
# slot) or cmp reg,17h (one past it).
SLOT_BOUND = rb"\x83[\xf8-\xff][\x16\x17]"


def slots_to_ten(b):
    """The bound made 9 (or 10): the walk ends after slot 9."""
    return b[:2] + bytes([b[2] - 0x16 + 0x09])


def weapon_random_fails(original, changed):
    """weapon_random given the original as the build, and given copies of
    the build with one thing broken each, must fail. The build's code is the
    Rust patch weapon_random's (inlined into its shim): the slots' bound,
    the spawn's time and the bind's test each changed in its instructions."""
    shim = "hook_weapon_random_on_spawn"
    copies = {
        "the original": original,
        "slot 10 left out (the slots end at 10)": broken_rust(
            changed, rust_code(changed, shim), SLOT_BOUND, slots_to_ten),
        # imul r,r/m,9E3779B1h: the time (a register, or EBP's saved dword)
        # replaced by another
        "the time left out": broken_rust(
            changed, shim, rb"\x69(?:[\xc0-\xff]|[\x40-\x7f].)\xb1\x79\x37\x9e",
            lambda b: bytes([b[0], b[1] ^ 1]) + b[2:] if b[1] >= 0xC0 else b[:2] + bytes([b[2] ^ 4]) + b[3:]),
        # cmp ...,2E30F0h then the branch on it: every item a weapon
        "every item taken for a weapon": broken_rust(
            changed, rust_code(changed, shim), rb"(?:\x3d|\x81[\xf8-\xff])\xf0\x30\x2e\x00[\x74\x75].",
            lambda b: b[:-2] + (b"\x90\x90" if b[-2] == 0x75 else b"\xeb" + b[-1:])),
    }
    for name, data in copies.items():
        try:
            weapon_random(original, data)
        except AssertionError:
            continue
        raise AssertionError(f"weapon_random passes with {name}")


# A weapon of a kit's last life, as it can be when the kit is not current:
# the time of its last tick, the fire mode and ammunition kind the player
# chose; the magazine 0 as the removal before a spawn leaves it (0x2E18E0);
# and, for the safety resets, values a weapon in hand had (aim, recoil,
# chamber, +5ECh, +5F5h, +5A8h), LIKELY never left so by the removal (the
# random state is weapon_random's).
USED_WEAPON = {0x5A4: (4, 58700), 0x5A8: (4, 61250), 0x5B1: (1, 1), 0x5B2: (1, 1), 0x5E5: (1, 0), 0x5E6: (1, 2),
               0x5E8: (1, 1), 0x5EC: (4, 3), 0x5F1: (1, 1), 0x5F5: (1, 1), 0x5FC: (4, 0x3E99999A),
               0x600: (4, 0x3F000000), 0x604: (4, 0xBDCCCCCD), 0x608: (4, 0x3C23D70A), 0x60C: (4, 61200)}


def field_value(blob, offset, size):
    return int.from_bytes(blob[offset:offset + size], "little")


def kit_state(original, changed):
    """patch_026g.py's KIT_STATE: the respawn rule's spawn on a kit that
    one game played (its weapons as USED_WEAPON) and another loaded a world
    since (its weapons as the constructor leaves them). In the build, at
    the initialize, both games' weapons of the new kit have -1 as the time
    of their last tick, and at a spawn into another kit every field of
    KIT_STATE_FIELDS at the constructor's value; at a spawn into the same
    kit (whose player is in every world, so the games have it alike) the
    rest is kept. Nothing else of any weapon changes but the random state
    weapon_random gives, the other kits' weapons not at all."""
    fresh = {offset: (size, value) for offset, size, value in patch.base.KIT_STATE_FIELDS}
    fresh[patch.base.KIT_STATE_TICK_FIELD] = (4, 0xFFFFFFFF)
    assert sorted(fresh) == sorted(USED_WEAPON) and all(fresh[o][0] == USED_WEAPON[o][0] for o in fresh)
    assert all(fresh[o][1] != USED_WEAPON[o][1] for o in fresh if o != 0x5E5)
    states = {kit: (1, 1) for kit in range(3)}
    reset = set(fresh)
    changed_bytes = lambda offsets: {o + i for o in offsets for i in range(fresh[o][0])} | set(range(0x5BC, 0x5C0))
    for place, old_kit, new_kit, time, protection in ((0, 0, 1, 21750, 0), (3, 1, 1, 148800, 3000),
                                                      (8, 2, 0, 729200, 0), (19, 0, 2, 0xFFFFFFCE, 2000),
                                                      (5, 2, 2, 564750, 0)):
        for data in (original, changed):
            game = {}
            for name in ("played", "loaded"):
                # A respawn into the same kit: that kit's player is in every
                # world, so a game that loaded one has it as the others do.
                new_kit_fields = USED_WEAPON if name == "played" or old_kit == new_kit else fresh
                fields = {kit: (USED_WEAPON if name == "played" else fresh) for kit in range(3)}
                fields[new_kit] = new_kit_fields
                observe = {}
                _, _, _, calls_right, rest = spawn(data, place, old_kit, new_kit, time, states, protection,
                                                   fields, observe)
                assert calls_right and rest, (name, place, old_kit, new_kit)
                for key, before in observe["before"].items():
                    at, after = observe["initialize"][key], observe["after"][key]
                    kit, slot = key
                    # The initialize is given the weapons as they end: nothing
                    # but weapon_random's state changes after it.
                    assert at[:0x5BC] == after[:0x5BC] and at[0x5C0:] == after[0x5C0:]
                    if kit != new_kit:
                        assert after == before, (name, key)
                        continue
                    differ = {i for i in range(0x1000) if after[i] != before[i]}
                    if data is changed:
                        wanted = reset if old_kit != new_kit else {patch.base.KIT_STATE_TICK_FIELD}
                        assert differ <= changed_bytes(wanted), (name, key, sorted(differ))
                        for offset, (size, value) in fresh.items():
                            if offset in wanted:
                                assert field_value(at, offset, size) == value, (name, key, hex(offset))
                            else:
                                assert field_value(at, offset, size) == field_value(before, offset, size)
                    else:
                        assert differ <= set(range(0x5BC, 0x5C0)), (name, key, sorted(differ))
                game[name] = {key: blob[:0x5BC] + blob[0x5C0:] for key, blob in observe["after"].items()
                              if key[0] == new_kit}
            if data is changed:
                assert game["played"] == game["loaded"], (place, old_kit, new_kit)
            elif old_kit != new_kit:
                assert game["played"] != game["loaded"]


def kit_state_fails(original, changed):
    """kit_state given the original as the build, and copies of the build
    with one thing broken each, must fail. The build's code is the Rust
    patch kit_state's (its shim, and the inventory's walk it calls): the
    slots' bound, the new kit's byte, the fire mode's store, the test of a
    change of kit and the last tick's store each changed in its
    instructions."""
    shim = "hook_kit_state_on_spawn"
    tick = rb"\xc7[\x80-\xbf]\xa4\x05\x00\x00\xff\xff\xff\xff"
    copies = {
        "the original": original,
        "slot 10 left out (the slots end at 10)": broken_rust(
            changed, rust_code(changed, shim), SLOT_BOUND, slots_to_ten),
        # the new kit's byte, the site's [esp+18h] (the saved ESP +20h): [esp+14h]
        "the change of kit read from the wrong place": broken_rust(
            changed, shim, rb"\x8a[\x40-\x7f]\x20", lambda b: b[:2] + b"\x1c"),
        "the fire mode left out": broken_rust(
            changed, shim, rb"\x88[\x80-\xbf]\xe6\x05\x00\x00", lambda b: b"\x90" * len(b)),
        # the jz past the reset after the tick's store: never taken
        "every spawn taken for a change of kit": broken_rust(
            changed, shim, tick + rb"\x74.", lambda b: b[:-2] + b"\x90\x90"),
        "the time of the last tick left out": broken_rust(
            changed, shim, tick, lambda b: b"\x90" * len(b)),
    }
    for name, data in copies.items():
        try:
            kit_state(original, data)
        except AssertionError:
            continue
        raise AssertionError(f"kit_state passes with {name}")


LOAD_CORE, LOAD_LISTS, LOAD_NODES, LOAD_PLACES, LOAD_STACK = 0x4600000, 0x4620000, 0x4640000, 0x4660000, 0x4680000
NODE_SIZE = 0x40


def first_load(data, places, time, pool_room=64):
    """The block of a game's first world (0x2D0FF2..0x2D105E, the game's own
    instructions and its two calls, patched or not) on a made-up core and
    world: `places` = {place: (kit in use, the world's next kit, the kit
    choices there are, the inputs there are)}, each list [(time, value)]
    oldest first, as the network's sorted inserts leave them; the nodes
    come from the game's own allocator (0x1FCB60 with 0x2D48B0 and the
    pool's 0x5A540) on a pool of `pool_room` nodes for each list. Then the
    game's own tick of each place (0x321790) for the ticks from the world's
    time on. Returns {place: (kit choices, inputs, every list walked both
    ways, the next kit after each tick)}."""
    m = Machine(data)
    uc = m.uc
    for base, size in ((LOAD_CORE, 0x19000), (LOAD_LISTS, 0x20000), (LOAD_NODES, 0x20000),
                       (LOAD_PLACES, 0x4000), (LOAD_STACK - 0x1000, 0x2000)):
        uc.mem_map(base, size)
    uc.mem_map(0x2260000, 0x1000)                 # 0x5A540's debug flag (0x22601BB): off
    m.write(LOAD_CORE + 0x18458, LOAD_PLACES)
    m.write(LOAD_CORE + 0x2FCC, LOAD_LISTS)
    m.write(LOAD_CORE + 0x89D8, LOAD_LISTS + 0x8000)
    free = [LOAD_NODES]

    def pool_for(header):
        """A pool of its own for the list at `header` (+10h), its free list
        strung through the nodes' first word."""
        pool = header + 0x100
        nodes = [free[0] + NODE_SIZE * i for i in range(pool_room)]
        free[0] += NODE_SIZE * pool_room
        for node, after in zip(nodes, nodes[1:] + [0]):
            m.write(node, after)
        m.write(header + 0x10, pool)
        m.write(pool + 0x20, nodes[0])
        m.write(pool + 0x24, 0)
        m.write(pool + 0x28, pool_room)
        return pool

    def fill(header, entries, inputs):
        pool = pool_for(header)
        previous = 0
        for at, value in entries:
            node = m.read(pool + 0x20)
            m.write(pool + 0x20, m.read(node))
            m.write(pool + 0x24, m.read(pool + 0x24) + 1)
            uc.mem_write(node, bytes(NODE_SIZE))
            m.write(node + 4, previous)
            m.write(node + 0x20, at)
            if inputs:
                m.write(node + 8, value)
                m.write(node + 0xC, value ^ 0x5A5A5A5A)
            else:
                uc.mem_write(node + 8, bytes([value]))
            if previous:
                m.write(previous, node)
            else:
                m.write(header + 4, node)
            m.write(header + 8, node)
            previous = node
        m.write(header, len(entries))

    mask = 0
    for place, (in_use, chosen, kits, inputs) in places.items():
        mask |= 1 << place
        entry = LOAD_PLACES + place * 0x30
        for kit in range(3):
            player = LOAD_PLACES + 0x1000 + place * 0x100 + kit * 0x40
            m.write(entry + kit * 4, player)
        uc.mem_write(entry + 0xC, bytes([in_use, chosen]))
        m.write(entry + 0x28, LOAD_CORE)
        uc.mem_write(entry + 0x2C, bytes([place]))
        # The player in use: its kit byte and its input, mapped apart.
        player = 0x46A0000 + place * 0x1000
        try:
            uc.mem_map(player, 0x1000)
        except Exception:
            pass
        m.write(entry + in_use * 4, player)
        uc.mem_write(player + 0x115, bytes([in_use]))
        m.write(player + 0x32C, 0x1000 + place)
        m.write(player + 0x330, 0x2000 + place)
        fill(LOAD_LISTS + place * 0x480, inputs, True)
        fill(LOAD_LISTS + 0x8000 + place * 0x480, kits, False)
    # The frame of 0x2D0B70 at 0x2D0FF2: [ebp+8] the core, [esp+14h] the
    # history item (its time at +C0BB0h), esi the mask left to do.
    uc.mem_map(0x4700000, 0xC1000)
    m.write(0x4700000 + 0xC0BB0, time)
    frame = LOAD_STACK
    m.write(frame + 8, LOAD_CORE)
    esp = LOAD_STACK - 0x80
    m.write(esp + 0x14, 0x4700000)
    uc.reg_write(UC_X86_REG_ESP, esp)
    uc.reg_write(UC_X86_REG_EBP, frame)
    uc.reg_write(UC_X86_REG_ESI, mask)
    uc.reg_write(UC_X86_REG_EDI, 0x77777777)
    uc.emu_start(0x2D0FF2, 0x2D105E, count=200000)
    assert uc.reg_read(UC_X86_REG_EIP) == 0x2D105E and uc.reg_read(UC_X86_REG_ESP) == esp
    assert uc.reg_read(UC_X86_REG_EBP) == frame and uc.reg_read(UC_X86_REG_ESI) == 0

    def walk(header, inputs):
        forward, node, before = [], m.read(header + 4), 0
        while node:
            assert m.read(node + 4) == before and len(forward) < 1000
            value = (m.read(node + 8), m.read(node + 0xC)) if inputs else uc.mem_read(node + 8, 1)[0]
            forward.append((m.read(node + 0x20), value))
            before, node = node, m.read(node)
        linked = m.read(header + 8) == before and m.read(header) == len(forward)
        return forward, linked

    out = {}
    for place in places:
        kits, kits_linked = walk(LOAD_LISTS + 0x8000 + place * 0x480, False)
        inputs, inputs_linked = walk(LOAD_LISTS + place * 0x480, True)
        entry = LOAD_PLACES + place * 0x30
        chosen = []
        for tick in range(time, time + 400, 50):
            uc.reg_write(UC_X86_REG_ECX, entry)
            m.start(0x321790, args=(0, tick, 0), count=2000)
            assert uc.reg_read(UC_X86_REG_ESP) == STACK + 16
            chosen.append(uc.mem_read(entry + 0xD, 1)[0])
        out[place] = (kits, inputs, kits_linked and inputs_linked, chosen)
    return out


def next_kits(entries, chosen, time):
    """What a game that played the match has as the place's next kit after
    each tick from `time` on: the world's choice at `time`, then the newest
    entry at or before the tick (every kit is there)."""
    out = []
    for tick in range(time, time + 400, 50):
        older = [value for at, value in entries if at <= tick]
        if older and older[-1] < 3:
            chosen = older[-1]
        out.append(chosen)
    return out


def first_world(original, changed):
    """patch_026g.py's FIRST_LOAD: the entries of a first world, in the
    original and in the build, against what the games that played the match
    have."""
    time = 300050
    cases = {
        # The newcomer case: a kit chosen before the world (2, the world's
        # next kit) and one relayed for later (1, at +100 ms); kit 0 in use.
        3: (0, 2, [(time - 200, 2), (time + 100, 1)], [(time - 100, 7), (time + 50, 8)]),
        # A first world of a match: nothing in the lists, the next kit the
        # one in use.
        0: (1, 1, [], []),
        # An entry of the world's very time, and only older ones.
        8: (2, 0, [(time - 50, 1), (time, 0)], [(time, 9)]),
        # Every entry newer than the world.
        19: (1, 0, [(time + 50, 2), (time + 150, 1)], [(time + 50, 3)]),
    }
    for data, build in ((original, False), (changed, True)):
        for room in (64, 2):          # 2: the pool full, its oldest node given back
            if room == 2 and not build:
                continue
            got = first_load(data, {p: c for p, c in cases.items() if room == 64 or p in (0, 19)}, time, room)
            for place, (kits, inputs, linked, chosen) in got.items():
                in_use, world_next, had_kits, had_inputs = cases[place]
                assert linked, place
                current = (0x1000 + place, 0x2000 + place)
                if build:
                    full = room == 2 and len(had_kits) >= 2
                    kept_kits = had_kits[1:] if full else had_kits
                    kept_inputs = had_inputs
                    # In time order; the world's entry at its time.
                    want_kits = sorted([e for e in kept_kits if e[0] != time] + [(time, world_next)])
                    want_inputs = sorted([(at, (v, v ^ 0x5A5A5A5A)) for at, v in kept_inputs if at != time]
                                         + [(time, current)])
                    assert kits == want_kits, (place, kits, want_kits)
                    assert inputs == want_inputs, (place, inputs, want_inputs)
                    # The next kit as every other game has it.
                    assert chosen == next_kits(kept_kits, world_next, time), (place, chosen)
                elif place == 3:
                    # The fault: the kit in use at the newest end, after the
                    # choice relayed for later, which it hides from then on.
                    assert kits == had_kits + [(time, in_use)], kits
                    assert chosen != next_kits(had_kits, world_next, time) and set(chosen) == {in_use}, chosen
                elif place == 0:
                    assert kits == [(time, in_use)] and inputs == [(time, current)]
    # With nothing newer and the next kit the one in use, the build's lists
    # are the original's.
    case = {0: (1, 1, [(time - 100, 1)], [(time - 50, 4)])}
    assert first_load(original, case, time)[0][:2] == first_load(changed, case, time)[0][:2]


def first_world_fails(original, changed):
    """first_world given the original as the build, and copies of the build
    with one thing broken each, must fail."""
    image = patch.base.Headers(changed)
    _, _, _, raw, _, _ = srp_section(changed)
    cave = raw + patch.base.LATE_JOIN_OFFSET + patch.base.FIRST_LOAD_INPUT_HOOK[2]
    code = bytes(changed[cave:cave + 0xC0])

    def broken(old, new):
        at = code.index(old)
        copy = bytearray(changed)
        copy[cave + at:cave + at + len(old)] = new
        return bytes(copy)

    copies = {
        "the original": original,
        "the kit in use for the world's choice": broken(bytes.fromhex("8a580d"), bytes.fromhex("8a580c")),
        "at the newest end (as the game's own)": broken(bytes.fromhex("394a207205"), bytes.fromhex("394a20eb05")),
        "an entry of the same time not found": broken(bytes.fromhex("394820743272"), bytes.fromhex("394820909072")),
    }
    for name, data in copies.items():
        try:
            first_world(original, data)
        except AssertionError:
            continue
        raise AssertionError(f"first_world passes with {name}")


SEEK_GAME, SEEK_WORLD, SEEK_OBJECTS = 0x4200000, 0x4210000, 0x4240000
CLIENT_WEAPON_ITEM_VTABLE = 0xA0422C


def seek_tail(data, places, user_seek=True, target=725000):
    """The end of the replay's seek (0x1EE39F..0x1EE3B5: after the clock is
    set to the target) on a made-up game and world. `places` is a list of
    three kits each, a kit None or a list of 23 slots, a slot None, "weapon"
    (a client weapon whose queue holds a trivial functor, a functor with a
    manager and an empty one, and whose fx history has entries) or "other"
    (an item of another class with the same layout). Returns (the managers'
    calls, [(queue begin, end, fx write index, fx oldest index)] of each
    weapon or other item, the game's two stores)."""
    machine = Machine(data)
    uc = machine.uc
    uc.mem_map(SEEK_GAME, 0x10000)
    uc.mem_map(SEEK_WORLD, 0x20000)
    uc.mem_map(SEEK_OBJECTS, 0x200000)
    manager = SEEK_OBJECTS                              # a functor manager: cdecl (in, out, op)
    uc.mem_write(manager, b"\xc3")
    managers = SEEK_OBJECTS + 0x10                      # [manager table] = manager
    machine.write(managers, manager)
    calls = []

    def visit(uc, address, size, _):
        if address == manager:
            esp = uc.reg_read(UC_X86_REG_ESP)
            calls.append((machine.read(esp + 4), machine.read(esp + 8), machine.read(esp + 12)))

    uc.hook_add(UC_HOOK_CODE, visit)
    free = SEEK_OBJECTS + 0x1000
    machine.write(SEEK_GAME + 0x4D64, SEEK_WORLD)
    array = SEEK_WORLD + 0x100
    machine.write(SEEK_WORLD + 0x18650, array)
    machine.write(SEEK_WORLD + 0x18654, array + 0x30 * len(places))
    items = []
    for p, kits in enumerate(places):
        for k, slots in enumerate(kits):
            if slots is None:
                machine.write(array + 0x30 * p + 4 * k, 0)
                continue
            player, inventory = free, free + 0x200
            free += 0x400
            machine.write(array + 0x30 * p + 4 * k, player)
            machine.write(player + 0x10C, inventory)
            for s, kind in enumerate(slots):
                if kind is None:
                    machine.write(inventory + 0x10C + 4 * s, 0)
                    continue
                weapon = free
                free += 0x2000
                item = weapon + 0x10
                machine.write(inventory + 0x10C + 4 * s, item)
                machine.write(item, CLIENT_WEAPON_ITEM_VTABLE if kind == "weapon" else 0xA04174)
                begin = weapon + 0xA08
                for i, functor in enumerate((0xA0B860 | 1, managers, 0)):
                    machine.write(begin + 0x38 * i, functor)
                machine.write(weapon + 0x9FC, begin)
                machine.write(weapon + 0xA00, begin + 3 * 0x38)
                machine.write(weapon + 0xA04, begin + 64 * 0x38)
                machine.write(weapon + 0x828, 7)
                machine.write(weapon + 0x82C, 2)
                items.append(weapon)
    uc.reg_write(UC_X86_REG_ESI, SEEK_GAME)
    uc.reg_write(UC_X86_REG_EDI, target)
    uc.reg_write(UC_X86_REG_EBX, 0x12345600 | (0 if user_seek else 1))
    uc.reg_write(UC_X86_REG_EBP, 0x5EED)
    uc.reg_write(UC_X86_REG_ESP, STACK)
    uc.emu_start(0x1EE39F, 0x1EE3B5, count=200000)
    assert uc.reg_read(UC_X86_REG_EIP) == 0x1EE3B5
    assert uc.reg_read(UC_X86_REG_ESP) == STACK
    assert (uc.reg_read(UC_X86_REG_ESI), uc.reg_read(UC_X86_REG_EDI), uc.reg_read(UC_X86_REG_EBP)) == \
        (SEEK_GAME, target, 0x5EED)
    state = [(machine.read(w + 0x9FC), machine.read(w + 0xA00), machine.read(w + 0x828), machine.read(w + 0x82C))
             for w in items]
    return calls, items, state, (machine.read(SEEK_GAME + 0x4EC), machine.read(SEEK_GAME + 0x57C))


def seek_clear(original, changed):
    """A replay's user seek empties every client weapon's delayed sounds and
    fx history (the Rust patch replay_viewer, replay.seek) as ~weapon does, and touches
    nothing else; a key frame in play (bl = 1) and the original leave them."""
    weapon_slots = ["weapon", None, "other"] + [None] * 17 + ["weapon", None, "weapon"]
    layouts = ([], [[None, None, None]], [[weapon_slots, None, ["other"] + [None] * 22]],
               [[weapon_slots, weapon_slots, weapon_slots]] * 3 + [[None, weapon_slots, None]] * 17)
    for places in layouts:
        for data, user_seek in ((changed, True), (changed, False), (original, True)):
            calls, items, state, stores = seek_tail(data, places, user_seek)
            assert stores == ((725000 - 1000, 725000 - 1000) if user_seek else (0, 0)), stores
            cleared = data is changed and user_seek
            kinds = [kind for kits in places for slots in kits if slots for kind in slots if kind]
            assert len(kinds) == len(items)
            for kind, weapon, (begin, end, write, oldest) in zip(kinds, items, state):
                if cleared and kind == "weapon":
                    assert (begin, end, write, oldest) == (weapon + 0xA08, weapon + 0xA08, 2, 2)
                else:
                    assert (begin, end, write, oldest) == (weapon + 0xA08, weapon + 0xA08 + 3 * 0x38, 7, 2)
            # The one functor with a manager is destroyed (op 2, on its own
            # storage), once per cleared weapon; nothing is invoked.
            expected = [(w + 0xA08 + 0x38 + 8,) * 2 + (2,) for kind, w in zip(kinds, items) if kind == "weapon"] \
                if cleared else []
            assert calls == expected, (calls, expected)


def check(original, nasm):
    assert hashlib.sha256(original).hexdigest() == patch.base.EXE_SHA256
    changed = patch.build(original, nasm)
    assert Machine(changed).uc.mem_read(0x380866, 1) == bytes([patch.PATCH_REVISION])
    assert Machine(changed).uc.mem_read(0x27a1d1, 2) == bytes.fromhex("33c0")
    native_direct_match(patch.build(original, nasm, test_session=0x1002), 0x1002)
    native_animations(changed)
    native_preparation(changed)
    for grenades, traps in ((2, 0), (0, 1), (6, 3), (1, 1)):
        native_preparation(changed, grenades, traps)
    saved_objects(changed)
    untouched_objects(changed)
    native_query(changed)
    native_profile(changed)
    native_histories(changed)
    statistics_scores(original, changed)
    pregame_progress(original, changed)
    weapon_random(original, changed)
    weapon_random_fails(original, changed)
    kit_state(original, changed)
    kit_state_fails(original, changed)
    first_world(original, changed)
    first_world_fails(original, changed)
    seek_clear(original, changed)
    for debug in (0, 0xff):
        m = Machine(changed)
        m.uc.reg_write(UC_X86_REG_EAX, BASE)
        m.uc.reg_write(UC_X86_REG_EDI, 4)
        m.uc.reg_write(UC_X86_REG_EBX, 0x12345600 | debug)
        m.write(STACK + 0x1c, 0x98765432)
        m.start(0x27A23D, end=0x27A244)
        assert m.uc.mem_read(BASE + 4, 1)[0] == (0x80 | (debug & 1))
        assert m.uc.reg_read(UC_X86_REG_EAX) == 0x98765432
        assert m.uc.reg_read(UC_X86_REG_EBX) == (0x12345680 | (debug & 1))
        assert m.uc.reg_read(UC_X86_REG_ESP) == STACK
    for roster in (0, 1, 2, 8, 9, 16, 20):
        for reading in (False, True):
            assert statistics(original, roster, reading) == list(range(roster))
            assert statistics(changed, roster, reading) == list(range(20))
        for setter, field in ((0x279610, 0x23D0E8), (0x27A2E0, 0x23CE80)):
            m = Machine(changed)
            m.uc.reg_write(UC_X86_REG_ECX, BASE)
            # Live and journaling match-client fields lie outside the core fixture.
            m.uc.mem_map(BASE + (field & ~4095), 4096)
            m.start(setter, args=(roster,))
            assert m.uc.mem_read(BASE + field, 1)[0] == 3
            assert m.uc.reg_read(UC_X86_REG_ESP) == STACK + 8
    for place in (0, 1, 8, 19):
        for kits in ((0,), (1,), (2,), (0, 2), (0, 1, 2)):
            native_binding(changed, place, kits)
            for world_objects in (0, 130, 255):
                native_binding(changed, place, kits, weapons=True, world_objects=world_objects)
        for consumed in (1, 17, 200):
            for old_counter in (0, 53, 4096):
                binding(changed, place, consumed, old_counter)
    h = patch.base.Headers(changed)
    assert struct.unpack_from("<I", changed, h.checksum_at)[0] == patch.base.pe_checksum(changed, h.checksum_at)
    old_security, old_size = struct.unpack_from("<II", original, patch.base.Headers(original).security_at)
    new_security, new_size = struct.unpack_from("<II", changed, h.security_at)
    assert old_size == new_size and original[old_security:] == changed[new_security:]
    # .srp, then .srpd (sections of later patches, such as patch 46's .srpc, may follow).
    names = [name for name, _, _, _, _, _ in h.sections(changed)]
    at = names.index(patch.base.LATE_JOIN_STATE_NAME)
    _, rva, size, _, _, flags = h.sections(changed)[at - 1]
    assert names[at - 1] == patch.base.SECTION_NAME and flags == patch.base.SECTION_CHARACTERISTICS
    assert struct.unpack_from("<I", changed, h.size_of_image_at)[0] >= rva + size
    name, rva, size, _, raw_size, flags = h.sections(changed)[at]
    assert name.startswith(b".srpd") and size == 4096 and raw_size == 0 and flags == 0xC0000080
    print("PASS: native preparation/install/ACK/cleanup fixture; query/callback ABI; bounded profile decoding; history capacity; statistics loops; fixed masks; capability; native weapon binding; stable animation IDs; slot and weapon IDs; a slot's grenades and traps in front of the saved objects, and in the states to roll back to; an untouched grenade and trap save and load as one zero byte; a newcomer's scoreboard score through rollbacks, and every hud cell at a match's start; the hud during a countdown and during play; the new kit's weapons' random states at a spawn, alike in a game that drew from them and in one that loaded a world (and the original and three broken copies failing it); the new kit's weapons' kept state at a spawn: the time of the last tick at every spawn, the fire mode, ammunition, chamber, aim and recoil at a change of kit, alike in a game that played the kit and one that loaded a world, nothing else changed (and the original and five broken copies failing it); a first world's inputs and kit choices in time order, the kit choice the world's, the next kit after each tick as the games that played have it (and the original and three broken copies failing it); a replay's user seek empties every client weapon's sound queue and fx history as ~weapon does, a key frame in play and the original do not; ABI; PE checksum and certificate")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("original", type=Path)
    parser.add_argument("--nasm", default="nasm")
    args = parser.parse_args()
    check(args.original.read_bytes(), args.nasm)


if __name__ == "__main__":
    main()
