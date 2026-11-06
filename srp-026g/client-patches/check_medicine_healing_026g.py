#!/usr/bin/env python3
"""Execute native 0.26g medicine healing against the authoritative TSV.

The real constructor reads relocated binary config values and normalizes
course totals into rates; the real tick and increase_health apply them.
Run with the stock executable (the full Unicorn gate), or a shipped patched
exe and --configs <extracted archive folder> to verify final content.

Only allocation, base item initialization, owner/pool lookup, identity skill
modifiers and unrelated observer/injury/activation-list bookkeeping are
stubbed. No incoming hits, regeneration, networking or HUD are simulated.
Each pool starts empty with its native maximum. Execution stops constructing
after the influence array has been loaded, before unrelated injury tables.
"""
from pathlib import Path
import argparse
import math
import struct
import sys
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'scripts/content-edit'))
import patch_026g as base
import content_edit as ce
from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
from unicorn.x86_const import *
MAXIMUM = {'head': 0.4, 'body': 1.0, 'pain': 1.0, 'left_hand': 0.5, 'right_hand': 0.5, 'left_leg': 0.5, 'right_leg': 0.5, 'infection': 1.0}
DATA = 0x30000000
CFG = DATA
DRUG = DATA + 0x10000
HEAP = DATA + 0x20000
STACK = DATA + 0x7f000
STOP = DATA + 0x80000
PLAYER = DATA + 0x40000
HOLDER = DATA + 0x42000
OWNER = DATA + 0x43000
VT = DATA + 0x44000
DM = DATA + 0x45000
POOLS = DATA + 0x48000
PROFILE = DATA + 0x50000
GET_PLAYER = STOP + 16
GET_DM = STOP + 32

class Game:

    def __init__(self, exe, config):
        self.u = u = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vs, raw, rs, _ in base.Headers(exe).sections(exe):
            start = base.IMAGE_BASE + rva
            u.mem_map(start, max(vs, rs) + 0xfff & ~0xfff)
            u.mem_write(start, exe[raw:raw + min(rs, vs or rs)])
        u.mem_map(DATA, 0x100000)
        self.next = HEAP
        self.pools = {}
        b = bytearray(config)
        recs = []

        def visit(at):
            d, n, crc, t, count = ce.rec(b, at)
            recs.append((at, d, n, crc, t, count))
            if t in (3, 4):
                for child in ce.children(b, at):
                    visit(child)
        visit(0)
        for at, d, n, crc, t, count in recs:
            ce.REC.pack_into(b, at, d + CFG if t >= 3 and d else d, n + CFG if n else 0, crc, t, count)
        u.mem_write(CFG, bytes(b))
        # Native config lookup CRC table, initialized as the game does.
        for i in range(256):
            value = i
            for _ in range(8):
                value = value >> 1 ^ (0xedb88320 if value & 1 else 0)
            self.word(0x225fd68 + i * 4, value)
        u.hook_add(UC_HOOK_CODE, self.hook)

    def word(self, a, v):
        self.u.mem_write(a, struct.pack('<I', v & 0xffffffff))

    def read(self, a):
        return struct.unpack('<I', self.u.mem_read(a, 4))[0]

    def flt(self, a, v):
        self.u.mem_write(a, struct.pack('<f', v))

    def f(self, a):
        return struct.unpack('<f', self.u.mem_read(a, 4))[0]

    def text(self, a):
        out = b''
        while self.u.mem_read(a + len(out), 1) != b'\x00':
            out += bytes(self.u.mem_read(a + len(out), 1))
        return out.decode()

    def ret(self, pop=0):
        sp = self.u.reg_read(UC_X86_REG_ESP)
        dest = self.read(sp)
        self.u.reg_write(UC_X86_REG_ESP, sp + 4 + pop)
        self.u.reg_write(UC_X86_REG_EIP, dest)

    def hook(self, u, a, size, _):
        sp = u.reg_read(UC_X86_REG_ESP)
        if a == STOP:
            u.emu_stop()
            return
        # Allocation/base-item setup and unrelated game-world plumbing.
        if a == 0x2ffb80:
            self.ret(12)
        elif a == 0x420789:
            u.reg_write(UC_X86_REG_EAX, 0)
            self.ret()
        elif a == 0xe4f90:
            size = u.reg_read(UC_X86_REG_EAX)
            p = self.next
            self.next += size + 15 & ~15
            u.reg_write(UC_X86_REG_EAX, p)
            self.ret(4)
        elif a == 0x424580:
            d, s, n = [self.read(sp + i) for i in (4, 8, 12)]
            u.mem_write(d, bytes(u.mem_read(s, n)))
            u.reg_write(UC_X86_REG_EAX, d)
            self.ret()
        elif a == GET_PLAYER:
            u.reg_write(UC_X86_REG_EAX, PLAYER)
            self.ret()
        elif a == GET_DM:
            u.reg_write(UC_X86_REG_EAX, DM)
            self.ret()
        elif a == 0x2d5ae0:
            u.reg_write(UC_X86_REG_XMM0, self.read(sp + 4))
            self.ret(8)
        elif a == 0x2e58b0:
            name = self.text(self.read(u.reg_read(UC_X86_REG_ECX)))
            u.reg_write(UC_X86_REG_EAX, self.pools[name])
            self.ret()
        elif a in (0x33da50, 0x2d5a90, 0x32a9c0, 0x32b920):
            self.ret(4 if a in (0x33da50, 0x32b920) else 0)
        elif a == 0x33d390:
            self.ret(8)

    def run(self, start, args=(), until=STOP, **regs):
        sp = STACK
        self.word(sp, STOP)
        for i, v in enumerate(args):
            self.word(sp + 4 * (i + 1), v)
        self.u.reg_write(UC_X86_REG_ESP, sp)
        for k, v in regs.items():
            self.u.reg_write(globals()['UC_X86_REG_' + k.upper()], v)
        self.u.emu_start(start, until, count=500000)
        assert self.u.reg_read(UC_X86_REG_EIP) == until, hex(self.u.reg_read(UC_X86_REG_EIP))

    def check(self):
        self.run(0x33cac0, [DRUG], until=0x33cde4, eax=CFG)
        count = self.u.mem_read(DRUG + 0x288, 1)[0]
        influences = self.read(DRUG + 0x280)
        result = {self.text(influences + i * 20): self.f(influences + i * 20 + 16) for i in range(count)}
        self.word(DRUG + 0x250, HOLDER)
        self.word(HOLDER + 0x174, OWNER)
        self.word(OWNER, VT)
        self.word(VT + 12, GET_PLAYER)
        self.u.mem_write(PLAYER + 0x33c, b'\x01')
        self.word(PLAYER + 0x6b8, PROFILE)
        self.word(PLAYER + 0x108, VT)
        self.word(VT + 8, GET_DM)
        self.word(DM, DM + 256)
        self.word(DM + 256 + 0xd0, 10)
        for i, name in enumerate(result):
            pool = POOLS + i * 0x400
            self.pools[name] = pool
            self.flt(pool + 0x9c, MAXIMUM[name])
            self.flt(pool + 0xa0, 0.0)
        duration = self.read(DRUG + 0x29c)
        delay = self.read(DRUG + 0x2a4)
        self.word(DRUG + 0x2a8, duration)
        self.word(DRUG + 0x2ac, delay)
        timeline = []
        for t in range(100, delay + duration + 1, 100):
            self.run(0x33d790, [100, t, 0], ecx=DRUG)
            if t <= delay:
                assert all((self.f(p + 0xa0) == 0.0 for p in self.pools.values())), 'healed before activation'
            timeline.append({name: self.f(p + 0xa0) for name, p in self.pools.items()})
        return (result, timeline)

