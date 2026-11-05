#!/usr/bin/env python3
"""Run the patched native lobby weight callback and config hook in Unicorn.

Only profile total weight and GFx Invoke are doubled. Config lookup, selected
kit modifier evaluation, caller frames and both arena hooks execute natively.
"""
import math
import struct
import sys
from pathlib import Path
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import *
import patch_026g as P
from check_artefacts_026g import Game, config_bytes, STACK, STOP


def fbits(n): return struct.unpack('<I', struct.pack('<f', n))[0]


class WeightGame(Game):
    def __init__(self, image):
        super().__init__(image)
        self.menu, self.profile = self.block(0x3200), self.block(0x4700)
        self.game, self.dictionary = self.block(0x5400), self.block(0x200)
        self.resource, self.holder, self.movie = self.block(0x120), self.block(0x10), self.block(0x10)
        self.config_resource, self.config = self.block(0x120), self.block(0x1000)
        self.word(self.menu+0x9c,self.game)
        self.word(self.menu+0x6a0,self.profile)
        self.word(self.menu+0x690,self.resource)
        self.word(self.game+0x53a4,self.dictionary)
        self.word(self.resource+0x108,self.holder)
        self.word(self.holder+4,self.movie)
        self.word(self.config_resource+0x108,self.config)
        for kit in range(3):
            base = self.profile+0x58+kit*0x704
            self.emu.mem_write(base+0x40,b'\xff\xff\x00\x00'*23)
            for index in range(68): self.emu.mem_write(base+0x9c+0x120+20*index,b'\1')
        self.weight = 25
        self.descriptor = self.block(0x400)
        self.calls = []
        self.kit_calls = []
        self.emu.hook_add(UC_HOOK_CODE,self.capture,begin=0x511600,end=0x511600)
        self.emu.hook_add(UC_HOOK_CODE,self.total_weight,begin=0x2f76f0,end=0x2f76f0)
        self.emu.hook_add(UC_HOOK_CODE,self.item_descriptor,begin=0x1f0090,end=0x1f0090)
        self.emu.hook_add(UC_HOOK_CODE,self.item_parameter,begin=0x2f7470,end=0x2f7470)
        self.emu.hook_add(UC_HOOK_CODE,lambda u,a,s,d:u.emu_stop(),begin=0x221115,end=0x221115)

    def ret(self, pop, result=1):
        sp = self.emu.reg_read(UC_X86_REG_ESP)
        self.emu.reg_write(UC_X86_REG_EAX,result)
        self.emu.reg_write(UC_X86_REG_EIP,self.read(sp))
        self.emu.reg_write(UC_X86_REG_ESP,sp+4+pop)

    def total_weight(self,u,a,s,d):
        u.reg_write(UC_X86_REG_XMM0,fbits(self.weight))
        self.ret(0)

    def item_descriptor(self,u,a,s,d):
        self.ret(4,self.descriptor)

    def item_parameter(self,u,a,s,d):
        # The native item's unchecked switch cannot accept general player
        # modifier indices. This is the path the real equipped kit exposed.
        index=self.read(u.reg_read(UC_X86_REG_ESP)+4)
        assert index==5, ('invalid equipment parameter',index)

    def capture(self,u,a,s,d):
        sp=u.reg_read(UC_X86_REG_ESP)
        assert u.reg_read(UC_X86_REG_ECX)==self.movie
        method=bytes(u.mem_read(self.read(sp+4),64)).split(b'\0')[0]
        assert self.read(sp+8)==0
        args,count=self.read(sp+12),self.read(sp+16)
        values=[]
        for i in range(count):
            raw=bytes(u.mem_read(args+24*i,24))
            assert struct.unpack_from('<II',raw)==(0,5)
            values.append(struct.unpack_from('<d',raw,8)[0])
        if method==b'root.set_kit_modifiers':
            assert count==68,count
            self.kit_calls.append(values)
        else:
            assert method==b'root.update_player_items_weight',method
            # The kit's floats come first, once for each weight callback.
            assert len(self.kit_calls)==len(self.calls)+1
            self.calls.append(values)
        self.ret(16)

    def load_config(self, minimum=20, movement=-0.2, spending=1.5, maximum=30):
        self.word(self.menu+0x3118,fbits(maximum))
        data=config_bytes({'player':{'stamina_params':{
            'min_carried_weight':minimum,
            'max_carried_weight_movement_speed_modifier':movement,
            'max_carried_weight_spending_speed_modifier':spending}}},self.config)
        self.emu.mem_write(self.config,data)
        # Real inline hook, including the displaced load and exact continuation.
        regs={UC_X86_REG_EBX:self.menu,UC_X86_REG_ESI:self.config_resource,
              UC_X86_REG_EDI:0x12345678,UC_X86_REG_EBP:0x76543210,
              UC_X86_REG_ECX:0x87654321,UC_X86_REG_EDX:0x12344321}
        for r,v in regs.items(): self.emu.reg_write(r,v)
        self.emu.reg_write(UC_X86_REG_ESP,STACK-0x100)
        self.emu.reg_write(UC_X86_REG_XMM0,fbits(30))
        self.emu.reg_write(UC_X86_REG_EFLAGS,0x246)
        self.emu.emu_start(0x22110f,STOP,count=200000)
        assert self.emu.reg_read(UC_X86_REG_EIP)==0x221115
        assert self.emu.reg_read(UC_X86_REG_EAX)==self.config
        assert self.emu.reg_read(UC_X86_REG_ESP)==STACK-0x100
        assert self.emu.reg_read(UC_X86_REG_XMM0)==fbits(30)
        assert self.emu.reg_read(UC_X86_REG_EFLAGS)==0x246
        assert all(self.emu.reg_read(r)==v for r,v in regs.items())

    def kit(self, index, bonus=0, porter=0, heavy=0, runner=0, skills=0, bear=0, spending=0, spending_skill=0):
        self.emu.mem_write(self.profile+0x4632,bytes([index]))
        container=self.profile+0xf4+index*0x704
        for param,value in [(5,bonus),(26,porter),(32,heavy),(31,runner),(4,skills),(62,bear),(19,spending),(67,spending_skill)]:
            self.word(container+4*param,fbits(value))

    def show(self):
        kept={UC_X86_REG_EBX:0x12345678,UC_X86_REG_ESI:0x23456789,
              UC_X86_REG_EDI:0x34567890,UC_X86_REG_EBP:0x45678901}
        for r,v in kept.items():self.emu.reg_write(r,v)
        self.call(0x2221e0,eax=self.menu)
        assert self.emu.reg_read(UC_X86_REG_EIP)==STOP
        assert self.emu.reg_read(UC_X86_REG_ESP)==STACK-0x100
        assert all(self.emu.reg_read(r)==v for r,v in kept.items())
        # The selected kit's 68 floats, every one by its index, before the weight.
        kit=self.emu.mem_read(self.profile+0x4632,1)[0]
        floats=struct.unpack('<68f',bytes(self.emu.mem_read(self.profile+0xf4+kit*0x704,68*4)))
        assert len(self.kit_calls[-1])==68 and all(a==e for a,e in zip(self.kit_calls[-1],floats)),(kit,self.kit_calls[-1],floats)
        return self.calls[-1]

    def distinct_floats(self):
        """Every kit's every float its own value (kit, index), so the order is checked."""
        for kit in range(3):
            for index in range(68):
                self.word(self.profile+0xf4+kit*0x704+4*index,fbits(kit*10+index*0.125-4))


