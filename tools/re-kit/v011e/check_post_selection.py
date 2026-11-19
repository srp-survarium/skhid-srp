#!/usr/bin/env python3
"""Execute the original 0.11e post effect selector for all quality/feature inputs.

Usage: check_post_selection.py ORIGINAL_EXE EXTRACTED_SHADER_FIXTURE
The fixture comes from resource-porter/extract_011e_weather_shaders.py. This
checks CPU selection and original bytecode identities, not GPU execution.
"""
import argparse
import hashlib
import itertools
import json
from pathlib import Path
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_EDI, UC_X86_REG_EIP

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'common/scripts'))
from exes import Pe

EXE_SHA = '83d6cd0728e3ab71acfa8507b9458c23373ac25ebace9ca4bf1539e53f9f2396'


def check(executable, fixture):
    pe = Pe(executable)
    if hashlib.sha256(pe.data).hexdigest() != EXE_SHA:
        raise ValueError('expected original retail 0.11e build 884')
    shaders = json.loads((fixture / 'manifest.json').read_text())['shaders']
    by_key = {}
    for digest, shader in shaders.items():
        if shader['name'] != 'complex_post_process_blend.ps':
            continue
        code = (fixture / shader['file']).read_bytes()
        if hashlib.sha256(code).hexdigest() != digest:
            raise ValueError('source fixture bytecode changed: ' + shader['file'])
        for permutation in shader['permutations']:
            by_key[permutation['key']] = digest

    emu = Uc(UC_ARCH_X86, UC_MODE_32)
    emu.mem_map(0x608000, 0x1000)
    start = pe.offset(0x608000)
    emu.mem_write(0x608000, pe.data[start:start + 0x1000])
    emu.mem_map(0x8c20000, 0x1000)
    emu.mem_map(0x30000000, 0x4000)
    stage, environment, options = 0x30000000, 0x30001000, 0x30002000
    emu.mem_write(0x8c20d70, struct.pack('<I', options))
    # Unique effect handles at all eight array entries, including the two
    # unreachable bokeh-image-without-DOF entries. A bad index cannot pass.
    emu.mem_write(stage + 0x80, struct.pack('<8I', *range(0x40000000, 0x40000008)))
    results = []
    for quality, dof, image, grain in itertools.product(range(4), range(2), range(2), range(2)):
        emu.mem_write(options + 0xc0, struct.pack('<I', quality))
        for offset, value in [(0x21, dof), (0x2c, image), (0x2d, grain)]:
            emu.mem_write(environment + offset, bytes([value]))
        emu.reg_write(UC_X86_REG_EBX, stage)
        emu.reg_write(UC_X86_REG_EDI, environment)
        emu.emu_start(0x608445, 0x60847a, count=100)
        assert emu.reg_read(UC_X86_REG_EIP) == 0x60847a
        selected = emu.reg_read(UC_X86_REG_EAX) - 0x40000000
        enabled_dof = dof if quality else 0
        enabled_image = image if enabled_dof else 0
        enabled_grain = grain if quality else 0
        expected = 4 * enabled_dof + 2 * enabled_image + enabled_grain
        assert selected == expected, (quality, dof, image, grain, selected, expected)
        # Original catalog mask: quality at 1, bokeh DOF/image at 22/23,
        # image grain at 40. Unused fields stay underscores.
        key = list('_' * 68)
        for index, value in [(1, quality), (22, enabled_dof), (23, enabled_image), (40, enabled_grain)]:
            key[index] = str(value)
        key = ''.join(key)
        digest = by_key[key]
        results.append(dict(quality=quality, dof=bool(dof), image=bool(image), grain=bool(grain),
                            effect_offset=hex(0x80 + selected * 4), permutation=key,
                            dxbc_sha256=digest))
    weather = [row for row in results if row['dof'] and row['image'] and row['grain']]
    return dict(executable_sha256=EXE_SHA, cases=len(results),
                weather_authored_all_three_enabled=weather, selections=results,
                scope='Original CPU effect selection and catalog identity; actual bound shader, textures and pixels require capture.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('executable', type=Path)
    parser.add_argument('fixture', type=Path)
    args = parser.parse_args()
    print(json.dumps(check(args.executable, args.fixture), indent=2))
