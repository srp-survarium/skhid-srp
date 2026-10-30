#!/usr/bin/env python3
"""Trace the original map cook's physics pointer into its match owner.

Execute the native bound-callback builder/invoker, constructor field writes
and destruction ordering. Resource construction itself is outside this fixture.
"""
from pathlib import Path
import struct
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "srp-026g/client-patches"))
from check_artefacts_026g import Game, STACK, STOP
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX,
                              UC_X86_REG_EDX, UC_X86_REG_EBP, UC_X86_REG_ESP,
                              UC_X86_REG_EIP)


def check(data):
    g = Game(data)
    frame = STACK - 0x800
    physics, cook, config = (g.block(n) for n in [0x60, 0x30, 24])
    # Distinct map parameters prevent confusing the physics pointer with the
    # render/scene pointer carried by the same callback.
    parameters = [0x1100 + n * 0x100 for n in range(8)]
    g.emu.mem_write(frame - 0x48, struct.pack('<8I', *parameters))
    g.word(frame - 0xC, config)
    g.word(frame - 0x18, physics)
    g.word(frame - 0x60, cook)
    g.emu.reg_write(UC_X86_REG_EBP, frame)
    g.emu.reg_write(UC_X86_REG_ESP, frame - 0x200)
    g.emu.emu_start(0x2CC7CD, 0x2CC841, count=10000)
    assert g.emu.reg_read(UC_X86_REG_EIP) == 0x2CC841
    bound = g.emu.reg_read(UC_X86_REG_EAX)
    assert bound == frame - 0xB8
    result = g.block(4)
    g.emu.reg_write(UC_X86_REG_ESP, STACK - 0x100)
    g.word(STACK - 0x100, STOP)
    g.word(STACK - 0xFC, result)
    g.emu.reg_write(UC_X86_REG_EAX, bound)
    g.emu.emu_start(0x2CCCD0, 0x2CBC60, count=1000)
    assert g.emu.reg_read(UC_X86_REG_EIP) == 0x2CBC60
    esp = g.emu.reg_read(UC_X86_REG_ESP)
    assert g.emu.reg_read(UC_X86_REG_ECX) == cook
    assert g.read(esp + 4) == result
    assert [g.read(esp + 8 + 4*n) for n in range(8)] == parameters
    assert g.read(esp + 0x28) == physics

    # Run the callback's actual match-constructor argument assembly. EBP is
    # the callback's own frame, so arg offsets differ by four from entry ESP.
    callback = STACK - 0x600
    for n in range(12):
        g.word(callback + 4 + 4*n, g.read(esp + 4*n))
    owner = g.block(0x19240)
    g.word(cook + 0x24, owner)
    g.word(callback - 0x14, cook)
    g.word(callback - 0x18, g.read(callback + 0x18))  # native local scene value
    g.word(callback - 0x4C, 0x2000)
    g.word(callback - 0x30, 0x3000)
    g.word(callback - 0x38, 0x4000)
    g.word(callback - 0x44, 0)
    g.emu.reg_write(UC_X86_REG_EBP, callback)
    g.emu.reg_write(UC_X86_REG_ESP, callback - 0x200)
    g.emu.reg_write(UC_X86_REG_EDX, owner)
    g.emu.reg_write(UC_X86_REG_EBX, 0)
    g.emu.emu_start(0x2CC0C5, 0x2D55B0, count=1000)
    assert g.emu.reg_read(UC_X86_REG_EIP) == 0x2D55B0
    entry = g.emu.reg_read(UC_X86_REG_ESP)
    assert g.read(entry + 4) == owner
    assert g.read(entry + 0x20) == physics
    assert g.read(entry + 0x10) == parameters[1]  # match options

    # Start after the native resource/vector initialization. Execute the
    # original pointer/reference writes with zero resource refs. The native
    # scene assignment helper runs too; no destructor side effects here.
    scene_ref, other_ref = g.block(4), g.block(4)
    g.word(entry + 0x14, scene_ref)
    g.word(entry + 0x18, other_ref)
    g.emu.reg_write(UC_X86_REG_ESP, entry - 24)
    g.emu.reg_write(UC_X86_REG_EBP, owner)
    g.emu.reg_write(UC_X86_REG_EBX, 0)
    g.emu.emu_start(0x2D562D, 0x2D5681, count=1000)
    assert g.emu.reg_read(UC_X86_REG_EIP) == 0x2D5681
    assert g.read(owner + 0x130) == physics
    assert g.read(owner + 0x124) == parameters[1]

    # The resource cook releases players/core first, then finalizes and
    # destroys this physics world. Endpoint stubs isolate that ownership
    # order from renderer and Bullet internals.
    scene = g.block(0x10060)
    g.word(owner + 0x19238, scene)
    owner_vt, physics_vt = g.block(4), g.block(16)
    g.word(owner, owner_vt)
    g.word(physics, physics_vt)
    endpoints = [STOP + 0x400, STOP + 0x410, STOP + 0x420]
    g.word(owner_vt, endpoints[0])
    g.word(physics_vt + 12, endpoints[1])
    g.word(physics_vt, endpoints[2])
    events = []

    def stop_at(emu, at, _size, _user):
        if at == 0x2EF590:
            # Scene cleanup is irrelevant to physics ownership.
            esp = emu.reg_read(UC_X86_REG_ESP)
            emu.reg_write(UC_X86_REG_EIP, g.read(esp))
            emu.reg_write(UC_X86_REG_ESP, esp + 8)
        elif at in endpoints:
            index = endpoints.index(at)
            assert emu.reg_read(UC_X86_REG_ECX) == (owner if index == 0 else physics)
            events.append(index)
            esp = emu.reg_read(UC_X86_REG_ESP)
            emu.reg_write(UC_X86_REG_EIP, g.read(esp))
            emu.reg_write(UC_X86_REG_ESP, esp + (4 if index == 1 else 8))
        elif at == 0x42A390:
            esp = emu.reg_read(UC_X86_REG_ESP)
            assert g.read(esp + 4) == physics
            emu.reg_write(UC_X86_REG_EAX, physics)
            emu.reg_write(UC_X86_REG_EIP, g.read(esp))
            emu.reg_write(UC_X86_REG_ESP, esp + 4)
        elif at == 0x2CBC3A:
            emu.emu_stop()  # physics destroyed; remaining operation frees storage

    g.emu.hook_add(UC_HOOK_CODE, stop_at)
    g.call(0x2CBBE0, (owner,))
    assert events == [0, 1, 2], events
    assert g.emu.reg_read(UC_X86_REG_EIP) == 0x2CBC3A


if __name__ == '__main__':
    check(Path(sys.argv[1]).read_bytes())
    print('pickup context: native callback -> match physics/options; teardown order passed')
