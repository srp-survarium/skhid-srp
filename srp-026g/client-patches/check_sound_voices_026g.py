#!/usr/bin/env python3
"""Run patch 96 (the Rust patch `sound_voices`, and SOUND_POOLS) in the
default build on made-up sound scenes.

    python check_sound_voices_026g.py <original survarium.exe>

Requires Python `unicorn`. The exe's sections are loaded as they are, and
calculate_hdr_audio_frame's voice loop runs from 0x5CAB0, the instruction
after its call of create_sound_voice (0x59E70), until 0x5CADC, where the
loop goes on to the next propagator, with EAX the call's answer and EBP a
frame whose second argument is the scene's propagators. create_sound_voice
and detach_voice (0x5CB80) are a model of the world's voice pool: create
takes a free voice (0 when there is none) and checks its four arguments
(world in EAX, then scene, propagator, emitter: the asking propagator's
proxy's), detach frees the propagator's voice and clears its +4. Each case
must leave every propagator's voice and +48h as expected, the asking one's
voice in its +4 and the voice's +24h set to 1 (the game's own store after
the call), ESI/EDI/EBX/EBP as they were and the stack as the call left it.
The original, run the same way, never takes a voice. Also: test_sound_pools
of the self-test (the two pool sizes of game_world::load), and the patch's
Rust tests (`cargo test` is the developer's; here only the build).
"""
import os
import struct
import sys

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ESP, UC_X86_REG_EBP, \
    UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EIP

if len(sys.argv) > 1:
    os.environ.setdefault("SRP_026G_EXE", sys.argv[1])
import patch_026g as base

SITE, NEXT = 0x5CAB0, 0x5CADC
CREATE, DETACH = 0x59E70, 0x5CB80
OBJ, STACK = 0x20000000, 0x21000000
WORLD, SCENE, USER, VOICES = OBJ + 0x100, OBJ + 0x400, OBJ + 0x800, OBJ + 0x1000
PROXIES, PROPS, LIST, SLOTS = OBJ + 0x2000, OBJ + 0x4000, OBJ + 0x8000, OBJ + 0x8100
EMITTER = 0x0BADF00D
GAME = {UC_X86_REG_EBX: 0x7D0, UC_X86_REG_EDI: 0x44444444}
VOICE, PROXY, EMIT, MASTER, LEVEL, HOLD, GAIN = 0x4, 0x8, 0xC, 0x14, 0x44, 0x48, 0x4C
HELD = 0x1234

# (what, sounds [(gain, level, voiced, master index)], the asking one's
# index, free voices, expected: voices taken back by index, the asking one
# voiced)
CASES = [
    ("a free voice: the world's answer stands", [(1.0, 1.0, False, None), (0.5, 0.1, True, None)], 0, 1, [], True),
    ("all taken, a tail at a tenth: it gives its voice",
     [(1.0, 0.9, False, None), (1.0, 0.05, True, None), (1.0, 0.8, True, None)], 0, 0, [1], True),
    ("all taken, the quietest of three gives it",
     [(0.9, 1.0, False, None), (0.4, 0.4, True, None), (0.3, 0.2, True, None), (0.8, 0.9, True, None)], 0, 0, [2], True),
    ("all taken, none less than half as loud: it waits",
     [(0.5, 0.5, False, None), (0.5, 0.3, True, None), (0.9, 0.9, True, None)], 0, 0, [], False),
    ("all taken, as loud as the quietest: it waits", [(0.3, 0.3, False, None), (0.3, 0.3, True, None)], 0, 0, [], False),
    ("a quiet sound never takes a loud one's voice", [(0.05, 0.1, False, None), (1.0, 1.0, True, None)], 0, 0, [], False),
    ("the gain of its master counts (amplitude 0x5C1C0)",
     [(0.0, 1.0, False, 2), (0.6, 0.2, True, None), (1.0, 1.0, False, None)], 0, 0, [1], True),
    ("a voiced sound's master counts too",
     [(1.0, 1.0, False, None), (1.0, 1.0, True, 2), (0.01, 1.0, False, None), (0.9, 0.9, True, None)], 0, 0, [1], True),
    ("nothing voiced: nothing to take", [(1.0, 1.0, False, None), (0.2, 0.2, False, None)], 0, 0, [], False),
]


