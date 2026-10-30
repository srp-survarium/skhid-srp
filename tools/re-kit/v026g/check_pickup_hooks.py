#!/usr/bin/env python3
"""Run the installed pickup lifecycle/timing hooks with the real linked Rust.

Only native heap allocation/free and thread identity are fixture endpoints,
in addition to check_pickup_inputs.py's documented timeline/logging stubs.
Generated NASM entries, Rust registry, journal observers, and original input
functions execute. This is not weapon transfer or gameplay verification.
"""
from pathlib import Path
import struct
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "srp-026g/client-patches"))
import patch_026g as base
import hooks_026g as hooks
from check_pickup_inputs import Inputs
from check_artefacts_026g import STACK
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX,
                              UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_ESI,
                              UC_X86_REG_EDI, UC_X86_REG_EIP, UC_X86_REG_EDX,
                              UC_X86_REG_EFLAGS)


class HookedInputs(Inputs):
    def __init__(self, data, linked):
        super().__init__(data)
        g = self.g
        g.emu.mem_map(linked.va, (linked.size + 0xFFF) & ~0xFFF)
        g.emu.mem_write(linked.va, linked.image)
        # Rust's constructor can use more stack than the small input fixture.
        g.emu.mem_map(0x20F00000, 0x100000)
        self.heap = 0x24000000
        g.emu.mem_map(self.heap, 0x400000)
        self.allocations = {}
        self.freed = []
        self.destroyed_profiles = []
        self.fail_allocation_after = None
        self.log_attempts = 0
        self.allocator = g.block(4)
        g.word(0x1F1D9C4, self.allocator)
        g.word(0x870118, 0x2200F000)
        g.emu.mem_write(0x2200F000, b"\xB8\x01\0\0\0\xC3")
        # No disk writes: CreateFileA reports INVALID_HANDLE_VALUE. Count
        # attempts so only the expected capture-failure paths can log.
        g.word(0x870048, 0x2200F020)
        g.emu.mem_write(0x2200F020, b"\xB8\xFF\xFF\xFF\xFF\xC2\x1C\0")
        g.emu.hook_add(UC_HOOK_CODE, self.heap_call)

        sections = base.Headers(data).sections(data)
        def original(va, size):
            for _, rva, _, raw, raw_size, _ in sections:
                if base.IMAGE_BASE + rva <= va < base.IMAGE_BASE + rva + raw_size:
                    offset = raw + va - base.IMAGE_BASE - rva
                    return data[offset:offset + size]
            raise AssertionError(f"unmapped original site {va:#x}")

        self.sites = {}
        for hook in hooks.hooks(exe=original):
            if hook["patch"] != "dropped_weapons":
                continue
            va, size = hook["site"], len(hook["original"])
            assert bytes(g.emu.mem_read(va, size)) == hook["original"]
            target = linked.symbols[hook["entry"]]
            g.emu.mem_write(va, b"\xE9" + struct.pack("<i", target - va - 5) + b"\x90" * (size - 5))
            self.sites[hook["handler"]] = (va, size)
        assert len(self.sites) == 20

    def heap_call(self, emu, at, _size, _user):
        if at == 0x2200F020:
            self.log_attempts += 1
        if at == 0x26F3B0:
            # Observe the real destructor; do not replace its execution.
            self.destroyed_profiles.append(emu.reg_read(UC_X86_REG_ECX))
        if at not in (0xE4F90, 0x1539D0):
            return
        g = self.g
        esp = emu.reg_read(UC_X86_REG_ESP)
        if at == 0xE4F90:
            assert emu.reg_read(UC_X86_REG_ESI) == self.allocator
            size = emu.reg_read(UC_X86_REG_EAX)
            assert 0 < size < 0x100000
            if self.fail_allocation_after is not None:
                if self.fail_allocation_after == 0:
                    self.fail_allocation_after = None
                    emu.reg_write(UC_X86_REG_EAX, 0)
                    emu.reg_write(UC_X86_REG_EIP, g.read(esp))
                    emu.reg_write(UC_X86_REG_ESP, esp + 8)
                    return
                self.fail_allocation_after -= 1
            allocated = self.heap
            self.heap += (size + 15) & ~15
            assert self.heap < 0x24400000
            g.emu.mem_write(allocated, b"\xA5" * size)
            self.allocations[allocated] = size
            emu.reg_write(UC_X86_REG_EAX, allocated)
        else:
            assert emu.reg_read(UC_X86_REG_ECX) == self.allocator
            allocated = emu.reg_read(UC_X86_REG_EAX)
            pointer = g.read(esp + 4)
            assert g.read(pointer) == allocated
            size = self.allocations.pop(allocated)
            if size == 0x4634:
                assert allocated in self.destroyed_profiles
                # Original destructor resets both self-relative vectors.
                for index in range(69):
                    item = allocated + 0x1568 + index * 0xB4
                    assert g.read(item + 0x18) == item + 0x20
                    assert g.read(item + 0x64) == item + 0x6C
            self.freed.append(allocated)
            g.emu.mem_write(allocated, b"\xDD" * size)
            g.word(pointer, 0)
        emu.reg_write(UC_X86_REG_EIP, g.read(esp))
        # malloc consumes its description; free leaves its pointer argument
        # for the generated thunk to remove (re-kit ABI: ret4 versus ret0).
        emu.reg_write(UC_X86_REG_ESP, esp + (8 if at == 0xE4F90 else 4))

    def lifecycle(self, create, core):
        """Execute the actual installed entry and overwritten instructions.

        We supply the already-constructed core and the native epilogue/frame;
        the resource-heavy original constructor/destructor are not simulated.
        """
        g = self.g
        va, size = self.sites["on_constructed" if create else "on_destroying"]
        esp = STACK - 0x100
        for n, value in enumerate((0x11, 0x22, 0x33)):
            g.word(esp + n * 4, value)
        g.emu.reg_write(UC_X86_REG_ESP, esp)
        g.emu.reg_write(UC_X86_REG_EBX, core)
        g.emu.reg_write(UC_X86_REG_EBP, core)
        g.emu.emu_start(va, va + size, count=2_000_000)
        assert g.emu.reg_read(UC_X86_REG_EIP) == va + size
        if create:
            assert g.emu.reg_read(UC_X86_REG_EAX) == core
            assert g.emu.reg_read(UC_X86_REG_EDI) == 0x11
            assert g.emu.reg_read(UC_X86_REG_ESI) == 0x22
            assert g.emu.reg_read(UC_X86_REG_EBX) == 0x33
            assert g.emu.reg_read(UC_X86_REG_ESP) == esp + 12
        else:
            assert g.emu.reg_read(UC_X86_REG_EAX) == g.read(core + 0x18F58)
            assert g.emu.reg_read(UC_X86_REG_ESP) == esp

    def drawing(self):
        """Run generated frame/weapon entries, preserving native registers.

        Rust unit tests check publication/selection; this proves the linked
        entries actually read native buffers and return to the right spans.
        No graphics driver, camera input or gameplay transfer is simulated.
        """
        g = self.g
        world, player, weapon = g.block(0x4D50), g.block(0x340), g.block(0x1814)
        model, instance, original, pose = [g.block(size) for size in (0x10C, 0x62B8, 0x120, 64)]
        g.word(world + 0x4D4C, self.core - 0x1F8)
        g.word(player + 0x120, self.core)
        g.word(player + 0x128, weapon)
        g.word(player + 0x328, 700)
        g.emu.mem_write(player + 0x114, b'\x03')
        g.emu.mem_write(player + 0x33F, b'\x01')
        g.word(weapon, 0xA04174)
        g.word(weapon + 0x1810, model)
        g.word(model + 0x108, instance)
        g.word(instance + 0x62B4, original)
        g.emu.mem_write(original + 0x108, struct.pack('<6f', .2, -.1, .3, .8, .1, .9))
        g.emu.mem_write(pose, struct.pack('<16f', 1, 0, 0, 0, 0, 1, 0, 0,
                                         0, 0, 1, 0, 10, 20, 30, 1))
        def invoke(handler, ebx, byte=0):
            va, size = self.sites[handler]
            esp = STACK - 0x100
            g.emu.mem_write(esp + 15, bytes([byte]))
            registers = {UC_X86_REG_EAX: pose, UC_X86_REG_EBX: ebx,
                         UC_X86_REG_ECX: 0x1234, UC_X86_REG_EDX: 0x5678,
                         UC_X86_REG_ESI: 0x8765, UC_X86_REG_EDI: 0x4321,
                         UC_X86_REG_EBP: STACK - 0x80, UC_X86_REG_ESP: esp}
            for reg, value in registers.items():
                g.emu.reg_write(reg, value)
            g.emu.emu_start(va, va + size, count=2_000_000)
            assert g.emu.reg_read(UC_X86_REG_EIP) == va + size
            if handler == 'on_weapon_draw':
                registers[UC_X86_REG_ECX] = weapon  # overwritten native MOV
            for reg, expected in registers.items():
                assert g.emu.reg_read(reg) == expected, (handler, reg)
            if handler == 'on_draw_complete':
                assert bool(g.emu.reg_read(UC_X86_REG_EFLAGS) & 0x40) == (byte == 0)

        for invalid in [False, True]:
            invoke('on_draw_begin', world)
            if invalid:
                g.word(weapon, 0xA11094)  # smaller core-only weapon: do not read model
            invoke('on_weapon_draw', player)
            invoke('on_draw_complete', world, int(invalid))
        # No model access for a living player, no native core read in lobby.
        g.emu.mem_write(player + 0x33C, b'\x01')
        invoke('on_weapon_draw', player)
        g.word(world + 0x4D4C, 0)
        invoke('on_draw_begin', world)
        invoke('on_draw_complete', world)


