#!/usr/bin/env python3
"""Run patch 68 (the Rust patch weapon_kit, rust/patches/src/weapon_kit.rs
and weapon_kit.toml over the weapon-kit crate: a weapon's upgrades in at most
three upgrade records, the weapon kit; ../docs/notes/weapon-kit.md) in the
default build and the original.

    python check_weapon_kit_026g.py <original survarium.exe>

Requires Python `unicorn` (2.1.4) and `capstone` (the alignment probe,
align_probe_026g.py: every aligned-SSE operand our code runs is 16-byte
aligned, which Unicorn does not fault on). The exe's sections are loaded as they
are, the arena with them.

- translate_query (0x2F1750) runs from its `call 0x2F2060` (0x2F1771, the
  code hook weapon_kit.query_room) through the game's own code to the end of
  its loop over the upgrade records (0x2F1960), on a made-up frame and item:
  0x2F2060 (the query's {dict id, modifications*, upgrades*, dictionary} to
  [edi]) is a stub that writes them (or refuses, al 0); the items
  dictionary's item_by_id (0x1F0090) and the configs' lookup (0x0F6B10) are
  stubs over made-up items whose `parameters/modification` names a row of
  the dictionary's modification table (+150h, 2Ch a row); the game's own
  alloca (0x424CA0) and entry push (0x2F2100) run. What must hold, read from
  the entries the game made (a row and the record's value each):
  - records without a kit: the original's entries, [ebp+40h] the item's own
    list;
  - a kit: the records (the kit's own valued 0, as its record was before the
    kit), then one entry for each dict id the kit names (its camouflage,
    muzzle and grip, in that order), the value 0; never more than six; none
    for a dict id the dictionary lacks; the item's own records and list
    unchanged, the list in the room A0h below translate_query's esp, above
    the entries' alloca;
  - a malformed kit (its own slot outside 2..5, naming its own slot, a slot
    or dict id twice, two kits): the original's entries;
  - 0x2F2060 refusing (al 0): the original's way.
- the lobby's upgrades to Flash (0x288180), from 0x2882E3 (after an object's
  `param`) to 0x2882E9 (its PushBack), on a made-up stack and GFx object
  whose interface is the AS3 one (its vtable 0x8FC288; SetMember 0x7C6A20 a
  stub): a kit record's object gets kit_own, kit_camo, kit_muzzle and
  kit_grip (uint, the object's display flag as the bridge's own); any other
  record's nothing, nor an object of another interface; ecx, eax and the
  stack as the original's.
- patch 48's apply (upgrade_preview_026g.asm, its kit_apply dword the build
  wrote) on made-up records and queues: the records afterwards are those
  this file's model of the server's packing gives (decals in slots 0 and 1,
  the kit), at most three, the registers kept; malformed records stay.
- the inventory's is_attached_to: setup_inventory_data (0x288520) from its
  entry to the end of its loop (0x2886C4) with the game's own pair push
  (0x28B0F0) and lookup, on a made-up lobby client and inventory; the
  item-to-Flash function (0x287E30) a stub that writes down each item and
  the host it was given, Scaleform's CreateArray, CreateObject and PushBack
  stubs. A scoped weapon whose kit names a camouflage: the camouflage (the
  lowest item id of that dict id) is attached to the weapon (the original:
  to nothing), its decal and scope as in the original, a second item of the
  same camouflage and an unworn one to nothing.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, \
    UC_X86_REG_ESP, UC_X86_REG_EBP, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

import align_probe_026g as align
import patch_026g as base

CALL_SITE, QUERY_BACK, LOOP_END = 0x2F1771, 0x2F1776, 0x2F1960
SETUP_INVENTORY, SETUP_LOOP_END, ITEM_TO_FLASH = 0x288520, 0x2886C4, 0x287E30
CREATE_OBJECT, CREATE_ARRAY = 0x511700, 0x511710
GET_QUERY, ITEM_BY_ID, CHILD = 0x2F2060, 0x1F0090, 0x0F6B10
BRIDGE_SITE, BRIDGE_BACK = 0x2882E3, 0x2882E9
AS3_INTERFACE, SET_MEMBER = 0x8FC288, 0x7C6A20   # the AS3 object interface's vtable, its SetMember (slot 5)
ROOM = 0xA0                            # weapon_kit_026g.asm: six records and their vector
KIT_MAGIC = 0xA7
FIELD_SLOTS = (2, 4, 5)                # camouflage, muzzle, grip
FIELD_SHIFTS = (0, 16, 32)
KIT_ORDER = (3, 2, 4, 5)               # the kit's own: the first of scope, camouflage, muzzle, grip

STACK, STOP = 0x21000000, 0x21100000
MENU, CLIENT = 0x32000000, 0x34000000
DICTIONARY, ENTRIES, CONFIGS, ITEM, QUERY, GFX = 0x30000000, 0x30010000, 0x30020000, 0x30030000, 0x30040000, \
    0x30050000
REGS = (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP)
MOD_ROW = 0x2C


# ------------------------------------------------------------ the model
# A record: (id, dict id, value, slot). The weapon-kit crate's rules
# (srp-026g/crates/weapon-kit), written again here: the server packs so, the
# client reads so.

class Malformed(Exception):
    pass


def kit_of(r):
    """None for a plain record; (own slot, [(dict id, slot) named]); Malformed."""
    if r[3] != 3 or r[2] >> 56 != KIT_MAGIC:
        return None
    own = (r[2] >> 48) & 0xFF
    if own not in KIT_ORDER:
        raise Malformed(f"own slot {own}")
    named = [((r[2] >> shift) & 0xFFFF, slot) for slot, shift in zip(FIELD_SLOTS, FIELD_SHIFTS)
             if (r[2] >> shift) & 0xFFFF]
    if any(slot == own for _, slot in named):
        raise Malformed("names its own slot")
    return own, named


def unpack(wire):
    """By logical slot: {slot: (id, dict id, value)} (a name's id 0); Malformed."""
    if len(wire) > 3:
        raise Malformed("more than three")
    logical, names, kits = {}, set(), 0

    def put(slot, held, named=False):
        if slot in logical:
            raise Malformed(f"slot {slot} twice")
        # a name beside a record of its dict id (two of one decal are fine)
        if any(other[1] == held[1] and (named or s in names) for s, other in logical.items()):
            raise Malformed(f"dict id {held[1]} twice")
        logical[slot] = held
        if named:
            names.add(slot)

    for r in wire:
        k = kit_of(r)
        if k is None:
            if r[3] > 5:
                raise Malformed(f"slot {r[3]}")
            put(r[3], r[:3])
            continue
        kits += 1
        if kits > 1:
            raise Malformed("two kits")
        put(k[0], (r[0], r[1], 0))
        for d, slot in k[1]:
            put(slot, (0, d, 0), named=True)
    return logical


def pack(logical):
    wire = [(*logical[slot], slot) for slot in (0, 1) if slot in logical]
    own = next((slot for slot in KIT_ORDER if slot in logical), None)
    if own is not None:
        value = KIT_MAGIC << 56 | own << 48
        for slot, shift in zip(FIELD_SLOTS, FIELD_SHIFTS):
            if slot in logical and slot != own:
                value |= (logical[slot][1] & 0xFFFF) << shift
        wire.append((logical[own][0], logical[own][1], value, 3))
    return wire


def apply(wire, changes):
    """Patch 48's queue {upgrade, dict id, slot or 0xFF} as the server
    applies it (items_rules::attach_upgrade_in, detach_upgrade); malformed
    records as they are."""
    try:
        logical = unpack(wire)
    except Malformed:
        return list(wire)
    held_is = lambda h, up, d: h[0] == up if h[0] else h[1] == d
    for up, d, slot in changes:
        if slot == 0xFF:
            logical = {s: h for s, h in logical.items() if not held_is(h, up, d)}
            continue
        if slot > 5:
            continue
        if slot in logical and held_is(logical[slot], up, d) and logical[slot][1] == d:
            continue
        value = next((h[2] for h in logical.values() if held_is(h, up, d) and h[1] == d), 0)
        logical = {s: h for s, h in logical.items() if not held_is(h, up, d)}
        logical[slot] = (up, d, value)
    return pack(logical)


def expand(wire, known):
    """translate_query's list for `wire`, None for the item's own (no kit, or
    malformed)."""
    try:
        kits = [k for k in map(kit_of, wire) if k is not None]
        if not kits:
            return None
        unpack(wire)
    except Malformed:
        return None
    out = [(r[0], r[1], 0, r[3]) if kit_of(r) else r for r in wire]
    return out + [(0, d, 0, slot) for d, slot in kits[0][1] if d in known]


def kit(own, own_slot, camo=0, muzzle=0, grip=0):
    return (own[0], own[1], KIT_MAGIC << 56 | own_slot << 48 | grip << 32 | muzzle << 16 | camo, 3)


# ------------------------------------------------------------ the runs

class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at in (DICTIONARY, ENTRIES, CONFIGS, ITEM, QUERY, GFX, STOP):
            emu.mem_map(at, 0x10000, UC_PROT_ALL)
        emu.mem_map(STACK - 0x20000, 0x40000, UC_PROT_ALL)
        for at in (GET_QUERY, ITEM_BY_ID, CHILD, GFX + 0x100, SET_MEMBER, ITEM_TO_FLASH, CREATE_OBJECT, CREATE_ARRAY,
                   GFX + 0x110, GFX + 0x120, GFX + 0x130):
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=at, end=at)
        self.stops = ()
        emu.hook_add(UC_HOOK_CODE, self.stop)
        self.children, self.items, self.refuse, self.set_members, self.attached = {}, {}, False, [], []
        for at in (MENU, CLIENT):
            emu.mem_map(at, 0x200000 if at == CLIENT else 0x10000, UC_PROT_ALL)

    def u32(self, at, value=None):
        if value is None:
            return struct.unpack("<I", self.emu.mem_read(at, 4))[0]
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def text(self, at):
        return bytes(self.emu.mem_read(at, 64)).split(b"\0")[0]

    def ret(self, value, pops):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        self.emu.reg_write(UC_X86_REG_EAX, value)
        self.emu.reg_write(UC_X86_REG_EIP, self.u32(sp))
        self.emu.reg_write(UC_X86_REG_ESP, sp + 4 + pops)

    def stub(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        if address == GET_QUERY:            # edi = [ebp+38h]: the query's four dwords; al
            if not self.refuse:
                emu.mem_write(emu.reg_read(UC_X86_REG_EDI), bytes(emu.mem_read(QUERY, 16)))
            self.ret(0 if self.refuse else 1, 0)
        elif address == ITEM_BY_ID:         # eax the dictionary, [esp+4] the id; ret 4
            self.ret(self.items.get(self.u32(sp + 4), 0), 4)
        elif address == CHILD:              # eax the table, edi the key: the child
            key = (emu.reg_read(UC_X86_REG_EAX), self.text(emu.reg_read(UC_X86_REG_EDI)))
            if key not in self.children:
                raise KeyError(f"CHILD {key[0]:#x} {key[1]!r}")
            self.ret(self.children[key], 0)
        elif address == ITEM_TO_FLASH:      # eax the item, [esp+4] the menu, +8 the value, +0Ch the host; ret 0Ch
            self.attached.append((self.u32(emu.reg_read(UC_X86_REG_EAX)), self.u32(sp + 12)))
            self.ret(0, 12)
        elif address == CREATE_OBJECT:      # thiscall (&value, ...): ret 10h
            self.emu.mem_write(self.u32(sp + 4), struct.pack("<III", GFX, 8, 0x5150))
            self.ret(0, 16)
        elif address == CREATE_ARRAY:       # thiscall (&value): ret 4
            self.emu.mem_write(self.u32(sp + 4), struct.pack("<III", GFX, 9, 0x5160))
            self.ret(0, 4)
        elif address == GFX + 0x110:        # PushBack(data, index, &value): ret 0Ch
            self.ret(0, 12)
        elif address == GFX + 0x120:        # the lobby client's getter (thiscall): the client
            self.ret(CLIENT, 0)
        elif address == GFX + 0x130:        # release: ret 8
            self.ret(0, 8)
        else:                               # SetMember(data, name, &value, display): thiscall, ret 16
            data, name, value, display = (self.u32(sp + 4 * i) for i in range(1, 5))
            self.set_members.append((emu.reg_read(UC_X86_REG_ECX), data, self.text(name).decode(),
                                     self.u32(value + 4), self.u32(value + 8), display & 0xFF))
            self.ret(0, 16)

    def stop(self, emu, address, size, _):
        if address in self.stops and self.started:
            self.stopped = address
            emu.emu_stop()
        self.started = True

    def run(self, start, stops, sp, regs):
        for reg, value in regs.items():
            self.emu.reg_write(reg, value)
        self.emu.reg_write(UC_X86_REG_ESP, sp)
        self.stops, self.stopped, self.started = stops, None, False
        self.emu.emu_start(start, 0, count=200000)
        return {reg: self.emu.reg_read(reg) for reg in REGS}, self.emu.reg_read(UC_X86_REG_ESP)

    def dictionary(self, dicts):
        """Made-up items: dict id d's config `parameters/modification` is
        10000 + d, row i of the modification table."""
        self.items, self.children = {}, {}
        self.emu.mem_write(DICTIONARY, bytes(0x1000))
        self.emu.mem_write(ENTRIES, bytes(0x10000))
        self.emu.mem_write(CONFIGS, bytes(0x10000))
        rows = DICTIONARY + 0x400
        self.u32(DICTIONARY + 0x150, rows)
        self.u32(DICTIONARY + 0x154, len(dicts))
        self.rows = {}
        for i, d in enumerate(dicts):
            entry = ENTRIES + 0x400 * i
            config, root = CONFIGS + 0x100 * i, CONFIGS + 0x100 * i + 0x40
            self.u32(entry + 0x114, config)
            self.u32(config + 0x108, root)
            self.u32(entry + 0x234, 0)          # no default modifications
            self.u32(entry + 0x238, 0)
            self.items[d] = entry
            node = root + 0x20
            self.u32(node, 10000 + d)
            self.children[(root, b"parameters")] = root + 0x10
            self.children[(root + 0x10, b"modification")] = node
            self.u32(rows + MOD_ROW * i, 10000 + d)
            self.rows[rows + MOD_ROW * i] = d

    def records(self, at, records):
        """An item record's upgrades at `at` (+60h: begin, end, capacity end; storage +6Ch)."""
        self.emu.mem_write(at, bytes(0xB4))
        storage = at + 0x6C
        for i, (rid, d, value, slot) in enumerate(records):
            self.emu.mem_write(storage + 0x18 * i, struct.pack("<IIQB", rid, d, value, slot) + bytes(7))
        self.u32(at + 0x60, storage)
        self.u32(at + 0x64, storage + 0x18 * len(records))
        self.u32(at + 0x68, storage + 0x18 * 3)

    def read_records(self, vector):
        begin, end = self.u32(vector), self.u32(vector + 4)
        out = []
        for at in range(begin, end, 0x18):
            rid, d, value, slot = struct.unpack("<IIQB", bytes(self.emu.mem_read(at, 0x11)))
            out.append((rid, d, value, slot))
        return out

    # translate_query from its call of 0x2F2060 to the end of its record loop
    def translate(self, records, refuse=False):
        emu = self.emu
        self.records(ITEM, records)
        self.u32(ITEM + 0x14, ITEM + 0x20)          # no modifications
        self.u32(ITEM + 0x18, ITEM + 0x20)
        self.u32(ITEM + 0x1C, ITEM + 0x60)
        for i, value in enumerate((501, ITEM + 0x14, ITEM + 0x60, DICTIONARY)):
            self.u32(QUERY + 4 * i, value)
        self.items[501] = self.items.get(501, ENTRIES + 0xF000)
        self.refuse = refuse
        ebp = STACK
        sp = ebp - 0xA0
        emu.mem_write(STACK - 0x20000, bytes(0x40000))
        before = bytes(emu.mem_read(ITEM, 0xB4))
        regs, esp = self.run(CALL_SITE, (QUERY_BACK,), sp,
                             {UC_X86_REG_EBP: ebp, UC_X86_REG_ESI: 0x55555555, UC_X86_REG_EDI: ebp + 0x38,
                              UC_X86_REG_EBX: 0x11111111})
        if self.stopped != QUERY_BACK:
            return None
        # past the lohook at 0x2F1776 (its span's end, where the original
        # goes on too): [ebp+40h] as the loop will read it
        regs, past = self.run(QUERY_BACK, (QUERY_BACK + 6,), esp, regs)
        if self.stopped != QUERY_BACK + 6 or past != esp:
            return None
        at_back = esp, self.u32(ebp + 0x40), regs
        if refuse:
            return at_back, None, bytes(emu.mem_read(ITEM, 0xB4)) == before
        self.run(QUERY_BACK + 6, (LOOP_END,), esp, regs)
        if self.stopped != LOOP_END:
            return at_back, "the loop not reached", False
        begin, end = self.u32(ebp + 8), self.u32(ebp + 0xC)
        entries = []
        for at in range(begin, end, 16):
            row, _, low, high = struct.unpack("<IIII", bytes(emu.mem_read(at, 16)))
            entries.append((self.rows.get(row), low | high << 32))
        return at_back, (entries, begin), bytes(emu.mem_read(ITEM, 0xB4)) == before

    # the bridge from after `param` to the PushBack: esi the item, [esp+80h] the index
    def bridge(self, records, index, kind=8, as3=True):
        emu = self.emu
        self.records(ITEM, records)
        self.set_members = []
        sp = STACK - 0x1000
        emu.mem_write(sp, bytes(0x100))
        self.u32(GFX, AS3_INTERFACE if as3 else GFX + 0x80)   # the interface's vtable: the AS3 one, or another
        self.u32(GFX + 0x80 + 0x14, GFX + 0x100)     # the other's SetMember
        self.u32(sp + 0x30, GFX)
        self.u32(sp + 0x34, kind)
        self.u32(sp + 0x38, 0x6D6D6D6D)               # the object's data
        self.u32(sp + 0x48, GFX + 0x200)              # the array value the PushBack's ecx reads
        self.u32(GFX + 0x200, GFX + 0x300)
        emu.mem_write(sp + 0x80, bytes([index]))
        before = bytes(emu.mem_read(sp, 0x100))
        regs, esp = self.run(BRIDGE_SITE, (BRIDGE_BACK,), sp,
                             {UC_X86_REG_ESI: ITEM, UC_X86_REG_EDI: 0x44444444, UC_X86_REG_EBX: 0x11111111,
                              UC_X86_REG_EBP: 0x22222222, UC_X86_REG_EAX: 0, UC_X86_REG_ECX: 0,
                              UC_X86_REG_EDX: 0x0D0D0D0D})
        return self.stopped, esp - sp, regs, bytes(emu.mem_read(sp, 0x100)) == before, list(self.set_members)

    # setup_inventory_data: the menu's lobby client's inventory, each item's is_attached_to
    def attached_to(self, inventory):
        """`inventory`: [(item id, dict id, [records])]; {item id: the host
        0x287E30 was given}."""
        emu = self.emu
        emu.mem_write(MENU, bytes(0x1000))
        self.u32(MENU + 0x9C, MENU + 0x100)
        self.u32(MENU + 0x100 + 0x53AC, MENU + 0x200)
        self.u32(MENU + 0x200, MENU + 0x300)
        self.u32(MENU + 0x300 + 0x3C, GFX + 0x120)
        self.u32(MENU + 0x690, MENU + 0x400)
        self.u32(MENU + 0x400 + 0x108, MENU + 0x500)
        self.u32(MENU + 0x504, MENU + 0x600)
        self.u32(GFX, GFX + 0x80)
        self.u32(GFX + 0x80 + 0x34, GFX + 0x110)
        self.u32(GFX + 0x80 + 0x08, GFX + 0x130)
        items = CLIENT + 0x1EB000
        for i, (item_id, dict_id, records) in enumerate(inventory):
            at = items + 0xB4 * i
            self.records(at, records)
            self.u32(at, item_id)
            self.emu.mem_write(at + 4, struct.pack("<H", dict_id))
        self.u32(CLIENT + 0x1EA978, items)
        self.u32(CLIENT + 0x1EA97C, items + 0xB4 * len(inventory))
        sp = STACK - 0x3000
        emu.mem_write(sp - 0x2000, bytes(0x2100))
        self.u32(sp, STOP)
        self.u32(sp + 4, MENU)
        self.attached = []
        self.run(SETUP_INVENTORY, (SETUP_LOOP_END,), sp, {UC_X86_REG_EBX: 0x11111111, UC_X86_REG_ESI: 0x33333333,
                                                         UC_X86_REG_EDI: 0x44444444, UC_X86_REG_EBP: 0x22222222})
        return self.stopped, dict(self.attached)

    # patch 48's apply: edi the item record, ebx its state
    def preview_apply(self, apply_va, state, records, changes):
        emu = self.emu
        self.records(ITEM, records)
        emu.mem_write(state, bytes(0xA0))
        self.u32(state + 4, len(changes))
        for i, change in enumerate(changes):
            emu.mem_write(state + 0x64 + 12 * i, struct.pack("<III", *change))
        sp = STACK - 0x2000
        self.u32(sp, STOP)
        regs, esp = self.run(apply_va, (STOP,), sp,
                             {UC_X86_REG_EDI: ITEM, UC_X86_REG_EBX: state, UC_X86_REG_ESI: 0x33333333,
                              UC_X86_REG_EBP: 0x22222222, UC_X86_REG_EAX: 0x0A0A0A0A,
                              UC_X86_REG_ECX: 0x0C0C0C0C, UC_X86_REG_EDX: 0x0D0D0D0D})
        return self.stopped, esp - sp, regs, self.read_records(ITEM + 0x60)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    align.install()                 # every Uc from here: aligned-SSE operands checked
    original = open(sys.argv[1], "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    built, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(), None, late_join)
    linked = base.arena()
    stock, patched = Run(original), Run(bytes(built))
    failed = 0

    def report(ok, text):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {text}")

    # dict ids: decals 603, 604; camouflages 601, 602; a scope variant 2002;
    # a muzzle 2201; a grip 2301; 2999 the dictionary lacks
    known = (501, 601, 602, 603, 604, 2002, 2201, 2301)
    for run in (stock, patched):
        run.dictionary(known)
    DECAL0, DECAL1 = (1603, 603, 0x0102030405060708, 0), (1604, 604, 0x1112131415161718, 1)
    SCOPE, CAMO, CAMO2 = (1605, 2002), (1601, 601), (1602, 602)

    print("translate_query (0x2F1771 to the end of its record loop, 0x2F1960)")
    plain = [
        ("an armour's camouflage and two decals", [DECAL0, DECAL1, (1601, 601, 0, 3)]),
        ("a weapon's scope record of before (value 0)", [DECAL0, (1605, 2002, 0, 3)]),
        ("no record", []),
        ("a slot-3 record whose value's top byte is not the kit's", [(1605, 2002, 0x5A07 << 48, 3)]),
        ("the kit's byte in another slot", [(1601, 601, KIT_MAGIC << 56 | 2 << 48 | 602, 2)]),
        ("a malformed kit: its own slot 7", [DECAL0, (1605, 2002, KIT_MAGIC << 56 | 7 << 48 | 601, 3)]),
        ("a malformed kit: it names its own slot", [kit(CAMO, 2, camo=602)]),
        ("a malformed kit: a dict id a record holds already", [DECAL0, (1601, 601, 0, 2), kit(SCOPE, 3, camo=601)]),
        ("a malformed kit: a camouflage named beside a record in slot 2",
         [(1602, 602, 0, 2), kit(SCOPE, 3, camo=601, grip=2301)]),
        ("a malformed kit: two kits", [kit(SCOPE, 3), kit(CAMO, 2)]),
    ]
    for name, records in plain:
        a, b = stock.translate(records), patched.translate(records)
        # [ebp+40h] is the query's upgrades* (the item's own list); the entries' alloca
        # is the room lower than the original's (every cook takes it)
        ok = a is not None and b is not None and b[0][1] == ITEM + 0x60 \
            and a[1][0] == b[1][0] and b[2] and [d for d, _ in b[1][0]] == [r[1] for r in records] \
            and [v for _, v in b[1][0]] == [r[2] for r in records] and expand(records, known) is None
        report(ok, f"{name}: the original's entries ({[d for d, _ in b[1][0]] if b and b[1] else '?'}), "
                   f"[ebp+40h] the item's own list, the item unchanged")

    kits = [
        ("decal, decal, the kit: scope, camouflage, muzzle and grip named",
         [DECAL0, DECAL1, kit(SCOPE, 3, camo=601, muzzle=2201, grip=2301)]),
        ("decal, decal, the kit: scope and camouflage (step A's most)",
         [DECAL0, DECAL1, kit(SCOPE, 3, camo=601)]),
        ("the kit alone: its own the camouflage, a muzzle named", [kit(CAMO, 2, muzzle=2201)]),
        ("the camouflage alone, a kit naming nothing", [kit(CAMO, 2)]),
        ("the dictionary lacks one it names (left out)", [kit(CAMO, 2, grip=2999)]),
        ("a kit with nothing named: its own alone", [DECAL0, kit(SCOPE, 3)]),
        ("the same decal twice beside the kit", [DECAL0, (1606, 603, 0x22, 1), kit(SCOPE, 3, camo=601)]),
    ]
    for name, records in kits:
        a, b = stock.translate(records), patched.translate(records)
        want = expand(records, known)
        got = b[1][0] if b and b[1] else None
        room, list_at = (b[0][0], b[0][1]) if b else (0, 0)
        ok = a is not None and b is not None and got is not None and b[2] and want is not None \
            and [d for d, _ in got] == [r[1] for r in want] \
            and [v for _, v in got] == [r[2] for r in want] \
            and [d for d, _ in a[1][0]] == [r[1] for r in records] \
            and len(got) <= 6 and room == a[0][0] - ROOM and list_at == room + 0x90 \
            and b[1][1] + 16 * len(got) <= room
        report(ok, f"{name}: the entries {[d for d, _ in got] if got else '?'} (the original's "
                   f"{[d for d, _ in a[1][0]] if a and a[1] else '?'}), the kit's own valued 0, the list in "
                   f"the room A0h below translate_query's esp (above the entries' alloca), the item's own "
                   f"records unchanged")

    six = [DECAL0, DECAL1, kit(SCOPE, 3, camo=601, muzzle=2201, grip=2301)]
    b = patched.translate(six)
    report(b is not None and len(b[1][0]) == 6 and stock.translate(six)[1][0][2][0] == 2002,
           "never more than six: the most a kit gives is decal, decal, the kit's own and three named")
    # the same modifications and values as the records srp sent before the kit
    before = [DECAL0, DECAL1, (1605, 2002, 0, 3), (1601, 601, 0, 2)]
    b = patched.translate([DECAL0, DECAL1, kit(SCOPE, 3, camo=601)])
    report(b is not None and sorted(b[1][0]) == sorted((r[1], r[2]) for r in before),
           "two decals, a scope and a camouflage: the modifications and values the four records of before "
           "would give (no simulation change)")

    for records in ([DECAL0], [DECAL0, kit(SCOPE, 3, camo=601)]):
        a, b = stock.translate(records, refuse=True), patched.translate(records, refuse=True)
        report(a is not None and b is not None and b[0][1] == a[0][1] and b[2],
               f"0x2F2060 refusing (another thread's query), {len(records)} record(s): the original's way, "
               f"[ebp+40h] untouched")

    print("the upgrades to Flash (0x2882E3 to 0x2882E9)")
    records = [DECAL0, DECAL1, kit(SCOPE, 3, camo=601, muzzle=2201, grip=2301)]
    for index in range(3):
        for kind in (8, 0x0A | 0x40):
            for as3 in (True, False):
                a, b = stock.bridge(records, index, kind, as3), patched.bridge(records, index, kind, as3)
                same = a[0] == b[0] == BRIDGE_BACK and a[1] == b[1] == 0 and a[3] and b[3] \
                    and a[2][UC_X86_REG_ECX] == b[2][UC_X86_REG_ECX] \
                    and a[2][UC_X86_REG_EAX] == b[2][UC_X86_REG_EAX] \
                    and all(a[2][r] == b[2][r] for r in (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI,
                                                         UC_X86_REG_EBP, UC_X86_REG_EDX))
                display = 1 if (kind & 0x8F) == 0x0A else 0
                members = (("kit_own", 3), ("kit_camo", 601), ("kit_muzzle", 2201), ("kit_grip", 2301))
                want = [(GFX, 0x6D6D6D6D, name, 4, value, display) for name, value in members] \
                    if index == 2 and as3 else []
                report(same and not a[4] and b[4] == want,
                       f"record {index} ({'the kit' if index == 2 else 'a decal'}), the object "
                       f"{'a display object' if display else 'plain'}, "
                       f"{'the AS3 interface' if as3 else 'another interface'}: "
                       + ("kit_own, kit_camo, kit_muzzle, kit_grip set" if want else "nothing set")
                       + ", registers and stack the original's")

    print("patch 48's apply (its kit_apply slot the build's weapon_kit_preview_apply)")
    late = base.weapon_camo_va(late_join) + base.UPGRADE_PREVIEW_OFFSET
    slot = late + base.UPGRADE_PREVIEW_KIT_APPLY
    target = base.arena_target(linked, base.WEAPON_KIT_PREVIEW_APPLY)
    report(patched.u32(slot) == target, f"the slot at {slot:#x} holds weapon_kit_preview_apply ({target:#x})")
    code = bytes(patched.emu.mem_read(late, 0x1000))
    at = code.find(b"\x83\x3D" + struct.pack("<I", slot) + b"\x00")
    apply_va = late + at
    state = base.late_join_state_va(late_join) + base.UPGRADE_PREVIEW_STATE_OFFSET
    T = 0xFF
    cases = [
        ("a camouflage on a weapon with nothing: the kit, the camouflage its own",
         [], [(1601, 601, 2)], [(1601, 601, KIT_MAGIC << 56 | 2 << 48, 3)]),
        ("a second decal beside decal, scope and camouflage: four in three records",
         [DECAL0, kit(SCOPE, 3, camo=601)], [(1604, 604, 1)],
         [DECAL0, (1604, 604, 0, 1), (1605, 2002, KIT_MAGIC << 56 | 3 << 48 | 601, 3)]),
        ("another camouflage replaces the one the kit names",
         [DECAL0, kit(SCOPE, 3, camo=601)], [(1602, 602, 2)],
         [DECAL0, (1605, 2002, KIT_MAGIC << 56 | 3 << 48 | 602, 3)]),
        ("the scope taken off: the camouflage becomes the kit's own",
         [DECAL0, kit(SCOPE, 3, camo=601)], [(1605, 2002, T)],
         [DECAL0, (0, 601, KIT_MAGIC << 56 | 2 << 48, 3)]),
        ("a scope put on a weapon whose kit is its camouflage",
         [kit(CAMO, 2)], [(1605, 2002, 3)], [(1605, 2002, KIT_MAGIC << 56 | 3 << 48 | 601, 3)]),
        ("a named camouflage taken off by its item (no id in the kit): by its dict id",
         [kit(SCOPE, 3, camo=601)], [(1601, 601, T)], [(1605, 2002, KIT_MAGIC << 56 | 3 << 48, 3)]),
        ("a decal moved from slot 0 to 1 keeps its place",
         [DECAL0, kit(CAMO, 2)], [(1603, 603, 1)], [(1603, 603, DECAL0[2], 1), (1601, 601, KIT_MAGIC << 56 | 2 << 48, 3)]),
        ("a put of what is there already: nothing changes",
         [DECAL0, kit(SCOPE, 3, camo=601)], [(1601, 601, 2)], [DECAL0, kit(SCOPE, 3, camo=601)]),
        ("everything taken off: no record", [DECAL0, kit(CAMO, 2)], [(1603, 603, T), (1601, 601, T)], []),
        ("malformed records (a kit naming its own slot) stay as they are",
         [DECAL0, kit(CAMO, 2, camo=602)], [(1605, 2002, 3)], [DECAL0, kit(CAMO, 2, camo=602)]),
    ]
    for name, records, changes, literal in cases:
        stopped, moved, regs, got = patched.preview_apply(apply_va, state, records, changes)
        want = apply(records, changes)
        kept = regs[UC_X86_REG_EDI] == ITEM and regs[UC_X86_REG_EBX] == state and regs[UC_X86_REG_ESI] == 0x33333333 \
            and regs[UC_X86_REG_EBP] == 0x22222222 and regs[UC_X86_REG_EAX] == 0x0A0A0A0A \
            and regs[UC_X86_REG_ECX] == 0x0C0C0C0C and regs[UC_X86_REG_EDX] == 0x0D0D0D0D
        report(stopped == STOP and moved == 4 and kept and got == want == literal and len(got) <= 3,
               f"{name}: {len(got)} record(s) as the model and as written here, every register kept")

    print("the inventory's is_attached_to (setup_inventory_data 0x288520 to 0x2886C4)")
    WEAPON, DECAL, SCOPE_ITEM, CAMO_A, CAMO_A2, CAMO_B = 100, 1603, 1605, 1601, 1650, 1602
    inventory = [(WEAPON, 501, [DECAL0, kit(SCOPE, 3, camo=601)]), (CAMO_A2, 601, []), (DECAL, 603, []),
                 (SCOPE_ITEM, 2101, []), (CAMO_A, 601, []), (CAMO_B, 602, [])]
    a, b = stock.attached_to(inventory), patched.attached_to(inventory)
    want = {WEAPON: 0, DECAL: WEAPON, SCOPE_ITEM: WEAPON, CAMO_A: WEAPON, CAMO_A2: 0, CAMO_B: 0}
    report(a[0] == b[0] == SETUP_LOOP_END and a[1] == {**want, CAMO_A: 0} and b[1] == want,
           f"a scoped weapon whose kit names camouflage 601: its camouflage item {CAMO_A} (the lowest of the "
           f"two of 601) attached to the weapon {WEAPON} (the original: to nothing), the decal and the scope "
           f"to it as in the original, the other 601 and an unworn camouflage to nothing")
    plain = [(WEAPON, 501, [DECAL0, (1601, 601, 0, 3)]), (DECAL, 603, []), (CAMO_A, 601, [])]
    a, b = stock.attached_to(plain), patched.attached_to(plain)
    report(a == b and a[1] == {WEAPON: 0, DECAL: WEAPON, CAMO_A: WEAPON},
           "records without a kit: the original's marks")
    print("the alignment probe (align_probe_026g.py)")
    align.report(report)             # report counts a failure
    print(f"weapon kit: {'all passed' if not failed else f'{failed} FAILED'}")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
