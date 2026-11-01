#!/usr/bin/env python3
"""Patch 90 (tactical_reload): a two-clip reload (state type 314) with rounds
left in the magazine lasts parameters/tactical_reload_time, an empty one
reload_time, as retail 0.69d0 times them; a weapon without
tactical_reload_time keeps the game's one speed.

Runs, in the original and the patched exe, on made-up objects:

- the cook's construction of the state, the sound handler's ctor 0x255260
  (the state's own ctor 0x30FC50 inside it, computed_show_animation_time_scale
  0x30D750 on made-up clips): the original's speed is the tactical clip's
  length over reload_time and +16Ch is never written; the patched one's, with
  tactical_reload_time in the config, is the empty clip's over reload_time and
  +16Ch the tactical clip's over tactical_reload_time; without it, the
  original's speed and +16Ch 0; every other byte of the state the original's;
- the reload's hands clip, get_weapon_lexeme 0x30FE90 to its lexeme call
  0x30FF33 (apply_modifier stubbed: it records its value and doubles it): the
  clip slot the game's (2 x empty + 4 x crouch), the speed handed to
  apply_modifier the tactical one with rounds left and the empty one without
  (patched, split), the original's for every magazine otherwise, the
  modifier's result the one the lexeme gets; the registers but ECX/EDX/EAX as
  the original's.

usage: python3 check_tactical_reload_026g.py <original survarium.exe>
"""
import importlib.util
import struct
import sys
from pathlib import Path

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX,
                               UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP,
                               UC_X86_REG_EIP, UC_X86_REG_XMM0)
import patch_026g as base

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('content_edit', ROOT / 'scripts/content-edit/content_edit.py')
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)

HANDLER_CTOR, HANDLER_RET = 0x255260, 0x18          # the sound handler's ctor: ESI this, EAX the clips; ret 18h
LEXEME, LEXEME_CALL = 0x30FE90, 0x30FF33            # get_weapon_lexeme: EDI this; ret 0Ch; its call of the lexeme
APPLY_MODIFIER = 0x2D5AE0                           # EAX modifiers, ESI index; value, weight; ret 8; XMM0
THREAD_SLOT = 0x870118                              # kernel32 GetCurrentThreadId's IAT slot (a resource's owner thread)
TIME_SCALE, TACTICAL_SCALE, HANDS = 0x138, 0x16C, 0x144
AMMO = 0x5E5

HEAP, STACK, SENTINEL = 0x40000000, 0x41000000, 0x42000000
STATE, PARAMS, WEAPON, PLAYER, PROFILE = (HEAP + i * 0x1000 for i in range(5))
CONFIG, CLIPS, HANDLER_ARGS = HEAP + 0x5000, HEAP + 0x6000, HEAP + 0x7000
THREAD = HEAP + 0xF0000                             # GetCurrentThreadId: mov eax, 1; ret
ANIMATIONS = HEAP + 0x10000                         # 10 made-up clips, 0x400 each
CFG = HEAP + 0x40000                                # the weapon config, relocated
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX,
        UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)
GAME = dict(zip(REGS, (0x0A0A0A0A, 0x0B0B0B0B, 0x0C0C0C0C, 0x0D0D0D0D, 0x05050505, 0x0E0E0E0E, 0x0019FF00)))

# The clips by the config's order (2 item clips, then the 8 hands clips by
# 4 x view + 2 x crouch + empty), each its length in frames (30 a second):
# the tactical clip of the first hands slot 160 frames, the empty one 180.
TACTICAL_FRAMES, EMPTY_FRAMES = 160.0, 180.0
FRAMES = [100.0, 120.0] + [TACTICAL_FRAMES, EMPTY_FRAMES, 150.0, 170.0, 161.0, 181.0, 151.0, 171.0]


def f32(x):
    return struct.unpack('<f', struct.pack('<f', x))[0]


def relocated_config(value):
    """The binary config `value` as the game's loader leaves it at CFG (check_lamps_026g's)."""
    b = bytearray(24)
    b[:24] = c.value_record(b, 0, '', value)
    offsets = []

    def visit(off):
        d, name, crc, t, count = c.rec(b, off)
        offsets.append((off, d, name, crc, t, count))
        if t in (3, 4):
            for kid in c.children(b, off):
                visit(kid)
    visit(0)
    for off, d, name, crc, t, count in offsets:
        c.REC.pack_into(b, off, d + CFG if t >= 3 and d else d, name + CFG if name else 0, crc, t, count)
    return b


