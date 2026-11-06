#!/usr/bin/env python3
"""Native ammo contracts needed by dropped-weapon pickup.

Run with the original 0.26g executable. This executes the real magazine,
death, resupply eligibility and resupply routines on synthetic inventories.
It does not claim that pickup, rollback or rendering is implemented.
"""
from pathlib import Path
import struct
import sys
from unicorn import UC_HOOK_MEM_READ, UC_HOOK_MEM_WRITE, UC_MEM_WRITE

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "srp-026g/client-patches"))
from check_artefacts_026g import Game, STACK, config_bytes
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_EBP,
                               UC_X86_REG_EDI, UC_X86_REG_ESP, UC_X86_REG_ECX,
                               UC_X86_REG_EIP)


class AmmoGame(Game):
    def byte(self, at, value):
        self.emu.mem_write(at, bytes([value]))

    def short(self, at, value):
        self.emu.mem_write(at, struct.pack("<H", value))

    def u8(self, at):
        return self.emu.mem_read(at, 1)[0]

    def u16(self, at):
        return struct.unpack("<H", self.emu.mem_read(at, 2))[0]

    def setup(self, magazine, reserve, chamber=False, capacity=30, target=150, storage=500):
        self.inv = self.block(0x190)
        self.profile = self.block(0x5000)
        self.gun = self.block(0x700)
        self.ammo = self.block(0x280)
        self.dictionary = self.block(0x240)
        self.ammo_slots = self.block(4)
        # Inventory holds the inventory_item subobject, 10h after active_object.
        self.word(self.inv + 0x10C + 7 * 4, self.gun + 0x10)
        self.word(self.inv + 0x10C + 8 * 4, self.ammo)
        self.word(self.gun + 0xE0, 2)
        self.word(self.gun + 0x260, self.inv)
        self.word(self.gun + 0x538, self.ammo)
        self.word(self.gun + 0x5AC, self.ammo_slots)
        self.word(self.ammo_slots, 8)
        self.byte(self.gun + 0x5B0, 1)
        self.byte(self.gun + 0x5E4, capacity)
        self.byte(self.gun + 0x5E5, magazine)
        self.byte(self.gun + 0x5F0, int(chamber))
        self.byte(self.gun + 0x5F1, int(chamber))
        self.word(self.ammo + 0xD0, 2)
        self.word(self.ammo + 0x250, self.inv)
        self.word(self.ammo + 0x25C, self.dictionary)
        self.byte(self.dictionary + 0x22A, 1)
        self.short(self.ammo + 0x260, reserve)
        # Empty profile slots, except ammo slot 8 -> profile item 0.
        for slot in range(23):
            self.short(self.profile + 0x98 + slot * 4, 0xFFFF)
        self.short(self.profile + 0x98 + 8 * 4, 0)
        self.short(self.profile + 0x9A + 8 * 4, target)
        self.word(self.profile + 0x1568 + 8, storage)

    def state(self):
        return (self.u8(self.gun + 0x5E5), self.u8(self.gun + 0x5F1),
                self.u16(self.ammo + 0x260), self.read(self.profile + 0x1570))

    def availability(self):
        self.call(0x2FDF50, (self.inv, self.profile), eax=0)
        return self.emu.reg_read(UC_X86_REG_EAX)

    def restock(self):
        self.call(0x2FDC10, (self.inv, self.profile, 0))


