#!/usr/bin/env python3
"""Run patch 221, the sun's cached cascades drawn again whole after a camera cut
(rust/patches/src/sun_shadow_cut.rs, vostok/sun_cascades.rs;
../docs/notes/renderer-026g.md 4.5), in the default build beside the original exe.

    python check_sun_shadow_cut_026g.py <original survarium.exe>

Requires Python `unicorn`. stage_shadow_direct::execute's step at the site
`shadow_direct.cascades_placed` (0x172FBF, cmp byte [esp+17h],0, after the
cascades are placed) runs to the next instruction (0x172FC4) in both exes, frame
after frame in one machine, over a made-up stage (its renderer context at +4,
the cache's reset byte +2Ch and its frame count +2Dh) and context (the eye at
+54A4h); every import a failure, GetCurrentThreadId 1.

- The original leaves the stage as it is in every frame.
- The build leaves it so in the first frame, in a walk (15 cm a frame) and
  standing still, and when the stage rebuilds whole already (its start, a sun
  change); after a cut (the eye 42 m from the frame before's, from the gym to
  School (v0.69)'s terrain_06; 3 m down) it sets the reset byte to 1 and the
  count to 101, so that the stage flushes every cascade that frame and caches
  again from the next (0x1730C3..0x1730ED).
- Everything else is the original's in the build: the flags the cmp sets for
  the je after it (r_use_csm_scrolling on and off), the registers and the stack.
"""
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_PROT_ALL
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDI,
                               UC_X86_REG_EDX, UC_X86_REG_EFLAGS, UC_X86_REG_EIP, UC_X86_REG_ESI, UC_X86_REG_ESP)

import patch_026g as base

SITE, AFTER = 0x172FBF, 0x172FC4
THREAD_SLOT = 0x870118
ENGINE_TSV = "engine_026g.tsv"
DATA, STACK, STUBS = 0x30000000, 0x21000000, 0x31000000
FORBIDDEN, THREAD = STUBS, STUBS + 0x10
STAGE, CONTEXT = DATA, DATA + 0x1000
SP = STACK + 0x8000
GYM, T06 = (-11.88, 17.92, 123.48), (-54.2, 18.25, 110.86)
# (what, the eye, the reset byte and its count before, r_use_csm_scrolling's copy at [esp+17h],
#  whether the build sets the reset for this frame)
FRAMES = (
    ("the first frame", GYM, (1, 40), 1, False),
    ("standing still, the cache kept", GYM, (0, 102), 1, False),
    ("a walk of 15 cm", (GYM[0] + 0.15, GYM[1], GYM[2]), (0, 103), 1, False),
    ("a cut of 42 m (the gym to terrain_06)", T06, (0, 104), 1, True),
    ("the frame after the cut", T06, (0, 102), 1, False),
    ("a cut while the stage rebuilds whole already", GYM, (1, 7), 1, False),
    ("a cut of 3 m down", (GYM[0], GYM[1] - 3.0, GYM[2]), (0, 120), 1, True),
    ("a walk with r_use_csm_scrolling off", (GYM[0] + 0.15, GYM[1] - 3.0, GYM[2]), (0, 121), 0, False),
)
KEPT = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI,
        UC_X86_REG_EBP)
FLAGS = 0x8D5                                                  # CF PF AF ZF SF OF


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        for at, size in ((DATA, 0x10000), (STACK, 0x10000), (STUBS, 0x1000)):
            emu.mem_map(at, size, UC_PROT_ALL)
        emu.mem_write(THREAD, bytes.fromhex("b801000000c3"))     # GetCurrentThreadId: mov eax,1 / ret
        self.forbidden = []
        for line in open(ENGINE_TSV, encoding="utf-8"):
            cells = line.rstrip("\n").split("\t")
            if len(cells) > 4 and cells[4] == "import":
                self.word(int(cells[1], 16), FORBIDDEN)
        self.word(THREAD_SLOT, THREAD)
        emu.hook_add(UC_HOOK_CODE, self.stub, begin=FORBIDDEN, end=FORBIDDEN)
        self.word(STAGE + 4, CONTEXT)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def stub(self, emu, address, size, _):
        self.forbidden.append(hex(struct.unpack("<I", bytes(emu.mem_read(emu.reg_read(UC_X86_REG_ESP), 4)))[0]))
        emu.emu_stop()

    def frame(self, eye, reset, scrolling):
        emu = self.emu
        emu.mem_write(CONTEXT + 0x54A4, struct.pack("<3f", *eye))
        emu.mem_write(STAGE + 0x2C, bytes(reset))
        emu.mem_write(SP, bytes(range(0x40)))                    # execute's frame, made up
        emu.mem_write(SP + 0x17, bytes([scrolling]))
        regs = {UC_X86_REG_EAX: 4, UC_X86_REG_EBX: 4, UC_X86_REG_ECX: 0x11111111, UC_X86_REG_EDX: 0x22222222,
                UC_X86_REG_ESI: 4, UC_X86_REG_EDI: 0x33333333, UC_X86_REG_EBP: STAGE}
        for reg, value in regs.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESP, SP)
        emu.reg_write(UC_X86_REG_EFLAGS, 0x202)
        emu.emu_start(SITE, AFTER, count=200_000)
        assert not self.forbidden, f"an import called: {self.forbidden}"
        assert emu.reg_read(UC_X86_REG_EIP) == AFTER, hex(emu.reg_read(UC_X86_REG_EIP))
        return {
            "stage": tuple(bytes(emu.mem_read(STAGE + 0x2C, 2))),
            "registers": {reg: emu.reg_read(reg) for reg in KEPT},
            "flags": emu.reg_read(UC_X86_REG_EFLAGS) & FLAGS,
            "stack pointer": emu.reg_read(UC_X86_REG_ESP) - SP,
            "frame": bytes(emu.mem_read(SP, 0x40)),
        }


def main():
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    built, _ = base.build(original, "section", code, None, open(base.late_join_bin(), "rb").read())
    failed = 0

    def case(what, ok, detail=""):
        nonlocal failed
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {what}" + (f"\n      {detail}" if not ok and detail else ""))

    case("the site is hooked in the build", bytes(built[SITE - 0x11000 + 0x400:][:1]) == b"\xe9"
         and bytes(original[SITE - 0x11000 + 0x400:][:5]) == bytes.fromhex("807c241700"))
    stock_run, build_run = Run(original), Run(bytes(built))
    for what, eye, reset, scrolling, cut in FRAMES:
        was, now = stock_run.frame(eye, reset, scrolling), build_run.frame(eye, reset, scrolling)
        case(f"{what}: the original leaves the stage {reset}", was["stage"] == reset, was["stage"])
        want = (1, 101) if cut else reset
        case(f"{what}: the build leaves {want}", now["stage"] == want, now["stage"])
        for key in ("registers", "flags", "stack pointer", "frame"):
            case(f"{what}: the {key} as the original's", was[key] == now[key], f"{was[key]!r} / {now[key]!r}")
    if failed:
        raise SystemExit(f"patch 221: {failed} failed")
    print(f"patch 221: {len(FRAMES)} frames at shadow_direct.cascades_placed: a cut of the eye (over 2 m from the "
          f"frame before's) sets the stage's own reset for that frame alone, nothing else does; flags, registers "
          f"and stack the original's")


if __name__ == "__main__":
    main()