class Run:
    def __init__(self, data):
        self.emu = emu = Uc(UC_ARCH_X86, UC_MODE_32)
        for _, rva, vsize, raw, rsize, _ in base.Headers(data).sections(data):
            start = base.IMAGE_BASE + rva
            emu.mem_map(start, (max(vsize, rsize) + 0xFFF) & ~0xFFF, UC_PROT_ALL)
            emu.mem_write(start, bytes(data[raw:raw + min(rsize, vsize or rsize)]))
        emu.mem_map(OBJ, 0x10000, UC_PROT_ALL)
        emu.mem_map(STACK, 0x10000, UC_PROT_ALL)
        emu.hook_add(UC_HOOK_CODE, self.model, begin=CREATE, end=CREATE)
        emu.hook_add(UC_HOOK_CODE, self.model, begin=DETACH, end=DETACH)

    def word(self, at, value):
        self.emu.mem_write(at, struct.pack("<I", value & 0xFFFFFFFF))

    def float(self, at, value):
        self.emu.mem_write(at, struct.pack("<f", value))

    def read(self, at):
        return struct.unpack("<I", self.emu.mem_read(at, 4))[0]

    def ret(self, value, pop):
        emu = self.emu
        sp = emu.reg_read(UC_X86_REG_ESP)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))
        emu.reg_write(UC_X86_REG_ESP, sp + 4 + pop)
        emu.reg_write(UC_X86_REG_EAX, value)

    def model(self, emu, address, _size, _data):
        sp = emu.reg_read(UC_X86_REG_ESP)
        if address == CREATE:
            scene, prop, emitter = (self.read(sp + 4 * i) for i in (1, 2, 3))
            ok = (emu.reg_read(UC_X86_REG_EAX) == WORLD and scene == SCENE and prop == self.asking
                  and emitter == EMITTER)
            self.calls.append(("create", ok))
            voice = self.free.pop() if self.free and ok else 0
            self.ret(voice, 12)
        else:
            prop = self.read(sp + 4)
            voice = self.read(prop + VOICE)
            self.calls.append(("detach", prop))
            if voice:
                self.free.append(voice)
                self.word(prop + VOICE, 0)
            self.ret(0, 4)

    def case(self, sounds, asking, free):
        emu = self.emu
        emu.mem_write(OBJ, bytes(0x10000))
        self.word(USER + 0x118, WORLD)
        self.calls = []
        self.free = [VOICES + 0x40 * (16 + i) for i in range(free)]
        props = [PROPS + 0x80 * i for i in range(len(sounds))]
        self.asking = props[asking]
        for i, (gain, level, voiced, master) in enumerate(sounds):
            p, proxy = props[i], PROXIES + 0x80 * i
            self.word(proxy + 0x64, USER)
            self.word(proxy + 0x68, SCENE)
            self.word(p + VOICE, VOICES + 0x40 * i if voiced else 0)
            self.word(p + PROXY, proxy)
            self.word(p + EMIT, EMITTER)
            self.word(p + MASTER, props[master] if master is not None else 0)
            self.float(p + LEVEL, level)
            self.word(p + HOLD, HELD)
            self.float(p + GAIN, gain)
            self.word(SLOTS + 4 * i, p)
        self.word(LIST, SLOTS)
        self.word(LIST + 4, SLOTS + 4 * len(sounds))
        frame = STACK + 0x8000
        self.word(frame + 0xC, LIST)
        # the loop's own locals: [esp+0Ch] the vector's end, EDI its iterator
        top = STACK + 0x7000
        self.word(top + 0xC, SLOTS + 4 * len(sounds))
        regs = dict(GAME)
        regs[UC_X86_REG_EDI] = SLOTS + 4 * asking
        for reg, value in regs.items():
            emu.reg_write(reg, value)
        emu.reg_write(UC_X86_REG_ESI, self.asking)
        emu.reg_write(UC_X86_REG_EBP, frame)
        emu.reg_write(UC_X86_REG_ESP, top)
        answer = self.free.pop() if self.free else 0          # create_sound_voice's answer at the site
        emu.reg_write(UC_X86_REG_EAX, answer)
        emu.emu_start(SITE, NEXT, count=500000)
        voices = [self.read(p + VOICE) for p in props]
        holds = [self.read(p + HOLD) for p in props]
        kept = all(emu.reg_read(r) == v for r, v in regs.items()) and emu.reg_read(UC_X86_REG_ESI) == self.asking \
            and emu.reg_read(UC_X86_REG_EBP) == frame and emu.reg_read(UC_X86_REG_ESP) == top
        flagged = voices[asking] == 0 or emu.mem_read(voices[asking] + 0x24, 1)[0] == 1
        return voices, holds, self.calls, kept and flagged


def check(run, sounds, asking, free, taken, voiced):
    before = [VOICES + 0x40 * i if s[2] else 0 for i, s in enumerate(sounds)]
    voices, holds, calls, kept = run.case(sounds, asking, free)
    ok = kept and (voices[asking] != 0) == voiced and all(c[1] for c in calls if c[0] == "create")
    for i in range(len(sounds)):
        if i == asking:
            continue
        gone = i in taken
        ok &= (voices[i] == 0) == gone if before[i] else voices[i] == 0
        ok &= holds[i] == (2000 if gone else HELD)
    ok &= [c[1] for c in calls if c[0] == "detach"] == [PROPS + 0x80 * i for i in taken]
    return ok, voices, calls


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    original = open(sys.argv[1], "rb").read()
    code = open(base.default_bin("section"), "rb").read()
    late = open(base.late_join_bin(), "rb").read()
    built, _ = base.build(original, "section", code, None, late)
    before, _ = base.build(original, "section", code, None, None)
    import selftest_026g as st
    st.test_sound_pools(original, st.Image(bytes(built)), st.Image(bytes(before)))
    hooks = [h for h in base.RUST_HOOKS if h[2].startswith("sound_voices_")]
    if [h[0] for h in hooks] != [SITE]:
        sys.exit(f"the build hooks {[hex(h[0]) for h in hooks]}, not {SITE:#x} alone")
    stock, patched = Run(original), Run(bytes(built))
    failed = 0
    for what, sounds, asking, free, taken, voiced in CASES:
        ok, voices, calls = check(patched, sounds, asking, free, taken, voiced)
        same, _, stock_calls = check(stock, sounds, asking, free, [], free > 0)
        ok &= same and not stock_calls
        failed += not ok
        print(f"  {'ok' if ok else 'FAIL'}  {what}: voices {[hex(v) for v in voices]}, calls {calls}"
              + ("" if same else " (the original's run differs from the original's rule)"))
    if failed or st.failures:
        sys.exit(f"{failed} case(s) failed, {len(st.failures)} self-test check(s) failed")
    print("all cases passed")


if __name__ == "__main__":
    main()
