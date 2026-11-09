#!/usr/bin/env python3
"""Compare the viewer's aim transform with unmodified 0.26g machine code.

Usage: python check_engine.py <original survarium.exe>
Requires numpy and unicorn. Does not build or patch a game executable.
"""

import hashlib
import json
from pathlib import Path
import struct
import sys

import numpy as np
from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL
from unicorn.x86_const import (
    UC_X86_REG_EAX,
    UC_X86_REG_ECX,
    UC_X86_REG_ESI,
    UC_X86_REG_ESP,
    UC_X86_REG_EIP,
)

import model

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tools/re-kit/common/scripts"))
from exes import Pe

STACK, WEAPON, BONES, OUT, ITEM, RETURN = (
    0x20000000,
    0x20100000,
    0x20200000,
    0x20300000,
    0x20400000,
    0x20500000,
)


def run(exe):
    pe = Pe(exe)
    # This check's addresses are only meaningful for the re-kit's original image.
    identity = json.loads((ROOT / "tools/re-kit/v026g/identity.json").read_text())
    recorded = json.dumps(identity).lower()
    if hashlib.sha256(pe.data).hexdigest() not in recorded:
        raise ValueError(
            "not the executable recorded in tools/re-kit/v026g/identity.json"
        )
    uc = Uc(UC_ARCH_X86, UC_MODE_32)
    for _, va, vsize, raw, rsize in pe.sections:
        uc.mem_map(va, (max(vsize, rsize) + 4095) & ~4095, UC_PROT_ALL)
        if rsize:
            uc.mem_write(va, pe.data[raw : raw + rsize])
    for address in (STACK, WEAPON, BONES, OUT, ITEM, RETURN):
        uc.mem_map(address, 0x10000)

    def write(address, m):
        uc.mem_write(address, np.asarray(m, dtype="<f4").tobytes())

    # calculate_locator's one-time rotation_y(pi), initialized before the call.
    uc.mem_write(0xA2627A8, b"\1\0\0\0")
    write(0xA262768, model.HALF)
    rng = np.random.default_rng(2777)
    for case in range(120):
        loc = model.matrix(rng.uniform(-0.3, 0.3, 3), rng.uniform(-0.4, 0.4, 3))
        pose = model.matrix(rng.uniform(-0.3, 0.3, 3), rng.uniform(-1, 1, 3))
        item = model.matrix(rng.uniform(-0.3, 0.3, 3), rng.uniform(-1, 1, 3))
        offset = model.matrix((0, 0, float(rng.uniform(-0.1, 0.1))))
        bone = 0xFFFF if case % 3 == 0 else 1
        write(WEAPON + 0x6A0, offset)
        uc.mem_write(WEAPON + 0x7A8, b"aim\0" + bytes(28))
        write(WEAPON + 0x7C8, loc)
        uc.mem_write(WEAPON + 0x808, struct.pack("<H", bone))
        write(BONES, np.eye(4))
        write(BONES + 64, pose)
        write(ITEM, item)
        esp = STACK + 0x8000
        uc.mem_write(esp, struct.pack("<II", RETURN, ITEM))
        for reg, value in (
            (UC_X86_REG_EAX, WEAPON),
            (UC_X86_REG_ECX, BONES),
            (UC_X86_REG_ESI, OUT),
            (UC_X86_REG_ESP, esp),
        ):
            uc.reg_write(reg, value)
        uc.emu_start(0x244D40, RETURN, count=20000)
        if (
            uc.reg_read(UC_X86_REG_EIP) != RETURN
            or uc.reg_read(UC_X86_REG_ESP) != esp + 8
        ):
            raise AssertionError("calculate_aim_alignment did not return with ret 4")
        actual = np.frombuffer(uc.mem_read(OUT, 64), "<f4").reshape(4, 4)
        expected = (
            np.linalg.inv(
                offset @ model.HALF @ loc @ (pose if bone != 0xFFFF else np.eye(4))
            )
            @ item
        )
        np.testing.assert_allclose(actual, expected, rtol=3e-5, atol=3e-6)
    print(
        "120 retail aim-alignment cases agree, including attached and unbound locators"
    )


if __name__ == "__main__":
    run(sys.argv[1])