def profiles(data, linked):
    """Real Rust bank + native constructor/body-copy/destructor at death.

    Stop after the overwritten prologue, before original death callbacks.
    This checks capture/lifetime, not those callbacks or weapon transfer.
    """
    i = HookedInputs(data, linked)
    g = i.g
    i.lifecycle(True, i.core)
    player = g.block(0x6BC)
    source = g.block(0x4634)
    g.call(0x2F73F0, ecx=source)
    g.word(player + 0x120, i.core)
    g.word(player + 0x328, 700)
    g.word(player + 0x6B8, source)
    g.emu.mem_write(player + 0x114, b'\x02')
    g.emu.mem_write(player + 0x33F, b'\x01')
    item = source + 0x1568
    g.word(item, 17)
    g.word(item + 0x20, 123)
    g.word(item + 0x24, 124)
    g.word(item + 0x18, item + 0x28)
    g.word(item + 0x6C, 456)
    g.word(item + 0x64, item + 0x84)
    g.emu.mem_write(source + 0x1564, b'\x01')
    g.emu.mem_write(source + 0x57, b'\x03')

    def invoke(handler):
        va, size = i.sites[handler]
        registers = {
            UC_X86_REG_EAX: 0x1111, UC_X86_REG_ECX: player,
            UC_X86_REG_EDX: 0x3333, UC_X86_REG_EBX: 0x4444,
            UC_X86_REG_EBP: 0x5555, UC_X86_REG_ESI: 0x6666,
            UC_X86_REG_EDI: 0x7777,
        }
        esp = STACK - 0x100
        for reg, value in registers.items():
            g.emu.reg_write(reg, value)
        g.emu.reg_write(UC_X86_REG_ESP, esp)
        g.emu.reg_write(UC_X86_REG_EFLAGS, 0x246)
        original = bytes(g.emu.mem_read(source, 0x4634))
        try:
            g.emu.emu_start(va, va + size, count=2_000_000)
        except Exception as error:
            state = {reg: hex(g.emu.reg_read(reg)) for reg in registers}
            raise AssertionError(f"death entry {handler}: eip="
                                 f"{g.emu.reg_read(UC_X86_REG_EIP):#x}, "
                                 f"registers={state}") from error
        assert g.emu.reg_read(UC_X86_REG_EIP) == va + size
        preserved = UC_X86_REG_EDI if handler == 'on_player_death' else UC_X86_REG_ESI
        assert g.emu.reg_read(UC_X86_REG_ESP) == esp - 8
        assert g.read(esp - 8) == 1
        assert g.read(esp - 4) == registers[preserved]
        for reg, value in registers.items():
            assert g.emu.reg_read(reg) == (player if reg == preserved else value)
        assert g.emu.reg_read(UC_X86_REG_EFLAGS) == 0x246
        assert bytes(g.emu.mem_read(source, 0x4634)) == original

    def snapshots():
        return {p for p, size in i.allocations.items() if size == 0x4634}

    # Uninserted and invalid-life observations do not create a template.
    g.emu.mem_write(player + 0x33F, b'\0')
    invoke('on_player_death')
    g.emu.mem_write(player + 0x33F, b'\x01')
    g.word(player + 0x328, 0xFFFFFFFF)
    invoke('on_base_player_death')
    assert len(i.allocations) == 1
    g.word(player + 0x328, 700)
    # Both allocation failures leave the bank empty and preserve the donor.
    # Failure of the entry allocation must destroy/free its copied profile.
    for allocations_before_failure in [0, 1]:
        freed_before = len(i.freed)
        i.fail_allocation_after = allocations_before_failure
        invoke('on_player_death')
        assert i.fail_allocation_after is None
        assert len(i.allocations) == 1 and not snapshots()
        assert len(i.freed) == freed_before + allocations_before_failure
    assert i.log_attempts == 2
    invoke('on_player_death')
    assert len(i.allocations) == 3
    first, = snapshots()
    g.word(item, 99)
    invoke('on_base_player_death')  # repeat/rollback of the same source life
    assert len(i.allocations) == 3 and g.read(first + 0x1568) == 17
    g.word(player + 0x328, 900)
    invoke('on_base_player_death')
    second, = snapshots() - {first}
    assert len(i.allocations) == 5 and g.read(second + 0x1568) == 99

    # The same place/spawn in another core is a different owner.
    other = g.block(0x19000)
    i.lifecycle(True, other)
    g.word(player + 0x120, other)
    g.word(item, 42)
    invoke('on_player_death')
    third, = snapshots() - {first, second}
    assert len(i.allocations) == 8 and g.read(third + 0x1568) == 42
    g.call(0x26F3B0, ecx=source)
    g.emu.mem_write(source, b'\xCC' * 0x4634)
    g.emu.mem_write(player, b'\xCC' * 0x6BC)
    for profile, equipment in [(first, 17), (second, 99), (third, 42)]:
        retained = profile + 0x1568
        assert g.read(retained) == equipment
        assert g.read(retained + 0x14) == retained + 0x20
        assert g.read(retained + 0x18) == retained + 0x28
        assert g.read(retained + 0x60) == retained + 0x6C
        assert g.read(retained + 0x64) == retained + 0x84
        assert g.read(retained + 0x20) == 123
        assert g.read(retained + 0x24) == 124
        assert g.read(retained + 0x6C) == 456
    i.lifecycle(False, i.core)
    assert snapshots() == {third} and len(i.allocations) == 3
    i.lifecycle(True, i.core)  # reused address starts without old templates
    assert snapshots() == {third} and len(i.allocations) == 4
    i.lifecycle(False, other)
    i.lifecycle(False, i.core)
    assert not i.allocations
    assert i.log_attempts == 2
    for profile in [first, second, third]:
        assert i.destroyed_profiles.count(profile) == 1
        assert i.freed.count(profile) == 1
    print("linked Rust death capture: both native entries, original profile copy, "
          "profile/entry allocation failure, duplicate life, donor mutation/destruction, independent cores, "
          "destructor-before-free and empty address reuse passed")


