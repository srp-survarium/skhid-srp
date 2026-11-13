#!/usr/bin/env python3
"""Compare patch 115's actual native entries with stock, including absent ids.

    python check_deserialize_guards_026g.py ORIGINAL_EXE

The generated hooks run at four stack alignments. Valid reads must preserve all
registers, flags, XMM state and stack results. Invalid ids never dereference past
an anomaly/artefact vector or call a null FSM state.
"""
import argparse
import struct
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import *
import patch_026g as base
from check_world_check_026g import Run, WORLD, STACK, STUBS, FRAME

REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX,
        UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP,
        UC_X86_REG_EFLAGS, UC_X86_REG_XMM0, UC_X86_REG_XMM7)
FSM, ANOMALY, ARTEFACT = 0x38CCF0, 0x31B0FF, 0x2FE65E
FSM_END, ANOMALY_END, ARTEFACT_END = 0x38CD37, 0x31B108, 0x2FE663
# get_death_animations(set, out, index): both death lists indexed by index.
DEATH, DEATH_RETURN = 0x338280, STUBS + 0x200
# A slot past a list: a pointer to nothing, which the stock takes a reference
# through (lock xadd [slot+0DCh]) and faults on.
PAST = 0x7F000000
# Each in-list slot names a resource of its own, its reference count at +0DCh.
REFS = 100

