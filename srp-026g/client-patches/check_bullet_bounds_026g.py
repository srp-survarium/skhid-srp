#!/usr/bin/env python3
"""Native physics bounds and bullet cutoff, before and after a transient body.

python check_bullet_bounds_026g.py ORIGINAL [PATCHED]

Runs native world construction, add/remove, AABB union/getter and the real
bullet trajectory and cutoff. The Bullet body insertion/removal, body/shape
queries and broadphase update endpoints are fixtures; no bounds or bullet
lifetime decision is stubbed. No private match data is required.
"""
import struct
import sys
from pathlib import Path
from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX,
    UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EIP)
import patch_026g as base

HEAP, STACK, STOP = 0x30000000, 0x30100000, 0x30200000
WORLD, BTWORLD, BODY, PHYSBODY, SHAPE, VT1, VT2, VT3, VT4, BULLET, MANAGER, AMMO, ACCEL = (
    HEAP + i * 0x1000 for i in range(13))
GETBODY, GETBOX, INSERT, REMOVE, UPDATE = (STOP + 0x100 + i * 0x100 for i in range(5))
LEVEL = ((-100., -10., -100.), (100., 20., 100.))
OUTSIDE = ((-1., -100., -1.), (1., -90., 1.))


class Game:
    def __init__(self, data):
        self.u = u = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vs, raw, size, _ in base.Headers(data).sections(data):
            u.mem_map(base.IMAGE_BASE + rva, (max(vs, size) + 4095) & ~4095)
            u.mem_write(base.IMAGE_BASE + rva, bytes(data[raw:raw + min(size, vs or size)]))
        u.mem_map(HEAP, 0x300000)
        self.box, self.events = LEVEL, []
        u.mem_write(GETBODY, b'\xb8' + struct.pack('<I', PHYSBODY) + b'\xc3')
        for addr, cleanup in ((GETBOX, 12), (INSERT, 12), (REMOVE, 4), (UPDATE, 4)):
            u.mem_write(addr, b'\xc2' + struct.pack('<H', cleanup))
            u.hook_add(UC_HOOK_CODE, self.endpoint, begin=addr, end=addr)
        self.word(VT1 + 0x20, UPDATE)
        self.word(VT1 + 0x64, 0x728f0)
        self.word(VT2 + 0x4c, INSERT)
        self.word(VT2 + 0x54, REMOVE)
        self.word(VT3 + 0xc, GETBODY)
        self.word(VT4 + 4, GETBOX)
        for at, val in ((BTWORLD, VT2), (BODY, VT3), (PHYSBODY + 0xcc, SHAPE), (SHAPE, VT4),
                        (MANAGER + 0x98, WORLD), (BULLET + 0x44, MANAGER), (BULLET + 0x40, AMMO)):
            self.word(at, val)
        u.mem_write(AMMO, struct.pack('<f', 1000.))
        u.mem_write(BULLET + 0x34, struct.pack('<3f', 1., 0., 0.))
        u.mem_write(BULLET + 0x6c, struct.pack('<f', .001))
        assert self.read(0x9e2ebc + 0x64) == 0x728f0
        self.fresh()

    def word(self, at, value): self.u.mem_write(at, struct.pack('<I', value))
    def read(self, at): return struct.unpack('<I', self.u.mem_read(at, 4))[0]

    def call(self, va, args=(), regs=None):
        sp = STACK + 0x8000
        self.u.mem_write(sp, struct.pack('<' + 'I' * (len(args) + 1), STOP, *args))
        self.u.reg_write(UC_X86_REG_ESP, sp)
        saved = (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)
        for i, reg in enumerate(saved): self.u.reg_write(reg, 0x13570000 + i)
        for reg, value in (regs or {}).items(): self.u.reg_write(reg, value)
        self.u.emu_start(va, STOP, count=200000)
        assert self.u.reg_read(UC_X86_REG_EIP) == STOP
        assert self.u.reg_read(UC_X86_REG_ESP) == sp + 4 + len(args) * 4
        assert [self.u.reg_read(r) for r in saved] == [0x13570000 + i for i in range(4)]

    def endpoint(self, u, address, size, data):
        sp = u.reg_read(UC_X86_REG_ESP)
        if address == GETBOX:
            for i, b in enumerate(self.box):
                u.mem_write(self.read(sp + 8 + i * 4), struct.pack('<4f', b[0], b[1], -self.box[1 - i][2], 0.))
        elif address == INSERT:
            self.events.append(('insert', self.read(sp + 4), self.read(sp + 8), self.read(sp + 12)))
        else:
            self.events.append(('remove' if address == REMOVE else 'update', self.read(sp + 4)))

    def fresh(self):
        self.call(0x74950, (0,), {UC_X86_REG_EAX: WORLD})
        self.word(WORLD, VT1)
        self.word(WORLD + 0x3c, BTWORLD)

    def add(self, group, box):
        self.box = box
        self.events.clear()
        self.call(0x758b0, (BODY, group, 0xffff), {UC_X86_REG_ECX: WORLD})
        assert self.events == [('insert', PHYSBODY, group, 0xffff), ('update', PHYSBODY)]

    def remove(self):
        self.events.clear()
        self.call(0x75a60, (BODY, 0), {UC_X86_REG_ECX: WORLD})
        assert self.events == [('remove', PHYSBODY)]

    def bounds(self): return struct.unpack('<6f', self.u.mem_read(WORLD + 0x48, 24))

    def survives(self, position):
        self.u.mem_write(BULLET + 0x28, struct.pack('<3f', *position))
        self.u.mem_write(BULLET + 0x10, struct.pack('<3f', *position))
        self.word(BULLET + 0x68, 0)
        reached = []
        def stop(u, address, size, data):
            reached.append(address)
            u.emu_stop()
        hooks = [self.u.hook_add(UC_HOOK_CODE, stop, begin=a, end=a) for a in (0x32ef6a, 0x32ef76)]
        sp = STACK + 0x8000
        self.u.mem_write(sp, struct.pack('<III', STOP, 0, ACCEL))
        self.u.reg_write(UC_X86_REG_ESP, sp)
        self.u.reg_write(UC_X86_REG_EAX, BULLET)
        self.u.emu_start(0x32ee90, STOP, count=20000)
        for hook in hooks: self.u.hook_del(hook)
        assert len(reached) == 1
        return reached[0] == 0x32ef76