def main():
    original=Path(sys.argv[1]).read_bytes()
    image,_=P.build(original,'section',Path(P.default_bin('section')).read_bytes(),None,Path(P.late_join_bin()).read_bytes())
    import selftest_026g
    shapes=[]
    selftest_026g.test_call_shapes(original,image,lambda ok,text:shapes.append((ok,text)))
    assert all(ok for ok,_ in shapes), [text for ok,text in shapes if not ok]
    g=WeightGame(image)
    g.word(g.menu+0x3118,fbits(30))
    g.distinct_floats()
    g.kit(0)
    assert g.show()==[25,30], 'old callback until config is available'
    count=0
    for minimum,movement,spending,maximum in [(20,-0.2,1.5,30),(15.0,-0.3,2,35.0)]:
        g.load_config(minimum,movement,spending,maximum)
        for kit,bonus,porter,heavy,runner,skills,bear,sp,sps in [(0,0,0,0,0,0,0,0,0),(1,10,0,0,0,0.03,0,-0.05,0),(2,0,-0.25,0,0,0,1,0,-0.1),(0,0,0,1,0,0.05,1,0.02,-0.04),(1,0,0,0,1,0.01,0,0,0)]:
            g.kit(kit,bonus,porter,heavy,runner,skills,bear,sp,sps)
            for weight in [0,minimum-0.01,minimum,minimum+0.01,25,29.99,30,maximum+bonus-0.01,maximum+bonus,50]:
                g.weight=weight
                actual=g.show()
                q=min(1,max(0,(weight-minimum)/(maximum+bonus-minimum)))
                expected=[weight,maximum+bonus,0 if heavy else movement*(1+porter)*q,spending*q,2 if heavy else int(weight>=maximum+bonus and not runner),skills,int(bear>0),maximum,bonus,porter,sp,sps]
                assert all(math.isclose(a,e,abs_tol=1e-5) for a,e in zip(actual,expected)),(actual,expected)
                assert len(actual)==12
                count+=1
    # Equipped inventory must execute the real item's parameter path for
    # capacity, but never for the weight-specific player modifiers.
    g.load_config()
    slot=g.read(0x92655c)
    g.word(g.descriptor+0x260+5*4,fbits(5))
    for kit in range(3):
        g.word(g.profile+0x58+kit*0x704+0x40+slot*4,0)
        g.kit(kit,porter=-0.25)
        g.weight=27.5
        actual=g.show()
        expected=[27.5,35,-0.075,0.75,0,0,0,30,0,-0.25,0,0]
        assert len(actual)==12 and all(math.isclose(a,e,abs_tol=1e-5) for a,e in zip(actual,expected)),(actual,expected)
        g.word(g.profile+0x58+kit*0x704+0x40+slot*4,0xffff)
    g.load_config(minimum=30,maximum=30)
    g.kit(0)
    assert len(g.show())==2, 'invalid weight interval does not invent effects'
    g.load_config()
    # A different lobby must not borrow a previous lobby's config.
    replacement=g.block(0x3200);g.emu.mem_write(replacement,bytes(g.emu.mem_read(g.menu,0x3200)));g.menu=replacement
    assert len(g.show())==2
    print(f'PASS: the kit\'s 68 floats before each of {len(g.kit_calls)} weight callbacks; {count} native callback cases (with the kit\'s movement skills, Bear Arms, the carry limit\'s base, skills and Packrat, and stamina use\'s 19 and 67), 3 equipped kits, config hooks, kit switching, compatibility, invalid config and owner isolation')

if __name__=='__main__': main()
