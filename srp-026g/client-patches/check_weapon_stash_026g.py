#!/usr/bin/env python3
"""Run weapon_stash_026g.asm's find_kit_or_show (patch 32's find_kit, grown:
client_patch.md section 3.30) in the build, with the game's own
inventory_item_descr ctor (0x2F7C10) and copy (0x270390), player_profile
ctor (0x2F73F0) and copy (0x284D10) and the inventory's find (0x277AA0), on
a made-up account: a profile with three kits and its item table, the
inventory vector after it, the lobby client's dictionary behind it.
    python check_weapon_stash_026g.py <original survarium.exe>
Requires Python `unicorn` (2.1.4). The dictionary's item_by_id (0x1F0090)
and the game's allocator (0x420789, 0xE4F90) are stubs that write down how
they were called. What must hold:
- a kit's held weapon (weapon 1, or weapon 2 without one): patch 32's
  find_kit's answer, the profile given back, nothing allocated;
- a weapon no kit holds as its held weapon (only in the stash; a weapon 2
  behind a weapon 1; already in the table but in no kit): the chosen kit and
  a scratch profile, made once, which is the account's profile with that
  weapon as the chosen kit's weapon 1 (its record appended after the last
  one when the table lacks it, a deep copy whose two vectors point into the
  scratch's own record), a revision of the chosen kit that no server sends
  and that changes with every scratch;
- the account's profile and the inventory are never written;
- no kit and the profile given back (eax and edx = -1) for armour in the
  stash, an item the inventory lacks or the dictionary does not know, a
  profile without the inventory behind it (a character's copy), no profile,
  a full table (69 records), a failed allocation;
- every register but eax, edx and the flags kept, and the stack.
Says nothing about what the game draws.
"""
import struct
import sys
from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_EIP, \
    UC_X86_REG_ESP, UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP

import patch_026g as base

ITEM_BY_ID, ALLOCATION_NAME, ALLOCATE = 0x1F0090, 0x420789, 0x0E4F90
ITEM_CTOR = 0x2F7C10                           # ecx = this
GAME_ALLOCATOR, ALLOCATION_WHERE = 0x1F1D9C4, 0xAC1220

CLIENT = 0x20000000                            # the lobby client: the profile at +1E6344h
PROFILE = CLIENT + 0x1E6344
PLAYERS = 0x20400000                           # [lc+1E56C0h]: +53A4h the items dictionary
DICTIONARY = 0x20500000
ENTRIES = 0x20600000                           # dictionary items: +228h the category
INVENTORY = 0x20700000                         # the inventory's records
SCRATCH = 0x20800000                           # what the allocator stub gives
OTHER = 0x20900000                             # a profile with no inventory behind it
STACK, STOP = 0x21000000, 0x21100000
ALLOCATOR = 0x5A110C00

KIT0, KIT_SIZE, SLOTS, ITEMS, TABLE, ITEM_SIZE = 0x58, 0x704, 0x40, 0x1564, 0x1568, 0xB4
REVISIONS, CHOSEN, KITS = 0x45FC, 0x4632, 0x57
WEAPON_1, WEAPON_2 = 7, 10
RIFLE, PISTOL, VEST = 15, 8, 3                 # categories: a rifle, a pistol, armour

SENTINELS = {UC_X86_REG_EBX: 0x11111111, UC_X86_REG_EBP: 0x22222222, UC_X86_REG_ESI: 0x33333333,
             UC_X86_REG_EDI: 0x44444444, UC_X86_REG_ECX: 0}


