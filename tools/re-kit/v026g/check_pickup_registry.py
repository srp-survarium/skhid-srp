#!/usr/bin/env python3
"""Prove weapon IDs and the native player's held-item restore ordering.

Runs the original weapon binder and both ID-decoding slices. Registry swaps
below are fixture operations, not a claim that native transfer is installed.
The activation, animation and inventory readers remain separate integration
requirements.
"""
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "srp-026g/client-patches"))
from check_artefacts_026g import Game, STACK, STOP
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import (
    UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_ESP, UC_X86_REG_EIP,
    UC_X86_REG_ECX,
)


def run(data):
    g = Game(data)
    core = g.block(0x19000)
    registry = g.block(16)
    g.word(core + 0x18B4C, registry)
    g.word(core + 0x18B50, registry)
    g.word(core + 0x18B54, registry + 16)
    original, other, replacement = [g.block(0x700) for _ in range(3)]
    for index, weapon in enumerate([original, other]):
        # ECX is the inventory_item subobject, not the active-object base.
        g.call(0x2E30F0, (core,), ecx=weapon + 0x10)
        assert g.emu.reg_read(UC_X86_REG_EIP) == STOP
        assert g.read(registry + index * 4) == weapon
        assert g.read(core + 0x18B50) == registry + (index + 1) * 4
        assert g.read(weapon + 4) == core
        assert g.read(weapon + 0x258) == core
        assert bytes(g.emu.mem_read(weapon + 0xC, 1)) == bytes([index])

    player, reader, wire = g.block(0x1000), g.block(16), g.block(4)
    g.word(player + 0x120, core)

    def restore(byte, start, stop, field):
        g.emu.mem_write(wire, bytes([byte]))
        g.word(reader + 4, wire)
        g.emu.reg_write(UC_X86_REG_EBX, player)
        g.emu.reg_write(UC_X86_REG_ESI, reader)
        g.emu.reg_write(UC_X86_REG_ESP, STACK - 0x100)
        g.emu.emu_start(start, stop, count=1000)
        assert g.emu.reg_read(UC_X86_REG_EIP) == stop
        assert g.read(reader + 4) == wire + 1
        return g.read(player + field)

    def held(byte):
        return restore(byte, 0x2DB626, 0x2DB65A, 0x128)

    def pending(byte):
        return restore(byte, 0x2DB796, 0x2DB7C0, 0x12C)

    for lookup in [held, pending]:
        assert lookup(0) == original
        assert lookup(1) == other
        assert lookup(255) == 0

    # A snapshot's byte ID alone cannot say which weapon occupies this slot.
    # Restore the ownership mapping before held/pending ID decoding, which
    # itself precedes inventory::deserialize at 2DB7D8.
    for mapped in [replacement, original, replacement, original]:
        g.word(registry, mapped)
        for lookup in [held, pending]:
            assert lookup(0) == mapped
            assert lookup(1) == other
        assert g.read(core + 0x18B50) == registry + 8

    # Negative control: changing the registry only after the lookup leaves
    # the already-decoded held/pending pointers on the old object.
    assert held(0) == original and pending(0) == original
    g.word(registry, replacement)
    assert g.read(player + 0x128) == original
    assert g.read(player + 0x12C) == original
    assert held(0) == replacement and pending(0) == replacement

    # Run the actual held-ID transition, with recording implementations of
    # its two virtual callbacks. The new mapping must leave held unchanged:
    # native decoding deactivates the old item, then binds the new user's
    # pointer. The callback bodies here are fixtures, not full animations.
    vtable, set_user, deactivate = g.block(0x40), g.block(16), g.block(16)
    g.emu.mem_write(set_user, bytes.fromhex("8b442404894108c20400"))
    g.emu.mem_write(deactivate, bytes.fromhex("c7410800000000c20400"))
    g.word(vtable + 0x10, set_user)
    g.word(vtable + 0x1C, deactivate)
    g.word(original, vtable)
    g.word(replacement, vtable)
    events = []

    def callback(emu, at, size, user):
        if at in (set_user, deactivate):
            events.append((at, emu.reg_read(UC_X86_REG_ECX),
                           g.read(emu.reg_read(UC_X86_REG_ESP) + 4)))

    g.emu.hook_add(UC_HOOK_CODE, callback, begin=set_user, end=deactivate + 15)
    g.word(original + 8, player)
    g.word(replacement + 8, 0)
    g.word(player + 0x128, original)
    assert restore(0, 0x2DB626, 0x2DB6AD, 0x128) == replacement
    assert events == [(deactivate, original, 1), (set_user, replacement, player)]
    assert g.read(original + 8) == 0 and g.read(replacement + 8) == player

    # Negative: prewriting held as a live pickup does suppresses both calls.
    # Stop at the equal-pointer branch before unrelated animation cleanup.
    events.clear()
    g.word(replacement + 8, 0)
    assert restore(0, 0x2DB626, 0x2DB67F, 0x128) == replacement
    assert not events and g.read(replacement + 8) == 0
    print("native pickup registry: item-base binding, byte IDs, held/pending "
          "resolution, native user transition, repeated mapping restore and ordering negative controls passed")


if __name__ == "__main__":
    run(Path(sys.argv[1]).read_bytes())