def run(data):
    # Execute the config loader's whole double_handed branch, including the
    # original config lookup functions. Prove the native reader's base/offset
    # instead of merely writing what the Rust fixture expects to read.
    for values, expected in [({}, 0), ({"double_handed": 0}, 0),
                             ({"double_handed": 1}, 1)]:
        g = AmmoGame(data)
        config, gun = g.block(0x400), g.block(0x700)
        g.emu.mem_write(config, config_bytes(values, config))
        g.byte(gun + 0x5E9, 0xCC)
        g.emu.reg_write(UC_X86_REG_EBP, gun)
        g.emu.reg_write(UC_X86_REG_EBX, config)
        g.emu.reg_write(UC_X86_REG_ESP, STACK - 0x100)
        g.emu.emu_start(0x30AB8D, 0x30ABC6, count=100_000)
        assert g.u8(gun + 0x5E9) == expected

    for loaded, reserve, expected in [(17, 30, (30, 17)), (0, 30, (30, 0)),
                                      (29, 1, (30, 0)), (0, 7, (7, 0))]:
        g = AmmoGame(data)
        g.setup(loaded, reserve)
        g.call(0x2E17D0, esi=g.gun)
        assert (g.state()[0], g.state()[2]) == expected
        assert g.u8(g.inv + 0x184) == 1
        assert g.read(g.ammo + 0xD0) == 2

    # The native initialize binding accepts a separately owned ammo cell,
    # using the same relative-slot encoding as external artefact containers.
    # This avoids stealing/retyping the recipient's existing ammunition.
    # Run only the binding/autoload section, not animation/player setup.
    for before in [False, True]:
        for loaded in [0, 17, 30]:
            g = AmmoGame(data)
            cell = g.block(4) if before else None
            g.setup(loaded, 30, chamber=True)
            cell = cell or g.block(4)
            g.word(cell, g.ammo)
            relative_slot = ((cell - g.inv - 0x10C) // 4) & 0xFFFFFFFF
            g.word(g.ammo_slots, relative_slot)
            g.byte(g.gun + 0x5F4, 0)  # pickup must NOT auto-load an empty magazine
            g.emu.reg_write(UC_X86_REG_EBP, 0)
            g.emu.reg_write(UC_X86_REG_EDI, g.gun)
            g.emu.reg_write(UC_X86_REG_ESP, STACK - 0x100)
            g.emu.emu_start(0x2E2A99, 0x2E2B88, count=100_000)
            assert g.read(g.gun + 0x538) == g.ammo
            assert g.state()[:3] == (loaded, 1, 30)
            assert g.read(g.ammo + 0xD0) == 2

    # Snapshot decoding has its own ammo binding path; it does not initialize
    # or grant a spare magazine. Execute that native slice and its real
    # intrusive-pointer assignment, including replacing a prior retained stack.
    for before in (False, True):
        for bound in (False, True):
            g = AmmoGame(data)
            cell = g.block(4) if before else None
            g.setup(17, 42, chamber=True)
            cell = cell or g.block(4)
            old = g.block(0x280)
            g.word(old + 0xD0, 2)
            g.word(g.ammo + 0xD0, 1)
            g.word(g.gun + 0x538, old if bound else 0)
            g.word(cell, g.ammo)
            g.word(g.ammo_slots, ((cell - g.inv - 0x10C) // 4) & 0xFFFFFFFF)
            original = g.state()
            for _ in range(2):
                g.emu.reg_write(UC_X86_REG_EDI, g.gun)
                g.emu.reg_write(UC_X86_REG_ECX, 0)  # selected ordinal already decoded into CL
                g.emu.reg_write(UC_X86_REG_ESP, STACK - 0x100)
                g.emu.emu_start(0x2E3513, 0x2E3594, count=100_000)
                assert g.emu.reg_read(UC_X86_REG_EIP) == 0x2E3594
                assert g.read(g.gun + 0x538) == g.ammo
                assert g.read(g.ammo + 0xD0) == 2
                assert g.read(old + 0xD0) == (1 if bound else 2)
                assert g.state() == original

    # Native unloading preserves the chambered round as well as the magazine.
    g = AmmoGame(data)
    g.setup(17, 30, chamber=True)
    g.call(0x2E18E0, eax=g.gun)
    assert g.state()[:3] == (0, 0, 48)

    # The native crate leaves the magazine/chamber alone and fills the reserve
    # to the kit's TOTAL allowance minus those loaded rounds, consuming storage.
    for chamber in [False, True]:
        g = AmmoGame(data)
        g.setup(17, 30, chamber=chamber)
        assert g.availability() == 0
        g.restock()
        added = 150 - 17 - int(chamber) - 30
        assert g.state() == (17, int(chamber), 30 + added, 500 - added)
        assert g.availability() == 2
        g.restock()
        assert g.state() == (17, int(chamber), 30 + added, 500 - added)
        assert g.read(g.gun + 0xE0) == 2

    g = AmmoGame(data)
    g.setup(17, 30, storage=10)
    g.restock()
    assert g.state() == (17, 0, 40, 0)
    assert g.availability() == 1

    # Borrow just the slot table for native profile accounting. The original
    # weapon is retained by InstalledPickup; the actual inventory still holds
    # the borrowed weapon throughout both native calls. No fake inventory is
    # installed in a player or assigned as an item's owner. Check every access
    # to the view: these routines only read its slots and write weight_dirty.
    for weapon_slot in (7, 10):
        for chamber in (False, True):
            for storage in (0, 10, 500):
                g = AmmoGame(data)
                g.setup(17, 30, chamber=chamber, storage=storage)
                original = g.gun
                if weapon_slot == 10:
                    g.word(g.inv + 0x10C + 7 * 4, 0)
                    g.word(g.inv + 0x10C + 10 * 4, original + 0x10)
                # A borrowed weapon with a different calibre/private reserve.
                borrowed, private_slots, private_ammo = g.block(0x700), g.block(4), g.block(0x280)
                g.word(borrowed + 0xE0, 2)
                g.word(borrowed + 0x5AC, private_slots)
                g.word(private_slots, 0x1000)
                g.byte(borrowed + 0x5B0, 1)
                g.byte(borrowed + 0x5E5, 7)
                g.word(borrowed + 0x538, private_ammo)
                g.short(private_ammo + 0x260, 30)
                g.word(g.inv + 0x10C + weapon_slot * 4, borrowed + 0x10)
                live_slots = bytes(g.emu.mem_read(g.inv + 0x10C, 23 * 4))
                view = g.block(0x188)
                g.emu.mem_write(view + 0x10C, live_slots)
                g.word(view + 0x10C + weapon_slot * 4, original + 0x10)
                accesses = []

                def view_access(emu, access, at, size, value, user):
                    assert (access != UC_MEM_WRITE and view + 0x10C <= at
                            and at + size <= view + 0x168) or (
                                access == UC_MEM_WRITE and at == view + 0x184 and size == 1)
                    accesses.append(access)

                g.emu.hook_add(UC_HOOK_MEM_READ | UC_HOOK_MEM_WRITE, view_access,
                               begin=view, end=view + 0x187)
                g.call(0x2FDF50, (view, g.profile), eax=0)
                assert g.emu.reg_read(UC_X86_REG_EAX) == (0 if storage else 1)
                g.call(0x2FDC10, (view, g.profile, 0))
                added = min(storage, 150 - 17 - int(chamber) - 30)
                assert g.state() == (17, int(chamber), 30 + added, storage - added)
                assert bytes(g.emu.mem_read(g.inv + 0x10C, 23 * 4)) == live_slots
                assert g.read(original + 0xE0) == g.read(borrowed + 0xE0) == 2
                assert g.u8(borrowed + 0x5E5) == 7 and g.u16(private_ammo + 0x260) == 30
                assert g.read(g.ammo + 0x250) == g.inv
                assert UC_MEM_WRITE in accesses
                g.call(0x2FDF50, (view, g.profile), eax=0)
                assert g.emu.reg_read(UC_X86_REG_EAX) == (2 if storage == 500 else 1)

    # A replacement ammo object in the same slot is NOT sufficient: the native
    # crate still charges whichever profile entry slot 8 names. There is no
    # comparison between the stack's dictionary and the profile item here.
    g = AmmoGame(data)
    g.setup(17, 30)
    g.short(g.profile + 0x98 + 8 * 4, 1)
    g.word(g.profile + 0x1568 + 0xB4 + 8, 20)
    g.restock()
    assert g.state() == (17, 0, 50, 500)
    assert g.read(g.profile + 0x1568 + 0xB4 + 8) == 0

    # The death callback receives inventory_item*, not active_object*. Its
    # +5E5 store clears active_object+5F5 (auto reload), NOT loaded ammunition.
    g = AmmoGame(data)
    g.setup(17, 30, chamber=True)
    gun_dict = g.block(0x240)
    g.word(g.gun + 0x26C, gun_dict)
    g.word(g.gun + 0x270, struct.unpack("<I", struct.pack("<f", 0.9))[0])
    g.word(g.gun + 0x244, struct.unpack("<I", struct.pack("<f", 0.1))[0])
    g.call(0x2E3390, ecx=g.gun + 0x10)
    assert g.state()[:3] == (17, 1, 30)
    print("native weapon ammo: handedness config, reload, chamber, reserve, external ammo binding, crate allowance/storage, "
          "snapshot ammo rebind, original-loadout crate view, profile-slot mismatch and death callback contracts passed")


if __name__ == "__main__":
    run(Path(sys.argv[1]).read_bytes())