def table_config(row):
    influences = []
    for part in MAXIMUM:
        column = 'limbs' if part in ('left_hand', 'right_hand', 'left_leg', 'right_leg') else part
        if row[column]:
            influences.append({'body_part': part, 'amount': float(row[column])})
    tree = {'parameters': {'activity_time_skill_influence': 1.0}, 'data': {'activation_delay_sec': float(row['use_s']), 'activity_time_sec': float(row['active_s']), 'influences': dict(enumerate(influences))}}
    config = bytearray(24)
    config[:24] = ce.value_record(config, 0, '', tree)
    if not influences:
        at = ce.find(config, 'data/influences')
        data, name, crc, _, count = ce.rec(config, at)
        ce.REC.pack_into(config, at, data, name, crc, ce.T_INDEXED, count)
    return bytes(config)

def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('exe', type=Path)
    ap.add_argument('--configs', type=Path, help='also check extracted final gameplay/items files')
    args = ap.parse_args()
    exe = args.exe.read_bytes()
    rows = ce.read_stats(str(HERE.parent / 'scripts/content-edit/medicines.tsv'))[2]
    # Design expectations are independent of TSV values and card calculations.
    full = dict(MAXIMUM, infection=0.0)
    expected = {'medkit': dict(full, pain=0.4), 'science_medkit': dict(full, pain=0.5, infection=1.0), 'military_medkit': dict(full, body=0.6, head=0.24, pain=0.5), 'blackmarket_medkit': dict.fromkeys(('body', 'pain'), 1.0) | {'head': 0.4}, 'bandages': dict(full, body=0.35, head=0.14, pain=0.35), 'antitoxin': {'infection': 0.5}, 'painkiller': {}}
    assert set(rows) == set(expected), 'audit every medicine'
    for name, row in rows.items():
        configs = [('table', table_config(row))]
        if args.configs:
            folder = 'drugs' if name in ('antitoxin', 'painkiller') else 'medicines'
            configs.append(('packed', (args.configs / f'gameplay/items/{folder}/{name}.options').read_bytes()))
        for source, config in configs:
            game = Game(exe, config)
            rates, timeline = game.check()
            final = timeline[-1]
            for part in MAXIMUM:
                assert math.isclose(final.get(part, 0), expected[name].get(part, 0), abs_tol=2e-06), (name, source, part, final.get(part, 0), expected[name].get(part, 0))
            active = int(float(row['active_s']) * 1000)
            delay = int(float(row['use_s']) * 1000)
            midpoint = timeline[(delay + active // 2) // 100 - 1]
            for part, rate in rates.items():
                assert math.isclose(midpoint[part], min(MAXIMUM[part], rate * active / 2000), abs_tol=2e-06), (name, source, 'partial course', part)
            print(f'PASS {name} ({source}): native course totals {final}')
        # Duration changes the rate, not the total course dose.
        longer = dict(row, active_s=str(float(row['active_s']) * 2))
        _, timeline = Game(exe, table_config(longer)).check()
        for part in MAXIMUM:
            assert math.isclose(timeline[-1].get(part, 0), expected[name].get(part, 0), abs_tol=3e-06), (name, 'duration invariance', part)
    print('All seven medicines: native constructor, delay, partial/full course, pool caps and doubled-duration totals passed.')
if __name__ == '__main__':
    main()
