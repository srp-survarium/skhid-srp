"""wall_slide_check.py: run bullet_character_controller's get_wall_slide_vector (0x7AC50, 0.26g) with Unicorn on made-up moves and wall
normals and compare it with the rule read from its listing:

    slide = unit(m - n (m.n) + K n) * |m| * f(c),   c = |m^ . n|,
    f = 1 for c < D, else (1 - c) / (1 - D)

with D = cc_wall_full_slide_dot (0xA7AB14) and K = cc_wall_slide_bouncing_factor (0xA7D5D8), checked at the exe's defaults
(0.9, 0.001) and at the values `startup.cfg` of the game's resources.db sets (0.50, 0.080). Needs the unicorn package and the
0.26g exe (exes.py). Run: python wall_slide_check.py (exits 1 on a mismatch)."""
import math
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
# This folder's dis.py would shadow the standard library's dis for what unicorn imports: out of the path until unicorn is in.
sys.path[:] = [p for p in sys.path if os.path.abspath(p or '.') != HERE]
from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_EDI, UC_X86_REG_ESI, UC_X86_REG_ESP

sys.path.append(HERE)
from exes import Pe, exe_path

FUNCTION, FULL_SLIDE_DOT, BOUNCE = 0x7AC50, 0xA7AB14, 0xA7D5D8
DATA, STACK, RET = 0x41000000, 0x41100000, 0x41200000

pe = Pe(exe_path("026g"))
emu = Uc(UC_ARCH_X86, UC_MODE_32)
for name, start, vsize, raw, rsize in pe.sections:
    if name not in (".text", ".rdata", ".data"):
        continue
    size = min(rsize, vsize)
    first, last = start & ~0xFFF, (start + size + 0xFFF) & ~0xFFF
    emu.mem_map(first, last - first, UC_PROT_ALL)
    emu.mem_write(start, bytes(pe.data[raw:raw + size]))
emu.mem_map(DATA, 0x10000, UC_PROT_ALL)
emu.mem_map(STACK - 0x10000, 0x20000, UC_PROT_ALL)
emu.mem_map(RET, 0x1000, UC_PROT_ALL)
emu.mem_write(RET, b"\xf4")


def slide(m, n, dot, bounce):
    emu.mem_write(FULL_SLIDE_DOT, struct.pack("<f", dot))
    emu.mem_write(BOUNCE, struct.pack("<f", bounce))
    emu.mem_write(DATA, struct.pack("<3f", *m))           # ESI: the move
    emu.mem_write(DATA + 0x20, struct.pack("<3f", *n))    # EDI: the wall's normal
    emu.mem_write(DATA + 0x40, b"\0" * 16)                # the out argument
    sp = STACK - 4
    emu.mem_write(sp, struct.pack("<I", DATA + 0x40))
    sp -= 4
    emu.mem_write(sp, struct.pack("<I", RET))
    emu.reg_write(UC_X86_REG_ESP, sp)
    emu.reg_write(UC_X86_REG_ESI, DATA)
    emu.reg_write(UC_X86_REG_EDI, DATA + 0x20)
    emu.reg_write(UC_X86_REG_EBX, 0)
    emu.reg_write(UC_X86_REG_EBP, 0)
    emu.emu_start(FUNCTION, RET, timeout=2_000_000, count=5000)
    return struct.unpack("<3f", emu.mem_read(DATA + 0x40, 12))


def rule(m, n, dot, bounce):
    ml = math.sqrt(sum(x * x for x in m))
    d = sum(a / ml * b for a, b in zip(m, n))
    t = [a / ml - b * d + b * bounce for a, b in zip(m, n)]
    tl = math.sqrt(sum(x * x for x in t))
    c = abs(d)
    f = 1.0 if c < dot else (1 - c) / (1 - dot)
    return [x / tl * ml * f for x in t]


bad = 0
for dot, bounce in ((0.9, 0.001), (0.5, 0.08)):
    for deg in (0, 10, 20, 30, 45, 60, 80):
        a = math.radians(deg)
        m = (0.15 * math.cos(a), 0.0, 0.15 * math.sin(a))
        n = (-1.0, 0.0, 0.0)
        got, want = slide(m, n, dot, bounce), rule(m, n, dot, bounce)
        ok = max(abs(g - w) for g, w in zip(got, want)) < 2e-6
        bad += not ok
        print(f"D {dot} K {bounce}: {deg:2d} deg off the normal: client {tuple(round(x, 4) for x in got)}, rule {tuple(round(x, 4) for x in want)} {'ok' if ok else 'MISMATCH'}")
sys.exit(1 if bad else 0)