class Game:
    def __init__(self, data):
        self.u = u = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            u.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            u.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at in (HEAP, STACK, SENTINEL):
            u.mem_map(at, 0x100000, UC_PROT_ALL)
        u.hook_add(UC_HOOK_CODE, self.modifier, begin=APPLY_MODIFIER, end=APPLY_MODIFIER)
        self.w(THREAD_SLOT, THREAD)
        u.hook_add(UC_HOOK_CODE, self.stop)
        self.stops, self.applied = (), []

    def w(self, at, value, fmt='<I'):
        self.u.mem_write(at, struct.pack(fmt, value))

    def r(self, at, fmt='<I'):
        return struct.unpack(fmt, self.u.mem_read(at, struct.calcsize(fmt)))[0]

    def modifier(self, u, *_):
        """apply_modifier: records (index, value, weight), returns 2 x value."""
        sp = u.reg_read(UC_X86_REG_ESP)
        value, weight = self.r(sp + 4, '<f'), self.r(sp + 8, '<f')
        self.applied.append((u.reg_read(UC_X86_REG_ESI), value, weight))
        u.reg_write(UC_X86_REG_XMM0, struct.unpack('<I', struct.pack('<f', 2 * value))[0])
        u.reg_write(UC_X86_REG_ESP, sp + 12)
        u.reg_write(UC_X86_REG_EIP, self.r(sp))

    def stop(self, u, address, *_):
        if address in self.stops:
            self.stopped = address
            u.emu_stop()

    def run(self, start, stops, sp, regs):
        for reg, value in {**GAME, **regs}.items():
            self.u.reg_write(reg, value)
        self.u.reg_write(UC_X86_REG_ESP, sp)
        self.stops, self.stopped, self.applied = stops, None, []
        try:
            self.u.emu_start(start, 0, count=200000)
        except Exception as error:
            raise AssertionError(f'{error} at {self.u.reg_read(UC_X86_REG_EIP):#x}') from error
        assert self.stopped in stops, hex(self.u.reg_read(UC_X86_REG_EIP))
        return {reg: self.u.reg_read(reg) for reg in REGS}, self.u.reg_read(UC_X86_REG_ESP)

    def setup(self, parameters, ammo):
        """The objects: a 1B0h state (CCh), its params, the config with `parameters`, the clips."""
        self.u.mem_write(HEAP, bytes(0x100000))
        self.u.mem_write(THREAD, bytes.fromhex('b801000000c3'))
        self.u.mem_write(STATE, b'\xCC' * 0x1B0)
        self.u.mem_write(CFG, bytes(relocated_config({'parameters': parameters})))
        self.w(CONFIG + 0x108, CFG)
        self.w(CONFIG + 0xD0, 5)                          # a held resource's count
        self.w(PARAMS + 0x0, CONFIG)
        self.w(PARAMS + 0x4, WEAPON)
        self.w(PARAMS + 0x10, parameters['reload_time'], '<f')
        self.w(WEAPON + 0x8, PLAYER)
        self.u.mem_write(WEAPON + AMMO, bytes([ammo]))
        self.w(PLAYER + 0x6B8, PROFILE)
        for i, frames in enumerate(FRAMES):
            # a clip resource: its count +0DCh, its memory block [+0D4h], whose
            # pinned data (+34h) has the knots computed_show_animation_time_scale
            # reads: [D+14h] the record's offset E - D; [E] 2 knots, [E+4] 10h;
            # the first knot at E + 30h, the last at E + 34h.
            clip = ANIMATIONS + 0x400 * i
            block = clip + 0x100
            d = block + 0x34
            e = d + 0x20
            self.w(clip + 0xDC, 5)
            self.w(clip + 0xD4, block)
            self.w(d + 0x14, 0x20)
            self.w(e, 2)
            self.w(e + 4, 0x10)
            self.w(e + 0x30, 0.0, '<f')
            self.w(e + 0x34, frames, '<f')
            self.w(CLIPS + 4 * i, clip)

    def construct(self):
        """The sound handler's ctor, as on_subresources_ready calls it (0x25331E)."""
        sp = STACK + 0x8000
        args = [PARAMS] + [HANDLER_ARGS + 0x100 * i for i in range(5)]
        self.u.mem_write(sp, struct.pack('<7I', SENTINEL, *args))
        _, after = self.run(HANDLER_CTOR, (SENTINEL,), sp, {UC_X86_REG_ESI: STATE, UC_X86_REG_EAX: CLIPS})
        assert after == sp + 4 + HANDLER_RET, hex(after)
        return bytes(self.u.mem_read(STATE, 0x1B0))

    def lexeme(self, crouch):
        """get_weapon_lexeme to its lexeme call: (the hands slot, apply_modifier's calls, the registers)."""
        sp = STACK + 0x9000
        self.u.mem_write(sp, struct.pack('<4I', SENTINEL, HEAP + 0x8000, HEAP + 0x9000, 1 if crouch else 0))
        regs, _ = self.run(LEXEME, (LEXEME_CALL,), sp, {UC_X86_REG_EDI: STATE})
        slot = (self.r(self.u.reg_read(UC_X86_REG_ESP) + 4) - (STATE + HANDS)) // 4
        return slot, self.applied, regs


def scale(frames, seconds):
    """computed_show_animation_time_scale: (end - start) x (1/30) / time, in f32 as the game's."""
    return f32(f32(f32(frames) * f32(1 / 30)) / f32(seconds))


