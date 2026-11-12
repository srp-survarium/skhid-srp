#!/usr/bin/env python3
"""Observe the stock scene post-constant constructor/setter in Unicorn.

The real x86 bodies run; string interning, host registration and the backend
constant upload are intercepted. This checks the CPU binding contract, not
shader execution, scene ownership or GPU output. Requires Python unicorn.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
from unicorn.x86_const import (
    UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EDI, UC_X86_REG_EIP,
    UC_X86_REG_ESI, UC_X86_REG_ESP,
)

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'common/scripts'))
from exes import Pe

EXPECTED_SHA = 'ac33d9937015c2299bf62cc6b127cb308b48a3475b30fff2ddd65479ae3bf153'
HOSTS = {
    0x00: 'frame_height_lights_and_desaturation',
    0x04: 'frame_mid_tones',
    0x08: 'frame_shadows',
    0x0c: 'gamma_correction_factor',
    0x10: 'frame_fade',
    0x14: 'filmic_tonemap_packed_parameters_0',
    0x18: 'filmic_tonemap_packed_parameters_1',
    0x1c: 'bloom_parameters',
    0x20: 'blueshift',
    0x24: 's_eye_ray_corner',
    0x28: 'image_grain_parameters',
}
DOF_HOSTS = {
    0x00: 'dof_parameters',
    0x04: 'dof_height_lights',
    0x08: 'blurriness_amount',
    0x0c: 'bokeh_dof_parameters',
}
HEAP, STACK, STOP = 0x30000000, 0x31000000, 0x32000000


def audit(executable):
    pe = Pe(executable)
    digest = hashlib.sha256(pe.data).hexdigest()
    if digest != EXPECTED_SHA:
        raise ValueError('expected the reviewed stock 0.26g executable')
    emu = Uc(UC_ARCH_X86, UC_MODE_32)
    for _, start, vsize, raw, rsize in pe.sections:
        emu.mem_map(start, (max(vsize, rsize) + 4095) & ~4095)
        emu.mem_write(start, pe.data[raw:raw+rsize])
    for address in (HEAP, STACK, STOP):
        emu.mem_map(address, 0x10000)

    def word(address):
        return struct.unpack('<I', emu.mem_read(address, 4))[0]

    def write_words(address, *values):
        emu.mem_write(address, struct.pack('<' + 'I' * len(values), *values))

    def return_from_call(cleanup, result=None):
        sp = emu.reg_read(UC_X86_REG_ESP)
        target = word(sp)
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + cleanup)
        if result is not None:
            emu.reg_write(UC_X86_REG_EAX, result)
        emu.reg_write(UC_X86_REG_EIP, target)

    interned, registered, uploads = {}, {}, []

    def hook(_, address, size, user):
        sp = emu.reg_read(UC_X86_REG_ESP)
        if address == 0xe0b50:
            source = word(sp + 4)
            text = bytearray()
            while (byte := emu.mem_read(source + len(text), 1)) != b'\0':
                text.extend(byte)
            shared = HEAP + 0x1000 + len(interned) * 0x100
            interned[shared] = text.decode('ascii')
            write_words(shared, 1)
            return_from_call(4, shared)
        elif address == 0x3c8bc0:
            assert emu.reg_read(UC_X86_REG_EAX) == HEAP + 0x8000
            assert word(sp + 8) == 0
            name = interned[word(word(sp + 4))]
            host = HEAP + 0x3000 + len(registered) * 0x100
            registered[host] = name
            return_from_call(8, host)
        elif address == 0x132a90:
            assert emu.reg_read(UC_X86_REG_EDI) == HEAP + 0x8000
            host = emu.reg_read(UC_X86_REG_EAX)
            name = registered[host]
            # Mid/shadow are float3; gamma is scalar. Do not read past them.
            count = 3 if name in ('frame_mid_tones', 'frame_shadows') else (
                1 if name == 'gamma_correction_factor' else 4)
            values = struct.unpack('<' + 'f' * count, emu.mem_read(word(sp + 4), count * 4))
            uploads.append((name, values))
            return_from_call(4)
        elif address in (0xe0ae0,):
            raise AssertionError('unexpected destruction of a registered name')

    emu.hook_add(UC_HOOK_CODE, hook)
    write_words(0x2260490, HEAP + 0x8000)
    sp = STACK + 0x8000
    write_words(sp, STOP)
    emu.reg_write(UC_X86_REG_ESP, sp)
    emu.reg_write(UC_X86_REG_EDI, HEAP)
    emu.emu_start(0x1af7c0, STOP, count=10000)
    assert emu.reg_read(UC_X86_REG_EIP) == STOP
    assert emu.reg_read(UC_X86_REG_ESP) == sp + 4
    assert emu.reg_read(UC_X86_REG_EAX) == HEAP
    actual = {offset: registered[word(HEAP + offset)] for offset in HOSTS}
    assert actual == HOSTS, actual

    for case in range(64):
        # Distinct exactly representable markers expose swapped pointers/components.
        def vector(address, number, count):
            values = tuple(float(case * 128 + number + i) for i in range(count))
            emu.mem_write(address, struct.pack('<' + 'f' * count, *values))
            return values

        height = vector(HEAP + 0x4000, 1, 3)
        fade = vector(HEAP + 0x4100, 11, 3)
        mid = vector(HEAP + 0x4200, 21, 3)
        shadow = vector(HEAP + 0x4300, 31, 3)
        grain = vector(HEAP + 0x4400, 41, 4)
        blue = vector(HEAP + 0x5000 + 0x1f8, 51, 4)
        bloom = vector(HEAP + 0x5000 + 0x230, 61, 1)
        gamma = float(case + 1)
        gamma_word, = struct.unpack('<I', struct.pack('<f', gamma))
        write_words(sp, STOP, HEAP + 0x4200, HEAP + 0x4300, gamma_word,
                    HEAP + 0x4400, HEAP + 0x5000)
        emu.reg_write(UC_X86_REG_ESP, sp)
        emu.reg_write(UC_X86_REG_ESI, HEAP)
        emu.reg_write(UC_X86_REG_ECX, HEAP + 0x4000)
        emu.reg_write(UC_X86_REG_EAX, HEAP + 0x4100)
        uploads.clear()
        emu.emu_start(0x1afb10, STOP, count=10000)
        assert emu.reg_read(UC_X86_REG_EIP) == STOP
        assert emu.reg_read(UC_X86_REG_ESP) == sp + 24
        assert uploads == [
            (HOSTS[0], height + (1.0,)), (HOSTS[4], mid), (HOSTS[8], shadow),
            (HOSTS[12], (gamma,)), (HOSTS[16], fade + (1.0,)),
            (HOSTS[32], blue), (HOSTS[28], bloom + (0.0, 0.0, 0.0)),
            (HOSTS[40], grain),
        ], uploads

    scene_uploads = [name for name, _ in uploads]
    # The native DOF setter retains the bokeh upload, but hardcodes focus
    # power to 1.0 instead of accepting the original explicit power argument.
    dof_object = HEAP + 0x6000
    write_words(sp, STOP)
    emu.reg_write(UC_X86_REG_ESP, sp)
    emu.reg_write(UC_X86_REG_EDI, dof_object)
    emu.emu_start(0x1af590, STOP, count=10000)
    assert emu.reg_read(UC_X86_REG_EIP) == STOP
    assert emu.reg_read(UC_X86_REG_ESP) == sp + 4
    assert emu.reg_read(UC_X86_REG_EAX) == dof_object
    assert {offset: registered[word(dof_object + offset)] for offset in DOF_HOSTS} == DOF_HOSTS
    for case in range(64):
        height = vector(HEAP + 0x4000, 1, 3)
        values = tuple(float(case * 128 + 11 + i) for i in range(6))
        distance, region, near, far, radius, density = values
        emu.mem_write(sp, struct.pack('<I6f', STOP, *values))
        emu.reg_write(UC_X86_REG_ESP, sp)
        emu.reg_write(UC_X86_REG_ESI, dof_object)
        emu.reg_write(UC_X86_REG_EAX, HEAP + 0x4000)
        uploads.clear()
        emu.emu_start(0x1af6d0, STOP, count=10000)
        assert emu.reg_read(UC_X86_REG_EIP) == STOP
        assert emu.reg_read(UC_X86_REG_ESP) == sp + 28
        assert uploads == [
            (DOF_HOSTS[4], height + (0.0,)),
            (DOF_HOSTS[0], (distance, region, 1.0, 0.0)),
            (DOF_HOSTS[8], (near, far, 0.0, 0.0)),
            (DOF_HOSTS[12], (radius, density, 0.0, 0.0)),
        ], uploads

    return dict(executable_sha256=digest, cases=64,
                constructor='0x001af7c0', setter='0x001afb10',
                host_offsets={f'0x{k:02x}': v for k, v in HOSTS.items()},
                uploaded=scene_uploads,
                registered_but_not_uploaded=sorted(set(HOSTS.values()) - set(scene_uploads)),
                dof=dict(cases=64, constructor='0x001af590', setter='0x001af6d0',
                         host_offsets={f'0x{k:02x}': v for k, v in DOF_HOSTS.items()},
                         uploaded=[name for name, _ in uploads], focus_power=1.0),
                scope='Real stock x86; backend uploads intercepted, no GPU or scene lifecycle test.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('executable', type=Path)
    args = parser.parse_args()
    print(json.dumps(audit(args.executable), indent=2))