def checks(original, built):
    for data, fixed in ((original, False), (built, True)):
        game = Game(data)
        for transient in (0x818, 0xa18):
            game.fresh()
            game.add(8, LEVEL)
            expected = LEVEL[0] + LEVEL[1]
            assert game.bounds() == expected
            assert not game.survives((-50., -30., 50.))
            game.add(transient, OUTSIDE)
            game.remove()  # Its disappearance never shrinks native bounds.
            assert game.survives((-50., -30., 50.)) == (not fixed)
            if fixed: assert game.bounds() == expected
            # Loading earlier state or replaying its insertion also cannot
            # change the fixed bounds; collision insertion still happens.
            game.add(transient, ((-300., -200., -300.), (-290., -190., -290.)))
            game.remove()
            if fixed: assert game.bounds() == expected
            assert game.survives((0., 0., 0.))
            game.fresh()
            game.add(8, LEVEL)
            assert not game.survives((-50., -30., 50.))
        # Both kinds of level collision still grow the bounds. Other native
        # groups stay ignored, including players, hit models and visuals.
        for group in (2, 8, 0xa, 0x824, 0x40, 0x80, 1, 0):
            game.fresh()
            game.add(8, LEVEL)
            game.add(group, OUTSIDE)
            assert (game.bounds()[1] == -100.) == bool(group & 0xa)
    print('ok: original historical bounds diverge; patched native add/remove and bullet cutoff converge')


if __name__ == '__main__':
    original = Path(sys.argv[1]).read_bytes()
    if len(sys.argv) > 2:
        built = Path(sys.argv[2]).read_bytes()
    else:
        built, _ = base.build(original, 'section', Path(base.default_bin('section')).read_bytes(), None,
                              Path(base.late_join_bin()).read_bytes())
    checks(original, built)
