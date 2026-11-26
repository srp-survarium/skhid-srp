#!/usr/bin/env python3
"""Run both decal-UV hooks in the linked client and inspect each draw's material-channel flags."""
from pathlib import Path
import sys

from unicorn.x86_const import UC_X86_REG_EDI, UC_X86_REG_EDX, UC_X86_REG_ESI

import check_forward_effects_026g as fx
import patch_026g as base


def check(original, built):
    stage, surface, thread = 0x38000000, 0x38001000, 0x38002000
    stock, patched = fx.Run(original, 0), fx.Run(built, 0)
    for run in (stock, patched):
        run.emu.mem_map(stage, 0x3000)
        run.word(fx.BACKEND_GLOBAL, fx.BACKEND)
        run.emu.mem_write(thread, bytes.fromhex('b801000000c3'))
        run.word(fx.THREAD_SLOT, thread)
    ctor = {UC_X86_REG_ESI: stage}
    draw = {UC_X86_REG_EDI: fx.BACKEND, UC_X86_REG_EDX: surface}
    name = 'srp_weapon_bake_channels'
    assert patched.span(0x1777E6, 0x1777ED, draw) == stock.span(0x1777E6, 0x1777ED, draw)
    assert not patched.probe_calls
    assert patched.span(0x177000, 0x177005, ctor) == stock.span(0x177000, 0x177005, ctor)
    assert [call for _, call in patched.probe_calls] == [
        ('create', name), ('assign', name), ('register', fx.BACKEND, name, 0)]
    # Complete a bake, then draw a plain surface again on the same backend.
    for mask in (*range(16), 0):
        values = tuple(float(bool(mask & (1 << i))) for i in range(4))
        for run in (stock, patched):
            for i, bound in enumerate(values):
                run.word(surface + 4*i, surface + 0x100 if bound else 0)
        assert patched.span(0x1777E6, 0x1777ED, draw) == stock.span(0x1777E6, 0x1777ED, draw)
        assert [call for _, call in patched.probe_calls] == [('set_ps_constant', fx.HOST, fx.BACKEND, values)]
    print("registered the shader host and selected all 16 combinations of native texture slots on successive draws")


if __name__ == '__main__':
    source = Path(sys.argv[1]).read_bytes()
    client, _ = base.build(source, "section", Path(base.default_bin("section")).read_bytes(), None,
                           Path(base.late_join_bin()).read_bytes())
    check(source, client)
