#!/usr/bin/env python3
"""Original portable-core ownership and snapshot ABI, without launching a game.

The constructor, resource binding, selector transitions, field writers, reader,
node destruction and transition destruction are original code. Heap allocation/free, RTTI name
lookup, physics-interface lookup and skeleton attachment are fixtures. This
proves the independent-core lifecycle, resource references and idle snapshot,
not real clip playback, hand rendering or the final artefact-carry adapter.
"""
from pathlib import Path
import struct
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "srp-026g/client-patches"))
from check_artefacts_026g import Game
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_ESP, UC_X86_REG_EIP


class PortableGame(Game):
    def __init__(self, data):
        super().__init__(data)
        self.allocated, self.freed = set(), set()
        allocator = self.block(0x40)
        self.word(0x1F1D9C4, allocator)
        self.word(0x1F1DA44, allocator)
        for address in (0xE4F90, 0x1015E0, 0x420789, 0x42A390, 0x38DDB0):
            self.emu.hook_add(UC_HOOK_CODE, self.fixture, begin=address, end=address)

    def fixture(self, emu, address, _size, _data):
        esp = emu.reg_read(UC_X86_REG_ESP)
        cleanup = 0
        if address == 0xE4F90:
            pointer = self.block(emu.reg_read(UC_X86_REG_EAX))
            self.allocated.add(pointer)
            emu.reg_write(UC_X86_REG_EAX, pointer)
            cleanup = 4
        elif address == 0x1015E0:
            pointer = emu.reg_read(UC_X86_REG_EAX)
            assert pointer in self.allocated and pointer not in self.freed
            self.freed.add(pointer)
        elif address == 0x420789:  # type_info name used only as allocation label
            emu.reg_write(UC_X86_REG_EAX, emu.reg_read(UC_X86_REG_ECX) + 8)
        elif address == 0x42A390:  # most-derived address; these nodes have one base
            emu.reg_write(UC_X86_REG_EAX, self.read(esp + 4))
        elif address == 0x38DDB0:  # skeleton attachment: no render assets in fixture
            pass
        else:
            raise AssertionError(hex(address))
        emu.reg_write(UC_X86_REG_EIP, self.read(esp))
        emu.reg_write(UC_X86_REG_ESP, esp + 4 + cleanup)

    def core(self, seed):
        result = self.block(0x200)
        self.call(0x2FEBC0, (result,), eax=seed)
        assert self.read(result + 8) == seed
        assert self.read(result + 0x1F8) == 0
        return result

    def destroy_unbound(self, core):
        self.call(0x231BC0, esi=core + 0x60)
        self.call(0x2FB8A0, (core + 8,))

    def animation_resource(self):
        """Loaded-resource fixture, including the eight required hit clips.

        It is not a rendered clip: each managed animation supplies the
        duration curve read by the original binder. Native retain/release,
        pin/unpin and all eight duration calculations still run unchanged.
        """
        resource = self.block(0xA40)
        self.word(resource + 0xD0, 1)
        clips = []
        for index in range(8):
            clip, allocation = self.block(0x100), self.block(0x100)
            self.word(clip + 0xDC, 1)
            self.word(clip + 0xD4, allocation)
            data = allocation + 0x34
            self.word(data + 0x14, 0x20)
            # Two polynomial entries, followed by their two times.
            self.word(data + 0x20, 2)
            self.word(data + 0x24, 8)
            self.emu.mem_write(data + 0x28, struct.pack('<10f', *([0] * 9), 30 + index))
            self.word(resource + 0x570 + index * 4, clip)
            clips.append((clip, allocation))
        return resource, clips

    def bind_resource(self, core, resource):
        reference = self.block(4)
        self.word(reference, resource)
        self.call(0x2DF050, eax=core, edi=reference)
        assert self.read(core + 0x48) == resource
        assert self.read(core + 0x120) == resource

    def bind_idle(self, core, seed, lobby_pose=False):
        player = self.block(0xF000)
        parameters, parameters_ref = self.block(0x300), self.block(4)
        active, active_vtable, zero = self.block(0x20), self.block(0x80), self.block(0x10)
        self.emu.mem_write(zero, b"\x31\xc0\xc3")
        self.word(player + 0x494, parameters_ref)
        self.word(parameters_ref, parameters)
        self.emu.mem_write(parameters + 0x202, bytes([lobby_pose]))
        self.word(player + 0x128, active)
        self.word(active, active_vtable)
        for slot in range(0, 0x80, 4):
            self.word(active_vtable + slot, zero)
        physics, physics_ref = self.block(0x400), self.block(4)
        interface, getter = self.block(0x10), self.block(0x10)
        self.word(physics_ref, physics)
        self.emu.mem_write(getter, b"\xb8" + struct.pack("<I", physics_ref) + b"\xc3")
        self.word(interface + 8, getter)
        self.word(player + 0x108, interface)
        self.emu.mem_write(player + 0x33C, b"\x01")
        self.call(0x2FEE50, (player,), ecx=core)
        self.call(0x2FEEC0, (seed,), ecx=core)
        current = self.read(core + 0x20)
        assert current != 0
        self.call(0x2FB980, edi=core + 8)
        assert self.read(core + 0x20) == current
        return player

    def snapshot(self, core):
        writer, buffer, data = self.block(0x20), self.block(0x10), self.block(0x400)
        self.word(writer + 0x14, buffer)
        self.word(buffer + 4, data)
        self.word(buffer + 8, 0x400)
        self.call(0x2FF930, (writer, 20000), ecx=core)
        return bytes(self.emu.mem_read(data, self.read(buffer + 0xC)))

    def restore(self, core, blob):
        reader, data = self.block(0x10), self.block(len(blob))
        self.emu.mem_write(data, blob)
        self.word(reader, data)
        self.word(reader + 4, data)
        self.word(reader + 8, data + len(blob))
        self.call(0x2FF970, (reader, 20000, 0), ecx=core)
        assert self.read(reader + 4) == data + len(blob)