def run(data):
    linked = base.arena()
    profiles(data, linked)
    i = HookedInputs(data, linked)
    i.lifecycle(True, i.core)
    assert len(i.allocations) == 1
    first_allocation = next(iter(i.allocations))
    i.drawing()
    assert i.submit(901, place=0, network=False) == (0, 901)
    assert i.submit(912, place=0, network=False, actions=8) == (0, 912)
    assert i.inputs(0) == [(912, 0, 12)]

    event = i.g.block(0x1C0)
    i.g.word(event + 0x1BC, 912)
    i.g.word(i.core + 4, event)
    i.g.word(i.core + 0x18FBC, 50)
    i.g.call(0x2D4E40, (i.core, 1000))
    assert i.inputs(0) == [(950, 0, 12)]
    assert i.move(950, 1000) == 0
    assert i.inputs(0) == [(1000, 0, 12)]
    removed = i.g.read(i.nodes + 4)
    assert i.discard(1000) == 0
    assert i.inputs(0) == []
    assert i.discard_receipts == [(removed, i.core, 0)]

    # Native refusals must never reach the success-only journal hook.
    assert i.submit(800, place=0, network=False) == (0, 800)
    history = i.g.read(i.core + 0x2FC0)
    for time, status in [(800, 6), (800, 4), (799, 1), (900, 5)]:
        if status == 6:
            i.g.word(i.core + 0x18FD4, 800)
        elif status == 4:
            i.g.word(i.core + 0x2FC0, 0)
        assert i.discard(time) == status
        assert i.inputs(0) == [(800, 0, 4)]
        assert i.discard_receipts == [(removed, i.core, 0)]
        i.g.word(i.core + 0x18FD4, 0)
        i.g.word(i.core + 0x2FC0, history)

    second_core = i.g.block(0x19000)
    i.lifecycle(True, second_core)
    assert len(i.allocations) == 2
    i.lifecycle(False, i.core)  # remove a non-head owner
    assert i.freed == [first_allocation] and len(i.allocations) == 1
    i.lifecycle(True, i.core)  # same native address, new empty journal
    assert len(i.allocations) == 2
    i.lifecycle(False, second_core)
    i.lifecycle(False, i.core)
    assert not i.allocations and len(i.freed) == 3
    print("linked Rust pickup hooks: native entries, independent world allocation, "
          "frame publication and weapon bounds, local merge, tick snap, move/discard and refusal, "
          "teardown and core-address reuse passed")


if __name__ == "__main__":
    run(Path(sys.argv[1]).read_bytes())
