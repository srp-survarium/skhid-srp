#!/usr/bin/env python3
"""Execute the native stowed weapon codec needed for displaced originals.

The held/user/controller branch is deliberately excluded: an installed pickup
has already deactivated its retained original. Inventory holders are fixtures;
the item/weapon codecs, buffer writes and ammo reference assignment are real.
"""
from pathlib import Path
import struct
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "srp-026g/client-patches"))
from check_artefacts_026g import Game, STOP
from unicorn.x86_const import UC_X86_REG_EIP


def run(data):
    for chamber in [False, True]:
        g = Game(data)
        g.emu.mem_write(0x1F1D32C, b"\0\0\0")  # no debug annotation objects
        core, roster = g.block(0x19000), g.block(96)
        inv, player = g.block(0x190), g.block(0x120)
        holder, vtable, getter = g.block(16), g.block(16), g.block(16)
        g.word(core + 0x18458, roster)
        g.word(roster + 48 + 2 * 4, player)
        g.word(player + 0x10C, inv)
        g.emu.mem_write(player + 0x114, bytes([1, 2]))
        g.word(inv + 0x174, holder)
        g.word(holder, vtable)
        g.word(vtable + 12, getter)
        g.emu.mem_write(getter, b"\xB8" + struct.pack("<I", player) + b"\xC3")
        entry, slots = g.block(0x230), g.block(8)
        ammo = [g.block(0x280), g.block(0x280)]
        g.word(slots, 8)
        g.word(slots + 4, 9)
        for index, stack in enumerate(ammo):
            g.word(inv + 0x10C + (8 + index) * 4, stack)
            g.word(stack + 0xD0, 10)
        original, restored = [g.block(0x1900) for _ in range(2)]
        for gun in [original, restored]:
            g.word(gun + 0x258, core)
            g.word(gun + 0x260, inv)
            g.word(gun + 0x264, 7)
            g.word(gun + 0x26C, entry)
            g.word(gun + 0x5AC, slots)
            g.emu.mem_write(gun + 0x5B0, b"\x02\x01")
            g.emu.mem_write(gun + 0x5F0, bytes([chamber, chamber]))
        g.word(original + 0x270, 0x3F400000)  # durability 0.75
        g.word(original + 0x5BC, 0x3F200000)
        g.word(original + 0x5A4, 123456)
        g.emu.mem_write(original + 0x5E5, b"\x07\x02")
        g.emu.mem_write(original + 0x5F4, b"\x01")
        g.emu.mem_write(original + 0x181E, b"\x01")
        g.word(restored + 0x538, ammo[0])

        def writer():
            context, buffer, payload = g.block(24), g.block(16), g.block(128)
            g.word(context + 20, buffer)
            g.word(buffer + 4, payload)
            g.word(buffer + 8, 128)
            return context, buffer, payload

        main, aux = writer(), writer()
        g.call(0x244A00, (main[0], aux[0]), ecx=original)
        assert g.emu.reg_read(UC_X86_REG_EIP) == STOP
        encoded = bytes(g.emu.mem_read(main[2], g.read(main[1] + 12)))
        expected = struct.pack("<IBBBIIBBBB", 0x3F400000, 1, 2, 7,
                               0x3F200000, 123456, 255, 7, 2, 1)
        if chamber:
            expected += b"\xFF"
        assert encoded == expected, (encoded.hex(), expected.hex())
        assert g.read(aux[1] + 12) == 1
        assert bytes(g.emu.mem_read(aux[2], 1)) == b"\xFF"

        def reader(payload, size):
            at = g.block(12)
            g.word(at, payload)
            g.word(at + 4, payload)
            g.word(at + 8, size)
            return at

        main_read, aux_read = reader(main[2], len(encoded)), reader(aux[2], 1)
        g.call(0x244A40, (main_read, aux_read, 0), ecx=restored)
        assert g.emu.reg_read(UC_X86_REG_EIP) == STOP
        assert g.read(main_read + 4) == main[2] + len(encoded)
        assert g.read(aux_read + 4) == aux[2] + 1
        assert g.read(restored + 0x538) == ammo[1]
        assert [g.read(stack + 0xD0) for stack in ammo] == [9, 11]
        assert g.read(restored + 0x260) == inv
        assert g.read(restored + 0x264) == 7
        assert bytes(g.emu.mem_read(restored + 0x181E, 1)) == b"\x01"
        # The Rust owner uses the core codec with no presentation reader.
        # Reapplying it must preserve the selected stack's reference count
        # and never consume the independent client byte.
        g.word(main_read + 4, main[2])
        g.call(0x2E3490, (main_read, 0, 0), ecx=restored)
        assert g.emu.reg_read(UC_X86_REG_EIP) == STOP
        assert g.read(main_read + 4) == main[2] + len(encoded)
        assert [g.read(stack + 0xD0) for stack in ammo] == [9, 11]
        assert bytes(g.emu.mem_read(restored + 0x181E, 1)) == b"\x01"
        g.word(main[1] + 12, 0)
        g.word(aux[1] + 12, 0)
        g.call(0x244A00, (main[0], aux[0]), ecx=restored)
        assert bytes(g.emu.mem_read(main[2], g.read(main[1] + 12))) == encoded
        assert g.read(aux[1] + 12) == 1
    print("native displaced weapon: stowed client/core codecs, chamber variants, owner/slot and repeated ammo refs round-trip")


if __name__ == "__main__":
    run(Path(sys.argv[1]).read_bytes())
