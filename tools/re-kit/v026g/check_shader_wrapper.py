#!/usr/bin/env python3
"""Run the stock shader wrapper's bytecode-selection instructions in Unicorn.

Usage: check_shader_wrapper.py STOCK_EXE PORTER_BUNDLE
Checks every shaders/sm_4_0 file in the bundle. This executes 0x3dca69..0x3dca92
after resource pinning; it does not test the resource manager or a GPU device.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32
from unicorn.x86_const import UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'common/scripts'))
from exes import Pe

EXE_SHA = 'ac33d9937015c2299bf62cc6b127cb308b48a3475b30fff2ddd65479ae3bf153'


def check(executable, bundle):
    pe = Pe(executable)
    digest = hashlib.sha256(pe.data).hexdigest()
    if digest != EXE_SHA:
        raise ValueError('expected the reviewed stock 0.26g executable')
    paths = sorted(p for p in (bundle / 'overlay/shaders/sm_4_0').rglob('*') if p.is_file())
    if not paths:
        raise ValueError('bundle contains no shader resources')
    results = []
    for path in paths:
        blob = path.read_bytes()
        count, = struct.unpack_from('<I', blob)
        offset = 4 + 264 * count
        code = blob[offset:]
        if code[:4] != b'DXBC' or len(code) < 32 or struct.unpack_from('<I', code, 24)[0] != len(code):
            raise ValueError(f'{path}: invalid source wrapper or DXBC extent')
        emu = Uc(UC_ARCH_X86, UC_MODE_32)
        emu.mem_map(0x3dc000, 0x1000)
        start = pe.offset(0x3dc000)
        emu.mem_write(0x3dc000, pe.data[start:start + 0x1000])
        emu.mem_map(0x30000000, 0x10000)
        emu.mem_map(0x40000000, (len(blob) + 4095) & ~4095)
        emu.mem_write(0x40000000, blob)
        continuation, resource, stack = 0x30000000, 0x30001000, 0x30008000
        emu.mem_write(continuation + 12, struct.pack('<I', resource))
        emu.mem_write(stack + 0x20, struct.pack('<II', 0x40000000, len(blob)))
        emu.reg_write(UC_X86_REG_ESI, continuation)
        emu.reg_write(UC_X86_REG_ESP, stack)
        emu.emu_start(0x3dca69, 0x3dca92, count=100)
        assert emu.reg_read(UC_X86_REG_EIP) == 0x3dca92
        pointer, = struct.unpack('<I', emu.mem_read(resource + 0x120, 4))
        size, = struct.unpack('<I', emu.mem_read(resource + 0x12c, 4))
        assert pointer == 0x40000000 + offset
        assert size == len(code)
        assert bytes(emu.mem_read(pointer, size)) == code
        results.append(dict(path=str(path.relative_to(bundle / 'overlay')),
                            source_records=count, bytecode_offset=offset,
                            bytecode_size=size, dxbc_sha256=hashlib.sha256(code).hexdigest()))
    return dict(executable_sha256=digest, shaders=results,
                scope='Stock bytecode pointer/length calculation only; resource pinning and GPU creation not exercised.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('executable', type=Path)
    parser.add_argument('bundle', type=Path)
    args = parser.parse_args()
    print(json.dumps(check(args.executable, args.bundle), indent=2))
