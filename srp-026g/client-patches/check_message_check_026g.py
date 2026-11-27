#!/usr/bin/env python3
"""Run patch 114's generated dispatch hook, original dispatch beside it.

    python check_message_check_026g.py ORIGINAL_EXE

Valid payloads reach their unchanged table handlers with the same machine state.
Malformed payloads consume the reader, close once with reason 2 and leave through
the dispatch epilogue. The late-join profile's deeper check remains its own.
"""
import argparse
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import *
import patch_026g as base
from check_world_check_026g import Run, CLIENT, READER, WORLD, FRAME

SITE, TABLE, END = 0x27A31F, 0x27A738, 0x27A72C
REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX,
        UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP,
        UC_X86_REG_EFLAGS, UC_X86_REG_XMM0, UC_X86_REG_XMM7)
KEPT = (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP)

class MessageRun(Run):
    def __init__(self, data):
        super().__init__(data)
        self.targets = [self.read(TABLE+4*i) for i in range(20)]
        for at in set(self.targets + [END]):
            self.emu.hook_add(UC_HOOK_CODE,self.stop,begin=at,end=at)

    def message(self, ident, payload, align=0, invalid_cursor=False):
        self.emu.mem_write(WORLD, bytes([ident])+payload+b'\x00')
        for offset,value in ((0,WORLD),(4,WORLD+1),(8,len(payload)+1)):
            self.word(READER+offset,value)
        if invalid_cursor: self.word(READER+4,WORLD+len(payload)+2)
        self.word(FRAME+12,ident)
        regs = {UC_X86_REG_EAX: ident, UC_X86_REG_EBX: READER,
                UC_X86_REG_ECX: ident, UC_X86_REG_EDX: 0x12345678,
                UC_X86_REG_ESI: CLIENT, UC_X86_REG_EDI: 0, UC_X86_REG_EBP: FRAME}
        self.emu.reg_write(UC_X86_REG_EFLAGS,0x202)
        self.emu.reg_write(UC_X86_REG_XMM0,0x123456789abcdef)
        self.emu.reg_write(UC_X86_REG_XMM7,0xfedcba987654321)
        self.start(SITE,regs,FRAME-0x100+align)
        return self.stopped, tuple(self.emu.reg_read(r) for r in REGS), self.read(READER+4)

def valid():
    for ident,n in ((0xe3,4),(0xe5,7),(0xe6,4),(0xe7,2),(0xe9,2),(0xea,2),
                    (0xeb,10),(0xec,11),(0xef,2),(0xf0,4),(0xf1,1),(0xf2,2),
                    (0xf3,3),(0xf4,2),(0xf5,1),(0xf6,1)):
        yield ident,bytes(n)
    yield 0xe4,b'\x00\x01\x00\x00\x00\x00' # one stored byte plus LZMA2 end
    yield 0xe4,b'\xffLJ\x01'+bytes(4)+b'\x01\x00\x00\x00\x01\x00'
    yield 0xe8,bytes(10)  # raw envelope; world semantics belong to patch 87
    yield 0xe8,b'\x00\x01\x01\x00\x00\x00\x00'

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('exe');args=parser.parse_args()
    original=open(args.exe,'rb').read()
    built,_=base.build(original,'section',open(base.default_bin('section'),'rb').read(),None,
                       open(base.late_join_bin(),'rb').read())
    baseline=bytearray(built)
    for _,rva,vsize,offset,rsize,_ in base.Headers(original).sections(original):
        if base.IMAGE_BASE+rva <= SITE < base.IMAGE_BASE+rva+vsize:
            at=offset+SITE-base.IMAGE_BASE-rva; baseline[at:at+8]=original[at:at+8];break
    stock,ours=MessageRun(bytes(baseline)),MessageRun(bytes(built));checks=0
    for align in (0,4,8,12):
        for ident,payload in valid():
            expected=stock.message(ident,payload,align)
            actual=ours.message(ident,payload,align)
            assert actual==expected and not ours.calls and not ours.lines,(hex(ident),actual,expected,ours.calls)
            checks+=1
        bad=[(0xe6,b'\0'*3),(0xe9,b'\0\x14'),(0xeb,b'\0'*6+b'\0\0\0\x40'),
             (0xe4,b'\0\x01'),(0xe8,b'\0\x02'+bytes(8)),(0xe8,b'\0\x01\x01'),
             (0xe4,b'\xffLJ\x01'+bytes(10))]
        for ident,payload in bad:
            at,regs,cursor=ours.message(ident,payload,align)
            assert at==END and not ours.calls,(hex(ident),hex(at),ours.calls)
            assert cursor==WORLD+1+len(payload)
            assert len(ours.lines)==1 and b'message_check: message 0x000000%02x' % ident in ours.lines[0] \
                and b'bytes) discarded: ' in ours.lines[0],ours.lines
            assert tuple(regs[REGS.index(r)] for r in KEPT)==(READER,CLIENT,0,FRAME)
            assert regs[REGS.index(UC_X86_REG_ESP)]==FRAME-0x100+align
            assert regs[REGS.index(UC_X86_REG_XMM7)]==0xfedcba987654321
            checks+=1
        at,_,_=ours.message(0xe6,bytes(4),align,True)
        assert at==END and not ours.calls
        checks+=1
    print(f'{checks} native message cases passed')
if __name__=='__main__': main()