def run(data):
    game = PortableGame(data)
    first, second = game.core(12345), game.core(54321)
    assert len(game.allocated) == 90
    assert game.read(first + 0x18) != game.read(second + 0x18)
    game.destroy_unbound(first)
    assert len(game.freed) == 45
    assert game.read(second + 8) == 54321
    game.destroy_unbound(second)
    assert game.freed == game.allocated

    # Real resource binder and destructors: shared immutable clips, separate
    # mutable selectors, and balanced references in either teardown order.
    for order in [(0, 1), (1, 0)]:
        game = PortableGame(data)
        resource, clips = game.animation_resource()
        cores = [game.core(12345), game.core(54321)]
        for core in cores:
            game.bind_resource(core, resource)
            durations = [game.read(core + 0x74 + index * 24) for index in range(8)]
            assert durations == [1000, 1033, 1066, 1100, 1133, 1166, 1200, 1233], durations
        assert game.read(resource + 0xD0) == 5
        for remaining, index in zip([3, 1], order):
            game.destroy_unbound(cores[index])
            assert game.read(resource + 0xD0) == remaining
            for clip, allocation in clips:
                assert game.read(clip + 0xDC) == 1
                assert game.read(allocation + 0x2C) == 0
        assert game.freed == game.allocated

    for lobby_pose, expected_index in [(False, 4), (True, 0)]:
        game = PortableGame(data)
        first, second = game.core(12345), game.core(54321)
        game.bind_idle(first, 12345, lobby_pose)
        game.bind_idle(second, 54321, lobby_pose)
        # Nonzero hit-animation values prove all eight triples round-trip.
        for i in range(8):
            for j in range(3):
                game.word(first + 0x60 + 8 + i * 24 + j * 4, 100 * i + j)
        snapshot = game.snapshot(first)
        assert len(snapshot) == 102
        assert snapshot[:6] == struct.pack("<IBB", 12345, 255, expected_index)
        before = game.snapshot(second)
        assert before != snapshot
        game.restore(second, snapshot)
        assert game.snapshot(second) == snapshot
        assert game.read(first + 0x20) != game.read(second + 0x20)
        # Both selectors retain distinct node graphs after reconstruction.
        game.call(0x2FB980, edi=second + 8)
        assert game.snapshot(second) == snapshot
    print("original portable core: independent 45-node ownership, resource binding/reference balance, complete unbound teardown, "
          "bound idle selection and native 102-byte snapshot roundtrip passed")


if __name__ == "__main__":
    run(Path(sys.argv[1]).read_bytes())