class GuardRun(Run):
    def __init__(self, data):
        super().__init__(data)
        for at in (FSM_END, ANOMALY_END, ARTEFACT_END, DEATH_RETURN):
            self.emu.hook_add(UC_HOOK_CODE, self.stop, begin=at, end=at)
        self.emu.hook_add(UC_HOOK_CODE, self.state_reader, begin=STUBS + 0x90, end=STUBS + 0x90)

    def state_reader(self, emu, address, size, _):
        sp = emu.reg_read(UC_X86_REG_ESP)
        self.fsm_calls.append((emu.reg_read(UC_X86_REG_ECX),
                               tuple(self.read(sp + 4*i) for i in (1,2,3))))
        emu.reg_write(UC_X86_REG_EAX, 0x1234)
        emu.reg_write(UC_X86_REG_ESP, sp + 16)
        emu.reg_write(UC_X86_REG_EIP, self.read(sp))

    def guard(self, kind, index, align, count=3):
        self.emu.mem_write(WORLD, bytes(0x20000))
        self.emu.mem_write(STACK, bytes(0x10000))
        self.fsm_calls = []
        obj, table, reader = WORLD, WORLD + 0x1a000, WORLD + 0x1b000
        regs = {r: 0x11111111*(i+1) for i,r in enumerate(REGS[:7])}
        sp = FRAME - 0x100 + align
        self.emu.reg_write(UC_X86_REG_EFLAGS, 0x202)
        self.emu.reg_write(UC_X86_REG_XMM0, 0x123456789abcdef)
        self.emu.reg_write(UC_X86_REG_XMM7, 0xfedcba987654321)
        for n in range(count):
            node = table + 0x100 + n*0x20
            self.word(table + n*4, node)
            self.word(node, table + 0x500)
            self.word(node+4, node+0x20 if n+1<count else 0)
        self.word(table+0x500+0x14, STUBS+0x90)
        if kind == FSM:
            self.word(obj+8, table+0x100 if count else 0)
            self.word(reader+4, reader+0x20)
            self.emu.mem_write(reader+0x20, bytes([index]))
            self.word(sp+4, 0x11)
            self.word(sp+8, 0x22)
            regs.update({UC_X86_REG_ESI: obj, UC_X86_REG_EDI: reader})
        elif kind == ANOMALY:
            self.word(obj+0x68, table)
            self.word(obj+0x6c, table+count*4)
            regs.update({UC_X86_REG_EDI: obj, UC_X86_REG_ECX: index})
        else:
            self.word(obj+0x18940, table)
            self.word(obj+0x18944, table+count*4)
            regs.update({UC_X86_REG_ECX: obj, UC_X86_REG_EAX: index, UC_X86_REG_EDX: table})
        self.start(kind, regs, sp)
        return self.stopped, tuple(self.emu.reg_read(r) for r in REGS), self.fsm_calls

    def death(self, index, align, count=2):
        """get_death_animations from a call with `index`, the set's two lists
        `count` long, each slot a resource of its own and every slot past them
        PAST: the result is the out pair and every resource's count, so which
        slot was read shows."""
        self.emu.mem_write(WORLD, bytes(0x20000))
        self.emu.mem_write(STACK, bytes(0x10000))
        self.fsm_calls = []
        sets, death, hud, out = WORLD, WORLD + 0x1a000, WORLD + 0x1b000, WORLD + 0x1c000
        resources = [WORLD + 0x1d000 + 0x100*(t*4 + n) for t in range(2) for n in range(count)]
        for t, table in enumerate((death, hud)):
            for n in range(count):
                self.word(resources[t*count + n] + 0xDC, REFS)
                self.word(table + n*4, resources[t*count + n])
            for n in range(count, 256):
                self.word(table + n*4, PAST)
        self.word(sets + 0x590, death)
        self.word(sets + 0x594, count)
        self.word(sets + 0xA20, hud)
        self.word(sets + 0xA24, count)
        regs = {r: 0x11111111*(i+1) for i,r in enumerate(REGS[:7])}
        sp = FRAME - 0x100 + align
        for n, value in enumerate((DEATH_RETURN, sets, out, index)):
            self.word(sp + 4*n, value)
        self.emu.reg_write(UC_X86_REG_EFLAGS, 0x202)
        self.emu.reg_write(UC_X86_REG_XMM0, 0x123456789abcdef)
        self.emu.reg_write(UC_X86_REG_XMM7, 0xfedcba987654321)
        try:
            self.start(DEATH, regs, sp)
        except RuntimeError:
            return None
        counts = tuple(self.read(r + 0xDC) for r in resources)
        return self.stopped, tuple(self.emu.reg_read(r) for r in REGS), self.read(out), self.read(out+4), counts


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('exe')
    args = parser.parse_args()
    original = open(args.exe,'rb').read()
    built, _ = base.build(original, 'section', open(base.default_bin('section'),'rb').read(),
                          None, open(base.late_join_bin(),'rb').read())
    stock, ours = GuardRun(original), GuardRun(bytes(built))
    checks = 0
    for align in (0,4,8,12):
        for kind, end in ((FSM,FSM_END),(ANOMALY,ANOMALY_END),(ARTEFACT,ARTEFACT_END)):
            for index in (0,1,2):
                expected = stock.guard(kind,index,align)
                actual = ours.guard(kind,index,align)
                assert actual == expected, (hex(kind), index, align, actual, expected)
                checks += 1
            for count,index in ((3,3),(3,254),(3,255),(0,0)):
                at, regs, calls = ours.guard(kind,index,align,count)
                assert at == end, (hex(kind),index,hex(at))
                if kind == FSM: assert not calls
                elif kind == ANOMALY: assert regs[REGS.index(UC_X86_REG_EBP)] == 0
                else: assert regs[REGS.index(UC_X86_REG_EAX)] == 0
                assert regs[REGS.index(UC_X86_REG_ESP)] == FRAME-0x100+align
                assert regs[REGS.index(UC_X86_REG_XMM0)] == 0x123456789abcdef
                assert regs[REGS.index(UC_X86_REG_XMM7)] == 0xfedcba987654321
                checks += 1
        for count in (1, 2):
            for index in range(count):
                expected = stock.death(index, align, count)
                assert expected is not None and expected[0] == DEATH_RETURN, (index, count, expected)
                assert expected[-1] != (REFS,)*(2*count), expected  # a reference was taken
                assert ours.death(index, align, count) == expected, (index, count, align)
                checks += 1
            for index in (count, count + 1, 255):
                assert stock.death(index, align, count) is None, (index, count)  # the live fault
                # Past the lists: what the game does with the first death animation.
                assert ours.death(index, align, count) == stock.death(0, align, count), (index, count, align)
                checks += 1
    print(f'{checks} native guard cases passed (stock equivalence and invalid indices)')

if __name__ == '__main__':
    main()