class Emu:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        headers = base.Headers(data)
        for _, rva, vsize, raw, rsize, _ in headers.sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((CLIENT, 0x200000), (PLAYERS, 0x10000), (DICTIONARY, 0x10000), (ENTRIES, 0x10000),
                         (INVENTORY, 0x100000), (SCRATCH, 0x10000), (OTHER, 0x10000), (STACK - 0x10000, 0x10000),
                         (STOP, 0x1000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        self.pristine = {at: bytes(emu.mem_read(at, 0x10000)) for at in (SCRATCH,)}
        for at in (ITEM_BY_ID, ALLOCATION_NAME, ALLOCATE):
            emu.hook_add(UC_HOOK_CODE, self.stub, begin=at, end=at)
        self.state = base.late_join_state_va(open(base.late_join_bin(), "rb").read()) \
            + base.WEAPON_PREVIEW_STATE_OFFSET
        late_join = open(base.late_join_bin(), "rb").read()
        self.entry = base.weapon_camo_va(late_join) + base.WEAPON_STASH_OFFSET

    def u32(self, at, value=None):
        if value is None:
            return struct.unpack("<I", self.emu.mem_read(at, 4))[0]
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def u16(self, at, value=None):
        if value is None:
            return struct.unpack("<H", self.emu.mem_read(at, 2))[0]
        self.emu.mem_write(at, struct.pack("<H", value))

    def u8(self, at, value=None):
        if value is None:
            return self.emu.mem_read(at, 1)[0]
        self.emu.mem_write(at, bytes([value]))

    def ret(self, value, pop):
        esp = self.emu.reg_read(UC_X86_REG_ESP)
        self.emu.reg_write(UC_X86_REG_EAX, value)
        self.emu.reg_write(UC_X86_REG_EIP, self.u32(esp))
        self.emu.reg_write(UC_X86_REG_ESP, esp + 4 + pop)

    def stub(self, emu, address, size, _):
        esp = emu.reg_read(UC_X86_REG_ESP)
        if address == ITEM_BY_ID:
            dict_id = self.u32(esp + 4)
            self.calls.append(("item_by_id", emu.reg_read(UC_X86_REG_EAX), dict_id))
            self.ret(self.entries.get(dict_id, 0), 4)
        elif address == ALLOCATION_NAME:
            self.calls.append(("allocation_name", emu.reg_read(UC_X86_REG_ECX)))
            self.ret(ALLOCATION_WHERE + 8, 0)
        elif address == ALLOCATE:
            self.calls.append(("allocate", emu.reg_read(UC_X86_REG_ESI), emu.reg_read(UC_X86_REG_EAX),
                               self.u32(esp + 4)))
            self.ret(0 if self.fail_allocation else SCRATCH, 4)

    def call(self, address, regs):
        emu = self.emu
        for reg, value in regs.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, STACK - 0x100)
        self.u32(STACK - 0x100, STOP)
        emu.emu_start(address, STOP)

    def construct_item(self, at):
        self.call(ITEM_CTOR, {UC_X86_REG_ECX: at})

    def setup(self, kits, table, inventory, chosen=0, categories=None, fail_allocation=False, fresh=True):
        """kits: [{slot: item id}]; table: the profile's item ids (in kits or
        not); inventory: [(item id, dict id)]; categories: {dict id: item
        category} the dictionary knows."""
        emu = self.emu
        self.calls, self.fail_allocation = [], fail_allocation
        if fresh:
            emu.mem_write(self.state, bytes(base.WEAPON_PREVIEW_STATE_SIZE))
            emu.mem_write(SCRATCH, self.pristine[SCRATCH])
        emu.mem_write(PROFILE, bytes(0x4640))
        self.call(0x2F73F0, {UC_X86_REG_ECX: PROFILE})      # the profile ctor
        self.u8(PROFILE + KITS, len(kits))
        self.u8(PROFILE + CHOSEN, chosen)
        index = {item: i for i, item in enumerate(table)}
        for i, item in enumerate(table):
            record = PROFILE + TABLE + i * ITEM_SIZE
            self.u32(record, item)
            self.u16(record + 4, item % 1000)
            self.u32(record + 8, 1000000)
        self.u8(PROFILE + ITEMS, len(table))
        for k, kit in enumerate(kits):
            for slot in range(23):
                self.u16(PROFILE + KIT0 + k * KIT_SIZE + SLOTS + 4 * slot,
                         index[kit[slot]] if slot in kit else 0xFFFF)
                self.u16(PROFILE + KIT0 + k * KIT_SIZE + SLOTS + 4 * slot + 2, 1)
            self.u32(PROFILE + KIT0 + k * KIT_SIZE + 0x6FC, 9000 + k)    # the kit's id
            self.u32(PROFILE + REVISIONS + 4 * k, 3 + k)
        # the inventory: {begin, end, allocator, storage end} after the profile
        for i, (item, dict_id) in enumerate(inventory):
            record = INVENTORY + i * ITEM_SIZE
            self.construct_item(record)
            self.u32(record, item)
            self.u16(record + 4, dict_id)
            self.u32(record + 8, 777000 + i)
            # one modification and one upgrade (the camouflage: its item id, dict id, value, slot 2)
            emu.mem_write(record + 0x20, struct.pack("<IB3x", 1230, 0))
            self.u32(record + 0x18, record + 0x28)
            emu.mem_write(record + 0x6C, struct.pack("<IIQB7x", 4400 + i, 190, 0, 2))
            self.u32(record + 0x64, record + 0x84)
        self.u32(PROFILE + 0x4634, INVENTORY)
        self.u32(PROFILE + 0x4638, INVENTORY + len(inventory) * ITEM_SIZE)
        self.u32(PROFILE + 0x463C, ALLOCATOR)
        self.u32(PROFILE + 0x4640, INVENTORY + len(inventory) * ITEM_SIZE)
        self.u32(GAME_ALLOCATOR, ALLOCATOR)
        self.u32(PROFILE - 0xC84, PLAYERS)
        self.u32(PLAYERS + 0x53A4, DICTIONARY)
        self.entries = {}
        for i, (dict_id, category) in enumerate((categories or {}).items()):
            self.entries[dict_id] = ENTRIES + i * 0x3A0
            self.u8(ENTRIES + i * 0x3A0 + 0x228, category)
        self.index = index

    def find(self, item, profile=PROFILE):
        regs = dict(SENTINELS)
        regs[UC_X86_REG_EAX], regs[UC_X86_REG_ECX], regs[UC_X86_REG_EDX] = profile, item, 0x55555555
        before = bytes(self.emu.mem_read(PROFILE, 0x4644)), bytes(self.emu.mem_read(INVENTORY, 0x2000))
        self.call(self.entry, regs)
        after = bytes(self.emu.mem_read(PROFILE, 0x4644)), bytes(self.emu.mem_read(INVENTORY, 0x2000))
        kept = all(self.emu.reg_read(reg) == value for reg, value in SENTINELS.items() if reg != UC_X86_REG_ECX) \
            and self.emu.reg_read(UC_X86_REG_ECX) == item \
            and self.emu.reg_read(UC_X86_REG_ESP) == STACK - 0x100 + 4
        edx = self.emu.reg_read(UC_X86_REG_EDX)
        return self.emu.reg_read(UC_X86_REG_EAX), edx - (1 << 32) if edx >> 31 else edx, kept, before == after

    def slot(self, profile, kit, slot):
        return self.u16(profile + KIT0 + kit * KIT_SIZE + SLOTS + 4 * slot)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    late_join = open(base.late_join_bin(), "rb").read()
    build, _ = base.build(original, "section", open(base.default_bin("section"), "rb").read(),
                          late_join=late_join)
    e = Emu(build)
    failures = 0

    def say(ok, text):
        nonlocal failures
        failures += not ok
        print(("ok    " if ok else "FAIL  ") + text)

    AK, SVD, PISTOL_ITEM, STASH, VEST_ITEM, BEHIND, LOOSE = 1501, 1502, 1503, 1504, 1505, 1506, 1507
    inventory = [(AK, 501), (SVD, 502), (PISTOL_ITEM, 503), (STASH, 504), (VEST_ITEM, 505), (BEHIND, 506),
                 (LOOSE, 507)]
    categories = {501: RIFLE, 502: RIFLE, 503: PISTOL, 504: RIFLE, 505: VEST, 506: RIFLE, 507: RIFLE}
    kits = [{WEAPON_1: AK, WEAPON_2: BEHIND, 2: VEST_ITEM}, {WEAPON_2: SVD}, {WEAPON_1: PISTOL_ITEM}]
    table = [AK, BEHIND, VEST_ITEM, SVD, PISTOL_ITEM, LOOSE]

    # a kit's held weapon: patch 32's own answer
    for name, item, kit in (("kit 1's weapon 1", AK, 0), ("kit 2's weapon 2 (no weapon 1)", SVD, 1),
                            ("kit 3's pistol", PISTOL_ITEM, 2)):
        e.setup(kits, table, inventory, chosen=1, categories=categories)
        eax, edx, kept, untouched = e.find(item)
        say(eax == PROFILE and edx == kit and kept and untouched and not e.calls,
            f"{name}: kit {edx}, the profile given back, nothing allocated, registers kept")

    # a weapon no kit holds: the chosen kit's weapon 1 in a scratch profile
    for name, item, appended in (("a weapon only in the stash", STASH, True),
                                 ("a weapon 2 behind a weapon 1", BEHIND, False),
                                 ("a weapon in the table and in no kit", LOOSE, False)):
        for chosen in (0, 2):
            e.setup(kits, table, inventory, chosen=chosen, categories=categories)
            eax, edx, kept, untouched = e.find(item)
            count = e.u8(SCRATCH + ITEMS)
            at = count - 1 if appended else e.index[item]
            record = SCRATCH + TABLE + at * ITEM_SIZE
            source = INVENTORY + [i for i, _ in inventory].index(item) * ITEM_SIZE
            copied = e.u32(record) == item and e.u16(record + 4) == e.u16(source + 4) and (
                not appended or (
                    e.u32(record + 8) == e.u32(source + 8)
                    and e.u32(record + 0x14) == record + 0x20 and e.u32(record + 0x18) == record + 0x28
                    and bytes(e.emu.mem_read(record + 0x20, 8)) == bytes(e.emu.mem_read(source + 0x20, 8))
                    and e.u32(record + 0x60) == record + 0x6C and e.u32(record + 0x64) == record + 0x84
                    and bytes(e.emu.mem_read(record + 0x6C, 0x18)) == bytes(e.emu.mem_read(source + 0x6C, 0x18))))
            others = all(e.slot(SCRATCH, k, s) == e.slot(PROFILE, k, s)
                         for k in range(3) for s in range(23) if (k, s) != (chosen, WEAPON_1))
            allocated = [c[0] for c in e.calls if c[0] in ("allocation_name", "allocate")]
            say(eax == SCRATCH and edx == chosen and kept and untouched
                and e.slot(SCRATCH, chosen, WEAPON_1) == at and count == len(table) + appended and copied
                and others and e.u32(SCRATCH + REVISIONS + 4 * chosen) == 0x80000001
                and e.u32(SCRATCH + REVISIONS + 4 * (1 - chosen % 2)) == e.u32(PROFILE + REVISIONS + 4 * (1 - chosen % 2))
                and allocated == ["allocation_name", "allocate"]
                and e.calls[-1][1:] == (ALLOCATOR, 0x4634, ALLOCATION_WHERE + 8),
                f"{name} (chosen kit {chosen}): the scratch, its kit's weapon 1 the record {at} "
                f"({'appended, a deep copy' if appended else 'the table' + chr(39) + 's own'}), the other slots "
                f"the account's, a new revision, allocated once, the account's profile and inventory untouched")
    # a second scratch: no new allocation, the next revision
    e.setup(kits, table, inventory, chosen=0, categories=categories)
    e.find(STASH)
    e.setup(kits, table, inventory, chosen=0, categories=categories, fresh=False)
    eax, edx, kept, untouched = e.find(STASH)
    say(eax == SCRATCH and edx == 0 and kept and untouched
        and not [c for c in e.calls if c[0] == "allocate"]
        and e.u32(SCRATCH + REVISIONS) == 0x80000002 and e.u8(SCRATCH + ITEMS) == len(table) + 1,
        "a second scratch: the same memory (no allocation), the next revision, made from the account's again")

    # no kit and the profile given back
    full = [AK, BEHIND, VEST_ITEM, SVD, PISTOL_ITEM] + [3000 + i for i in range(64)]
    cases = (
        ("armour in the stash", dict(item=VEST_ITEM), {}),
        ("an item the inventory lacks", dict(item=9999), {}),
        ("an item the dictionary does not know", dict(item=STASH), dict(categories={501: RIFLE})),
        ("a profile with no inventory behind it", dict(item=STASH, profile=OTHER), {}),
        ("no profile", dict(item=STASH, profile=0), {}),
        ("a full table", dict(item=STASH), dict(table=full)),
        ("a failed allocation", dict(item=STASH), dict(fail_allocation=True)),
    )
    for name, find_args, setup_args in cases:
        args = dict(kits=kits, table=table, inventory=inventory, categories=categories)
        args.update(setup_args)
        if name == "armour in the stash":
            args["kits"] = [{WEAPON_1: AK}, {WEAPON_2: SVD}, {WEAPON_1: PISTOL_ITEM}]
            args["table"] = [AK, SVD, PISTOL_ITEM]
        if name == "a profile with no inventory behind it":
            e.setup(**args)
            e.emu.mem_write(OTHER, bytes(e.emu.mem_read(PROFILE, 0x4640)))
            e.u32(OTHER + 0x463C, 0x12345678)
        else:
            e.setup(**args)
        profile = find_args.get("profile", PROFILE)
        eax, edx, kept, untouched = e.find(find_args["item"], profile)
        say(eax == profile and edx == -1 and kept and untouched,
            f"{name}: no kit, the profile given back, registers kept, nothing of the account's written")

    print(f"weapon stash: {'all checks passed' if not failures else f'{failures} FAILED'}")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