def main():
    original = open(sys.argv[1], 'rb').read()
    patched, _ = base.build(original, 'section', open(base.default_bin('section'), 'rb').read(), None,
                            open(base.late_join_bin(), 'rb').read())
    old, new = Game(original), Game(patched)
    reload_time, tactical_time = 3.0, 2.2
    split = {'reload_time': reload_time, 'tactical_reload_time': tactical_time}
    native = {'reload_time': reload_time}

    native_speed = scale(TACTICAL_FRAMES, reload_time)
    empty_speed, tactical_speed = scale(EMPTY_FRAMES, reload_time), scale(TACTICAL_FRAMES, tactical_time)

    for g in (old, new):
        g.setup(split, 30)
    before, after = old.construct(), new.construct()
    assert struct.unpack_from('<f', before, TIME_SCALE)[0] == native_speed
    assert before[TACTICAL_SCALE:TACTICAL_SCALE + 4] == b'\xCC' * 4, 'the original writes +16Ch'
    assert struct.unpack_from('<f', after, TIME_SCALE)[0] == empty_speed, struct.unpack_from('<f', after, TIME_SCALE)
    assert struct.unpack_from('<f', after, TACTICAL_SCALE)[0] == tactical_speed
    changed = [i for i in range(0, 0x1B0, 4) if before[i:i + 4] != after[i:i + 4]]
    assert changed == [TIME_SCALE, TACTICAL_SCALE], [hex(i) for i in changed]
    print(f'ok cooked with tactical_reload_time: the empty clip {empty_speed:.4f} (180 frames in {reload_time} s), '
          f'the tactical {tactical_speed:.4f} (160 in {tactical_time} s); the original {native_speed:.4f} '
          'for both and +16Ch untouched; nothing else differs')

    for (label, ammo, crouch, slot, speed) in (('rounds left, standing', 12, False, 0, tactical_speed),
                                               ('rounds left, crouching', 1, True, 4, tactical_speed),
                                               ('empty, standing', 0, False, 2, empty_speed),
                                               ('empty, crouching', 0, True, 6, empty_speed)):
        new.u.mem_write(WEAPON + AMMO, bytes([ammo]))
        old.u.mem_write(WEAPON + AMMO, bytes([ammo]))
        got, applied, regs = new.lexeme(crouch)
        was, was_applied, was_regs = old.lexeme(crouch)
        assert got == was == slot, (label, got, was)
        assert applied == [(13, speed, 1.0)], (label, applied)
        assert was_applied == [(13, native_speed, 1.0)], (label, was_applied)
        lexeme_speed = new.r(new.u.reg_read(UC_X86_REG_ESP) + 0xC, '<f')
        assert lexeme_speed == f32(2 * speed), (label, lexeme_speed)
        for reg in (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP):
            assert regs[reg] == was_regs[reg], (label, reg)
        print(f'ok {label}: slot {slot} (the game\'s), speed {speed:.4f} into apply_modifier 13, its result to the lexeme')

    for g in (old, new):
        g.setup(native, 30)
    before, after = old.construct(), new.construct()
    assert struct.unpack_from('<f', after, TIME_SCALE)[0] == native_speed
    assert struct.unpack_from('<f', after, TACTICAL_SCALE)[0] == 0.0
    changed = [i for i in range(0, 0x1B0, 4) if before[i:i + 4] != after[i:i + 4]]
    assert changed == [TACTICAL_SCALE], [hex(i) for i in changed]
    for ammo, crouch in ((30, False), (0, False), (5, True), (0, True)):
        new.u.mem_write(WEAPON + AMMO, bytes([ammo]))
        old.u.mem_write(WEAPON + AMMO, bytes([ammo]))
        got, applied, _ = new.lexeme(crouch)
        was, was_applied, _ = old.lexeme(crouch)
        assert (got, applied) == (was, was_applied) == (got, [(13, native_speed, 1.0)]), (ammo, crouch)
    print(f'ok cooked without tactical_reload_time: the game\'s speed {native_speed:.4f} for both clips, '
          'every magazine and stance as the original')

    for label, parameters in (('a tactical time of 0', {'reload_time': 3.0, 'tactical_reload_time': 0.0}),
                              ('a negative tactical time', {'reload_time': 3.0, 'tactical_reload_time': -1.0}),
                              ('a text', {'reload_time': 3.0, 'tactical_reload_time': 'fast'})):
        new.setup(parameters, 0)
        after = new.construct()
        assert struct.unpack_from('<f', after, TIME_SCALE)[0] == native_speed, label
        assert struct.unpack_from('<f', after, TACTICAL_SCALE)[0] == 0.0, label
    print('ok a tactical time that is none (0, negative, a text): the game\'s timing')

    new.setup({'reload_time': 2.5, 'tactical_reload_time': 1.75}, 7)
    after = new.construct()
    _, applied, _ = new.lexeme(False)
    assert applied == [(13, scale(TACTICAL_FRAMES, 1.75), 1.0)]
    new.u.mem_write(WEAPON + AMMO, bytes([0]))
    _, applied, _ = new.lexeme(False)
    assert applied == [(13, scale(EMPTY_FRAMES, 2.5), 1.0)]
    print('ok other times (2.5 s empty, 1.75 s tactical): each clip its own')


if __name__ == '__main__':
    main()
